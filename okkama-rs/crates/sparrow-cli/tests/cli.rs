use okkama_sparrow::{Constant, Dst, FuncRef, Function, Instruction, Program, Register, Target, Type, TypeRef};
use std::{
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
struct File(PathBuf);
impl File {
    fn new(bytes: &[u8]) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "sparrow-cli-{}-{}.spb",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, bytes).unwrap();
        Self(path)
    }
    fn run(&self, command: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_sparrow"))
            .arg(command)
            .arg(&self.0)
            .env("PATH", "")
            .output()
            .unwrap()
    }
}
impl Drop for File {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn main(registers: usize, code: Vec<Instruction>) -> Program {
    Program {
        module: "app".into(),
        version: "1.0.0".into(),
        types: vec![Type::Int],
        functions: vec![Function {
            symbol: "main".into(),
            is_async: false,
            native: None,
            intrinsic: None,
            registers: vec![TypeRef(0); registers],
            params: Vec::new(),
            result: Some(TypeRef(0)),
            code,
            handlers: Vec::new(),
        }],
        entry: Some(FuncRef(0)),
        ..Program::default()
    }
}

fn value(text: &str) -> Vec<u8> {
    okkama_sparrow::encode(&main(
        1,
        vec![
            Instruction::Const { dst: Dst(Register(0)), value: Constant::Int(text.into()) },
            Instruction::Return { value: Some(Register(0)) },
        ],
    ))
    .unwrap()
}
#[test]
fn standalone_run_and_check_report_exit_codes() {
    for (text, expected) in [
        ("0", 0),
        ("42", 42),
        ("255", 255),
        ("256", 1),
        ("-1", 1),
        ("99999999999999999999999999", 1),
    ] {
        let file = File::new(&value(text));
        assert!(file.run("check").status.success());
        let result = file.run("run");
        let direct = Command::new(env!("CARGO_BIN_EXE_sparrow"))
            .arg(&file.0)
            .env("PATH", "")
            .output()
            .unwrap();
        assert_eq!(direct.status.code(), result.status.code());
        assert_eq!(direct.stdout, result.stdout);
        assert_eq!(direct.stderr, result.stderr);
        assert_eq!(result.status.code(), Some(expected));
        assert!(result.stdout.is_empty());
        if expected == 1 {
            assert!(String::from_utf8_lossy(&result.stderr).contains("0..255"));
        } else {
            assert!(result.stderr.is_empty());
        }
    }
}
#[test]
fn check_does_not_execute_a_valid_infinite_loop() {
    let p = main(0, vec![Instruction::Jump { target: Target { pc: 0, moves: vec![] } }]);
    assert!(
        File::new(&okkama_sparrow::encode(&p).unwrap())
            .run("check")
            .status
            .success()
    );
}
#[test]
fn malformed_large_and_missing_files_are_diagnostics() {
    let file = File::new(b"SPRW");
    for cmd in ["run", "check"] {
        let result = file.run(cmd);
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
    }
    std::fs::File::create(&file.0)
        .unwrap()
        .set_len(okkama_sparrow::MAX_FILE_BYTES as u64 + 1)
        .unwrap();
    assert_eq!(file.run("run").status.code(), Some(1));
    std::fs::remove_file(&file.0).unwrap();
    assert_eq!(file.run("run").status.code(), Some(3));
}
#[test]
fn help_version_and_usage_are_explicit() {
    for args in [
        vec!["--help"],
        vec!["run", "--help"],
        vec!["check", "--help"],
        vec!["--version"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_sparrow"))
            .args(args)
            .output()
            .unwrap();
        assert!(result.status.success());
        assert!(result.stderr.is_empty());
    }
    for args in [
        vec![],
        vec!["run"],
        vec!["check", "a", "b"],
        vec!["--assemble", "a"],
        vec!["unknown", "a"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_sparrow"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn assembly_round_trips_through_the_command_line() {
    let file = File::new(&value("42"));
    let text = Command::new(env!("CARGO_BIN_EXE_sparrow")).arg("--disassemble").arg(&file.0).output().unwrap();
    assert!(text.status.success());
    let listing = String::from_utf8(text.stdout).unwrap();
    assert!(listing.contains("const r0, int 42"), "{listing}");
    let source = File::new(listing.as_bytes());
    let output = File::new(b"");
    let assembled = Command::new(env!("CARGO_BIN_EXE_sparrow"))
        .arg("--assemble")
        .arg(&source.0)
        .arg("-o")
        .arg(&output.0)
        .output()
        .unwrap();
    assert!(assembled.status.success(), "{}", String::from_utf8_lossy(&assembled.stderr));
    assert_eq!(std::fs::read(&output.0).unwrap(), value("42"));
    assert_eq!(output.run("run").status.code(), Some(42));
    let broken = File::new(listing.replace("int 42", "int 042").as_bytes());
    let failed = Command::new(env!("CARGO_BIN_EXE_sparrow")).arg("--assemble").arg(&broken.0).arg("-o").arg(&output.0).output().unwrap();
    assert_eq!(failed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failed.stderr).contains("line"));
}

#[test]
fn uncaught_failures_are_reported_like_okkama() {
    let file = File::new(
        &okkama_sparrow::encode(&main(
            0,
            vec![Instruction::Trap { code: okkama_sparrow::TrapCode::Confirm, message: "confirm failed".into() }],
        ))
        .unwrap(),
    );
    let result = file.run("run");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&result.stderr), "RuntimeError: confirm failed\n");
}
