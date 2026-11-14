//! R01: int is exact — arithmetic never wraps at signed 64 bits.
#[path = "sparrow_support/mod.rs"]
mod support;
use okkama_compiler::{SourceFile, lowering, syntax};

fn check(body: &str, expected: &str) {
    let source = SourceFile::new("integers.ok", format!(
        "module app v1.0.0; object Calc {{ Calc(); value() is int {{ {body} }} }} main() is int {{ if Calc().value() == {expected} {{ 42; }} else {{ 0; }} }}"
    )).unwrap();
    let ast = syntax::parse(&source, &syntax::ParseOptions::default()).unwrap();
    let module = lowering::lower_program(&ast).unwrap();
    support::assert_value(&module, "42");
}

#[test]
fn arithmetic_is_exact_beyond_signed_64_r01() {
    check(
            "let max = 9223372036854775807; let min = -9223372036854775808; if max + 1 == 9223372036854775808 && min - 1 == -9223372036854775809 && max * 2 == 18446744073709551614 && -min == 9223372036854775808 && min * -1 == 9223372036854775808 && min % -1 == 0 { 42; } else { 0; }"
        ,
        "42"
    );
}

#[test]
fn negation_is_exact_and_preserves_canonical_zero() {
    for (body, expected) in [
        ("-0;", "0"),
        ("let n = 42; -n;", "-42"),
        ("let n = -42; -n;", "42"),
        ("-(1 + 2);", "-3"),
        (
            "-(-999999999999999999999999999999999999999999);",
            "999999999999999999999999999999999999999999",
        ),
        ("let n = -9223372036854775808; -n;", "9223372036854775808"),
    ] {
        check(body, expected);
    }
}

#[test]
fn subtraction_handles_signs_borrows_and_large_results() {
    for (body, expected) in [
        ("8 - 3;", "5"),
        ("3 - 8;", "-5"),
        ("-8 - 3;", "-11"),
        ("8 - (-3);", "11"),
        ("-8 - (-3);", "-5"),
        ("-3 - (-8);", "5"),
        ("let n = -42; n - n;", "0"),
        (
            "1000000000000000000000000000000000000000000 - 1;",
            "999999999999999999999999999999999999999999",
        ),
        (
            "-999999999999999999999999999999999999999999 - 1;",
            "-1000000000000000000000000000000000000000000",
        ),
        ("1 - 2 - 3;", "-4"),
    ] {
        check(body, expected);
    }
}

#[test]
fn computed_slice_bounds_use_subtraction() {
    check(
            r#"
        let s = "А🙂Я";
        let end = s.length();
        if s.slice(end - 2, end - 1) == "🙂" { 42; } else { 0; }
    "#
        ,
        "42"
    );
}
