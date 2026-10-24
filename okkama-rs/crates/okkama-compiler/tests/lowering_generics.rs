//! Generics (§6.5, OIR-1 §10): мономорфизация шаблонов в экземпляры,
//! вывод аргументов типа и диагностика в формате Perl-интерпретатора.

use okkama_compiler::SourceFile;
use okkama_compiler::ir;
use okkama_compiler::lowering::{LoweringError, lower_program};
use okkama_compiler::syntax::{self, ParseOptions, prune};

fn lower(text: &str) -> Result<ir::Module, LoweringError> {
    let source = SourceFile::new("test.ok", text).expect("small source");
    let program = syntax::parse(&source, &ParseOptions::default()).expect("valid syntax");
    lower_program(&prune::strip_test_items(program))
}

const BOX: &str = "module app v1.0.0; \
    object Box<T> { has value is T; Box(:value); get() is T { value; } }";

#[test]
fn each_specialization_is_one_object_with_its_own_functions() {
    let module = lower(&format!(
        "{BOX} main() is int {{ let a = Box(value: 7); let b is Box<@int> = Box(value: []); \
         let c = Box(value: 8); a.get() + b.get().size() + c.get(); }}"
    ))
    .expect("lowering succeeds");
    ir::verify(&module).expect("valid IR");
    let names: Vec<&str> = module
        .objects
        .iter()
        .map(|object| object.name.as_str())
        .collect();
    // `Box(value: 8)` — тот же экземпляр `Box<int>`, что и `a`.
    assert_eq!(names, ["Box<int>", "Box<@int>"]);
    let text = module.to_string();
    for expected in [
        "func @Box<int>.get(%0: Box<int>) -> int [method Box<int>]",
        "func @Box<@int>.new#0(%0: array<int>) -> Box<@int> [ctor Box<@int>]",
        // `[]` получает тип параметра конструктора сразу (§6.5).
        "array<int> = array.new []",
    ] {
        assert!(text.contains(expected), "{expected} in\n{text}");
    }
}

#[test]
fn generic_rule_violations_use_the_interpreter_texts() {
    for (text, expected) in [
        (
            format!("{BOX} main() is int {{ let p = Box(value: []); 0; }}"),
            "Cannot infer generic parameter \"T\" for object \"Box\"",
        ),
        (
            format!("{BOX} main() is int {{ let x is Box<int, int> = Box(value: 1); 0; }}"),
            "Generic type \"Box\" expects 1 type argument(s), got 2",
        ),
        (
            format!("{BOX} object Coin {{ Coin(); }} main() is int {{ let c is Coin<int> = Coin(); 0; }}"),
            "Type \"Coin\" is not generic and cannot accept type arguments",
        ),
        (
            "module app v1.0.0; object Same<T> { has a is T; has b is T; Same(:a, :b); } \
             main() is int { let p = Same(a: 1, b: \"x\"); 0; }"
                .to_string(),
            "Generic parameter \"T\" for object \"Same\" cannot be both int and string",
        ),
        (
            "module app v1.0.0; role Ranked<T> { beats(other is T) is bool; } \
             object Best<T: Ranked<T>> { has first is T; Best(:first); } \
             main() is int { let b = Best(first: 1); 0; }"
                .to_string(),
            "Generic parameter \"T\" for object \"Best\" has type int, which does not satisfy Ranked<int>",
        ),
        (
            "module app v1.0.0; role Echo<T> { echo(value is T) is T; } \
             object EchoBox<T> does Echo<T> { has seed is T; EchoBox(:seed); echo(value is T) is T { value; } } \
             main() is int { let e is Echo<string> = EchoBox(seed: 1); 0; }"
                .to_string(),
            "Generic parameter \"T\" for object \"EchoBox\" cannot be both string and int",
        ),
    ] {
        let error = lower(&text).unwrap_err();
        assert!(matches!(error, LoweringError::Invalid { .. }), "{text}: {error:?}");
        assert_eq!(error.message(), expected, "{text}");
    }
}
