//! Sparrow execution core (rebuild journal: okkama-docs `runtime/new-runtime/core-rebuild.md`).
//!
//! Model, validated by `crates/sparrow-proto` (`docs/new-runtime/core-spike.md`):
//! - synchronous functions are compiled whole and call each other directly on
//!   the native stack; roles dispatch through per-descriptor tables;
//! - async functions are resumable state machines whose heap frame is also
//!   their Future; `spawn` starts eagerly, Tokio drives external waits;
//! - registers carry no tags where the SPB type is static; objects are pointers
//!   to `[header, fields…]` with inline allocation;
//! - a precise collector finds roots through Cranelift stack maps, async
//!   frames and Rust-side visitors, and runs only at safepoints, never inside a
//!   native provider; it does not move objects (decision R1);
//! - no preemption inside synchronous code; under execution budgets
//!   (`budget`) function entries and loop back-edges poll a fuel cell, so
//!   step, memory and output budgets and the host's interrupt flag stop
//!   runaway code (async cancellation still cannot stop a synchronous loop).
//!
//! Stages: 1 heap and values, 2 synchronous execution, 3 async, 4 natives and
//! CLI (`sparrow-next` switches here), 5 performance and std/db, 6 removal of
//! `sparrow-engine`.

// Runtime helpers are `extern "C"` entry points of machine code: they take
// the context as a raw pointer and are sound for the code that calls them.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

pub mod budget;
mod code_memory;
mod contracts;
pub mod drivers;
pub mod failures;
pub mod heap;
pub(crate) mod host;
pub mod io;
pub mod intrinsics;
pub mod jit;
pub mod natives;
pub mod maps;
pub mod ops;
pub mod primitives;
pub mod program;
pub mod runtime;
mod scalar;
pub mod tasks;
pub mod word;
pub mod worker;
mod worker_process;

/// Machine stack of the VM thread (R2): deep recursion is bounded by this
/// reservation; physical memory is committed only as frames are used.
pub const STACK_BYTES: u64 = 1 << 30;

/// Load and run an SPB program's entry on a VM thread.
pub fn run(bytes: Vec<u8>) -> Result<i64, Error> {
    run_with(bytes, Vec::new())
}

/// `run` with the program's command-line arguments.
pub fn run_with(bytes: Vec<u8>, arguments: Vec<Vec<u8>>) -> Result<i64, Error> {
    run_configured(bytes, arguments, false, Vec::new())
}

/// Everything loading checks before a program runs: the SPB verifier and
/// the intrinsic contracts of the core (`sparrow check`).
pub fn check(bytes: &[u8]) -> Result<(), Error> {
    let program = okkama_sparrow::decode(bytes).map_err(|e| Error(e.to_string()))?;
    contracts::check(&program::Loaded::new(program))
}

/// `run_with`, optionally printing runtime event counts to stderr at exit
/// (`--stats`; also `SPARROW_CORE_STATS=1`). `worker_command` is the
/// executable and SPB that process workers run (`--worker` goes after the
/// executable); empty, `spawn` of a synchronous action fails.
pub fn run_configured(
    bytes: Vec<u8>,
    arguments: Vec<Vec<u8>>,
    stats: bool,
    worker_command: Vec<std::ffi::OsString>,
) -> Result<i64, Error> {
    run_limited(bytes, arguments, stats, worker_command, budget::Limits::default())
}

/// `run_configured` under execution budgets (`budget::Limits`); a run that
/// exhausts one fails with its report ("Execution budget exhausted: …").
pub fn run_limited(
    bytes: Vec<u8>,
    arguments: Vec<Vec<u8>>,
    stats: bool,
    worker_command: Vec<std::ffi::OsString>,
    limits: budget::Limits,
) -> Result<i64, Error> {
    run_described(bytes, arguments, stats, worker_command, limits)?.map_err(|(error, _)| error)
}

/// `run_limited`, keeping the program's failure apart from a program that
/// does not load: the inner error is the report printed on stderr, with the
/// uncaught failure described (its call stack included).
pub fn run_described(
    bytes: Vec<u8>,
    arguments: Vec<Vec<u8>>,
    stats: bool,
    worker_command: Vec<std::ffi::OsString>,
    limits: budget::Limits,
) -> Result<Result<i64, (Error, Option<Uncaught>)>, Error> {
    on_vm_thread(move || {
        let program = okkama_sparrow::decode(&bytes).map_err(|e| Error(e.to_string()))?;
        let mut runtime = Box::new(runtime::Runtime::new(program::Loaded::new(program))?);
        runtime.worker_command = worker_command;
        runtime.limits = limits;
        if stats && runtime.stats.is_none() {
            runtime.stats = Some(Box::default());
            runtime.heap.allocations = Some(Box::default());
        }
        if let Some(host) = &mut runtime.host {
            host.arguments = arguments;
        }
        let result = runtime.run_entry(STACK_BYTES);
        if let Some(report) = runtime.stats_report() {
            eprint!("{report}");
        }
        Ok(result.map_err(|error| (error, runtime.uncaught.take())))
    })
}

/// An uncaught failure of `main`: its OCS category (`uncaught-error`,
/// `runtime`, `panic`), the error type of a thrown value or `NativeFailure`,
/// its message, and the activations it unwound through, innermost first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uncaught {
    pub category: &'static str,
    pub error_type: Option<String>,
    pub message: String,
    /// Function and instruction numbers; they are shared with OIR, so a
    /// compiler can map them to source (`okkama_runtime::render_trace`).
    pub trace: Vec<Site>,
}

pub use okkama_runtime_core::failure::Site;
pub use okkama_runtime_core::stdout;

/// The call stack of an uncaught failure in Sparrow terms: function, number
/// and text of the instruction of each frame. Empty for an empty stack.
pub fn render_trace(program: &okkama_sparrow::Program, trace: &[Site]) -> String {
    use okkama_runtime_core::failure::{frame_name, stack_text_by};
    if trace.is_empty() {
        return String::new();
    }
    // Строки только показанных кадров: стек глубокой рекурсии — миллионы (#241).
    stack_text_by("Stack trace (Sparrow, most recent call first):", trace, |site| {
        match program.functions.get(site.function as usize) {
            Some(function) => {
                let name = frame_name(&function.symbol);
                match function.code.get(site.pc as usize) {
                    Some(instruction) => format!("{name} [f{} @{}: {}]", site.function, site.pc, instruction.text()),
                    None => format!("{name} [f{} @{}]", site.function, site.pc),
                }
            }
            None => format!("? [f{} @{}]", site.function, site.pc),
        }
    })
}

/// `run_configured`, with the uncaught failure (if any) described for
/// protocol adapters; the error text is the report printed on stderr.
pub fn run_reported(
    bytes: Vec<u8>,
    arguments: Vec<Vec<u8>>,
    worker_command: Vec<std::ffi::OsString>,
) -> Result<Result<i64, (Error, Option<Uncaught>)>, Error> {
    run_described(bytes, arguments, false, worker_command, budget::Limits::default())
}

/// Run the module's test profile and write the standard textual report;
/// true when no test failed. Each test runs on a fresh runtime.
pub fn run_tests(bytes: Vec<u8>, out: Box<dyn std::io::Write + Send>) -> Result<bool, Error> {
    on_vm_thread(move || {
        let mut out = out;
        let program = okkama_sparrow::decode(&bytes).map_err(|e| Error(e.to_string()))?;
        let tests = program.tests.clone().ok_or_else(|| Error("module has no tests".into()))?;
        if tests.is_empty() {
            return Err(Error("Test run requires at least one test".into()));
        }
        let io = |e: std::io::Error| Error(format!("test report: {e}"));
        let mut passed = true;
        for (name, function) in tests {
            writeln!(out, "=== RUN   {name}").map_err(io)?;
            out.flush().map_err(io)?;
            let declaration = &program.functions[function.0 as usize];
            let (failures, skipped) = if !declaration.params.is_empty() || declaration.result.is_some() {
                (vec!["invalid test entry signature".to_owned()], None)
            } else {
                let mut runtime = Box::new(runtime::Runtime::new(program::Loaded::new(program.clone()))?);
                if let Some(host) = &mut runtime.host {
                    host.test_failures = Some(Vec::new());
                }
                let outcome = runtime.run_function(function.0 as usize, STACK_BYTES);
                let host = runtime.host.as_mut();
                let (mut failures, skipped) =
                    host.map(|h| (h.test_failures.take().unwrap_or_default(), h.test_skipped.take())).unwrap_or_default();
                // A skipped test ends with the failure of its `test.skip`.
                if let (Err(error), None) = (outcome, &skipped) {
                    failures.push(error.0);
                }
                (failures, skipped)
            };
            if !failures.is_empty() {
                passed = false;
                writeln!(out, "--- FAIL: {name}").map_err(io)?;
                for failure in failures {
                    writeln!(out, "    {}", failure.replace('\n', "\n    ")).map_err(io)?;
                }
            } else if let Some(reason) = skipped {
                writeln!(out, "--- SKIP: {name}").map_err(io)?;
                if !reason.is_empty() {
                    writeln!(out, "    {}", reason.replace('\n', "\n    ")).map_err(io)?;
                }
            } else {
                writeln!(out, "--- PASS: {name}").map_err(io)?;
            }
            out.flush().map_err(io)?;
        }
        writeln!(out, "{}", if passed { "PASS" } else { "FAIL" }).map_err(io)?;
        out.flush().map_err(io)?;
        Ok(passed)
    })
}

/// The child side of a process worker: answer the framed `request` for the
/// program `bytes`; nested spawns run `worker_command`. For the CLIs' worker
/// processes only, not an embedding entry point: a worker runs without
/// budgets or native permissions.
#[doc(hidden)]
pub fn run_worker(bytes: Vec<u8>, request: Vec<u8>, worker_command: Vec<std::ffi::OsString>) -> Result<Vec<u8>, Error> {
    on_vm_thread(move || {
        let program = okkama_sparrow::decode(&bytes).map_err(|e| Error(e.to_string()))?;
        let mut runtime = Box::new(runtime::Runtime::new(program::Loaded::new(program))?);
        runtime.worker_command = worker_command;
        Ok(worker::respond(&mut runtime, &request, STACK_BYTES))
    })
}

fn on_vm_thread<T: Send + 'static>(body: impl FnOnce() -> Result<T, Error> + Send + 'static) -> Result<T, Error> {
    std::thread::Builder::new()
        .name("sparrow-core".into())
        .stack_size(STACK_BYTES as usize)
        .spawn(move || {
            // The program's cwd and environment live as long as this run (#40).
            let _host = okkama_runtime_core::host::enter(
                okkama_runtime_core::host::Context::process().without("SPARROW_INTERNAL_WORKER"),
            );
            body()
        })
        .map_err(|e| Error(format!("VM thread: {e}")))?
        .join()
        .map_err(|payload| Error(format!("internal error: {}", runtime::panic_text(&*payload))))?
}

/// A variable the launcher adds to the program's environment when the
/// process lacks it (`okkama run` passes the pinned ICU this way).
pub fn default_environment(name: &str, value: &str) {
    okkama_runtime_core::host::default_environment(name, value);
}

/// Natives and whether this core provides them (`--native-capabilities`).
pub fn native_capabilities() -> impl Iterator<Item = (&'static str, bool)> {
    okkama_runtime_abi::Native::ALL.iter().map(|&n| (n.symbol(), natives::supported(n)))
}

/// Engine-level failure: an invalid program or an internal invariant, never a
/// language-level error (those are values unwound by the program).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}
