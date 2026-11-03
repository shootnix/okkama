use okkama_sparrow::{
    Constant, Dst, FuncRef, Function, Instruction as I, Program, Register as R, Type, TypeRef,
};
use sparrow_engine::{Executable, Mode, Outcome};
fn int(r: u32, n: i64) -> I {
    I::Const {
        dst: Dst(R(r)),
        value: Constant::Int(n.to_string()),
    }
}
fn str_(r: u32, s: &str) -> I {
    I::Const {
        dst: Dst(R(r)),
        value: Constant::Str(s.into()),
    }
}
fn program(types: &[u32], code: Vec<I>) -> Program {
    Program {
        module: "arrays".into(),
        version: "1.0.0".into(),
        entry: Some(FuncRef(0)),
        types: vec![
            Type::Int,
            Type::Str,
            Type::Byte,
            Type::Array(TypeRef(0)),
            Type::Builder(TypeRef(0)),
            Type::Array(TypeRef(1)),
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
            registers: types.iter().map(|t| TypeRef(*t)).collect(),
            code,
        }],
        ..Program::default()
    }
}
fn check(p: Program, expected: i64) {
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut s = code.start();
        loop {
            if let Outcome::Returned(n) = s.resume(1).unwrap() {
                assert_eq!(n, expected);
                break;
            }
        }
    }
}
#[test]
fn builder_aliases_share_updates_and_finish_transfers_without_mutating_old_array() {
    check(
        program(
            &[4, 4, 0, 3, 3, 0, 0, 0],
            vec![
                I::BuilderNew { dst: Dst(R(0)) },
                I::Move {
                    dst: Dst(R(1)),
                    source: R(0),
                },
                int(2, 17),
                I::BuilderPush {
                    dst: Dst(R(0)),
                    builder: R(0),
                    value: R(2),
                },
                I::BuilderFinish {
                    dst: Dst(R(3)),
                    builder: R(1),
                },
                int(2, 29),
                I::BuilderPush {
                    dst: Dst(R(1)),
                    builder: R(1),
                    value: R(2),
                },
                I::BuilderFinish {
                    dst: Dst(R(4)),
                    builder: R(0),
                },
                int(5, 0),
                I::ArrayGet {
                    dst: Dst(R(6)),
                    array: R(3),
                    index: R(5),
                },
                I::ArrayGet {
                    dst: Dst(R(7)),
                    array: R(4),
                    index: R(5),
                },
                I::IntAdd {
                    dst: Dst(R(2)),
                    left: R(6),
                    right: R(7),
                },
                I::Return { value: Some(R(2)) },
            ],
        ),
        46,
    );
}
#[test]
fn slices_and_builder_append_preserve_element_order() {
    check(
        program(
            &[0, 0, 0, 3, 3, 4, 3, 0],
            vec![
                int(0, 7),
                int(1, 11),
                int(2, 19),
                I::ArrayNew {
                    dst: Dst(R(3)),
                    elements: vec![R(0), R(1), R(2)],
                },
                int(0, 1),
                int(1, 3),
                I::ArraySlice {
                    dst: Dst(R(4)),
                    array: R(3),
                    start: R(0),
                    end: R(1),
                },
                I::BuilderNew { dst: Dst(R(5)) },
                I::BuilderAppend {
                    dst: Dst(R(5)),
                    builder: R(5),
                    array: R(4),
                },
                I::BuilderFinish {
                    dst: Dst(R(6)),
                    builder: R(5),
                },
                int(0, 0),
                I::ArrayGet {
                    dst: Dst(R(7)),
                    array: R(6),
                    index: R(0),
                },
                I::Return { value: Some(R(7)) },
            ],
        ),
        11,
    );
}
#[test]
fn utf8_and_empty_separator_split_have_distinct_byte_and_codepoint_lengths() {
    check(
        program(
            &[1, 1, 5, 6, 0, 0, 0],
            vec![
                str_(0, "A🙂e\u{301}"),
                str_(1, ""),
                I::StrSplit {
                    dst: Dst(R(2)),
                    text: R(0),
                    separator: R(1),
                },
                I::StrUtf8 {
                    dst: Dst(R(3)),
                    text: R(0),
                },
                I::ArrayLen {
                    dst: Dst(R(4)),
                    array: R(2),
                },
                I::ArrayLen {
                    dst: Dst(R(5)),
                    array: R(3),
                },
                I::IntAdd {
                    dst: Dst(R(6)),
                    left: R(4),
                    right: R(5),
                },
                I::Return { value: Some(R(6)) },
            ],
        ),
        12,
    );
    check(
        program(
            &[1, 1, 5, 1, 0],
            vec![
                str_(0, "A🙂e\u{301}"),
                str_(1, ""),
                I::StrSplit {
                    dst: Dst(R(2)),
                    text: R(0),
                    separator: R(1),
                },
                I::ArrayJoin {
                    dst: Dst(R(3)),
                    array: R(2),
                    separator: R(1),
                },
                I::StrLen {
                    dst: Dst(R(4)),
                    text: R(3),
                },
                I::Return { value: Some(R(4)) },
            ],
        ),
        4,
    );
}
#[test]
fn enormous_array_indices_are_rejected_without_truncation() {
    let p = program(
        &[0, 3, 0],
        vec![
            int(0, 42),
            I::ArrayNew {
                dst: Dst(R(1)),
                elements: vec![R(0)],
            },
            I::Const {
                dst: Dst(R(0)),
                value: Constant::Int("18446744073709551616".into()),
            },
            I::ArrayGet {
                dst: Dst(R(2)),
                array: R(1),
                index: R(0),
            },
            I::Return { value: Some(R(2)) },
        ],
    );
    for mode in [Mode::Cold, Mode::Jit] {
        assert!(
            Executable::new(&p, mode)
                .unwrap()
                .run()
                .unwrap_err()
                .0
                .contains("Array index out of bounds")
        );
    }
}

#[test]
fn fallback_edges_read_old_registers_before_binding_the_found_value() {
    for (index, expected) in [(0, 49), (-1, 7), (2, 7)] {
        check(
            program(
                &[0, 3, 0, 0, 0],
                vec![
                    int(0, 42),
                    I::ArrayNew {
                        dst: Dst(R(1)),
                        elements: vec![R(0)],
                    },
                    int(2, index),
                    int(3, 7),
                    I::ArrayLookup {
                        array: R(1),
                        index: R(2),
                        value: Dst(R(3)),
                        found: okkama_sparrow::Target {
                            pc: 5,
                            moves: vec![(R(4), R(3)), (R(3), R(0))],
                        },
                        missing: okkama_sparrow::Target {
                            pc: 7,
                            moves: vec![],
                        },
                    },
                    I::IntAdd {
                        dst: Dst(R(4)),
                        left: R(4),
                        right: R(3),
                    },
                    I::Return { value: Some(R(4)) },
                    I::Return { value: Some(R(3)) },
                ],
            ),
            expected,
        );
    }
}

#[test]
fn exact_ranges_cross_machine_boundaries_without_overflow() {
    let mut p = program(
        &[0, 0, 7, 0, 0],
        vec![
            int(0, i64::MAX),
            int(1, i64::MAX),
            I::RangeNew {
                dst: Dst(R(2)),
                start: R(0),
                end: R(1),
                inclusive: true,
            },
            I::RangePart {
                dst: Dst(R(3)),
                range: R(2),
                end: true,
            },
            I::RangePart {
                dst: Dst(R(4)),
                range: R(2),
                end: false,
            },
            I::IntSub {
                dst: Dst(R(3)),
                left: R(3),
                right: R(4),
            },
            I::Return { value: Some(R(3)) },
        ],
    );
    p.types.push(Type::Range);
    check(p, 1);
    let p = program(
        &[0, 0, 4, 3, 0, 0],
        vec![
            int(0, i64::MAX - 1),
            I::Const {
                dst: Dst(R(1)),
                value: Constant::Int("9223372036854775808".into()),
            },
            I::BuilderNew { dst: Dst(R(2)) },
            I::BuilderRange {
                dst: Dst(R(2)),
                builder: R(2),
                start: R(0),
                end: R(1),
                inclusive: true,
            },
            I::BuilderFinish {
                dst: Dst(R(3)),
                builder: R(2),
            },
            int(4, 2),
            I::ArrayGet {
                dst: Dst(R(5)),
                array: R(3),
                index: R(4),
            },
            I::Return { value: Some(R(5)) },
        ],
    );
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(
            Executable::new(&p, mode)
                .unwrap()
                .run()
                .unwrap()
                .to_string(),
            "9223372036854775808"
        );
    }
}
#[test]
fn empty_and_reversed_builder_ranges_keep_the_contract() {
    for inclusive in [false, true] {
        check(
            program(
                &[0, 4, 3, 0],
                vec![
                    int(0, 7),
                    I::BuilderNew { dst: Dst(R(1)) },
                    I::BuilderRange {
                        dst: Dst(R(1)),
                        builder: R(1),
                        start: R(0),
                        end: R(0),
                        inclusive,
                    },
                    I::BuilderFinish {
                        dst: Dst(R(2)),
                        builder: R(1),
                    },
                    I::ArrayLen {
                        dst: Dst(R(3)),
                        array: R(2),
                    },
                    I::Return { value: Some(R(3)) },
                ],
            ),
            i64::from(inclusive),
        );
    }
    let p = program(
        &[0, 0, 4],
        vec![
            int(0, 2),
            int(1, 1),
            I::BuilderNew { dst: Dst(R(2)) },
            I::BuilderRange {
                dst: Dst(R(2)),
                builder: R(2),
                start: R(0),
                end: R(1),
                inclusive: false,
            },
            I::Return { value: Some(R(0)) },
        ],
    );
    for mode in [Mode::Cold, Mode::Jit] {
        assert!(
            Executable::new(&p, mode)
                .unwrap()
                .run()
                .unwrap_err()
                .0
                .contains("Range start must be less than or equal")
        );
    }
}
