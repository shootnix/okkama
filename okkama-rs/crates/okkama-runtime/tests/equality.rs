use okkama_compiler::{SourceFile, codegen, lowering, syntax};
use okkama_runtime::{execute, sparrow};

fn check(declarations: &str, body: &str) {
    let source = SourceFile::new("equality.ok", format!("module app v1.0.0; {declarations} main() is int {{ if {body} {{ 42; }} else {{ 1; }} }}")).unwrap();
    let module =
        lowering::lower_program(&syntax::parse(&source, &syntax::ParseOptions::default()).unwrap())
            .unwrap();
    assert_eq!(execute(&module).unwrap().to_string(), "42");
    let program = codegen::sparrow::generate(&module).unwrap();
    let bytes = okkama_sparrow::encode(&program).unwrap();
    assert_eq!(
        sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap())
            .unwrap()
            .to_string(),
        "42"
    );
}

#[test]
fn arrays_and_generic_private_fields_compare_structurally() {
    check(
        "",
        "[[1, 2], [3]] == [[1, 2], [3]] && [1, 2] != [2, 1] && [1] != [1, 2]",
    );
    let declarations = "object Box<T> { has value is T; Box(:value); }";
    check(
        declarations,
        "Box(value: [1, 2]) == Box(value: [1, 2]) && Box(value: [1, 2]) != Box(value: [1, 3])",
    );
    check(
        "enum Color = (red, blue);",
        "[Color.red, Color.blue] == [Color.red, Color.blue] && [Color.red] != [Color.blue]",
    );
}

#[test]
fn role_equality_uses_concrete_type_and_never_calls_user_methods() {
    check(
        "role Reading { read() is int; } object A does Reading { has n is int; A(:n); read() is int { panic \"must not run\"; } } object B does Reading { has n is int; B(:n); read() is int { panic \"must not run\"; } } object Compare { Compare(); equal(a is Reading, b is Reading) is bool { a == b; } }",
        "Compare().equal(A(n: 1), A(n: 1)) && !Compare().equal(A(n: 1), B(n: 1)) && !Compare().equal(A(n: 1), A(n: 2))",
    );
}

#[test]
fn successor_keeps_the_original_comparison_value() {
    check(
        "object Counter { has n is int; Counter(:n); inc() is ^Counter { ^n = n + 1; } } object Check { Check(); run() is bool { let original = Counter(n: 1); let alias = original; let changed = original; changed = changed.inc(); original == alias && original == Counter(n: 1) && changed == Counter(n: 2) && original != changed; } }",
        "Check().run()",
    );
}
