//! A terminal read interrupted by a timeout or a cancelled task gives its
//! late bytes back to the session, and a resize reports the new geometry
//! (#12). Runs `tests/terminal/cancel.ok` under a pseudo-terminal driven by
//! `tests/terminal/pty_driver.py`, on every engine that can read a terminal.

use std::path::Path;
use std::process::Command;

#[test]
#[cfg(unix)]
fn terminal_reads_survive_cancellation_and_resizes_report_geometry() {
    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipped: no python3");
        return;
    }
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let std = here.join("../../../std");
    let driver = here.join("tests/terminal/pty_driver.py");
    let program = here.join("tests/terminal/cancel.ok");
    let mut engines = vec!["sparrow"];
    if Command::new("java").arg("-version").output().is_ok() {
        engines.push("jvm");
    }
    let failures: Vec<String> = engines
        .into_iter()
        .filter_map(|engine| {
            let output = Command::new("python3")
                .arg(&driver)
                .arg(env!("CARGO_BIN_EXE_okkama"))
                .arg(engine)
                .arg(&program)
                .env("OKKAMA_STDLIB_DIR", &std)
                .output()
                .expect("run the pty driver");
            (!output.status.success()).then(|| {
                format!(
                    "{engine}:\n{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
            })
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
