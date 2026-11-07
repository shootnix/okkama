//! Execution budgets (hardening H-04): steps, memory, output and external
//! interruption. Every budget is off by default; machine code polls only when
//! one is set, so an unbounded run compiles to the same code as before.
//!
//! - Steps: one per function entry and per loop back-edge. Machine code
//!   decrements `Ctx::fuel` there and calls `rt_poll` when a slice is used up;
//!   the poll charges the slice, refills, and checks the other budgets.
//! - Memory: bytes the process allocator holds beyond the start of the run,
//!   read by the embedder's [`MemoryProbe`] (the CLIs read mimalloc). It covers
//!   the core heap, whose pages and large objects are allocator blocks, and
//!   Rust-side values (BigInt, builders, maps). Polls check it; machine-code
//!   allocation is refused before it would cross it; large heap allocations
//!   from Rust probe it ([`Watch`]); a BigInt product is checked before the
//!   multiplication.
//! - Output: bytes written to stdout and stderr by the program.
//! - Interruption: a flag the host sets from any thread ([`Limits::interrupt`]);
//!   polls and the scheduler's waits see it.
//!
//! Exhaustion stops the run with its own status (`EXHAUSTED`): handlers do not
//! catch it, destroys do not run, and the runtime is discarded with its heap.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

/// Steps between two polls when no step budget is nearer.
pub const SLICE: u64 = 1 << 16;

/// Budgets of one run; `None` is unbounded.
#[derive(Debug, Clone, Default)]
pub struct Limits {
    /// Function entries and loop back-edges.
    pub steps: Option<u64>,
    /// Allocator bytes beyond the start of the run; also caps the machine
    /// stack, so deep recursion fails as a stack overflow within it.
    pub memory: Option<u64>,
    /// Bytes written to stdout and stderr.
    pub output: Option<u64>,
    /// Set by the host to stop the run.
    pub interrupt: Option<Arc<AtomicBool>>,
}

impl Limits {
    /// Does machine code poll (any budget or interruption set)?
    pub fn polled(&self) -> bool {
        self.steps.is_some() || self.memory.is_some() || self.interrupt.is_some()
    }

    pub fn interrupted(&self) -> bool {
        self.interrupt.as_ref().is_some_and(|flag| flag.load(Ordering::Relaxed))
    }

    /// Apply a command-line budget option: `--max-steps N`, `--max-memory
    /// SIZE`, `--max-output SIZE` (bytes, or with a K/M/G suffix of 1024s) or
    /// `--timeout SECONDS` (wall clock; interrupts the run). Ok(false) when
    /// `option` is not one of them.
    pub fn option(&mut self, option: &str, value: &str) -> Result<bool, String> {
        let invalid = || format!("invalid value `{value}` for {option}");
        match option {
            "--max-steps" => self.steps = Some(value.parse().map_err(|_| invalid())?),
            "--max-memory" => self.memory = Some(size(value).ok_or_else(invalid)?),
            "--max-output" => self.output = Some(size(value).ok_or_else(invalid)?),
            "--timeout" => {
                let seconds: f64 = value.parse().ok().filter(|s: &f64| s.is_finite() && *s >= 0.0).ok_or_else(invalid)?;
                let flag = self.interrupt.get_or_insert_with(Arc::default).clone();
                std::thread::Builder::new()
                    .name("sparrow-timeout".into())
                    .spawn(move || {
                        std::thread::sleep(std::time::Duration::from_secs_f64(seconds));
                        flag.store(true, Ordering::Relaxed);
                    })
                    .map_err(|e| format!("{option}: {e}"))?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// Command-line help of the budget options.
pub const OPTIONS_HELP: &str = "\
    --max-steps N      stop after N steps (function entries and loop iterations)
    --max-memory SIZE  stop when the run holds more than SIZE bytes (K/M/G suffixes)
    --max-output SIZE  stop when stdout and stderr exceed SIZE bytes
    --timeout SECONDS  interrupt the run after SECONDS of wall-clock time
";

/// Bytes with an optional K, M or G suffix (powers of 1024).
fn size(text: &str) -> Option<u64> {
    let (digits, unit) = match text.char_indices().last()? {
        (at, 'K' | 'k') => (&text[..at], 1u64 << 10),
        (at, 'M' | 'm') => (&text[..at], 1 << 20),
        (at, 'G' | 'g') => (&text[..at], 1 << 30),
        _ => (text, 1),
    };
    digits.parse::<u64>().ok()?.checked_mul(unit)
}

/// What ran out; the text of the failure report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exhausted {
    Steps,
    Memory,
    Output,
    Interrupted,
}

impl Exhausted {
    pub fn message(self) -> &'static str {
        match self {
            Exhausted::Steps => "Execution budget exhausted: steps",
            Exhausted::Memory => "Execution budget exhausted: memory",
            Exhausted::Output => "Execution budget exhausted: output",
            Exhausted::Interrupted => "Execution interrupted",
        }
    }
}

/// The bytes the process allocator holds now; set by the embedder (the CLIs
/// read mimalloc's committed memory). A memory budget needs one.
pub type MemoryProbe = fn() -> usize;

static PROBE: std::sync::OnceLock<MemoryProbe> = std::sync::OnceLock::new();

/// Install the memory probe once per process; later calls are ignored.
pub fn set_memory_probe(probe: MemoryProbe) {
    let _ = PROBE.set(probe);
}

pub fn memory_probe() -> Option<MemoryProbe> {
    PROBE.get().copied()
}

/// mimalloc's committed bytes, a probe for processes whose allocator is
/// mimalloc (the CLIs); referencing it needs mimalloc linked.
pub fn mimalloc_committed() -> usize {
    unsafe extern "C" {
        fn mi_process_info(
            elapsed: *mut usize,
            user: *mut usize,
            system: *mut usize,
            rss: *mut usize,
            peak_rss: *mut usize,
            commit: *mut usize,
            peak_commit: *mut usize,
            faults: *mut usize,
        );
    }
    let null = std::ptr::null_mut();
    let mut commit = 0;
    // SAFETY: mimalloc writes only the non-null outputs.
    unsafe { mi_process_info(null, null, null, null, null, &mut commit, null, null) };
    commit
}

/// Net bytes allocated through [`Counted`].
static NET: AtomicIsize = AtomicIsize::new(0);

/// A counting global allocator for embedders without a probe of their own
/// (and tests): [`counted_bytes`] is its probe. It costs one atomic add per
/// call, so the CLIs use their allocator's statistics instead.
#[derive(Debug)]
pub struct Counted<A>(pub A);

// SAFETY: every call forwards to the wrapped allocator unchanged.
unsafe impl<A: GlobalAlloc> GlobalAlloc for Counted<A> {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        NET.fetch_add(layout.size() as isize, Ordering::Relaxed);
        unsafe { self.0.alloc(layout) }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        NET.fetch_add(layout.size() as isize, Ordering::Relaxed);
        unsafe { self.0.alloc_zeroed(layout) }
    }

    #[inline]
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        NET.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        unsafe { self.0.dealloc(pointer, layout) }
    }

    #[inline]
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        NET.fetch_add(size as isize - layout.size() as isize, Ordering::Relaxed);
        unsafe { self.0.realloc(pointer, layout, size) }
    }
}

/// The probe of [`Counted`].
pub fn counted_bytes() -> usize {
    NET.load(Ordering::Relaxed).max(0) as usize
}

/// The fuel cell of a runtime: machine code decrements it at every step (its
/// address is a constant of the code) and polls below zero. It is touched only
/// through its raw pointer, never borrowed.
#[derive(Debug)]
pub struct Fuel(*mut i64);

impl Default for Fuel {
    fn default() -> Self {
        Self(Box::into_raw(Box::new(0)))
    }
}

impl Fuel {
    pub fn address(&self) -> i64 {
        self.0 as i64
    }

    pub fn set(&self, value: i64) {
        // SAFETY: a live allocation owned by `self`.
        unsafe { self.0.write_volatile(value) }
    }
}

impl Drop for Fuel {
    fn drop(&mut self) {
        // SAFETY: allocated by `default`, freed once.
        drop(unsafe { Box::from_raw(self.0) });
    }
}

/// The memory of one run: the probe's reading at its start.
#[derive(Debug, Clone, Copy)]
pub struct Meter {
    probe: MemoryProbe,
    base: usize,
}

impl Meter {
    pub fn start(probe: MemoryProbe) -> Self {
        Self { probe, base: probe() }
    }

    /// Bytes held beyond the start (negative after freeing older ones).
    pub fn used(&self) -> isize {
        (self.probe)() as isize - self.base as isize
    }
}

/// The heap's check of large allocations against a memory budget: past it,
/// the fuel is emptied so the next step polls and stops the run. Probing is
/// a system call, so it happens once per `PROBE_BYTES` of heap growth.
#[derive(Debug)]
pub struct Watch {
    pub meter: Meter,
    pub limit: u64,
    pub fuel: *mut i64,
    unprobed: usize,
}

/// Heap growth between two probes of a watched run.
const PROBE_BYTES: usize = 256 << 10;

impl Watch {
    pub fn new(meter: Meter, limit: u64, fuel: &Fuel) -> Self {
        Self { meter, limit, fuel: fuel.0, unprobed: 0 }
    }

    /// `bytes` more are being allocated.
    pub fn grow(&mut self, bytes: usize) {
        self.unprobed += bytes;
        if self.unprobed < PROBE_BYTES {
            return;
        }
        self.unprobed = 0;
        if self.meter.used() > self.limit as isize {
            // SAFETY: the runtime owns both the heap and its fuel cell.
            unsafe { self.fuel.write_volatile(0) };
        }
    }
}
