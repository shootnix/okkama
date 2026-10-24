//! Behaviour of the `okkama` binary: commands, options, streams and exit
//! codes.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const HELLO: &str = "module app v1.0.0;\n\nmain() is int {\n    0;\n}\n";

fn okkama() -> Command {
    Command::new(env!("CARGO_BIN_EXE_okkama"))
}

fn run(args: &[&str]) -> Output {
    okkama().args(args).output().expect("run okkama")
}

fn run_stdin(args: &[&str], input: &[u8]) -> Output {
    let mut child = okkama()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn okkama");
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

/// A temporary file that is removed when dropped.
struct TempFile(PathBuf);

impl TempFile {
    fn new(name: &str, contents: &[u8]) -> TempFile {
        let path = std::env::temp_dir().join(format!(
            "okkama-cli-test-{}-{}-{name}",
            std::process::id(),
            name.len()
        ));
        std::fs::write(&path, contents).unwrap();
        TempFile(path)
    }

    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }

    fn absent(name: &str) -> TempFile {
        let file = Self::new(name, b"");
        std::fs::remove_file(&file.0).unwrap();
        file
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn example(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name);
    path.to_str().unwrap().to_string()
}

#[test]
fn native_stdio_preserves_bytes_and_shared_input_state_in_both_engines() {
    let source = TempFile::new(
        "native-stdio.ok",
        br#"module app v1.0.0;
        import "runtime/stdio" take stdout_write, stderr_write, stdin_read, stdin_read_line;
        main() is int {
            let first = stdin_read_line();
            let middle = stdin_read(2);
            let tail = stdin_read_line();
            stdout_write(first); stdout_write(middle); stdout_write(tail);
            stdout_write(@byte(0, 255)); stderr_write(@byte(69, 10)); 0;
        }"#,
    );
    let bytecode = TempFile::absent("native-stdio.spb");
    let input = b"one\r\nabtail\n";
    let expected = [input.as_slice(), &[0, 255]].concat();
    let ir = run_stdin(&["run", source.path()], input);
    assert_eq!(ir.status.code(), Some(0), "{}", stderr(&ir));
    assert_eq!(ir.stdout, expected);
    assert_eq!(ir.stderr, b"E\n");
    let build = run_stdin(
        &[
            "build",
            source.path(),
            "--target",
            "sparrow",
            "-o",
            bytecode.path(),
        ],
        b"",
    );
    assert!(build.status.success(), "{}", stderr(&build));
    let vm = run_stdin(&["exec", bytecode.path()], input);
    assert_eq!(vm.status.code(), Some(0), "{}", stderr(&vm));
    assert_eq!(vm.stdout, expected);
    assert_eq!(vm.stderr, b"E\n");
}

#[test]
fn standard_stdout_uses_real_native_providers_in_the_ir_interpreter() {
    let source = TempFile::new("stdlib-stdout-ir.ok", "module app v1.0.0; import \"io/stdout\" take Stdout, Stderr; main() is int { Stdout().writeln(\"Aé🙂\"); Stderr().write(\"error\"); 0; }".as_bytes());
    let output = run_stdin(&["run", source.path()], b"");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout(&output), "Aé🙂\n");
    assert_eq!(stderr(&output), "error");
}

#[test]
fn ocs_native_engines_report_execution_and_limitations_without_java() {
    let source = TempFile::new("ocs-native.ok", HELLO.as_bytes());
    let request = TempFile::new("ocs-request.json", b"");
    let response = TempFile::new("ocs-response.json", b"");
    std::fs::write(
        &request.0,
        format!(
            "{{\"mode\":\"run\",\"sources\":[{}],\"args\":[]}}",
            format_args!("{:?}", source.path())
        ),
    )
    .unwrap();
    for engine in ["ir", "sparrow"] {
        for (body, outcome, code) in [
            ("6 * 7;", "completed", 42),
            ("256;", "failed", 1),
            ("7 % 0;", "failed", 1),
            ("panic \"stop\";", "failed", 1),
            ("if -7 % 3 == -1 { 42; } else { 0; }", "completed", 42),
            ("let x = 1.5; if x * 2.0 == 3.0 { 42; } else { 0; }", "completed", 42),
            ("let b is int = \"world\"; 0;", "rejected", 1),
        ] {
            std::fs::write(
                &source.0,
                format!("module app v1.0.0; main() is int {{ {body} }}"),
            )
            .unwrap();
            let output = okkama()
                .args([
                    "--ocs-request",
                    request.path(),
                    "--ocs-response",
                    response.path(),
                ])
                .env("OCS_RUST_ENGINE", engine)
                .env("PATH", "")
                .output()
                .unwrap();
            let reply = std::fs::read_to_string(&response.0).unwrap();
            assert_eq!(output.status.code(), Some(code), "{engine}: {reply}");
            assert!(
                reply.contains(&format!("\"outcome\": \"{outcome}\"")),
                "{reply}"
            );
            assert!(output.stdout.is_empty());
            if body.starts_with("panic ") {
                assert!(reply.contains("\"category\": \"panic\""), "{reply}");
                assert!(reply.contains("Panic: stop"), "{reply}");
            }
            if outcome == "completed" {
                assert!(reply.contains("\"exit_status\": 42"));
                assert!(output.stderr.is_empty());
            }
        }
    }
}

#[test]
fn ocs_capabilities_follow_the_selected_engine() {
    for engine in ["ir", "sparrow", "jvm"] {
        let output = okkama()
            .arg("--ocs-capabilities")
            .env("OCS_RUST_ENGINE", engine)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(stdout(&output).contains(&format!("\"engine\": \"{engine}\"")));
        // Все движки исполняют весь язык, включая режим тестов.
        assert!(stdout(&output).contains("\"test\""), "{engine}");
    }
    let output = okkama()
        .arg("--ocs-capabilities")
        .env_remove("OCS_RUST_ENGINE")
        .output()
        .unwrap();
    // Как `okkama run`: JVM по умолчанию.
    assert!(stdout(&output).contains("\"engine\": \"jvm\""));
    let output = okkama()
        .arg("--ocs-capabilities")
        .env("OCS_RUST_ENGINE", "typo")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn sparrow_build_and_exec_need_neither_source_nor_java() {
    let artifact = TempFile::absent("sparrow-roundtrip.spb");
    let output = run_stdin(
        &["build", "-", "--target", "sparrow", "-o", artifact.path()],
        b"module app v1.0.0; main() is int { let a = 20; a + 22; }",
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(std::fs::read(&artifact.0).unwrap().starts_with(b"SPRW"));
    let output = okkama()
        .args(["exec", artifact.path()])
        .env("PATH", "")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(42), "{}", stderr(&output));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn native_run_engines_execute_source_without_java() {
    let source = TempFile::new(
        "native-engines.ok",
        b"module app v1.0.0; main() is int { 6 * 7; }",
    );
    // По умолчанию `run` — JVM: без java в PATH это явная ошибка, а не тихий
    // переход на другой движок.
    let output = okkama()
        .args(["run", source.path()])
        .env("PATH", "")
        .env_remove("OKKAMA_ENGINE")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3), "{}", stderr(&output));
    assert!(stderr(&output).contains("cannot start java"));
    // OKKAMA_ENGINE меняет умолчание.
    let output = okkama()
        .args(["run", source.path()])
        .env("PATH", "")
        .env("OKKAMA_ENGINE", "ir")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(42), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
    for engine in ["ir", "sparrow"] {
        let output = okkama()
            .args(["run", "--engine", engine, source.path()])
            .env("PATH", "")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(42), "{}", stderr(&output));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
    let output = run_stdin(
        &["run", "--engine", "ir", "-"],
        b"module app v1.0.0; main() is int { let xs = [[1, 2], [40]]; xs[0][1] + xs[1][0]; }",
    );
    assert_eq!(output.status.code(), Some(42), "{}", stderr(&output));
    let output = run_stdin(
        &["run", "--engine", "ir", "-"],
        b"module app v1.0.0; main() is int { let xs = [1]; xs[2]; }",
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("Array index out of bounds"));
}

#[test]
fn legacy_backend_alias_is_supported_but_conflicts_are_errors() {
    let file = TempFile::new("target-alias.spb", b"previous");
    let output = run_stdin(
        &["build", "-", "--backend", "sparrow", "-o", file.path()],
        HELLO.as_bytes(),
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let previous = std::fs::read(&file.0).unwrap();
    let output = run_stdin(
        &[
            "build",
            "-",
            "--target",
            "jvm",
            "--backend",
            "sparrow",
            "-o",
            file.path(),
        ],
        HELLO.as_bytes(),
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("conflicting target"));
    assert_eq!(std::fs::read(&file.0).unwrap(), previous);
}

#[test]
fn native_run_does_not_ignore_arguments_or_fallback_to_java() {
    for args in [
        vec!["run", "--engine"],
        vec!["run", "--engine", "unknown", "-"],
    ] {
        assert_eq!(run(&args).status.code(), Some(2));
    }
    for engine in ["ir", "sparrow"] {
        let output = run_stdin(
            &["run", "--engine", engine, "-"],
            b"module runtime/unlinked v1.0.0; native nothing() is int; main() is int { nothing(); }",
        );
        assert_eq!(output.status.code(), Some(1));
        assert!(
            stderr(&output).contains("native provider is not implemented"),
            "{}",
            stderr(&output)
        );
    }
}

#[test]
fn sparrow_build_failure_preserves_artifact() {
    let artifact = TempFile::new("sparrow-preserved.spb", b"previous artifact");
    let output = run_stdin(
        &["build", "-", "--target", "sparrow", "-o", artifact.path()],
        b"module app v1.0.0; main() is int { missing_name; }",
    );
    assert_ne!(output.status.code(), Some(0));
    assert_eq!(std::fs::read(&artifact.0).unwrap(), b"previous artifact");
    // Float — часть языка Sparrow: та же программа собирается и исполняется.
    let output = run_stdin(
        &["build", "-", "--target", "sparrow", "-o", artifact.path()],
        b"module app v1.0.0; main() is int { let x = 1.5; (x * 4.0).int(); }",
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(run(&["exec", artifact.path()]).status.code(), Some(6));
}

#[test]
fn sparrow_exec_validates_file_and_exit_status() {
    let artifact = TempFile::new("sparrow-invalid.spb", b"SPRW");
    let output = run(&["exec", artifact.path()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("SPB"));
    for value in ["0", "255", "256", "999999999999999999999999999999999"] {
        let output = run_stdin(
            &["build", "-", "--target", "sparrow", "-o", artifact.path()],
            format!("module app v1.0.0; main() is int {{ {value}; }}").as_bytes(),
        );
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        let output = run(&["exec", artifact.path()]);
        match value.parse::<u8>() {
            Ok(code) => {
                assert_eq!(output.status.code(), Some(code.into()));
                assert!(output.stderr.is_empty());
            }
            Err(_) => {
                assert_eq!(output.status.code(), Some(1));
                assert!(stderr(&output).contains("0..255"));
            }
        }
    }
    std::fs::File::create(&artifact.0)
        .unwrap()
        .set_len(okkama_sparrow::MAX_FILE_BYTES as u64 + 1)
        .unwrap();
    assert_eq!(run(&["exec", artifact.path()]).status.code(), Some(1));
}

#[test]
fn sparrow_exec_usage_and_io_errors() {
    for args in [&["exec"][..], &["exec", "--unknown"]] {
        assert_eq!(run(args).status.code(), Some(2));
    }
    // Аргументы после файла — аргументы программы; отсутствующий файл — I/O.
    assert_eq!(run(&["exec", "missing-file.spb", "b"]).status.code(), Some(3));
    for args in [&["exec", "--help"][..], &["help", "exec"]] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(0));
        assert!(stdout(&output).contains("okkama exec"));
    }
    let absent = TempFile::absent("sparrow-missing.spb");
    assert_eq!(run(&["exec", absent.path()]).status.code(), Some(3));
}

#[test]
fn sparrow_cli_executes_control_flow_and_v2_function_calls() {
    use okkama_sparrow::{Function, Instruction::*, Program, Register as R};
    let file = TempFile::absent("sparrow-control-flow.spb");
    let source =
        b"module app v1.0.0; main() is int { for 1..=5 -> i { if i < 4 { next; } i + 1; } }";
    let output = run_stdin(
        &["build", "-", "--target", "sparrow", "-o", file.path()],
        source,
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let output = run(&["exec", file.path()]);
    assert_eq!(output.status.code(), Some(6), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
    let function = |symbol: &str, registers: usize, params: Vec<R>, code| Function {
        symbol: symbol.into(),
        is_async: false,
        native: None,
        intrinsic: None,
        registers: vec![okkama_sparrow::TypeRef(0); registers],
        params,
        result: Some(okkama_sparrow::TypeRef(0)),
        code,
        handlers: Vec::new(),
    };
    let dst = |index| okkama_sparrow::Dst(R(index));
    let program = Program {
        module: "app".into(),
        version: "1.0.0".into(),
        types: vec![okkama_sparrow::Type::Int],
        functions: vec![
            function(
                "main",
                2,
                vec![],
                vec![
                    Const { dst: dst(0), value: okkama_sparrow::Constant::Int("21".into()) },
                    Call { dst: Some(dst(1)), function: okkama_sparrow::FuncRef(1), args: vec![R(0)] },
                    Return { value: Some(R(1)) },
                ],
            ),
            function(
                "double",
                2,
                vec![R(0)],
                vec![IntAdd { dst: dst(1), left: R(0), right: R(0) }, Return { value: Some(R(1)) }],
            ),
        ],
        entry: Some(okkama_sparrow::FuncRef(0)),
        ..Program::default()
    };
    std::fs::write(&file.0, okkama_sparrow::encode(&program).unwrap()).unwrap();
    let output = okkama()
        .args(["exec", file.path()])
        .env("PATH", "")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(42), "{}", stderr(&output));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn help_and_version() {
    for args in [&["--help"][..], &["-h"], &["help"]] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(0));
        assert!(stdout(&output).contains("okkama ast <FILE|->"));
        assert!(stdout(&output).contains("okkama ir <FILE|->"));
        assert!(stdout(&output).contains("okkama build <FILE|-> --target jvm -o <FILE>"));
        assert!(stderr(&output).is_empty());
    }
    let output = run(&["help", "ast"]);
    assert!(stdout(&output).contains("--prune-tests"));
    let output = run(&["ast", "--help"]);
    assert!(stdout(&output).contains("--mode production"));
    let output = run(&["tokens", "--help"]);
    assert!(stdout(&output).contains("okkama tokens"));
    for args in [&["help", "ir"][..], &["ir", "--help"]] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(0));
        assert!(stdout(&output).contains("okkama ir <FILE|->"));
        assert!(stdout(&output).contains("ret"));
        assert!(stderr(&output).is_empty());
    }
    for args in [&["help", "build"][..], &["build", "--help"]] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(0));
        assert!(stdout(&output).contains("--target jvm"));
        assert!(stdout(&output).contains("--output"));
        assert!(stdout(&output).contains("executable JAR"));
        assert!(stderr(&output).is_empty());
    }
    let output = run(&["--version"]);
    assert_eq!(
        stdout(&output),
        format!("okkama {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn no_arguments_prints_help_and_fails() {
    let output = run(&[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("USAGE"));
}

#[test]
fn tokens_text_and_json() {
    let output = run(&["tokens", &example("hello.ok")]);
    assert_eq!(output.status.code(), Some(0));
    let text = stdout(&output);
    assert!(
        text.starts_with("1:1-1:7      MODULE         \"module\"\n"),
        "{text}"
    );
    assert!(
        text.contains("STRING         \"endl\"  string endl \"\\n\""),
        "{text}"
    );
    assert!(text.trim_end().ends_with("EOF            \"\""));

    let output = run(&["tokens", &example("hello.ok"), "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    let json = stdout(&output);
    assert!(json.starts_with("[\n  {\n    \"kind\": \"MODULE\",\n    \"lexeme\": \"module\","));
    assert!(json.contains("\"kind\": \"EOF\""));
}

#[test]
fn ast_formats_spans_and_stdin() {
    let file = TempFile::new("hello.ok", HELLO.as_bytes());
    let tree = run(&["ast", file.path()]);
    assert_eq!(tree.status.code(), Some(0));
    assert!(stdout(&tree).starts_with("Program\n  module: ModuleDecl\n    path: [app]\n"));
    assert!(!stdout(&tree).contains('@'), "no spans without --spans");

    let spans = run(&["ast", file.path(), "--spans"]);
    assert!(stdout(&spans).starts_with("Program @1:1-6:1\n  module: ModuleDecl @1:1-1:19\n"));

    let json = run(&["ast", file.path(), "--format", "json"]);
    assert!(stdout(&json).starts_with("{\n  \"node\": \"Program\",\n  \"module\": {"));
    let json_spans = run(&["ast", file.path(), "--format=json", "--spans"]);
    assert!(stdout(&json_spans).contains("\"span\": {\n    \"start\": 0,"));

    let from_stdin = run_stdin(&["ast", "-"], HELLO.as_bytes());
    assert_eq!(from_stdin.status.code(), Some(0));
    assert_eq!(stdout(&from_stdin), stdout(&tree));
}

#[test]
fn ir_file_and_stdin_include_module_function_and_return() {
    let source = b"module app v1.0.0; main() is int { 1 + 1; }";
    let file = TempFile::new("ir-addition.ok", source);
    let expected = concat!(
        "oir 1 high\n",
        "module app v1.0.0\n",
        "requires core\n",
        "entry @main\n",
        "\n",
        "func @main() -> int [entry] {\n",
        "^b0:\n",
        "    %0: int = const 1\n",
        "    %1: int = const 1\n",
        "    %2: int = int.add %0, %1\n",
        "    ret %2\n",
        "}\n",
    );
    for output in [
        run(&["ir", file.path()]),
        run_stdin(&["ir", "-", "--format=text"], source),
        run(&["ir", "--format", "text", file.path()]),
    ] {
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        assert_eq!(stdout(&output), expected);
        assert!(stderr(&output).is_empty());
    }
}

#[test]
fn ir_keeps_all_statements_and_nested_additions() {
    let output = run_stdin(
        &["ir", "-"],
        b"module app v1.0.0; main() is int { 7; 1 + 2 + 3; }",
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        concat!(
            "oir 1 high\n",
            "module app v1.0.0\n",
            "requires core\n",
            "entry @main\n",
            "\n",
            "func @main() -> int [entry] {\n",
            "^b0:\n",
            "    %0: int = const 7\n",
            "    %1: int = const 1\n",
            "    %2: int = const 2\n",
            "    %3: int = int.add %1, %2\n",
            "    %4: int = const 3\n",
            "    %5: int = int.add %3, %4\n",
            "    ret %5\n",
            "}\n",
        )
    );
    assert!(stderr(&output).is_empty());
}

#[test]
fn ir_explicit_return_is_canonical_and_missing_imports_are_errors() {
    let output = run_stdin(
        &["ir", "-"],
        b"module app/math v1.2.3; import \"missing/math\" take Number; main() is int { 0; }",
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        "<stdin>:1:25: semantic error: Import \"missing/math\" not found\n"
    );
    let output = run_stdin(
        &["ir", "-"],
        b"module app/math v1.2.3; main() is int { return 0x2a; }",
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        concat!(
            "oir 1 high\n",
            "module app/math v1.2.3\n",
            "requires core\n",
            "entry @main\n",
            "\n",
            "func @main() -> int [entry] {\n",
            "^b0:\n",
            "    %0: int = const 42\n",
            "    ret %0\n",
            "}\n",
        )
    );
    assert!(stderr(&output).is_empty());
}

#[test]
fn ir_control_flow_uses_blocks_and_parameters() {
    let output = run_stdin(
        &["ir", "-"],
        b"module app v1.0.0; main() is int { let x = 9223372036854775808; if x > 0 { 1; } else { 2; } }",
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        text.contains("%0: int = const 9223372036854775808\n"),
        "{text}"
    );
    assert!(text.contains("cond %2, ^b1, ^b2\n"), "{text}");
    assert!(text.contains("^b3(%5: int):\n    ret %5\n"), "{text}");
}

#[test]
fn ir_reports_unsupported_and_invalid_code_without_partial_output() {
    for (source, kind, expected) in [
        (
            "module app v1.0.0; main() is int { 1 + missing; }",
            "semantic error",
            "Undefined variable \"missing\"",
        ),
        (
            "module app v1.0.0; main() is int {}",
            "semantic error",
            "must produce a return value",
        ),
        (
            "module app v1.0.0; main() is int { let xs is @any = [1..3, \"a\"]; 0; }",
            "lowering error",
            "ranges inside `@any` literals",
        ),
        (
            "module app v1.0.0; main() is string { 1; }",
            "semantic error",
            "`main() is int`",
        ),
        (
            "module app v1.0.0; object P { P(); } object W { W(); f(p is P) is int { 1; } } async main() is int { await spawn W().f(p: P()); }",
            "semantic error",
            "spawn cannot transfer value of type P",
        ),
        (
            "module app v1.0.0; main() is int { 1; } main() is int { 2; }",
            "semantic error",
            "Duplicate action name",
        ),
    ] {
        let output = run_stdin(&["ir", "-"], source.as_bytes());
        assert_eq!(
            output.status.code(),
            Some(1),
            "{source}: {}",
            stderr(&output)
        );
        assert!(stdout(&output).is_empty(), "{source}");
        assert!(
            stderr(&output).contains(&format!("{kind}:")),
            "{}",
            stderr(&output)
        );
        assert!(stderr(&output).contains(expected), "{}", stderr(&output));
    }
}

#[test]
fn ir_accepts_role_bound_actions_worker_spawn_and_native_handles() {
    for source in [
        "module app v1.0.0; main() is int { 1; } native object Handle;",
        "module app v1.0.0; role R { f() is int; } object O does R { O(); f() is int { 1; } } main() is int { let r is R = O(); let g is () is int = r.f; g(); }",
        "module app v1.0.0; object W { W(); f() is int { 1; } } async main() is int { let x = spawn W().f(); 0; }",
    ] {
        let output = run_stdin(&["ir", "-"], source.as_bytes());
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        assert!(!output.stdout.is_empty());
    }
}

#[test]
fn ir_lowering_error_points_at_the_offending_operand() {
    let output = run_stdin(
        &["ir", "-"],
        b"module app v1.0.0;\nmain() is int {\n    1 + missing;\n}\n",
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).is_empty());
    assert!(
        stderr(&output).starts_with("<stdin>:3:9: semantic error:"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn build_dispatches_file_and_stdin_to_jvm_and_writes_a_class_file() {
    let target = TempFile::absent("build-jvm-output.class");
    let source = example("arithmetic.ok");
    for output in [
        run(&["build", &source, "--target", "jvm", "-o", target.path()]),
        run_stdin(
            &["build", "--target=jvm", "--output", target.path(), "-"],
            HELLO.as_bytes(),
        ),
        run(&[
            "build",
            &source,
            "--target=jvm",
            &format!("--output={}", target.path()),
        ]),
    ] {
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        assert!(stdout(&output).is_empty());
        assert!(stderr(&output).is_empty());
        assert!(
            std::fs::read(&target.0)
                .unwrap()
                .starts_with(&[0xca, 0xfe, 0xba, 0xbe])
        );
    }
}

#[test]
fn failed_build_preserves_an_existing_output_file() {
    let target = TempFile::new("build-preserved.class", b"previous artifact");
    let output = run_stdin(
        &["build", "-", "--target", "jvm", "-o", target.path()],
        b"module app v1.0.0; import \"missing\" take X; main() is int { 1; }",
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("semantic error: Import \"missing\" not found"));
    assert_eq!(std::fs::read(&target.0).unwrap(), b"previous artifact");
}

#[test]
fn build_reports_frontend_errors_before_invoking_the_backend() {
    let target = TempFile::absent("build-source-errors.class");
    for (source, expected) in [
        ("module app v1.0.0; main() is int { 1 2; }", "syntax error"),
        (
            "module app v1.0.0; main() is int { \"open }",
            "lexical error",
        ),
        (
            "module app v1.0.0; main() is int { let xs is @any = [1..3, \"a\"]; 0; }",
            "lowering error",
        ),
        (
            "module app v1.0.0; main() is int { true; }",
            "semantic error",
        ),
    ] {
        let output = run_stdin(
            &["build", "-", "--target", "jvm", "-o", target.path()],
            source.as_bytes(),
        );
        assert_eq!(output.status.code(), Some(1));
        assert!(stdout(&output).is_empty());
        assert!(stderr(&output).contains(expected), "{}", stderr(&output));
        assert!(!stderr(&output).contains("codegen error"));
        assert!(!target.0.exists());
    }
}

#[test]
fn build_option_errors_are_usage_errors() {
    for args in [
        &["build"][..],
        &["build", "input.ok", "-o", "out.class"],
        &["build", "input.ok", "--target", "jvm"],
        &["build", "--target", "jvm", "-o", "out.class"],
        &["build", "input.ok", "--target"],
        &["build", "input.ok", "--target", "jvm", "-o"],
        &["build", "input.ok", "--target", "native", "-o", "out.class"],
        &["build", "input.ok", "--target", "jvm", "--output="],
        // Файлы одного модуля можно перечислить, но не вместе со stdin.
        &[
            "build",
            "input.ok",
            "-",
            "--target",
            "jvm",
            "-o",
            "out.class",
        ],
        &[
            "build",
            "input.ok",
            "--target",
            "jvm",
            "-o",
            "out.class",
            "--spans",
        ],
        &[
            "build",
            "input.ok",
            "--target",
            "jvm",
            "-o",
            "out.class",
            "--format",
            "text",
        ],
        &["ir", "input.ok", "--target", "jvm"],
        &["ast", "input.ok", "-o", "out.class"],
    ] {
        let output = run(args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            stderr(&output)
        );
        assert!(stdout(&output).is_empty(), "{args:?}");
        assert!(stderr(&output).contains("okkama:"), "{args:?}");
    }
}

#[test]
fn build_reports_input_io_errors() {
    let target = TempFile::absent("build-unreadable.class");
    let output = run(&[
        "build",
        "/nonexistent/okkama/input.ok",
        "--target",
        "jvm",
        "-o",
        target.path(),
    ]);
    assert_eq!(output.status.code(), Some(3));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("cannot read"));
    assert!(!target.0.exists());
}

#[test]
fn build_reports_output_write_errors() {
    let parent = TempFile::absent("build-missing-parent");
    let path = parent.0.join("out.class");
    let output = run_stdin(
        &[
            "build",
            "-",
            "--target",
            "jvm",
            "-o",
            path.to_str().unwrap(),
        ],
        HELLO.as_bytes(),
    );
    assert_eq!(output.status.code(), Some(3));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("cannot write"));
    assert!(!parent.0.exists());
}

#[test]
fn production_mode_and_pruning() {
    let path = example("tests-and-fakes.ok");
    let full = stdout(&run(&["ast", &path]));
    assert!(
        full.contains("TestDecl") && full.contains("FakeBody") && !full.contains("SkippedBody")
    );
    let production = stdout(&run(&["ast", &path, "--mode", "production"]));
    assert!(production.contains("TestDecl") && production.contains("SkippedBody"));
    let pruned = stdout(&run(&[
        "ast",
        &path,
        "--mode",
        "production",
        "--prune-tests",
    ]));
    assert!(!pruned.contains("TestDecl") && !pruned.contains("FakeDecl"));
    let pruned_full = stdout(&run(&["ast", &path, "--prune-tests"]));
    assert_eq!(pruned, pruned_full, "pruning makes both modes equal");
}

#[test]
fn syntax_errors_exit_one_with_nothing_on_stdout() {
    let file = TempFile::new(
        "bad.ok",
        b"module app v1.0.0;\nmain() is int {\n    let x = 1 2;\n}\n",
    );
    for command in ["ast", "tokens", "ir"] {
        let output = run(&[command, file.path()]);
        if command == "tokens" {
            // The token stream itself is valid.
            assert_eq!(output.status.code(), Some(0));
            continue;
        }
        assert_eq!(output.status.code(), Some(1));
        assert!(stdout(&output).is_empty());
        let message = stderr(&output);
        assert!(
            message.starts_with(&format!(
                "{}:3:15: syntax error at \"2\": Expect \";\" after let declaration.\n",
                file.path()
            )),
            "{message}"
        );
        assert!(message.contains("3 |     let x = 1 2;\n"), "{message}");
    }
}

#[test]
fn lexical_errors_exit_one_for_both_commands() {
    let file = TempFile::new("lex.ok", b"module app v1.0.0; main() is int { \"open }\n");
    for command in ["ast", "tokens", "ir"] {
        let output = run(&[command, file.path()]);
        assert_eq!(output.status.code(), Some(1), "{command}");
        assert!(stdout(&output).is_empty(), "{command}");
        assert!(
            stderr(&output).contains("lexical error: Unterminated string."),
            "{command}"
        );
    }
}

#[test]
fn invalid_utf8_is_a_source_error() {
    let file = TempFile::new("latin1.ok", b"module app v1.0.0; // caf\xe9\n");
    for command in ["ast", "ir"] {
        let output = run(&[command, file.path()]);
        assert_eq!(output.status.code(), Some(1));
        assert!(stdout(&output).is_empty());
        assert!(stderr(&output).contains("not valid UTF-8 (invalid byte at offset 25)"));
    }
}

#[test]
fn unreadable_input_exits_three() {
    for command in ["ast", "ir"] {
        let output = run(&[command, "/nonexistent/okkama/input.ok"]);
        assert_eq!(output.status.code(), Some(3));
        assert!(stdout(&output).is_empty());
        assert!(stderr(&output).contains("cannot read `/nonexistent/okkama/input.ok`"));
    }
}

#[test]
fn usage_errors_exit_two() {
    for args in [
        &["ast"][..],
        &["ast", "a.ok", "b.ok"],
        &["ast", "a.ok", "--format", "xml"],
        &["ast", "a.ok", "--mode", "debug"],
        &["ast", "a.ok", "--format"],
        &["tokens", "a.ok", "--spans"],
        &["tokens", "a.ok", "--format", "tree"],
        &["ast", "a.ok", "--colour"],
        &["ir"],
        &["ir", "-", "b.ok"],
        &["ir", "a.ok", "--format", "json"],
        &["ir", "a.ok", "--format"],
        &["ir", "a.ok", "--spans"],
        &["ir", "a.ok", "--mode", "production"],
        &["run", "--frobnicate", "a.ok"],
        &["compile", "a.ok"],
        &["jit", "a.ok"],
    ] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(stdout(&output).is_empty(), "{args:?}");
        assert!(stderr(&output).contains("okkama:"), "{args:?}");
    }
}

#[test]
fn every_project_example_parses() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut count = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "ok") {
            count += 1;
            let output = run(&["ast", path.to_str().unwrap(), "--format", "json"]);
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}: {}",
                path.display(),
                stderr(&output)
            );
        }
    }
    assert!(count >= 5);
}

#[test]
fn check_reports_warnings_without_failing() {
    let source = "module app v1.0.0; main() is int { let n = 2; for 0..n -> index { index; } }";
    let output = run_stdin(&["check", "-"], source.as_bytes());
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).is_empty());
    assert_eq!(
        stderr(&output),
        "<stdin>:1:47: warning: For loop value is undefined when its source is empty: the body may not run\n"
    );
    let output = run_stdin(
        &["check", "-"],
        b"module app v1.0.0; main() is int { let a = 1 + true; let b = missing; 0; }",
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        "<stdin>:1:44: semantic error: Operator + cannot be applied to int and bool\n\
         <stdin>:1:62: semantic error: Undefined variable \"missing\"\n"
    );
}

#[test]
fn several_files_form_one_module() {
    // §5.1: файлы — одно пространство объявлений; ошибка — у своего файла.
    let main = TempFile::new(
        "multi-main.ok",
        b"module app v1.0.0;\nmain() is int { Greeter().code(); }\n",
    );
    let part = TempFile::new(
        "multi-part.ok",
        b"module app v1.0.0;\nobject Greeter { Greeter(); code() is int { 7; } }\n",
    );
    let output = run(&["check", main.path(), part.path()]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let output = run(&["ir", main.path(), part.path()]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("object Greeter {"));
    let other = TempFile::new(
        "multi-other.ok",
        b"module app v1.0.1;\nobject Greeter { Greeter(); }\nmain() is int { 1; }\n",
    );
    let output = run(&["check", main.path(), other.path()]);
    assert_eq!(output.status.code(), Some(1));
    let text = stderr(&output);
    assert!(
        text.contains(&format!(
            "{}:1:1: semantic error: module \"app\" has conflicting versions \"v1.0.0\" and \"v1.0.1\"",
            other.path()
        )),
        "{text}"
    );
    assert!(
        text.contains("semantic error: Module declares main() more than once"),
        "{text}"
    );
}
