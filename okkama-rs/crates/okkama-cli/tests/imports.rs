//! Глубина графа импортов (аудит S06, #20): обход не расходует стек
//! потока, а слишком глубокая цепочка — диагностика, не аварийный выход.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct WorkDir(PathBuf);

impl WorkDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "okkama-imports-test-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// `main.ok` → m1 → m2 → … → m`depth`; `last` — импорты последнего.
    fn chain(&self, depth: usize, last: &str) -> &Path {
        for index in 1..=depth {
            let import = if index < depth {
                format!("import \"m{}\" take T{};\n", index + 1, index + 1)
            } else {
                last.to_string()
            };
            let text = format!("module m{index};\n{import}object T{index} {{ T{index}(); }}\n");
            std::fs::write(self.0.join(format!("m{index}.ok")), text).unwrap();
        }
        let main = "module app;\nimport \"m1\" take T1;\nmain() is int { 0; }\n";
        std::fs::write(self.0.join("main.ok"), main).unwrap();
        &self.0
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn check(dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_okkama"))
        .arg("check")
        .arg("main.ok")
        .current_dir(dir)
        .output()
        .unwrap()
}

#[test]
fn a_deep_import_chain_checks_without_exhausting_the_stack() {
    let dir = WorkDir::new();
    let output = check(dir.chain(1000, ""));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn an_import_chain_past_the_depth_limit_is_a_diagnostic() {
    // 16 000 модулей раньше переполняли стек (SIGABRT).
    let dir = WorkDir::new();
    let output = check(dir.chain(16_000, ""));
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("m1024.ok:2:1: semantic error: Import \"m1025\" nests deeper than 1024 modules"),
        "{stderr}"
    );
}

#[test]
fn a_cycle_at_the_end_of_a_long_chain_keeps_its_diagnostic() {
    let dir = WorkDir::new();
    let output = check(dir.chain(600, "import \"m598\" take T598;\n"));
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("m600.ok:2:1: semantic error: Import cycle detected: m598 -> m599 -> m600 -> m598"),
        "{stderr}"
    );
}
