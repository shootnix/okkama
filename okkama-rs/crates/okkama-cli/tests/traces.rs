//! Call stacks of uncaught failures on the Sparrow core (#10): `okkama run`
//! maps them to source exactly as the IR engine does, `okkama exec` and
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
const REPORTED: [&str; 8] = ["recursion", "closure", "rethrow", "task", "cause", "chain", "cleanup", "deep"];

#[test]
fn the_core_and_the_ir_engine_print_the_same_report() {
    // The whole report: the failure, its secondary cleanup failures and its
    // causes (#270), then the place and the stack.
    for name in REPORTED {
        let path = program(name);
        let ir = okkama(&["run", "--engine", "ir"], &path, true);
        let core = okkama(&["run", "--engine", "sparrow"], &path, true);
        assert_eq!(ir.status.code(), Some(1), "{name}: {}", stderr(&ir));
        assert_eq!(core.status.code(), Some(1), "{name}: {}", stderr(&core));
        let (ir, core) = (stderr(&ir), stderr(&core));
        assert!(core.contains("Stack trace (most recent call first):\n"), "{name}:\n{core}");
        assert!(place_and_stack(&core).len() < core.len(), "{name}");
        assert_eq!(core, ir, "{name}");
    }
    let cause = stderr(&okkama(&["run", "--engine", "ir"], &program("cause"), true));
    assert!(cause.starts_with("OrderError: OrderError.failed(7)\nCaused by:\n  StoreError: StoreError.closed\n"), "{cause}");
    let cleanup = stderr(&okkama(&["run", "--engine", "ir"], &program("cleanup"), true));
    assert!(
        cleanup.starts_with(
            "JobError: JobError.broken\nSecondary cleanup failure:\n  CloseError: CloseError.stuck(\"b\")\n\
             Secondary cleanup failure:\n  CloseError: CloseError.stuck(\"a\")\nCaused by:\n  StoreError: StoreError.closed\n"
        ),
        "{cleanup}"
    );
    // AllFutures on the core: the first failure observed stays primary, the
    // rest follow in input order, a failure given twice once (#443, #445).
    // The IR engine cancels these siblings before they fail (#546).
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
