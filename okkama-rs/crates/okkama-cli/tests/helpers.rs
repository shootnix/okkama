//! Every runtime helper of the Sparrow core runs in a test (H-06, #23): two
//! programs reach all of them, and `--stats` counts each helper call.

use std::collections::BTreeSet;
use std::process::Command;

/// Helper names counted while running `name`, with its stdout and status.
fn helpers_of(name: &str) -> (BTreeSet<String>, String, Option<i32>) {
    let program = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/helpers/{name}.ok"));
    // A step budget makes machine code poll, which reaches the poll helper.
    let output = Command::new(env!("CARGO_BIN_EXE_okkama"))
        .args(["run", "--engine", "sparrow", "--max-steps", "1000000000000"])
        .arg(&program)
        .env("SPARROW_CORE_STATS", "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let seen = stderr
        .lines()
        .filter_map(|line| line.trim().split_once(" helper "))
        .map(|(_, helper)| helper.to_string())
        .collect();
    (seen, String::from_utf8_lossy(&output.stdout).into_owned(), output.status.code())
}

#[test]
fn every_runtime_helper_runs() {
    let (mut seen, stdout, status) = helpers_of("main");
    // Both programs end with a runtime error on purpose (trap, float).
    assert_eq!(status, Some(1));
    assert!(stdout.ends_with("20000\n99999\n113\n"), "{stdout}");
    let (float, _, status) = helpers_of("float");
    assert_eq!(status, Some(1));
    seen.extend(float);
    let missing: Vec<&str> = sparrow_core::jit::HELPERS
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !seen.contains(*name))
        .collect();
    assert!(missing.is_empty(), "helpers no test runs: {missing:?}");
}
