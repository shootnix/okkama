//! Неизменяемые массивы: создание, размер, индекс и перенос через вызовы.
#[path = "sparrow_support/mod.rs"]
mod support;
use okkama_compiler::{SourceFile, ir, lowering, syntax};
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
        support::assert_value(&lower("", body), expected);
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
        let result = support::sparrow(&lower(
            "",
            &format!("let xs = [1]; xs.slice({start}, {end}).size();"),
        ));
        assert!(matches!(result, support::Ending::Failure { message, .. } if message == expected));
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
        support::assert_value(&lower("", body), expected);
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
    support::assert_value(&module, "42");
    support::assert_value(&module, "42");
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
    support::assert_value(&module, "42");
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
        support::assert_value(&module, if flag == "true" { "0" } else { "2" });
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
        support::assert_trap(&module, "Array index out of bounds");
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
        support::assert_value(&lower("", body), expected);
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
    support::assert_value(&present, "42");
    let missing = lower(decls, "let xs is @int = []; xs[0] else Reader().fail();");
    assert!(
        matches!(support::sparrow(&missing), support::Ending::Failure { message, .. } if message == "Array index out of bounds")
    );
    let fallback = lower(decls, "let xs is @int = []; xs[0] else Reader().answer();");
    support::assert_value(&fallback, "42");
}

#[test]
fn found_payload_preserves_nested_arrays_and_objects() {
    support::assert_value(&lower(
            "",
            "let xs = [[40, 2]]; let row = xs[0] else []; row[0] + row[1];"
        ), "42");
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
        support::assert_value(&module, if index == 0 { "42" } else { "7" });
    }
}

#[test]
fn lookup_continues_with_the_correct_values_in_a_loop() {
    let module = lower(
        "",
        "let xs = [10, 20]; for 0..=3 -> i { let n = xs[i] else i + 100; n + 1; }",
    );
    support::assert_value(&module, "104");
}
