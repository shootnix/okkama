//! Whole-function compilation to machine code (Cranelift), lazily on first call.
//!
//! Every function takes the context and its parameters as words and returns
//! one word (floats travel as bits). Calls go through the code table: a zero
//! entry compiles the callee first. Integer and reference registers are
//! declared for stack maps, so every call is a precise safepoint; the
//! collector walks frame records (frame pointers are preserved) and reads the
//! spilled slots of each frame from the map of its return address.
#![allow(unsafe_code)]

use crate::Error;
use crate::heap::{self, Heap};
use crate::intrinsics::Intrinsic;
use crate::program::Kind;
use crate::runtime::{self, Ctx, FAILED, OK, Runtime};
use crate::word::{self, Repr, Word};
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{AbiParam, Block, InstBuilder, MemFlagsData, Signature, StackSlotData, StackSlotKind, Value, types};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module, ModuleReloc, default_libcall_names};
use okkama_sparrow::{Cmp, Constant, Instruction as I, Target, Type};
use std::collections::BTreeMap;

/// What a runtime helper may do, beyond computing its result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Allocates heap objects.
    Allocates,
    /// May collect (a safepoint: the caller's frame is walked for roots).
    Collects,
    /// Runs machine code (compiled functions, destroys, async bodies).
    Reenters,
    /// May set a failure status, which machine code reads after the call or
    /// returns to its caller.
    Fails,
}

/// One description per helper (H-06): its Rust signature, checked at compile
/// time (the function must coerce to exactly that `extern "C"` pointer type;
/// every word is 8 bytes), the Cranelift signature derived from it, and its
/// effects. A panic inside a helper aborts with its message at the `extern
/// "C"` boundary and never unwinds through machine code.
macro_rules! helpers {
    ($($field:ident = $function:path as ($ctx:ident $(, $arg:ident: $ty:ty)*) $(-> $ret:ty)? ; [$($effect:ident),*];)*) => {
        #[derive(Debug)]
        struct Helpers { $($field: FuncId,)* }

        /// Every helper and its effects, in declaration order.
        pub const HELPERS: &[(&str, &[Effect])] = &[$((stringify!($field), &[$(Effect::$effect),*]),)*];

        $(
            const _: () = {
                $(assert!(std::mem::size_of::<$ty>() == 8);)*
                $(assert!(std::mem::size_of::<$ret>() == 8);)?
            };
            const _: extern "C" fn(*mut Ctx $(, $ty)*) $(-> $ret)? = $function;
        )*

        fn declare_helpers(builder: &mut JITBuilder) {
            $(builder.symbol(concat!("sparrow_core_", stringify!($field)), $function as *const u8);)*
        }

        fn import_helpers(module: &mut JITModule) -> Result<Helpers, Error> {
            Ok(Helpers { $($field: {
                let mut signature = module.make_signature();
                signature.params.push(AbiParam::new(types::I64));
                $(let _ = stringify!($arg); signature.params.push(AbiParam::new(types::I64));)*
                $(let _ = stringify!($ret); signature.returns.push(AbiParam::new(types::I64));)?
                module
                    .declare_function(concat!("sparrow_core_", stringify!($field)), Linkage::Import, &signature)
                    .map_err(|e| Error(format!("JIT helper: {e}")))?
            },)* })
        }

        impl Helpers {
            /// The ids in `HELPERS` order.
            fn ids(&self) -> Vec<FuncId> {
                vec![$(self.$field),*]
            }
        }
    };
}

helpers! {
    allocate = runtime::rt_allocate as (ctx, words: u64) -> Word; [Allocates, Collects];
    poll = runtime::rt_poll as (ctx); [Fails];
    compile = runtime::rt_compile as (ctx, function: u64) -> u64; [Fails];
    int = runtime::rt_int as (ctx, op: u64, left: Word, right: Word) -> Word; [Allocates, Collects, Fails];
    int_order = runtime::rt_int_order as (ctx, left: Word, right: Word) -> i64; [];
    concat = runtime::rt_concat as (ctx, left: Word, right: Word) -> Word; [Allocates, Collects, Fails];
    array = runtime::rt_array as (ctx, repr: u64, elements: *const Word, count: u64) -> Word; [Allocates, Collects];
    trap = runtime::rt_trap as (ctx, message: Word) -> Word; [Allocates, Fails];
    stack_overflow = runtime::rt_stack_overflow as (ctx) -> Word; [Allocates, Fails];
    throw = runtime::rt_throw as (ctx, value: Word) -> Word; [Allocates, Fails];
    trace = runtime::rt_trace as (ctx, function: u64, pc: u64, frame: u64); [];
    native = runtime::rt_native as (ctx, function: u64, arguments: *const Word, count: u64) -> Word; [Allocates, Collects, Fails];
    float = runtime::rt_float as (ctx, op: u64, left: u64, right: u64) -> u64; [Allocates, Fails];
    exec = crate::ops::rt_exec as (ctx, function: u64, pc: u64, uses: *const Word, registers: *const okkama_sparrow::Register, count: u64) -> Word; [Allocates, Collects, Reenters, Fails];
    closure = runtime::rt_closure as (ctx, function: u64, captures: *const Word, count: u64) -> Word; [Allocates, Collects];
    call_closure = runtime::rt_call_closure as (ctx, closure: Word, arguments: *const Word, count: u64) -> Word; [Allocates, Collects, Reenters, Fails];
    map_find = runtime::rt_map_find as (ctx, function: u64, pc: u64, map: Word, key: Word) -> u64; [];
    intrinsic = crate::intrinsics::rt_intrinsic as (ctx, function: u64, arguments: *const Word, count: u64) -> Word; [Allocates, Collects, Reenters, Fails];
    failure_op = crate::failures::rt_failure_op as (ctx, function: u64, pc: u64, uses: *const Word) -> Word; [Allocates, Collects, Reenters, Fails];
    catch = crate::failures::rt_catch as (ctx, function: u64, pc: u64, failure: Word) -> u64; [Allocates];
    created = crate::failures::rt_created as (ctx, descriptor: u64, object: Word); [];
    region_add = crate::failures::rt_region_add as (ctx, region: Word, value: Word, own: u64); [];
    builder_push = crate::ops::rt_builder_push as (ctx, builder: Word, item: Word) -> Word; [];
    trie_get = crate::ops::rt_trie_get as (ctx, array: Word, position: u64) -> Word; [Allocates];
    int_text = crate::ops::rt_int_text as (ctx, word: Word) -> Word; [Allocates, Collects];
    future_new = crate::tasks::rt_future_new as (ctx, function: u64, arguments: *const Word, count: u64) -> Word; [Allocates, Collects, Fails];
    await_ = crate::tasks::rt_await as (ctx, this: Word, future: Word) -> u64; [Collects, Reenters, Fails];
    region_settle = crate::tasks::rt_region_settle as (ctx, this: Word, region: Word, keep: Word, failure: Word) -> u64; [Allocates, Collects, Reenters, Fails];
    complete = crate::tasks::rt_complete as (ctx, frame: Word, value: Word); [Collects, Reenters];
    spawn = crate::tasks::rt_spawn as (ctx, future: Word) -> Word; [Reenters, Fails];
    fut = crate::tasks::rt_fut as (ctx, op: u64, future: Word) -> Word; [Reenters, Fails];
}

pub struct Jit {
    module: JITModule,
    helpers: Helpers,
    ids: BTreeMap<usize, FuncId>,
    /// Stack map of each call site: return address, the frame's FP-to-SP
    /// distance, and the SP offsets of its live words.
    maps: Vec<(u64, u32, Vec<u32>)>,
    /// Address ranges of compiled functions, sorted by start.
    ranges: Vec<(u64, u64)>,
    /// (role, slot) → function index by object descriptor (u64::MAX: none).
    dispatch: BTreeMap<(u32, u32), Box<[u64]>>,
    /// The registers each `rt_exec` call site passes, in visit order: the
    /// helper reads them instead of visiting the instruction on every call
    /// (#325). Machine code holds their addresses, so they live as long.
    operands: Vec<Box<[okkama_sparrow::Register]>>,
    /// Calls of each helper (`HELPERS` order) from code compiled under
    /// `--stats`; machine code increments them, other code does not count.
    helper_counts: Box<[u64]>,
    /// The largest machine frame compiled, bytes (`--stats`).
    largest_frame: u32,
    /// Functions whose CLIF and machine code go to stderr (`SPARROW_CORE_DUMP`).
    dump: Chosen,
    /// Functions compiled without Cranelift's optimizations, and the target
    /// that does it (`SPARROW_CORE_UNOPTIMIZED`).
    unoptimized: Chosen,
    unoptimized_isa: Option<std::sync::Arc<dyn cranelift_codegen::isa::TargetIsa>>,
    /// Functions compiled without the core's inline fast paths: each
    /// instruction that has one runs through the shared slow path instead
    /// (`SPARROW_CORE_PLAIN`).
    plain: Chosen,
    /// Functions whose stack maps Cranelift's frontend keeps, as before
    /// #354 (`SPARROW_CORE_FRONTEND_MAPS`).
    frontend_maps: Chosen,
    /// Allocations of each inline allocation site compiled under `--stats`:
    /// its label and the count and bytes machine code adds there (#55).
    alloc_sites: Vec<(String, Box<[u64; 2]>)>,
    /// Where compilation spends its time (`SPARROW_CORE_PROFILE_COMPILE=1`,
    /// #345): a line per function on stderr, and Cranelift's own passes,
    /// summed, when the JIT is dropped.
    profile: Option<cranelift_codegen::timing::PassTimes>,
}

impl Drop for Jit {
    fn drop(&mut self) {
        if let Some(passes) = &self.profile {
            eprintln!("; Cranelift passes, all compiled functions\n{passes}");
        }
    }
}

/// Functions chosen by symbol in an environment variable: a comma-separated
/// list, or `*` for all. Bisecting a wrong result over these lists narrows it
/// to one compiled function without rebuilding (#44).
#[derive(Debug, Default)]
struct Chosen {
    all: bool,
    symbols: std::collections::BTreeSet<String>,
}

impl Chosen {
    fn from_env(variable: &str) -> Self {
        let Some(text) = std::env::var_os(variable) else {
            return Self::default();
        };
        let text = text.to_string_lossy();
        let symbols: std::collections::BTreeSet<String> =
            text.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
        Self { all: symbols.contains("*"), symbols }
    }

    fn any(&self) -> bool {
        self.all || !self.symbols.is_empty()
    }

    fn has(&self, symbol: &str) -> bool {
        self.all || self.symbols.contains(symbol)
    }
}

/// The target for compiled code, at a Cranelift optimization level.
fn target(opt_level: &str) -> Result<std::sync::Arc<dyn cranelift_codegen::isa::TargetIsa>, Error> {
    let mut flags = settings::builder();
    let set = |flags: &mut settings::Builder, name: &str, value: &str| {
        flags.set(name, value).map_err(|e| Error(format!("JIT flag {name}: {e}")))
    };
    set(&mut flags, "opt_level", opt_level)?;
    set(&mut flags, "is_pic", "false")?;
    set(&mut flags, "use_colocated_libcalls", "false")?;
    // The collector walks frame records to find each frame's stack map.
    set(&mut flags, "preserve_frame_pointers", "true")?;
    // Cranelift's IR verifier checks the emitter, not the program: a quarter
    // of the compile time of a large async program (#345). Debug builds,
    // and so the gate's debug tests, keep it; `SPARROW_CORE_VERIFY=1` turns
    // it on in a release build.
    let verify = cfg!(debug_assertions) || std::env::var("SPARROW_CORE_VERIFY").is_ok_and(|value| value == "1");
    set(&mut flags, "enable_verifier", if verify { "true" } else { "false" })?;
    cranelift_native::builder()
        .map_err(|e| Error(format!("JIT target: {e}")))?
        .finish(settings::Flags::new(flags))
        .map_err(|e| Error(format!("JIT target: {e}")))
}

/// Largest machine frame of a compiled function: a quarter of the stack
/// headroom (`STACK_HEADROOM`), leaving the rest to the Rust helpers below the
/// deepest machine frame.
pub const MAX_FRAME: u64 = crate::runtime::STACK_HEADROOM / 4;

/// Memory compiling one function may take (#73). Machine-code liveness and
/// register allocation keep a set per Cranelift block, so the cost is about
/// Σ registers live at an instruction × blocks it emits — quadratic for k
/// values live across k fallible instructions. A memory budget lowers it.
pub const MAX_COMPILE_BYTES: u64 = 256 << 20;
/// Compiler bytes per live register in one block (measured: about 58).
const BYTES_PER_LIVE_BLOCK: u64 = 64;
/// No instruction emits more blocks than this (inline allocation, checks).
const MAX_BLOCKS_PER_INSTRUCTION: u64 = 16;

impl std::fmt::Debug for Jit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Jit")
            .field("functions", &self.ids.len())
            .field("safepoints", &self.maps.len())
            .finish_non_exhaustive()
    }
}

impl Jit {
    pub fn new() -> Result<Self, Error> {
        let isa = target("speed")?;
        let unoptimized = Chosen::from_env("SPARROW_CORE_UNOPTIMIZED");
        let unoptimized_isa = if unoptimized.any() { Some(target("none")?) } else { None };
        let mut builder = JITBuilder::with_isa(isa, default_libcall_names());
        builder.memory_provider(Box::new(crate::code_memory::CodeMemory::new()));
        declare_helpers(&mut builder);
        let mut module = JITModule::new(builder);
        let helpers = import_helpers(&mut module)?;
        Ok(Self {
            module,
            helpers,
            ids: BTreeMap::new(),
            maps: Vec::new(),
            ranges: Vec::new(),
            dispatch: BTreeMap::new(),
            operands: Vec::new(),
            helper_counts: vec![0; HELPERS.len()].into_boxed_slice(),
            largest_frame: 0,
            dump: Chosen::from_env("SPARROW_CORE_DUMP"),
            unoptimized,
            unoptimized_isa,
            plain: Chosen::from_env("SPARROW_CORE_PLAIN"),
            frontend_maps: Chosen::from_env("SPARROW_CORE_FRONTEND_MAPS"),
            alloc_sites: Vec::new(),
            profile: std::env::var("SPARROW_CORE_PROFILE_COMPILE")
                .is_ok_and(|value| !value.is_empty() && value != "0")
                .then(|| {
                    eprintln!("; compile\tfunction\tinstructions\tregisters\tsuspensions\tclif blocks\tclif instructions\tsafepoints\tcode bytes\tframe bytes\tdeclare\tliveness\temit\tfinalize\tcodegen\tmaps\tdefinitions\ttables (microseconds)");
                    cranelift_codegen::timing::PassTimes::default()
                }),
        })
    }

    /// `--stats` lines of the helpers machine code called, and the largest
    /// machine frame.
    pub(crate) fn helper_calls(&self) -> Vec<(String, u64)> {
        HELPERS
            .iter()
            .zip(self.helper_counts.iter())
            .filter(|(_, count)| **count > 0)
            .map(|((name, _), &count)| (format!("helper {name}"), count))
            .chain([("largest machine frame, bytes".to_string(), u64::from(self.largest_frame))])
            .chain(self.alloc_sites.iter().filter(|(_, counts)| counts[0] > 0).flat_map(|(site, counts)| {
                [(format!("alloc site {site}, count"), counts[0]), (format!("alloc site {site}, bytes"), counts[1])]
            }))
            .collect()
    }

    fn in_code(&self, address: u64) -> bool {
        let index = self.ranges.partition_point(|&(start, _)| start <= address);
        index > 0 && address < self.ranges[index - 1].1
    }

    /// Report the traced values of every machine frame above `frame`, the
    /// frame pointer of a runtime function called from machine code.
    ///
    /// SAFETY: frame records from `frame` up to `entry` are live frames; those
    /// returning into code compiled here have stack maps.
    pub(crate) unsafe fn frame_roots(&self, mut frame: u64, entry: u64, visit: &mut dyn FnMut(Word, Repr)) {
        // Rust frames between machine frames (a helper that calls a closure)
        // are passed over; the walk ends at the frame that entered machine code.
        while frame != 0 && frame != entry {
            let (caller, ret) = unsafe { (*(frame as *const u64), *((frame + 8) as *const u64)) };
            if !self.in_code(ret) {
                frame = caller;
                continue;
            }
            if let Ok(index) = self.maps.binary_search_by_key(&ret, |(address, _, _)| *address) {
                // The machine frame's SP at the call, from its own FP (`caller`):
                // the callee's frame record says nothing about where the caller's
                // SP was (Linux puts the record below the callee's saved registers).
                let sp = caller - u64::from(self.maps[index].1);
                for &offset in &self.maps[index].2 {
                    // Integer slots and pointers share one rule: a pointer is
                    // non-zero with a clear low bit.
                    visit(unsafe { *((sp + u64::from(offset)) as *const u64) }, Repr::Int);
                }
            }
            frame = caller;
        }
    }

    fn signature(&self, params: usize) -> Signature {
        let mut signature = self.module.make_signature();
        for _ in 0..=params {
            signature.params.push(AbiParam::new(types::I64));
        }
        signature.returns.push(AbiParam::new(types::I64));
        signature
    }
}

/// Call compiled code with the context and `words` as parameters.
///
/// SAFETY: `code` is a function compiled here taking exactly `words.len()`
/// parameters after the context.
pub(crate) unsafe fn invoke(code: u64, ctx: *mut Ctx, words: &[Word]) -> Word {
    type W = Word;
    let c = ctx;
    let w = words;
    unsafe {
        match w.len() {
            0 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx) -> W>(code)(c),
            1 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx, W) -> W>(code)(c, w[0]),
            2 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx, W, W) -> W>(code)(c, w[0], w[1]),
            3 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx, W, W, W) -> W>(code)(c, w[0], w[1], w[2]),
            4 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx, W, W, W, W) -> W>(code)(c, w[0], w[1], w[2], w[3]),
            5 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx, W, W, W, W, W) -> W>(code)(c, w[0], w[1], w[2], w[3], w[4]),
            6 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx, W, W, W, W, W, W) -> W>(code)(c, w[0], w[1], w[2], w[3], w[4], w[5]),
            7 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx, W, W, W, W, W, W, W) -> W>(code)(c, w[0], w[1], w[2], w[3], w[4], w[5], w[6]),
            8 => std::mem::transmute::<u64, extern "C" fn(*mut Ctx, W, W, W, W, W, W, W, W) -> W>(code)(c, w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7]),
            _ => {
                let runtime = &mut *(*c).runtime;
                crate::runtime::fatal(&mut *c, runtime, "closure with more than 8 parameters".into())
            }
        }
    }
}

/// Compile `function` unless it is compiled; returns its code address.
/// Instructions where control enters other than by falling through: the
/// first, every jump/branch/lookup/catch target, every handler target and
/// every `await` (its resume point). Only these get their own Cranelift
/// block: a block per instruction made stack-map liveness and register
/// allocation quadratic in straight-line code with many live values (#73).
fn leaders(source: &okkama_sparrow::Function) -> Vec<bool> {
    let mut leaders = vec![false; source.code.len()];
    let mut mark = |target: &Target| {
        if let Some(leader) = leaders.get_mut(target.pc as usize) {
            *leader = true;
        }
    };
    for instruction in &source.code {
        match instruction {
            I::Jump { target } => mark(target),
            I::Branch { then_target, else_target, .. } => {
                mark(then_target);
                mark(else_target);
            }
            I::Catch { handler, next, .. } => {
                mark(handler);
                mark(next);
            }
            I::ArrayLookup { found, missing, .. } | I::MapLookup { found, missing, .. } => {
                mark(found);
                mark(missing);
            }
            _ => {}
        }
    }
    for handler in &source.handlers {
        mark(&handler.target);
    }
    for (pc, instruction) in source.code.iter().enumerate() {
        if suspends(instruction) {
            leaders[pc] = true;
        }
    }
    if let Some(first) = leaders.first_mut() {
        *first = true;
    }
    leaders
}

pub fn compile(runtime: &mut Runtime, function: usize) -> Result<u64, Error> {
    if runtime.code[function] != 0 {
        return Ok(runtime.code[function]);
    }
    let Runtime { heap, loaded, jit, code, constants, resources, limits, budget, stats, .. } = runtime;
    let started = stats.is_some().then(std::time::Instant::now);
    // Stage boundaries under `SPARROW_CORE_PROFILE_COMPILE` (#345).
    let profiling = jit.profile.is_some();
    let mut marks: Vec<std::time::Instant> = Vec::new();
    macro_rules! mark {
        () => {
            if profiling {
                marks.push(std::time::Instant::now());
            }
        };
    }
    mark!();
    let program = &loaded.program;
    let source = &program.functions[function];
    let symbol = source.symbol.as_str();
    if source.is_async && (source.native.is_some() || source.intrinsic.is_some()) {
        return Err(Error(format!("{}: async natives and intrinsics are driven, not compiled", source.symbol)));
    }
    // An async body takes its frame (see `tasks`).
    let params = if source.is_async { 1 } else { source.params.len() };
    let id = match jit.ids.get(&function) {
        Some(id) => *id,
        None => {
            let signature = jit.signature(params);
            let id = jit
                .module
                .declare_function(&format!("f{function}"), Linkage::Local, &signature)
                .map_err(|e| Error(format!("JIT declare: {e}")))?;
            jit.ids.insert(function, id);
            id
        }
    };
    mark!();
    // Live registers per instruction, for the cost of large functions only:
    // a function whose instructions × registers × blocks cannot reach the
    // limit skips the analysis.
    let cost_limit = limits.memory.map_or(MAX_COMPILE_BYTES, |memory| memory.min(MAX_COMPILE_BYTES)) / BYTES_PER_LIVE_BLOCK;
    let bound = (source.code.len() as u64)
        .saturating_mul(source.registers.len() as u64)
        .saturating_mul(MAX_BLOCKS_PER_INSTRUCTION);
    let live = if bound > cost_limit {
        Some(okkama_sparrow::live_counts(source).ok_or_else(|| {
            Error(format!("{}: too large to compile ({} instructions, {} registers)", source.symbol, source.code.len(), source.registers.len()))
        })?)
    } else {
        None
    };
    mark!();
    let plain = jit.plain.has(symbol);
    // Explicit roots unless the analysis is too large (#354).
    let root_live = (!jit.frontend_maps.has(symbol)).then(|| okkama_sparrow::live_sets(source)).flatten();
    let config = jit.module.target_config();
    let mut context = jit.module.make_context();
    context.func.signature = jit.signature(params);
    let mut frontend = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut context.func, &mut frontend);
        let mut emit = Emitter {
            builder: &mut builder,
            heap,
            loaded,
            jit,
            code_table: code.as_ptr() as i64,
            constants,
            resources,
            function,
            kinds: &loaded.functions[function].kinds,
            vars: Vec::new(),
            blocks: Vec::new(),
            ctx: None,
            frame: None,
            polled: limits.polled(),
            count_helpers: stats.is_some(),
            fuel: budget.fuel.address(),
            refusable: limits.memory.is_some(),
            pc: 0,
            spill_area: None,
            live,
            cost: 0,
            cost_limit,
            plain,
            root_live,
            roots: None,
        };
        emit.body()?;
        mark!();
        builder.seal_all_blocks();
        builder.finalize(config);
    }
    mark!();
    let clif = (context.func.layout.blocks().count(), context.func.dfg.num_insts());
    let dump = jit.dump.has(symbol);
    if dump {
        // Callees by their CLIF names: the runtime's helpers and the program's functions.
        let callees: String = context
            .func
            .params
            .user_named_funcs()
            .values()
            .map(|name| {
                let id = FuncId::from_u32(name.index);
                let linkage = jit.module.declarations().get_function_decl(id).linkage_name(id).into_owned();
                let callee = match linkage.strip_prefix('f').and_then(|n| n.parse::<usize>().ok()) {
                    Some(n) => program.functions[n].symbol.clone(),
                    None => linkage,
                };
                format!(";   u{}:{} = {callee}\n", name.namespace, name.index)
            })
            .collect();
        eprintln!("; CLIF of {symbol} (f{function}), before Cranelift's passes\n{callees}{}", context.func.display());
        context.set_disasm(true);
    }
    match &jit.unoptimized_isa {
        Some(isa) if jit.unoptimized.has(symbol) => {
            // What `define_function` does, with the unoptimizing target.
            context.compile(&**isa, &mut Default::default()).map_err(|e| Error(format!("{symbol}: {:?}", e.inner)))?;
            let compiled = context.compiled_code().expect("compiled function");
            let alignment = u64::from(compiled.buffer.min_alignment);
            let relocs: Vec<ModuleReloc> =
                compiled.buffer.relocs().iter().map(|reloc| ModuleReloc::from_mach_reloc(reloc, &context.func, id)).collect();
            jit.module
                .define_function_bytes(id, alignment, compiled.code_buffer(), &relocs)
                .map_err(|e| Error(format!("{symbol}: {e:?}")))?;
        }
        _ => jit.module.define_function(id, &mut context).map_err(|e| Error(format!("{symbol}: {e:?}")))?,
    }
    mark!();
    let compiled = context.compiled_code().expect("compiled function");
    let maps: Vec<(u32, Vec<u32>)> = compiled
        .buffer
        .user_stack_maps()
        .iter()
        .map(|(ret, _, map)| (*ret, map.entries().map(|(_, offset)| offset).collect()))
        .collect();
    // Distance from the frame's FP down to its SP inside the body.
    let fp_to_sp = compiled.buffer.frame_layout().map_or(0, |layout| layout.frame_to_fp_offset);
    // The prologue compares SP with the stack limit after reserving the
    // frame; a frame within the headroom below that limit cannot reach the
    // guard page before the comparison fails (#25).
    if u64::from(fp_to_sp) > MAX_FRAME {
        return Err(Error(format!(
            "{}: machine frame of {fp_to_sp} bytes exceeds the {} KiB limit",
            source.symbol,
            MAX_FRAME >> 10
        )));
    }
    jit.largest_frame = jit.largest_frame.max(fp_to_sp);
    let size = compiled.code_info().total_size;
    mark!();
    jit.module
        .finalize_definitions()
        .map_err(|e| Error(format!("JIT finalize: {e}")))?;
    mark!();
    let safepoints = maps.len();
    let address = jit.module.get_finalized_function(id) as u64;
    for (ret, slots) in maps {
        jit.maps.push((address + u64::from(ret), fp_to_sp, slots));
    }
    jit.maps.sort_by_key(|(address, _, _)| *address);
    jit.ranges.push((address, address + u64::from(size)));
    jit.ranges.sort_unstable();
    code[function] = address;
    mark!();
    if let Some(passes) = jit.profile.as_mut() {
        passes.add(&cranelift_codegen::timing::take_current());
        let micros: Vec<String> = marks.windows(2).map(|pair| (pair[1] - pair[0]).as_micros().to_string()).collect();
        eprintln!(
            "; compile\t{symbol}\t{}\t{}\t{}\t{}\t{}\t{safepoints}\t{size}\t{fp_to_sp}\t{}",
            source.code.len(),
            source.registers.len(),
            source.code.iter().filter(|instruction| suspends(instruction)).count(),
            clif.0,
            clif.1,
            micros.join("\t")
        );
    }
    if dump && let Some(listing) = &compiled.vcode {
        eprintln!("; machine code of {symbol} at {address:#x}, {size} bytes, frame {fp_to_sp} bytes\n{listing}");
    }
    if let (Some(stats), Some(started)) = (stats, started) {
        let micros = started.elapsed().as_micros() as u64;
        for (key, value) in [
            (format!("compile {symbol}, microseconds"), micros),
            (format!("machine code {symbol}, bytes"), u64::from(size)),
            ("compile, microseconds".to_string(), micros),
            ("machine code, bytes".to_string(), u64::from(size)),
        ] {
            *stats.entry(key).or_default() += value;
        }
    }
    Ok(address)
}

struct Emitter<'a, 'b> {
    builder: &'a mut FunctionBuilder<'b>,
    heap: &'a mut Heap,
    loaded: &'a crate::program::Loaded,
    jit: &'a mut Jit,
    code_table: i64,
    constants: &'a mut Vec<(Word, Repr)>,
    resources: &'a crate::failures::Resources,
    function: usize,
    kinds: &'a [Kind],
    vars: Vec<Variable>,
    /// The block of each instruction where control enters other than by
    /// falling through (`leaders`); straight-line instructions share one.
    blocks: Vec<Option<Block>>,
    /// Registers live before each instruction, when the function is large
    /// enough to need the compile-cost check (#73).
    live: Option<Vec<u32>>,
    /// Σ live registers × blocks emitted so far, and its limit.
    cost: u64,
    cost_limit: u64,
    /// Instructions with an inline fast path take the shared slow path
    /// (`SPARROW_CORE_PLAIN`).
    plain: bool,
    ctx: Option<Value>,
    /// The frame variable of an async body.
    frame: Option<Variable>,
    /// Budget polls at entry and on back-edges (`budget`).
    polled: bool,
    /// Count helper calls (`--stats`).
    count_helpers: bool,
    /// Address of the runtime's fuel cell.
    fuel: i64,
    /// Allocation slow paths may refuse under a memory budget.
    refusable: bool,
    /// The instruction being emitted.
    pc: usize,
    /// Stack slot of `spill`, created on first use.
    spill_area: Option<cranelift_codegen::ir::StackSlot>,
    /// Registers live before each instruction, for explicit roots (#354);
    /// `None`: Cranelift's frontend keeps the stack maps.
    root_live: Option<Vec<Vec<u64>>>,
    roots: Option<Roots>,
}

/// GC roots kept by the core instead of Cranelift's frontend (#354). The
/// heap does not move, so a pointer never has to be reloaded after a
/// collection; what a collection needs is to find it. Every traced register
/// has a word in one frame slot, written at each definition of the
/// register, and a call reports the words of the registers live before its
/// instruction or written by it. The frontend's scheme stores a value at
/// its definition too, but reloads it at every use after any call, which
/// made more than half of the code of large async bodies.
///
/// No reported word may hold a stale pointer: a live register was written
/// on every path in this activation; a register the instruction may write
/// but that is not live before it is cleared when the instruction starts;
/// an async body clears every word at entry, as a resumption enters an
/// instruction in the middle.
struct Roots {
    slot: cranelift_codegen::ir::StackSlot,
    /// The word of each traced register.
    word: Vec<Option<u32>>,
    /// The word of an async body's frame.
    frame: Option<u32>,
    /// The words a call reports now.
    reported: Vec<u32>,
}



const MEMORY: MemFlagsData = MemFlagsData::trusted();

impl Emitter<'_, '_> {
    fn ctx(&self) -> Value {
        self.ctx.expect("context")
    }

    fn helper(&mut self, id: fn(&Helpers) -> FuncId, args: &[Value]) -> Option<Value> {
        let id = id(&self.jit.helpers);
        if self.count_helpers {
            let index = self.jit.helpers.ids().iter().position(|known| *known == id).expect("helper");
            let counter = self.iconst(&self.jit.helper_counts[index] as *const u64 as i64);
            let count = self.builder.ins().load(types::I64, MEMORY, counter, 0);
            let count = self.builder.ins().iadd_imm_s(count, 1);
            self.builder.ins().store(MEMORY, count, counter, 0);
        }
        let reference = self.jit.module.declare_func_in_func(id, self.builder.func);
        let call = self.builder.ins().call(reference, args);
        self.report_roots(call);
        self.builder.inst_results(call).first().copied()
    }

    /// The roots a collection in `call` finds: the words of [`Roots`].
    fn report_roots(&mut self, call: cranelift_codegen::ir::Inst) {
        if let Some(roots) = &self.roots {
            for &word in &roots.reported {
                self.builder.func.dfg.append_user_stack_map_entry(
                    call,
                    cranelift_codegen::ir::UserStackMapEntry { ty: types::I64, slot: roots.slot, offset: word * 8 },
                );
            }
        }
    }

    /// Instruction `pc` starts: what its calls report, and its written
    /// registers that are not live cleared, so that none holds a stale
    /// pointer.
    fn enter_roots(&mut self, pc: usize) {
        let (Some(roots), Some(live)) = (&self.roots, &self.root_live) else {
            return;
        };
        let source = &self.loaded.program.functions[self.function];
        let live_before = &live[pc];
        let is_live = |register: usize| live_before.get(register / 64).is_some_and(|word| word >> (register % 64) & 1 != 0);
        let mut reported: Vec<u32> = roots.frame.into_iter().collect();
        let mut cleared = Vec::new();
        for (register, word) in roots.word.iter().enumerate() {
            if let Some(word) = word
                && is_live(register)
            {
                reported.push(*word);
            }
        }
        for register in okkama_sparrow::written(source, pc) {
            if let Some(word) = roots.word[register.0 as usize]
                && !is_live(register.0 as usize)
            {
                reported.push(word);
                cleared.push(word);
            }
        }
        let slot = roots.slot;
        if !cleared.is_empty() {
            let zero = self.iconst(0);
            for word in cleared {
                self.builder.ins().stack_store(types::I64, zero, slot, (word * 8) as i32);
            }
        }
        if let Some(roots) = &mut self.roots {
            roots.reported = reported;
        }
    }

    fn iconst(&mut self, value: i64) -> Value {
        self.builder.ins().iconst(types::I64, value)
    }

    fn get(&mut self, register: &okkama_sparrow::Register) -> Value {
        self.builder.use_var(self.vars[register.0 as usize])
    }

    fn set(&mut self, register: &okkama_sparrow::Register, value: Value) {
        self.builder.def_var(self.vars[register.0 as usize], value);
        if let Some(roots) = &self.roots
            && let Some(word) = roots.word[register.0 as usize]
        {
            let slot = roots.slot;
            self.builder.ins().stack_store(types::I64, value, slot, (word * 8) as i32);
        }
    }

    /// A register as a word (floats as bits).
    fn word(&mut self, register: &okkama_sparrow::Register) -> Value {
        let value = self.get(register);
        if self.kinds[register.0 as usize] == Kind::Float {
            self.builder.ins().bitcast(types::I64, MemFlagsData::new(), value)
        } else {
            value
        }
    }

    /// Store a word into a register of its kind.
    fn set_word(&mut self, register: &okkama_sparrow::Register, value: Value) {
        let value = if self.kinds[register.0 as usize] == Kind::Float {
            self.builder.ins().bitcast(types::F64, MemFlagsData::new(), value)
        } else {
            value
        };
        self.set(register, value);
    }

    fn status_offset() -> i32 {
        std::mem::offset_of!(Ctx, status) as i32
    }

    /// After anything that can fail: on a pending failure go to the handler
    /// covering `pc`, otherwise record this activation and return.
    fn check(&mut self, pc: usize) {
        let ctx = self.ctx();
        let status = self.builder.ins().load(types::I64, MEMORY, ctx, Self::status_offset());
        let failed = self.builder.create_block();
        let next = self.builder.create_block();
        self.builder.ins().brif(status, failed, &[], next, &[]);
        self.builder.switch_to_block(failed);
        let is_failure = self.builder.ins().icmp_imm_s(IntCC::Equal, status, FAILED as i64);
        let recorded = self.builder.create_block();
        let fatal = self.builder.create_block();
        self.builder.ins().brif(is_failure, recorded, &[], fatal, &[]);
        self.builder.switch_to_block(fatal);
        let zero = self.iconst(0);
        self.builder.ins().return_(&[zero]);
        self.builder.switch_to_block(recorded);
        let function = self.iconst(self.function as i64);
        let at = self.iconst(pc as i64);
        let frame = self.builder.ins().get_frame_pointer(types::I64);
        self.helper(|h| h.trace, &[ctx, function, at, frame]);
        let handler = self.loaded.program.functions[self.function]
            .handlers
            .iter()
            .find(|h| (h.start as usize..h.end as usize).contains(&pc))
            .cloned();
        match handler {
            Some(handler) => {
                let failure = self.builder.ins().load(
                    types::I64,
                    MEMORY,
                    ctx,
                    std::mem::offset_of!(Ctx, failure) as i32,
                );
                let ok = self.iconst(OK as i64);
                self.builder.ins().store(MEMORY, ok, ctx, Self::status_offset());
                // Edge moves read the old registers; the failure is written last.
                self.jump_then(&handler.target, handler.failure, failure);
            }
            None => {
                let zero = self.iconst(0);
                self.builder.ins().return_(&[zero]);
            }
        }
        self.builder.switch_to_block(next);
    }

    /// One budget step (`budget`): below zero fuel calls `rt_poll`, and a
    /// stopped run returns from this activation like a fatal status.
    fn poll(&mut self) {
        let ctx = self.ctx();
        let cell = self.iconst(self.fuel);
        let fuel = self.builder.ins().load(types::I64, MEMORY, cell, 0);
        let left = self.builder.ins().iadd_imm_s(fuel, -1);
        self.builder.ins().store(MEMORY, left, cell, 0);
        let empty = self.builder.ins().icmp_imm_s(IntCC::SignedLessThan, left, 0);
        let slow = self.builder.create_block();
        let next = self.builder.create_block();
        self.builder.set_cold_block(slow);
        self.builder.ins().brif(empty, slow, &[], next, &[]);
        self.builder.switch_to_block(slow);
        self.helper(|h| h.poll, &[ctx]);
        self.return_unless_ok(next);
    }

    /// Continue at `next` when the status is OK, otherwise return 0.
    fn return_unless_ok(&mut self, next: Block) {
        let ctx = self.ctx();
        let status = self.builder.ins().load(types::I64, MEMORY, ctx, Self::status_offset());
        let stop = self.builder.create_block();
        self.builder.set_cold_block(stop);
        self.builder.ins().brif(status, stop, &[], next, &[]);
        self.builder.switch_to_block(stop);
        let zero = self.iconst(0);
        self.builder.ins().return_(&[zero]);
        self.builder.switch_to_block(next);
    }

    /// A refused allocation (0 under a memory budget) returns.
    fn refused(&mut self, cell: Value) {
        if !self.refusable {
            return;
        }
        let next = self.builder.create_block();
        let stop = self.builder.create_block();
        self.builder.set_cold_block(stop);
        self.builder.ins().brif(cell, next, &[], stop, &[]);
        self.builder.switch_to_block(stop);
        let zero = self.iconst(0);
        self.builder.ins().return_(&[zero]);
        self.builder.switch_to_block(next);
    }

    /// The block of a leader instruction.
    fn block(&self, pc: usize) -> Block {
        self.blocks[pc].expect("a jump, handler or resume target starts a block")
    }

    fn jump(&mut self, target: &Target) {
        if self.polled && target.pc as usize <= self.pc {
            self.poll();
        }
        let values: Vec<_> = target.moves.iter().map(|(_, source)| self.get(source)).collect();
        for ((dst, _), value) in target.moves.iter().zip(values) {
            self.set(dst, value);
        }
        let block = self.block(target.pc as usize);
        self.builder.ins().jump(block, &[]);
    }

    /// Inline allocation of `words` words from the free list of their class.
    fn allocate(&mut self, words: usize) -> Value {
        if self.count_helpers {
            self.count_allocation(words);
        }
        let ctx = self.ctx();
        let Some(class) = Heap::class_of(words) else {
            let size = self.iconst(words as i64);
            let cell = self.helper(|h| h.allocate, &[ctx, size]).expect("result");
            self.refused(cell);
            return cell;
        };
        let head = self.heap.free_head(class) as i64;
        let head = self.iconst(head);
        let cell = self.builder.ins().load(types::I64, MEMORY, head, 0);
        let fast = self.builder.create_block();
        let slow = self.builder.create_block();
        let done = self.builder.create_block();
        self.builder.append_block_param(done, types::I64);
        self.builder.ins().brif(cell, fast, &[], slow, &[]);
        self.builder.switch_to_block(fast);
        let next = self.builder.ins().load(types::I64, MEMORY, cell, 8);
        self.builder.ins().store(MEMORY, next, head, 0);
        self.builder.ins().jump(done, &[cell.into()]);
        self.builder.switch_to_block(slow);
        let size = self.iconst(words as i64);
        let fresh = self.helper(|h| h.allocate, &[ctx, size]).expect("result");
        self.refused(fresh);
        self.builder.ins().jump(done, &[fresh.into()]);
        self.builder.switch_to_block(done);
        self.builder.block_params(done)[0]
    }

    /// Count an allocation of `words` words at this instruction (`--stats`):
    /// the site is the function, the instruction and, for objects, the type.
    fn count_allocation(&mut self, words: usize) {
        let program = &self.loaded.program;
        let source = &program.functions[self.function];
        let instruction = &source.code[self.pc];
        let what = match instruction {
            I::ObjNew { object, .. } => format!("{} {}", instruction.mnemonic(), program.objects[object.0 as usize].name),
            _ => instruction.mnemonic().to_string(),
        };
        let label = format!("{}:{} {what}", source.symbol, self.pc);
        let counts = Box::new([0u64; 2]);
        let address = counts.as_ptr() as i64;
        self.jit.alloc_sites.push((label, counts));
        let address = self.iconst(address);
        for (offset, step) in [(0, 1), (8, words as i64 * 8)] {
            let value = self.builder.ins().load(types::I64, MEMORY, address, offset);
            let value = self.builder.ins().iadd_imm_s(value, step);
            self.builder.ins().store(MEMORY, value, address, offset);
        }
    }

    fn constant_text(&mut self, text: &str) -> Value {
        let word = self.heap.allocate_text(text);
        self.constants.push((word, Repr::Ref));
        self.iconst(word as i64)
    }

    /// Call `function` through the code table, compiling it first if needed.
    fn call(&mut self, pc: usize, function: Value, args: &[Value]) -> Value {
        let ctx = self.ctx();
        let base = self.iconst(self.code_table);
        let offset = self.builder.ins().ishl_imm_u(function, 3);
        let entry = self.builder.ins().iadd(base, offset);
        let code = self.builder.ins().load(types::I64, MEMORY, entry, 0);
        let ready = self.builder.create_block();
        let compile = self.builder.create_block();
        self.builder.append_block_param(ready, types::I64);
        self.builder.ins().brif(code, ready, &[code.into()], compile, &[]);
        self.builder.switch_to_block(compile);
        let compiled = self.helper(|h| h.compile, &[ctx, function]).expect("result");
        self.check(pc);
        self.builder.ins().jump(ready, &[compiled.into()]);
        self.builder.switch_to_block(ready);
        let code = self.builder.block_params(ready)[0];
        let signature = self.jit.signature(args.len());
        let signature = self.builder.import_signature(signature);
        let mut values = vec![ctx];
        values.extend_from_slice(args);
        let call = self.builder.ins().call_indirect(signature, code, &values);
        self.report_roots(call);
        let result = self.builder.inst_results(call)[0];
        self.check(pc);
        result
    }

    /// Words of `registers` in the function's spill area; returns its address.
    /// One area serves every helper call (its contents are read before the
    /// call returns), sized for the widest: a slot per call site made the
    /// machine frame grow with the number of calls (#25).
    fn spill(&mut self, registers: &[okkama_sparrow::Register]) -> Value {
        let size = u32::try_from(registers.len().max(1).saturating_mul(8)).unwrap_or(u32::MAX);
        let slot = match self.spill_area {
            Some(slot) => {
                let data = &mut self.builder.func.sized_stack_slots[slot];
                data.size = data.size.max(size);
                slot
            }
            None => {
                let slot = self.builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, size, 3));
                self.spill_area = Some(slot);
                slot
            }
        };
        for (i, register) in registers.iter().enumerate() {
            let value = self.word(register);
            self.builder.ins().stack_store(types::I64, value, slot, (i * 8) as i32);
        }
        self.builder.ins().stack_addr(types::I64, slot, 0)
    }

    fn register_type(&self, register: &okkama_sparrow::Register) -> Option<&Type> {
        let source = &self.loaded.program.functions[self.function];
        self.loaded.program.ty(source.registers[register.0 as usize])
    }

    fn body(&mut self) -> Result<(), Error> {
        let program = &self.loaded.program;
        let source = &program.functions[self.function];
        let entry = self.builder.create_block();
        self.builder.append_block_params_for_function_params(entry);
        self.builder.switch_to_block(entry);
        self.ctx = Some(self.builder.block_params(entry)[0]);
        let explicit = self.root_live.is_some();
        let mut words = Vec::with_capacity(self.kinds.len());
        let mut next = 0u32;
        for kind in self.kinds {
            let var = self.builder.declare_var(if *kind == Kind::Float { types::F64 } else { types::I64 });
            if kind.traced() && explicit {
                words.push(Some(next));
                next += 1;
            } else {
                if kind.traced() {
                    self.builder.declare_var_needs_stack_map(var);
                }
                words.push(None);
            }
            self.vars.push(var);
        }
        if explicit {
            let frame = source.is_async.then_some(next);
            let count = next + u32::from(frame.is_some());
            let slot = self.builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                count.max(1) * 8,
                3,
            ));
            if source.is_async {
                // A resumption enters an instruction in the middle: no word
                // may keep what an earlier activation left.
                let zero = self.iconst(0);
                for word in 0..count {
                    self.builder.ins().stack_store(types::I64, zero, slot, (word * 8) as i32);
                }
            }
            self.roots = Some(Roots { slot, word: words, frame, reported: frame.into_iter().collect() });
        }
        // Registers start unbound; the verifier proves defined use.
        for (register, kind) in self.kinds.iter().enumerate() {
            let value = if *kind == Kind::Float {
                self.builder.ins().f64const(f64::from_bits(word::UNBOUND_RAW))
            } else {
                self.iconst(0)
            };
            self.builder.def_var(self.vars[register], value);
        }
        if source.native.is_some() || source.intrinsic.is_some() {
            // A trampoline, for calls through the code table (roles, closures).
            let params = self.builder.block_params(entry)[1..].to_vec();
            let slot = self.builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                (params.len().max(1) * 8) as u32,
                3,
            ));
            for (i, value) in params.iter().enumerate() {
                self.builder.ins().stack_store(types::I64, *value, slot, (i * 8) as i32);
            }
            let address = self.builder.ins().stack_addr(types::I64, slot, 0);
            let ctx = self.ctx();
            let index = self.iconst(self.function as i64);
            let count = self.iconst(params.len() as i64);
            let result = if source.native.is_some() {
                self.helper(|h| h.native, &[ctx, index, address, count])
            } else {
                self.helper(|h| h.intrinsic, &[ctx, index, address, count])
            }
            .expect("result");
            self.builder.ins().return_(&[result]);
            return Ok(());
        }
        if source.is_async {
            let frame = self.builder.declare_var(types::I64);
            let value = self.builder.block_params(entry)[1];
            match &self.roots {
                Some(Roots { slot, frame: Some(word), .. }) => {
                    let (slot, word) = (*slot, *word);
                    self.builder.ins().stack_store(types::I64, value, slot, (word * 8) as i32);
                }
                _ => self.builder.declare_var_needs_stack_map(frame),
            }
            self.builder.def_var(frame, value);
            self.frame = Some(frame);
        } else {
            for (i, register) in source.params.iter().enumerate() {
                let value = self.builder.block_params(entry)[i + 1];
                self.set_word(register, value);
            }
            // The prologue's calls find the parameters.
            if let Some(roots) = &mut self.roots {
                roots.reported = source.params.iter().filter_map(|register| roots.word[register.0 as usize]).collect();
            }
        }
        self.blocks = leaders(source)
            .into_iter()
            .map(|leader| leader.then(|| self.builder.create_block()))
            .collect();
        // R2: stack exhaustion is a typed failure.
        let ctx = self.ctx();
        let sp = self.builder.ins().get_stack_pointer(types::I64);
        let limit = self.builder.ins().load(
            types::I64,
            MEMORY,
            ctx,
            std::mem::offset_of!(Ctx, stack_limit) as i32,
        );
        let low = self.builder.ins().icmp(IntCC::UnsignedLessThan, sp, limit);
        let overflow = self.builder.create_block();
        let start = if source.is_async { self.builder.create_block() } else { self.block(0) };
        let entered = if self.polled { self.builder.create_block() } else { start };
        self.builder.ins().brif(low, overflow, &[], entered, &[]);
        if self.polled {
            // A function entry is a step.
            self.builder.switch_to_block(entered);
            self.poll();
            self.builder.ins().jump(start, &[]);
        }
        if source.is_async {
            self.resume_dispatch(start);
        }
        self.builder.switch_to_block(overflow);
        self.helper(|h| h.stack_overflow, &[ctx]);
        let zero = self.iconst(0);
        self.builder.ins().return_(&[zero]);
        // Open: the previous instruction falls through into this one.
        let mut open = false;
        for pc in 0..source.code.len() {
            match self.blocks[pc] {
                Some(block) => {
                    if open {
                        self.builder.ins().jump(block, &[]);
                    }
                    self.builder.switch_to_block(block);
                }
                // Unreachable: nothing jumps here and nothing falls through.
                None if !open => {
                    let block = self.builder.create_block();
                    self.builder.switch_to_block(block);
                }
                None => {}
            }
            self.pc = pc;
            self.enter_roots(pc);
            let before = self.builder.func.dfg.num_blocks();
            open = !self.instruction(pc)?;
            if let Some(live) = &self.live {
                let blocks = self.builder.func.dfg.num_blocks() - before + usize::from(self.blocks[pc].is_some());
                self.cost = self.cost.saturating_add(u64::from(live[pc]) * blocks as u64);
                if self.cost > self.cost_limit {
                    return Err(Error(format!(
                        "{}: compiling it would take more than {} MiB (registers live across its blocks, at instruction {pc}); \
                         keep fewer values alive at once or split the function",
                        source.symbol,
                        (self.cost_limit * BYTES_PER_LIVE_BLOCK) >> 20
                    )));
                }
            }
        }
        if open {
            return Err(Error(format!("{}: falls off the end", source.symbol)));
        }
        Ok(())
    }

    /// Entry of an async body: start with the parameters from the frame, or
    /// re-enter a suspended `await` with every register reloaded.
    fn resume_dispatch(&mut self, dispatch: Block) {
        let source = &self.loaded.program.functions[self.function];
        self.builder.switch_to_block(dispatch);
        let frame = self.builder.use_var(self.frame.expect("frame"));
        let resume = self.builder.ins().load(types::I64, MEMORY, frame, crate::tasks::RESUME as i32);
        let start = self.builder.create_block();
        let mut switch = cranelift_frontend::Switch::new();
        let mut reloads = Vec::new();
        for (pc, instruction) in source.code.iter().enumerate() {
            if suspends(instruction) {
                let block = self.builder.create_block();
                switch.set_entry(pc as u128 + 1, block);
                reloads.push((pc, block));
            }
        }
        switch.emit(self.builder, resume, start);
        self.builder.switch_to_block(start);
        for register in &source.params {
            let value = self.frame_register(register.0 as usize);
            self.set_word(register, value);
        }
        let first = self.block(0);
        self.builder.ins().jump(first, &[]);
        for (pc, block) in reloads {
            self.builder.switch_to_block(block);
            for register in 0..self.kinds.len() {
                let value = self.frame_register(register);
                self.set_word(&okkama_sparrow::Register(register as u32), value);
            }
            let resumed = self.block(pc);
            self.builder.ins().jump(resumed, &[]);
        }
    }

    fn frame_register(&mut self, register: usize) -> Value {
        let frame = self.builder.use_var(self.frame.expect("frame"));
        let offset = crate::tasks::REGISTERS as i32 + register as i32 * 8;
        self.builder.ins().load(types::I64, MEMORY, frame, offset)
    }

    /// Emit one instruction; true when it ends its block.
    /// After an await-like helper: continue when ready (its value, if any, in
    /// `scratch`), or save every register and return pending; the resumed
    /// frame re-enters the instruction at `pc`.
    fn suspend_unless_ready(&mut self, pc: usize, outcome: Value, dst: Option<okkama_sparrow::Dst>) {
        let ctx = self.ctx();
        let suspend = self.builder.create_block();
        let ready = self.builder.create_block();
        let pending = self.builder.ins().icmp_imm_u(IntCC::Equal, outcome, crate::tasks::AWAIT_PENDING as i64);
        self.builder.ins().brif(pending, suspend, &[], ready, &[]);
        // Save every register and re-enter this await when resumed.
        self.builder.switch_to_block(suspend);
        let frame = self.builder.use_var(self.frame.expect("frame"));
        for register in 0..self.kinds.len() {
            let value = self.word(&okkama_sparrow::Register(register as u32));
            let offset = crate::tasks::REGISTERS as i32 + register as i32 * 8;
            self.builder.ins().store(MEMORY, value, frame, offset);
        }
        let resume = self.iconst(pc as i64 + 1);
        self.builder.ins().store(MEMORY, resume, frame, crate::tasks::RESUME as i32);
        let one = self.iconst(1);
        self.builder.ins().return_(&[one]);
        self.builder.switch_to_block(ready);
        if let Some(dst) = dst {
            let value = self.builder.ins().load(
                types::I64,
                MEMORY,
                ctx,
                std::mem::offset_of!(Ctx, scratch) as i32,
            );
            self.set_word(&dst.0, value);
        }
    }

    fn instruction(&mut self, pc: usize) -> Result<bool, Error> {
        let program = &self.loaded.program;
        let source = &program.functions[self.function];
        let instruction = &source.code[pc];
        let ctx = self.ctx();
        let unsupported = || {
            Error(format!(
                "{}:{pc}: {} is not supported by sparrow-core yet",
                source.symbol,
                instruction.mnemonic()
            ))
        };
        if self.plain && generic(instruction) {
            self.generic_instruction(pc, instruction);
            return Ok(false);
        }
        match instruction {
            I::Const { dst, value } => {
                let value = match value {
                    Constant::Int(text) => {
                        let n: num_bigint::BigInt =
                            text.parse().map_err(|_| Error("invalid integer constant".into()))?;
                        let word = self.heap.big(n);
                        if word & 1 == 0 {
                            self.constants.push((word, Repr::Int));
                        }
                        self.iconst(word as i64)
                    }
                    Constant::Bool(b) => self.iconst(i64::from(*b)),
                    Constant::Byte(b) => self.iconst(i64::from(*b)),
                    Constant::Char(c) => self.iconst(i64::from(u32::from(*c))),
                    Constant::Float(bits) => self.builder.ins().f64const(f64::from_bits(*bits)),
                    Constant::Str(text) => self.constant_text(text),
                };
                self.set(&dst.0, value);
            }
            I::Move { dst, source } => {
                let value = self.get(source);
                self.set(&dst.0, value);
            }
            I::IntAdd { dst, left, right }
            | I::IntSub { dst, left, right }
            | I::IntMul { dst, left, right }
            | I::IntMod { dst, left, right } => {
                let (l, r) = (self.get(left), self.get(right));
                let both = self.builder.ins().band(l, r);
                let small = self.builder.ins().band_imm_u(both, 1);
                let fast = self.builder.create_block();
                let slow = self.builder.create_block();
                let done = self.builder.create_block();
                self.builder.append_block_param(done, types::I64);
                self.builder.ins().brif(small, fast, &[], slow, &[]);
                self.builder.switch_to_block(fast);
                let op = match instruction {
                    I::IntAdd { .. } => runtime::INT_ADD,
                    I::IntSub { .. } => runtime::INT_SUB,
                    I::IntMul { .. } => runtime::INT_MUL,
                    _ => runtime::INT_MOD,
                };
                if op == runtime::INT_MOD {
                    // Divisors 0 and -1 go to the exact path.
                    let divisor = self.builder.ins().sshr_imm_u(r, 1);
                    let shifted = self.builder.ins().iadd_imm_s(divisor, 1);
                    let special = self.builder.ins().icmp_imm_u(IntCC::UnsignedLessThanOrEqual, shifted, 1);
                    let commit = self.builder.create_block();
                    self.builder.ins().brif(special, slow, &[], commit, &[]);
                    self.builder.switch_to_block(commit);
                    let dividend = self.builder.ins().sshr_imm_u(l, 1);
                    let remainder = self.builder.ins().srem(dividend, divisor);
                    let doubled = self.builder.ins().ishl_imm_u(remainder, 1);
                    let tagged = self.builder.ins().bor_imm_u(doubled, 1);
                    self.builder.ins().jump(done, &[tagged.into()]);
                } else {
                    let even = self.builder.ins().iadd_imm_s(r, -1);
                    let (value, overflow) = match op {
                        runtime::INT_ADD => self.builder.ins().sadd_overflow(l, even),
                        runtime::INT_SUB => self.builder.ins().ssub_overflow(l, even),
                        _ => {
                            let half = self.builder.ins().sshr_imm_u(l, 1);
                            let (product, overflow) = self.builder.ins().smul_overflow(half, even);
                            (self.builder.ins().bor_imm_u(product, 1), overflow)
                        }
                    };
                    let commit = self.builder.create_block();
                    self.builder.ins().brif(overflow, slow, &[], commit, &[]);
                    self.builder.switch_to_block(commit);
                    self.builder.ins().jump(done, &[value.into()]);
                }
                self.builder.switch_to_block(slow);
                let op = self.iconst(op as i64);
                let result = self.helper(|h| h.int, &[ctx, op, l, r]).expect("result");
                self.check(pc);
                self.builder.ins().jump(done, &[result.into()]);
                self.builder.switch_to_block(done);
                let value = self.builder.block_params(done)[0];
                self.set(&dst.0, value);
            }
            I::IntCmp { op, dst, left, right } => {
                let (l, r) = (self.get(left), self.get(right));
                let both = self.builder.ins().band(l, r);
                let small = self.builder.ins().band_imm_u(both, 1);
                let fast = self.builder.create_block();
                let slow = self.builder.create_block();
                let done = self.builder.create_block();
                self.builder.append_block_param(done, types::I64);
                self.builder.ins().brif(small, fast, &[], slow, &[]);
                let condition = int_condition(*op);
                self.builder.switch_to_block(fast);
                let flag = self.builder.ins().icmp(condition, l, r);
                let flag = self.builder.ins().uextend(types::I64, flag);
                self.builder.ins().jump(done, &[flag.into()]);
                self.builder.switch_to_block(slow);
                let order = self.helper(|h| h.int_order, &[ctx, l, r]).expect("result");
                let flag = self.builder.ins().icmp_imm_s(condition, order, 0);
                let flag = self.builder.ins().uextend(types::I64, flag);
                self.builder.ins().jump(done, &[flag.into()]);
                self.builder.switch_to_block(done);
                let value = self.builder.block_params(done)[0];
                self.set(&dst.0, value);
            }
            I::BoolNot { dst, operand } => {
                let value = self.get(operand);
                let value = self.builder.ins().bxor_imm_u(value, 1);
                self.set(&dst.0, value);
            }
            I::BoolCmp { op, dst, left, right }
            | I::ByteCmp { op, dst, left, right }
            | I::CharCmp { op, dst, left, right } => {
                let (l, r) = (self.get(left), self.get(right));
                let flag = self.builder.ins().icmp(int_condition(*op), l, r);
                let flag = self.builder.ins().uextend(types::I64, flag);
                self.set(&dst.0, flag);
            }
            I::FloatAdd { dst, left, right }
            | I::FloatSub { dst, left, right }
            | I::FloatMul { dst, left, right }
            | I::FloatDiv { dst, left, right } => {
                let (l, r) = (self.get(left), self.get(right));
                let (value, op) = match instruction {
                    I::FloatAdd { .. } => (self.builder.ins().fadd(l, r), crate::scalar::FLOAT_ADD),
                    I::FloatSub { .. } => (self.builder.ins().fsub(l, r), crate::scalar::FLOAT_SUB),
                    I::FloatMul { .. } => (self.builder.ins().fmul(l, r), crate::scalar::FLOAT_MUL),
                    _ => (self.builder.ins().fdiv(l, r), crate::scalar::FLOAT_DIV),
                };
                let magnitude = self.builder.ins().fabs(value);
                let maximum = self.builder.ins().f64const(f64::MAX);
                let finite = self.builder.ins().fcmp(FloatCC::LessThanOrEqual, magnitude, maximum);
                let slow = self.builder.create_block();
                let done = self.builder.create_block();
                self.builder.append_block_param(done, types::F64);
                self.builder.ins().brif(finite, done, &[value.into()], slow, &[]);
                self.builder.switch_to_block(slow);
                let op = self.iconst(op as i64);
                let lw = self.builder.ins().bitcast(types::I64, MemFlagsData::new(), l);
                let rw = self.builder.ins().bitcast(types::I64, MemFlagsData::new(), r);
                let bits = self.helper(|h| h.float, &[ctx, op, lw, rw]).expect("result");
                self.check(pc);
                let value = self.builder.ins().bitcast(types::F64, MemFlagsData::new(), bits);
                self.builder.ins().jump(done, &[value.into()]);
                self.builder.switch_to_block(done);
                let value = self.builder.block_params(done)[0];
                self.set(&dst.0, value);
            }
            I::FloatNeg { dst, operand } | I::FloatAbs { dst, operand } => {
                let value = self.get(operand);
                let value = if matches!(instruction, I::FloatNeg { .. }) {
                    self.builder.ins().fneg(value)
                } else {
                    self.builder.ins().fabs(value)
                };
                self.set(&dst.0, value);
            }
            I::FloatCmp { op, dst, left, right } => {
                let (l, r) = (self.get(left), self.get(right));
                let condition = match op {
                    Cmp::Eq => FloatCC::Equal,
                    Cmp::Ne => FloatCC::NotEqual,
                    Cmp::Lt => FloatCC::LessThan,
                    Cmp::Le => FloatCC::LessThanOrEqual,
                    Cmp::Gt => FloatCC::GreaterThan,
                    Cmp::Ge => FloatCC::GreaterThanOrEqual,
                };
                let flag = self.builder.ins().fcmp(condition, l, r);
                let flag = self.builder.ins().uextend(types::I64, flag);
                self.set(&dst.0, flag);
            }
            I::StrConcat { dst, left, right } => {
                let (l, r) = (self.get(left), self.get(right));
                let text = self.helper(|h| h.concat, &[ctx, l, r]).expect("result");
                self.check(pc);
                self.set(&dst.0, text);
            }
            I::ArrayNew { dst, elements } => {
                let repr = match self.register_type(&dst.0) {
                    Some(Type::Array(element)) if Kind::of(program, *element) == Kind::Byte => runtime::ARRAY_BYTES,
                    Some(Type::Array(element)) => Kind::of(program, *element).repr() as u64,
                    _ => return Err(unsupported()),
                };
                let address = self.spill(elements);
                let repr = self.iconst(repr as i64);
                let count = self.iconst(elements.len() as i64);
                let array = self.helper(|h| h.array, &[ctx, repr, address, count]).expect("result");
                self.set(&dst.0, array);
            }
            I::ArrayLen { dst, array } => {
                let array = self.get(array);
                let header = self.builder.ins().load(types::I64, MEMORY, array, 0);
                let length = self.builder.ins().ushr_imm_u(header, 32);
                let doubled = self.builder.ins().ishl_imm_u(length, 1);
                let tagged = self.builder.ins().bor_imm_u(doubled, 1);
                self.set(&dst.0, tagged);
            }
            I::ArrayGet { dst, array, index } => {
                let array_register = *array;
                let (array, index) = (self.get(array), self.get(index));
                let header = self.builder.ins().load(types::I64, MEMORY, array, 0);
                let length = self.builder.ins().ushr_imm_u(header, 32);
                let small = self.builder.ins().band_imm_u(index, 1);
                let position = self.builder.ins().sshr_imm_u(index, 1);
                // Unsigned compare also rejects negative positions.
                let inside = self.builder.ins().icmp(IntCC::UnsignedLessThan, position, length);
                let inside = self.builder.ins().uextend(types::I64, inside);
                let valid = self.builder.ins().band(small, inside);
                let load = self.builder.create_block();
                let outside = self.builder.create_block();
                self.builder.ins().brif(valid, load, &[], outside, &[]);
                self.builder.switch_to_block(outside);
                let message = self.constant_text("Array index out of bounds");
                self.helper(|h| h.trap, &[ctx, message]);
                self.check(pc);
                self.builder.ins().jump(load, &[]);
                self.builder.switch_to_block(load);
                let value = self.element(array, header, position, self.bytes_array(array_register));
                self.set_word(&dst.0, value);
            }
            I::ObjNew { dst, object, fields } => {
                let words = fields.len().max(1) + 1;
                let pointer = self.allocate(words);
                let header = self.iconst(heap::header(heap::OBJECT, Repr::Raw, object.0) as i64);
                self.builder.ins().store(MEMORY, header, pointer, 0);
                for (i, field) in fields.iter().enumerate() {
                    let value = self.word(field);
                    self.builder.ins().store(MEMORY, value, pointer, ((i + 1) * 8) as i32);
                }
                self.set(&dst.0, pointer);
                self.created(object.0 as usize, pointer);
            }
            I::ObjGet { dst, object, field } => {
                let pointer = self.get(object);
                let value = self.builder.ins().load(types::I64, MEMORY, pointer, ((*field + 1) * 8) as i32);
                self.set_word(&dst.0, value);
            }
            I::ObjInit { object, field, value } => {
                let pointer = self.get(object);
                let value = self.word(value);
                self.builder.ins().store(MEMORY, value, pointer, ((*field + 1) * 8) as i32);
            }
            I::ObjNext { dst, object, updates } => {
                let declared = match self.register_type(object) {
                    Some(Type::Object(o) | Type::Successor(o)) => o.0 as usize,
                    _ => return Err(unsupported()),
                };
                let count = program.objects[declared].fields.len();
                let pointer = self.allocate(count.max(1) + 1);
                let source_object = self.get(object);
                for word_index in 0..=count {
                    let value = match updates.iter().find(|(f, _)| *f as usize + 1 == word_index) {
                        Some((_, register)) => self.word(register),
                        None => self.builder.ins().load(types::I64, MEMORY, source_object, (word_index * 8) as i32),
                    };
                    self.builder.ins().store(MEMORY, value, pointer, (word_index * 8) as i32);
                }
                self.set(&dst.0, pointer);
                self.created(declared, pointer);
            }
            I::RoleWrap { dst, source: value, .. } => {
                let value = self.get(value);
                self.set(&dst.0, value);
            }
            // Вариант (#290): значение — объект своего случая, только с полями
            // payload. Случаи идут подряд (это проверяет загрузка), поэтому
            // номер случая — разность дескрипторов.
            I::VariantNew { dst, variant, case, fields } => {
                let object = program.variants[variant.0 as usize].cases[*case as usize].1;
                let words = fields.len().max(1) + 1;
                let pointer = self.allocate(words);
                let header = self.iconst(heap::header(heap::OBJECT, Repr::Raw, object.0) as i64);
                self.builder.ins().store(MEMORY, header, pointer, 0);
                for (i, field) in fields.iter().enumerate() {
                    let value = self.word(field);
                    self.builder.ins().store(MEMORY, value, pointer, ((i + 1) * 8) as i32);
                }
                self.set(&dst.0, pointer);
                self.created(object.0 as usize, pointer);
            }
            I::VariantTag { dst, value } => {
                let first = match self.register_type(value) {
                    Some(Type::Variant(v)) => program.variants[v.0 as usize].cases[0].1.0,
                    _ => return Err(unsupported()),
                };
                let pointer = self.get(value);
                let header = self.builder.ins().load(types::I64, MEMORY, pointer, 0);
                let descriptor = self.builder.ins().ushr_imm_u(header, 32);
                let tag = self.builder.ins().iadd_imm_s(descriptor, -i64::from(first));
                let doubled = self.builder.ins().ishl_imm_u(tag, 1);
                let tagged = self.builder.ins().bor_imm_u(doubled, 1);
                self.set(&dst.0, tagged);
            }
            I::VariantGet { dst, value, case, field } => {
                let object = match self.register_type(value) {
                    Some(Type::Variant(v)) => program.variants[v.0 as usize].cases[*case as usize].1.0,
                    _ => return Err(unsupported()),
                };
                let pointer = self.get(value);
                let header = self.builder.ins().load(types::I64, MEMORY, pointer, 0);
                let descriptor = self.builder.ins().ushr_imm_u(header, 32);
                let active = self.builder.ins().icmp_imm_s(IntCC::Equal, descriptor, i64::from(object));
                let load = self.builder.create_block();
                let inactive = self.builder.create_block();
                self.builder.ins().brif(active, load, &[], inactive, &[]);
                self.builder.switch_to_block(inactive);
                let message = self.constant_text("Variant payload of an inactive case");
                self.helper(|h| h.trap, &[ctx, message]);
                self.check(pc);
                self.builder.ins().jump(load, &[]);
                self.builder.switch_to_block(load);
                let value = self.builder.ins().load(types::I64, MEMORY, pointer, ((*field + 1) * 8) as i32);
                self.set_word(&dst.0, value);
            }
            I::VariantOf { dst, source, .. } => {
                let value = self.get(source);
                self.set(&dst.0, value);
            }
            I::CallRole { dst, receiver, role, slot, args } => {
                let objects = program.objects.len();
                let table = self.jit.dispatch.entry((role.0, *slot)).or_insert_with(|| {
                    program
                        .objects
                        .iter()
                        .map(|o| {
                            o.does
                                .iter()
                                .find(|w| w.role == *role)
                                .map_or(u64::MAX, |w| u64::from(w.slots[*slot as usize].0))
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice()
                });
                debug_assert_eq!(table.len(), objects);
                let table = table.as_ptr() as i64;
                let object = self.get(receiver);
                let header = self.builder.ins().load(types::I64, MEMORY, object, 0);
                let descriptor = self.builder.ins().ushr_imm_u(header, 32);
                let offset = self.builder.ins().ishl_imm_u(descriptor, 3);
                let base = self.iconst(table);
                let entry = self.builder.ins().iadd(base, offset);
                let function = self.builder.ins().load(types::I64, MEMORY, entry, 0);
                // Async witnesses: the call makes a cold Future of the method.
                let asynchronous = program
                    .objects
                    .iter()
                    .flat_map(|o| &o.does)
                    .filter(|w| w.role == *role)
                    .any(|w| program.functions[w.slots[*slot as usize].0 as usize].is_async);
                let result = if asynchronous {
                    let mut registers = vec![*receiver];
                    registers.extend(args.iter().copied());
                    let address = self.spill(&registers);
                    let count = self.iconst(registers.len() as i64);
                    let future = self.helper(|h| h.future_new, &[ctx, function, address, count]).expect("result");
                    self.check(pc);
                    future
                } else {
                    let mut values = vec![object];
                    for arg in args {
                        let value = self.word(arg);
                        values.push(value);
                    }
                    self.call(pc, function, &values)
                };
                if let Some(dst) = dst {
                    self.set_word(&dst.0, result);
                }
            }
            I::Call { dst, function, args } => {
                let callee = &program.functions[function.0 as usize];
                // FutureCompletions.completion reads its origins at the call.
                let eager = callee.intrinsic.as_ref().is_some_and(|i| i.name == "okkama.collect.FutureCompletions.completion");
                let result = if callee.is_async && !eager {
                    // Async functions (natives and intrinsics too) make a cold Future.
                    let address = self.spill(args);
                    let index = self.iconst(i64::from(function.0));
                    let count = self.iconst(args.len() as i64);
                    let future = self.helper(|h| h.future_new, &[ctx, index, address, count]).expect("result");
                    self.check(pc);
                    future
                } else if let Some(atomic) = atomic_access(callee) {
                    self.atomic(pc, function.0, atomic, args)
                } else if callee.native.is_some() || callee.intrinsic.is_some() {
                    let address = self.spill(args);
                    let index = self.iconst(i64::from(function.0));
                    let count = self.iconst(args.len() as i64);
                    let result = if callee.native.is_some() {
                        self.helper(|h| h.native, &[ctx, index, address, count])
                    } else {
                        self.helper(|h| h.intrinsic, &[ctx, index, address, count])
                    }
                    .expect("result");
                    self.check(pc);
                    result
                } else {
                    let mut values = Vec::with_capacity(args.len());
                    for arg in args {
                        let value = self.word(arg);
                        values.push(value);
                    }
                    let index = self.iconst(i64::from(function.0));
                    self.call(pc, index, &values)
                };
                if let Some(dst) = dst {
                    self.set_word(&dst.0, result);
                }
            }
            I::CallCap { dst, cap, ty, field } => {
                let function = program.caps[ty.0 as usize]
                    .fields
                    .get(*field as usize)
                    .ok_or_else(unsupported)?
                    .1
                    .0;
                let index = self.iconst(i64::from(function));
                let result = if program.functions[function as usize].is_async {
                    let address = self.spill(std::slice::from_ref(cap));
                    let count = self.iconst(1);
                    let future = self.helper(|h| h.future_new, &[ctx, index, address, count]).expect("result");
                    self.check(pc);
                    future
                } else {
                    let receiver = self.word(cap);
                    self.call(pc, index, &[receiver])
                };
                self.set_word(&dst.0, result);
            }
            I::Throw { value } => {
                let value = self.get(value);
                self.helper(|h| h.throw, &[ctx, value]);
                self.check(pc);
                let zero = self.iconst(0);
                self.builder.ins().return_(&[zero]);
                return Ok(true);
            }
            I::Trap { message, .. } => {
                let message = self.constant_text(message);
                self.helper(|h| h.trap, &[ctx, message]);
                self.check(pc);
                let zero = self.iconst(0);
                self.builder.ins().return_(&[zero]);
                return Ok(true);
            }
            I::Jump { target } => {
                self.jump(target);
                return Ok(true);
            }
            I::Branch { condition, then_target, else_target } => {
                let condition = self.get(condition);
                let (yes, no) = (self.builder.create_block(), self.builder.create_block());
                self.builder.ins().brif(condition, yes, &[], no, &[]);
                self.builder.switch_to_block(yes);
                self.jump(then_target);
                self.builder.switch_to_block(no);
                self.jump(else_target);
                return Ok(true);
            }
            I::Return { value } => {
                let value = match value {
                    Some(register) => self.word(register),
                    None => self.iconst(0),
                };
                if let Some(frame) = self.frame {
                    let frame = self.builder.use_var(frame);
                    self.helper(|h| h.complete, &[ctx, frame, value]);
                    let done = self.iconst(0);
                    self.builder.ins().return_(&[done]);
                } else {
                    self.builder.ins().return_(&[value]);
                }
                return Ok(true);
            }
            I::RegionSettle { .. } | I::RegionSettleUnwind { .. } if self.frame.is_some() => {
                let this = self.builder.use_var(self.frame.expect("frame"));
                let unbound = self.iconst(word::UNBOUND as i64);
                let (region, keep, failure, dst) = match instruction {
                    I::RegionSettle { region, keep } => {
                        let keep = keep.map(|keep| self.word(&keep)).unwrap_or(unbound);
                        (self.get(region), keep, unbound, None)
                    }
                    I::RegionSettleUnwind { dst, region, failure } => (self.get(region), unbound, self.get(failure), Some(*dst)),
                    _ => unreachable!("matched above"),
                };
                let outcome = self.helper(|h| h.region_settle, &[ctx, this, region, keep, failure]).expect("result");
                self.check(pc);
                self.suspend_unless_ready(pc, outcome, dst);
            }
            I::Await { dst, future } if self.frame.is_some() => {
                let this = self.builder.use_var(self.frame.expect("frame"));
                let future = self.get(future);
                let outcome = self.helper(|h| h.await_, &[ctx, this, future]).expect("result");
                self.check(pc);
                self.suspend_unless_ready(pc, outcome, *dst);
            }
            I::FutSpawn { dst, future } => {
                let future = self.get(future);
                let spawned = self.helper(|h| h.spawn, &[ctx, future]).expect("result");
                self.check(pc);
                self.set(&dst.0, spawned);
            }
            I::Fut { op, dst, future } => {
                let future = self.get(future);
                let op = self.iconst(*op as i64);
                let result = self.helper(|h| h.fut, &[ctx, op, future]).expect("result");
                self.check(pc);
                if let Some(dst) = dst {
                    self.set_word(&dst.0, result);
                }
            }
            I::ArrayLookup { array, index, value, found, missing } => {
                let array_register = *array;
                let (array, index) = (self.get(array), self.get(index));
                let header = self.builder.ins().load(types::I64, MEMORY, array, 0);
                let length = self.builder.ins().ushr_imm_u(header, 32);
                let small = self.builder.ins().band_imm_u(index, 1);
                let position = self.builder.ins().sshr_imm_u(index, 1);
                let inside = self.builder.ins().icmp(IntCC::UnsignedLessThan, position, length);
                let inside = self.builder.ins().uextend(types::I64, inside);
                let valid = self.builder.ins().band(small, inside);
                let hit = self.builder.create_block();
                let miss = self.builder.create_block();
                self.builder.ins().brif(valid, hit, &[], miss, &[]);
                self.builder.switch_to_block(hit);
                let item = self.element(array, header, position, self.bytes_array(array_register));
                // Edge moves read the old registers; the found value is written last.
                self.jump_then(found, value.0, item);
                self.builder.switch_to_block(miss);
                self.jump(missing);
                return Ok(true);
            }
            I::MapLookup { map, key, value, found, missing } => {
                let (map, key) = (self.get(map), self.word(key));
                let function = self.iconst(self.function as i64);
                let at = self.iconst(pc as i64);
                let present = self.helper(|h| h.map_find, &[ctx, function, at, map, key]).expect("result");
                let hit = self.builder.create_block();
                let miss = self.builder.create_block();
                self.builder.ins().brif(present, hit, &[], miss, &[]);
                self.builder.switch_to_block(hit);
                let item = self.builder.ins().load(
                    types::I64,
                    MEMORY,
                    ctx,
                    std::mem::offset_of!(Ctx, scratch) as i32,
                );
                self.jump_then(found, value.0, item);
                self.builder.switch_to_block(miss);
                self.jump(missing);
                return Ok(true);
            }
            I::Closure { dst, function, captures } => {
                let address = self.spill(captures);
                let index = self.iconst(i64::from(function.0));
                let count = self.iconst(captures.len() as i64);
                let closure = self.helper(|h| h.closure, &[ctx, index, address, count]).expect("result");
                self.set(&dst.0, closure);
            }
            I::CallFn { dst, callee, args } => {
                let closure = self.get(callee);
                let address = self.spill(args);
                let count = self.iconst(args.len() as i64);
                let result = self.helper(|h| h.call_closure, &[ctx, closure, address, count]).expect("result");
                self.check(pc);
                if let Some(dst) = dst {
                    self.set_word(&dst.0, result);
                }
            }
            I::Catch { failure, value, handler, next, .. } => {
                let failure = self.get(failure);
                let function = self.iconst(self.function as i64);
                let at = self.iconst(pc as i64);
                let matched = self.helper(|h| h.catch, &[ctx, function, at, failure]).expect("result");
                let hit = self.builder.create_block();
                let miss = self.builder.create_block();
                self.builder.ins().brif(matched, hit, &[], miss, &[]);
                self.builder.switch_to_block(hit);
                let payload = self.builder.ins().load(
                    types::I64,
                    MEMORY,
                    ctx,
                    std::mem::offset_of!(Ctx, scratch) as i32,
                );
                self.jump_then(handler, value.0, payload);
                self.builder.switch_to_block(miss);
                self.jump(next);
                return Ok(true);
            }
            I::RegionEnter { dst } => {
                // `[overflow, slots…]`, all unbound (see `failures::REGION_SLOTS`).
                let length = 1 + crate::failures::REGION_SLOTS;
                let cell = self.allocate(1 + length);
                let header = self.iconst(heap::header(heap::ARRAY, Repr::Ref, length as u32) as i64);
                self.builder.ins().store(MEMORY, header, cell, 0);
                let unbound = self.iconst(0);
                for index in 0..length {
                    self.builder.ins().store(MEMORY, unbound, cell, (8 + index * 8) as i32);
                }
                self.set(&dst.0, cell);
            }
            I::RegionAdopt { region, value } | I::RegionTrack { region, value } => {
                let (region, value) = (self.get(region), self.word(value));
                let own = self.iconst(i64::from(matches!(instruction, I::RegionAdopt { .. })));
                self.helper(|h| h.region_add, &[ctx, region, value, own]);
            }
            I::RegionLeave { region, .. } => {
                // A region that tracked nothing closes without a call.
                let cell = self.get(region);
                let entries = self.builder.ins().load(types::I64, MEMORY, cell, 16);
                let (close, done) = (self.builder.create_block(), self.builder.create_block());
                self.builder.ins().brif(entries, close, &[], done, &[]);
                self.builder.switch_to_block(close);
                let instruction = instruction.clone();
                self.failure_generic(pc, &instruction);
                self.builder.ins().jump(done, &[]);
                self.builder.switch_to_block(done);
            }
            other if failure_or_region(other) => {
                let mut uses = Vec::new();
                let mut def = None;
                other.visit(&mut |access| match access {
                    okkama_sparrow::Access::Use(register) => uses.push(register),
                    okkama_sparrow::Access::Def(register) => def = Some(register),
                    _ => {}
                });
                let address = self.spill(&uses);
                let function = self.iconst(self.function as i64);
                let at = self.iconst(pc as i64);
                let result = self.helper(|h| h.failure_op, &[ctx, function, at, address]).expect("result");
                self.check(pc);
                if matches!(other, I::Raise { .. } | I::Panic { .. }) {
                    let zero = self.iconst(0);
                    self.builder.ins().return_(&[zero]);
                    return Ok(true);
                }
                if let Some(def) = def {
                    self.set_word(&def, result);
                }
            }
            I::IntAnd { dst, left, right }
            | I::IntOr { dst, left, right }
            | I::IntXor { dst, left, right }
            | I::IntShl { dst, left, right }
            | I::IntShr { dst, left, right } => {
                // Two inline integers stay inline; the rest goes the exact way.
                let (l, r) = (self.get(left), self.get(right));
                let both = self.builder.ins().band(l, r);
                let small = self.builder.ins().band_imm_u(both, 1);
                let (fast, slow, done) = (self.builder.create_block(), self.builder.create_block(), self.builder.create_block());
                self.builder.append_block_param(done, types::I64);
                self.builder.ins().brif(small, fast, &[], slow, &[]);
                self.builder.switch_to_block(fast);
                match instruction {
                    I::IntAnd { .. } => {
                        let value = self.builder.ins().band(l, r);
                        self.builder.ins().jump(done, &[value.into()]);
                    }
                    I::IntOr { .. } => {
                        let value = self.builder.ins().bor(l, r);
                        self.builder.ins().jump(done, &[value.into()]);
                    }
                    I::IntXor { .. } => {
                        let x = self.builder.ins().bxor(l, r);
                        let value = self.builder.ins().bor_imm_u(x, 1);
                        self.builder.ins().jump(done, &[value.into()]);
                    }
                    _ => {
                        let left_shift = matches!(instruction, I::IntShl { .. });
                        let v = self.builder.ins().sshr_imm_u(l, 1);
                        let n = self.builder.ins().sshr_imm_u(r, 1);
                        let in_range = self.builder.ins().icmp_imm_u(IntCC::UnsignedLessThan, n, 62);
                        let commit = self.builder.create_block();
                        self.builder.ins().brif(in_range, commit, &[], slow, &[]);
                        self.builder.switch_to_block(commit);
                        if left_shift {
                            // No bit may be lost: shifting back gives the value,
                            // and the result still fits an inline integer.
                            let shifted = self.builder.ins().ishl(v, n);
                            let tagged = self.builder.ins().ishl_imm_u(shifted, 1);
                            let tagged = self.builder.ins().bor_imm_u(tagged, 1);
                            let back = self.builder.ins().sshr(shifted, n);
                            let same = self.builder.ins().icmp(IntCC::Equal, back, v);
                            let untagged = self.builder.ins().sshr_imm_u(tagged, 1);
                            let fits = self.builder.ins().icmp(IntCC::Equal, untagged, shifted);
                            let ok = self.builder.ins().band(same, fits);
                            let exact = self.builder.create_block();
                            self.builder.ins().brif(ok, exact, &[], slow, &[]);
                            self.builder.switch_to_block(exact);
                            self.builder.ins().jump(done, &[tagged.into()]);
                        } else {
                            let shifted = self.builder.ins().sshr(v, n);
                            let tagged = self.builder.ins().ishl_imm_u(shifted, 1);
                            let tagged = self.builder.ins().bor_imm_u(tagged, 1);
                            self.builder.ins().jump(done, &[tagged.into()]);
                        }
                    }
                }
                self.builder.switch_to_block(slow);
                let result = self.exec_generic(pc, instruction);
                self.builder.ins().jump(done, &[result.into()]);
                self.builder.switch_to_block(done);
                let value = self.builder.block_params(done)[0];
                self.set(&dst.0, value);
            }
            I::IntNot { dst, operand } | I::IntNeg { dst, operand } => {
                let v = self.get(operand);
                let small = self.builder.ins().band_imm_u(v, 1);
                let (fast, slow, done) = (self.builder.create_block(), self.builder.create_block(), self.builder.create_block());
                self.builder.append_block_param(done, types::I64);
                self.builder.ins().brif(small, fast, &[], slow, &[]);
                self.builder.switch_to_block(fast);
                if matches!(instruction, I::IntNot { .. }) {
                    // !(2v+1) | 1 == 2(!v)+1
                    let x = self.builder.ins().bnot(v);
                    let value = self.builder.ins().bor_imm_u(x, 1);
                    self.builder.ins().jump(done, &[value.into()]);
                } else {
                    // 2(-v)+1 == 2 - (2v+1)
                    let two = self.iconst(2);
                    let (value, overflow) = self.builder.ins().ssub_overflow(two, v);
                    let commit = self.builder.create_block();
                    self.builder.ins().brif(overflow, slow, &[], commit, &[]);
                    self.builder.switch_to_block(commit);
                    self.builder.ins().jump(done, &[value.into()]);
                }
                self.builder.switch_to_block(slow);
                let result = self.exec_generic(pc, instruction);
                self.builder.ins().jump(done, &[result.into()]);
                self.builder.switch_to_block(done);
                let value = self.builder.block_params(done)[0];
                self.set(&dst.0, value);
            }
            I::StrCmp { op: op @ (Cmp::Eq | Cmp::Ne), dst, left, right } => {
                // The same string, or strings of different byte lengths, decide
                // equality inline; the rest compares bytes in the slow path.
                let (l, r) = (self.get(left), self.get(right));
                let equal = u64::from(*op == Cmp::Eq) as i64;
                let (decided, compare, slow, done) = (
                    self.builder.create_block(),
                    self.builder.create_block(),
                    self.builder.create_block(),
                    self.builder.create_block(),
                );
                self.builder.append_block_param(decided, types::I64);
                self.builder.append_block_param(done, types::I64);
                let same = self.builder.ins().icmp(IntCC::Equal, l, r);
                let yes = self.iconst(equal);
                self.builder.ins().brif(same, decided, &[yes.into()], compare, &[]);
                self.builder.switch_to_block(compare);
                let (hl, hr) = (
                    self.builder.ins().load(types::I64, MEMORY, l, 0),
                    self.builder.ins().load(types::I64, MEMORY, r, 0),
                );
                let (nl, nr) = (self.builder.ins().ushr_imm_u(hl, 32), self.builder.ins().ushr_imm_u(hr, 32));
                let differ = self.builder.ins().icmp(IntCC::NotEqual, nl, nr);
                let no = self.iconst(1 - equal);
                self.builder.ins().brif(differ, decided, &[no.into()], slow, &[]);
                self.builder.switch_to_block(decided);
                let flag = self.builder.block_params(decided)[0];
                self.builder.ins().jump(done, &[flag.into()]);
                self.builder.switch_to_block(slow);
                let result = self.exec_generic(pc, instruction);
                self.builder.ins().jump(done, &[result.into()]);
                self.builder.switch_to_block(done);
                let value = self.builder.block_params(done)[0];
                self.set(&dst.0, value);
            }
            I::ValCmp { op: op @ (Cmp::Eq | Cmp::Ne), dst, left, right }
                if matches!(self.kinds[left.0 as usize], Kind::Int | Kind::Raw | Kind::Byte | Kind::Float) =>
            {
                // Scalars compare by word (exact ints: two inline integers),
                // floats numerically (#314); anything else takes the
                // structural slow path.
                let int = self.kinds[left.0 as usize] == Kind::Int;
                let (l, r) = (self.get(left), self.get(right));
                let flag = if self.kinds[left.0 as usize] == Kind::Float {
                    let condition = if *op == Cmp::Eq { FloatCC::Equal } else { FloatCC::NotEqual };
                    self.builder.ins().fcmp(condition, l, r)
                } else {
                    let condition = if *op == Cmp::Eq { IntCC::Equal } else { IntCC::NotEqual };
                    self.builder.ins().icmp(condition, l, r)
                };
                let flag = self.builder.ins().uextend(types::I64, flag);
                if int {
                    let both = self.builder.ins().band(l, r);
                    let small = self.builder.ins().band_imm_u(both, 1);
                    let (fast, slow, done) = (self.builder.create_block(), self.builder.create_block(), self.builder.create_block());
                    self.builder.append_block_param(done, types::I64);
                    self.builder.ins().brif(small, fast, &[], slow, &[]);
                    self.builder.switch_to_block(fast);
                    self.builder.ins().jump(done, &[flag.into()]);
                    self.builder.switch_to_block(slow);
                    let result = self.exec_generic(pc, instruction);
                    self.builder.ins().jump(done, &[result.into()]);
                    self.builder.switch_to_block(done);
                    let value = self.builder.block_params(done)[0];
                    self.set(&dst.0, value);
                } else {
                    self.set(&dst.0, flag);
                }
            }
            I::BuilderPush { dst, builder, value } => {
                let (builder, value) = (self.get(builder), self.word(value));
                let result = self.helper(|h| h.builder_push, &[ctx, builder, value]).expect("result");
                self.set(&dst.0, result);
            }
            I::Convert { dst, source }
                if matches!(self.register_type(source), Some(Type::Int)) && matches!(self.register_type(&dst.0), Some(Type::Str)) =>
            {
                let value = self.get(source);
                let text = self.helper(|h| h.int_text, &[ctx, value]).expect("result");
                self.set(&dst.0, text);
            }
            I::StrLen { dst, text } => {
                // ASCII strings know their length in characters: the byte length.
                let string = self.get(text);
                let header = self.builder.ins().load(types::I64, MEMORY, string, 0);
                let ascii = self.builder.ins().band_imm_u(header, heap::ASCII as i64);
                let (fast, slow, done) = (self.builder.create_block(), self.builder.create_block(), self.builder.create_block());
                self.builder.append_block_param(done, types::I64);
                self.builder.ins().brif(ascii, fast, &[], slow, &[]);
                self.builder.switch_to_block(fast);
                let length = self.builder.ins().ushr_imm_u(header, 32);
                let doubled = self.builder.ins().ishl_imm_u(length, 1);
                let tagged = self.builder.ins().bor_imm_u(doubled, 1);
                self.builder.ins().jump(done, &[tagged.into()]);
                self.builder.switch_to_block(slow);
                let result = self.exec_generic(pc, instruction);
                self.builder.ins().jump(done, &[result.into()]);
                self.builder.switch_to_block(done);
                let value = self.builder.block_params(done)[0];
                self.set(&dst.0, value);
            }
            I::EnumCase { dst, ty, case } => {
                let value = self.iconst(((u64::from(ty.0) << 32) | u64::from(*case)) as i64);
                self.set_word(&dst.0, value);
            }
            I::EnumCmp { op, dst, left, right } => {
                // Enum words compare as unsigned integers (see `ops`).
                let (l, r) = (self.word(left), self.word(right));
                let condition = match op {
                    Cmp::Eq => IntCC::Equal,
                    Cmp::Ne => IntCC::NotEqual,
                    Cmp::Lt => IntCC::UnsignedLessThan,
                    Cmp::Le => IntCC::UnsignedLessThanOrEqual,
                    Cmp::Gt => IntCC::UnsignedGreaterThan,
                    Cmp::Ge => IntCC::UnsignedGreaterThanOrEqual,
                };
                let flag = self.builder.ins().icmp(condition, l, r);
                let flag = self.builder.ins().uextend(types::I64, flag);
                self.set(&dst.0, flag);
            }
            I::ByteFrom { dst, source, .. } if self.kinds[source.0 as usize] == Kind::Int => {
                let v = self.get(source);
                let n = self.builder.ins().sshr_imm_u(v, 1);
                let small = self.builder.ins().band_imm_u(v, 1);
                let in_range = self.builder.ins().icmp_imm_u(IntCC::UnsignedLessThanOrEqual, n, 255);
                let in_range = self.builder.ins().uextend(types::I64, in_range);
                let ok = self.builder.ins().band(small, in_range);
                let (fast, slow, done) = (self.builder.create_block(), self.builder.create_block(), self.builder.create_block());
                self.builder.append_block_param(done, types::I64);
                self.builder.ins().brif(ok, fast, &[], slow, &[]);
                self.builder.switch_to_block(fast);
                self.builder.ins().jump(done, &[n.into()]);
                self.builder.switch_to_block(slow);
                let result = self.exec_generic(pc, instruction);
                self.builder.ins().jump(done, &[result.into()]);
                self.builder.switch_to_block(done);
                let value = self.builder.block_params(done)[0];
                self.set(&dst.0, value);
            }
            other if generic(other) => self.generic_instruction(pc, other),
            _ => return Err(unsupported()),
        }
        Ok(false)
    }

    /// Is `register` statically a byte array (packed, one byte per element)?
    fn bytes_array(&self, register: okkama_sparrow::Register) -> bool {
        matches!(self.register_type(&register), Some(Type::Array(element)) if Kind::of(&self.loaded.program, *element) == Kind::Byte)
    }

    /// Element `position` (in bounds) of an array whose header is `header`: a
    /// word, or a byte of a packed array. One branch on two header bits keeps
    /// flat arrays on the plain load; a shared version (`heap::SHARED`, #246)
    /// reads from the address of its first element (one load more), a trie (`heap::TRIE`,
    /// #250) calls a helper. Byte arrays are never either.
    fn element(&mut self, array: Value, header: Value, position: Value, bytes: bool) -> Value {
        if bytes {
            let address = self.builder.ins().iadd(array, position);
            return self.builder.ins().uload8(types::I64, MEMORY, address, 8);
        }
        let special = self.builder.ins().band_imm_u(header, (heap::SHARED | heap::TRIE) as i64);
        let (flat, other, view, trie, done) = (
            self.builder.create_block(),
            self.builder.create_block(),
            self.builder.create_block(),
            self.builder.create_block(),
            self.builder.create_block(),
        );
        self.builder.append_block_param(done, types::I64);
        self.builder.ins().brif(special, other, &[], flat, &[]);
        self.builder.switch_to_block(flat);
        let offset = self.builder.ins().ishl_imm_u(position, 3);
        let address = self.builder.ins().iadd(array, offset);
        let value = self.builder.ins().load(types::I64, MEMORY, address, 8);
        self.builder.ins().jump(done, &[value.into()]);
        self.builder.switch_to_block(other);
        let shared = self.builder.ins().band_imm_u(header, heap::SHARED as i64);
        self.builder.ins().brif(shared, view, &[], trie, &[]);
        self.builder.switch_to_block(view);
        let first = self.builder.ins().load(types::I64, MEMORY, array, 8);
        let offset = self.builder.ins().ishl_imm_u(position, 3);
        let address = self.builder.ins().iadd(first, offset);
        let value = self.builder.ins().load(types::I64, MEMORY, address, 0);
        self.builder.ins().jump(done, &[value.into()]);
        self.builder.switch_to_block(trie);
        let ctx = self.ctx();
        let value = self.helper(|h| h.trie_get, &[ctx, array, position]).expect("result");
        self.builder.ins().jump(done, &[value.into()]);
        self.builder.switch_to_block(done);
        self.builder.block_params(done)[0]
    }

    /// Inline access to an Atomic cell (`[header, value, closed]`; a closed
    /// cell takes the intrinsic call, which throws AtomicClosed) and to the
    /// set flag of an Event cell (`[header, initial, set, waiters]`).
    fn atomic(&mut self, pc: usize, function: u32, access: Intrinsic, args: &[okkama_sparrow::Register]) -> Value {
        let cell = self.get(&args[0]);
        let ctx = self.ctx();
        match access {
            Intrinsic::AtomicClosed | Intrinsic::EventReady => self.builder.ins().load(types::I64, MEMORY, cell, 16),
            Intrinsic::AtomicClose => {
                let one = self.iconst(1);
                self.builder.ins().store(MEMORY, one, cell, 16);
                let unbound = self.iconst(0);
                self.builder.ins().store(MEMORY, unbound, cell, 8);
                self.iconst(0)
            }
            _ => {
                let closed = self.builder.ins().load(types::I64, MEMORY, cell, 16);
                let (open, slow, done) = (self.builder.create_block(), self.builder.create_block(), self.builder.create_block());
                self.builder.append_block_param(done, types::I64);
                self.builder.ins().brif(closed, slow, &[], open, &[]);
                self.builder.switch_to_block(open);
                let result = if access == Intrinsic::AtomicValue {
                    self.builder.ins().load(types::I64, MEMORY, cell, 8)
                } else {
                    let value = self.word(&args[1]);
                    self.builder.ins().store(MEMORY, value, cell, 8);
                    self.iconst(0)
                };
                self.builder.ins().jump(done, &[result.into()]);
                self.builder.switch_to_block(slow);
                let address = self.spill(args);
                let index = self.iconst(i64::from(function));
                let count = self.iconst(args.len() as i64);
                let result = self.helper(|h| h.intrinsic, &[ctx, index, address, count]).expect("result");
                self.check(pc);
                self.builder.ins().jump(done, &[result.into()]);
                self.builder.switch_to_block(done);
                self.builder.block_params(done)[0]
            }
        }
    }

    /// Run a failure or region instruction through `rt_failure_op`; its result.
    fn failure_generic(&mut self, pc: usize, instruction: &I) -> Value {
        let mut uses = Vec::new();
        instruction.visit(&mut |access| {
            if let okkama_sparrow::Access::Use(register) = access {
                uses.push(register);
            }
        });
        let ctx = self.ctx();
        let address = self.spill(&uses);
        let function = self.iconst(self.function as i64);
        let at = self.iconst(pc as i64);
        let result = self.helper(|h| h.failure_op, &[ctx, function, at, address]).expect("result");
        self.check(pc);
        result
    }

    /// Compile `instruction` as a call of the shared slow path, defining its result.
    fn generic_instruction(&mut self, pc: usize, instruction: &I) {
        let mut def = None;
        instruction.visit(&mut |access| {
            if let okkama_sparrow::Access::Def(register) = access {
                def = Some(register);
            }
        });
        let result = self.exec_generic(pc, instruction);
        if let Some(def) = def {
            self.set_word(&def, result);
        }
    }

    /// Run `instruction` through the shared slow path (`rt_exec`); its result word.
    fn exec_generic(&mut self, pc: usize, instruction: &I) -> Value {
        let mut uses = Vec::new();
        instruction.visit(&mut |access| {
            if let okkama_sparrow::Access::Use(register) = access {
                uses.push(register);
            }
        });
        let ctx = self.ctx();
        let address = self.spill(&uses);
        let function = self.iconst(self.function as i64);
        let at = self.iconst(pc as i64);
        let count = self.iconst(uses.len() as i64);
        let registers: Box<[okkama_sparrow::Register]> = uses.into_boxed_slice();
        let table = self.iconst(registers.as_ptr() as i64);
        self.jit.operands.push(registers);
        let result = self.helper(|h| h.exec, &[ctx, function, at, address, table, count]).expect("result");
        self.check(pc);
        result
    }

    /// Give a new object a resource identity when its type can reach a destroy.
    fn created(&mut self, descriptor: usize, object: Value) {
        let resources = &self.resources;
        if resources.enabled && resources.flags[descriptor] {
            let ctx = self.ctx();
            let descriptor = self.iconst(descriptor as i64);
            self.helper(|h| h.created, &[ctx, descriptor, object]);
        }
    }

    /// Jump to `target` after its edge moves, then define `register` as `word`.
    fn jump_then(&mut self, target: &Target, register: okkama_sparrow::Register, word: Value) {
        if self.polled && target.pc as usize <= self.pc {
            self.poll();
        }
        let values: Vec<_> = target.moves.iter().map(|(_, source)| self.get(source)).collect();
        for ((dst, _), value) in target.moves.iter().zip(values) {
            self.set(dst, value);
        }
        self.set_word(&register, word);
        let block = self.block(target.pc as usize);
        self.builder.ins().jump(block, &[]);
    }
}

/// Instructions executed by `failures::rt_failure_op`.
/// An instruction that may suspend its async frame and is re-entered on
/// resumption: `await` and the region closes that await async destroys (#154).
fn suspends(instruction: &I) -> bool {
    matches!(instruction, I::Await { .. } | I::RegionSettle { .. } | I::RegionSettleUnwind { .. })
}

fn failure_or_region(instruction: &I) -> bool {
    matches!(
        instruction,
        I::RegionEnter { .. }
            | I::RegionAdopt { .. }
            | I::RegionTrack { .. }
            | I::RegionTransfer { .. }
            | I::RegionLeave { .. }
            | I::RegionUnwind { .. }
            | I::FailSecondary { .. }
            | I::FailCaused { .. }
            | I::Raise { .. }
            | I::Panic { .. }
            | I::RoleNarrow { .. }
    )
}

/// Instructions executed by `ops::rt_exec`.
/// Atomic operations compiled inline (see `Emitter::atomic`).
fn atomic_access(callee: &okkama_sparrow::Function) -> Option<Intrinsic> {
    let intrinsic = Intrinsic::resolve(&callee.intrinsic.as_ref()?.name)?;
    matches!(
        intrinsic,
        Intrinsic::AtomicValue
            | Intrinsic::AtomicReplace
            | Intrinsic::AtomicClosed
            | Intrinsic::AtomicClose
            | Intrinsic::EventReady
    )
    .then_some(intrinsic)
}

fn generic(instruction: &I) -> bool {
    matches!(
        instruction,
        I::ByteAnd { .. }
            | I::ByteOr { .. }
            | I::ByteXor { .. }
            | I::ByteShl { .. }
            | I::ByteShr { .. }
            | I::ByteUshr { .. }
            | I::ByteNot { .. }
            | I::Bits { .. }
            | I::RangeNew { .. }
            | I::RangePart { .. }
            | I::StrUtf8 { .. }
            | I::StrFn { .. }
            | I::GlobalGet { .. }
            | I::StrReplaceFirst { .. }
            | I::IntAnd { .. }
            | I::IntOr { .. }
            | I::IntXor { .. }
            | I::IntShl { .. }
            | I::IntShr { .. }
            | I::IntUshr { .. }
            | I::IntNot { .. }
            | I::IntNeg { .. }
            | I::IntAbs { .. }
            | I::Sign { .. }
            | I::FloatMod { .. }
            | I::FloatPow { .. }
            | I::FloatSqrt { .. }
            | I::Convert { .. }
            | I::ByteFrom { .. }
            | I::RatNew { .. }
            | I::RatAdd { .. }
            | I::RatSub { .. }
            | I::RatMul { .. }
            | I::RatDiv { .. }
            | I::RatMod { .. }
            | I::RatNeg { .. }
            | I::RatAbs { .. }
            | I::RatCmp { .. }
            | I::RatPart { .. }
            | I::StrCmp { .. }
            | I::StrLen { .. }
            | I::StrSlice { .. }
            | I::StrSplit { .. }
            | I::StrReplace { .. }
            | I::StrTrim { .. }
            | I::StrChomp { .. }
            | I::EnumCase { .. }
            | I::EnumCmp { .. }
            | I::ValCmp { .. }
            | I::Undef { .. }
            | I::AnyBox { .. }
            | I::ArraySlice { .. }
            | I::ArrayReplace { .. }
            | I::ArrayRemove { .. }
            | I::ArrayJoin { .. }
            | I::ArrayConcat { .. }
            | I::ArrayDiff { .. }
            | I::MapMerge { .. }
            | I::MapDiff { .. }
            | I::BuilderNew { .. }
            | I::BuilderPush { .. }
            | I::BuilderAppend { .. }
            | I::BuilderRange { .. }
            | I::BuilderFinish { .. }
            | I::MapNew { .. }
            | I::MapLen { .. }
            | I::MapHas { .. }
            | I::MapGet { .. }
            | I::MapKeys { .. }
            | I::MapValues { .. }
            | I::MapBuilderNew { .. }
            | I::MapBuilderPut { .. }
            | I::MapBuilderFinish { .. }
            | I::WorkerSpawn { .. }
            | I::WorkerBind { .. }
    )
}

fn int_condition(op: Cmp) -> IntCC {
    match op {
        Cmp::Eq => IntCC::Equal,
        Cmp::Ne => IntCC::NotEqual,
        Cmp::Lt => IntCC::SignedLessThan,
        Cmp::Le => IntCC::SignedLessThanOrEqual,
        Cmp::Gt => IntCC::SignedGreaterThan,
        Cmp::Ge => IntCC::SignedGreaterThanOrEqual,
    }
}
