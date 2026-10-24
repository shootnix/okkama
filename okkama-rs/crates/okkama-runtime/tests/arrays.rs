//! Неизменяемые массивы: создание, размер, индекс и перенос через вызовы.
use okkama_compiler::{SourceFile, ir, lowering, syntax};
use okkama_runtime::{RuntimeError, execute as execute_ir};

fn execute(module: &ir::Module) -> Result<ir::IntValue, RuntimeError> {
    let result = execute_ir(module);
    let program = okkama_compiler::codegen::sparrow::generate(module).unwrap();
    let bytes = okkama_sparrow::encode(&program).unwrap();
    let actual = okkama_runtime::sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap());
    match (&result, actual) {
        (Ok(expected), Ok(actual)) => assert_eq!(*expected, actual),
        (
            Err(RuntimeError::Trap { message, .. }),
            Err(okkama_runtime::sparrow::Error::Operation {
                error:
                    RuntimeError::Trap {
                        message: actual, ..
                    },
                ..
            }),
        ) => assert_eq!(*message, actual),
        (_, actual) => panic!("IR {result:?} differs from Sparrow {actual:?}"),
    }
    result
}

fn lower(decls: &str, body: &str) -> ir::Module {
    let source = SourceFile::new(
        "arrays.ok",
        format!("module app v1.0.0; {decls} main() is int {{ {body} }}"),
    )
    .unwrap();
    let ast = syntax::parse(&source, &syntax::ParseOptions::default()).unwrap();
    lowering::lower_program(&ast).unwrap()
}

#[test]
fn slices_preserve_elements_aliases_and_exact_bounds() {
    for (body, expected) in [
        (
            "let xs = [10, 20, 22]; let ys = xs.slice(1, 3); ys[0] + ys[1];",
            "42",
        ),
        ("let xs = [[42], [7]]; xs.slice(0, 1)[0][0];", "42"),
        ("let xs = [1, 2]; xs.slice(2, 2).size() + xs.size();", "2"),
        ("let xs is @int = []; xs.slice(0, 0).size();", "0"),
    ] {
        assert_eq!(execute(&lower("", body)).unwrap().to_string(), expected);
    }
    for (start, end, expected) in [
        ("-1", "0", "Slice start must not be negative"),
        (
            "2",
            "1",
            "Slice start must be less than or equal to slice end",
        ),
        (
            "0",
            "99999999999999999999999999",
            "Slice end must not exceed length",
        ),
    ] {
        let result = execute(&lower(
            "",
            &format!("let xs = [1]; xs.slice({start}, {end}).size();"),
        ));
        assert!(matches!(result, Err(RuntimeError::Trap { message, .. }) if message == expected));
    }
}

#[test]
fn literals_sizes_and_indices_work_for_supported_element_types() {
    for (body, expected) in [
        ("let xs = [10, 20, 30]; xs[0] + xs[xs.size() - 1];", "40"),
        ("let xs is @int = []; xs.size();", "0"),
        (
            r#"let xs = ["Привет", "🙂"]; xs[1].length() + xs.size();"#,
            "3",
        ),
        ("let xs = [false, true]; if xs[1] { 42; } else { 0; }", "42"),
        ("let xs = [[1, 2], [40]]; xs[0][1] + xs[1][0];", "42"),
    ] {
        assert_eq!(
            execute(&lower("", body)).unwrap().to_string(),
            expected,
            "{body}"
        );
    }
}

#[test]
fn arrays_survive_frames_and_pass_through_generic_fields() {
    let module = lower(
        r#"
        object Box<T> { has value is T; Box(:value); get() is T { value; } }
        object Factory {
            Factory();
            make(n is int) is @int { [n, n + 1]; }
            size(xs is @int) is int { xs.size(); }
        }
    "#,
        r#"
        let f = Factory();
        let xs = f.make(39);
        let box = Box(value: xs);
        box.get()[1] + f.size(xs);
    "#,
    );
    assert_eq!(execute(&module).unwrap().to_string(), "42");
    assert_eq!(execute(&module).unwrap().to_string(), "42");
}

#[test]
fn object_elements_keep_old_values_after_successor_rebinding() {
    let module = lower(
        r#"
        object Counter {
            has n is int; Counter(:n);
            inc() is ^Counter { ^n = n + 1; }
            get() is int { n; }
        }
    "#,
        r#"
        let c = Counter(n: 20);
        let xs = [c];
        let alias = xs;
        c = c.inc();
        if xs[0].get() == 20 && alias[0].get() == 20 && c.get() == 21 { 42; } else { 0; }
    "#,
    );
    assert_eq!(execute(&module).unwrap().to_string(), "42");
}

#[test]
fn empty_array_branches_receive_the_expected_type() {
    for flag in ["true", "false"] {
        let module = lower(
            "",
            &format!(
                "let flag = {flag}; let xs is @int = if flag {{ []; }} else {{ [1, 2]; }}; xs.size();"
            ),
        );
        assert_eq!(
            execute(&module).unwrap().to_string(),
            if flag == "true" { "0" } else { "2" }
        );
    }
}

#[test]
fn invalid_indices_report_a_program_error_at_the_index_expression() {
    for body in [
        "let xs is @int = []; xs[0];",
        "let xs = [1]; xs[-1];",
        "let xs = [1]; xs[1];",
        "let xs = [1]; xs[999999999999999999999999999999999999999999];",
        "let xs = [1]; xs[-999999999999999999999999999999999999999999];",
    ] {
        let module = lower("", body);
        let span = module
            .functions
            .iter()
            .flat_map(|f| &f.blocks)
            .flat_map(|b| &b.insts)
            .find(|inst| matches!(inst.kind, ir::InstKind::ArrayGet { .. }))
            .unwrap()
            .span;
        assert_eq!(
            execute(&module),
            Err(RuntimeError::Trap {
                span,
                message: "Array index out of bounds".into()
            }),
            "{body}"
        );
    }
}

#[test]
fn lookup_uses_fallback_only_for_missing_indices() {
    for (body, expected) in [
        ("let xs = [42]; xs[0] else 99;", "42"),
        ("let xs = [0]; xs[0] else 99;", "0"),
        ("let xs is @int = []; xs[0] else 42;", "42"),
        ("let xs = [1]; xs[-1] else 42;", "42"),
        ("let xs = [1]; xs[1] else 42;", "42"),
        (
            "let xs = [1]; xs[999999999999999999999999999999999999999] else 42;",
            "42",
        ),
        (
            "let xs = [1]; xs[-999999999999999999999999999999999999999] else 42;",
            "42",
        ),
        (
            "let xs = [false]; let b = xs[0] else true; if b { 0; } else { 42; }",
            "42",
        ),
        (
            r#"let xs = [""]; let s = xs[0] else "fallback"; s.length();"#,
            "0",
        ),
    ] {
        assert_eq!(
            execute(&lower("", body)).unwrap().to_string(),
            expected,
            "{body}"
        );
    }
}

#[test]
fn fallback_is_lazy_and_propagates_errors_when_selected() {
    let decls = r#"
        object Reader {
            Reader();
            fail() is int { let empty is @int = []; empty[0]; }
            answer() is int { 40 + 2; }
        }
    "#;
    let present = lower(decls, "let xs = [42]; xs[0] else Reader().fail();");
    assert_eq!(execute(&present).unwrap().to_string(), "42");
    let missing = lower(decls, "let xs is @int = []; xs[0] else Reader().fail();");
    assert!(
        matches!(execute(&missing), Err(RuntimeError::Trap { message, .. }) if message == "Array index out of bounds")
    );
    let fallback = lower(decls, "let xs is @int = []; xs[0] else Reader().answer();");
    assert_eq!(execute(&fallback).unwrap().to_string(), "42");
}

#[test]
fn found_payload_preserves_nested_arrays_and_objects() {
    assert_eq!(
        execute(&lower(
            "",
            "let xs = [[40, 2]]; let row = xs[0] else []; row[0] + row[1];"
        ))
        .unwrap()
        .to_string(),
        "42"
    );
    let decls = r#"
        object Number {
            has n is int; Number(:n);
            get() is int { n; }
        }
    "#;
    for index in [0, 1] {
        let module = lower(
            decls,
            &format!("let xs = [Number(n: 42)]; let n = xs[{index}] else Number(n: 7); n.get();"),
        );
        assert_eq!(
            execute(&module).unwrap().to_string(),
            if index == 0 { "42" } else { "7" }
        );
    }
}

#[test]
fn lookup_continues_with_the_correct_values_in_a_loop() {
    let module = lower(
        "",
        "let xs = [10, 20]; for 0..=3 -> i { let n = xs[i] else i + 100; n + 1; }",
    );
    assert_eq!(execute(&module).unwrap().to_string(), "104");
}
