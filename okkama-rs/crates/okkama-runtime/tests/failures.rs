use okkama_compiler::{SourceFile, codegen, ir, lowering, syntax};
use okkama_runtime::{RuntimeError, execute, sparrow};

fn lower(body: &str) -> ir::Module {
    let source = SourceFile::new("failures.ok", format!("module app v1.0.0; object Worker {{ Worker(); fail(message is string) is int {{ panic message; }} }} main() is int {{ {body} }}")).unwrap();
    lowering::lower_program(&syntax::parse(&source, &syntax::ParseOptions::default()).unwrap())
        .unwrap()
}

#[test]
fn panic_preserves_computed_unicode_message_across_calls() {
    let module = lower("Worker().fail(\"ошибка: \" + \"42\");");
    assert!(
        matches!(execute(&module), Err(RuntimeError::Panic { message, .. }) if message == "ошибка: 42")
    );
    let bytes = okkama_sparrow::encode(&codegen::sparrow::generate(&module).unwrap()).unwrap();
    assert!(
        matches!(sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap()), Err(sparrow::Error::Operation { error: RuntimeError::Panic { message, .. }, .. }) if message == "ошибка: 42")
    );
}

#[test]
fn untaken_panic_is_lazy_and_bytecode_message_must_be_a_string() {
    let module = lower("if false { Worker().fail(\"unused\"); } else { 42; }");
    assert_eq!(execute(&module).unwrap().to_string(), "42");
    let program = codegen::sparrow::generate(&module).unwrap();
    let bytes = okkama_sparrow::encode(&program).unwrap();
    assert_eq!(
        sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap())
            .unwrap()
            .to_string(),
        "42"
    );
    // Проверка SPB 9 структурна: тип операнда проверяет исполнение — не
    // строка в `panic` даёт ошибку состояния VM, а не отказ программы.
    let module = lower("panic \"stop\";");
    let mut invalid = codegen::sparrow::generate(&module).unwrap();
    let main = invalid.entry.unwrap().0 as usize;
    let load = invalid.functions[main]
        .code
        .iter_mut()
        .find(|inst| matches!(inst, okkama_sparrow::Instruction::Const { value: okkama_sparrow::Constant::Str(_), .. }))
        .unwrap();
    let okkama_sparrow::Instruction::Const { value, .. } = load else {
        unreachable!()
    };
    *value = okkama_sparrow::Constant::Bool(false);
    assert!(invalid.verify().is_ok());
    assert!(matches!(
        sparrow::execute(&invalid),
        Err(sparrow::Error::Operation { error: RuntimeError::InvalidState { .. }, .. })
    ));
}

#[test]
fn ir_unwind_runs_defer_in_reverse_order_and_keeps_primary_failure() {
    let module = lower(
        "defer Worker().fail(\"first\"); defer Worker().fail(\"second\"); Worker().fail(\"primary\");",
    );
    let error = execute(&module).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Panic: primary\nSecondary cleanup failure:\n  Panic: second\nSecondary cleanup failure:\n  Panic: first"
    );
}

#[test]
fn ir_program_errors_unwind_and_panic_bypasses_error_catches() {
    let module = lower("defer Worker().fail(\"cleanup\"); 7 % 0;");
    let error = execute(&module).unwrap_err();
    assert_eq!(
        error.to_string(),
        "RuntimeError: Modulo by zero\nSecondary cleanup failure:\n  Panic: cleanup"
    );
    let module = lower("try { Worker().fail(\"primary\"); } catch e is Error { 42; }");
    assert!(
        matches!(execute(&module), Err(RuntimeError::Panic { message, .. }) if message == "primary")
    );
    let module = lower("try { 7 % 0; } catch e is Error { 42; }");
    assert!(
        matches!(execute(&module), Err(RuntimeError::Trap { message, .. }) if message == "Modulo by zero")
    );
}

fn typed(body: &str) -> ir::Module {
    let source = SourceFile::new("typed.ok", format!("module app v1.0.0; object Missing does Error {{ has code is int; Missing(:code); text() is string {{ \"missing \" + code.string(); }} }} object Denied does Error {{ Denied(); text() is string {{ \"denied\"; }} }} object Bomb does Error {{ Bomb(); text() is string {{ panic \"text must stay lazy\"; }} }} object Sender {{ Sender(); fail() is int {{ throw Missing(code: 42); }} }} main() is int {{ {body} }}")).unwrap();
    lowering::lower_program(&syntax::parse(&source, &syntax::ParseOptions::default()).unwrap())
        .unwrap()
}

#[test]
fn ir_typed_failure_matches_nominal_catches_and_role_references() {
    for body in [
        "try { Sender().fail(); } catch e is Denied { 1; } catch e is Missing { if e.text() == \"missing 42\" { 42; } else { 2; } }",
        "let problem is Error = Missing(code: 42); try { throw problem; 0; } catch e is Missing { 42; }",
        "try { throw Bomb(); 0; } catch e is Error { 42; } catch e is Bomb { 1; }",
        "try { try { throw Missing(code: 42); 0; } catch e is Denied { 1; } } catch e is Missing { 42; }",
        "try { try { throw Missing(code: 42); 0; } catch e is Missing { throw; 1; } } catch e is Missing { 42; }",
    ] {
        assert_eq!(execute(&typed(body)).unwrap().to_string(), "42", "{body}");
        assert_eq!(run_sparrow(&typed(body)).unwrap().to_string(), "42", "{body}");
    }
}

#[test]
fn ir_uncaught_typed_failure_formats_text_only_at_the_boundary() {
    let error = execute(&typed("Sender().fail();")).unwrap_err();
    assert_eq!(error.to_string(), "Missing: missing 42");
    let module = typed("defer Sender().fail(); panic \"primary\";");
    assert_eq!(
        execute(&module).unwrap_err().to_string(),
        "Panic: primary\nSecondary cleanup failure:\n  Missing: missing 42"
    );
}

#[test]
fn ir_new_failure_keeps_handled_failure_as_its_cause() {
    let error = execute(&typed(
        "try { throw Missing(code: 42); 0; } catch e is Missing { throw Denied(); 0; }",
    ))
    .unwrap_err();
    let RuntimeError::Unwind { failure, .. } = error else {
        panic!("expected typed failure");
    };
    assert_eq!(failure.to_string(), "Denied: denied");
    assert_eq!(failure.category(), "uncaught-error");
    assert!(
        matches!(&failure.cause().unwrap().kind, okkama_runtime::FailureKind::Thrown { name, .. } if name == "Missing")
    );
}

#[test]
fn sparrow_unwind_preserves_primary_and_cleanup_failures_across_calls() {
    for body in [
        "defer Worker().fail(\"first\"); defer Worker().fail(\"second\"); Worker().fail(\"primary\");",
        "defer Worker().fail(\"cleanup\"); 7 % 0;",
        "defer 1 + 2; Worker().fail(\"primary\");",
    ] {
        let module = lower(body);
        let expected = execute(&module).unwrap_err().to_string();
        let program = codegen::sparrow::generate(&module).unwrap();
        let bytes = okkama_sparrow::encode(&program).unwrap();
        assert_eq!(&bytes[4..6], &okkama_sparrow::VERSION.to_be_bytes());
        let decoded = okkama_sparrow::decode(&bytes).unwrap();
        let Err(sparrow::Error::Operation { error, .. }) = sparrow::execute(&decoded) else {
            panic!("expected failure");
        };
        assert_eq!(error.to_string(), expected, "{body}");
    }
}

fn run_sparrow(module: &ir::Module) -> Result<ir::IntValue, RuntimeError> {
    let program = codegen::sparrow::generate(module).unwrap();
    let bytes = okkama_sparrow::encode(&program).unwrap();
    match sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap()) {
        Ok(value) => Ok(value),
        Err(sparrow::Error::Operation { error, .. }) => Err(error),
        Err(error) => panic!("{error}"),
    }
}
#[test]
fn sparrow_typed_diagnostics_causes_and_uncatchable_failures_match_ir() {
    for body in [
        "Sender().fail();",
        "defer Sender().fail(); panic \"primary\";",
        "try { throw Missing(code: 42); 0; } catch e is Missing { throw Denied(); 0; }",
        "try { panic \"primary\"; } catch e is Error { 42; }",
        "try { 7 % 0; } catch e is Error { 42; }",
    ] {
        let module = typed(body);
        assert_eq!(
            run_sparrow(&module).unwrap_err().to_string(),
            execute(&module).unwrap_err().to_string()
        );
    }
}
