//! Stack safety of the Sparrow core (H-02, H-12, #25): recursion that goes
//! through Rust (`destroy`, closures) ends as a `Stack overflow` failure, the
//! process stays usable, and helper calls do not grow the machine frame.

use sparrow_core::budget::{Counted, Limits, Meter, counted_bytes};
use std::path::{Path, PathBuf};
use std::process::Command;

#[global_allocator]
static ALLOCATOR: Counted<std::alloc::System> = Counted(std::alloc::System);

fn program(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/nested/{name}.ok"))
}

/// Bytecode of `name`, built by the okkama binary.
fn build(name: &str) -> Vec<u8> {
    let output = std::env::temp_dir().join(format!("okkama-nested-{}-{name}.spb", std::process::id()));
    let status = Command::new(env!("CARGO_BIN_EXE_okkama"))
        .args(["build", "--target", "sparrow", "-o"])
        .arg(&output)
        .arg(program(name))
        .status()
        .unwrap();
    assert!(status.success(), "build {name}");
    let bytes = std::fs::read(&output).unwrap();
    let _ = std::fs::remove_file(&output);
    bytes
}

#[test]
fn recursion_through_rust_ends_in_a_failure_and_the_process_goes_on() {
    sparrow_core::budget::set_memory_probe(counted_bytes);
    // A memory budget also bounds the machine stack: 64 MiB overflows fast.
    let limits = || Limits { memory: Some(64 << 20), ..Limits::default() };
    let programs = [build("destroy"), build("closure")];
    let meter = Meter::start(counted_bytes);
    for _ in 0..2 {
        for bytes in &programs {
            let error = sparrow_core::run_limited(bytes.clone(), Vec::new(), false, Vec::new(), limits()).unwrap_err();
            assert!(error.0.contains("Stack overflow"), "{}", error.0);
        }
        assert_eq!(sparrow_core::run_limited(build("calls"), Vec::new(), false, Vec::new(), limits()).unwrap(), 0);
        assert!(meter.used() < 4 << 20, "held after the runs: {} bytes", meter.used());
    }
}

#[test]
fn helper_calls_share_one_argument_area() {
    let output = Command::new(env!("CARGO_BIN_EXE_okkama"))
        .args(["run", "--engine", "sparrow"])
        .arg(program("calls"))
        .env("SPARROW_CORE_STATS", "1")
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout), "20\n");
    let stats = String::from_utf8_lossy(&output.stderr);
    let frame: u64 = stats
        .lines()
        .find_map(|line| line.trim().strip_suffix("largest machine frame, bytes"))
        .and_then(|count| count.trim().parse().ok())
        .unwrap_or_else(|| panic!("{stats}"));
    // Twelve `slice` calls once took a slot each.
    assert!(frame <= 256, "largest machine frame: {frame} bytes");
}
