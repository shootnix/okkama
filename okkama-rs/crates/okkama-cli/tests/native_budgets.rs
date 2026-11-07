//! A memory budget stops one operation whose result would cross it before
//! the result is built (#72): a join of 200 copies of a 1 MiB string under a
//! 64 MiB budget stops without allocating 200 MiB.

use std::process::Command;

const JOIN: &str = "module app v1.0.0;\nobject Text {\n    Text();\n    doubled(text is string, times is int) is string { if times == 0 { text; } else { self.doubled(text + text, times - 1); } }\n}\nmain() is int {\n    let mib = Text().doubled(\"x\", 20);\n    let copies = (0..200).map() -> i { mib; };\n    copies.join(\"\").length() % 256;\n}\n";

#[test]
fn a_join_past_the_memory_budget_is_stopped_before_it_is_built() {
    let path = std::env::temp_dir().join(format!("native-budgets-{}.ok", std::process::id()));
    std::fs::write(&path, JOIN).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut command = if cfg!(target_os = "macos") {
        let mut timed = Command::new("/usr/bin/time");
        timed.arg("-l").arg(env!("CARGO_BIN_EXE_okkama"));
        timed
    } else {
        Command::new(env!("CARGO_BIN_EXE_okkama"))
    };
    let output = command
        .args(["run", "--engine", "sparrow", "--max-memory", "64M", "--timeout", "60"])
        .arg(&path)
        .env("OKKAMA_STDLIB_DIR", root.join("std"))
        .env("OKKAMA_TRACE", "off")
        .output()
        .unwrap();
    let _ = std::fs::remove_file(path);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.starts_with("Execution budget exhausted: memory"), "{stderr}");
    if cfg!(target_os = "macos") {
        let peak: u64 = stderr
            .lines()
            .find_map(|line| line.trim().strip_suffix("maximum resident set size").and_then(|n| n.trim().parse().ok()))
            .expect("peak memory");
        assert!(peak < 150 << 20, "peak {} MiB", peak >> 20);
    }
}
