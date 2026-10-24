//! Вызовы проверенного OIR: кадры, аргументы, продолжение и рекурсия.
//! Свободные helper-функции существуют в IR; исходный синтаксис их не требует.
use okkama_compiler::{SourceFile, ir::*, lowering, source::Span, syntax};
use okkama_runtime::{RuntimeError, execute};

const SPAN: Span = Span { start: 0, end: 1 };

fn inst(result: Option<usize>, kind: InstKind) -> Inst {
    Inst {
        result: result.map(ValueId),
        kind,
        span: SPAN,
    }
}

fn int(result: usize, value: i64) -> Inst {
    inst(
        Some(result),
        InstKind::Const(Constant::Int(IntValue::from_i64(value))),
    )
}

fn call(result: Option<usize>, func: usize, args: &[usize]) -> Inst {
    inst(
        result,
        InstKind::Call {
            func: FuncId(func),
            args: args.iter().copied().map(ValueId).collect(),
        },
    )
}

fn add(result: usize, left: usize, right: usize) -> Inst {
    inst(
        Some(result),
        InstKind::IntBinary {
            op: IntBinaryOp::Add,
            left: ValueId(left),
            right: ValueId(right),
        },
    )
}

fn block(insts: Vec<Inst>, kind: TermKind) -> Block {
    Block {
        params: vec![],
        unwind: None,
        insts,
        term: Terminator { kind, span: SPAN },
    }
}

fn ret(value: usize) -> TermKind {
    TermKind::Ret(Some(ValueId(value)))
}

fn function(
    name: &str,
    params: &[usize],
    result: ResultType,
    values: Vec<Type>,
    blocks: Vec<Block>,
) -> Function {
    Function {
        symbol: name.into(),
        params: params.iter().copied().map(ValueId).collect(),
        result,
        attrs: vec![],
        values,
        blocks,
        span: SPAN,
    }
}

fn module(mut functions: Vec<Function>) -> Module {
    let source = SourceFile::new("test.ok", "module app v1.0.0; main() is int { 0; }").unwrap();
    let ast = syntax::parse(&source, &syntax::ParseOptions::default()).unwrap();
    let mut module = lowering::lower_program(&ast).unwrap();
    functions[0].attrs.push(Attr::Entry);
    module.functions = functions;
    module.entry = Some(FuncId(0));
    module
}

#[test]
fn string_arguments_and_results_survive_serialized_calls() {
    let program = module(vec![
        function(
            "main",
            &[],
            ResultType::Value(Type::Int),
            vec![Type::Str, Type::Str, Type::Int],
            vec![block(
                vec![
                    inst(Some(0), InstKind::Const(Constant::Str("hello".into()))),
                    call(Some(1), 1, &[0]),
                    inst(
                        Some(2),
                        InstKind::Str {
                            op: StrOp::Len,
                            args: vec![ValueId(1)],
                        },
                    ),
                ],
                ret(2),
            )],
        ),
        function(
            "join",
            &[0],
            ResultType::Value(Type::Str),
            vec![Type::Str; 3],
            vec![block(
                vec![
                    inst(Some(1), InstKind::Const(Constant::Str("!".into()))),
                    inst(
                        Some(2),
                        InstKind::StrConcat {
                            left: ValueId(0),
                            right: ValueId(1),
                        },
                    ),
                ],
                ret(2),
            )],
        ),
    ]);
    assert_eq!(execute(&program).unwrap().to_string(), "6");
    assert_sparrow(&program, "6");
}

#[test]
fn nested_calls_keep_locals_and_resume_after_the_call() {
    let program = module(vec![
        function(
            "main",
            &[],
            ResultType::Value(Type::Int),
            vec![Type::Int; 4],
            vec![block(
                vec![
                    int(0, 10),
                    call(Some(1), 1, &[0]),
                    call(Some(2), 1, &[1]),
                    add(3, 0, 2),
                ],
                ret(3),
            )],
        ),
        function(
            "twice",
            &[0],
            ResultType::Value(Type::Int),
            vec![Type::Int; 3],
            vec![block(vec![call(Some(1), 2, &[0]), add(2, 0, 1)], ret(2))],
        ),
        function(
            "identity",
            &[0],
            ResultType::Value(Type::Int),
            vec![Type::Int],
            vec![block(vec![], ret(0))],
        ),
    ]);
    assert_eq!(execute(&program).unwrap().to_string(), "50");
    assert_sparrow(&program, "50");
    assert_eq!(execute(&program).unwrap().to_string(), "50");
}

#[test]
fn void_and_bool_calls_return_to_the_caller() {
    let program = module(vec![
        function(
            "main",
            &[],
            ResultType::Value(Type::Int),
            vec![Type::Bool, Type::Int, Type::Int],
            vec![
                block(
                    vec![call(None, 1, &[]), call(Some(0), 2, &[])],
                    TermKind::Cond {
                        condition: ValueId(0),
                        then_target: Target::new(BlockId(1)),
                        else_target: Target::new(BlockId(2)),
                    },
                ),
                block(vec![int(1, 42)], ret(1)),
                block(vec![int(2, 0)], ret(2)),
            ],
        ),
        function(
            "nothing",
            &[],
            ResultType::Void,
            vec![],
            vec![block(vec![], TermKind::Ret(None))],
        ),
        function(
            "yes",
            &[],
            ResultType::Value(Type::Bool),
            vec![Type::Bool],
            vec![block(
                vec![inst(Some(0), InstKind::Const(Constant::Bool(true)))],
                ret(0),
            )],
        ),
    ]);
    assert_eq!(execute(&program).unwrap().to_string(), "42");
    assert_sparrow(&program, "42");
}

#[test]
fn deep_recursion_uses_the_explicit_stack_and_unwinds_results() {
    let program = module(vec![
        function(
            "main",
            &[],
            ResultType::Value(Type::Int),
            vec![Type::Int; 2],
            vec![block(vec![int(0, 0), call(Some(1), 1, &[0])], ret(1))],
        ),
        function(
            "count",
            &[0],
            ResultType::Value(Type::Int),
            vec![
                Type::Int,
                Type::Int,
                Type::Bool,
                Type::Int,
                Type::Int,
                Type::Int,
                Type::Int,
                Type::Int,
            ],
            vec![
                block(
                    vec![
                        int(1, 20000),
                        inst(
                            Some(2),
                            InstKind::IntCompare {
                                op: CompareOp::Lt,
                                left: ValueId(0),
                                right: ValueId(1),
                            },
                        ),
                    ],
                    TermKind::Cond {
                        condition: ValueId(2),
                        then_target: Target::new(BlockId(1)),
                        else_target: Target::new(BlockId(2)),
                    },
                ),
                block(
                    vec![
                        int(3, 1),
                        add(4, 0, 3),
                        call(Some(5), 1, &[4]),
                        add(6, 5, 3),
                    ],
                    ret(6),
                ),
                block(vec![int(7, 0)], ret(7)),
            ],
        ),
    ]);
    assert_eq!(execute(&program).unwrap().to_string(), "20000");
    assert_sparrow(&program, "20000");
}

#[test]
fn errors_in_a_callee_return_to_the_host_and_do_not_poison_later_runs() {
    let mut program = module(vec![
        function(
            "main",
            &[],
            ResultType::Value(Type::Int),
            vec![Type::Int],
            vec![block(vec![call(Some(0), 1, &[])], ret(0))],
        ),
        function(
            "helper",
            &[],
            ResultType::Value(Type::Int),
            vec![Type::Int; 3],
            vec![block(
                vec![
                    int(0, 2),
                    int(1, 64),
                    inst(
                        Some(2),
                        InstKind::IntBinary {
                            op: IntBinaryOp::Shl,
                            left: ValueId(0),
                            right: ValueId(1),
                        },
                    ),
                ],
                ret(2),
            )],
        ),
    ]);
    assert!(matches!(
        execute(&program),
        Err(RuntimeError::Trap { span: SPAN, message }) if message == "Shift count must be in 0..63"
    ));
    program.functions[1].blocks[0].insts[2] = add(2, 0, 1);
    assert_eq!(execute(&program).unwrap().to_string(), "66");
    assert_sparrow(&program, "66");
    // Несовпадение аргументов отвергает верификатор до запуска.
    program.functions[0].blocks[0].insts[0] = call(Some(0), 1, &[0]);
    assert!(matches!(execute(&program), Err(RuntimeError::InvalidIr(_))));
}

fn assert_sparrow(program: &Module, expected: &str) {
    let bytecode = okkama_compiler::codegen::sparrow::generate(program).unwrap();
    let bytes = okkama_sparrow::encode(&bytecode).unwrap();
    assert_eq!(&bytes[4..6], &okkama_sparrow::VERSION.to_be_bytes());
    let loaded = okkama_sparrow::decode(&bytes).unwrap();
    assert_eq!(
        okkama_runtime::sparrow::execute(&loaded).unwrap().to_string(),
        expected
    );
}
