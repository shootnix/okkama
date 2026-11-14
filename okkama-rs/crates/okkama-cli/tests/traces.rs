//! Call stacks of uncaught failures on the Sparrow core (#10): `okkama run`
//! maps them to source, `okkama exec` and
//! `sparrow` print them in Sparrow terms, `OKKAMA_TRACE=off` leaves them out.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn program(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/traces/{name}.ok"))
}

fn okkama(args: &[&str], path: &Path, trace: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_okkama"));
    command.args(args).arg(path);
    if !trace {
        command.env("OKKAMA_TRACE", "off");
    }
    command.output().unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The place and the stack: everything from the source snippet on.
fn place_and_stack(text: &str) -> &str {
    let start = text.find(" --> ").or_else(|| text.find("  --> ")).unwrap_or_else(|| panic!("no place in:\n{text}"));
    &text[text[..start].rfind('\n').map_or(0, |i| i + 1)..]
}

/// Programs whose uncaught failure every engine reports the same way (#270).
const REPORTED: [&str; 9] = ["recursion", "closure", "rethrow", "task", "cause", "chain", "cleanup", "deep", "collect"];

#[test]
fn sparrow_reports_failures_causes_and_cleanup() {
    // Every root and cause has a place and stack; secondary cleanup
    // failures keep their text-only entries.
    for name in REPORTED {
        let path = program(name);
        let core = okkama(&["run", "--engine", "sparrow"], &path, true);
        assert_eq!(core.status.code(), Some(1), "{name}: {}", stderr(&core));
        let core = stderr(&core);
        assert!(core.contains("Stack trace (most recent call first):\n"), "{name}:\n{core}");
        assert!(place_and_stack(&core).len() < core.len(), "{name}");
        if name == "cleanup" || name == "collect" {
            let stacks = if name == "cleanup" { 2 } else { 1 };
            assert_eq!(core.matches("Stack trace (most recent call first):").count(), stacks, "{core}");
            for secondary in core.split("Secondary cleanup failure:\n").skip(1) {
                assert!(secondary.starts_with("  CloseError:") || secondary.starts_with("  Broken:"), "{core}");
                let entry = secondary.split("Caused by:").next().unwrap();
                assert!(!entry.contains("  Stack trace (most recent call first):"), "{core}");
            }
        }
    }
    let cause = stderr(&okkama(&["run", "--engine", "sparrow"], &program("cause"), false));
    assert!(cause.starts_with("OrderError: OrderError.failed(7)\nCaused by:\n  StoreError: StoreError.closed\n"), "{cause}");
    let cleanup = stderr(&okkama(&["run", "--engine", "sparrow"], &program("cleanup"), false));
    assert!(
        cleanup.starts_with(
            "JobError: JobError.broken\nSecondary cleanup failure:\n  CloseError: CloseError.stuck(\"b\")\n\
             Secondary cleanup failure:\n  CloseError: CloseError.stuck(\"a\")\nCaused by:\n  StoreError: StoreError.closed\n"
        ),
        "{cleanup}"
    );
    // AllFutures: the first failure observed stays primary, the rest follow
    // in input order, a failure given twice once (#443, #445); siblings
    // woken before the cancellation fail on their own (#546).
    let collect = stderr(&okkama(&["run", "--engine", "sparrow"], &program("collect"), true));
    assert!(
        collect.starts_with(
            "Broken: Broken.broken(\"f3\")\nSecondary cleanup failure:\n  Broken: Broken.broken(\"f1\")\n\
             Secondary cleanup failure:\n  Broken: Broken.broken(\"f0\")\n\
             Secondary cleanup failure:\n  Broken: Broken.broken(\"f2\")\n  --> "
        ),
        "{collect}"
    );
    let core = stderr(&okkama(&["run", "--engine", "sparrow"], &program("recursion"), true));
    let stack = &core[core.find("Stack trace").unwrap()..];
    let frames: Vec<&str> = stack.lines().skip(1).map(|line| line.split(" (").next().unwrap()).collect();
    assert_eq!(frames, ["  at Worker.fail", "  at Worker.fail", "  at main"], "{core}");
    assert!(stack.contains("  (\u{00d7}2)\n"), "{core}");
}

#[test]
fn bytecode_runs_print_the_stack_in_sparrow_terms() {
    let output = std::env::temp_dir().join(format!("okkama-traces-{}.spb", std::process::id()));
    let status = Command::new(env!("CARGO_BIN_EXE_okkama"))
        .args(["build", "--target", "sparrow", "-o"])
        .arg(&output)
        .arg(program("recursion"))
        .status()
        .unwrap();
    assert!(status.success());
    let traced = stderr(&okkama(&["exec"], &output, true));
    let plain = stderr(&okkama(&["exec"], &output, false));
    let _ = std::fs::remove_file(&output);
    assert!(
        traced.starts_with("Panic: stop\nStack trace (Sparrow, most recent call first):\n  at Worker.fail [f"),
        "{traced}"
    );
    assert!(traced.contains(": panic r") && traced.contains("  at main [f"), "{traced}");
    assert_eq!(plain, "Panic: stop\n");
}

#[test]
fn cause_stacks_preserve_each_origin() {
    for (name, entries) in [("chain", 3), ("rethrow", 1), ("cause", 2), ("cause_callee", 2)] {
        let output = okkama(&["run", "--engine", "sparrow"], &program(name), true);
        let text = stderr(&output);
        assert_eq!(output.status.code(), Some(1), "{text}");
        println!("=== {name} ===\n{text}");
        assert_eq!(text.matches("Caused by:").count(), entries - 1, "{text}");
        assert_eq!(text.matches("Stack trace (most recent call first):").count(), entries, "{text}");
        assert_eq!(text.matches("--> ").count(), entries, "{text}");
        let parts: Vec<_> = text.split("Caused by:\n").collect();
        let expected: &[&[&str]] = match name {
            "chain" => &[
                &["chain.ok:18:57", "at Orders.place", "at main"],
                &["chain.ok:12:56", "at Store.save", "at Orders.place"],
                &["chain.ok:7:22", "at Disk.write", "at Store.save"],
            ],
            "rethrow" => &[&["rethrow.ok:5:21", "at Store.load", "at Store.retry", "at main"]],
            "cause" => &[
                &["cause.ok:8:56", "at Orders.place", "at main"],
                &["cause.ok:6:30", "at Orders.save", "at Orders.place"],
            ],
            "cause_callee" => &[
                &["cause_callee.ok:10:23", "at Store.failed", "at Store.save", "at main"],
                &["cause_callee.ok:6:22", "at Disk.write", "at Store.save"],
            ],
            _ => unreachable!(),
        };
        for (part, expected) in parts.iter().zip(expected) {
            for fragment in *expected {
                assert!(part.contains(fragment), "missing {fragment}:\n{text}");
            }
        }
        for cause in parts.iter().skip(1) {
            assert!(cause.lines().all(|line| line.starts_with("  ")), "{text}");
        }
        if name == "rethrow" {
            assert_eq!(text.matches("Broken: Broken.broken").count(), 1, "{text}");
            assert!(!text.contains("rethrow.ok:10:"), "{text}");
        }
        let plain = stderr(&okkama(&["run", "--engine", "sparrow"], &program(name), false));
        assert_eq!(plain.matches("Caused by:").count(), entries - 1, "{plain}");
        assert!(!plain.contains("Stack trace") && !plain.contains("-->"), "{plain}");
    }
}

#[test]
fn bytecode_cause_stacks_preserve_each_origin() {
    for (name, entries, frames) in [
        ("chain", 3, &["Orders.place", "Store.save", "Disk.write"][..]),
        ("rethrow", 1, &["Store.load", "Store.retry"][..]),
        ("cause", 2, &["Orders.place", "Orders.save"][..]),
        ("cause_callee", 2, &["Store.failed", "Disk.write"][..]),
    ] {
        let output = std::env::temp_dir().join(format!("okkama-cause-traces-{}-{name}.spb", std::process::id()));
        let built = Command::new(env!("CARGO_BIN_EXE_okkama"))
            .args(["build", "--target", "sparrow", "-o"])
            .arg(&output)
            .arg(program(name))
            .output()
            .unwrap();
        assert!(built.status.success(), "{}", stderr(&built));
        let traced = okkama(&["exec"], &output, true);
        let plain = okkama(&["exec"], &output, false);
        let _ = std::fs::remove_file(&output);
        assert_eq!(traced.status.code(), Some(1));
        assert_eq!(plain.status.code(), Some(1));
        let traced = stderr(&traced);
        let plain = stderr(&plain);
        assert_eq!(traced.matches("Caused by:").count(), entries - 1, "{traced}");
        assert_eq!(traced.matches("Stack trace (Sparrow, most recent call first):").count(), entries, "{traced}");
        for frame in frames {
            assert!(traced.contains(&format!("at {frame} [f")), "{traced}");
        }
        for cause in traced.split("Caused by:\n").skip(1) {
            assert!(cause.lines().all(|line| line.starts_with("  ")), "{traced}");
        }
        assert!(!plain.contains("Stack trace"), "{plain}");
        assert_eq!(plain.matches("Caused by:").count(), entries - 1, "{plain}");
        if name == "rethrow" {
            assert_eq!(plain, "Broken: Broken.broken\n");
        }
    }
}
