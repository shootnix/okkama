use okkama_sparrow::{
    Constant, Dst, FuncRef, Function, Instruction as I, Program, Register as R, Target, Type,
    TypeRef,
};
use sparrow_engine::{Executable, Mode, Outcome};
fn c(r: u32, n: i64) -> I {
    I::Const {
        dst: Dst(R(r)),
        value: Constant::Int(n.to_string()),
    }
}
fn s(r: u32, t: &str) -> I {
    I::Const {
        dst: Dst(R(r)),
        value: Constant::Str(t.into()),
    }
}
fn program(kinds: &[u32], code: Vec<I>) -> Program {
    Program {
        module: "maps".into(),
        version: "1.0.0".into(),
        entry: Some(FuncRef(0)),
        types: vec![
            Type::Int,
            Type::Str,
            Type::Float,
            Type::Bool,
            Type::Map(TypeRef(1), TypeRef(0)),
            Type::MapBuilder(TypeRef(1), TypeRef(0)),
            Type::Array(TypeRef(1)),
            Type::Array(TypeRef(0)),
            Type::Map(TypeRef(2), TypeRef(0)),
            Type::Array(TypeRef(2)),
        ],
        functions: vec![Function {
            symbol: "main".into(),
            params: vec![],
            is_async: false,
            native: None,
            intrinsic: None,
            handlers: vec![],
            result: Some(TypeRef(0)),
            registers: kinds.iter().map(|t| TypeRef(*t)).collect(),
            code,
        }],
        ..Program::default()
    }
}
fn check(p: &Program, n: i64) {
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(p, mode).unwrap();
        let mut session = code.start();
        loop {
            if let Outcome::Returned(value) = session.resume(1).unwrap() {
                assert_eq!(value, n);
                break;
            }
        }
    }
}
#[test]
fn duplicate_keys_replace_values_and_enumeration_is_sorted() {
    let p = program(
        &[1, 1, 0, 0, 0, 4, 6, 1, 0, 7, 0],
        vec![
            s(0, "b"),
            s(1, "a"),
            c(2, 7),
            c(3, 11),
            c(4, 19),
            I::MapNew {
                dst: Dst(R(5)),
                entries: vec![(R(0), R(2)), (R(1), R(3)), (R(0), R(4))],
            },
            I::MapKeys {
                dst: Dst(R(6)),
                map: R(5),
            },
            c(2, 0),
            I::ArrayGet {
                dst: Dst(R(7)),
                array: R(6),
                index: R(2),
            },
            I::MapGet {
                dst: Dst(R(8)),
                map: R(5),
                key: R(7),
            },
            I::MapValues {
                dst: Dst(R(9)),
                map: R(5),
            },
            c(2, 1),
            I::ArrayGet {
                dst: Dst(R(10)),
                array: R(9),
                index: R(2),
            },
            I::IntAdd {
                dst: Dst(R(8)),
                left: R(8),
                right: R(10),
            },
            I::Return { value: Some(R(8)) },
        ],
    );
    check(&p, 30);
}
#[test]
fn builder_finish_keeps_aliases_usable_and_previous_maps_immutable() {
    let p = program(
        &[5, 5, 1, 0, 4, 4, 0, 0],
        vec![
            I::MapBuilderNew { dst: Dst(R(0)) },
            I::Move {
                dst: Dst(R(1)),
                source: R(0),
            },
            s(2, "x"),
            c(3, 42),
            I::MapBuilderPut {
                dst: Dst(R(0)),
                builder: R(0),
                key: R(2),
                value: R(3),
            },
            I::MapBuilderFinish {
                dst: Dst(R(4)),
                builder: R(1),
            },
            c(3, 99),
            I::MapBuilderPut {
                dst: Dst(R(1)),
                builder: R(1),
                key: R(2),
                value: R(3),
            },
            I::MapBuilderFinish {
                dst: Dst(R(5)),
                builder: R(0),
            },
            I::MapGet {
                dst: Dst(R(6)),
                map: R(4),
                key: R(2),
            },
            I::MapGet {
                dst: Dst(R(7)),
                map: R(5),
                key: R(2),
            },
            I::IntAdd {
                dst: Dst(R(6)),
                left: R(6),
                right: R(7),
            },
            I::Return { value: Some(R(6)) },
        ],
    );
    check(&p, 141);
}
#[test]
fn lookup_edges_preserve_old_values_and_strict_get_reports_key() {
    for (key, expected) in [("x", 49), ("missing", 7)] {
        let p = program(
            &[1, 0, 4, 1, 0, 0],
            vec![
                s(0, "x"),
                c(1, 42),
                I::MapNew {
                    dst: Dst(R(2)),
                    entries: vec![(R(0), R(1))],
                },
                s(3, key),
                c(4, 7),
                I::MapLookup {
                    map: R(2),
                    key: R(3),
                    value: Dst(R(4)),
                    found: Target {
                        pc: 6,
                        moves: vec![(R(5), R(4))],
                    },
                    missing: Target {
                        pc: 8,
                        moves: vec![],
                    },
                },
                I::IntAdd {
                    dst: Dst(R(5)),
                    left: R(5),
                    right: R(4),
                },
                I::Return { value: Some(R(5)) },
                I::Return { value: Some(R(4)) },
            ],
        );
        check(&p, expected);
    }
    let p = program(
        &[4, 1, 0],
        vec![
            I::MapNew {
                dst: Dst(R(0)),
                entries: vec![],
            },
            s(1, "missing"),
            I::MapGet {
                dst: Dst(R(2)),
                map: R(0),
                key: R(1),
            },
            I::Return { value: Some(R(2)) },
        ],
    );
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(
            Executable::new(&p, mode).unwrap().run().unwrap_err().0,
            "Map key not found: \"missing\""
        );
    }
}

#[test]
fn enum_keys_keep_declaration_and_case_and_report_case_names() {
    let mut p = program(
        &[10, 10, 0, 11, 0, 3],
        vec![
            I::EnumCase {
                dst: Dst(R(0)),
                ty: okkama_sparrow::EnumRef(0),
                case: 0,
            },
            I::EnumCase {
                dst: Dst(R(1)),
                ty: okkama_sparrow::EnumRef(0),
                case: 1,
            },
            c(2, 42),
            I::MapNew {
                dst: Dst(R(3)),
                entries: vec![(R(1), R(2))],
            },
            I::EnumCmp {
                op: okkama_sparrow::Cmp::Ne,
                dst: Dst(R(5)),
                left: R(0),
                right: R(1),
            },
            I::Branch {
                condition: R(5),
                then_target: Target {
                    pc: 6,
                    moves: vec![],
                },
                else_target: Target {
                    pc: 8,
                    moves: vec![],
                },
            },
            I::MapGet {
                dst: Dst(R(4)),
                map: R(3),
                key: R(1),
            },
            I::Return { value: Some(R(4)) },
            c(2, 0),
            I::Return { value: Some(R(2)) },
        ],
    );
    p.types.extend([
        Type::Enum(okkama_sparrow::EnumRef(0)),
        Type::Map(TypeRef(10), TypeRef(0)),
    ]);
    p.enums.push(okkama_sparrow::EnumDecl {
        name: "Choice".into(),
        cases: vec!["First".into(), "Second".into()],
    });
    check(&p, 42);
    p.functions[0].code[6] = I::MapGet {
        dst: Dst(R(4)),
        map: R(3),
        key: R(0),
    };
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(
            Executable::new(&p, mode).unwrap().run().unwrap_err().0,
            "Map key not found: \"First\""
        );
    }
}
