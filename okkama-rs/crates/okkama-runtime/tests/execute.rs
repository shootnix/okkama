use okkama_compiler::{SourceFile, ir, lowering, syntax};
use okkama_runtime::{RuntimeError, execute};

fn lower(body: &str) -> ir::Module {
    let source = SourceFile::new(
        "test.ok",
        format!("module app v1.0.0; main() is int {{ {body} }}"),
    )
    .unwrap();
    let ast = syntax::parse(&source, &syntax::ParseOptions::default()).unwrap();
    lowering::lower_program(&ast).unwrap()
}

#[test]
fn source_to_ir_to_result_in_the_same_process() {
    for (body, expected) in [
        ("1 + 1;", "2"),
        ("let a = 20; let b is int = 22; a + b;", "42"),
        ("let a = 3; return a + a + 1;", "7"),
        ("0 + 0;", "0"),
        ("9223372036854775807 + 1;", "9223372036854775808"),
        (
            "999999999999999999999999999999999999999999 + 1;",
            "1000000000000000000000000000000000000000000",
        ),
    ] {
        let module = lower(body);
        assert_eq!(execute(&module).unwrap().to_string(), expected);
        // Повторное выполнение не наследует слоты предыдущего вызова.
        assert_eq!(execute(&module).unwrap().to_string(), expected);
    }
}

#[test]
fn large_ir_constants_use_exact_arithmetic() {
    let mut module = lower("1 + 1;");
    let function = &mut module.functions[module.entry.unwrap().0];
    for inst in &mut function.blocks[0].insts {
        if let ir::InstKind::Const(ir::Constant::Int(value)) = &mut inst.kind {
            *value = ir::IntValue::parse("-999999999999999999999999999999").unwrap();
        }
    }
    assert_eq!(
        execute(&module).unwrap().to_string(),
        "-1999999999999999999999999999998"
    );
}

#[test]
fn unsupported_programs_return_errors() {
    let source = SourceFile::new(
        "unlinked.ok",
        "module runtime/unlinked v1.0.0; native nothing() is int; main() is int { nothing(); }",
    )
    .unwrap();
    let unlinked =
        lowering::lower_program(&syntax::parse(&source, &syntax::ParseOptions::default()).unwrap())
            .unwrap();
    assert!(matches!(
        execute(&unlinked),
        Err(RuntimeError::Unsupported { .. })
    ));
    let mut module = lower("1;");
    module.functions[module.entry.unwrap().0].attrs.clear();
    module.entry = None;
    assert_eq!(execute(&module), Err(RuntimeError::MissingEntry));
}

#[test]
fn branches_and_loops_execute_from_source() {
    for (body, expected) in [
        ("loop { last 7; }", "7"),
        ("let x = 3; if x < 4 { 10; } else { 20; }", "10"),
        ("let x = 5; if x < 4 { 10; } else { 20; }", "20"),
        (
            "let x = 3; let y = if x == 3 { 40; } else { 0; }; y + 2;",
            "42",
        ),
        ("for 1..=5 -> i { i + 1; }", "6"),
        (
            "for 1..=5 -> i { next if i == 3; last if i == 4; } 42;",
            "42",
        ),
        ("let b = true; if !b { 0; } else { 1; }", "1"),
        // Ошибка в невыбранной ветви не возникает.
        ("let x = 1; if x == 1 { 42; } else { 1 << 64; }", "42"),
        (
            "if 999999999999999999999999999999 > 100000000000000000000000000000 { 42; } else { 0; }",
            "42",
        ),
    ] {
        let module = lower(body);
        assert_eq!(execute(&module).unwrap().to_string(), expected, "{body}");
    }
}

#[test]
fn every_comparison_selects_the_correct_branch() {
    for (op, expected) in [
        ("==", "0"),
        ("!=", "1"),
        ("<", "1"),
        ("<=", "1"),
        (">", "0"),
        (">=", "0"),
    ] {
        let module = lower(&format!("if 2 {op} 3 {{ 1; }} else {{ 0; }}"));
        assert_eq!(execute(&module).unwrap().to_string(), expected, "{op}");
    }
    for (op, expected) in [("==", "0"), ("!=", "1")] {
        assert_eq!(
            execute(&lower(&format!(
                "if true {op} false {{ 1; }} else {{ 0; }}"
            )))
            .unwrap()
            .to_string(),
            expected
        );
    }
}

#[test]
fn invalid_ir_is_rejected_before_execution() {
    let mut module = lower("1 + 1;");
    let function = &mut module.functions[module.entry.unwrap().0];
    function.blocks[0].term.kind = ir::TermKind::Ret(Some(ir::ValueId(999)));
    assert!(matches!(execute(&module), Err(RuntimeError::InvalidIr(_))));
}

#[test]
fn loop_edge_swaps_parameters_simultaneously() {
    use ir::{
        Block, BlockId, Constant, Inst, InstKind, Target, TermKind, Terminator, Type, ValueId,
    };
    let mut module = lower("0;");
    let function = &mut module.functions[module.entry.unwrap().0];
    let span = function.span;
    let target = |block, args: &[usize]| Target {
        block: BlockId(block),
        args: args.iter().copied().map(ValueId).collect(),
    };
    let inst = |id, kind| Inst {
        result: Some(ValueId(id)),
        kind,
        span,
    };
    let block = |params: &[usize], insts, kind| Block {
        params: params.iter().copied().map(ValueId).collect(),
        unwind: None,
        insts,
        term: Terminator { kind, span },
    };
    function.values = vec![
        Type::Int,
        Type::Int,
        Type::Bool,
        Type::Int,
        Type::Int,
        Type::Bool,
        Type::Bool,
    ];
    function.blocks = vec![
        block(
            &[],
            vec![
                inst(0, InstKind::Const(Constant::Int(ir::IntValue::from_i64(1)))),
                inst(1, InstKind::Const(Constant::Int(ir::IntValue::from_i64(2)))),
                inst(2, InstKind::Const(Constant::Bool(false))),
            ],
            TermKind::Br(target(1, &[0, 1, 2])),
        ),
        block(
            &[3, 4, 5],
            vec![inst(6, InstKind::BoolNot(ValueId(5)))],
            TermKind::Cond {
                condition: ValueId(5),
                then_target: target(2, &[]),
                else_target: target(1, &[4, 3, 6]),
            },
        ),
        block(&[], vec![], TermKind::Ret(Some(ValueId(4)))),
    ];
    // После одного оборота (1, 2) стало (2, 1). Последовательная запись дала бы (2, 2).
    assert_eq!(execute(&module).unwrap().to_string(), "1");
}
