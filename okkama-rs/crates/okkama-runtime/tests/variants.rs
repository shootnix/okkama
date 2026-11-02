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
        okkama_runtime::sparrow::execute(&program)
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
        "main() is int { let a = Presence<string>.missing.collapse() else \"missing\"; let b = Presence.some(\"\").collapse() else \"wrong\"; let c = Maybe.some(false).as_presence(); if a == \"missing\" && b == \"\" && c == Presence.some(false) { 42; } else { 0; } }",
        "42",
    );
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
