//! Sparrow execution core (rebuild, see `docs/new-runtime/core-rebuild.md`).
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
//! - no preemption inside synchronous code: loop back-edges poll an interrupt
//!   flag so cancellation and timeouts stop runaway loops with a typed failure.
//!
//! Stages: 1 heap and values, 2 synchronous execution, 3 async, 4 natives and
//! CLI (`sparrow-next` switches here), 5 performance and std/db, 6 removal of
//! `sparrow-engine`.

// Runtime helpers are `extern "C"` entry points of machine code: they take
// the context as a raw pointer and are sound for the code that calls them.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

pub mod drivers;
pub mod failures;
pub mod heap;
pub(crate) mod host;
pub mod io;
pub mod intrinsics;
pub mod jit;
pub mod natives;
pub mod ops;
pub mod primitives;
pub mod program;
pub mod runtime;
mod scalar;
pub mod tasks;
pub mod word;

/// Machine stack of the VM thread (R2): deep recursion is bounded by this
/// reservation; physical memory is committed only as frames are used.
pub const STACK_BYTES: u64 = 1 << 30;

/// Load and run an SPB program's entry on a VM thread.
pub fn run(bytes: Vec<u8>) -> Result<i64, Error> {
    run_with(bytes, Vec::new())
}

/// `run` with the program's command-line arguments.
pub fn run_with(bytes: Vec<u8>, arguments: Vec<Vec<u8>>) -> Result<i64, Error> {
    on_vm_thread(move || {
        let program = okkama_sparrow::decode(&bytes).map_err(|e| Error(e.to_string()))?;
        let mut runtime = Box::new(runtime::Runtime::new(program::Loaded::new(program))?);
        if let Some(host) = &mut runtime.host {
            host.arguments = arguments;
        }
        let result = runtime.run_entry(STACK_BYTES);
        if let Some(report) = runtime.stats_report() {
            eprint!("{report}");
        }
        result
    })
}

/// Run the module's test profile and write the standard textual report;
/// true when every test passed. Each test runs on a fresh runtime.
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
            let failures = if !declaration.params.is_empty() || declaration.result.is_some() {
                vec!["invalid test entry signature".to_owned()]
            } else {
                let mut runtime = Box::new(runtime::Runtime::new(program::Loaded::new(program.clone()))?);
                if let Some(host) = &mut runtime.host {
                    host.test_failures = Some(Vec::new());
                }
                let outcome = runtime.run_function(function.0 as usize, STACK_BYTES);
                let mut failures = runtime.host.as_mut().and_then(|h| h.test_failures.take()).unwrap_or_default();
                if let Err(error) = outcome {
                    failures.push(error.0);
                }
                failures
            };
            if failures.is_empty() {
                writeln!(out, "--- PASS: {name}").map_err(io)?;
            } else {
                passed = false;
                writeln!(out, "--- FAIL: {name}").map_err(io)?;
                for failure in failures {
                    writeln!(out, "    {}", failure.replace('\n', "\n    ")).map_err(io)?;
                }
            }
            out.flush().map_err(io)?;
        }
        writeln!(out, "{}", if passed { "PASS" } else { "FAIL" }).map_err(io)?;
        out.flush().map_err(io)?;
        Ok(passed)
    })
}

fn on_vm_thread<T: Send + 'static>(body: impl FnOnce() -> Result<T, Error> + Send + 'static) -> Result<T, Error> {
    std::thread::Builder::new()
        .name("sparrow-core".into())
        .stack_size(STACK_BYTES as usize)
        .spawn(body)
        .map_err(|e| Error(format!("VM thread: {e}")))?
        .join()
        .map_err(|_| Error("VM thread panicked".into()))?
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
