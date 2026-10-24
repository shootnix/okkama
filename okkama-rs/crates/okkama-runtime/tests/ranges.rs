//! Range values retain their bounds across serialization and use exact loop counters.
use okkama_compiler::{SourceFile, codegen, lowering, syntax};

fn check(declarations: &str, body: &str, expected: &str) {
    let source = SourceFile::new(
        "ranges.ok",
        format!("module app v1.0.0; {declarations} main() is int {{ {body} }}"),
    )
    .unwrap();
    let ast = syntax::parse(&source, &syntax::ParseOptions::default()).unwrap();
    let module = lowering::lower_program(&ast).unwrap();
    assert_eq!(okkama_runtime::execute(&module).unwrap().to_string(), expected);
    let program = codegen::sparrow::generate(&module).unwrap();
    let bytes = okkama_sparrow::encode(&program).unwrap();
    assert_eq!(
        okkama_runtime::sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap())
            .unwrap()
            .to_string(),
        expected
    );
}

#[test]
fn stored_ranges_iterate_with_their_original_bounds() {
    check("", "let r = 1..4; for r -> n { n; }", "3");
    check("", "let r = 1..=4; for r -> n { n; }", "4");
    check(
        "object Box<T> { has value is T; Box(:value); get() is T { value; } }",
        "let b = Box(value: 1..=4); for b.get() -> n { n; }",
        "4",
    );
}

#[test]
fn range_equality_preserves_inclusive_flag() {
    check(
        "",
        "if (1..=3) == (1..=3) && (1..=3) != (1..4) { 42; } else { 1; }",
        "42",
    );
}

#[test]
fn loop_counters_cross_signed_integer_boundary_exactly() {
    for body in [
        "for 9223372036854775807..=9223372036854775808 -> n { n; }",
        "let r = 9223372036854775807..=9223372036854775808; for r -> n { n; }",
    ] {
        check("", body, "9223372036854775808");
    }
}
