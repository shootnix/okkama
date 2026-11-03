use okkama_sparrow::{
    Cmp, Constant, Dst, FuncRef, Function, Instruction as I, ObjectDecl, ObjectRef, Program,
    Register as R, Target, Type, TypeRef,
};
use sparrow_engine::{Executable, Mode, Outcome};
fn c(r: u32, n: i64) -> I {
    I::Const {
        dst: Dst(R(r)),
        value: Constant::Int(n.to_string()),
    }
}
fn f(name: &str, kinds: &[u32], code: Vec<I>) -> Function {
    Function {
        symbol: name.into(),
        is_async: false,
        native: None,
        intrinsic: None,
        params: vec![],
        handlers: vec![],
        result: Some(TypeRef(0)),
        registers: kinds.iter().map(|k| TypeRef(*k)).collect(),
        code,
    }
}
fn program() -> Program {
    let obj = |name: &str, ty| ObjectDecl {
        name: name.into(),
        fields: vec![("value".into(), TypeRef(ty))],
        does: vec![],
        destroy: None,
        builtin: false,
    };
    let mut p = Program {
        module: "objects".into(),
        version: "1.0.0".into(),
        entry: Some(FuncRef(0)),
        types: vec![
            Type::Int,
            Type::Bool,
            Type::Object(ObjectRef(0)),
            Type::Object(ObjectRef(1)),
        ],
        objects: vec![obj("Leaf", 0), obj("Wrapper", 2)],
        functions: vec![
            f(
                "main",
                &[0, 0, 2, 3, 0, 2, 0],
                vec![
                    c(0, i64::MAX),
                    c(1, 2),
                    I::IntMul {
                        dst: Dst(R(0)),
                        left: R(0),
                        right: R(1),
                    },
                    I::ObjNew {
                        dst: Dst(R(2)),
                        object: ObjectRef(0),
                        fields: vec![R(0)],
                    },
                    I::ObjNew {
                        dst: Dst(R(3)),
                        object: ObjectRef(1),
                        fields: vec![R(2)],
                    },
                    I::Call {
                        dst: Some(Dst(R(4))),
                        function: FuncRef(1),
                        args: vec![],
                    },
                    I::ObjGet {
                        dst: Dst(R(5)),
                        object: R(3),
                        field: 0,
                    },
                    I::ObjGet {
                        dst: Dst(R(6)),
                        object: R(5),
                        field: 0,
                    },
                    I::Return { value: Some(R(6)) },
                ],
            ),
            f(
                "allocate",
                &[0, 0, 0, 1, 2],
                vec![
                    c(0, 1000),
                    c(1, 1),
                    c(2, 0),
                    I::ObjNew {
                        dst: Dst(R(4)),
                        object: ObjectRef(0),
                        fields: vec![R(0)],
                    },
                    I::IntSub {
                        dst: Dst(R(0)),
                        left: R(0),
                        right: R(1),
                    },
                    I::IntCmp {
                        op: Cmp::Gt,
                        dst: Dst(R(3)),
                        left: R(0),
                        right: R(2),
                    },
                    I::Branch {
                        condition: R(3),
                        then_target: Target {
                            pc: 3,
                            moves: vec![],
                        },
                        else_target: Target {
                            pc: 7,
                            moves: vec![],
                        },
                    },
                    I::Return { value: Some(R(0)) },
                ],
            ),
        ],
        ..Program::default()
    };
    // Force real allocations: an interprocedural consumer prevents local
    // elision, so this fixture continues exercising GC after optimizations.
    p.functions[1].code.insert(
        4,
        I::Call {
            dst: None,
            function: FuncRef(2),
            args: vec![R(4)],
        },
    );
    if let I::Branch { else_target, .. } = &mut p.functions[1].code[7] {
        else_target.pc = 8;
    }
    let mut consume = f(
        "consume",
        &[2, 0],
        vec![
            I::ObjGet {
                dst: Dst(R(1)),
                object: R(0),
                field: 0,
            },
            I::Return { value: Some(R(1)) },
        ],
    );
    consume.params = vec![R(0)];
    p.functions.push(consume);
    let mut identity = f(
        "wrapper_identity",
        &[3],
        vec![I::Return { value: Some(R(0)) }],
    );
    identity.params = vec![R(0)];
    identity.result = Some(TypeRef(3));
    p.functions.push(identity);
    p.functions[0].code.insert(
        5,
        I::Call {
            dst: Some(Dst(R(3))),
            function: FuncRef(3),
            args: vec![R(3)],
        },
    );
    p
}
#[test]
fn object_graph_keeps_numeric_children_alive_across_calls_collection_and_suspension() {
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&program(), mode).unwrap();
        for budget in [1, 4096] {
            let mut session = code.start();
            loop {
                if let Outcome::Returned(n) = session.resume(budget).unwrap() {
                    assert_eq!(n.to_string(), "18446744073709551614");
                    assert!(session.collection_count() >= 7);
                    break;
                }
            }
        }
    }
}
#[test]
fn object_layout_and_cleanup_requirements_are_checked_before_execution() {
    let mut p = program();
    p.functions[0].code[7] = I::ObjGet {
        dst: Dst(R(5)),
        object: R(3),
        field: 1,
    };
    assert!(Executable::new(&p, Mode::Jit).is_err());
    let mut p = program();
    p.functions[0].code[4] = I::ObjNew {
        dst: Dst(R(3)),
        object: ObjectRef(1),
        fields: vec![R(0)],
    };
    assert!(Executable::new(&p, Mode::Jit).is_err());
    let mut p = program();
    p.objects[0].destroy = Some(FuncRef(1));
    assert!(Executable::new(&p, Mode::Cold).is_err());
}

#[test]
fn successor_preserves_aliases_and_replaced_destination() {
    let mut p = program();
    p.functions[0] = f(
        "main",
        &[0, 2, 2, 2, 0, 0, 0],
        vec![
            c(0, 17),
            I::ObjNew {
                dst: Dst(R(1)),
                object: ObjectRef(0),
                fields: vec![R(0)],
            },
            I::Move {
                dst: Dst(R(2)),
                source: R(1),
            },
            c(0, 29),
            // Duplicate update entries follow the existing last-write rule.
            I::ObjNext {
                dst: Dst(R(1)),
                object: R(1),
                updates: vec![(0, R(0)), (0, R(0))],
            },
            I::Call {
                dst: Some(Dst(R(4))),
                function: FuncRef(1),
                args: vec![],
            },
            I::ObjGet {
                dst: Dst(R(5)),
                object: R(2),
                field: 0,
            },
            I::ObjGet {
                dst: Dst(R(6)),
                object: R(1),
                field: 0,
            },
            I::IntAdd {
                dst: Dst(R(0)),
                left: R(5),
                right: R(6),
            },
            I::Return { value: Some(R(0)) },
        ],
    );
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut s = code.start();
        loop {
            if let Outcome::Returned(n) = s.resume(1).unwrap() {
                assert_eq!(n, 46);
                break;
            }
        }
    }
}

#[test]
fn late_binding_preserves_aliases_and_cycles_across_collection() {
    let mut p = program();
    p.types.push(Type::Object(ObjectRef(2)));
    p.objects.push(ObjectDecl {
        name: "Node".into(),
        fields: vec![("next".into(), TypeRef(4)), ("value".into(), TypeRef(0))],
        does: vec![],
        destroy: None,
        builtin: false,
    });
    p.functions[0] = f(
        "main",
        &[4, 4, 0, 4, 0, 4, 0],
        vec![
            I::Undef { dst: Dst(R(0)) },
            c(2, 42),
            I::ObjNew {
                dst: Dst(R(1)),
                object: ObjectRef(2),
                fields: vec![R(0), R(2)],
            },
            I::Move {
                dst: Dst(R(3)),
                source: R(1),
            },
            I::ObjInit {
                object: R(1),
                field: 0,
                value: R(1),
            },
            I::Call {
                dst: Some(Dst(R(4))),
                function: FuncRef(1),
                args: vec![],
            },
            I::ObjGet {
                dst: Dst(R(5)),
                object: R(3),
                field: 0,
            },
            I::ObjGet {
                dst: Dst(R(6)),
                object: R(5),
                field: 1,
            },
            I::Return { value: Some(R(6)) },
        ],
    );
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut s = code.start();
        loop {
            if let Outcome::Returned(n) = s.resume(1).unwrap() {
                assert_eq!(n, 42);
                break;
            }
        }
        let mut twice = p.clone();
        twice.functions[0].code.insert(
            5,
            I::ObjInit {
                object: R(3),
                field: 0,
                value: R(1),
            },
        );
        assert!(
            Executable::new(&twice, mode)
                .unwrap()
                .run()
                .unwrap_err()
                .0
                .contains("already bound")
        );
    }
}

#[test]
fn unbound_float_is_not_silently_evaluated_as_zero() {
    let mut p = program();
    p.types.push(Type::Float);
    p.functions[0] = f(
        "main",
        &[4, 0],
        vec![
            I::Undef { dst: Dst(R(0)) },
            I::FloatNeg {
                dst: Dst(R(0)),
                operand: R(0),
            },
            c(1, 0),
            I::Return { value: Some(R(1)) },
        ],
    );
    for mode in [Mode::Cold, Mode::Jit] {
        assert!(
            Executable::new(&p, mode)
                .unwrap()
                .run()
                .unwrap_err()
                .0
                .contains("unbound")
        );
    }
}

#[test]
fn local_object_elision_captures_fields_before_source_register_changes() {
    let mut p = program();
    p.functions[0] = f(
        "main",
        &[0, 2, 0],
        vec![
            c(0, 17),
            I::ObjNew {
                dst: Dst(R(1)),
                object: ObjectRef(0),
                fields: vec![R(0)],
            },
            c(0, 99),
            I::ObjGet {
                dst: Dst(R(2)),
                object: R(1),
                field: 0,
            },
            I::Return { value: Some(R(2)) },
        ],
    );
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        assert_eq!(code.elided_object_count(), 1);
        let mut s = code.start();
        loop {
            if let Outcome::Returned(n) = s.resume(1).unwrap() {
                assert_eq!(n, 17);
                break;
            }
        }
        let mut escapes = p.clone();
        escapes.functions[0].code.insert(
            2,
            I::Call {
                dst: None,
                function: FuncRef(2),
                args: vec![R(1)],
            },
        );
        let code = Executable::new(&escapes, mode).unwrap();
        assert_eq!(code.elided_object_count(), 0);
        assert_eq!(code.run().unwrap(), 17);
    }
}

#[test]
fn role_dispatch_and_narrowing_preserve_the_underlying_object() {
    use okkama_sparrow::{RoleDecl, RoleMethod, RoleRef, Witness};
    let mut p = program();
    p.types
        .extend([Type::Role(RoleRef(0)), Type::Role(RoleRef(1))]);
    for name in ["Readable", "OtherReadable"] {
        p.roles.push(RoleDecl {
            name: name.into(),
            builtin: false,
            methods: vec![RoleMethod {
                name: "read".into(),
                params: vec![],
                result: Some(TypeRef(0)),
            }],
        });
    }
    let mut getter = f(
        "read",
        &[2, 0],
        vec![
            I::ObjGet {
                dst: Dst(R(1)),
                object: R(0),
                field: 0,
            },
            I::Return { value: Some(R(1)) },
        ],
    );
    getter.params = vec![R(0)];
    p.functions.push(getter);
    p.objects[0].does = vec![
        Witness {
            role: RoleRef(0),
            slots: vec![FuncRef(4)],
        },
        Witness {
            role: RoleRef(1),
            slots: vec![FuncRef(4)],
        },
    ];
    p.functions[0] = f(
        "main",
        &[0, 2, 4, 5, 0],
        vec![
            c(0, 17),
            I::ObjNew {
                dst: Dst(R(1)),
                object: ObjectRef(0),
                fields: vec![R(0)],
            },
            I::RoleWrap {
                dst: Dst(R(2)),
                source: R(1),
                role: RoleRef(0),
            },
            I::RoleNarrow {
                dst: Dst(R(3)),
                source: R(2),
                role: RoleRef(1),
                message: "cannot narrow".into(),
            },
            I::CallRole {
                dst: Some(Dst(R(4))),
                receiver: R(3),
                role: RoleRef(1),
                slot: 0,
                args: vec![],
            },
            I::Return { value: Some(R(4)) },
        ],
    );
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut session = code.start();
        loop {
            if let Outcome::Returned(n) = session.resume(1).unwrap() {
                assert_eq!(n, 17);
                break;
            }
        }
        let mut absent = p.clone();
        absent.objects[0].does.pop();
        assert_eq!(
            Executable::new(&absent, mode).unwrap().run().unwrap_err().0,
            "cannot narrow"
        );
        let mut invalid = p.clone();
        invalid.functions[4].registers[0] = TypeRef(0);
        assert!(Executable::new(&invalid, mode).is_err());
    }
}

#[test]
fn cap_access_recomputes_the_field_over_the_same_owner() {
    use okkama_sparrow::{CapDecl, CapRef};
    let mut p = program();
    p.types.push(Type::Cap(CapRef(0)));
    let mut getter = f(
        "read",
        &[2, 0],
        vec![
            I::ObjGet {
                dst: Dst(R(1)),
                object: R(0),
                field: 0,
            },
            I::Return { value: Some(R(1)) },
        ],
    );
    getter.params = vec![R(0)];
    p.functions.push(getter);
    p.caps.push(CapDecl {
        name: "View".into(),
        owner: ObjectRef(0),
        fields: vec![("value".into(), FuncRef(4))],
    });
    p.functions[0] = f(
        "main",
        &[0, 2, 4, 0],
        vec![
            I::Undef { dst: Dst(R(0)) },
            I::ObjNew {
                dst: Dst(R(1)),
                object: ObjectRef(0),
                fields: vec![R(0)],
            },
            I::Move {
                dst: Dst(R(2)),
                source: R(1),
            },
            I::CallCap {
                dst: Dst(R(3)),
                cap: R(2),
                ty: CapRef(0),
                field: 0,
            },
            c(0, 42),
            I::ObjInit {
                object: R(1),
                field: 0,
                value: R(0),
            },
            I::CallCap {
                dst: Dst(R(3)),
                cap: R(2),
                ty: CapRef(0),
                field: 0,
            },
            I::Return { value: Some(R(3)) },
        ],
    );
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&p, mode).unwrap().run().unwrap(), 42);
        let mut invalid = p.clone();
        invalid.caps[0].owner = ObjectRef(1);
        assert!(Executable::new(&invalid, mode).is_err());
    }
}
#[test]
fn successor_return_and_take_preserve_the_new_object() {
    let mut p = program();
    p.types.push(Type::Successor(ObjectRef(0)));
    let mut successor = f(
        "next",
        &[0, 2],
        vec![
            c(0, 42),
            I::ObjNew {
                dst: Dst(R(1)),
                object: ObjectRef(0),
                fields: vec![R(0)],
            },
            I::Return { value: Some(R(1)) },
        ],
    );
    successor.result = Some(TypeRef(4));
    p.functions.push(successor);
    p.functions[0] = f(
        "main",
        &[4, 2, 0],
        vec![
            I::Call {
                dst: Some(Dst(R(0))),
                function: FuncRef(4),
                args: vec![],
            },
            I::Move {
                dst: Dst(R(1)),
                source: R(0),
            },
            I::ObjGet {
                dst: Dst(R(2)),
                object: R(1),
                field: 0,
            },
            I::Return { value: Some(R(2)) },
        ],
    );
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&p, mode).unwrap().run().unwrap(), 42);
    }
}
