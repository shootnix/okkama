//! Memory budgets of the runtime (#269): two Sparrow runtimes in one process,
//! each with a budget of its own heap. The one that grows past its budget
//! stops; the other, meanwhile, runs to its end within a smaller one.

use sparrow_core::budget::{Limits, MemoryScope};
use std::path::Path;
use std::process::Command;

/// Bytecode of `tests/budgets/<name>.ok`, built by the okkama binary.
fn build(name: &str) -> Vec<u8> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/budgets/{name}.ok"));
    let output = std::env::temp_dir().join(format!("okkama-budgets-{}-{name}.spb", std::process::id()));
    let status = Command::new(env!("CARGO_BIN_EXE_okkama"))
        .args(["build", "--target", "sparrow", "-o"])
        .arg(&output)
        .arg(&source)
        .status()
        .unwrap();
    assert!(status.success(), "build {name}");
    let bytes = std::fs::read(&output).unwrap();
    let _ = std::fs::remove_file(&output);
    bytes
}

fn budget(bytes: u64) -> Limits {
    Limits { memory: Some(bytes), memory_scope: MemoryScope::Runtime, ..Limits::default() }
}

#[test]
fn runtimes_in_one_process_keep_their_own_memory_budgets() {
    let (grow, steady) = (build("grow"), build("steady"));
    // No memory probe is installed: a budget of the runtime reads its heap.
    let large = std::thread::spawn(move || sparrow_core::run_limited(grow, Vec::new(), false, Vec::new(), budget(64 << 20)));
    let small = std::thread::spawn(move || {
        (0..3)
            .map(|_| sparrow_core::run_limited(steady.clone(), Vec::new(), false, Vec::new(), budget(16 << 20)))
            .collect::<Vec<_>>()
    });
    let error = large.join().unwrap().unwrap_err();
    assert!(error.0.contains("Execution budget exhausted: memory"), "{}", error.0);
    for result in small.join().unwrap() {
        assert_eq!(result.map_err(|error| error.0), Ok(0));
    }
}
