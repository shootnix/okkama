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
            "oir 2 high\n",
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
    // The subject is a parameter: constants would fold (`fold`, #56).
    let text = listing(
        "module app v1.0.0; object M { M(); pick(n is int) is int { match n { case 1, 2 { 5; } else { 6; } } } } main() is int { M().pick(2); }",
    );
    let start = text.find("func @M.pick").expect("pick is printed");
    let end = start + text[start..].find("\n}\n").expect("end of pick") + 3;
    assert_eq!(
        &text[start..end],
        concat!(
            "func @M.pick(%0: M, %1: int) -> int [method M] {\n",
            "^b0:\n",
            "    %2: int = const 1\n",
            "    %3: bool = int.eq %1, %2\n",
            "    cond %3, ^b1, ^b2\n",
            "^b1:\n",
            "    %4: bool = const true\n",
            "    br ^b3(%4)\n",
            "^b2:\n",
            "    %5: int = const 2\n",
            "    %6: bool = int.eq %1, %5\n",
            "    br ^b3(%6)\n",
            "^b3(%7: bool):\n",
            "    cond %7, ^b4, ^b5\n",
            "^b4:\n",
            "    %8: int = const 5\n",
            "    br ^b6(%8)\n",
            "^b5:\n",
            "    %9: int = const 6\n",
            "    br ^b6(%9)\n",
            "^b6(%10: int):\n",
            "    ret %10\n",
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
