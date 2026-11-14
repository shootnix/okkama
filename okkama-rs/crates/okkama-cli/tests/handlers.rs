//! Handlers, cleanup regions and nested failing calls under a stressed
//! collector (audit S03, #22): Sparrow preserves the report under GC stress.

use std::process::Command;

fn run(engine: &str, stress: &str) -> (Option<i32>, String) {
    let program = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/handlers/main.ok");
    let output = Command::new(env!("CARGO_BIN_EXE_okkama"))
        .args(["run", "--engine", engine])
        .arg(&program)
        .env("SPARROW_CORE_GC_STRESS", stress)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout).into_owned() + &String::from_utf8_lossy(&output.stderr);
    (output.status.code(), text)
}

#[test]
fn handlers_preserve_reports_under_collector_stress() {
    let expected = run("sparrow", "0");
    assert_eq!(expected.0, Some(0), "{}", expected.1);
    assert!(expected.1.ends_with("total 504\n"), "{}", expected.1);
    assert_eq!(run("sparrow", "1"), expected);
}
