use okkama_sparrow::{
    CatchKind, Constant, Dst, FuncRef, Function, Handler, Instruction as I, ObjectDecl, ObjectRef,
    Program, Register as R, Target, Type, TypeRef,
};
use sparrow_engine::{Executable, Mode, Outcome};
fn c(r: u32, n: i64) -> I {
    I::Const {
        dst: Dst(R(r)),
        value: Constant::Int(n.to_string()),
    }
}
fn f(name: &str, types: &[u32], code: Vec<I>) -> Function {
    Function {
        symbol: name.into(),
        params: vec![],
        is_async: false,
        native: None,
        intrinsic: None,
        handlers: vec![],
        result: Some(TypeRef(0)),
        registers: types.iter().map(|t| TypeRef(*t)).collect(),
        code,
    }
}
fn program() -> Program {
    let mut main = f(
        "main",
        &[0, 1, 2, 0],
        vec![
            c(0, 7),
            I::Call {
                dst: Some(Dst(R(0))),
                function: FuncRef(1),
                args: vec![],
            },
            I::Return { value: Some(R(0)) },
            I::Catch {
                failure: R(2),
                kind: CatchKind::Object,
                ty: 0,
                value: Dst(R(1)),
                handler: Target {
                    pc: 4,
                    moves: vec![],
                },
                next: Target {
                    pc: 7,
                    moves: vec![],
                },
            },
            I::ObjGet {
                dst: Dst(R(3)),
                object: R(1),
                field: 0,
            },
            I::IntAdd {
                dst: Dst(R(0)),
                left: R(0),
                right: R(3),
            },
            I::Return { value: Some(R(0)) },
            I::Raise { failure: R(2) },
        ],
    );
    main.handlers = vec![Handler {
        start: 1,
        end: 2,
        target: Target {
            pc: 3,
            moves: vec![],
        },
        failure: R(2),
    }];
    let throwing = f(
        "throw",
        &[0, 1],
        vec![
            c(0, 42),
            I::ObjNew {
                dst: Dst(R(1)),
                object: ObjectRef(0),
                fields: vec![R(0)],
            },
            I::Throw { value: R(1) },
        ],
    );
    Program {
        module: "failure".into(),
        version: "1.0.0".into(),
        entry: Some(FuncRef(0)),
        types: vec![
            Type::Int,
            Type::Object(ObjectRef(0)),
            Type::Failure,
            Type::Float,
        ],
        objects: vec![ObjectDecl {
            name: "Problem".into(),
            fields: vec![("value".into(), TypeRef(0))],
            does: vec![],
            destroy: None,
            builtin: false,
        }],
        functions: vec![main, throwing],
        ..Program::default()
    }
}
fn check(p: &Program, expected: i64) {
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(p, mode).unwrap();
        for budget in [1, 4096] {
            let mut session = code.start();
            loop {
                if let Outcome::Returned(n) = session.resume(budget).unwrap() {
                    assert_eq!(n, expected);
                    break;
                }
            }
        }
    }
}
#[test]
fn thrown_payload_unwinds_flat_calls_and_preserves_old_result_register() {
    check(&program(), 49);
}
#[test]
fn rethrow_reaches_outer_handler_with_the_same_payload() {
    let mut p = program();
    let mut middle = f(
        "middle",
        &[0, 2],
        vec![
            I::Call {
                dst: Some(Dst(R(0))),
                function: FuncRef(1),
                args: vec![],
            },
            I::Return { value: Some(R(0)) },
            I::Raise { failure: R(1) },
        ],
    );
    middle.handlers = vec![Handler {
        start: 0,
        end: 1,
        target: Target {
            pc: 2,
            moves: vec![],
        },
        failure: R(1),
    }];
    p.functions.push(middle);
    p.functions[0].code[1] = I::Call {
        dst: Some(Dst(R(0))),
        function: FuncRef(2),
        args: vec![],
    };
    check(&p, 49);
}
#[test]
fn native_unbound_guard_spills_handler_only_values() {
    let mut p = program();
    let mut main = f(
        "main",
        &[0, 3, 2],
        vec![
            c(0, 42),
            I::Undef { dst: Dst(R(1)) },
            I::FloatNeg {
                dst: Dst(R(1)),
                operand: R(1),
            },
            I::Return { value: Some(R(0)) },
            I::Return { value: Some(R(0)) },
        ],
    );
    main.handlers = vec![Handler {
        start: 2,
        end: 3,
        target: Target {
            pc: 4,
            moves: vec![],
        },
        failure: R(2),
    }];
    p.functions = vec![main];
    check(&p, 42);
}
#[test]
fn program_failure_does_not_match_a_typed_object_catch() {
    let mut p = program();
    p.functions[1].code = vec![I::Trap {
        code: okkama_sparrow::TrapCode::Empty,
        message: "program failure".into(),
    }];
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(
            Executable::new(&p, mode).unwrap().run().unwrap_err().0,
            "program failure"
        );
    }
}

#[test]
fn real_compiler_try_catch_fixture_executes_without_legacy_runtime() {
    let p = okkama_sparrow::assemble(include_str!("fixtures/caught-error.asm")).unwrap();
    check(&p, 42);
}

#[test]
fn compiler_resource_cleanup_throws_into_the_surrounding_catch() {
    let p = okkama_sparrow::assemble(include_str!("fixtures/resource-cleanup.asm")).unwrap();
    check(&p, 42);
}

#[test]
fn compiler_defer_failure_precedes_destroy_and_returned_resource_escapes() {
    for source in [
        include_str!("fixtures/resource-defer.asm"),
        include_str!("fixtures/resource-return.asm"),
    ] {
        let p = okkama_sparrow::assemble(source).unwrap();
        check(&p, 42);
    }
}

#[test]
fn rethrow_trace_records_each_activation_once_at_its_original_failure_pc() {
    let mut p = program();
    p.functions[0].handlers.clear();
    p.functions[0].code.truncate(2);
    // Keep main as an actual activation: an immediate return would be a tail call.
    p.functions[0].code.extend([
        I::IntAdd {
            dst: Dst(R(0)),
            left: R(0),
            right: R(0),
        },
        I::Return { value: Some(R(0)) },
    ]);
    let mut middle = f(
        "middle",
        &[0, 2],
        vec![
            I::Call {
                dst: Some(Dst(R(0))),
                function: FuncRef(1),
                args: vec![],
            },
            I::Return { value: Some(R(0)) },
            I::Raise { failure: R(1) },
        ],
    );
    middle.handlers.push(Handler {
        start: 0,
        end: 1,
        target: Target {
            pc: 2,
            moves: vec![],
        },
        failure: R(1),
    });
    p.functions.push(middle);
    p.functions[0].code[1] = I::Call {
        dst: Some(Dst(R(0))),
        function: FuncRef(2),
        args: vec![],
    };
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut session = code.start();
        loop {
            if session.resume(1).is_err() {
                break;
            }
        }
        let report = session.failure_report().unwrap().unwrap();
        assert_eq!(
            report.nodes[0]
                .trace
                .iter()
                .map(|s| (s.function.as_str(), s.pc))
                .collect::<Vec<_>>(),
            vec![("throw", 2), ("middle", 0), ("main", 1)]
        );
    }
}

#[test]
fn uncaught_error_text_runs_only_after_unwinding_and_survives_gc_and_yields() {
    let mut p = okkama_sparrow::assemble(include_str!("fixtures/caught-error.asm")).unwrap();
    // Source.value throws; turn it into the parameterless test entry.
    p.entry = Some(FuncRef(3));
    p.functions[3].params.clear();
    p.types.push(Type::Array(TypeRef(3)));
    let array = TypeRef((p.types.len() - 1) as u32);
    p.functions[0].registers.push(array);
    p.functions[0].code.splice(
        0..0,
        (0..300).map(|_| I::ArrayNew {
            dst: Dst(R(2)),
            elements: vec![],
        }),
    );
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut session = code.start();
        let error = loop {
            if let Err(e) = session.resume(1) {
                break e;
            }
        };
        assert_eq!(error.0, "Problem: problem");
        assert!(session.collection_count() >= 2);
        let report = session.failure_report().unwrap().unwrap();
        assert_eq!(report.nodes[0].message.as_deref(), Some("problem"));
        assert_eq!(
            report.nodes[0].trace.last().unwrap().function,
            "Source.value"
        );
        assert!(
            report.nodes[0]
                .trace
                .iter()
                .all(|s| s.function != "Problem.text")
        );
        assert_eq!(session.resume(0).unwrap_err(), error);
    }
}

#[test]
fn caught_errors_do_not_evaluate_their_text_method() {
    let mut p = okkama_sparrow::assemble(include_str!("fixtures/caught-error.asm")).unwrap();
    p.functions[0].code = vec![I::Trap {
        code: okkama_sparrow::TrapCode::Empty,
        message: "text must stay lazy".into(),
    }];
    check(&p, 42);
}

#[test]
fn unreachable_is_supported_as_a_terminator_but_is_not_a_catchable_failure() {
    let mut p = program();
    p.functions[0] = f(
        "main",
        &[0, 2],
        vec![I::Unreachable {}, c(0, 42), I::Return { value: Some(R(0)) }],
    );
    p.functions[0].handlers.push(Handler {
        start: 0,
        end: 1,
        target: Target {
            pc: 1,
            moves: vec![],
        },
        failure: R(1),
    });
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut session = code.start();
        assert_eq!(
            session.resume(1).unwrap_err().0,
            "unreachable instruction reached"
        );
        assert!(session.failure_report().unwrap().is_none());
    }
    p.functions[0].handlers.clear();
    p.functions[0].code = vec![c(0, 42), I::Return { value: Some(R(0)) }, I::Unreachable {}];
    check(&p, 42);
}
