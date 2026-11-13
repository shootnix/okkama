//! Stackless async: the heap frame of an async call is also its Future.
//!
//! Frame words after the header: state, result, failure, waiter, awaiting,
//! resume point, flags, then every register of the function. An async body is
//! compiled as `(ctx, frame) -> 0 | 1` (completed or pending): it starts at
//! resume point 0 with its parameters in the frame, and a suspended `await` at
//! `pc` re-enters at `pc + 1` with every register reloaded. Whoever runs a body
//! (an `await` of a cold Future, `spawn`, the scheduler) turns a failure that
//! escaped it into the Future's failure. Semantics follow the previous engine
//! (`future.rs`, `await_future`, `take_cancellation`, `shutdown_tasks`).
//!
//! Async natives and intrinsics are frames too, run by a Rust driver
//! (`drivers`) instead of machine code. A driver that waits is PARKED: on a
//! resource (event, lock, timer…) or on other Futures; whatever completes the
//! wait wakes it through [`Runtime::wake`]. A cancellation reaching a parked
//! driver resumes it with DELIVER_NOW, and the driver withdraws its request.
#![allow(unsafe_code)]

use crate::heap::{self, Payload};
use crate::runtime::{Ctx, FAILED, FailureKind, OK, Runtime};
use crate::word::{self, Repr, UNBOUND, Word};

pub const COLD: u64 = 0;
pub const RUNNING: u64 = 1;
pub const DONE: u64 = 2;
pub const FAILURE_STATE: u64 = 3;

pub const STATE: u64 = 8;
pub const RESULT: u64 = 16;
pub const FAILURE: u64 = 24;
pub const WAITER: u64 = 32;
pub const AWAITING: u64 = 40;
pub const RESUME: u64 = 48;
pub const FLAGS: u64 = 56;
pub const REGISTERS: u64 = 64;
pub const FIXED_WORDS: usize = 7;

const SPAWNED: u64 = 1;
const OBSERVED: u64 = 2;
const CANCEL_REQUESTED: u64 = 4;
const CANCEL_DELIVERED: u64 = 8;
/// Resumed to receive its task's cancellation at the pending `await`.
pub const DELIVER_NOW: u64 = 16;
/// A driver waiting for a wake (see the module comment).
pub const PARKED: u64 = 32;
/// An async destroy awaited by a region close (#154): a cancellation neither
/// reaches it nor resumes its awaiter; it stays requested for the next await.
const SHIELDED: u64 = 64;
/// Flags above this bit hold the completion order of a settled Future.
const ORDER_SHIFT: u64 = 16;

/// Several frames awaiting one Future.
#[derive(Debug, Default)]
pub struct Waiters(pub Vec<Word>);
impl Payload for Waiters {
    fn trace(&self, visit: &mut dyn FnMut(Word, Repr)) {
        for &waiter in &self.0 {
            visit(waiter, Repr::Ref);
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[inline]
pub(crate) fn get(frame: Word, offset: u64) -> u64 {
    // SAFETY: `frame` is a live async frame; offsets are within its fixed words.
    unsafe { *((frame + offset) as *const u64) }
}
#[inline]
pub(crate) fn set(frame: Word, offset: u64, value: u64) {
    // SAFETY: as in `get`.
    unsafe { *((frame + offset) as *mut u64) = value }
}

impl Runtime {
    /// The header of a frame names its function (its layout in `Layouts::frames`).
    pub(crate) fn function_of(&self, frame: Word) -> usize {
        heap::payload(self.heap.header_of(frame)) as usize
    }

    fn is_driver(&self, frame: Word) -> bool {
        self.loaded.functions[self.function_of(frame)].driver
    }

    /// Offset of the state word of a driver frame (after its registers).
    pub(crate) fn driver_slot(&self, frame: Word) -> u64 {
        REGISTERS + self.loaded.functions[self.function_of(frame)].kinds.len() as u64 * 8
    }

    /// Completion order of a settled Future (0: not settled).
    pub(crate) fn completion_order(frame: Word) -> u64 {
        get(frame, FLAGS) >> ORDER_SHIFT
    }

    /// Queue a frame whose wait is over. A driver is queued once per park.
    pub(crate) fn wake(&mut self, frame: Word) {
        if self.is_driver(frame) {
            let flags = get(frame, FLAGS);
            if flags & PARKED == 0 {
                return;
            }
            set(frame, FLAGS, flags & !PARKED);
        }
        self.ready.push_back(frame);
    }

    /// True while `frame` is a parked driver (not woken, not being cancelled).
    pub(crate) fn parked(frame: Word) -> bool {
        get(frame, FLAGS) & PARKED != 0
    }

    /// A cold Future of async `function` with its parameters.
    pub fn future_new(&mut self, function: usize, arguments: &[Word]) -> Word {
        self.count(|r| format!("future {}", r.loaded.program.functions[function].symbol));
        let source = &self.loaded.program.functions[function];
        let registers = source.registers.len();
        let words = 1 + FIXED_WORDS + registers + usize::from(self.loaded.functions[function].driver);
        self.heap.counted("async frame", 0, words);
        let frame = self.heap.allocate(words);
        // SAFETY: fresh storage of `words` words.
        unsafe {
            *(frame as *mut u64) = heap::header(heap::FRAME, Repr::Raw, function as u32);
            std::ptr::write_bytes((frame + 8) as *mut u64, 0, words - 1);
        }
        let kinds = &self.loaded.functions[function].kinds;
        for (register, kind) in kinds.iter().enumerate() {
            if matches!(kind, crate::program::Kind::Float | crate::program::Kind::Raw | crate::program::Kind::Byte) {
                set(frame, REGISTERS + register as u64 * 8, word::UNBOUND_RAW);
            }
        }
        for (param, &argument) in source.params.iter().zip(arguments) {
            set(frame, REGISTERS + u64::from(param.0) * 8, argument);
        }
        frame
    }

    fn add_waiter(&mut self, future: Word, waiter: Word) {
        self.listen(future, waiter);
        set(waiter, AWAITING, future);
    }

    /// Add `waiter` to the frames woken when `future` settles.
    pub(crate) fn listen(&mut self, future: Word, waiter: Word) {
        let current = get(future, WAITER);
        if current == UNBOUND {
            set(future, WAITER, waiter);
        } else if heap::kind(self.heap.header_of(current)) == heap::FRAME {
            let list = self.heap.allocate_payload(Box::new(Waiters(vec![current, waiter])));
            set(future, WAITER, list);
        } else {
            self.heap.payload_mut::<Waiters>(current).expect("waiters").0.push(waiter);
        }
    }

    pub(crate) fn remove_waiter(&mut self, future: Word, waiter: Word) {
        let current = get(future, WAITER);
        if current == waiter {
            set(future, WAITER, UNBOUND);
        } else if current != UNBOUND && heap::kind(self.heap.header_of(current)) == heap::PAYLOAD {
            self.heap.payload_mut::<Waiters>(current).expect("waiters").0.retain(|&w| w != waiter);
        }
    }

    /// Settle a Future and queue the frames awaiting it.
    pub(crate) fn settle(&mut self, frame: Word, state: u64, value: Word) {
        set(frame, STATE, state);
        self.completions += 1;
        let flags = get(frame, FLAGS) & ((1 << ORDER_SHIFT) - 1);
        set(frame, FLAGS, flags | (self.completions << ORDER_SHIFT));
        if state == DONE {
            set(frame, RESULT, value);
        } else {
            set(frame, FAILURE, value);
        }
        let waiters = get(frame, WAITER);
        set(frame, WAITER, UNBOUND);
        if waiters == UNBOUND {
            return;
        }
        // One waiter, the common `await`, is the frame itself: woken without
        // a vector (#440). Several are a `Waiters` list.
        if heap::kind(self.heap.header_of(waiters)) == heap::FRAME {
            self.wake_waiter(frame, waiters);
            return;
        }
        let list = std::mem::take(&mut self.heap.payload_mut::<Waiters>(waiters).expect("waiters").0);
        for waiter in list {
            self.wake_waiter(frame, waiter);
        }
    }

    /// Wake `waiter`, which waited for the settled `frame`.
    fn wake_waiter(&mut self, frame: Word, waiter: Word) {
        if get(waiter, AWAITING) == frame {
            set(waiter, AWAITING, UNBOUND);
        }
        self.wake(waiter);
    }

    /// A Future that is already settled (for drivers' internal races).
    pub(crate) fn settled_new(&mut self, function: usize, state: u64, value: Word) -> Word {
        let future = self.future_new(function, &[]);
        self.settle(future, state, value);
        future
    }

    pub(crate) fn cancelled_failure(&mut self) -> Word {
        self.failure_new(FailureKind::Cancelled)
    }

    /// Run (start or resume) the body of a RUNNING frame on this stack.
    /// Returns false while it is pending; a failure that escaped the body
    /// settles the Future. A fatal engine stop is left in the context.
    pub fn run_body(&mut self, ctx: *mut Ctx, frame: Word) -> bool {
        let function = self.function_of(frame);
        if self.loaded.functions[function].driver {
            self.running.push(frame);
            let pending = crate::drivers::drive(self, ctx, frame);
            self.running.pop();
            return !pending;
        }
        let code = match crate::jit::compile(self, function) {
            Ok(code) => code,
            Err(error) => {
                crate::runtime::fatal(unsafe { &mut *ctx }, self, error.0);
                return true;
            }
        };
        self.running.push(frame);
        // SAFETY: async bodies take their frame.
        let pending = unsafe { crate::jit::invoke(code, ctx, &[frame]) } != 0;
        self.running.pop();
        let ctx = unsafe { &mut *ctx };
        if ctx.status == FAILED {
            ctx.status = OK;
            let failure = ctx.failure;
            ctx.failure = UNBOUND;
            self.settle(frame, FAILURE_STATE, failure);
            return true;
        }
        ctx.status != OK || !pending
    }

    /// Mark a cancellation of a running task delivered and return its failure.
    fn deliverable_cancel(&mut self) -> Option<Word> {
        // The frames running on this stack, innermost first, up to the task.
        for index in (0..self.running.len()).rev() {
            let frame = self.running[index];
            let flags = get(frame, FLAGS);
            if flags & DELIVER_NOW != 0 {
                set(frame, FLAGS, flags & !DELIVER_NOW);
                return Some(self.cancelled_failure());
            }
            if flags & CANCEL_REQUESTED != 0 && flags & CANCEL_DELIVERED == 0 {
                set(frame, FLAGS, flags | CANCEL_DELIVERED);
                return Some(self.cancelled_failure());
            }
            if flags & (SPAWNED | SHIELDED) != 0 {
                break;
            }
        }
        None
    }

    /// Deliver a cancellation to a suspended task: its innermost waiting
    /// frame resumes and its pending `await` raises Cancelled.
    fn deliver_cancel(&mut self, task: Word) {
        let mut frame = task;
        loop {
            let awaited = get(frame, AWAITING);
            if awaited == UNBOUND {
                break;
            }
            let header = self.heap.header_of(awaited);
            if heap::kind(header) != heap::FRAME || get(awaited, STATE) != RUNNING {
                break;
            }
            if get(awaited, FLAGS) & SHIELDED != 0 {
                return;
            }
            if get(awaited, FLAGS) & SPAWNED == 0 {
                frame = awaited;
                continue;
            }
            // Structured await (#147): a spawned task that no other frame
            // awaits is cancelled with its awaiter, which keeps waiting and
            // raises Cancelled once that task has settled.
            if !self.is_driver(frame) && self.sole_awaiter(awaited, frame) {
                set(task, FLAGS, get(task, FLAGS) | CANCEL_DELIVERED);
                set(frame, FLAGS, get(frame, FLAGS) | DELIVER_NOW);
                self.cancel_running(awaited);
                return;
            }
            break;
        }
        let awaited = get(frame, AWAITING);
        if awaited != UNBOUND {
            self.remove_waiter(awaited, frame);
            set(frame, AWAITING, UNBOUND);
        } else if !Self::parked(frame) {
            // Not waiting (or its wait already completed): the cancellation
            // stays requested for the next `await`.
            return;
        }
        set(frame, FLAGS, get(frame, FLAGS) & !PARKED);
        set(task, FLAGS, get(task, FLAGS) | CANCEL_DELIVERED);
        set(frame, FLAGS, get(frame, FLAGS) | DELIVER_NOW);
        self.ready.push_back(frame);
    }

    /// True when no frame but `frame` awaits `future` (frames that only
    /// listen to it, such as a group's, do not count).
    fn sole_awaiter(&self, future: Word, frame: Word) -> bool {
        let waiters = get(future, WAITER);
        if waiters == UNBOUND || heap::kind(self.heap.header_of(waiters)) == heap::FRAME {
            return true;
        }
        self.heap
            .payload::<Waiters>(waiters)
            .expect("waiters")
            .0
            .iter()
            .all(|&waiter| waiter == frame || get(waiter, AWAITING) != future)
    }

    /// Resume ready frames until `root` settles (or nothing can run).
    pub fn schedule(&mut self, ctx: *mut Ctx, root: Word) {
        while get(root, STATE) == RUNNING || get(root, STATE) == COLD {
            if (self.limits.interrupt.is_some() || self.limits.memory.is_some())
                && let Some(what) = self.over_budget() {
                    self.exhaust(unsafe { &mut *ctx }, what);
                    return;
                }
            self.poll_io();
            let Some(frame) = self.ready.pop_front() else {
                if self.wait_external() {
                    continue;
                }
                // A runtime failure of the run, worded as the IR engine and
                // the JVM report it (#342).
                let ctx = unsafe { &mut *ctx };
                ctx.failure = self.failure_new(FailureKind::Program("async main cannot complete: no task can make progress".into()));
                ctx.status = FAILED;
                return;
            };
            if get(frame, STATE) != RUNNING {
                continue;
            }
            self.run_body(ctx, frame);
            if unsafe { (*ctx).status } != OK {
                return;
            }
        }
    }

    /// Await `future` from driver `frame`: its result, its failure, or None
    /// when `frame` now waits for it (parked; woken when it settles).
    pub(crate) fn driver_await(&mut self, ctx: *mut Ctx, frame: Word, future: Word) -> Option<Result<Word, Word>> {
        set(future, FLAGS, get(future, FLAGS) | OBSERVED);
        if get(future, STATE) == COLD {
            set(future, STATE, RUNNING);
            self.run_body(ctx, future);
            if unsafe { (*ctx).status } != OK {
                return None;
            }
        }
        match get(future, STATE) {
            DONE => Some(Ok(get(future, RESULT))),
            FAILURE_STATE => Some(Err(get(future, FAILURE))),
            _ => {
                self.add_waiter(future, frame);
                None
            }
        }
    }

    /// Start `future` as a spawned task (no effect unless it is cold).
    pub(crate) fn spawn(&mut self, ctx: *mut Ctx, future: Word) {
        if get(future, STATE) == COLD {
            set(future, FLAGS, get(future, FLAGS) | SPAWNED);
            set(future, STATE, RUNNING);
            self.spawned.push(future);
            self.run_body(ctx, future);
        }
    }

    /// Drop finished spawned tasks that shutdown no longer needs (#503): it
    /// cancels running ones and reports unobserved failures. A long-running
    /// program (a server) otherwise kept every task it ever spawned, with
    /// everything its frame refers to, as a root of every collection.
    pub(crate) fn prune_spawned(&mut self) {
        let mut kept = Vec::with_capacity(self.spawned.len());
        for &task in &self.spawned {
            let state = get(task, STATE);
            let needed = match state {
                COLD | RUNNING => true,
                FAILURE_STATE => {
                    get(task, FLAGS) & OBSERVED == 0
                        && !matches!(self.failure(get(task, FAILURE)).map(|f| &f.kind), Some(FailureKind::Cancelled))
                }
                _ => false,
            };
            if needed {
                kept.push(task);
            }
        }
        self.spawned = kept;
    }

    pub(crate) fn observe(future: Word) {
        set(future, FLAGS, get(future, FLAGS) | OBSERVED);
    }

    /// After the root: cancel running spawned tasks and wait for them; an
    /// unobserved failure of a spawned task (not a cancellation) is reported.
    pub fn shutdown(&mut self, ctx: *mut Ctx) -> Option<Word> {
        loop {
            let mut pending = None;
            for index in 0..self.spawned.len() {
                let task = self.spawned[index];
                self.cancel(ctx, task);
                if pending.is_none() && get(task, STATE) == RUNNING {
                    pending = Some(task);
                }
            }
            match pending {
                Some(task) => {
                    self.schedule(ctx, task);
                    if unsafe { (*ctx).status } != OK {
                        return None;
                    }
                }
                None => break,
            }
        }
        let failure = self.spawned.iter().copied().find_map(|task| {
            let failed = get(task, STATE) == FAILURE_STATE && get(task, FLAGS) & OBSERVED == 0;
            let failure = get(task, FAILURE);
            (failed && !matches!(self.failure(failure).map(|f| &f.kind), Some(FailureKind::Cancelled)))
                .then_some(failure)
        });
        self.spawned.clear();
        failure
    }

    pub(crate) fn cancel(&mut self, ctx: *mut Ctx, future: Word) {
        let _ = ctx;
        match get(future, STATE) {
            COLD => {
                let failure = self.cancelled_failure();
                self.settle(future, FAILURE_STATE, failure);
            }
            RUNNING => self.cancel_running(future),
            _ => {}
        }
    }

    fn cancel_running(&mut self, future: Word) {
        let flags = get(future, FLAGS);
        if flags & CANCEL_REQUESTED == 0 {
            set(future, FLAGS, flags | CANCEL_REQUESTED);
            if get(future, AWAITING) != UNBOUND || Self::parked(future) {
                self.deliver_cancel(future);
            }
        }
    }
}

// ---- helpers called from machine code ----

unsafe fn parts<'a>(ctx: *mut Ctx) -> (&'a mut Ctx, &'a mut Runtime) {
    let ctx = unsafe { &mut *ctx };
    let runtime = unsafe { &mut *ctx.runtime };
    (ctx, runtime)
}

/// A cold Future of async `function` with `count` argument words.
#[inline(never)]
pub extern "C" fn rt_future_new(ctx: *mut Ctx, function: u64, arguments: *const Word, count: u64) -> Word {
    let frame_pointer = crate::runtime::frame_pointer!();
    let (ctx, runtime) = unsafe { parts(ctx) };
    let arguments = unsafe { std::slice::from_raw_parts(arguments, count as usize) };
    let future = runtime.future_new(function as usize, arguments);
    runtime.safepoint(frame_pointer, ctx, &[(future, Repr::Ref)]);
    future
}

pub const AWAIT_READY: u64 = 0;
pub const AWAIT_PENDING: u64 = 2;

/// `await future` from frame `this`: ready (value in `scratch`), failed
/// (status set), or pending (`this` now waits for it).
#[inline(never)]
pub extern "C" fn rt_await(ctx_ptr: *mut Ctx, this: Word, future: Word) -> u64 {
    let (ctx, runtime) = unsafe { parts(ctx_ptr) };
    runtime.count(|_| "await".into());
    if let Some(failure) = runtime.deliverable_cancel() {
        ctx.failure = failure;
        ctx.status = FAILED;
        return AWAIT_READY;
    }
    set(future, FLAGS, get(future, FLAGS) | OBSERVED);
    if get(future, STATE) == COLD {
        set(future, STATE, RUNNING);
        runtime.run_body(ctx_ptr, future);
        let ctx = unsafe { &mut *ctx_ptr };
        if ctx.status != OK {
            return AWAIT_READY;
        }
    }
    let ctx = unsafe { &mut *ctx_ptr };
    match get(future, STATE) {
        DONE => {
            ctx.scratch = get(future, RESULT);
            AWAIT_READY
        }
        FAILURE_STATE => {
            ctx.failure = get(future, FAILURE);
            ctx.status = FAILED;
            AWAIT_READY
        }
        _ => {
            runtime.add_waiter(future, this);
            AWAIT_PENDING
        }
    }
}

/// `region.settle` / `region.settle_unwind` from frame `this` (#154): close
/// the region from its last identity, awaiting each async destroy shielded
/// from cancellation. Ready (the unwind failure in `scratch`; a leave's
/// destroy failure raised) or pending: `this` waits for the destroy, keeps it
/// in the region, and re-enters here when it settles.
#[inline(never)]
pub extern "C" fn rt_region_settle(ctx_ptr: *mut Ctx, this: Word, region: Word, keep: Word, failure: Word) -> u64 {
    let (_, runtime) = unsafe { parts(ctx_ptr) };
    let unwind = failure != UNBOUND;
    let link = |runtime: &mut Runtime, primary: Option<Word>, cleanup: Word| match primary {
        Some(primary) => runtime.failure_link(primary, cleanup, true),
        None => cleanup,
    };
    let (settling, settled) = runtime.region_settling(region);
    let mut primary = settled.or(unwind.then_some(failure));
    if let Some(future) = settling {
        match get(future, STATE) {
            DONE => {}
            FAILURE_STATE => primary = Some(link(runtime, primary, get(future, FAILURE))),
            _ => {
                runtime.add_waiter(future, this);
                return AWAIT_PENDING;
            }
        }
        runtime.set_region_settling(region, None, primary);
    }
    let kept = if unwind {
        runtime.failure_payloads(failure)
    } else if keep == UNBOUND {
        Vec::new()
    } else {
        vec![keep]
    };
    loop {
        let (object, so_far) = match runtime.region_close_until(ctx_ptr, region, &kept, primary, true) {
            crate::failures::Closed::Done(done) => {
                primary = done;
                break;
            }
            crate::failures::Closed::Async(object, so_far) => (object, so_far),
        };
        primary = so_far;
        let descriptor = heap::payload(runtime.heap.header_of(object)) as usize;
        let destroy = runtime.loaded.program.objects[descriptor].destroy.expect("an async destroy").0 as usize;
        let rooted = [Some(region), Some(this), primary, (keep != UNBOUND).then_some(keep), unwind.then_some(failure)];
        let depth = runtime.roots.len();
        runtime.roots.extend(rooted.iter().flatten().map(|&word| (word, Repr::Ref)));
        let future = runtime.future_new(destroy, &[object]);
        set(future, FLAGS, get(future, FLAGS) | SHIELDED | OBSERVED);
        set(future, STATE, RUNNING);
        runtime.roots.push((future, Repr::Ref));
        runtime.run_body(ctx_ptr, future);
        runtime.roots.truncate(depth);
        if unsafe { (*ctx_ptr).status } != OK {
            return AWAIT_READY;
        }
        match get(future, STATE) {
            DONE => {}
            FAILURE_STATE => primary = Some(link(runtime, primary, get(future, FAILURE))),
            _ => {
                runtime.set_region_settling(region, Some(future), primary);
                runtime.add_waiter(future, this);
                return AWAIT_PENDING;
            }
        }
    }
    runtime.set_region_settling(region, None, None);
    let ctx = unsafe { &mut *ctx_ptr };
    if unwind {
        ctx.scratch = primary.unwrap_or(failure);
    } else if let Some(primary) = primary {
        ctx.failure = primary;
        ctx.status = FAILED;
    }
    AWAIT_READY
}

/// `return value` of an async body: settle its Future.
#[inline(never)]
pub extern "C" fn rt_complete(ctx: *mut Ctx, frame: Word, value: Word) {
    let (_, runtime) = unsafe { parts(ctx) };
    runtime.settle(frame, DONE, value);
}

/// `spawn future`: start a cold Future now, on this stack.
#[inline(never)]
pub extern "C" fn rt_spawn(ctx_ptr: *mut Ctx, future: Word) -> Word {
    let (_, runtime) = unsafe { parts(ctx_ptr) };
    runtime.spawn(ctx_ptr, future);
    future
}

/// `fut` operations: done, cancelled (flags) and cancel.
#[inline(never)]
pub extern "C" fn rt_fut(ctx_ptr: *mut Ctx, op: u64, future: Word) -> Word {
    let (_, runtime) = unsafe { parts(ctx_ptr) };
    let state = get(future, STATE);
    match op {
        0 => {
            runtime.cancel(ctx_ptr, future);
            UNBOUND
        }
        1 => Word::from(state == DONE || state == FAILURE_STATE),
        _ => Word::from(
            state == FAILURE_STATE
                && matches!(runtime.failure(get(future, FAILURE)).map(|f| &f.kind), Some(FailureKind::Cancelled)),
        ),
    }
}
