//! Host state belongs to one run (#40): a second program on the same thread
//! does not see the environment or working directory the first one changed.
#[path = "sparrow_support/mod.rs"]
mod support;
use okkama_compiler::{SourceFile, ir, lowering, syntax};

const ENVIRONMENT: &str = "native environment_get(name is @byte) is @byte;
    native environment_set(name is @byte, value is @byte) is bool;";
const FS: &str = "native cwd() is @byte; native chdir(path is @byte) is bool;";
/// `OKKAMA_HOST_40` as bytes.
const NAME: &str = "[79.byte(), 75.byte(), 75.byte(), 65.byte(), 77.byte(), 65.byte(), 95.byte(), 72.byte(), 79.byte(), 83.byte(), 84.byte(), 95.byte(), 52.byte(), 48.byte()]";

fn lower(module: &str, natives: &str, body: &str) -> ir::Module {
    let source = SourceFile::new("host.ok", format!("module {module} v1.0.0; {natives} main() is int {{ {body} }}")).unwrap();
    lowering::lower_program(&syntax::parse(&source, &syntax::ParseOptions::default()).unwrap()).unwrap()
}

/// Result of `main` on Sparrow, or `None` when it failed.
fn run(module: &ir::Module) -> Option<String> {
    match support::sparrow(module) {
        support::Ending::Value(value) => Some(value.to_string()),
        _ => None,
    }
}

#[test]
fn an_environment_change_does_not_outlive_its_run() {
    let set = lower("runtime/environment", ENVIRONMENT, &format!("if environment_set({NAME}, [49.byte()]) {{ environment_get({NAME}).size(); }} else {{ 0; }}"));
    let get = lower("runtime/environment", ENVIRONMENT, &format!("environment_get({NAME}).size();"));
    for _ in 0..2 {
        assert_eq!(run(&set), Some("1".into()));
        assert_eq!(run(&get), None, "the next run sees the variable");
    }
    assert!(std::env::var_os("OKKAMA_HOST_40").is_none(), "the process environment changed");
}

#[test]
fn a_working_directory_change_does_not_outlive_its_run() {
    let process = std::env::current_dir().unwrap().as_os_str().len().to_string();
    assert_ne!(process, "1");
    let chdir = lower("runtime/fs", FS, "if chdir([47.byte()]) { cwd().size(); } else { 0; }");
    let cwd = lower("runtime/fs", FS, "cwd().size();");
    for _ in 0..2 {
        assert_eq!(run(&chdir), Some("1".into()));
        assert_eq!(run(&cwd), Some(process.clone()), "the next run starts in /");
    }
    assert_eq!(std::env::current_dir().unwrap().as_os_str().len().to_string(), process);
}
