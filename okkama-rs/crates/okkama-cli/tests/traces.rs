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

#[test]
fn the_core_reports_the_same_place_and_stack_as_the_ir_engine() {
    // A rethrow inside `catch` links the caught failure as its cause: the core
    // prints the `Caused by:` chain, the IR engine does not, so only the place
    // and the stack are compared.
    for name in ["recursion", "closure", "rethrow", "task"] {
        let path = program(name);
        let ir = okkama(&["run", "--engine", "ir"], &path, true);
        let core = okkama(&["run", "--engine", "sparrow"], &path, true);
        assert_eq!(ir.status.code(), Some(1), "{name}: {}", stderr(&ir));
        assert_eq!(core.status.code(), Some(1), "{name}: {}", stderr(&core));
        let (ir, core) = (stderr(&ir), stderr(&core));
        assert!(core.contains("Stack trace (most recent call first):\n"), "{name}:\n{core}");
        assert_eq!(place_and_stack(&core), place_and_stack(&ir), "{name}");
    }
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
