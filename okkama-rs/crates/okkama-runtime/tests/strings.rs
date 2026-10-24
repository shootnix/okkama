//! Строки: исходник → анализ → OIR → исполнение внутри Rust.
use okkama_compiler::{SourceFile, lowering, syntax};
use okkama_runtime::{RuntimeError, execute};

fn run(declarations: &str, body: &str) -> String {
    let source = SourceFile::new(
        "strings.ok",
        format!("module app v1.0.0; {declarations} main() is int {{ {body} }}"),
    )
    .unwrap();
    let ast = syntax::parse(&source, &syntax::ParseOptions::default()).unwrap();
    let module = lowering::lower_program(&ast).unwrap();
    let result = execute(&module).unwrap();
    {
        let program = okkama_compiler::codegen::sparrow::generate(&module).unwrap();
        let bytes = okkama_sparrow::encode(&program).unwrap();
        assert_eq!(
            okkama_runtime::sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap()).unwrap(),
            result
        );
    }
    result.to_string().to_owned()
}

#[test]
fn split_and_join_preserve_empty_fields_and_unicode_code_points() {
    for body in [
        r#"if "".split(",").size() == 1 && "".split("").size() == 0 { 42; } else { 0; }"#,
        r#"if "a,,b,".split(",").join("|") == "a||b|" { 42; } else { 0; }"#,
        r#"if "🙂é".split("").join("|") == "🙂|e|́" { 42; } else { 0; }"#,
        r#"if "aaa".split("aa").join("|") == "|a" { 42; } else { 0; }"#,
        r#"let empty is @string = []; if empty.join(",") == "" { 42; } else { 0; }"#,
        r#"let parts = "a--b".split("--"); if parts[0] == "a" && parts.join("+") == "a+b" { 42; } else { 0; }"#,
    ] {
        assert_eq!(run("", body), "42", "{body}");
    }
}

#[test]
fn trim_chomp_and_literal_replacement_match_language_boundaries() {
    for body in [
        r#"if " \t\n\r  x y　  ".trim() == "x y" { 42; } else { 0; }"#,
        "if \"\u{200b}x\u{200b}\".trim().length() == 3 { 42; } else { 0; }",
        r#"if "a\r\n".chomp() == "a" && "a\n\n".chomp() == "a\n" && "a\r".chomp() == "a\r" { 42; } else { 0; }"#,
        r#"if "".chomp() == "" && "\r\n".chomp() == "" && "no".chomp() == "no" { 42; } else { 0; }"#,
        r#"if "ab".replace("", "-") == "-a-b-" && "ab".replace_first("", "-") == "-ab" { 42; } else { 0; }"#,
        r#"if "aaaa".replace("aa", "b") == "bb" && "aaa".replace("aa", "b") == "ba" { 42; } else { 0; }"#,
        r#"if "🙂Я".replace("", "-") == "-🙂-Я-" && "banana".replace_first("na", "NA") == "baNAna" { 42; } else { 0; }"#,
        r#"if "a.b".replace(".", "$") == "a$b" && "abc".replace("x", "y") == "abc" { 42; } else { 0; }"#,
    ] {
        assert_eq!(run("", body), "42", "{body}");
    }
}

#[test]
fn literals_concat_and_equality_preserve_unicode_and_empty_strings() {
    for body in [
        r#"if "" + "" == "" { 42; } else { 0; }"#,
        r#"if "hello" + "!" == "hello!" { 42; } else { 0; }"#,
        r#"if "Привет, " + "🌍" == "Привет, 🌍" { 42; } else { 0; }"#,
        r#"if "" + "🙂" == "🙂" + "" { 42; } else { 0; }"#,
        r#"if "a" != "ab" { 42; } else { 0; }"#,
        r#"if "a" == "b" { 0; } else { 42; }"#,
        r#"if "a" != "a" { 0; } else { 42; }"#,
        r#"if "\"" + "\n" == "\"\n" { 42; } else { 0; }"#,
    ] {
        assert_eq!(run("", body), "42", "{body}");
    }
}

#[test]
fn concatenation_preserves_the_original_and_its_alias() {
    assert_eq!(
        run(
            "",
            r#"
        let original = "hello";
        let alias = original;
        let joined = original + "!";
        if original == "hello" && alias == "hello" && joined == "hello!" { 42; } else { 0; }
    "#
        ),
        "42"
    );
}

#[test]
fn strings_pass_through_fields_calls_and_successors() {
    assert_eq!(
        run(
            r#"
        object Greeting {
            has text is string = "Привет";
            Greeting() {}
            get() is string { text; }
            append(suffix is string) is ^Greeting { ^text = text + suffix; }
            say(suffix is string) is string { self.get() + suffix; }
        }
    "#,
            r#"
        let old = Greeting();
        let new = old.append(", мир");
        let text = new.say("!");
        if text == "Привет, мир!" && old.get() == "Привет" { 42; } else { 0; }
    "#
        ),
        "42"
    );
}

#[test]
fn strings_cross_branch_parameters_and_generic_return_types() {
    assert_eq!(
        run(
            r#"
        object Box<T> { has value is T; Box(:value); get() is T { value; } }
    "#,
            r#"
        let flag = 1 < 2;
        let text = if flag { "left"; } else { "right"; };
        let box = Box(value: text + "!");
        if box.get() == "left!" { 42; } else { 0; }
    "#
        ),
        "42"
    );
}

#[test]
fn length_counts_code_points_not_utf8_bytes_or_graphemes() {
    for (text, length) in [
        ("", "0"),
        ("abc", "3"),
        ("Привет", "6"),
        ("🙂🌍", "2"),
        ("e\u{301}", "2"),
        ("👩‍💻", "3"),
    ] {
        assert_eq!(run("", &format!("\"{text}\".length();")), length);
    }
}

#[test]
fn slices_use_exclusive_code_point_boundaries() {
    for body in [
        r#"if "А🙂éЯ".slice(1, 2) == "🙂" { 42; } else { 0; }"#,
        r#"if "А🙂éЯ".slice(2, 4) == "é" { 42; } else { 0; }"#,
        r#"if "А🙂".slice(0, 2) == "А🙂" { 42; } else { 0; }"#,
        r#"if "А🙂".slice(2, 2) == "" { 42; } else { 0; }"#,
        r#"if "А🙂".slice(1, 1) == "" { 42; } else { 0; }"#,
        r#"if "".slice(0, 0) == "" { 42; } else { 0; }"#,
    ] {
        assert_eq!(run("", body), "42", "{body}");
    }
}

#[test]
fn invalid_slice_bounds_return_program_errors_with_source_spans() {
    for (start, end, message) in [
        ("-1", "0", "Slice start must not be negative"),
        ("-1", "-2", "Slice start must not be negative"),
        (
            "0",
            "-1",
            "Slice start must be less than or equal to slice end",
        ),
        (
            "2",
            "1",
            "Slice start must be less than or equal to slice end",
        ),
        ("0", "3", "Slice end must not exceed length"),
        (
            "999999999999999999999999999999",
            "0",
            "Slice start must be less than or equal to slice end",
        ),
        (
            "0",
            "999999999999999999999999999999",
            "Slice end must not exceed length",
        ),
    ] {
        let source = SourceFile::new(
            "bounds.ok",
            format!(
                "module app v1.0.0; main() is int {{ \"🙂Я\".slice({start}, {end}).length(); }}"
            ),
        )
        .unwrap();
        let ast = syntax::parse(&source, &syntax::ParseOptions::default()).unwrap();
        let module = lowering::lower_program(&ast).unwrap();
        let mut expected_span = None;
        for inst in module
            .functions
            .iter()
            .flat_map(|f| &f.blocks)
            .flat_map(|b| &b.insts)
        {
            use okkama_compiler::ir::{InstKind, StrOp};
            if matches!(
                inst.kind,
                InstKind::Str {
                    op: StrOp::Slice,
                    ..
                }
            ) {
                expected_span = Some(inst.span);
            }
        }
        let program = okkama_compiler::codegen::sparrow::generate(&module).unwrap();
        let bytes = okkama_sparrow::encode(&program).unwrap();
        assert!(
            matches!(okkama_runtime::sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap()),
            Err(okkama_runtime::sparrow::Error::Operation { error: RuntimeError::Trap { message: actual, .. }, .. }) if actual == message)
        );
        let error = execute(&module).unwrap_err();
        assert_eq!(
            error,
            RuntimeError::Trap {
                span: expected_span.unwrap(),
                message: message.into()
            }
        );
        assert_eq!(error.to_string(), format!("RuntimeError: {message}"));
    }
    // Ошибка предыдущего запуска не мешает следующему выполнению.
    assert_eq!(run("", r#""🙂Я".slice(1, 2).length();"#), "1");
}
