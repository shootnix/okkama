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
//!   multiplication. [`MemoryScope::Runtime`] measures this runtime's heap
//!   instead (#269): its pages and large objects, and the Rust memory its
//!   objects own, so that runtimes in one process keep budgets of their own.
//! - Output: bytes written to stdout and stderr by the program.
//! - Interruption: a flag the host sets from any thread ([`Limits::interrupt`]);
//!   polls and the scheduler's waits see it.
//!
//! Exhaustion stops the run with its own status (`EXHAUSTED`): handlers do not
//! catch it, destroys do not run, and the runtime is discarded with its heap.
#![allow(unsafe_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub use okkama_runtime_core::budget::{Counted, MemoryScope, counted_bytes, thread_counted_bytes};

/// Steps between two polls when no step budget is nearer.
pub const SLICE: u64 = 1 << 16;

/// Budgets of one run; `None` is unbounded.
#[derive(Debug, Clone, Default)]
pub struct Limits {
    /// Function entries and loop back-edges.
    pub steps: Option<u64>,
    /// Allocator bytes beyond the start of the run, or the runtime's own
    /// heap ([`MemoryScope`]); also caps the machine stack, so deep recursion
    /// fails as a stack overflow within it.
    pub memory: Option<u64>,
    /// What `memory` measures.
    pub memory_scope: MemoryScope,
    /// Bytes written to stdout and stderr.
    pub output: Option<u64>,
    /// Set by the host to stop the run.
    pub interrupt: Option<Arc<AtomicBool>>,
    /// Native groups the program may use; `None` allows every native (the
    /// CLIs). Natives outside every group (pure computation, stdout and
    /// stderr) are always allowed. Process workers need `Process`.
    pub natives: Option<Vec<NativeGroup>>,
}

/// Natives that reach the host, by what they reach (H-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeGroup {
    /// Files, directories, file snapshots, SQLite databases.
    Fs,
    /// Sockets, TLS, PostgreSQL.
    Net,
    /// Child processes (and process workers).
    Process,
    /// Environment variables, users and groups, host facts, the local zone.
    Env,
    /// Wall and monotonic clocks, sleeping.
    Clock,
    /// Secure random bytes.
    Random,
    /// The controlling terminal and standard input.
    Terminal,
}

/// What a native reaches: the host through one group, nothing beyond the
/// program (computation, stdout and stderr), or unknown: a module this table
/// does not name, refused while the groups are restricted (#361).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    Group(NativeGroup),
    Pure,
    Unclassified,
}

/// Modules whose natives compute without reaching the host. A module added
/// to the runtime is in no list until someone puts it in one, and the test
/// `every_native_is_classified` fails until then.
const PURE: &[&str] = &[
    "runtime/bigint", "runtime/binary", "runtime/buffer", "runtime/bytes", "runtime/collation",
    "runtime/collections", "runtime/crypto", "runtime/csv_state", "runtime/db", "runtime/debug",
    "runtime/decimal", "runtime/diff", "runtime/format", "runtime/hex", "runtime/host_crypto",
    "runtime/hpack", "runtime/html", "runtime/idna", "runtime/io_decorators", "runtime/json",
    "runtime/layout", "runtime/math_aggregate", "runtime/number_text", "runtime/pem",
    "runtime/public_suffix", "runtime/quoted_printable", "runtime/random_distribution",
    "runtime/regex", "runtime/secret", "runtime/single_byte", "runtime/statistics",
    "runtime/template", "runtime/testing", "runtime/time_format", "runtime/time_zone",
    "runtime/toml", "runtime/unicode_character", "runtime/unicode_normalization",
    "runtime/url_text", "runtime/utf", "runtime/uuid", "runtime/stdio", "runtime/base32",
    "runtime/base64", "runtime/base85", "runtime/x509",
];

impl NativeGroup {
    /// The group of `native`, if it reaches the host.
    pub fn of(native: okkama_runtime_abi::Native) -> Option<Self> {
        match Reach::of(native) {
            Reach::Group(group) => Some(group),
            Reach::Pure | Reach::Unclassified => None,
        }
    }
}

impl Reach {
    pub fn of(native: okkama_runtime_abi::Native) -> Self {
        static REACH: std::sync::OnceLock<Vec<Reach>> = std::sync::OnceLock::new();
        let reach = REACH.get_or_init(|| {
            let size = okkama_runtime_abi::Native::ALL.iter().map(|&n| n as usize + 1).max().unwrap_or(0);
            let mut reach = vec![Reach::Unclassified; size];
            for &native in okkama_runtime_abi::Native::ALL {
                reach[native as usize] = Self::classify(native);
            }
            reach
        });
        reach.get(native as usize).copied().unwrap_or(Reach::Unclassified)
    }

    fn classify(native: okkama_runtime_abi::Native) -> Self {
        let symbol = native.symbol();
        let module = symbol.split('@').next().unwrap_or(symbol);
        let name = symbol.split("::").nth(1).unwrap_or("");
        Self::Group(match module {
            "runtime/fs" | "runtime/fs_root" | "runtime/snapshot" | "runtime/database" => NativeGroup::Fs,
            "runtime/network" | "runtime/net" | "runtime/tls_transport" | "runtime/postgres" => NativeGroup::Net,
            "runtime/process" => NativeGroup::Process,
            "runtime/system" if name.starts_with("secure_random") => NativeGroup::Random,
            "runtime/environment" | "runtime/account" | "runtime/system" => NativeGroup::Env,
            "runtime/time_zone" if name.starts_with("timezone_local") => NativeGroup::Env,
            // The host's language preferences and trust anchors (#361).
            "runtime/locale" => NativeGroup::Env,
            "runtime/x509" if name.starts_with("system_certificates") => NativeGroup::Env,
            "runtime/time" => NativeGroup::Clock,
            "runtime/random" => NativeGroup::Random,
            "runtime/application" | "runtime/input_state" => NativeGroup::Terminal,
            "runtime/stdio" if name.starts_with("stdin") => NativeGroup::Terminal,
            _ if PURE.contains(&module) => return Self::Pure,
            _ => return Self::Unclassified,
        })
    }
}

impl Limits {
    /// Is `native` allowed? The refusal names its group.
    #[inline]
    pub fn permits(&self, native: okkama_runtime_abi::Native) -> Result<(), String> {
        let Some(allowed) = &self.natives else {
            return Ok(());
        };
        match Reach::of(native) {
            Reach::Group(group) if !allowed.contains(&group) => {
                Err(format!("Native {} needs the {group:?} permission", native.symbol()))
            }
            Reach::Unclassified => Err(format!(
                "Native {} is in no native group; it is refused while the native groups are restricted",
                native.symbol()
            )),
            Reach::Group(_) | Reach::Pure => Ok(()),
        }
    }
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
/// mimalloc (the CLIs, feature `mimalloc`); it needs mimalloc linked.
#[cfg(feature = "mimalloc")]
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
/// a system call, so it happens once per `PROBE_BYTES` of heap growth. A
/// budget of the runtime (`meter` None) reads the heap's own count instead.
#[derive(Debug)]
pub struct Watch {
    pub meter: Option<Meter>,
    pub limit: u64,
    pub fuel: *mut i64,
    unprobed: usize,
}

/// Heap growth between two probes of a watched run.
const PROBE_BYTES: usize = 256 << 10;

impl Watch {
    pub fn new(meter: Option<Meter>, limit: u64, fuel: &Fuel) -> Self {
        Self { meter, limit, fuel: fuel.0, unprobed: 0 }
    }

    /// `bytes` more are being allocated; `held` is what the heap holds.
    pub fn grow(&mut self, bytes: usize, held: usize) {
        self.unprobed += bytes;
        if self.unprobed < PROBE_BYTES {
            return;
        }
        self.unprobed = 0;
        let used = self.meter.map_or(held as isize, |meter| meter.used());
        if used > self.limit as isize {
            // SAFETY: the runtime owns both the heap and its fuel cell.
            unsafe { self.fuel.write_volatile(0) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use okkama_runtime_abi::Native;

    /// Every native is in a group or among the pure ones (#361): a module
    /// added without a place in the table fails here instead of passing a
    /// host that restricted the groups.
    #[test]
    fn every_native_is_classified() {
        let unclassified: Vec<&str> = Native::ALL
            .iter()
            .filter(|&&native| Reach::of(native) == Reach::Unclassified)
            .map(|native| native.symbol())
            .collect();
        assert!(unclassified.is_empty(), "natives in no group and not pure: {unclassified:?}");
    }

    #[test]
    fn the_preferred_languages_need_env() {
        let native = *Native::ALL.iter().find(|native| native.symbol().starts_with("runtime/locale@")).unwrap();
        assert_eq!(Reach::of(native), Reach::Group(NativeGroup::Env));
        let limits = Limits { natives: Some(vec![NativeGroup::Clock]), ..Limits::default() };
        assert!(limits.permits(native).unwrap_err().contains("needs the Env permission"));
        let everything = Limits::default();
        assert!(everything.permits(native).is_ok());
    }
}
