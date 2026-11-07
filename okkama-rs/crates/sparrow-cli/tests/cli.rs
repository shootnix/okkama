use okkama_sparrow::{Constant, Dst, FuncRef, Function, Handler, Instruction, Program, Register, Target, Type, TypeRef};
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
        self.run_with(&[command])
    }
    fn run_with(&self, options: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_sparrow"))
            .args(options)
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
    // The core and the previous VM add the Sparrow stack (the core since #10);
    // `OKKAMA_TRACE=off` — only the failure.
    assert_eq!(
        String::from_utf8_lossy(&result.stderr),
        "RuntimeError: confirm failed\n\
         Stack trace (Sparrow, most recent call first):\n  at main [f0 @0: trap confirm, \"confirm failed\"]\n"
    );
    let quiet_core = Command::new(env!("CARGO_BIN_EXE_sparrow")).arg("run").arg(&file.0).env("OKKAMA_TRACE", "off").output().unwrap();
    assert_eq!(String::from_utf8_lossy(&quiet_core.stderr), "RuntimeError: confirm failed\n");
    let origin = Command::new(env!("CARGO_BIN_EXE_sparrow-origin")).arg("run").arg(&file.0).output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&origin.stderr),
        "RuntimeError: confirm failed\n\
         Stack trace (Sparrow, most recent call first):\n  at main [f0 @0: trap confirm, \"confirm failed\"]\n"
    );
    let quiet = Command::new(env!("CARGO_BIN_EXE_sparrow-origin")).arg("run").arg(&file.0).env("OKKAMA_TRACE", "off").output().unwrap();
    assert_eq!(String::from_utf8_lossy(&quiet.stderr), "RuntimeError: confirm failed\n");
}

/// An Atomic read on an object the program builds itself: the core would
/// read the cell's closed flag past the object (#16).
#[test]
fn check_refuses_intrinsics_against_their_runtime_contract() {
    use okkama_sparrow::{Intrinsic, ObjectDecl, ObjectRef};
    let mut program = main(
        3,
        vec![
            Instruction::ObjNew { dst: Dst(Register(1)), object: ObjectRef(0), fields: vec![] },
            Instruction::Call { dst: Some(Dst(Register(2))), function: FuncRef(1), args: vec![Register(1)] },
            Instruction::Const { dst: Dst(Register(0)), value: Constant::Int("0".into()) },
            Instruction::Return { value: Some(Register(0)) },
        ],
    );
    program.types.extend([Type::Object(ObjectRef(0)), Type::Bool]);
    program.functions[0].registers = vec![TypeRef(0), TypeRef(1), TypeRef(2)];
    program.objects = vec![ObjectDecl { name: "Atomic".into(), fields: vec![], does: vec![], destroy: None, builtin: false }];
    program.functions.push(Function {
        symbol: "Atomic.closed".into(),
        is_async: false,
        native: None,
        intrinsic: Some(Intrinsic { name: "okkama.atomic.Atomic.closed".into(), objects: vec![], helpers: vec![] }),
        registers: vec![TypeRef(1)],
        params: vec![Register(0)],
        result: Some(TypeRef(2)),
        code: Vec::new(),
        handlers: Vec::new(),
    });
    let file = File::new(&okkama_sparrow::encode(&program).unwrap());
    for command in ["check", "run"] {
        let output = file.run(command);
        assert_eq!(output.status.code(), Some(1), "{command}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("intrinsic contract: parameter 0"), "{command}: {stderr}");
    }
    // Without the plain construction no such object can exist: accepted.
    program.functions[0].code[0] = Instruction::Undef { dst: Dst(Register(1)) };
    program.functions[0].code[1] = Instruction::Const { dst: Dst(Register(2)), value: Constant::Bool(false) };
    let file = File::new(&okkama_sparrow::encode(&program).unwrap());
    assert!(file.run("check").status.success());
}

/// An infinite loop inside a handler's range: budgets stop it, and the
/// handler does not catch exhaustion (#21).
#[test]
fn budgets_stop_a_runaway_loop_that_handlers_cannot_catch() {
    let program = Program {
        module: "app".into(),
        version: "1.0.0".into(),
        types: vec![Type::Int, Type::Failure],
        functions: vec![Function {
            symbol: "main".into(),
            is_async: false,
            native: None,
            intrinsic: None,
            registers: vec![TypeRef(0), TypeRef(1)],
            params: Vec::new(),
            result: Some(TypeRef(0)),
            code: vec![
                Instruction::Const { dst: Dst(Register(0)), value: Constant::Int("7".into()) },
                Instruction::Jump { target: Target { pc: 1, moves: vec![] } },
                Instruction::Return { value: Some(Register(0)) },
            ],
            handlers: vec![Handler { start: 1, end: 2, target: Target { pc: 2, moves: vec![] }, failure: Register(1) }],
        }],
        entry: Some(FuncRef(0)),
        ..Program::default()
    };
    let file = File::new(&okkama_sparrow::encode(&program).unwrap());
    let stopped = file.run_with(&["run", "--max-steps", "100000"]);
    assert_eq!(stopped.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&stopped.stderr), "Execution budget exhausted: steps\n");
    let interrupted = file.run_with(&["run", "--timeout", "0.2"]);
    assert_eq!(interrupted.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&interrupted.stderr), "Execution interrupted\n");
    let invalid = file.run_with(&["run", "--max-memory", "lots"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("invalid value `lots` for --max-memory"));
}

/// Exceptional edges keep the old value of a register the call or
/// instruction would overwrite (audit S03, #22): the handler reads it after
/// collections, with the collector stressed (every safepoint collects and
/// reclaimed cells are poisoned).
#[test]
fn handlers_read_old_registers_across_collections() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/handlers");
    for name in ["audit", "allocating-callee", "failing-instruction", "nested-calls"] {
        let text = std::fs::read_to_string(directory.join(format!("{name}.spa"))).unwrap();
        let program = okkama_sparrow::assemble(&text).unwrap();
        let file = File::new(&okkama_sparrow::encode(&program).unwrap());
        for stress in ["0", "1"] {
            let output = Command::new(env!("CARGO_BIN_EXE_sparrow"))
                .arg("run")
                .arg(&file.0)
                .env("SPARROW_CORE_GC_STRESS", stress)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(5), "{name}, stress {stress}: {}", String::from_utf8_lossy(&output.stderr));
        }
    }
}
