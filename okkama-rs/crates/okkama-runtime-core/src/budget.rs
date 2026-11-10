//! Execution budgets of a run on the IR engine (#71): the limits and reports
//! of the Sparrow core (`--max-steps`, `--max-memory`, `--max-output`,
//! `--timeout`). Steps are function entries and backward branches; memory is
//! what the process allocator holds beyond the start of the run (the
//! embedder's probe), or what this run's thread holds through [`Counted`]
//! ([`MemoryScope::Runtime`], #269); output is bytes written to stdout and stderr; the
//! interrupt flag is set by the host (the CLI's timer). Running out stops the
//! run with an error no handler catches.
use std::alloc::{GlobalAlloc, Layout};
use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

/// Budgets of one run; `None` is unbounded.
#[derive(Debug, Clone, Default)]
pub struct Limits {
    pub steps: Option<u64>,
    pub memory: Option<u64>,
    /// What `memory` measures.
    pub memory_scope: MemoryScope,
    pub output: Option<u64>,
    pub interrupt: Option<Arc<AtomicBool>>,
}

/// What a memory budget measures (#269).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MemoryScope {
    /// What the process allocator holds, through the embedder's probe:
    /// every thread's memory counts. The CLIs run one runtime per process.
    #[default]
    Process,
    /// This runtime's own memory, so that runtimes in one process keep
    /// budgets of their own. On the IR engine: what the run's thread has
    /// allocated through [`Counted`] and not freed (its values never leave
    /// that thread); on the Sparrow core: its heap. Memory a runtime cannot
    /// see, such as native libraries' and other threads', does not count.
    Runtime,
}

/// Net bytes allocated through [`Counted`], by every thread and by this one.
static NET: AtomicIsize = AtomicIsize::new(0);

thread_local! {
    static THREAD_NET: Cell<isize> = const { Cell::new(0) };
}

/// A counting global allocator for embedders without a probe of their own
/// (and tests): [`counted_bytes`] is its process probe and
/// [`thread_counted_bytes`] the measure of [`MemoryScope::Runtime`] on the IR
/// engine. It costs an atomic and a thread-local add per call, so the CLIs
/// use their allocator's statistics instead.
#[derive(Debug)]
pub struct Counted<A>(pub A);

#[inline]
fn count(bytes: isize) {
    NET.fetch_add(bytes, Ordering::Relaxed);
    // A thread being torn down has no counter left to keep.
    let _ = THREAD_NET.try_with(|net| net.set(net.get() + bytes));
}

// SAFETY: every call forwards to the wrapped allocator unchanged.
#[allow(unsafe_code)]
unsafe impl<A: GlobalAlloc> GlobalAlloc for Counted<A> {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size() as isize);
        unsafe { self.0.alloc(layout) }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size() as isize);
        unsafe { self.0.alloc_zeroed(layout) }
    }

    #[inline]
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        count(-(layout.size() as isize));
        unsafe { self.0.dealloc(pointer, layout) }
    }

    #[inline]
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count(size as isize - layout.size() as isize);
        unsafe { self.0.realloc(pointer, layout, size) }
    }
}

/// The process probe of [`Counted`].
pub fn counted_bytes() -> usize {
    NET.load(Ordering::Relaxed).max(0) as usize
}

/// Net bytes this thread has allocated through [`Counted`]; a thread that
/// frees what another allocated can go below zero, read as 0.
pub fn thread_counted_bytes() -> usize {
    THREAD_NET.with(Cell::get).max(0) as usize
}

/// Whether [`Counted`] is the global allocator: an allocation moves this
/// thread's counter.
fn counting() -> bool {
    let before = THREAD_NET.with(Cell::get);
    let probe = std::hint::black_box(Box::new(0u64));
    let moved = THREAD_NET.with(Cell::get) != before;
    drop(probe);
    moved
}

/// What ran out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exhausted {
    Steps,
    Memory,
    Output,
    Interrupted,
}

impl Exhausted {
    /// The report, as the Sparrow core words it.
    pub fn message(self) -> &'static str {
        match self {
            Exhausted::Steps => "Execution budget exhausted: steps",
            Exhausted::Memory => "Execution budget exhausted: memory",
            Exhausted::Output => "Execution budget exhausted: output",
            Exhausted::Interrupted => "Execution interrupted",
        }
    }
}

/// The bytes the process allocator holds now (set by the embedder).
pub type MemoryProbe = fn() -> usize;

static PROBE: std::sync::OnceLock<MemoryProbe> = std::sync::OnceLock::new();

/// Install the memory probe once per process; later calls are ignored.
pub fn set_memory_probe(probe: MemoryProbe) {
    let _ = PROBE.set(probe);
}

/// Steps between polls of memory and the interrupt flag.
const POLL: u64 = 1024;

struct State {
    steps: Option<u64>,
    output: Option<u64>,
    memory: Option<(u64, usize, MemoryProbe)>,
    interrupt: Option<Arc<AtomicBool>>,
    until_poll: u64,
    /// Exhaustion found inside a long intrinsic (#238), for the interpreter
    /// to report when the intrinsic returns.
    pending: Option<Exhausted>,
}

thread_local! {
    static BUDGET: RefCell<Option<State>> = const { RefCell::new(None) };
    /// Any budget on this thread: a run without one pays one load per step.
    static ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The budgets of the run on this thread, until the guard is dropped.
/// A memory budget of the process without a probe, or of the runtime
/// without [`Counted`], is an error.
pub fn begin(limits: &Limits) -> Result<Guard, String> {
    let memory = match (limits.memory, limits.memory_scope) {
        (Some(limit), MemoryScope::Process) => {
            let probe = *PROBE.get().ok_or("a memory budget needs the embedder's memory probe")?;
            Some((limit, probe(), probe))
        }
        (Some(limit), MemoryScope::Runtime) => {
            if !counting() {
                return Err("a runtime memory budget needs the Counted global allocator".into());
            }
            let probe: MemoryProbe = thread_counted_bytes;
            Some((limit, probe(), probe))
        }
        (None, _) => None,
    };
    let state = State {
        steps: limits.steps,
        output: limits.output,
        memory,
        interrupt: limits.interrupt.clone(),
        until_poll: POLL,
        pending: None,
    };
    let active = state.steps.is_some() || state.output.is_some() || state.memory.is_some() || state.interrupt.is_some();
    BUDGET.with(|budget| *budget.borrow_mut() = active.then_some(state));
    ACTIVE.with(|flag| flag.set(active));
    Ok(Guard(std::marker::PhantomData))
}

/// Clears the budgets of the run when dropped.
#[derive(Debug)]
pub struct Guard(std::marker::PhantomData<*const ()>);

impl Drop for Guard {
    fn drop(&mut self) {
        BUDGET.with(|budget| *budget.borrow_mut() = None);
        ACTIVE.with(|flag| flag.set(false));
    }
}

/// One step (a call or a backward branch).
#[inline]
pub fn step() -> Result<(), Exhausted> {
    if !ACTIVE.with(std::cell::Cell::get) {
        return Ok(());
    }
    BUDGET.with(|budget| {
        let mut budget = budget.borrow_mut();
        let Some(state) = budget.as_mut() else { return Ok(()) };
        if let Some(steps) = &mut state.steps {
            if *steps == 0 {
                return Err(Exhausted::Steps);
            }
            *steps -= 1;
        }
        state.until_poll -= 1;
        if state.until_poll == 0 {
            state.until_poll = POLL;
            if state.interrupt.as_ref().is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                return Err(Exhausted::Interrupted);
            }
            if let Some((limit, base, probe)) = state.memory
                && probe().saturating_sub(base) as u64 > limit {
                    return Err(Exhausted::Memory);
                }
        }
        Ok(())
    })
}

/// The interrupt flag and the memory budget, checked now by a long intrinsic
/// (a sort, #238). Exhaustion is also kept for [`take_pending`]: the
/// intrinsic returns a failure, and the interpreter reports the exhaustion
/// instead of letting a handler see that failure.
pub fn check_now() -> Result<(), Exhausted> {
    if !ACTIVE.with(std::cell::Cell::get) {
        return Ok(());
    }
    BUDGET.with(|budget| {
        let mut budget = budget.borrow_mut();
        let Some(state) = budget.as_mut() else { return Ok(()) };
        let exhausted = if state.interrupt.as_ref().is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            Some(Exhausted::Interrupted)
        } else {
            state
                .memory
                .filter(|(limit, base, probe)| probe().saturating_sub(*base) as u64 > *limit)
                .map(|_| Exhausted::Memory)
        };
        match exhausted {
            Some(what) => {
                state.pending = Some(what);
                Err(what)
            }
            None => Ok(()),
        }
    })
}

/// The run's interrupt flag, for work outside this thread's polls: a long
/// host operation watches it (#238).
pub fn interrupt_flag() -> Option<Arc<AtomicBool>> {
    if !ACTIVE.with(std::cell::Cell::get) {
        return None;
    }
    BUDGET.with(|budget| budget.borrow().as_ref().and_then(|state| state.interrupt.clone()))
}

/// Exhaustion a long intrinsic found (see [`check_now`]), once.
pub fn take_pending() -> Option<Exhausted> {
    if !ACTIVE.with(std::cell::Cell::get) {
        return None;
    }
    BUDGET.with(|budget| budget.borrow_mut().as_mut().and_then(|state| state.pending.take()))
}

/// `bytes` about to be written to stdout or stderr.
pub fn output(bytes: usize) -> Result<(), Exhausted> {
    BUDGET.with(|budget| {
        let mut budget = budget.borrow_mut();
        let Some(left) = budget.as_mut().and_then(|state| state.output.as_mut()) else { return Ok(()) };
        if bytes as u64 > *left {
            *left = 0;
            return Err(Exhausted::Output);
        }
        *left -= bytes as u64;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_and_output_run_out_and_the_guard_clears_them() {
        {
            let _run = begin(&Limits { steps: Some(3), output: Some(5), ..Limits::default() }).unwrap();
            assert!((0..3).all(|_| step().is_ok()));
            assert_eq!(step(), Err(Exhausted::Steps));
            assert_eq!(output(5), Ok(()));
            assert_eq!(output(1), Err(Exhausted::Output));
        }
        assert!((0..10_000).all(|_| step().is_ok()));
        assert_eq!(output(1 << 30), Ok(()));
    }

    #[test]
    fn the_interrupt_flag_is_polled() {
        let flag = Arc::new(AtomicBool::new(true));
        let _run = begin(&Limits { interrupt: Some(flag), ..Limits::default() }).unwrap();
        assert_eq!((0..POLL).map(|_| step()).find(Result::is_err), Some(Err(Exhausted::Interrupted)));
    }
}
