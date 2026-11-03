use okkama_sparrow::{
    Constant, Dst, FuncRef, Function, Instruction as I, Program, Register as R, Type, TypeRef,
};
use sparrow_engine::{Executable, Mode, Outcome};

fn function(name: &str, registers: usize, params: Vec<R>, code: Vec<I>) -> Function {
    Function {
        symbol: name.into(),
        is_async: false,
        native: None,
        intrinsic: None,
        registers: vec![TypeRef(0); registers],
        params,
        result: Some(TypeRef(0)),
        handlers: vec![],
        code,
    }
}
fn program(functions: Vec<Function>) -> Program {
    Program {
        module: "calls".into(),
        version: "1.0.0".into(),
        types: vec![Type::Int],
        entry: Some(FuncRef(0)),
        functions,
        ..Program::default()
    }
}
fn constant(dst: u32, value: i64) -> I {
    I::Const {
        dst: Dst(R(dst)),
        value: Constant::Int(value.to_string()),
    }
}
fn call(dst: u32, callee: u32, args: Vec<R>) -> I {
    I::Call {
        dst: Some(Dst(R(dst))),
        function: FuncRef(callee),
        args,
    }
}
fn ret(source: u32) -> I {
    I::Return {
        value: Some(R(source)),
    }
}

#[test]
fn ordinary_call_saves_only_live_caller_values() {
    let p = program(vec![
        function(
            "main",
            1000,
            vec![],
            vec![
                constant(0, 17),
                constant(1, 99),
                call(2, 1, vec![R(1)]),
                ret(0),
            ],
        ),
        function("identity", 1, vec![R(0)], vec![ret(0)]),
    ]);
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut session = code.start();
        assert_eq!(session.resume(3).unwrap(), Outcome::Yielded);
        assert_eq!(session.continuation_depth(), 1);
        assert_eq!(session.saved_value_count(), 1);
        assert_eq!(session.live_registers().collect::<Vec<_>>(), vec![0]);
        assert_eq!(session.resume(1).unwrap(), Outcome::Yielded);
        assert_eq!(session.continuation_depth(), 0);
        assert_eq!(session.saved_value_count(), 0);
        assert_eq!(session.resume(1).unwrap(), Outcome::Returned(17.into()));
    }
}

#[test]
fn tail_call_arguments_are_captured_before_frame_reuse() {
    let p = program(vec![
        function(
            "main",
            3,
            vec![],
            vec![
                constant(0, 17),
                constant(1, 99),
                call(2, 1, vec![R(0), R(1)]),
                ret(2),
            ],
        ),
        function(
            "swap",
            2,
            vec![R(0), R(1)],
            vec![call(0, 2, vec![R(1), R(0)]), ret(0)],
        ),
        function("first", 2, vec![R(0), R(1)], vec![ret(0)]),
    ]);
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut session = code.start();
        assert_eq!(session.resume(4).unwrap(), Outcome::Yielded);
        assert_eq!(session.continuation_depth(), 0);
        assert_eq!(session.saved_value_count(), 0);
        assert_eq!(session.resume(1).unwrap(), Outcome::Returned(99.into()));
    }
}

#[test]
fn mutual_tail_recursion_is_bounded_and_non_tail_recursion_is_explicit() {
    let tail = program(vec![
        function("main", 1, vec![], vec![call(0, 1, vec![]), ret(0)]),
        function("again", 1, vec![], vec![call(0, 0, vec![]), ret(0)]),
    ]);
    let ordinary = program(vec![function(
        "main",
        1000,
        vec![],
        vec![constant(0, 17), call(1, 0, vec![]), ret(0)],
    )]);
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&tail, mode).unwrap();
        let mut session = code.start();
        for _ in 0..10 {
            assert_eq!(session.resume(100_000).unwrap(), Outcome::Yielded);
        }
        assert_eq!(session.continuation_depth(), 0);
        assert_eq!(session.saved_value_count(), 0);
        let code = Executable::new(&ordinary, mode).unwrap();
        let mut session = code.start();
        assert_eq!(session.resume(20_000).unwrap(), Outcome::Yielded);
        assert_eq!(session.continuation_depth(), 10_000);
        assert_eq!(session.saved_value_count(), 10_000);
    }
}

#[test]
fn nested_returns_restore_the_correct_continuation() {
    for (mode, depth) in [(Mode::Cold, 2000), (Mode::Jit, 32)] {
        let functions = (0..depth)
            .map(|i| {
                let mut code = vec![constant(0, i as i64)];
                if i + 1 < depth {
                    code.push(call(1, i + 1, vec![]));
                }
                code.push(ret(0));
                function(&format!("f{i}"), 2, vec![], code)
            })
            .collect();
        let code = Executable::new(&program(functions), mode).unwrap();
        assert_eq!(code.run().unwrap(), 0);
    }
}

#[test]
fn void_calls_restore_live_values_without_inventing_a_result() {
    let mut middle = function(
        "middle",
        0,
        vec![],
        vec![
            I::Call {
                dst: None,
                function: FuncRef(2),
                args: vec![],
            },
            I::Return { value: None },
        ],
    );
    middle.result = None;
    let mut leaf = function("leaf", 0, vec![], vec![I::Return { value: None }]);
    leaf.result = None;
    let p = program(vec![
        function(
            "main",
            1,
            vec![],
            vec![
                constant(0, 42),
                I::Call {
                    dst: None,
                    function: FuncRef(1),
                    args: vec![],
                },
                ret(0),
            ],
        ),
        middle,
        leaf,
    ]);
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut session = code.start();
        loop {
            if let Outcome::Returned(n) = session.resume(1).unwrap() {
                assert_eq!(n, 42);
                break;
            }
        }
        let mut invalid = p.clone();
        invalid.functions[0].code[1] = I::Call {
            dst: Some(Dst(R(0))),
            function: FuncRef(1),
            args: vec![],
        };
        assert!(Executable::new(&invalid, mode).is_err());
    }
}

#[test]
fn closure_captures_survive_source_changes_calls_and_collection() {
    let mut main = function(
        "main",
        4,
        vec![],
        vec![
            constant(0, i64::MAX),
            constant(1, 2),
            I::IntMul {
                dst: Dst(R(0)),
                left: R(0),
                right: R(1),
            },
            I::Closure {
                dst: Dst(R(2)),
                function: FuncRef(1),
                captures: vec![R(0)],
            },
            constant(0, 99),
            I::Call {
                dst: Some(Dst(R(3))),
                function: FuncRef(2),
                args: vec![],
            },
            constant(1, 1),
            I::CallFn {
                dst: Some(Dst(R(0))),
                callee: R(2),
                args: vec![R(1)],
            },
            ret(0),
        ],
    );
    main.registers[2] = TypeRef(1);
    let add = function(
        "add_capture",
        3,
        vec![R(0), R(1)],
        vec![
            I::IntAdd {
                dst: Dst(R(2)),
                left: R(0),
                right: R(1),
            },
            ret(2),
        ],
    );
    let mut pressure = function(
        "allocate",
        5,
        vec![],
        vec![
            constant(0, 500),
            constant(1, 1),
            constant(2, 0),
            I::ArrayNew {
                dst: Dst(R(4)),
                elements: vec![R(0)],
            },
            I::IntSub {
                dst: Dst(R(0)),
                left: R(0),
                right: R(1),
            },
            I::IntCmp {
                op: okkama_sparrow::Cmp::Gt,
                dst: Dst(R(3)),
                left: R(0),
                right: R(2),
            },
            I::Branch {
                condition: R(3),
                then_target: okkama_sparrow::Target {
                    pc: 3,
                    moves: vec![],
                },
                else_target: okkama_sparrow::Target {
                    pc: 7,
                    moves: vec![],
                },
            },
            ret(0),
        ],
    );
    pressure.registers[3] = TypeRef(3);
    pressure.registers[4] = TypeRef(2);
    let mut p = program(vec![main, add, pressure]);
    p.types.extend([
        Type::Fn {
            params: vec![TypeRef(0)],
            result: Some(TypeRef(0)),
        },
        Type::Array(TypeRef(0)),
        Type::Bool,
    ]);
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut session = code.start();
        let mut completed = false;
        for _ in 0..4096 {
            if let Outcome::Returned(n) = session.resume(1).unwrap() {
                assert_eq!(n.to_string(), "18446744073709551615");
                assert!(session.collection_count() >= 3);
                completed = true;
                break;
            }
        }
        assert!(
            completed,
            "closure GC fixture exceeded 4096 instruction slices"
        );
        let mut bad = p.clone();
        if let I::Closure { captures, .. } = &mut bad.functions[0].code[3] {
            captures.clear();
        }
        assert!(Executable::new(&bad, mode).is_err());
    }
}
#[test]
fn void_closure_has_no_synthetic_result() {
    let mut main = function(
        "main",
        2,
        vec![],
        vec![
            constant(0, 42),
            I::Closure {
                dst: Dst(R(1)),
                function: FuncRef(1),
                captures: vec![R(0)],
            },
            I::CallFn {
                dst: None,
                callee: R(1),
                args: vec![],
            },
            ret(0),
        ],
    );
    main.registers[1] = TypeRef(1);
    let mut target = function("void", 1, vec![R(0)], vec![I::Return { value: None }]);
    target.result = None;
    let mut p = program(vec![main, target]);
    p.types.push(Type::Fn {
        params: vec![],
        result: None,
    });
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&p, mode).unwrap().run().unwrap(), 42);
    }
}
