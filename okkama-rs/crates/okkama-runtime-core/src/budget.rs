//! Execution budgets of a run on the IR engine (#71): the limits and reports
//! of the Sparrow core (`--max-steps`, `--max-memory`, `--max-output`,
//! `--timeout`). Steps are function entries and backward branches; memory is
//! what the process allocator holds beyond the start of the run (the
//! embedder's probe); output is bytes written to stdout and stderr; the
//! interrupt flag is set by the host (the CLI's timer). Running out stops the
//! run with an error no handler catches.
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Budgets of one run; `None` is unbounded.
#[derive(Debug, Clone, Default)]
pub struct Limits {
    pub steps: Option<u64>,
    pub memory: Option<u64>,
    pub output: Option<u64>,
    pub interrupt: Option<Arc<AtomicBool>>,
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
/// A memory budget without a probe is an error.
pub fn begin(limits: &Limits) -> Result<Guard, String> {
    let memory = match limits.memory {
        Some(limit) => {
            let probe = *PROBE.get().ok_or("a memory budget needs the embedder's memory probe")?;
            Some((limit, probe(), probe))
        }
        None => None,
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
