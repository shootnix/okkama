//! Variant values use the same typed representation on IR and Sparrow.
use okkama_compiler::{SourceFile, ir, lowering, syntax};

fn lower(body: &str) -> Result<ir::Module, String> {
    let source = SourceFile::new("variants.ok", format!("module app v1.0.0; {body}")).unwrap();
    let ast =
        syntax::parse(&source, &syntax::ParseOptions::default()).map_err(|e| format!("{e:?}"))?;
    lowering::lower_program(&ast).map_err(|e| format!("{e:?}"))
}
fn check(source: &str, expected: &str) {
    let module = lower(source).unwrap();
    assert_eq!(
        okkama_runtime::execute(&module).unwrap().to_string(),
        expected
    );
    let program = okkama_compiler::codegen::sparrow::generate(&module).unwrap();
    let program = okkama_sparrow::decode(&okkama_sparrow::encode(&program).unwrap()).unwrap();
    assert_eq!(
        sparrow_vm::execute(&program)
            .unwrap()
            .to_string(),
        expected
    );
}
#[test]
fn payload_patterns_and_generic_inference() {
    check(
        "enum Reading<T> = (empty, value(item is T), pair(left is T, right is T)); main() is int { let r = Reading.pair(20, 22); match r { case Reading.empty { 0; } case Reading.value(n) { n; } case Reading.pair(a, b) { a + b; } } }",
        "42",
    );
}
#[test]
fn optional_fallback_is_lazy_and_preserves_zero() {
    check(
        "main() is int { let a = Maybe.some(0) else panic \"unexpected\"; let b = Maybe<int>.none else 42; a + b; }",
        "42",
    );
}
#[test]
fn presence_preserves_all_three_states() {
    check(
        "object Reader { Reader(); get() is Presence<int> { Presence.missing; } } main() is int { match Reader().get() { case Presence.some(v) { v; } case Presence.none { 1; } case Presence.missing { 42; } } }",
        "42",
    );
}
#[test]
fn variant_diagnostics() {
    for (source, diagnostic) in [
        ("main() is int { let a = Maybe.none; 0; }", "Cannot infer"),
        (
            "main() is int { Maybe.some(1) else \"bad\"; 0; }",
            "Fallback",
        ),
        (
            "main() is int { Presence<int>.missing else 1; }",
            "requires Maybe",
        ),
        (
            "main() is int { match Maybe.some(1) { case Maybe.some(n) { n; } } }",
            "Non-exhaustive",
        ),
    ] {
        let error = lower(source).unwrap_err();
        assert!(error.contains(diagnostic), "{error}");
    }
}

#[test]
fn equality_is_nominal_and_compares_only_the_active_payload() {
    check(
        "enum Pair<T> = (empty, two(a is T, b is T)); main() is int { let a = Pair.two(20, 22); if a == Pair.two(20, 22) && a != Pair.two(21, 22) && a != Pair<int>.empty { 42; } else { 0; } }",
        "42",
    );
    check(
        "main() is int { let a is Maybe<Maybe<int>> = Maybe.some(Maybe.none); if a != Maybe.none && a == Maybe.some(Maybe<int>.none) { 42; } else { 0; } }",
        "42",
    );
}

#[test]
fn explicit_conversions_and_empty_values() {
    check(
        "main() is int { let a = Presence<string>.missing.fold() else \"missing\"; let b = Presence.some(\"\").fold() else \"wrong\"; let c = Maybe.some(false).as_presence(); if a == \"missing\" && b == \"\" && c == Presence.some(false) { 42; } else { 0; } }",
        "42",
    );
}

#[test]
fn method_results_use_fallback_or_payload_patterns() {
    check(
        r#"object User { User(); name() is Maybe<string> { Maybe.some(""); } }
        main() is int {
            let obj = User();
            let a = obj.name() else "no name";
            let b = match obj.name() {
                case Maybe.none { "no name"; }
                case Maybe.some(value) { value; }
            };
            let c = Presence<string>.none.fold() else "no name";
            if a == "" && b == "" && c == "no name" { 42; } else { 0; }
        }"#,
        "42",
    );
    for source in [
        "main() is int { Maybe.some(42).some else 0; }",
        "main() is int { Presence<int>.missing.collapse() else 0; }",
    ] {
        assert!(lower(source).is_err(), "must reject: {source}");
    }
}

#[test]
fn values_survive_collections_and_generic_calls() {
    check(
        "object Box<T> { has item is T; Box(:item); get() is T { item; } } main() is int { let values is @Maybe<int> = [Maybe.none, Maybe.some(42)]; let b = Box(item: values[1]); b.get() else 0; }",
        "42",
    );
}

#[test]
fn fallback_can_throw_and_does_not_catch_errors_from_the_left() {
    check(
        "object Absent does Error { Absent(); text() is string { \"absent\"; } } main() is int { try { Maybe<int>.none else throw Absent(); } catch e is Absent { 42; } }",
        "42",
    );
    check(
        "object Failed does Error { Failed(); text() is string { \"failed\"; } } object Reader { Reader(); get() is Maybe<int> { throw Failed(); } } main() is int { try { Reader().get() else 0; } catch e is Failed { 42; } }",
        "42",
    );
}

#[test]
fn bad_patterns_and_recursive_variants_are_rejected() {
    for (source, diagnostic) in [
        (
            "enum Recursive = (link(value is Recursive)); main() is int { 0; }",
            "Recursive variant",
        ),
        (
            "enum A = (link(value is Maybe<A>)); main() is int { 0; }",
            "Recursive variant",
        ),
        (
            "main() is int { match Maybe.some(1) { case Maybe.some(a) { a; } case Maybe.some(b) { b; } case Maybe.none { 0; } } }",
            "Duplicate variant",
        ),
        (
            "main() is int { match Maybe.some(1) { case Maybe.some(a, b) { 0; } case Maybe.none { 0; } } }",
            "number of variant",
        ),
        ("main() is int { let a = [Maybe.some(1): 1]; 0; }", "key"),
        (
            "object Bad { Bad(); get() is string { Maybe.some(\"x\"); } } main() is int { 0; }",
            "string",
        ),
    ] {
        let error = lower(source).unwrap_err();
        assert!(
            error.to_lowercase().contains(&diagnostic.to_lowercase()),
            "{error}"
        );
    }
}

#[test]
fn variants_compare_inside_containers_without_unbound_fields() {
    check(
        "main() is int { let a is @Maybe<int> = [Maybe.none, Maybe.some(42)]; let b is @Maybe<int> = [Maybe.none, Maybe.some(42)]; if a == b { 42; } else { 0; } }",
        "42",
    );
}

#[test]
fn payload_and_fallback_cross_role_boundaries() {
    check(
        "role Reading { read() is int; } object Item does Reading { Item(); read() is int { 42; } } main() is int { let item is Maybe<Reading> = Maybe.some(Item()); let value = item else Item(); value.read(); }",
        "42",
    );
}

#[test]
fn formatter_preserves_variant_payloads_and_fallback() {
    let source = SourceFile::new("format.ok", "module app v1.0.0; enum Reading<T>=(empty,value(item is T)); main() is int { let a = Maybe<int>.none else 42; a; }".to_string()).unwrap();
    let formatted = syntax::format::format(&source).unwrap();
    check(formatted.split_once(';').unwrap().1, "42");
    let twice =
        syntax::format::format(&SourceFile::new("format.ok", formatted.clone()).unwrap()).unwrap();
    assert_eq!(formatted, twice);
}

#[test]
fn compact_choices_are_lazy_typed_and_preserve_payloads() {
    check(
        r#"
        enum Color = (red, blue);
        main() is int {
            let a = true ?? 20 !! panic "unused";
            let b = false ?? panic "unused" !! 22;
            let name = Presence.some("") ?? .some(v): v !! .none: "null" !! .missing: "missing";
            let n = Maybe<int>.none ?? .some(v): v !! _: 42;
            let color = Color.red ?? .red: 42 !! .blue: 0;
            let negated = !!false ?? 0 !! 42;
            if name == "" && n == 42 && color == 42 && negated == 42 { a + b; } else { 0; }
        }
    "#,
        "42",
    );
}

#[test]
fn compact_choice_rejects_nesting_and_invalid_contracts() {
    for choice in [
        "true ?? Sink().write() !! 0",
        "Maybe.some(1) ?? .some(v): Sink().write() !! .none: 0",
    ] {
        let code = format!(
            "object Sink {{ Sink(); write() is void {{}} }} main() is int {{ {choice}; 0; }}"
        );
        assert!(lower(&code).unwrap_err().contains("must produce a value"));
    }

    for source in [
        "true ?? (false ?? 1 !! 2) !! 3",
        "true ?? 1 !! (false ?? 2 !! 3)",
        "(true ?? true !! false) ?? 1 !! 2",
        "true ?? [false ?? 1 !! 2][0] !! 3",
    ] {
        let error = lower(&format!("main() is int {{ {source}; }}")).unwrap_err();
        assert!(error.contains("Nested ?? / !!"), "{error}");
    }

    for source in [
        "true ?? (false ?? 1 !! 2) !! 3",
        "true ?? 1 !! (false ?? 2 !! 3)",
        "(true ?? true !! false) ?? 1 !! 2",
        "true ?? [false ?? 1 !! 2][0] !! 3",
        "Maybe.some(1) ?? .some(v): (true ?? v !! 0) !! .none: 0",
        "1 ?? 2 !! 3",
        "true ?? 1 !! false",
        "true ?? 1",
        "true ?? 1 !! 2 !! 3",
        "Maybe.some(1) ?? .some(v): v",
        "Maybe.some(1) ?? .some(v): v !! .some(w): w !! .none: 0",
        "Maybe.some(1) ?? .some(v): v !! .none: v",
        "Maybe.some(1) ?? _: 0 !! .none: 1",
        "Presence<int>.missing ?? .some(v): v !! .none: 0",
        "true ?? _: 1",
    ] {
        assert!(
            lower(&format!("main() is int {{ {source}; }}")).is_err(),
            "accepted: {source}"
        );
    }
    assert!(lower("enum Color = (red, blue); main() is int { Color.red ?? .red: 1; }").is_err());
}

#[test]
fn compact_choice_formatter_roundtrip() {
    let code = "module app v1.0.0; main() is int { let a=true??20!!0; let b=Maybe.some(22)??.some(v):v!!.none:0; a+b; }";
    let source = SourceFile::new("choice.ok", code.to_owned()).unwrap();
    let formatted = syntax::format::format(&source).unwrap();
    assert!(formatted.contains(" ?? ") && formatted.contains(" !! "));
    check(formatted.split_once(';').unwrap().1, "42");
    let twice =
        syntax::format::format(&SourceFile::new("choice.ok", formatted.clone()).unwrap()).unwrap();
    assert_eq!(formatted, twice);
}

#[test]
fn compact_choices_compose_without_nesting_and_can_throw() {
    check(
        r#"
        object Absent does Error { Absent(); text() is string { "absent"; } }
        object Pair { has a is int; has b is int; Pair(:a, :b); sum() is int { a + b; } }
        main() is int {
            let pair = Pair(a: true ?? 20 !! 0, b: Maybe.some(22) ?? .some(v): v !! _: 0);
            try { Presence<int>.missing ?? .some(v): v !! _: throw Absent(); }
            catch e is Absent { pair.sum(); }
        }
    "#,
        "42",
    );
}

#[test]
fn tail_expression_semicolons_are_optional_without_changing_values() {
    check("main() is int { if true { 42 } else { 0 }; }", "42");
    check(
        "main() is int { let a = if true { 20 } else { 0 }; let b = Maybe.some(22); match b { case Maybe.some(v) { a + v } case Maybe.none { 0 } } }",
        "42",
    );
    check(
        "object Answer { Answer(); value() is int { 42 } } main() is int { Answer().value() }",
        "42",
    );
    for source in [
        "main() is int { 1 42 }",
        "main() is int { let a = 42 }",
        "main() is int { return 42 }",
        "main() is int { defer 1 }",
    ] {
        assert!(lower(source).is_err(), "accepted: {source}");
    }
    let source = SourceFile::new(
        "tail.ok",
        "module app v1.0.0; main() is int { if true { 42 } else { 0 }; }".to_string(),
    )
    .unwrap();
    let formatted = syntax::format::format(&source).unwrap();
    assert!(!formatted.contains("42;"), "{formatted}");
    check(formatted.split_once(';').unwrap().1, "42");
    let twice =
        syntax::format::format(&SourceFile::new("tail.ok", formatted.clone()).unwrap()).unwrap();
    assert_eq!(formatted, twice);
}

#[test]
fn postfix_functionals_compose_with_optional_and_compact_choices() {
    check(
        r#"main() is int {
            let total = 1 + [20, 21].reduce(0) -> sum, value { sum + value };
            let fallback = Maybe<int>.none else [20, 22].reduce(0) -> sum, value { sum + value };
            let selected = total == 42 ?? [20, 22].reduce(0) -> sum, value { sum + value } !! 0;
            let values = [Maybe.some(42), Maybe<int>.none].map() -> item { item else 42 };
            for (values.map() -> value { value == 42 ?? value !! 0 }) -> value {
                if value != 42 { return 0; }
            }
            if total == fallback && fallback == selected { total } else { 0 };
        }"#,
        "42",
    );
}
