//! Verifier rules of OIR-1 §17 on hand-built modules.

use okkama_compiler::Span;
use okkama_compiler::ir::{
    self, Attr, Block, BlockId, CompareOp, Constant, FuncId, Function, Inst, InstKind, IntValue,
    Module, ResultType, Target, TermKind, Terminator, Type, ValueId,
};

fn inst(result: usize, kind: InstKind) -> Inst {
    Inst {
        result: Some(ValueId(result)),
        kind,
        span: Span::empty(0),
    }
}

fn int(value: i64) -> InstKind {
    InstKind::Const(Constant::Int(IntValue::from_i64(value)))
}

fn term(kind: TermKind) -> Terminator {
    Terminator {
        kind,
        span: Span::empty(0),
    }
}

fn block(insts: Vec<Inst>, kind: TermKind) -> Block {
    Block {
        params: Vec::new(),
        unwind: None,
        insts,
        term: term(kind),
    }
}

fn module(values: Vec<Type>, blocks: Vec<Block>) -> Module {
    Module {
        name: "app".to_string(),
        version: "v1.0.0".to_string(),
        imports: Vec::new(),
        enums: Vec::new(),
        roles: Vec::new(),
        objects: Vec::new(),
        handles: Vec::new(),
        caps: Vec::new(),
        tests: None,
        functions: vec![Function {
            symbol: "main".to_string(),
            params: Vec::new(),
            result: ResultType::Value(Type::Int),
            attrs: vec![Attr::Entry],
            values,
            blocks,
            span: Span::empty(0),
        }],
        entry: Some(FuncId(0)),
        span: Span::empty(0),
    }
}

/// `if %cond { 1 } else { 2 }` с параметром слияния: корректная форма.
fn diamond() -> Module {
    module(
        vec![Type::Bool, Type::Int, Type::Int, Type::Int],
        vec![
            block(
                vec![inst(0, InstKind::Const(Constant::Bool(true)))],
                TermKind::Cond {
                    condition: ValueId(0),
                    then_target: Target::new(BlockId(1)),
                    else_target: Target::new(BlockId(2)),
                },
            ),
            block(
                vec![inst(1, int(1))],
                TermKind::Br(Target {
                    block: BlockId(3),
                    args: vec![ValueId(1)],
                }),
            ),
            block(
                vec![inst(2, int(2))],
                TermKind::Br(Target {
                    block: BlockId(3),
                    args: vec![ValueId(2)],
                }),
            ),
            Block {
                params: vec![ValueId(3)],
                unwind: None,
                insts: Vec::new(),
                term: term(TermKind::Ret(Some(ValueId(3)))),
            },
        ],
    )
}

fn rejects(module: &Module, expected: &str) {
    let error = ir::verify(module).expect_err(expected);
    assert!(
        error.to_string().contains(expected),
        "expected `{expected}`, got `{error}`"
    );
}

#[test]
fn a_well_formed_diamond_verifies() {
    ir::verify(&diamond()).unwrap();
}

#[test]
fn uses_must_be_dominated_by_their_definitions() {
    let mut module = diamond();
    // ^b2 читает %1 из соседней ветки ^b1.
    module.functions[0].blocks[2].term = term(TermKind::Br(Target {
        block: BlockId(3),
        args: vec![ValueId(1)],
    }));
    rejects(
        &module,
        "%1 is used in ^b2 where its definition does not dominate",
    );

    let mut module = diamond();
    // Использование до определения в том же блоке.
    module.functions[0].blocks[1].insts = vec![inst(
        1,
        InstKind::IntCompare {
            op: CompareOp::Eq,
            left: ValueId(1),
            right: ValueId(1),
        },
    )];
    rejects(&module, "does not dominate");
}

#[test]
fn values_are_defined_exactly_once_and_typed() {
    let mut module = diamond();
    module.functions[0].blocks[2].insts = vec![inst(1, int(2))];
    rejects(&module, "%1 is defined more than once");

    let mut module = diamond();
    module.functions[0].values.push(Type::Int);
    rejects(&module, "%4 is declared but never defined");

    let mut module = diamond();
    // Неиспользуемая константа int, объявленная как bool: нарушено только это правило.
    module.functions[0].values.push(Type::Bool);
    module.functions[0].blocks[1].insts.push(inst(4, int(5)));
    rejects(&module, "%4: bool cannot hold the int result");

    let mut module = diamond();
    module.functions[0].blocks[0].insts = vec![inst(0, int(1))];
    module.functions[0].values[0] = Type::Int;
    rejects(&module, "%0 has type int, expected bool");

    let mut module = diamond();
    module.functions[0].blocks[1].insts = vec![inst(
        1,
        InstKind::Const(Constant::Int(IntValue::from_decimal_digits("7").unwrap())),
    )];
    ir::verify(&module).unwrap();
}

#[test]
fn branches_must_match_target_parameters() {
    let mut module = diamond();
    module.functions[0].blocks[1].term = term(TermKind::Br(Target::new(BlockId(3))));
    rejects(&module, "passes 0 arguments for 1 parameters");

    let mut module = diamond();
    module.functions[0].blocks[1].term = term(TermKind::Br(Target {
        block: BlockId(3),
        args: vec![ValueId(0)],
    }));
    rejects(&module, "passes %0: bool for %3: int");

    let mut module = diamond();
    module.functions[0].blocks[1].term = term(TermKind::Br(Target::new(BlockId(9))));
    rejects(&module, "branch to missing block ^b9");

    let mut module = diamond();
    module.functions[0].blocks[1].term = term(TermKind::Br(Target::new(BlockId(0))));
    rejects(&module, "the entry block cannot be a branch target");
}

#[test]
fn every_block_must_be_reachable() {
    let mut module = diamond();
    module.functions[0]
        .blocks
        .push(block(Vec::new(), TermKind::Unreachable));
    rejects(&module, "block ^b4 is unreachable");
}

#[test]
fn results_and_entry_signature_are_checked() {
    let mut module = diamond();
    module.functions[0].blocks[3].term = term(TermKind::Ret(None));
    rejects(&module, "`ret` without a value");

    let mut module = diamond();
    module.functions[0].result = ResultType::Void;
    rejects(&module, "entry @main must have signature () -> int");

    let mut module = diamond();
    module.functions[0].attrs.clear();
    rejects(
        &module,
        "entry attribute must mark exactly the module entry",
    );

    let mut module = diamond();
    module.functions.push(module.functions[0].clone());
    module.functions[1].attrs.clear();
    rejects(&module, "duplicate function symbol @main");

    let mut module = diamond();
    module.functions[0].blocks[3].term = term(TermKind::Panic(ValueId(3)));
    rejects(&module, "%3 has type int, expected str");
}

fn lowered(text: &str) -> Module {
    use okkama_compiler::SourceFile;
    use okkama_compiler::lowering::lower_program;
    use okkama_compiler::syntax::{self, ParseOptions};
    let source = SourceFile::new("test.ok", text).unwrap();
    let program = syntax::parse(&source, &ParseOptions::default()).unwrap();
    lower_program(&program).unwrap()
}

const OBJECTS: &str = "module app v1.0.0;
object C {
    has n is int = 0;
    C() {}
    inc() is ^C { ^n = n + 1; }
    value() is int { n; }
    private:
    secret() is int { n; }
}
main() is int { let d = C().inc(); d.value(); }";

fn symbol(module: &Module, name: &str) -> usize {
    module
        .functions
        .iter()
        .position(|function| function.symbol == name)
        .unwrap_or_else(|| panic!("@{name}"))
}

#[test]
fn object_instructions_require_the_owner_authority() {
    let mut module = lowered(OBJECTS);
    ir::verify(&module).unwrap();
    let value = symbol(&module, "C.value");
    module.functions[value].attrs = Vec::new();
    rejects(
        &module,
        "obj.get is allowed only in functions of its object",
    );

    let mut module = lowered(OBJECTS);
    let ctor = symbol(&module, "C.new#0");
    let block = &mut module.functions[ctor].blocks[0];
    let InstKind::ObjNew { fields, .. } = &mut block.insts.last_mut().unwrap().kind else {
        panic!("constructor ends with obj.new")
    };
    fields.clear();
    rejects(&module, "obj.new C lists 0 fields, the object declares 1");

    let mut module = lowered(OBJECTS);
    let inc = symbol(&module, "C.inc");
    module.functions[inc].attrs = vec![Attr::Method(ir::ObjectId(0))];
    rejects(&module, "a method returning ^T must be marked `successor`");
}

#[test]
fn calls_check_arguments_and_privacy() {
    let mut module = lowered(OBJECTS);
    let main = symbol(&module, "main");
    let secret = symbol(&module, "C.secret");
    let value = symbol(&module, "C.value");
    for inst in &mut module.functions[main].blocks[0].insts {
        if let InstKind::Call { func, .. } = &mut inst.kind {
            if func.0 == value {
                *func = FuncId(secret);
            }
        }
    }
    rejects(&module, "@C.secret is private to its object");

    let mut module = lowered(OBJECTS);
    let main = symbol(&module, "main");
    for inst in &mut module.functions[main].blocks[0].insts {
        if let InstKind::Call { args, .. } = &mut inst.kind {
            if !args.is_empty() {
                args.clear();
            }
        }
    }
    rejects(&module, "passes 0 arguments for 1 parameters");
}

#[test]
fn successor_results_must_be_taken_on_every_normal_path() {
    let mut module = lowered(OBJECTS);
    let main = symbol(&module, "main");
    let (ctor, inc) = (symbol(&module, "C.new#0"), symbol(&module, "C.inc"));
    let object = Type::Object(ir::ObjectId(0));
    // %1: ^C получен и брошен: обязательство доходит до `ret`.
    let function = &mut module.functions[main];
    function.values = vec![object.clone(), Type::Successor(ir::ObjectId(0)), Type::Int];
    function.blocks = vec![block(
        vec![
            inst(
                0,
                InstKind::Call {
                    func: FuncId(ctor),
                    args: vec![],
                },
            ),
            inst(
                1,
                InstKind::Call {
                    func: FuncId(inc),
                    args: vec![ValueId(0)],
                },
            ),
            inst(2, int(0)),
        ],
        TermKind::Ret(Some(ValueId(2))),
    )];
    rejects(
        &module,
        "successor value %1 reaches `ret` in ^b0 without succ.take",
    );

    // Погашение на одном пути не покрывает другой.
    let function = &mut module.functions[main];
    function.values.extend([Type::Bool, object]);
    function.blocks = vec![
        block(
            vec![
                inst(
                    0,
                    InstKind::Call {
                        func: FuncId(ctor),
                        args: vec![],
                    },
                ),
                inst(
                    1,
                    InstKind::Call {
                        func: FuncId(inc),
                        args: vec![ValueId(0)],
                    },
                ),
                inst(2, int(0)),
                inst(3, InstKind::Const(Constant::Bool(true))),
            ],
            TermKind::Cond {
                condition: ValueId(3),
                then_target: Target::new(BlockId(1)),
                else_target: Target::new(BlockId(2)),
            },
        ),
        block(
            vec![inst(4, InstKind::SuccTake(ValueId(1)))],
            TermKind::Ret(Some(ValueId(2))),
        ),
        block(Vec::new(), TermKind::Ret(Some(ValueId(2)))),
    ];
    rejects(
        &module,
        "successor value %1 reaches `ret` in ^b2 without succ.take",
    );
}

const FAILURES: &str = "module app v1.0.0;
object Boom does Error { Boom(); text() is string { \"boom\"; } }
object C { has n is int = 0; C() {} inc() is ^C { ^n = n + 1; } value() is int { n; } }
main() is int {
    let c = C();
    let r = try { c = c.inc(); throw Boom(); } catch e is Boom { 7; };
    r + c.value();
}";

#[test]
fn unwind_arguments_must_exist_before_the_source_block_starts() {
    let mut module = lowered(FAILURES);
    ir::verify(&module).unwrap();
    let main = symbol(&module, "main");
    // ^b1 определяет %2 (succ.take) и сам не может передать его в свою цель unwind.
    let block = &mut module.functions[main].blocks[1];
    block.unwind.as_mut().unwrap().args = vec![ValueId(2)];
    rejects(
        &module,
        "%2 is passed to the unwind target of ^b1 but is not defined before the block starts",
    );

    let mut module = lowered(FAILURES);
    let main = symbol(&module, "main");
    module.functions[main].blocks[1]
        .unwind
        .as_mut()
        .unwrap()
        .args
        .clear();
    rejects(&module, "passes 1 arguments for 2 parameters");
}

#[test]
fn catch_handlers_receive_the_caught_type_and_throw_requires_error() {
    let mut module = lowered(FAILURES);
    let main = symbol(&module, "main");
    // Последний параметр handler должен иметь тип пойманного объекта.
    let handler = module.functions[main].blocks[4].params[2];
    module.functions[main].values[handler.0] = Type::Int;
    rejects(
        &module,
        &format!("the last parameter {handler} of ^b4 must have type object#0"),
    );

    let mut module = lowered(FAILURES);
    let main = symbol(&module, "main");
    assert!(matches!(
        module.functions[main].blocks[2].term.kind,
        TermKind::Throw(_)
    ));
    module.functions[main].blocks[2].term.kind = TermKind::Throw(ValueId(0));
    rejects(&module, "throw %0: object#1 does not do Error");
}

/// `%b` — сборщик из блока 0; `then`/`else` — тела блоков 1 и 2.
fn builder_branches(then_insts: Vec<Inst>, else_insts: Vec<Inst>) -> Module {
    let builder = Type::Builder(Box::new(Type::Int));
    let array = Type::Array(Box::new(Type::Int));
    module(
        vec![
            builder.clone(),
            Type::Int,
            Type::Bool,
            builder.clone(),
            array.clone(),
            Type::Int,
            builder,
            array,
            Type::Int,
        ],
        vec![
            block(
                vec![
                    inst(0, InstKind::BuilderNew),
                    inst(1, int(1)),
                    inst(2, InstKind::Const(Constant::Bool(true))),
                ],
                TermKind::Cond {
                    condition: ValueId(2),
                    then_target: Target::new(BlockId(1)),
                    else_target: Target::new(BlockId(2)),
                },
            ),
            block(then_insts, TermKind::Ret(Some(ValueId(5)))),
            block(else_insts, TermKind::Ret(Some(ValueId(8)))),
        ],
    )
}

#[test]
fn builders_are_used_at_most_once_on_every_path() {
    let push = |result, builder| {
        inst(
            result,
            InstKind::BuilderPush {
                builder: ValueId(builder),
                value: ValueId(1),
            },
        )
    };
    let finish = |result, builder, len| {
        [
            inst(result, InstKind::BuilderFinish(ValueId(builder))),
            inst(len, InstKind::ArrayLen(ValueId(result))),
        ]
    };
    // Взаимоисключающие пути: `%0` дописан в одной ветви и закрыт в другой.
    let mut then_insts = vec![push(3, 0)];
    then_insts.extend(finish(4, 3, 5));
    let mut else_insts = vec![push(6, 0)];
    else_insts.extend(finish(7, 6, 8));
    ir::verify(&builder_branches(then_insts, else_insts)).unwrap();

    // Два использования на одном пути.
    let mut reused = vec![push(3, 0), push(6, 0)];
    reused.extend(finish(4, 3, 5));
    rejects(
        &builder_branches(reused, finish(7, 0, 8).to_vec()),
        "builder %0 is used more than once on a path",
    );

    // Использование в цикле без параметра заголовка повторяется на каждой итерации.
    let builder = Type::Builder(Box::new(Type::Int));
    let looped = module(
        vec![builder.clone(), Type::Int, Type::Bool, builder, Type::Int],
        vec![
            block(
                vec![
                    inst(0, InstKind::BuilderNew),
                    inst(1, int(1)),
                    inst(2, InstKind::Const(Constant::Bool(true))),
                ],
                TermKind::Br(Target::new(BlockId(1))),
            ),
            block(
                vec![push(3, 0)],
                TermKind::Cond {
                    condition: ValueId(2),
                    then_target: Target::new(BlockId(1)),
                    else_target: Target::new(BlockId(2)),
                },
            ),
            block(vec![inst(4, int(0))], TermKind::Ret(Some(ValueId(4)))),
        ],
    );
    rejects(&looped, "builder %0 is used more than once on a path");
}

#[test]
fn map_keys_are_int_bool_or_str() {
    let bad = Type::Map(
        Box::new(Type::Array(Box::new(Type::Int))),
        Box::new(Type::Int),
    );
    let module = module(
        vec![bad, Type::Int],
        vec![block(
            vec![inst(0, InstKind::MapNew(Vec::new())), inst(1, int(0))],
            TermKind::Ret(Some(ValueId(1))),
        )],
    );
    rejects(&module, "map key type array<int> is not int, bool or str");

    let never = Box::new(Type::Never);
    let good = module_with_map(Type::Map(never.clone(), never));
    ir::verify(&good).unwrap();
}

fn module_with_map(ty: Type) -> Module {
    module(
        vec![ty, Type::Int],
        vec![block(
            vec![
                inst(0, InstKind::MapNew(Vec::new())),
                inst(1, InstKind::MapLen(ValueId(0))),
            ],
            TermKind::Ret(Some(ValueId(1))),
        )],
    )
}
