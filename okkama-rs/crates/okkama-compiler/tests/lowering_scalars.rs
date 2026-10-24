//! Lowering скаляров `byte` и `char` (§16): константы, `byte.*`, `char.eq`,
//! битовые действия и диагностика в формате Perl-интерпретатора.

use okkama_compiler::SourceFile;
use okkama_compiler::ir;
use okkama_compiler::lowering::{LoweringError, lower_program};
use okkama_compiler::syntax::{self, ParseOptions, prune};

fn lower(body: &str) -> Result<ir::Module, LoweringError> {
    let text = format!("module app v1.0.0; main() is int {{ {body} }}");
    let source = SourceFile::new("test.ok", &text).expect("small source");
    let program = syntax::parse(&source, &ParseOptions::default()).expect("valid syntax");
    lower_program(&prune::strip_test_items(program))
}

fn main_listing(body: &str) -> String {
    let module = lower(body).unwrap_or_else(|error| panic!("{body}: {error}"));
    ir::verify(&module).expect("lowering output verifies");
    let text = module.to_string();
    let start = text.find("func @main").expect("main is printed");
    // Только `main`: lowering по достижимости печатает её первой.
    let end = text[start..]
        .find("\n}\n")
        .map_or(text.len(), |end| start + end + 3);
    text[start..end].to_string()
}

#[test]
fn bytes_widen_to_int_for_arithmetic_and_stay_bytes_for_bits() {
    assert_eq!(
        main_listing(
            "let b = byte(200); let c = 'x'; let s = b + byte(1); let k = (b << 1) & byte(15); \
             if c == 'y' { 0; } else { s + k.bit_count(); }"
        ),
        concat!(
            "func @main() -> int [entry] {\n",
            "^b0:\n",
            "    %0: byte = const 200\n",
            "    %1: char = const 'x'\n",
            "    %2: byte = const 1\n",
            "    %3: int = cvt %0\n",
            "    %4: int = cvt %2\n",
            "    %5: int = int.add %3, %4\n",
            "    %6: int = const 1\n",
            "    %7: byte = byte.shl %0, %6\n",
            "    %8: byte = const 15\n",
            "    %9: byte = byte.and %7, %8\n",
            "    %10: char = const 'y'\n",
            "    %11: bool = char.eq %1, %10\n",
            "    cond %11, ^b1, ^b2\n",
            "^b1:\n",
            "    %12: int = const 0\n",
            "    br ^b3(%12)\n",
            "^b2:\n",
            "    %13: int = byte.bit_count %9\n",
            "    %14: int = int.add %5, %13\n",
            "    br ^b3(%14)\n",
            "^b3(%15: int):\n",
            "    ret %15\n",
            "}\n",
        )
    );
    // Вне литерала диапазон проверяется во время исполнения; у `@byte` —
    // с номером аргумента в тексте ошибки.
    let checked = main_listing("let n = 300; let b = byte(n); let xs = @byte(1, n); 0;");
    assert!(checked.contains("byte = byte.from %"), "{checked}");
    assert!(checked.contains(", \"@byte() argument 2\""), "{checked}");
    // Упорядочивание после явного каста `byte` → `int`.
    let ordered = main_listing("let b = byte(3); if b.int() < 9 { 1; } else { 2; }");
    assert!(ordered.contains("int = cvt %0\n"), "{ordered}");
    assert!(ordered.contains("bool = int.lt %"), "{ordered}");
    let rotated = main_listing("let n = 1; n.rotate_left(3);");
    assert!(rotated.contains("int = int.rotl %"), "{rotated}");
    let escaped = main_listing("let c = '\\n'; let q = '\\''; 0;");
    assert!(escaped.contains("char = const '\\n'"), "{escaped}");
    assert!(escaped.contains("char = const '\\''"), "{escaped}");
}

#[test]
fn byte_and_char_rule_violations_are_semantic_errors() {
    for (body, expected) in [
        (
            "let b = byte(7); b & 12;",
            "Bitwise operands must have the same int or byte category",
        ),
        (
            "let b = byte(7); b << byte(1);",
            "Shift count must have type int",
        ),
        (
            "let b = byte(3); if b == 3 { 1; } else { 2; }",
            "Cannot compare byte and int",
        ),
        (
            "let c = 'a'; if c == \"a\" { 1; } else { 2; }",
            "Cannot compare char and string",
        ),
        (
            "let c = 'a'; if c < 'b' { 1; } else { 2; }",
            "Operator < cannot be applied to char and char",
        ),
        ("let s = string('a'); 0;", "Cannot convert char to string"),
        ("let b = byte(true); 0;", "Cannot convert bool to byte"),
        ("let b = byte(\"1\"); 0;", "Cannot convert string to byte"),
        // Концептуально разные типы взаимодействуют только после явного каста.
        (
            "let b = byte(3); if b < 9 { 1; } else { 2; }",
            "Operator < cannot be applied to byte and int",
        ),
        (
            "let b = byte(3); b + 1;",
            "Operator + cannot be applied to byte and int",
        ),
        (
            "let b = byte(3); 10 - b;",
            "Operator - cannot be applied to int and byte",
        ),
        (
            "let b = byte(3); b / 2;",
            "Operator / cannot be applied to byte and int",
        ),
        (
            "let xs = [1, 2]; xs[byte(1)];",
            "Array index must be int, got byte",
        ),
        // D-007: `byte` — не ключ словаря.
        (
            "let m = [byte(1): 1]; 0;",
            "Map key type byte is not a scalar key type",
        ),
        (
            "let xs = [byte(0)..3]; 0;",
            "Range start must be int, got byte",
        ),
    ] {
        let error = lower(body).unwrap_err();
        assert!(
            matches!(error, LoweringError::Invalid { .. }),
            "{body}: {error:?}"
        );
        assert_eq!(error.message(), expected, "{body}");
    }
}

#[test]
fn float_never_mixes_with_other_numeric_types() {
    let listing =
        main_listing("let x = 1.5 + float(1); let y = -0.0; if x > 2.0 { 1; } else { 0; }");
    for expected in [
        "float = const 1.5\n",
        "float = cvt %",
        "float = float.add %",
        "float = const -0\n",
        "bool = float.gt %",
    ] {
        assert!(listing.contains(expected), "{expected} in\n{listing}");
    }
    for (body, expected) in [
        ("let s = 1 == 1.0; 0;", "Cannot compare int and float"),
        (
            "let x = 1.5 + 1; 0;",
            "Operator + cannot be applied to float and int",
        ),
        (
            "let x = 3 / 2.0; 0;",
            "Operator / cannot be applied to int and float",
        ),
        (
            "let x = byte(1) * 2.0; 0;",
            "Operator * cannot be applied to byte and float",
        ),
        // R02: упорядочивание разных числовых категорий — только с явным приведением.
        (
            "let s = 1 < 1.5; 0;",
            "Operator < cannot be applied to int and float",
        ),
        (
            "let x = 1.less(2.0); 0;",
            "Cannot pass float to builtin action \"less\" parameter of type int",
        ),
    ] {
        let error = lower(body).unwrap_err();
        assert_eq!(error.message(), expected, "{body}");
    }
}
