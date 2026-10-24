//! Exact scalar conversions through both engines and the bytecode file format.
use okkama_compiler::{SourceFile, codegen, ir, lowering, syntax};
use okkama_runtime::{RuntimeError, execute, sparrow};

fn lower(declarations: &str, body: &str) -> ir::Module {
    let source = SourceFile::new(
        "scalars.ok",
        format!("module app v1.0.0; {declarations} main() is int {{ {body} }}"),
    )
    .unwrap();
    lowering::lower_program(&syntax::parse(&source, &syntax::ParseOptions::default()).unwrap())
        .unwrap()
}

fn check(module: &ir::Module, expected: &str) {
    assert_eq!(execute(module).unwrap().to_string(), expected);
    let bytes = okkama_sparrow::encode(&codegen::sparrow::generate(module).unwrap()).unwrap();
    assert_eq!(
        sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap())
            .unwrap()
            .to_string(),
        expected
    );
}

#[test]
fn integer_bitwise_operations_use_signed_words_and_exact_shift_counts() {
    for (expression, expected) in [
        ("12 & 10", "8"),
        ("240 | 15", "255"),
        ("255 ^ 15", "240"),
        ("~0", "-1"),
        ("~-1", "0"),
        ("1 << 63", "-9223372036854775808"),
        ("-3 >> 1", "-2"),
        ("-1 >>> 1", "9223372036854775807"),
        ("-1 >>> 0", "-1"),
        ("-1 >>> 63", "1"),
    ] {
        check(&lower("", &format!("{expression};")), expected);
    }
    // R01: int точный, а битовые операции — над словом signed-64; больший
    // операнд — ошибка, как в JVM-рантайме.
    for expression in [
        "18446744073709551616 | 1",
        "9223372036854775808 >> 63",
        "~9223372036854775808",
    ] {
        let module = lower("", &format!("{expression};"));
        let expected = "Bitwise operand is outside signed-64 range";
        assert!(
            matches!(execute(&module), Err(RuntimeError::Trap { message, .. }) if message == expected),
            "{expression}"
        );
        let bytes = okkama_sparrow::encode(&codegen::sparrow::generate(&module).unwrap()).unwrap();
        let error = sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap()).unwrap_err();
        assert!(
            matches!(error, sparrow::Error::Operation { error: RuntimeError::Trap { message, .. }, .. } if message == expected),
            "{expression}"
        );
    }
    for operator in ["<<", ">>", ">>>"] {
        for count in ["-1", "64", "18446744073709551616"] {
            let module = lower("", &format!("1 {operator} {count};"));
            let expected = "Shift count must be in 0..63";
            assert!(
                matches!(execute(&module), Err(RuntimeError::Trap { message, .. }) if message == expected)
            );
            let bytes =
                okkama_sparrow::encode(&codegen::sparrow::generate(&module).unwrap()).unwrap();
            let error = sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap()).unwrap_err();
            assert!(
                matches!(error, sparrow::Error::Operation { error: RuntimeError::Trap { message, .. }, .. } if message == expected)
            );
        }
    }
}

#[test]
fn byte_bitwise_operations_are_eight_bit_and_shifts_check_exact_counts() {
    for (expression, expected) in [
        ("byte(12) & byte(10)", "8"),
        ("byte(240) | byte(15)", "255"),
        ("byte(255) ^ byte(15)", "240"),
        ("~byte(0)", "255"),
        ("~byte(255)", "0"),
        ("byte(128) << 1", "0"),
        ("byte(255) << 7", "128"),
        ("byte(128) >> 7", "1"),
        ("byte(255) >>> 4", "15"),
        ("byte(255) << 0", "255"),
        ("byte(255) >> 0", "255"),
    ] {
        check(&lower("", &format!("({expression}).int();")), expected);
    }
    for operator in ["<<", ">>", ">>>"] {
        for count in ["-1", "8", "999999999999999999999999999999"] {
            let module = lower("", &format!("(byte(1) {operator} {count}).int();"));
            let message = "Shift count must be in 0..7";
            assert!(
                matches!(execute(&module), Err(RuntimeError::Trap { message: actual, .. }) if actual == message)
            );
            let bytes =
                okkama_sparrow::encode(&codegen::sparrow::generate(&module).unwrap()).unwrap();
            assert!(
                matches!(sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap()), Err(sparrow::Error::Operation { error: RuntimeError::Trap { message: actual, .. }, .. }) if actual == message)
            );
        }
    }
    check(
        &lower("", "if false { (byte(1) << 8).int(); } else { 42; }"),
        "42",
    );
}

#[test]
fn ranges_in_arrays_preserve_exact_endpoints_and_order() {
    for (body, expected) in [
        (
            "let xs = [10, 1..3, 7]; xs[0] + xs[1] + xs[2] + xs[3];",
            "20",
        ),
        ("let xs = [-2..=2]; xs.size() + xs[0] + xs[4];", "5"),
        ("let xs = [3..3]; xs.size();", "0"),
        ("let xs = [3..=3]; xs[0];", "3"),
        (
            "let xs = [999999999999999999999999999999..=1000000000000000000000000000001]; xs[2] - xs[0];",
            "2",
        ),
        (
            "let xs = [-1000000000000000000000000000001..-999999999999999999999999999999]; xs.size();",
            "2",
        ),
    ] {
        check(&lower("", body), expected);
    }
    let module = lower("", "let xs = [3..2]; xs.size();");
    let message = "Range start must be less than or equal to range end";
    assert!(
        matches!(execute(&module), Err(RuntimeError::Trap { message: actual, .. }) if actual == message)
    );
    let bytes = okkama_sparrow::encode(&codegen::sparrow::generate(&module).unwrap()).unwrap();
    assert!(
        matches!(sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap()), Err(sparrow::Error::Operation { error: RuntimeError::Trap { message: actual, .. }, .. }) if actual == message)
    );
    check(
        &lower("", "if false { let xs = [3..2]; xs.size(); } else { 42; }"),
        "42",
    );
}

#[test]
fn exact_scalar_conversions_keep_types_and_decimal_text() {
    for (body, expected) in [
        ("int(byte(255));", "255"),
        ("byte(0).int();", "0"),
        (
            r#"if string(true) + string(false) == "10" { 42; } else { 0; }"#,
            "42",
        ),
        (
            r#"if string(byte(7)) == "7" && string(-99999999999999999999999) == "-99999999999999999999999" { 42; } else { 0; }"#,
            "42",
        ),
        (
            "if byte(255) > byte(0) && byte(42) == byte(42) { 42; } else { 0; }",
            "42",
        ),
        ("if '🙂' == '🙂' && 'é' != 'e' { 42; } else { 0; }", "42"),
        ("let xs = @byte(0, 42, 255); xs[1].int();", "42"),
        (
            "let nums = [11, 22]; let xs = @byte(0, nums..., 255); xs.size() + xs[1].int() + xs[2].int();",
            "37",
        ),
        (
            r#"if @string(1, true, byte(7)).join(",") == "1,1,7" { 42; } else { 0; }"#,
            "42",
        ),
        (
            "let xs = ['a', '🙂']; if xs[1] == '🙂' { 42; } else { 0; }",
            "42",
        ),
    ] {
        check(&lower("", body), expected);
    }
    check(
        &lower(
            "object Box<T> { has value is T; Box(:value); get() is T { value; } }",
            "let b = Box(value: byte(42)); b.get().int();",
        ),
        "42",
    );
}

#[test]
fn array_builders_execute_map_and_filter_without_mutating_inputs() {
    check(
        &lower(
            "",
            "let xs = [1, 2, 3]; let ys = xs.map() -> x { x + 10; }; ys[0] + ys[1] + ys[2] + xs.size();",
        ),
        "39",
    );
    check(
        &lower(
            "",
            "let xs = [1, 2, 3, 4]; let ys = xs.filter() -> x { x > 2; }; ys[0] + ys[1];",
        ),
        "7",
    );
}

#[test]
fn byte_casts_trap_without_truncation_and_keep_argument_context() {
    for body in [
        "byte(-1).int();",
        "byte(256).int();",
        "byte(999999999999999999999999999999).int();",
        "let xs = @byte(1, 256); xs.size();",
    ] {
        let module = lower("", body);
        let Err(RuntimeError::Trap { message, .. }) = execute(&module) else {
            panic!("expected byte range trap: {body}");
        };
        assert!(message.contains("expected 0..255"), "{message}");
        if body.contains("@byte") {
            assert!(message.contains("argument 2"), "{message}");
        }
        let bytes = okkama_sparrow::encode(&codegen::sparrow::generate(&module).unwrap()).unwrap();
        assert!(
            matches!(sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap()), Err(sparrow::Error::Operation { error: RuntimeError::Trap { message: actual, .. }, .. }) if actual == message)
        );
    }
    check(
        &lower("", "if false { byte(256).int(); } else { 42; }"),
        "42",
    );
}

#[test]
fn utf8_instruction_preserves_scalar_encoding_and_zero_bytes() {
    // The source adapter is a role; exercise its underlying IR operation directly.
    let mut module = lower("", "\"\0Aé🙂\".length();");
    let function = &mut module.functions[0];
    let bytes = ir::ValueId(function.values.len());
    function
        .values
        .push(ir::Type::Array(Box::new(ir::Type::Byte)));
    let block = &mut function.blocks[0];
    let position = block
        .insts
        .iter()
        .position(|i| {
            matches!(
                i.kind,
                ir::InstKind::Str {
                    op: ir::StrOp::Len,
                    ..
                }
            )
        })
        .unwrap();
    let ir::InstKind::Str { args, .. } = block.insts[position].kind.clone() else {
        unreachable!()
    };
    let span = block.insts[position].span;
    block.insts[position].kind = ir::InstKind::ArrayLen(bytes);
    block.insts.insert(
        position,
        ir::Inst {
            result: Some(bytes),
            kind: ir::InstKind::Str {
                op: ir::StrOp::Utf8,
                args,
            },
            span,
        },
    );
    check(&module, "8");
}
