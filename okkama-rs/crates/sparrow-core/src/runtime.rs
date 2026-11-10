//! Runtime state shared with machine code, runtime helpers called from it, and
//! root enumeration for the collector.
//!
//! Generated code receives a [`Ctx`] pointer. A failure sets `status` (and
//! `failure`), and every caller checks it after each call or helper, so
//! failures unwind through machine frames without Rust unwinding. Helpers that
//! may allocate collect at their end when the heap asks for it; roots are the
//! machine frames above the helper (found through frame records and Cranelift
//! stack maps), runtime constants and the context.
#![allow(unsafe_code)]

use crate::Error;
use crate::heap::{self, Heap, Integer, Payload};
use crate::program::{Kind, Loaded};
use crate::budget::Exhausted;
use crate::word::{Repr, UNBOUND, Word};
use num_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};

pub const OK: u64 = 0;
/// A language failure is pending in `Ctx::failure`.
pub const FAILED: u64 = 1;
/// The engine stopped (unsupported construct, internal invariant).
pub const FATAL: u64 = 2;
/// A budget ran out or the host interrupted the run (`budget`); like
/// `FATAL`, it unwinds every activation without handlers.
pub const EXHAUSTED: u64 = 3;
/// Stack kept below the limit machine code checks: room for one frame
/// (`jit::MAX_FRAME`) and the Rust helpers it calls.
pub const STACK_HEADROOM: u64 = 1 << 20;

/// Shared with machine code; offsets come from `offset_of!`.
#[repr(C)]
#[derive(Debug)]
pub struct Ctx {
    pub status: u64,
    pub stack_limit: u64,
    pub failure: Word,
    pub runtime: *mut Runtime,
    /// Frame pointer of the Rust function that entered machine code; the
    /// collector's frame walk ends there.
    pub entry_frame: u64,
    /// Second result of helpers that return a flag (a found map value).
    pub scratch: Word,
}

/// A language failure value: what failed, where it passed, what caused it.
#[derive(Debug)]
pub struct Failure {
    pub kind: FailureKind,
    /// (function, pc) of each activation it unwound through, innermost first.
    pub trace: Vec<(u32, u32)>,
    pub cause: Option<Word>,
    pub secondary: Vec<Word>,
    last_frame: Option<(u32, u64)>,
}
#[derive(Debug)]
pub enum FailureKind {
    Cancelled,
    /// A `NativeFailure` object from a provider.
    Native(Word),
    Thrown(Word),
    Program(String),
    Panic(String),
}
impl Payload for Failure {
    fn trace(&self, visit: &mut dyn FnMut(Word, Repr)) {
        if let FailureKind::Thrown(value) | FailureKind::Native(value) = self.kind {
            visit(value, Repr::Ref);
        }
        for &word in self.cause.iter().chain(&self.secondary) {
            visit(word, Repr::Ref);
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
pub struct Runtime {
    pub heap: Heap,
    pub loaded: Loaded,
    pub jit: crate::jit::Jit,
    /// Code address per function; 0 until compiled on its first call.
    pub code: Box<[u64]>,
    /// Constants embedded in machine code; always roots.
    pub constants: Vec<(Word, Repr)>,
    /// Values held by Rust while machine code runs (entry arguments, results).
    pub roots: Vec<(Word, Repr)>,
    /// `const` arrays and maps (#153), built at their first read; each is
    /// then also in `constants`, so it stays a root.
    pub globals: Vec<Option<Word>>,
    pub fatal: Option<String>,
    /// The last failure `describe` reported, as the OCS protocol sees it.
    pub uncaught: Option<crate::Uncaught>,
    pub resources: crate::failures::Resources,
    /// Async frames ready to resume; spawned tasks; frames running on the
    /// machine stack (innermost last). All are roots.
    pub ready: std::collections::VecDeque<Word>,
    pub spawned: Vec<Word>,
    pub running: Vec<Word>,
    /// Settled Futures so far (their completion order).
    pub completions: u64,
    /// Sleeping drivers by deadline; roots.
    pub timers: Vec<(std::time::Instant, Word)>,
    /// Async primitive state by owning object (see `primitives`).
    pub states: crate::word::FastMap<Word, crate::primitives::State>,
    /// Native provider of each function, resolved once.
    pub natives: Vec<Option<okkama_runtime_abi::Native>>,
    /// Intrinsic of each function, resolved once.
    pub intrinsics: Vec<Option<crate::intrinsics::Intrinsic>>,
    /// Native provider state; taken out while a provider runs.
    pub(crate) host: Option<Box<crate::host::Heap>>,
    pub io: crate::io::Io,
    /// Event counts by name when `SPARROW_CORE_STATS` is set (diagnostics).
    pub stats: Option<Box<std::collections::HashMap<String, u64>>>,
    /// When this runtime was made: `--stats` reports the run's wall time, so
    /// machine code ≈ run − compile − collection (#55).
    created: std::time::Instant,
    /// The command (`sparrow-next`, FILE.spb) process workers run; empty
    /// when the embedder configured none.
    pub worker_command: Vec<std::ffi::OsString>,
    /// Budgets of the next run (`budget::Limits`); set before it starts.
    pub limits: crate::budget::Limits,
    /// A native panicked: the runtime's state is unknown and it runs nothing more.
    pub poisoned: bool,
    pub(crate) budget: Budget,
}

/// Budget state of the current run.
#[derive(Debug, Default)]
pub(crate) struct Budget {
    pub exhausted: Option<Exhausted>,
    steps_left: Option<u64>,
    /// Size of the slice given at the last refill.
    slice: u64,
    meter: Option<crate::budget::Meter>,
    pub written: u64,
    /// Steps left in the current slice; machine code polls below zero.
    pub fuel: crate::budget::Fuel,
    /// The memory budget caps the machine stack: overflowing it is running
    /// out of memory, as on the IR engine and the JVM (#241).
    pub stack_capped: bool,
}

/// Frame pointer of the calling function's own frame record.
macro_rules! frame_pointer {
    () => {{
        let frame: u64;
        // SAFETY: reads the frame pointer register of the current function.
        #[cfg(target_arch = "aarch64")]
        unsafe {
            std::arch::asm!("mov {}, x29", out(reg) frame)
        };
        #[cfg(target_arch = "x86_64")]
        unsafe {
            std::arch::asm!("mov {}, rbp", out(reg) frame)
        };
        frame
    }};
}

impl Runtime {
    pub fn new(loaded: Loaded) -> Result<Self, Error> {
        crate::contracts::check(&loaded)?;
        let mut heap = Heap::new(loaded.layouts());
        heap.set_stress(std::env::var_os("SPARROW_CORE_GC_STRESS").is_some_and(|value| value == "1"));
        if std::env::var_os("SPARROW_CORE_STATS").is_some() {
            heap.allocations = Some(Box::default());
        }
        let functions = loaded.program.functions.len();
        let resources = crate::failures::Resources::new(&loaded.program);
        let intrinsics = loaded
            .program
            .functions
            .iter()
            .map(|f| f.intrinsic.as_ref().and_then(|i| crate::intrinsics::Intrinsic::resolve(&i.name)))
            .collect();
        Ok(Self {
            resources,
            ready: std::collections::VecDeque::new(),
            spawned: Vec::new(),
            running: Vec::new(),
            completions: 0,
            timers: Vec::new(),
            states: crate::word::FastMap::default(),
            natives: loaded
                .program
                .functions
                .iter()
                .map(|f| f.native.as_deref().and_then(okkama_runtime_abi::Native::resolve))
                .collect(),
            intrinsics,
            host: Some(Box::new(crate::host::Heap::new(&loaded.program))),
            io: crate::io::Io::default(),
            stats: std::env::var_os("SPARROW_CORE_STATS").map(|_| Box::default()),
            created: std::time::Instant::now(),
            worker_command: Vec::new(),
            limits: crate::budget::Limits::default(),
            poisoned: false,
            budget: Budget::default(),
            heap,
            jit: crate::jit::Jit::new()?,
            code: vec![0; functions].into_boxed_slice(),
            globals: vec![None; loaded.program.globals.len()],
            loaded,
            constants: Vec::new(),
            roots: Vec::new(),
            fatal: None,
            uncaught: None,
        })
    }

    /// Collect when the heap asks for it. `frame` is the frame pointer of a
    /// runtime helper called directly from machine code; `extra` are words
    /// the helper still holds.
    pub fn safepoint(&mut self, frame: u64, ctx: &Ctx, extra: &[(Word, Repr)]) {
        if !self.heap.needs_collection() {
            return;
        }
        self.count(|_| "gc collection".into());
        let jit = &self.jit;
        let constants = &self.constants;
        let roots = &self.roots;
        let tasks: Vec<Word> = self
            .ready
            .iter()
            .chain(&self.spawned)
            .chain(&self.running)
            .chain(self.timers.iter().map(|(_, frame)| frame))
            .chain(self.io.requests.keys())
            .copied()
            .collect();
        let states = &self.states;
        self.heap.collect(|visit| {
            for state in states.values() {
                state.trace(visit);
            }
            // SAFETY: frame records above a helper called from machine code
            // lead through JIT frames to the Rust entry.
            unsafe { jit.frame_roots(frame, ctx.entry_frame, visit) };
            for &(word, repr) in constants.iter().chain(roots).chain(extra) {
                visit(word, repr);
            }
            visit(ctx.failure, Repr::Ref);
            for &task in &tasks {
                visit(task, Repr::Ref);
            }
        });
        let released = self.heap.take_released();
        self.resources.forget(&released);
        for object in &released {
            self.states.remove(object);
        }
        if let Some(host) = &mut self.host {
            host.released(&released);
        }
    }

    /// Count an event (only when statistics are on).
    #[inline]
    pub fn count(&mut self, event: impl FnOnce(&Runtime) -> String) {
        if self.stats.is_some() {
            let key = event(self);
            if let Some(stats) = &mut self.stats {
                *stats.entry(key).or_default() += 1;
            }
        }
    }

    /// The statistics report, most frequent first.
    pub fn stats_report(&self) -> Option<String> {
        let stats = self.stats.as_ref()?;
        let heap = self.heap.stats();
        let extra = [
            ("heap bytes allocated".to_string(), heap.allocated_bytes as u64),
            ("heap live bytes, mean after collection".to_string(), (heap.live_sum / heap.collections.max(1)) as u64),
            ("collections".to_string(), heap.collections as u64),
            // Rust memory owned by heap objects, estimated (#331).
            ("owned Rust bytes charged, estimate".to_string(), heap.external_allocated_bytes as u64),
            ("owned Rust bytes live after the last collection, estimate".to_string(), heap.external_live_bytes as u64),
            ("collections triggered by owned Rust memory".to_string(), heap.external_collections as u64),
        ];
        let helpers = self.jit.helper_calls();
        // Runtime allocations by kind (objects by type) and collection time.
        let program = &self.loaded.program;
        let allocations: Vec<(String, u64)> = self
            .heap
            .allocations
            .iter()
            .flat_map(|allocations| {
                allocations
                    .kinds
                    .iter()
                    .flat_map(|(&(kind, descriptor), &(count, bytes))| {
                        let what = match kind {
                            "object" => format!("object {}", program.objects.get(descriptor as usize).map_or("?", |object| object.name.as_str())),
                            other => other.to_string(),
                        };
                        [(format!("alloc runtime {what}, count"), count), (format!("alloc runtime {what}, bytes"), bytes)]
                    })
                    .chain([
                        ("collection, microseconds".to_string(), allocations.collection_nanos / 1000),
                        ("run, microseconds".to_string(), self.created.elapsed().as_micros() as u64),
                    ])
            })
            .collect();
        let mut entries: Vec<_> = stats
            .iter()
            .chain(extra.iter().map(|(k, v)| (k, v)))
            .chain(helpers.iter().map(|(k, v)| (k, v)))
            .chain(allocations.iter().map(|(k, v)| (k, v)))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        Some(entries.iter().map(|(k, v)| format!("{v:>12} {k}\n")).collect())
    }

    pub fn failure_new(&mut self, kind: FailureKind) -> Word {
        self.heap.allocate_payload(Box::new(Failure {
            kind,
            trace: Vec::new(),
            cause: None,
            secondary: Vec::new(),
            last_frame: None,
        }))
    }

    pub fn failure(&self, word: Word) -> Option<&Failure> {
        self.heap.payload::<Failure>(word)
    }

    fn integer_value(&self, word: Word) -> BigInt {
        match self.heap.integer(word) {
            Integer::Small(n) => BigInt::from(n),
            Integer::Big(n) => n.clone(),
        }
    }
}

// ---- helpers called from machine code ----
//
// SAFETY (all helpers): `ctx` is the context the generated code was entered
// with; its runtime pointer is valid and not otherwise borrowed while machine
// code runs.

unsafe fn runtime<'a>(ctx: *mut Ctx) -> (&'a mut Ctx, &'a mut Runtime) {
    let ctx = unsafe { &mut *ctx };
    let runtime = unsafe { &mut *ctx.runtime };
    (ctx, runtime)
}

fn fail(ctx: &mut Ctx, runtime: &mut Runtime, kind: FailureKind) -> Word {
    ctx.failure = runtime.failure_new(kind);
    ctx.status = FAILED;
    0
}

pub(crate) fn fatal(ctx: &mut Ctx, runtime: &mut Runtime, message: String) -> Word {
    if runtime.fatal.is_none() {
        runtime.fatal = Some(message);
    }
    ctx.status = FATAL;
    0
}

/// Slow path of inline allocation: collect if due, then a cell of `words`.
/// Under a memory budget, 0 when the allocation would cross it (status
/// `EXHAUSTED`); machine code then returns.
#[inline(never)]
pub extern "C" fn rt_allocate(ctx: *mut Ctx, words: u64) -> Word {
    let frame = frame_pointer!();
    let (ctx, runtime) = unsafe { runtime(ctx) };
    runtime.count(|_| "allocation slow path".into());
    if let (Some(limit), Some(meter)) = (runtime.limits.memory, &runtime.budget.meter) {
        // A small cell comes from a new page when its free list is empty.
        let bytes = match Heap::class_of(words as usize) {
            Some(_) => crate::heap::PAGE_BYTES,
            None => words as usize * 8,
        };
        if meter.used().saturating_add(bytes as isize) > limit as isize {
            return runtime.exhaust(ctx, Exhausted::Memory);
        }
    }
    runtime.safepoint(frame, ctx, &[]);
    runtime.heap.allocate(words as usize)
}

/// A step slice is used up (or the run must stop): charge it, check the
/// other budgets and the interrupt flag, and give the next slice.
#[inline(never)]
pub extern "C" fn rt_poll(ctx: *mut Ctx) {
    let (ctx, runtime) = unsafe { runtime(ctx) };
    if let Some(what) = runtime.over_budget() {
        runtime.exhaust(ctx, what);
        return;
    }
    match runtime.refill() {
        // The step that polled is the first of the new slice.
        Some(fuel) => runtime.budget.fuel.set(fuel - 1),
        None => {
            runtime.exhaust(ctx, Exhausted::Steps);
        }
    }
}

impl Runtime {
    /// Stop the run because `what` ran out; the first reason is kept.
    pub(crate) fn exhaust(&mut self, ctx: &mut Ctx, what: Exhausted) -> Word {
        self.budget.exhausted.get_or_insert(what);
        ctx.status = EXHAUSTED;
        self.budget.fuel.set(0);
        0
    }

    /// Mark the memory budget exhausted from code without a context (a
    /// native): the run stops at its next poll.
    pub(crate) fn refuse_memory(&mut self) {
        self.budget.exhausted.get_or_insert(Exhausted::Memory);
        self.budget.fuel.set(0);
    }

    /// Would `bytes` more cross the memory budget?
    pub(crate) fn would_exceed(&self, bytes: u64) -> bool {
        match (self.limits.memory, &self.budget.meter) {
            (Some(limit), Some(meter)) => meter.used().saturating_add(bytes.min(isize::MAX as u64) as isize) > limit as isize,
            _ => false,
        }
    }

    /// The reason to stop at a poll, other than steps.
    pub(crate) fn over_budget(&self) -> Option<Exhausted> {
        if let Some(what) = self.budget.exhausted {
            return Some(what);
        }
        if self.limits.interrupted() {
            return Some(Exhausted::Interrupted);
        }
        match (self.limits.memory, &self.budget.meter) {
            (Some(limit), Some(meter)) if meter.used() > limit as isize => Some(Exhausted::Memory),
            _ => None,
        }
    }

    /// Charge the slice just used and size the next; None when the step
    /// budget is spent.
    fn refill(&mut self) -> Option<i64> {
        let Some(left) = &mut self.budget.steps_left else {
            return Some(crate::budget::SLICE as i64);
        };
        *left = left.saturating_sub(self.budget.slice);
        if *left == 0 {
            return None;
        }
        self.budget.slice = (*left).min(crate::budget::SLICE);
        Some(self.budget.slice as i64)
    }

    /// Start the budgets of a run: the first slice of steps.
    fn start_budget(&mut self) -> Result<i64, Error> {
        if let Some(limit) = self.limits.memory {
            let probe = crate::budget::memory_probe().ok_or_else(|| {
                Error("a memory budget needs a memory probe (sparrow_core::budget::set_memory_probe)".into())
            })?;
            let meter = *self.budget.meter.get_or_insert_with(|| crate::budget::Meter::start(probe));
            self.heap.watch = Some(crate::budget::Watch::new(meter, limit, &self.budget.fuel));
        }
        Ok(match self.limits.steps {
            None => crate::budget::SLICE as i64,
            Some(steps) => {
                self.budget.steps_left = Some(steps);
                self.budget.slice = steps.min(crate::budget::SLICE);
                self.budget.slice as i64
            }
        })
    }

    /// The report of an exhausted run.
    fn exhausted_report(&mut self) -> Error {
        let message = self.budget.exhausted.unwrap_or(Exhausted::Interrupted).message();
        self.uncaught = Some(crate::Uncaught { category: "exhausted", error_type: None, message: message.into(), trace: Vec::new() });
        Error(message.into())
    }
}

/// Compile a function on its first call; returns its code address.
#[inline(never)]
pub extern "C" fn rt_compile(ctx: *mut Ctx, function: u64) -> u64 {
    let (ctx, runtime) = unsafe { runtime(ctx) };
    match crate::jit::compile(runtime, function as usize) {
        Ok(code) => code,
        Err(error) => fatal(ctx, runtime, error.0),
    }
}

pub const INT_ADD: u64 = 0;
pub const INT_SUB: u64 = 1;
pub const INT_MUL: u64 = 2;
pub const INT_MOD: u64 = 3;

/// Exact integer arithmetic beyond the inline fast path.
#[inline(never)]
pub extern "C" fn rt_int(ctx: *mut Ctx, op: u64, left: Word, right: Word) -> Word {
    let frame = frame_pointer!();
    let (ctx, runtime) = unsafe { runtime(ctx) };
    // Borrow BigInt operands instead of cloning them; inline ones are widened.
    fn widen(n: Integer<'_>) -> std::borrow::Cow<'_, BigInt> {
        match n {
            Integer::Small(n) => std::borrow::Cow::Owned(BigInt::from(n)),
            Integer::Big(n) => std::borrow::Cow::Borrowed(n),
        }
    }
    let (l, r) = (widen(runtime.heap.integer(left)), widen(runtime.heap.integer(right)));
    let result = match op {
        INT_ADD => &*l + &*r,
        INT_SUB => &*l - &*r,
        INT_MUL => {
            // The product's size is known before the (long) multiplication.
            if runtime.would_exceed((l.bits() + r.bits()) / 8) {
                return runtime.exhaust(ctx, Exhausted::Memory);
            }
            &*l * &*r
        }
        _ => {
            if r.is_zero() {
                return fail(ctx, runtime, FailureKind::Program("Modulo by zero".into()));
            }
            &*l % &*r
        }
    };
    let word = runtime.heap.big(result);
    if runtime.heap.needs_collection() {
        runtime.safepoint(frame, ctx, &[(word, Repr::Int)]);
    }
    word
}

/// Integer order beyond two inline integers: -1, 0 or 1.
#[inline(never)]
pub extern "C" fn rt_int_order(ctx: *mut Ctx, left: Word, right: Word) -> i64 {
    let (_, runtime) = unsafe { runtime(ctx) };
    match runtime.integer_value(left).cmp(&runtime.integer_value(right)) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

#[inline(never)]
pub extern "C" fn rt_concat(ctx: *mut Ctx, left: Word, right: Word) -> Word {
    let frame = frame_pointer!();
    let (ctx, runtime) = unsafe { runtime(ctx) };
    // The size of what is allocated is known before the copy (#72): a
    // memory budget stops it here.
    let Some(word) = crate::ops::concat_text(runtime, left, right, |runtime, bytes| !runtime.would_exceed(bytes as u64)) else {
        return runtime.exhaust(ctx, Exhausted::Memory);
    };
    runtime.safepoint(frame, ctx, &[(word, Repr::Ref)]);
    word
}

/// `rt_array` representation argument for a packed byte array.
pub const ARRAY_BYTES: u64 = 3;

/// Array from `count` words at `elements` (a stack slot of the caller).
#[inline(never)]
pub extern "C" fn rt_array(ctx: *mut Ctx, repr: u64, elements: *const Word, count: u64) -> Word {
    let frame = frame_pointer!();
    let (ctx, runtime) = unsafe { runtime(ctx) };
    let elements = unsafe { std::slice::from_raw_parts(elements, count as usize) };
    let word = if repr == ARRAY_BYTES {
        let bytes: Vec<u8> = elements.iter().map(|&w| w as u8).collect();
        runtime.heap.allocate_bytes(&bytes)
    } else {
        runtime.heap.allocate_array(Repr::from_bits(repr), elements)
    };
    runtime.safepoint(frame, ctx, &[(word, Repr::Ref)]);
    word
}

/// A program failure with a constant message string.
#[inline(never)]
pub extern "C" fn rt_trap(ctx: *mut Ctx, message: Word) -> Word {
    let (ctx, runtime) = unsafe { runtime(ctx) };
    let message = runtime.heap.text(message).to_owned();
    fail(ctx, runtime, FailureKind::Program(message))
}

/// The machine stack ran out at a function entry: the memory budget when it
/// caps the stack, otherwise the failure `Stack overflow`.
#[inline(never)]
pub extern "C" fn rt_stack_overflow(ctx: *mut Ctx) -> Word {
    let (ctx, runtime) = unsafe { runtime(ctx) };
    if runtime.budget.stack_capped {
        return runtime.exhaust(ctx, Exhausted::Memory);
    }
    fail(ctx, runtime, FailureKind::Program("Stack overflow".into()))
}

/// `throw value`.
#[inline(never)]
pub extern "C" fn rt_throw(ctx: *mut Ctx, value: Word) -> Word {
    let (ctx, runtime) = unsafe { runtime(ctx) };
    let header = runtime.heap.header_of(value);
    let native = heap::kind(header) == heap::OBJECT
        && runtime.loaded.program.objects[heap::payload(header) as usize].is_native_failure();
    fail(ctx, runtime, if native { FailureKind::Native(value) } else { FailureKind::Thrown(value) })
}

/// Record that the pending failure reaches `function` at `pc` in the
/// activation with frame pointer `frame`; each activation is recorded once, at
/// the pc where the failure first reached it (a rethrow does not add it again).
#[inline(never)]
pub extern "C" fn rt_trace(ctx: *mut Ctx, function: u64, pc: u64, frame: u64) {
    let (ctx, runtime) = unsafe { runtime(ctx) };
    let failure = ctx.failure;
    if let Some(failure) = runtime.heap.payload_mut::<Failure>(failure)
        && failure.last_frame != Some((function as u32, frame)) {
            failure.trace.push((function as u32, pc as u32));
            failure.last_frame = Some((function as u32, frame));
        }
}

/// Call a native provider with `count` argument words at `arguments`.
#[inline(never)]
pub extern "C" fn rt_native(ctx: *mut Ctx, function: u64, arguments: *const Word, count: u64) -> Word {
    let frame = frame_pointer!();
    let (ctx, runtime) = unsafe { runtime(ctx) };
    // SAFETY: the caller's argument slot outlives the call; natives do not collect.
    let arguments = unsafe { std::slice::from_raw_parts(arguments, count as usize) };
    runtime.count(|r| format!("native {}", r.loaded.program.functions[function as usize].symbol));
    // A panicking provider must not unwind through machine code: it stops the
    // run with an internal error and poisons the runtime (H-14).
    let called = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::natives::call(runtime, function as usize, arguments)
    }));
    let called = match called {
        Ok(called) => called,
        Err(payload) => {
            let symbol = runtime.loaded.program.functions[function as usize].symbol.clone();
            runtime.poisoned = true;
            return fatal(ctx, runtime, format!("internal error in native {symbol}: {}", panic_text(&*payload)));
        }
    };
    match called {
        Ok((word, repr)) => {
            runtime.safepoint(frame, ctx, &[(word, repr)]);
            word
        }
        Err(crate::natives::Refusal::Failure(kind)) => fail(ctx, runtime, kind),
        Err(crate::natives::Refusal::Raise(failure)) => {
            ctx.failure = failure;
            ctx.status = FAILED;
            0
        }
        Err(crate::natives::Refusal::Fatal(message)) => fatal(ctx, runtime, message),
        Err(crate::natives::Refusal::Exhausted(what)) => runtime.exhaust(ctx, what),
    }
}

impl Runtime {
    /// Run the synchronous entry function; returns its integer result.
    pub fn run_entry(self: &mut Box<Self>, stack: u64) -> Result<i64, Error> {
        let entry = self
            .loaded
            .program
            .entry
            .ok_or_else(|| Error("Sparrow program has no entry function".into()))?
            .0 as usize;
        let result = self.run_function(entry, stack)?;
        match self.heap.integer(result) {
            Integer::Small(n) => Ok(n),
            Integer::Big(n) => n.to_i64().ok_or_else(|| Error("entry result outside 0..255".into())),
        }
    }

    /// Run a parameterless function (sync or async) to completion; its
    /// result word, or the report of its failure.
    pub fn run_function(self: &mut Box<Self>, entry: usize, stack: u64) -> Result<Word, Error> {
        if !self.loaded.program.functions[entry].params.is_empty() {
            return Err(Error("parameterized entry is not supported".into()));
        }
        self.run_function_with(entry, &[], stack)
    }

    /// Run a function (sync or async) over `arguments` to completion: its
    /// result word, or the report of its failure (process workers).
    pub fn run_function_with(self: &mut Box<Self>, entry: usize, arguments: &[Word], stack: u64) -> Result<Word, Error> {
        if self.poisoned {
            return Err(Error("internal error: the runtime is unusable after a panic in a native".into()));
        }
        let function = &self.loaded.program.functions[entry];
        if function.params.len() != arguments.len() {
            return Err(Error("argument count mismatch".into()));
        }
        let asynchronous = function.is_async;
        let fuel = self.start_budget()?;
        self.budget.fuel.set(fuel);
        // A memory budget also bounds the machine stack.
        self.budget.stack_capped = self.limits.memory.is_some_and(|memory| memory.max(2 << 20) < stack);
        let stack = self.limits.memory.map_or(stack, |memory| stack.min(memory.max(2 << 20)));
        let marker = 0u8;
        let mut ctx = Ctx {
            status: OK,
            // R2: the VM thread has a large stack; keep a headroom below the limit.
            stack_limit: (&marker as *const u8 as u64).saturating_sub(stack.saturating_sub(STACK_HEADROOM)),
            failure: UNBOUND,
            runtime: &mut **self,
            entry_frame: frame_pointer!(),
            scratch: UNBOUND,
        };
        let result = if asynchronous {
            // The root Future, its task tree, then shutdown of spawned tasks.
            let root = self.future_new(entry, arguments);
            self.roots.push((root, Repr::Ref));
            unsafe { *((root + crate::tasks::STATE) as *mut u64) = crate::tasks::RUNNING };
            self.run_body(&mut ctx, root);
            if ctx.status == OK {
                self.schedule(&mut ctx, root);
            }
            if ctx.status == OK
                && let Some(failure) = self.shutdown(&mut ctx) {
                    let message = self.describe(&mut ctx, failure);
                    return Err(Error(message));
                }
            if ctx.status == OK {
                let state = unsafe { *((root + crate::tasks::STATE) as *const u64) };
                if state == crate::tasks::FAILURE_STATE {
                    ctx.failure = unsafe { *((root + crate::tasks::FAILURE) as *const u64) };
                    ctx.status = FAILED;
                }
            }
            unsafe { *((root + crate::tasks::RESULT) as *const u64) }
        } else {
            let code = rt_compile(&mut ctx, entry as u64);
            if ctx.status != OK {
                return Err(Error(self.fatal.clone().unwrap_or_default()));
            }
            // SAFETY: compiled functions take the context and their parameters.
            unsafe { crate::jit::invoke(code, &mut ctx, arguments) }
        };
        match ctx.status {
            OK => Ok(result),
            FAILED => {
                ctx.status = OK;
                let failure = ctx.failure;
                Err(Error(self.describe(&mut ctx, failure)))
            }
            EXHAUSTED => Err(self.exhausted_report()),
            _ => Err(Error(self.fatal.clone().unwrap_or_else(|| "engine failure".into()))),
        }
    }
}

pub(crate) use frame_pointer;

/// Representation of an argument or result kind for root reporting.
pub fn repr_of(kind: Option<Kind>) -> Repr {
    kind.map_or(Repr::Raw, Kind::repr)
}


/// Float operation whose inline result was not finite (or a zero divisor).
#[inline(never)]
pub extern "C" fn rt_float(ctx: *mut Ctx, op: u64, left: u64, right: u64) -> u64 {
    let (ctx, runtime) = unsafe { runtime(ctx) };
    match crate::scalar::float_binary(op, f64::from_bits(left), f64::from_bits(right)) {
        Ok(value) => value.to_bits(),
        Err(message) => fail(ctx, runtime, FailureKind::Program(message.into())),
    }
}

/// A closure of `function` capturing `count` words at `captures`.
#[inline(never)]
pub extern "C" fn rt_closure(ctx: *mut Ctx, function: u64, captures: *const Word, count: u64) -> Word {
    let frame = frame_pointer!();
    let (ctx, runtime) = unsafe { runtime(ctx) };
    let captures = unsafe { std::slice::from_raw_parts(captures, count as usize) };
    let closure = runtime.heap.closure_new(function as u32, captures);
    runtime.safepoint(frame, ctx, &[(closure, Repr::Ref)]);
    closure
}

/// Call a closure with `count` argument words: its captures fill the leading
/// parameters of its function.
#[inline(never)]
pub extern "C" fn rt_call_closure(ctx: *mut Ctx, closure: Word, arguments: *const Word, count: u64) -> Word {
    let (ctx_ref, runtime) = unsafe { runtime(ctx) };
    runtime.count(|_| "closure call".into());
    let function = heap::payload(runtime.heap.header_of(closure)) as usize;
    let params = runtime.loaded.program.functions[function].params.len();
    let captured = params - count as usize;
    let mut words = [UNBOUND; 8];
    if params > words.len() {
        return fatal(ctx_ref, runtime, "closure with more than 8 parameters".into());
    }
    for (i, word) in words.iter_mut().enumerate().take(captured) {
        *word = runtime.heap.field(closure, i);
    }
    words[captured..params].copy_from_slice(unsafe { std::slice::from_raw_parts(arguments, count as usize) });
    if runtime.loaded.program.functions[function].is_async {
        // Calling an async action makes its cold Future.
        return runtime.future_new(function, &words[..params]);
    }
    let code = match runtime.code[function] {
        0 => match crate::jit::compile(runtime, function) {
            Ok(code) => code,
            Err(error) => return fatal(ctx_ref, runtime, error.0),
        },
        code => code,
    };
    // SAFETY: compiled functions take the context and `params` words.
    unsafe { crate::jit::invoke(code, ctx, &words[..params]) }
}

/// Look a key up: 1 and the value in `scratch` when present, else 0.
#[inline(never)]
pub extern "C" fn rt_map_find(ctx: *mut Ctx, function: u64, pc: u64, map: Word, key: Word) -> u64 {
    let (ctx, runtime) = unsafe { runtime(ctx) };
    match crate::ops::map_find(runtime, function as usize, pc as usize, map, key) {
        Some(value) => {
            ctx.scratch = value;
            1
        }
        None => 0,
    }
}

/// The message of a panic payload.
pub(crate) fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|text| text.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic".into())
}

#[cfg(test)]
mod embedding_tests {
    use super::*;
    use crate::budget::{Limits, NativeGroup};
    use okkama_sparrow::{Dst, FuncRef, Function, Instruction as I, Program, Register, Type, TypeRef};

    /// `main` returns the wall clock: one native of the Clock group.
    fn clock() -> Box<Runtime> {
        let native = Function {
            symbol: "runtime/time@1::wall_clock/0".into(),
            is_async: false,
            native: Some("runtime/time@1::wall_clock/0".into()),
            intrinsic: None,
            registers: Vec::new(),
            params: Vec::new(),
            result: Some(TypeRef(0)),
            code: Vec::new(),
            handlers: Vec::new(),
        };
        let main = Function {
            symbol: "main".into(),
            is_async: false,
            native: None,
            intrinsic: None,
            registers: vec![TypeRef(0)],
            params: Vec::new(),
            result: Some(TypeRef(0)),
            code: vec![
                I::Call { dst: Some(Dst(Register(0))), function: FuncRef(0), args: Vec::new() },
                I::Return { value: Some(Register(0)) },
            ],
            handlers: Vec::new(),
        };
        let program = Program {
            module: "app".into(),
            version: "1.0.0".into(),
            types: vec![Type::Int],
            functions: vec![native, main],
            entry: Some(FuncRef(1)),
            ..Program::default()
        };
        program.verify().unwrap();
        Box::new(Runtime::new(Loaded::new(program)).unwrap())
    }

    #[test]
    fn a_native_outside_the_permitted_groups_fails_the_program() {
        let mut runtime = clock();
        runtime.limits = Limits { natives: Some(vec![NativeGroup::Fs]), ..Limits::default() };
        let error = runtime.run_function(1, 1 << 24).unwrap_err();
        assert!(error.0.contains("Native runtime/time@1::wall_clock/0 needs the Clock permission"), "{}", error.0);
        let mut runtime = clock();
        runtime.limits = Limits { natives: Some(vec![NativeGroup::Clock]), ..Limits::default() };
        assert!(runtime.run_function(1, 1 << 24).is_ok());
    }

    #[test]
    fn a_panicking_native_is_an_internal_error_and_poisons_the_runtime() {
        let mut runtime = clock();
        crate::natives::PANIC_NEXT.with(|next| next.set(true));
        let error = runtime.run_function(1, 1 << 24).unwrap_err();
        assert!(error.0.contains("internal error in native runtime/time@1::wall_clock/0: provider failure"), "{}", error.0);
        assert!(runtime.poisoned);
        let again = runtime.run_function(1, 1 << 24).unwrap_err();
        assert!(again.0.contains("unusable"), "{}", again.0);
        // A fresh runtime in the same process works.
        assert!(clock().run_function(1, 1 << 24).is_ok());
    }
}
