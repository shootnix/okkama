//! Code point positions of long strings (#52): every engine slices them the
//! same way, one code point at a time in either direction.

use std::path::Path;
use std::process::Command;

#[test]
fn every_engine_slices_long_strings_by_code_point() {
    let program = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/strings/positions.ok");
    for engine in ["ir", "sparrow", "jvm"] {
        let output = Command::new(env!("CARGO_BIN_EXE_okkama")).args(["run", "--engine", engine]).arg(&program).output().unwrap();
        assert_eq!(output.status.code(), Some(0), "{engine}: {}", String::from_utf8_lossy(&output.stderr));
    }
}
