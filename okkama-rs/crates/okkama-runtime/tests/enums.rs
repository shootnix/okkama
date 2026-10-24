//! Enum identity remains nominal when values pass through collections and calls.
use okkama_compiler::{SourceFile, ir, lowering, syntax};
use okkama_runtime::execute;

fn lower(declarations: &str, body: &str) -> ir::Module {
    let source = SourceFile::new(
        "enums.ok",
        format!("module app v1.0.0; {declarations} main() is int {{ {body} }}"),
    )
    .unwrap();
    lowering::lower_program(&syntax::parse(&source, &syntax::ParseOptions::default()).unwrap())
        .unwrap()
}

#[test]
fn enum_values_survive_arrays_generic_fields_and_role_dispatch() {
    let module = lower(
        "enum Color = (red, blue); role Reading { get() is Color; } object Box<T> { has value is T; Box(:value); get() is T { value; } } object Reader does Reading { has color is Color; Reader(:color); get() is Color { color; } }",
        "let colors is @Color = [Color.red, Color.blue]; let boxed = Box(value: colors[1]); let reader is Reading = Reader(color: boxed.get()); if reader.get() == Color.blue && colors[0] != Color.blue { 42; } else { 0; }",
    );
    check(&module, "42");
}

#[test]
fn enum_match_selects_cases_and_default() {
    for (case, expected) in [("red", "10"), ("amber", "20"), ("green", "42")] {
        let module = lower(
            "enum Light = (red, amber, green);",
            &format!(
                "let light = Light.{case}; match light {{ case Light.red {{ 10; }} case Light.green {{ 42; }} else {{ 20; }} }}"
            ),
        );
        check(&module, expected);
    }
}

#[test]
fn shared_enum_comparison_rejects_cross_type_and_undefined_values() {
    use okkama_runtime_core::Value;
    let red = Value::Enum {
        declaration: 0,
        case: 0,
    };
    assert!(red.enum_eq(&red.clone()).unwrap());
    assert!(
        !red.enum_eq(&Value::Enum {
            declaration: 0,
            case: 1
        })
        .unwrap()
    );
    assert!(
        red.enum_eq(&Value::Enum {
            declaration: 1,
            case: 0
        })
        .is_err()
    );
    assert!(red.enum_eq(&Value::Undef).is_err());
}

fn check(module: &ir::Module, expected: &str) {
    assert_eq!(execute(module).unwrap().to_string(), expected);
    let program = okkama_compiler::codegen::sparrow::generate(module).unwrap();
    let bytes = okkama_sparrow::encode(&program).unwrap();
    assert_eq!(&bytes[4..6], &okkama_sparrow::VERSION.to_be_bytes());
    let decoded = okkama_sparrow::decode(&bytes).unwrap();
    assert_eq!(program, decoded);
    assert_eq!(
        okkama_runtime::sparrow::execute(&decoded).unwrap().to_string(),
        expected
    );
}
