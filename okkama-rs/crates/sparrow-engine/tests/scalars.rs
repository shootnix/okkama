use okkama_sparrow::{
    Cmp, Constant, Dst, FuncRef, Function, Instruction as I, Program, Register as R, Target, Type,
    TypeRef,
};
use sparrow_engine::{Executable, Mode, Outcome};

fn function(registers: Vec<TypeRef>, code: Vec<I>) -> Function {
    Function {
        symbol: "main".into(),
        is_async: false,
        native: None,
        intrinsic: None,
        registers,
        params: vec![],
        result: Some(TypeRef(0)),
        handlers: vec![],
        code,
    }
}
fn program(f: Function) -> Program {
    Program {
        module: "scalars".into(),
        version: "1.0.0".into(),
        types: vec![Type::Int, Type::Float, Type::Bool, Type::Byte, Type::Char],
        entry: Some(FuncRef(0)),
        functions: vec![f],
        ..Program::default()
    }
}
fn float(dst: u32, value: f64) -> I {
    I::Const {
        dst: Dst(R(dst)),
        value: Constant::Float(value.to_bits()),
    }
}
fn int(dst: u32, value: i64) -> I {
    I::Const {
        dst: Dst(R(dst)),
        value: Constant::Int(value.to_string()),
    }
}
fn comparison_tail(condition: u32, yes: u32, no: u32, pc: u32) -> Vec<I> {
    vec![
        I::Branch {
            condition: R(condition),
            then_target: Target { pc, moves: vec![] },
            else_target: Target {
                pc: pc + 1,
                moves: vec![],
            },
        },
        I::Return {
            value: Some(R(yes)),
        },
        I::Return { value: Some(R(no)) },
    ]
}
fn floats(left: f64, right: f64, op: I, expected: f64) -> Program {
    let mut code = vec![
        float(0, left),
        float(1, right),
        op,
        float(3, expected),
        I::FloatCmp {
            op: Cmp::Eq,
            dst: Dst(R(4)),
            left: R(2),
            right: R(3),
        },
        int(5, 0),
        int(6, 1),
    ];
    code.extend(comparison_tail(4, 5, 6, 8));
    program(function(
        vec![
            TypeRef(1),
            TypeRef(1),
            TypeRef(1),
            TypeRef(1),
            TypeRef(2),
            TypeRef(0),
            TypeRef(0),
        ],
        code,
    ))
}

#[test]
fn finite_float_arithmetic_uses_native_and_resumable_paths() {
    let cases = [
        (
            0.1,
            0.2,
            I::FloatAdd {
                dst: Dst(R(2)),
                left: R(0),
                right: R(1),
            },
            0.1 + 0.2,
        ),
        (
            1.0,
            3.0,
            I::FloatDiv {
                dst: Dst(R(2)),
                left: R(0),
                right: R(1),
            },
            1.0 / 3.0,
        ),
        (
            -5.5,
            2.0,
            I::FloatMod {
                dst: Dst(R(2)),
                left: R(0),
                right: R(1),
            },
            -1.5,
        ),
        (
            2.0,
            10.0,
            I::FloatPow {
                dst: Dst(R(2)),
                left: R(0),
                right: R(1),
            },
            1024.0,
        ),
        (
            9.0,
            0.0,
            I::FloatSqrt {
                dst: Dst(R(2)),
                operand: R(0),
            },
            3.0,
        ),
        (
            -0.0,
            0.0,
            I::FloatNeg {
                dst: Dst(R(2)),
                operand: R(0),
            },
            0.0,
        ),
    ];
    for mode in [Mode::Cold, Mode::Jit] {
        for (left, right, op, expected) in &cases {
            let code =
                Executable::new(&floats(*left, *right, op.clone(), *expected), mode).unwrap();
            for budget in [1, 3, 100] {
                let mut session = code.start();
                loop {
                    if let Outcome::Returned(value) = session.resume(budget).unwrap() {
                        assert_eq!(value, 0);
                        break;
                    }
                }
            }
        }
    }
}

#[test]
fn float_failures_are_errors_not_nonfinite_values() {
    let cases = [
        (
            f64::MAX,
            2.0,
            I::FloatMul {
                dst: Dst(R(2)),
                left: R(0),
                right: R(1),
            },
            "finite",
        ),
        (
            1.0,
            -0.0,
            I::FloatDiv {
                dst: Dst(R(2)),
                left: R(0),
                right: R(1),
            },
            "Division by zero",
        ),
        (
            1.0,
            0.0,
            I::FloatMod {
                dst: Dst(R(2)),
                left: R(0),
                right: R(1),
            },
            "Modulo by zero",
        ),
        (
            -1.0,
            0.0,
            I::FloatSqrt {
                dst: Dst(R(2)),
                operand: R(0),
            },
            "sqrt",
        ),
    ];
    for mode in [Mode::Cold, Mode::Jit] {
        for (left, right, op, expected) in &cases {
            assert!(
                Executable::new(&floats(*left, *right, op.clone(), 0.0), mode)
                    .unwrap()
                    .run()
                    .unwrap_err()
                    .0
                    .contains(expected)
            );
        }
    }
}

#[test]
fn scalar_conversions_and_unicode_comparisons() {
    for mode in [Mode::Cold, Mode::Jit] {
        let p = program(function(
            vec![TypeRef(1), TypeRef(3), TypeRef(0)],
            vec![
                float(0, 255.9),
                I::ByteFrom {
                    dst: Dst(R(1)),
                    source: R(0),
                    context: None,
                },
                I::Convert {
                    dst: Dst(R(2)),
                    source: R(1),
                },
                I::Return { value: Some(R(2)) },
            ],
        ));
        assert_eq!(Executable::new(&p, mode).unwrap().run().unwrap(), 255);
        let p = program(function(
            vec![TypeRef(1), TypeRef(0)],
            vec![
                float(0, -42.9),
                I::Convert {
                    dst: Dst(R(1)),
                    source: R(0),
                },
                I::Return { value: Some(R(1)) },
            ],
        ));
        assert_eq!(Executable::new(&p, mode).unwrap().run().unwrap(), -42);
        let mut code = vec![
            I::Const {
                dst: Dst(R(0)),
                value: Constant::Char('é'),
            },
            I::Const {
                dst: Dst(R(1)),
                value: Constant::Char('水'),
            },
            I::CharCmp {
                op: Cmp::Lt,
                dst: Dst(R(2)),
                left: R(0),
                right: R(1),
            },
            int(3, 0),
            int(4, 1),
        ];
        code.extend(comparison_tail(2, 3, 4, 6));
        let p = program(function(
            vec![TypeRef(4), TypeRef(4), TypeRef(2), TypeRef(0), TypeRef(0)],
            code,
        ));
        assert_eq!(Executable::new(&p, mode).unwrap().run().unwrap(), 0);
    }
}

#[test]
fn invalid_typed_bytecode_is_rejected_before_native_codegen() {
    let mut bad_move = program(function(
        vec![TypeRef(1), TypeRef(0)],
        vec![
            float(0, 1.0),
            I::Move {
                dst: Dst(R(1)),
                source: R(0),
            },
            I::Return { value: Some(R(1)) },
        ],
    ));
    // The shared verifier accepts initialized/bounded registers; the new typed
    // verifier must reject the ABI mismatch in both modes without panicking.
    bad_move.verify().unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert!(
            Executable::new(&bad_move, mode)
                .unwrap_err()
                .0
                .contains("operand types")
        );
    }
    bad_move.functions[0].code[1] = I::Convert {
        dst: Dst(R(1)),
        source: R(0),
    };
    bad_move.functions[0].code.insert(
        2,
        I::Call {
            dst: Some(Dst(R(1))),
            function: FuncRef(1),
            args: vec![R(1), R(1)],
        },
    );
    let mut callee = function(vec![TypeRef(0)], vec![I::Return { value: Some(R(0)) }]);
    callee.symbol = "one_parameter".into();
    callee.params = vec![R(0)];
    bad_move.functions.push(callee);
    bad_move.verify().unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert!(
            Executable::new(&bad_move, mode)
                .unwrap_err()
                .0
                .contains("call signature")
        );
    }
}

#[test]
fn byte_bit_operations_keep_eight_bit_width_and_integer_counts() {
    use okkama_sparrow::BitOp;
    for mode in [Mode::Cold, Mode::Jit] {
        for (op, expected) in [
            (BitOp::BitCount, 2),
            (BitOp::LeadingZeros, 0),
            (BitOp::TrailingZeros, 0),
            (BitOp::RotateLeft, 3),
            (BitOp::RotateRight, 192),
            (BitOp::ReverseBits, 129),
            (BitOp::SwapBytes, 129),
        ] {
            let rotates = matches!(op, BitOp::RotateLeft | BitOp::RotateRight);
            let p = program(function(
                vec![
                    TypeRef(3),
                    TypeRef(0),
                    TypeRef(if op.counts() { 0 } else { 3 }),
                    TypeRef(0),
                ],
                vec![
                    I::Const {
                        dst: Dst(R(0)),
                        value: Constant::Byte(129),
                    },
                    int(1, 1),
                    I::Bits {
                        op,
                        byte: true,
                        dst: Dst(R(2)),
                        value: R(0),
                        count: rotates.then_some(R(1)),
                    },
                    if op.counts() {
                        I::Move {
                            dst: Dst(R(3)),
                            source: R(2),
                        }
                    } else {
                        I::Convert {
                            dst: Dst(R(3)),
                            source: R(2),
                        }
                    },
                    I::Return { value: Some(R(3)) },
                ],
            ));
            assert_eq!(Executable::new(&p, mode).unwrap().run().unwrap(), expected);
        }
        for (shift, expected) in [
            (
                I::ByteShl {
                    dst: Dst(R(2)),
                    left: R(0),
                    right: R(1),
                },
                2,
            ),
            (
                I::ByteUshr {
                    dst: Dst(R(2)),
                    left: R(0),
                    right: R(1),
                },
                64,
            ),
        ] {
            let p = program(function(
                vec![TypeRef(3), TypeRef(0), TypeRef(3), TypeRef(0)],
                vec![
                    I::Const {
                        dst: Dst(R(0)),
                        value: Constant::Byte(129),
                    },
                    int(1, 1),
                    shift,
                    I::Convert {
                        dst: Dst(R(3)),
                        source: R(2),
                    },
                    I::Return { value: Some(R(3)) },
                ],
            ));
            assert_eq!(Executable::new(&p, mode).unwrap().run().unwrap(), expected);
        }
    }
}
