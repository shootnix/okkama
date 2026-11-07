//! Name conflicts name both sources, and `check` does not depend on the
//! order of its inputs (H-16, #33).

use std::path::Path;
use std::process::Command;

fn check(files: &[&str]) -> String {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/names");
    let output = Command::new(env!("CARGO_BIN_EXE_okkama")).arg("check").args(files).current_dir(&directory).output().unwrap();
    assert_eq!(output.status.code(), Some(1), "{files:?}");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn a_conflict_names_both_sources() {
    assert_eq!(
        check(&["take2.ok"]),
        "take2.ok:3:1: semantic error: Object \"Thing\" is already declared by module \"lib/a\"; this import takes another from module \"lib/b\"\n"
    );
    assert_eq!(
        check(&["own.ok"]),
        "own.ok:2:1: semantic error: Object \"Thing\" is already declared by module \"app\"; this import takes another from module \"lib/a\"\n"
    );
    assert_eq!(
        check(&["as2.ok"]),
        "as2.ok:3:1: semantic error: Namespace \"m\" is already declared for module \"lib/a\"; this import names module \"lib/b\"\n"
    );
    assert_eq!(
        check(&["first.ok", "second.ok"]),
        "second.ok:2:8: semantic error: Object \"Thing\" is already declared in another file of this module\n\
         first.ok:2:8: semantic error: Object \"Thing\" is first declared here\n"
    );
}

#[test]
fn the_order_of_inputs_changes_nothing() {
    assert_eq!(check(&["first.ok", "second.ok"]), check(&["second.ok", "first.ok"]));
}
