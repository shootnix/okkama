#[path = "sparrow_support/mod.rs"]
mod support;
use okkama_compiler::{SourceFile, codegen, ir, lowering, syntax};

fn lower(body: &str) -> ir::Module {
    let source = SourceFile::new("failures.ok", format!("module app v1.0.0; object Worker {{ Worker(); fail(message is string) is int {{ panic message; }} }} main() is int {{ {body} }}")).unwrap();
    lowering::lower_program(&syntax::parse(&source, &syntax::ParseOptions::default()).unwrap())
        .unwrap()
}

#[test]
fn panic_preserves_computed_unicode_message_across_calls() {
    let module = lower("Worker().fail(\"ошибка: \" + \"42\");");

    support::assert_panic(&module, "ошибка: 42");
}

#[test]
fn untaken_panic_is_lazy_and_bytecode_message_must_be_a_string() {
    let module = lower("if false { Worker().fail(\"unused\"); } else { 42; }");

    support::assert_value(&module, "42");
    // Проверка SPB 9 типизирована: не строка в `panic` отвергается до
    // исполнения (#16).
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
    let error = invalid.verify().unwrap_err().to_string();
    assert!(error.contains("str cannot hold the bool result"), "{error}");
}

#[test]
fn unwind_runs_defer_in_reverse_order_and_keeps_primary_failure() {
    let module = lower(
        "defer Worker().fail(\"first\"); defer Worker().fail(\"second\"); Worker().fail(\"primary\");",
    );
    let error = support::report(&module);
    assert_eq!(
        error.to_string(),
        "Panic: primary\nSecondary cleanup failure:\n  Panic: second\nSecondary cleanup failure:\n  Panic: first"
    );
}

#[test]
fn program_errors_unwind_and_panic_bypasses_error_catches() {
    let module = lower("defer Worker().fail(\"cleanup\"); 7 % 0;");
    let error = support::report(&module);
    assert_eq!(
        error.to_string(),
        "RuntimeError: Modulo by zero\nSecondary cleanup failure:\n  Panic: cleanup"
    );
    let module = lower("try { Worker().fail(\"primary\"); } catch e is Error { 42; }");
    support::assert_panic(&module, "primary");

    let module = lower("try { 7 % 0; } catch e is Error { 42; }");
    support::assert_trap(&module, "Modulo by zero");

}

fn typed(body: &str) -> ir::Module {
    let source = SourceFile::new("typed.ok", format!("module app v1.0.0; enum Missing is Error = (code(value is int)); enum Denied is Error = (denied); object Sender {{ Sender(); fail() is int {{ throw Missing.code(42); }} }} main() is int {{ {body} }}")).unwrap();
    lowering::lower_program(&syntax::parse(&source, &syntax::ParseOptions::default()).unwrap())
        .unwrap()
}

#[test]
fn typed_failure_matches_nominal_catches_and_role_references() {
    for body in [
        "try { Sender().fail(); } catch e is Denied { 1; } catch e is Missing { if e.text() == \"Missing.code(42)\" { 42; } else { 2; } }",
        "let problem is Error = Missing.code(42); try { throw problem; 0; } catch e is Missing { 42; }",
        "try { throw Denied.denied; 0; } catch e is Error { 42; } catch e is Denied { 1; }",
        "try { try { throw Missing.code(42); 0; } catch e is Denied { 1; } } catch e is Missing { 42; }",
        "try { try { throw Missing.code(42); 0; } catch e is Missing { throw; 1; } } catch e is Missing { 42; }",
    ] {

        support::assert_value(&typed(body), "42");
    }
}

#[test]
fn uncaught_typed_failure_formats_text_only_at_the_boundary() {
    let error = support::report(&typed("Sender().fail();"));
    assert_eq!(error.to_string(), "Missing: Missing.code(42)");
    let module = typed("defer Sender().fail(); panic \"primary\";");
    assert_eq!(
        support::report(&module),
        "Panic: primary\nSecondary cleanup failure:\n  Missing: Missing.code(42)"
    );
}

#[test]
fn new_failure_keeps_handled_failure_as_its_cause() {
    let module = typed("try { throw Missing.code(42); 0; } catch e is Missing { throw Denied.denied; 0; }");
    assert_eq!(support::report(&module), "Denied: Denied.denied\nCaused by:\n  Missing: Missing.code(42)");
}

#[test]
fn sparrow_unwind_preserves_primary_and_cleanup_failures_across_calls() {
    for (body, expected) in [
        ("defer Worker().fail(\"first\"); defer Worker().fail(\"second\"); Worker().fail(\"primary\");", "Panic: primary\nSecondary cleanup failure:\n  Panic: second\nSecondary cleanup failure:\n  Panic: first"),
        ("defer Worker().fail(\"cleanup\"); 7 % 0;", "RuntimeError: Modulo by zero\nSecondary cleanup failure:\n  Panic: cleanup"),
        ("defer 1 + 2; Worker().fail(\"primary\");", "Panic: primary"),
    ] {
        let module = lower(body);
        assert_eq!(support::report(&module), expected, "{body}");
    }
}

#[test]
fn typed_diagnostics_and_uncatchable_failures() {
    for (body, expected) in [
        ("Sender().fail();", "Missing: Missing.code(42)"),
        ("defer Sender().fail(); panic \"primary\";", "Panic: primary\nSecondary cleanup failure:\n  Missing: Missing.code(42)"),
        ("try { panic \"primary\"; } catch e is Error { 42; }", "Panic: primary"),
        ("try { 7 % 0; } catch e is Error { 42; }", "RuntimeError: Modulo by zero"),
    ] {
        assert_eq!(support::report(&typed(body)), expected, "{body}");
    }
}
