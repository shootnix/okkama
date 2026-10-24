//! Lowering `enum` и `match`: `enum.case`, `enum.eq`, ленивые образцы,
//! полное покрытие enum и диагностика в формате Perl-интерпретатора.

use okkama_compiler::SourceFile;
use okkama_compiler::ir;
use okkama_compiler::lowering::{LoweringError, lower_program};
use okkama_compiler::syntax::{self, ParseOptions, prune};

fn lower(text: &str) -> Result<ir::Module, LoweringError> {
    let source = SourceFile::new("test.ok", text).expect("small source");
    let program = syntax::parse(&source, &ParseOptions::default()).expect("valid syntax");
    lower_program(&prune::strip_test_items(program))
}

fn listing(text: &str) -> String {
    let module = lower(text).unwrap_or_else(|error| panic!("{text}: {error}"));
    ir::verify(&module).expect("lowering output verifies");
    module.to_string()
}

#[test]
fn a_covering_enum_match_skips_the_last_test() {
    assert_eq!(
        listing(
            "module app v1.0.0; enum Light = (red, amber, green); main() is int { \
             let l = Light.amber; \
             match l { case Light.red { 1; } case Light.amber, Light.green { 2; } } }"
        ),
        concat!(
            "oir 1 high\n",
            "module app v1.0.0\n",
            "requires core\n",
            "entry @main\n",
            "\n",
            "enum Light = (red, amber, green)\n",
            "\n",
            "func @main() -> int [entry] {\n",
            "^b0:\n",
            "    %0: Light = enum.case Light.amber\n",
            "    %1: Light = enum.case Light.red\n",
            "    %2: bool = enum.eq %0, %1\n",
            "    cond %2, ^b1, ^b2\n",
            "^b1:\n",
            "    %3: int = const 1\n",
            "    br ^b3(%3)\n",
            "^b2:\n",
            "    %4: int = const 2\n",
            "    br ^b3(%4)\n",
            "^b3(%5: int):\n",
            "    ret %5\n",
            "}\n",
        )
    );
}

#[test]
fn case_patterns_are_tested_lazily_left_to_right() {
    let text = listing(
        "module app v1.0.0; main() is int { let n = 2; match n { case 1, 2 { 5; } else { 6; } } }",
    );
    let start = text.find("func @main").expect("main is printed");
    assert_eq!(
        &text[start..],
        concat!(
            "func @main() -> int [entry] {\n",
            "^b0:\n",
            "    %0: int = const 2\n",
            "    %1: int = const 1\n",
            "    %2: bool = int.eq %0, %1\n",
            "    cond %2, ^b1, ^b2\n",
            "^b1:\n",
            "    %3: bool = const true\n",
            "    br ^b3(%3)\n",
            "^b2:\n",
            "    %4: int = const 2\n",
            "    %5: bool = int.eq %0, %4\n",
            "    br ^b3(%5)\n",
            "^b3(%6: bool):\n",
            "    cond %6, ^b4, ^b5\n",
            "^b4:\n",
            "    %7: int = const 5\n",
            "    br ^b6(%7)\n",
            "^b5:\n",
            "    %8: int = const 6\n",
            "    br ^b6(%8)\n",
            "^b6(%9: int):\n",
            "    ret %9\n",
            "}\n",
        )
    );
}

#[test]
fn enum_and_match_rule_violations_are_semantic_errors() {
    let decls = "enum Color = (red, blue); enum Light = (red, green);";
    for (text, expected) in [
        (
            "enum Color = (red, red); main() is int { 0; }".to_string(),
            "Enum \"Color\" already has case \"red\"",
        ),
        (
            "enum Color = (red); enum Color = (blue); main() is int { 0; }".to_string(),
            "Enum \"Color\" is already declared",
        ),
        (
            "object Color { Color() {} } enum Color = (red); main() is int { 0; }".to_string(),
            "Enum \"Color\" is already declared",
        ),
        (
            format!("{decls} main() is int {{ let c = Color.purple; 0; }}"),
            "Enum \"Color\" has no case \"purple\"",
        ),
        (
            format!("{decls} main() is int {{ let s = Color.red == Light.red; 0; }}"),
            "Cannot compare Color and Light",
        ),
        (
            format!("{decls} main() is int {{ let c is Color = Light.green; 0; }}"),
            "Cannot initialize variable \"c\" of type Color with Light",
        ),
        (
            format!(
                "{decls} main() is int {{ match Color.red {{ case 1 {{ 5; }} else {{ 6; }} }} }}"
            ),
            "Cannot compare Color and int",
        ),
        (
            "main() is int { let n = 1; match n { case \"1\" { 1; } else { 2; } } }".to_string(),
            "Cannot compare int and string",
        ),
    ] {
        let error = lower(&format!("module app v1.0.0; {text}")).unwrap_err();
        assert!(
            matches!(error, LoweringError::Invalid { .. }),
            "{text}: {error:?}"
        );
        assert_eq!(error.message(), expected, "{text}");
    }
}
