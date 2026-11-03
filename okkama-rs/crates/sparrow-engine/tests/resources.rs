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
fn f(name: &str, types: &[u32], params: &[u32], result: Option<u32>, code: Vec<I>) -> Function {
    Function {
        symbol: name.into(),
        registers: types.iter().map(|t| TypeRef(*t)).collect(),
        params: params.iter().map(|r| R(*r)).collect(),
        result: result.map(TypeRef),
        code,
        handlers: vec![],
        is_async: false,
        native: None,
        intrinsic: None,
    }
}
fn program(throwing: bool, unwind: bool) -> Program {
    // Types: int, builder<int>, array<int>, resource, region, failure.
    let mut body = vec![
        I::ObjGet {
            dst: Dst(R(1)),
            object: R(0),
            field: 0,
        },
        I::ObjGet {
            dst: Dst(R(2)),
            object: R(0),
            field: 1,
        },
    ];
    // Many safepoints while the closing region and outer saved frame are roots.
    body.extend((0..300).map(|_| I::ArrayNew {
        dst: Dst(R(3)),
        elements: vec![],
    }));
    body.push(I::BuilderPush {
        dst: Dst(R(1)),
        builder: R(1),
        value: R(2),
    });
    body.push(if throwing {
        I::Throw { value: R(0) }
    } else {
        I::Return { value: None }
    });
    let destroy = f("Resource.destroy", &[3, 1, 0, 2], &[0], None, body);
    let mut main = f(
        "main",
        &[1, 4, 0, 3, 3, 2, 0, 0, 0, 5, 5, 3, 0],
        &[],
        Some(0),
        vec![
            I::BuilderNew { dst: Dst(R(0)) },
            I::RegionEnter { dst: Dst(R(1)) },
            c(2, 2),
            I::ObjNew {
                dst: Dst(R(3)),
                object: ObjectRef(0),
                fields: vec![R(0), R(2)],
            },
            I::RegionAdopt {
                region: R(1),
                value: R(3),
            },
            c(2, 4),
            I::ObjNew {
                dst: Dst(R(4)),
                object: ObjectRef(0),
                fields: vec![R(0), R(2)],
            },
            I::RegionAdopt {
                region: R(1),
                value: R(4),
            },
        ],
    );
    if unwind {
        // Primary error is an independent identity; unwind must preserve it.
        main.code.extend([
            c(2, 7),
            I::ObjNew {
                dst: Dst(R(11)),
                object: ObjectRef(0),
                fields: vec![R(0), R(2)],
            },
        ]);
        let throw = main.code.len() as u32;
        main.code.push(I::Throw { value: R(11) });
        let handle = main.code.len() as u32;
        main.handlers.push(Handler {
            start: throw,
            end: throw + 1,
            target: Target {
                pc: handle,
                moves: vec![],
            },
            failure: R(9),
        });
        main.code.push(I::RegionUnwind {
            dst: Dst(R(10)),
            region: R(1),
            failure: R(9),
        });
    } else {
        let close = main.code.len() as u32;
        main.code.push(I::RegionLeave {
            region: R(1),
            keep: None,
        });
        if throwing {
            main.code.push(I::Trap {
                code: okkama_sparrow::TrapCode::Empty,
                message: "expected destroy failure".into(),
            });
            main.handlers.push(Handler {
                start: close,
                end: close + 1,
                target: Target {
                    pc: close + 2,
                    moves: vec![],
                },
                failure: R(10),
            });
        }
    }
    if throwing || unwind {
        let pc = main.code.len() as u32;
        main.code.push(I::Catch {
            failure: R(10),
            kind: CatchKind::Object,
            ty: 0,
            value: Dst(R(11)),
            handler: Target {
                pc: pc + 2,
                moves: vec![],
            },
            next: Target {
                pc: pc + 1,
                moves: vec![],
            },
        });
        main.code.push(I::Raise { failure: R(10) });
        main.code.push(I::ObjGet {
            dst: Dst(R(12)),
            object: R(11),
            field: 1,
        });
    } else {
        main.code.push(c(12, 0));
    }
    main.code.extend([
        I::BuilderFinish {
            dst: Dst(R(5)),
            builder: R(0),
        },
        c(2, 0),
        I::ArrayGet {
            dst: Dst(R(6)),
            array: R(5),
            index: R(2),
        },
        c(2, 1),
        I::ArrayGet {
            dst: Dst(R(7)),
            array: R(5),
            index: R(2),
        },
        c(2, 10),
        I::IntMul {
            dst: Dst(R(8)),
            left: R(6),
            right: R(2),
        },
        I::IntAdd {
            dst: Dst(R(8)),
            left: R(8),
            right: R(7),
        },
        I::IntAdd {
            dst: Dst(R(8)),
            left: R(8),
            right: R(12),
        },
        I::Return { value: Some(R(8)) },
    ]);
    Program {
        module: "resources".into(),
        version: "1.0.0".into(),
        entry: Some(FuncRef(0)),
        types: vec![
            Type::Int,
            Type::Builder(TypeRef(0)),
            Type::Array(TypeRef(0)),
            Type::Object(ObjectRef(0)),
            Type::Region,
            Type::Failure,
        ],
        objects: vec![ObjectDecl {
            name: "Resource".into(),
            fields: vec![("log".into(), TypeRef(1)), ("value".into(), TypeRef(0))],
            does: vec![],
            destroy: Some(FuncRef(1)),
            builtin: false,
        }],
        functions: vec![main, destroy],
        ..Program::default()
    }
}
fn check(p: &Program, expected: i64) {
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(p, mode).unwrap();
        for budget in [1, 4096] {
            let mut session = code.start();
            let mut completed = false;
            for _ in 0..4096 / budget {
                match session.resume(budget).unwrap() {
                    Outcome::Returned(n) => {
                        assert_eq!(n, expected);
                        completed = true;
                        break;
                    }
                    Outcome::Yielded => {}
                    Outcome::Waiting => {
                        panic!("synchronous cleanup fixture unexpectedly suspended")
                    }
                }
            }
            assert!(completed, "cleanup fixture exceeded 4096 VM instructions");
            assert!(session.collection_count() >= 4);
        }
    }
}
#[test]
fn destroy_is_lifo_and_its_state_survives_yields_and_gc() {
    check(&program(false, false), 42);
}
#[test]
fn cleanup_continues_after_throw_and_reports_the_first_failure() {
    check(&program(true, false), 46);
}
#[test]
fn unwind_preserves_primary_failure_while_running_every_destroy() {
    check(&program(true, true), 49);
}
#[test]
fn malformed_destroy_receiver_is_rejected_before_running() {
    let mut p = program(false, false);
    p.functions[1].registers[0] = TypeRef(0);
    for mode in [Mode::Cold, Mode::Jit] {
        assert!(
            Executable::new(&p, mode)
                .unwrap_err()
                .0
                .contains("destroy signature")
        );
    }
}
#[test]
fn deeply_nested_cleanup_uses_explicit_continuations_and_keeps_outer_roots() {
    let mut p = program(false, false);
    p.types.push(Type::Bool);
    p.functions[0] = f(
        "main",
        &[1, 4, 0, 3, 2],
        &[],
        Some(0),
        vec![
            I::BuilderNew { dst: Dst(R(0)) },
            I::RegionEnter { dst: Dst(R(1)) },
            c(2, 1000),
            I::ObjNew {
                dst: Dst(R(3)),
                object: ObjectRef(0),
                fields: vec![R(0), R(2)],
            },
            I::RegionAdopt {
                region: R(1),
                value: R(3),
            },
            I::RegionLeave {
                region: R(1),
                keep: None,
            },
            I::BuilderFinish {
                dst: Dst(R(4)),
                builder: R(0),
            },
            I::ArrayLen {
                dst: Dst(R(2)),
                array: R(4),
            },
            I::Return { value: Some(R(2)) },
        ],
    );
    p.functions[1] = f(
        "Resource.destroy",
        &[3, 1, 0, 0, 6, 4, 3],
        &[0],
        None,
        vec![
            I::ObjGet {
                dst: Dst(R(1)),
                object: R(0),
                field: 0,
            },
            I::ObjGet {
                dst: Dst(R(2)),
                object: R(0),
                field: 1,
            },
            c(3, 0),
            I::IntCmp {
                op: okkama_sparrow::Cmp::Gt,
                dst: Dst(R(4)),
                left: R(2),
                right: R(3),
            },
            I::Branch {
                condition: R(4),
                then_target: Target {
                    pc: 5,
                    moves: vec![],
                },
                else_target: Target {
                    pc: 11,
                    moves: vec![],
                },
            },
            c(3, 1),
            I::IntSub {
                dst: Dst(R(3)),
                left: R(2),
                right: R(3),
            },
            I::RegionEnter { dst: Dst(R(5)) },
            I::ObjNew {
                dst: Dst(R(6)),
                object: ObjectRef(0),
                fields: vec![R(1), R(3)],
            },
            I::RegionAdopt {
                region: R(5),
                value: R(6),
            },
            I::RegionLeave {
                region: R(5),
                keep: None,
            },
            I::BuilderPush {
                dst: Dst(R(1)),
                builder: R(1),
                value: R(2),
            },
            I::Return { value: None },
        ],
    );
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut session = code.start();
        let mut peak = 0;
        loop {
            let result = session.resume(1).unwrap();
            peak = peak.max(session.continuation_depth());
            if let Outcome::Returned(n) = result {
                assert_eq!(n, 1001);
                break;
            }
        }
        assert_eq!(peak, 1001);
        assert!(session.collection_count() > 5);
    }
}

#[test]
fn terminal_report_retains_primary_and_secondary_cleanup_traces() {
    let mut p = program(true, false);
    p.functions[0].handlers.clear();
    p.functions[0].code.truncate(9);
    p.functions[0]
        .code
        .extend([c(2, 0), I::Return { value: Some(R(2)) }]);
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut session = code.start();
        loop {
            if session.resume(1).is_err() {
                break;
            }
        }
        let report = session.failure_report().unwrap().unwrap();
        assert_eq!(report.nodes.len(), 2);
        assert_eq!(report.nodes[0].secondary, vec![1]);
        assert_eq!(
            report.nodes[0]
                .trace
                .iter()
                .map(|s| (s.function.as_str(), s.pc))
                .collect::<Vec<_>>(),
            vec![("Resource.destroy", 303), ("main", 8)]
        );
        assert_eq!(
            report.nodes[1]
                .trace
                .iter()
                .map(|s| s.function.as_str())
                .collect::<Vec<_>>(),
            vec!["Resource.destroy"]
        );
        assert_eq!(session.failure_report().unwrap().unwrap(), report);
        assert!(session.resume(0).is_err());
    }
}

#[test]
fn every_uncaught_cleanup_error_gets_its_own_lazy_text_and_is_reported() {
    let mut p = program(true, false);
    p.functions[0].handlers.clear();
    p.functions[0].code.truncate(9);
    p.functions[0]
        .code
        .extend([c(2, 0), I::Return { value: Some(R(2)) }]);
    p.types.push(Type::Str);
    p.roles.push(okkama_sparrow::RoleDecl {
        name: "Error".into(),
        builtin: true,
        methods: vec![okkama_sparrow::RoleMethod {
            name: "text".into(),
            params: vec![],
            result: Some(TypeRef(6)),
        }],
    });
    p.objects[0].does.push(okkama_sparrow::Witness {
        role: okkama_sparrow::RoleRef(0),
        slots: vec![FuncRef(2)],
    });
    let mut body = vec![I::ObjGet {
        dst: Dst(R(1)),
        object: R(0),
        field: 1,
    }];
    body.extend((0..300).map(|_| I::ArrayNew {
        dst: Dst(R(3)),
        elements: vec![],
    }));
    body.extend([
        I::Convert {
            dst: Dst(R(2)),
            source: R(1),
        },
        I::Return { value: Some(R(2)) },
    ]);
    p.functions
        .push(f("Resource.text", &[3, 0, 6, 2], &[0], Some(6), body));
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut session = code.start();
        let error = loop {
            if let Err(e) = session.resume(1) {
                break e;
            }
        };
        assert_eq!(
            error.0,
            "Resource: 4\nSecondary cleanup failure:\n  Resource: 2"
        );
        let report = session.failure_report().unwrap().unwrap();
        assert_eq!(report.nodes[0].message.as_deref(), Some("4"));
        assert_eq!(report.nodes[1].message.as_deref(), Some("2"));
        assert!(session.collection_count() >= 9);
    }
}
