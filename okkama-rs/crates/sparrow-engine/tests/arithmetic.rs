use okkama_sparrow::{
    Constant, Dst, FuncRef, Function, Instruction as I, Program, Register as R, Type, TypeRef,
};
use sparrow_engine::{Executable, Mode};

fn program(left: &str, right: &str, op: I) -> Program {
    Program {
        module: "arithmetic".into(),
        version: "1.0.0".into(),
        types: vec![Type::Int],
        entry: Some(FuncRef(0)),
        functions: vec![Function {
            symbol: "main".into(),
            is_async: false,
            native: None,
            intrinsic: None,
            registers: vec![TypeRef(0); 3],
            params: vec![],
            result: Some(TypeRef(0)),
            handlers: vec![],
            code: vec![
                I::Const {
                    dst: Dst(R(0)),
                    value: Constant::Int(left.into()),
                },
                I::Const {
                    dst: Dst(R(1)),
                    value: Constant::Int(right.into()),
                },
                op,
                I::Return { value: Some(R(2)) },
            ],
        }],
        ..Program::default()
    }
}

#[test]
fn exact_operations_cross_machine_integer_boundaries() {
    let ops: [fn() -> I; 4] = [
        || I::IntAdd {
            dst: Dst(R(2)),
            left: R(0),
            right: R(1),
        },
        || I::IntSub {
            dst: Dst(R(2)),
            left: R(0),
            right: R(1),
        },
        || I::IntMul {
            dst: Dst(R(2)),
            left: R(0),
            right: R(1),
        },
        || I::IntMod {
            dst: Dst(R(2)),
            left: R(0),
            right: R(1),
        },
    ];
    for (l, r) in [
        ("5", "2"),
        ("-5", "2"),
        ("5", "-2"),
        ("7", "-1"),
        ("0", "5"),
        ("-9223372036854775808", "7"),
        ("9223372036854775807", "1"),
        ("-9223372036854775808", "-1"),
        ("9223372036854775808", "-9223372036854775808"),
        ("123456789012345678901234567890", "98765432109876543210"),
    ] {
        let left = l.parse::<num_bigint::BigInt>().unwrap();
        let right = r.parse::<num_bigint::BigInt>().unwrap();
        let expected = [
            &left + &right,
            &left - &right,
            &left * &right,
            &left % &right,
        ];
        for (op, expected) in ops.iter().zip(expected) {
            for mode in [Mode::Cold, Mode::Jit] {
                let code = Executable::new(&program(l, r, op()), mode).unwrap();
                assert_eq!(code.run().unwrap().to_string(), expected.to_string());
                let mut session = code.start();
                loop {
                    if let sparrow_engine::Outcome::Returned(value) = session.resume(1).unwrap() {
                        assert_eq!(value.to_string(), expected.to_string());
                        break;
                    }
                }
            }
        }
    }
}

#[test]
fn zero_divisor_is_an_error_in_both_modes() {
    for (mode, left) in [
        (Mode::Cold, "12345678901234567890"),
        (Mode::Jit, "12345678901234567890"),
        (Mode::Cold, "5"),
        (Mode::Jit, "5"),
    ] {
        let code = Executable::new(
            &program(
                left,
                "0",
                I::IntMod {
                    dst: Dst(R(2)),
                    left: R(0),
                    right: R(1),
                },
            ),
            mode,
        )
        .unwrap();
        assert!(code.run().unwrap_err().0.contains("division by zero"));
    }
}

#[test]
fn collection_preserves_big_integers_in_suspended_callers() {
    let mut p = program(
        "1000000000000000000000000000000",
        "1",
        I::IntAdd {
            dst: Dst(R(2)),
            left: R(0),
            right: R(1),
        },
    );
    p.types.push(Type::Bool);
    p.functions[0].registers.push(TypeRef(0));
    p.functions[0].code.insert(
        3,
        I::Call {
            dst: Some(Dst(R(3))),
            function: FuncRef(1),
            args: vec![],
        },
    );
    let c = |r, v: &str| I::Const {
        dst: Dst(R(r)),
        value: Constant::Int(v.into()),
    };
    p.functions.push(Function {
        symbol: "allocate".into(),
        is_async: false,
        native: None,
        intrinsic: None,
        registers: vec![TypeRef(0), TypeRef(0), TypeRef(0), TypeRef(1), TypeRef(0)],
        params: vec![],
        result: Some(TypeRef(0)),
        handlers: vec![],
        code: vec![
            c(0, "400"),
            c(1, "1000000000000000000000000000000"),
            c(2, "1"),
            c(4, "0"),
            I::IntAdd {
                dst: Dst(R(1)),
                left: R(1),
                right: R(2),
            },
            I::IntSub {
                dst: Dst(R(0)),
                left: R(0),
                right: R(2),
            },
            I::IntCmp {
                op: okkama_sparrow::Cmp::Gt,
                dst: Dst(R(3)),
                left: R(0),
                right: R(4),
            },
            I::Branch {
                condition: R(3),
                then_target: okkama_sparrow::Target {
                    pc: 4,
                    moves: vec![],
                },
                else_target: okkama_sparrow::Target {
                    pc: 8,
                    moves: vec![],
                },
            },
            I::Return { value: Some(R(1)) },
        ],
    });
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        for budget in [1, 11, 4096] {
            let mut session = code.start();
            loop {
                if let sparrow_engine::Outcome::Returned(value) = session.resume(budget).unwrap() {
                    assert_eq!(value.to_string(), "1000000000000000000000000000001");
                    break;
                }
            }
        }
    }
}

#[test]
fn unary_and_word_operations_preserve_exact_and_fixed_width_contracts() {
    let cases = [
        (
            "-9223372036854775808",
            "0",
            I::IntNeg {
                dst: Dst(R(2)),
                operand: R(0),
            },
            "9223372036854775808",
        ),
        (
            "-9223372036854775808",
            "0",
            I::IntAbs {
                dst: Dst(R(2)),
                operand: R(0),
            },
            "9223372036854775808",
        ),
        (
            "-9999999999999999999999999",
            "0",
            I::Sign {
                dst: Dst(R(2)),
                operand: R(0),
            },
            "-1",
        ),
        (
            "0",
            "0",
            I::IntNot {
                dst: Dst(R(2)),
                operand: R(0),
            },
            "-1",
        ),
        (
            "1",
            "63",
            I::IntShl {
                dst: Dst(R(2)),
                left: R(0),
                right: R(1),
            },
            "-9223372036854775808",
        ),
        (
            "-1",
            "63",
            I::IntShr {
                dst: Dst(R(2)),
                left: R(0),
                right: R(1),
            },
            "-1",
        ),
        (
            "-1",
            "1",
            I::IntUshr {
                dst: Dst(R(2)),
                left: R(0),
                right: R(1),
            },
            "9223372036854775807",
        ),
        (
            "-1",
            "255",
            I::IntAnd {
                dst: Dst(R(2)),
                left: R(0),
                right: R(1),
            },
            "255",
        ),
        (
            "128",
            "255",
            I::IntXor {
                dst: Dst(R(2)),
                left: R(0),
                right: R(1),
            },
            "127",
        ),
    ];
    for mode in [Mode::Cold, Mode::Jit] {
        for (left, right, op, expected) in &cases {
            assert_eq!(
                Executable::new(&program(left, right, op.clone()), mode)
                    .unwrap()
                    .run()
                    .unwrap()
                    .to_string(),
                *expected
            );
        }
        for count in ["-1", "64", "9999999999999999999999"] {
            let p = program(
                "9223372036854775808",
                count,
                I::IntShl {
                    dst: Dst(R(2)),
                    left: R(0),
                    right: R(1),
                },
            );
            assert!(
                Executable::new(&p, mode)
                    .unwrap()
                    .run()
                    .unwrap_err()
                    .0
                    .contains("Shift count")
            );
        }
        let p = program(
            "9223372036854775808",
            "0",
            I::IntAnd {
                dst: Dst(R(2)),
                left: R(0),
                right: R(1),
            },
        );
        assert!(
            Executable::new(&p, mode)
                .unwrap()
                .run()
                .unwrap_err()
                .0
                .contains("signed-64")
        );
    }
}
