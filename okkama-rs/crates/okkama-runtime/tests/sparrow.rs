use okkama_compiler::{SourceFile, codegen, ir, lowering, syntax};
use okkama_runtime::{execute, sparrow};
use okkama_sparrow::{Constant, Dst, FuncRef, Function, Instruction::*, Program, Register as R, Type, TypeRef};

/// Программа SPB 9 из одного `main` над `registers` регистрами `int`.
fn handmade(registers: usize, code: Vec<okkama_sparrow::Instruction>) -> Program {
    Program {
        module: "app".into(),
        version: "1.0.0".into(),
        types: vec![Type::Int],
        functions: vec![Function {
            symbol: "main".into(),
            is_async: false,
            native: None,
            intrinsic: None,
            registers: vec![TypeRef(0); registers],
            params: Vec::new(),
            result: Some(TypeRef(0)),
            code,
            handlers: Vec::new(),
        }],
        entry: Some(FuncRef(0)),
        ..Program::default()
    }
}

fn int(text: &str) -> Constant {
    Constant::Int(text.into())
}

fn d(index: u32) -> Dst {
    Dst(R(index))
}

fn lower(body: &str) -> ir::Module {
    let source = SourceFile::new(
        "sparrow.ok",
        format!("module app v1.0.0; main() is int {{ {body} }}"),
    )
    .unwrap();
    let ast = syntax::parse(&source, &syntax::ParseOptions::default()).unwrap();
    lowering::lower_program(&ast).unwrap()
}

#[test]
fn source_ir_sparrow_and_vm_agree_on_exact_arithmetic() {
    for (body, expected) in [
        ("1 + 1;", "2"),
        ("6 * 7;", "42"),
        ("-6 * 7;", "-42"),
        ("-6 * -7;", "42"),
        ("-7 % 3;", "-1"),
        ("7 % -3;", "1"),
        ("-7 % -3;", "-1"),
        ("7 % 3;", "1"),
        ("-6 % 3;", "0"),
        ("1 % 999999999999999999999999999999999999999999;", "1"),
        ("-999999999999999999999999999999999999999999 % 10;", "-9"),
        (
            "1000000000000000000000000000000000000000001 % 100000000000000000000;",
            "1",
        ),
        ("0 * -999999999999999999999999999999;", "0"),
        (
            "99999999999999999999 * 99999999999999999999;",
            "9999999999999999999800000000000000000001",
        ),
        ("let a = 20; let b is int = 22; a + b;", "42"),
        ("let a = 3; a + a + 1;", "7"),
        ("0;", "0"),
        ("let a = 40; a - -2;", "42"),
        ("-0;", "0"),
        ("1 - 3;", "-2"),
        (
            "-(-999999999999999999999999999999 + 1);",
            "999999999999999999999999999998",
        ),
        ("9223372036854775807 + 1;", "9223372036854775808"),
        (
            "999999999999999999999999999999999999999999 + 1;",
            "1000000000000000000000000000000000000000000",
        ),
    ] {
        let module = lower(body);
        let program = codegen::sparrow::generate(&module).unwrap();
        let bytes = okkama_sparrow::encode(&program).unwrap();
        let loaded = okkama_sparrow::decode(&bytes).unwrap();
        assert_eq!(sparrow::execute(&loaded).unwrap().to_string(), expected);
        assert_eq!(
            sparrow::execute(&program).unwrap().to_string(),
            expected,
            "{body}"
        );
        assert_eq!(
            sparrow::execute(&program).unwrap(),
            execute(&module).unwrap()
        );
        assert_eq!(program, codegen::sparrow::generate(&module).unwrap());
    }
}

#[test]
fn modulo_by_zero_fails_only_on_the_executed_path() {
    let module = lower("7 % 0;");
    assert!(
        matches!(execute(&module), Err(okkama_runtime::RuntimeError::Trap { message, .. }) if message == "Modulo by zero")
    );
    let bytes = okkama_sparrow::encode(&codegen::sparrow::generate(&module).unwrap()).unwrap();
    assert!(
        matches!(sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap()),
        Err(sparrow::Error::Operation { error: okkama_runtime::RuntimeError::Trap { message, .. }, .. }) if message == "Modulo by zero")
    );
    let module = lower("if false { 7 % 0; } else { 42; }");
    assert_eq!(execute(&module).unwrap().to_string(), "42");
    assert_eq!(
        sparrow::execute(&codegen::sparrow::generate(&module).unwrap())
            .unwrap()
            .to_string(),
        "42"
    );
}

#[test]
fn subtraction_and_negation_read_before_overwriting() {
    let program = handmade(
        1,
        vec![
            Const { dst: d(0), value: int("99") },
            IntNeg { dst: d(0), operand: R(0) },
            IntSub { dst: d(0), left: R(0), right: R(0) },
            Return { value: Some(R(0)) },
        ],
    );
    let bytes = okkama_sparrow::encode(&program).unwrap();
    assert_eq!(okkama_sparrow::decode(&bytes).unwrap(), program);
    assert_eq!(sparrow::execute(&program).unwrap().to_string(), "0");
    for instruction in [
        IntNeg { dst: d(0), operand: R(0) },
        IntSub { dst: d(0), left: R(0), right: R(0) },
        IntNeg { dst: d(1), operand: R(0) },
    ] {
        let mut invalid = program.clone();
        invalid.functions[0].code[0] = instruction;
        assert!(invalid.verify().is_err());
    }
}

#[test]
fn one_plus_one_lowers_to_register_code() {
    let program = codegen::sparrow::generate(&lower("1 + 1;")).unwrap();
    let main = &program.functions[program.entry.unwrap().0 as usize];
    assert_eq!(
        main.code,
        vec![
            Const { dst: d(0), value: int("1") },
            Const { dst: d(1), value: int("1") },
            IntAdd { dst: d(2), left: R(0), right: R(1) },
            Return { value: Some(R(2)) },
        ]
    );
}

#[test]
fn register_reuse_and_signed_constants_work_without_ir_execution() {
    let program = handmade(
        201,
        vec![
            Const { dst: d(100), value: int("-999999999999999999999999") },
            Const { dst: d(200), value: int("1") },
            IntAdd { dst: d(100), left: R(100), right: R(200) },
            Return { value: Some(R(100)) },
        ],
    );
    assert_eq!(sparrow::execute(&program).unwrap().to_string(), "-999999999999999999999998");
}

#[test]
fn malformed_sparrow_is_rejected_before_register_access() {
    let mut program = codegen::sparrow::generate(&lower("1 + 1;")).unwrap();
    let main = program.entry.unwrap().0 as usize;
    program.functions[main].code[0] = Const { dst: d(999), value: int("1") };
    assert!(matches!(sparrow::execute(&program), Err(sparrow::Error::InvalidProgram(_))));
    program.functions[main].code = vec![Return { value: Some(R(0)) }];
    assert!(matches!(sparrow::execute(&program), Err(sparrow::Error::InvalidProgram(_))));
}

#[test]
fn generation_covers_floats_and_rationals_and_rejects_invalid_ir() {
    for body in ["let text = 1.5; 2;", "let x = 1 / 2; 0;", "(1.5 * 4.0).int();", "(7 / 2 + 1 / 2).int();"] {
        let module = lower(body);
        let program = codegen::sparrow::generate(&module).unwrap_or_else(|error| panic!("{body}: {error}"));
        assert_eq!(sparrow::execute(&program).unwrap(), execute(&module).unwrap(), "{body}");
    }
    let mut module = lower("1;");
    module.functions[0].blocks[0].term.kind = ir::TermKind::Ret(Some(ir::ValueId(999)));
    assert!(codegen::sparrow::generate(&module).unwrap_err().message.contains("invalid IR"));
    let mut module = lower("1;");
    let mut extra = module.functions[0].clone();
    extra.attrs.clear();
    extra.symbol = "helper".into();
    module.functions.push(extra);
    let compiled = codegen::sparrow::generate(&module).unwrap();
    assert_eq!(compiled.functions.len(), 2);
    assert_eq!(sparrow::execute(&compiled).unwrap().to_string(), "1");
    module.functions.swap(0, 1);
    module.entry = Some(ir::FuncId(1));
    assert_eq!(sparrow::execute(&codegen::sparrow::generate(&module).unwrap()).unwrap().to_string(), "1");
}

#[test]
fn control_flow_matches_ir_before_and_after_serialization() {
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
        (
            "if 999999999999999999999999999999 > 100000000000000000000000000000 { 42; } else { 0; }",
            "42",
        ),
        ("if false && true { 0; } else { 42; }", "42"),
        ("if false || true { 42; } else { 0; }", "42"),
    ] {
        let module = lower(body);
        let program = codegen::sparrow::generate(&module).unwrap_or_else(|e| panic!("{body}: {e}"));
        let bytes = okkama_sparrow::encode(&program).unwrap();
        let loaded = okkama_sparrow::decode(&bytes).unwrap();
        assert_eq!(
            sparrow::execute(&loaded).unwrap().to_string(),
            expected,
            "{body}"
        );
        assert_eq!(
            sparrow::execute(&loaded).unwrap(),
            execute(&module).unwrap()
        );
    }
    for op in ["==", "!=", "<", "<=", ">", ">="] {
        let module = lower(&format!("if 2 {op} 3 {{ 1; }} else {{ 0; }}"));
        let program = codegen::sparrow::generate(&module).unwrap();
        assert_eq!(
            sparrow::execute(&program).unwrap(),
            execute(&module).unwrap()
        );
    }
    for op in ["==", "!="] {
        let module = lower(&format!("if true {op} false {{ 1; }} else {{ 0; }}"));
        let program = codegen::sparrow::generate(&module).unwrap();
        assert_eq!(
            sparrow::execute(&program).unwrap(),
            execute(&module).unwrap()
        );
    }
}

#[test]
fn selected_loop_traps_preserve_messages() {
    for bound in ["0", "1"] {
        let mut module = lower("for 1..5 -> i { i; }");
        let function = &mut module.functions[module.entry.unwrap().0];
        function.blocks[0].insts[1].kind =
            ir::InstKind::Const(ir::Constant::Int(ir::IntValue::parse(bound).unwrap()));
        let program = codegen::sparrow::generate(&module).unwrap();
        let bytes = okkama_sparrow::encode(&program).unwrap();
        let error = sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap()).unwrap_err();
        let sparrow::Error::Operation { error, .. } = error else {
            panic!("expected runtime error")
        };
        let ir_error = execute(&module).unwrap_err();
        assert_eq!(error.to_string(), ir_error.to_string());
    }
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
    let program = codegen::sparrow::generate(&module).unwrap();
    let bytes = okkama_sparrow::encode(&program).unwrap();
    assert_eq!(
        sparrow::execute(&okkama_sparrow::decode(&bytes).unwrap())
            .unwrap()
            .to_string(),
        "1"
    );
}

#[test]
fn values_of_separate_branches_share_registers() {
    let body = "let x = 3; let a = if x == 1 { 10 + x; } else { if x == 2 { 20 + x; } else { if x == 3 { 30 + x; } else { 40 + x; } } }; let b = a * 2; b + a;";
    let module = lower(body);
    let program = codegen::sparrow::generate(&module).unwrap();
    let main = &program.functions[program.entry.unwrap().0 as usize];
    let values = module.functions.iter().find(|function| function.symbol == "main").unwrap().values.len();
    assert!(main.registers.len() < values, "{} registers for {values} values", main.registers.len());
    assert_eq!(sparrow::execute(&program).unwrap().to_string(), "99");
    assert_eq!(sparrow::execute(&program).unwrap(), execute(&module).unwrap());
}
