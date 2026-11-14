//! Execution budgets on Sparrow and the legacy JVM backend (#71, #237):
//! the same options, reports and exit status. Every run carries a
//! timeout, so a budget that failed to stop a program cannot hang the test.

use std::process::{Command, Output};

fn run(engine: &str, source: &str, budget: &[&str]) -> Output {
    // One file per run: tests run in parallel, and two of them may run the
    // same engine with the same budget.
    static RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "budgets-{}-{run}-{engine}-{}.ok",
        std::process::id(),
        budget.join("_").replace(['-', '/'], "")
    ));
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
fn every_budget_stops_the_ir_engine_and_the_jvm_as_it_stops_the_core() {
    let cases: [(&str, &[&str], &str); 5] = [
        (SPIN, &["--max-steps", "100000", "--timeout", "30"], "Execution budget exhausted: steps"),
        (RECURSE, &["--max-steps", "1000", "--timeout", "30"], "Execution budget exhausted: steps"),
        (PRINT, &["--max-output", "1K", "--timeout", "30"], "Execution budget exhausted: output"),
        (GROW, &["--max-memory", "64M", "--max-steps", "100000000", "--timeout", "60"], "Execution budget exhausted: memory"),
        (SPIN, &["--timeout", "1"], "Execution interrupted"),
    ];
    for (source, budget, report) in cases {
        for engine in ["sparrow", "jvm"] {
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

const DEEP: &str = "module app v1.0.0;\nobject Calc {\n    Calc();\n    down(n is int) is int { if n == 0 { 0; } else { self.down(n - 1) + 1; } }\n}\nmain() is int { if Calc().down(333) == 333 { 0; } else { 1; } }\n";
const CAUGHT: &str = "module app v1.0.0;\nimport \"io/stdout\" take Stdout;\nobject Spin {\n    Spin();\n    run() is int { loop { } 0; }\n}\nmain() is int {\n    let out = Stdout();\n    defer out.writeln(\"defer ran\");\n    let result = try { Spin().run(); } catch e is Error { out.writeln(\"caught\"); 7; };\n    result;\n}\n";

/// Steps are counted alike on every engine: a function entry and a branch
/// back to the same or an earlier block. DEEP takes exactly 336 steps (main,
/// the constructor, 334 calls of `down`); running out is not an error the
/// program can catch, and its cleanup does not run.
#[test]
fn steps_are_counted_alike_and_running_out_cannot_be_caught() {
    for engine in ["sparrow", "jvm"] {
        let enough = run(engine, DEEP, &["--max-steps", "336", "--timeout", "60"]);
        assert_eq!(enough.status.code(), Some(0), "{engine}: {}", String::from_utf8_lossy(&enough.stderr));
        let short = run(engine, DEEP, &["--max-steps", "335", "--timeout", "60"]);
        assert_eq!(String::from_utf8_lossy(&short.stderr).trim_end(), "Execution budget exhausted: steps", "{engine}");
        let caught = run(engine, CAUGHT, &["--max-steps", "5000", "--timeout", "60"]);
        assert_eq!(caught.status.code(), Some(1), "{engine}");
        assert_eq!(String::from_utf8_lossy(&caught.stderr).trim_end(), "Execution budget exhausted: steps", "{engine}");
        assert!(caught.stdout.is_empty(), "{engine}: {}", String::from_utf8_lossy(&caught.stdout));
    }
}

/// `build --target jvm --budgeted` counts steps; the limit comes from
/// `-Dokkama.steps` when the JAR runs. A plain build pays nothing and has no
/// step limit; the memory limit is the runtime's and works on either.
#[test]
fn budgeted_jvm_builds_take_the_limit_from_the_command_line() {
    let dir = std::env::temp_dir().join(format!("budgets-build-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("deep.ok");
    std::fs::write(&source, DEEP).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for (flag, jar) in [(Some("--budgeted"), "budgeted.jar"), (None, "plain.jar")] {
        let built = Command::new(env!("CARGO_BIN_EXE_okkama"))
            .args(["build", "--target", "jvm"])
            .args(flag)
            .arg(&source)
            .arg("-o")
            .arg(dir.join(jar))
            .env("OKKAMA_STDLIB_DIR", root.join("std"))
            .output()
            .unwrap();
        assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    }
    let java = |properties: &[&str], jar: &str| {
        Command::new("java").args(properties).arg("-jar").arg(dir.join(jar)).output().unwrap()
    };
    assert_eq!(java(&["-Dokkama.steps=336"], "budgeted.jar").status.code(), Some(0));
    let short = java(&["-Dokkama.steps=335"], "budgeted.jar");
    assert_eq!(short.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&short.stderr).trim_end(), "Execution budget exhausted: steps");
    assert_eq!(java(&[], "budgeted.jar").status.code(), Some(0));
    assert_eq!(java(&["-Dokkama.steps=10"], "plain.jar").status.code(), Some(0));
    let sparrow = Command::new(env!("CARGO_BIN_EXE_okkama"))
        .args(["build", "--target", "sparrow", "--budgeted"])
        .arg(&source)
        .arg("-o")
        .arg(dir.join("x.spb"))
        .output()
        .unwrap();
    assert_eq!(sparrow.status.code(), Some(2));
    let _ = std::fs::remove_dir_all(dir);
}

const DEEP_FAILURE: &str = "module app v1.0.0;\nobject Down {\n    Down();\n    go(n is int, zero is int) is int { if n == 0 { 1 % zero; } else { self.go(n - 1, zero) + 1; } }\n}\nmain() is int { Down().go(300000, 0); }\n";
const DEEP_FOREVER: &str = "module app v1.0.0;\nobject Down {\n    Down();\n    go(n is int) is int { if n < 0 { 0; } else { self.go(n + 1) + 1; } }\n}\nmain() is int { Down().go(0); }\n";

/// Deep non-tail recursion (#241): the memory budget stops it with the same
/// report on every engine (on the core the budget caps the machine stack),
/// and a failure 300000 frames deep prints its collapsed trace at once
/// instead of rendering a line per frame.
#[test]
fn deep_recursion_is_bounded_and_its_trace_is_cheap() {
    for engine in ["sparrow", "jvm"] {
        let started = std::time::Instant::now();
        let output = run(engine, DEEP_FOREVER, &["--max-memory", "64M", "--timeout", "60"]);
        assert_eq!(String::from_utf8_lossy(&output.stderr).trim_end(), "Execution budget exhausted: memory", "{engine}");
        assert!(started.elapsed().as_secs() < 30, "{engine}: {:?}", started.elapsed());
    }
    {
        let engine = "sparrow";
        let path = std::env::temp_dir().join(format!("budgets-trace-{}-{engine}.ok", std::process::id()));
        std::fs::write(&path, DEEP_FAILURE).unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let started = std::time::Instant::now();
        let output = Command::new(env!("CARGO_BIN_EXE_okkama"))
            .args(["run", "--engine", engine, "--timeout", "60"])
            .arg(&path)
            .env("OKKAMA_STDLIB_DIR", root.join("std"))
            .env_remove("OKKAMA_TRACE")
            .output()
            .unwrap();
        let _ = std::fs::remove_file(path);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{engine}: {stderr}");
        assert!(stderr.contains("(\u{00d7}300000)"), "{engine}: {stderr}");
        assert!(stderr.lines().count() < 60, "{engine}: {stderr}");
        assert!(started.elapsed().as_secs() < 20, "{engine}: {:?}", started.elapsed());
    }
}

// Ten million integers: integer comparisons go through big values, so the
// sort runs for seconds on any machine and a one-second timeout lands in it.
const SORT: &str = "module app v1.0.0;\nimport \"io/stdout\" take Stdout;\nimport \"compare\" take Comparing;\nmain() is int {\n    let values = (0..10000000).map() -> i { (i * 7919 + 13) % 1000003; };\n    let sorted = try { values.ordering(); } catch e is Error { Stdout().writeln(\"caught\"); values; };\n    Stdout().writeln(sorted[0].string());\n    0;\n}\n";

/// A long sort of scalars or text stops for the timeout (#238): the
/// comparisons look at the interrupt flag, and the program's `catch` does
/// not see the stop.
#[test]
fn a_long_sort_stops_for_the_timeout() {
    {
        let engine = "sparrow";
        let started = std::time::Instant::now();
        let output = run(engine, SORT, &["--timeout", "1"]);
        assert_eq!(String::from_utf8_lossy(&output.stderr).trim_end(), "Execution interrupted", "{engine}");
        assert!(output.stdout.is_empty(), "{engine}: {}", String::from_utf8_lossy(&output.stdout));
        assert!(started.elapsed().as_secs() < 10, "{engine}: {:?}", started.elapsed());
    }
}

// 2^32 - 1 PBKDF2 rounds take minutes; Argon2 over 64 MiB for 1000 passes
// takes tens of seconds and cannot look at the flag.
const PBKDF2: &str = "module app v1.0.0;\nimport \"io/stdout\" take Stdout;\nimport \"crypto/kdf\" take Pbkdf2Secret, KdfHash;\nimport \"crypto/secret\" take SecretBytes;\nmain() is int {\n    let key = try { Pbkdf2Secret(KdfHash.sha256, SecretBytes([1.byte()]), [2.byte()], 4294967295, 32).secret().size(); } catch e is Error { Stdout().writeln(\"caught\"); 0; };\n    Stdout().writeln(key.string());\n    0;\n}\n";
const ARGON2: &str = "module app v1.0.0;\nimport \"io/stdout\" take Stdout;\nimport \"crypto/password\" take PasswordHash, Argon2Policy;\nimport \"crypto/nonce\" take SystemEntropy;\nimport \"crypto/secret\" take SecretBytes;\nmain() is int {\n    Stdout().writeln(PasswordHash(SecretBytes([1.byte()]), Argon2Policy(65536, 1000, 1), SystemEntropy()).string());\n    0;\n}\n";

/// A long host operation stops for the timeout (#238): PBKDF2 looks at the
/// interrupt flag between rounds, and the program's `catch` does not see
/// the stop; Argon2 cannot, and the CLI ends the run a second later.
#[test]
fn long_host_operations_stop_for_the_timeout() {
    for (name, source) in [("pbkdf2", PBKDF2), ("argon2", ARGON2)] {
        {
            let engine = "sparrow";
            let started = std::time::Instant::now();
            let output = run(engine, source, &["--timeout", "1"]);
            assert_eq!(String::from_utf8_lossy(&output.stderr).trim_end(), "Execution interrupted", "{name} {engine}");
            assert!(output.stdout.is_empty(), "{name} {engine}: {}", String::from_utf8_lossy(&output.stdout));
            assert!(started.elapsed().as_secs() < 10, "{name} {engine}: {:?}", started.elapsed());
        }
    }
}

/// Finished tasks are not kept (#503): each step starts a task in its own
/// FutureGroup, awaits it and drops the group. The core kept every spawned
/// task as a root and every group's tasks as roots, so memory grew with the
/// steps until the budget stopped it; now it stays flat on every engine.
const WORKERS: &str = "module app v1.0.0;
import \"async/scope\" take FutureGroup;
import \"async/event\" take Event;
import \"async/atomic\" take Atomic;
import \"io/stdout\" take Stdout;
object Worker {
    has group is FutureGroup<bool>;
    has done is Event;
    Worker() { group = FutureGroup(); done = Event(); }
    start() is void { group.start(self.run()); }
    async run() is bool { defer done.set(); true; }
    async settled() is bool { await group.wait(); true; }
}
async main() is int {
    let count = Atomic(0);
    loop {
        last if count.value() == 500000;
        let worker = Worker();
        worker.start();
        await worker.settled();
        count.replace(count.value() + 1);
    }
    Stdout().writeln(count.value().string());
    0;
}
";

#[test]
fn finished_tasks_and_their_groups_are_collected() {
    let output = run("sparrow", WORKERS, &["--max-memory", "64M", "--timeout", "120"]);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim_end(), "500000", "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.status.code(), Some(0));
}

