//! IR passes keep meaning (H-09, #29): programs print the same with every
//! pass, each pass alone and none, and a deliberately wrong pass is caught.

use std::process::Command;

const PASSES: [&str; 7] = ["all", "none", "tail-calls", "hoist", "regions", "jumps", "undef"];

/// Stdout, stderr and status of `program` on the IR engine with `passes`.
fn run(program: &str, passes: &str) -> (String, String, Option<i32>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/ir-passes/{program}.ok"));
    let output = Command::new(env!("CARGO_BIN_EXE_okkama"))
        .args(["run", "--engine", "ir"])
        .arg(&path)
        .env("OKKAMA_IR_PASSES", passes)
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

#[test]
fn every_pass_selection_prints_the_same() {
    for program in ["arithmetic", "failures"] {
        let reference = run(program, "none");
        for passes in PASSES {
            assert_eq!(run(program, passes), reference, "{program} with OKKAMA_IR_PASSES={passes}");
        }
    }
    let (stdout, stderr, status) = run("failures", "all");
    assert_eq!(stdout, "close inner-4\nclose outer-4\n40\n5000050000\n");
    assert!(stderr.starts_with("RuntimeError: Modulo by zero"), "{stderr}");
    assert_eq!(status, Some(1));
}

#[test]
fn a_wrong_pass_is_caught() {
    assert_ne!(run("arithmetic", "sabotage-int-add"), run("arithmetic", "none"));
}
