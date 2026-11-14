//! Removing the interpreter must reject stale engine selections before loading source.
use std::process::{Command, Output};

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_okkama"));
    command.env_remove("OKKAMA_ENGINE").env_remove("OCS_RUST_ENGINE");
    command
}

fn removed(output: Output) {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains("IR engine") && message.contains("removed"), "{message}");
    assert!(message.contains("Sparrow is the only"), "{message}");
}

#[test]
fn explicit_ir_selection_is_rejected_for_programs_and_tests() {
    for flags in [&["run", "--engine", "ir"][..], &["run", "-t", "--engine", "ir"][..]] {
        removed(command().args(flags).arg("missing.ok").output().unwrap());
    }
}

#[test]
fn ir_environment_is_rejected() {
    removed(command().env("OKKAMA_ENGINE", "ir").args(["run", "missing.ok"]).output().unwrap());
}

#[test]
fn ocs_rejects_ir_and_defaults_to_sparrow() {
    removed(command().env("OCS_RUST_ENGINE", "ir").arg("--ocs-capabilities").output().unwrap());
    let output = command().arg("--ocs-capabilities").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"engine\": \"sparrow\""));
}

#[test]
fn repl_is_no_longer_a_command() {
    let output = command().arg("repl").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let help = command().arg("--help").output().unwrap();
    assert!(!String::from_utf8_lossy(&help.stdout).contains("okkama repl"));
}

const REMOVED_BACKEND: &str = "the JVM backend was removed in 0.6.0 (#585); Sparrow is the only Okkama engine";

#[test]
fn removed_backend_selections_are_usage_errors() {
    for flags in [
        &["run", "--engine", "jvm", "missing.ok"][..],
        &["run", "-t", "--engine", "jvm", "missing.ok"],
        &["build", "--target", "jvm", "missing.ok", "-o", "out.spb"],
        &["build", "--target=jvm", "missing.ok", "-o", "out.spb"],
    ] {
        let output = command().args(flags).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(REMOVED_BACKEND));
    }
    for variable in ["OKKAMA_ENGINE", "OCS_RUST_ENGINE"] {
        let mut selected = command();
        selected.env(variable, "jvm");
        if variable == "OKKAMA_ENGINE" {
            selected.args(["run", "missing.ok"]);
        } else {
            selected.arg("--ocs-capabilities");
        }
        let output = selected.output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains(REMOVED_BACKEND));
    }
}
