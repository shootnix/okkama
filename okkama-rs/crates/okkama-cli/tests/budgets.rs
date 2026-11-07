//! Execution budgets on the IR engine (#71): the same options, reports and
//! exit status as the Sparrow core. Every run carries a timeout, so a budget
//! that failed to stop a program cannot hang the test.

use std::process::{Command, Output};

fn run(engine: &str, source: &str, budget: &[&str]) -> Output {
    let path = std::env::temp_dir().join(format!("budgets-{}-{engine}-{}.ok", std::process::id(), budget.join("_").replace(['-', '/'], "")));
    std::fs::write(&path, source).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let output = Command::new(env!("CARGO_BIN_EXE_okkama"))
        .args(["run", "--engine", engine])
        .args(budget)
        .arg(&path)
        .env("OKKAMA_STDLIB_DIR", root.join("std"))
        .env("OKKAMA_TRACE", "off")
        .output()
        .unwrap();
    let _ = std::fs::remove_file(path);
    output
}

const SPIN: &str = "module app v1.0.0;\nmain() is int {\n    loop { }\n    0;\n}\n";
const RECURSE: &str = "module app v1.0.0;\nobject Down {\n    Down();\n    go(n is int) is int { if n < 0 { 0; } else { self.go(n + 1); } }\n}\nmain() is int { Down().go(0); }\n";
const PRINT: &str = "module app v1.0.0;\nimport \"io/stdout\" take Stdout;\nmain() is int {\n    let out = Stdout();\n    loop { out.writeln(\"0123456789\"); }\n    0;\n}\n";
const GROW: &str = "module app v1.0.0;\nobject Keep {\n    Keep();\n    grow(items is @@int, n is int) is int { if n < 0 { 0; } else { self.grow(items + [(0..1000).map() -> i { i; }], n + 1); } }\n}\nmain() is int { Keep().grow([], 0); }\n";

#[test]
fn every_budget_stops_the_ir_engine_as_it_stops_the_core() {
    let cases: [(&str, &[&str], &str); 5] = [
        (SPIN, &["--max-steps", "100000", "--timeout", "30"], "Execution budget exhausted: steps"),
        (RECURSE, &["--max-steps", "1000", "--timeout", "30"], "Execution budget exhausted: steps"),
        (PRINT, &["--max-output", "1K", "--timeout", "30"], "Execution budget exhausted: output"),
        (GROW, &["--max-memory", "64M", "--max-steps", "100000000", "--timeout", "60"], "Execution budget exhausted: memory"),
        (SPIN, &["--timeout", "1"], "Execution interrupted"),
    ];
    for (source, budget, report) in cases {
        for engine in ["ir", "sparrow"] {
            let output = run(engine, source, budget);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.code(), Some(1), "{engine} {budget:?}: {stderr}");
            assert_eq!(stderr.trim_end(), report, "{engine} {budget:?}");
            if report.ends_with("output") {
                assert!(output.stdout.len() <= 1024, "{engine}: {} bytes", output.stdout.len());
            }
        }
    }
}

/// The JVM: output and timeout through the launcher; steps and memory
/// refused until #237 decides how to count them.
#[test]
fn the_jvm_has_output_and_timeout_budgets() {
    for (budget, report) in [(&["--max-output", "1K", "--timeout", "60"][..], "Execution budget exhausted: output"), (&["--timeout", "2"][..], "Execution interrupted")] {
        let source = if report.ends_with("output") { PRINT } else { SPIN };
        let output = run("jvm", source, budget);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{budget:?}: {stderr}");
        assert_eq!(stderr.trim_end(), report, "{budget:?}");
        assert!(output.stdout.len() <= 1024);
    }
    let output = run("jvm", SPIN, &["--max-steps", "10"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("#237"));
}
