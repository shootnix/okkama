use okkama_sparrow::{
    Cmp, Constant, Dst, FuncRef, Function, Instruction as I, Program, Register as R, Target, Type,
    TypeRef,
};
use sparrow_engine::{Executable, Mode, Outcome};
fn c(dst: u32, n: &str) -> I {
    I::Const {
        dst: Dst(R(dst)),
        value: Constant::Int(n.into()),
    }
}
fn make(a: &str, b: &str, c_: &str, d: &str, op: I, n: &str, den: &str) -> Program {
    Program {
        module: "rational".into(),
        version: "1.0.0".into(),
        types: vec![Type::Int, Type::Rat, Type::Bool],
        entry: Some(FuncRef(0)),
        functions: vec![Function {
            symbol: "main".into(),
            is_async: false,
            native: None,
            intrinsic: None,
            params: vec![],
            handlers: vec![],
            result: Some(TypeRef(0)),
            registers: vec![
                TypeRef(0),
                TypeRef(0),
                TypeRef(1),
                TypeRef(0),
                TypeRef(0),
                TypeRef(1),
                TypeRef(1),
                TypeRef(0),
                TypeRef(0),
                TypeRef(1),
                TypeRef(2),
                TypeRef(0),
                TypeRef(0),
            ],
            code: vec![
                c(0, a),
                c(1, b),
                I::RatNew {
                    dst: Dst(R(2)),
                    left: R(0),
                    right: R(1),
                },
                c(3, c_),
                c(4, d),
                I::RatNew {
                    dst: Dst(R(5)),
                    left: R(3),
                    right: R(4),
                },
                op,
                c(7, n),
                c(8, den),
                I::RatNew {
                    dst: Dst(R(9)),
                    left: R(7),
                    right: R(8),
                },
                I::RatCmp {
                    op: Cmp::Eq,
                    dst: Dst(R(10)),
                    left: R(6),
                    right: R(9),
                },
                c(11, "0"),
                c(12, "1"),
                I::Branch {
                    condition: R(10),
                    then_target: Target {
                        pc: 14,
                        moves: vec![],
                    },
                    else_target: Target {
                        pc: 15,
                        moves: vec![],
                    },
                },
                I::Return { value: Some(R(11)) },
                I::Return { value: Some(R(12)) },
            ],
        }],
        ..Program::default()
    }
}
#[test]
fn exact_rational_operations_in_both_engines() {
    let cases = [
        (
            "1",
            "2",
            "1",
            "3",
            I::RatAdd {
                dst: Dst(R(6)),
                left: R(2),
                right: R(5),
            },
            "5",
            "6",
        ),
        (
            "1",
            "2",
            "1",
            "3",
            I::RatSub {
                dst: Dst(R(6)),
                left: R(2),
                right: R(5),
            },
            "1",
            "6",
        ),
        (
            "-7",
            "2",
            "1",
            "1",
            I::RatMod {
                dst: Dst(R(6)),
                left: R(2),
                right: R(5),
            },
            "-1",
            "2",
        ),
        (
            "1",
            "-2",
            "-3",
            "4",
            I::RatDiv {
                dst: Dst(R(6)),
                left: R(2),
                right: R(5),
            },
            "2",
            "3",
        ),
        (
            "1000000000000000000000000000000",
            "3",
            "3",
            "2",
            I::RatMul {
                dst: Dst(R(6)),
                left: R(2),
                right: R(5),
            },
            "500000000000000000000000000000",
            "1",
        ),
        (
            "-2147483648",
            "1",
            "1",
            "1",
            I::RatNeg {
                dst: Dst(R(6)),
                operand: R(2),
            },
            "2147483648",
            "1",
        ),
    ];
    for mode in [Mode::Cold, Mode::Jit] {
        for (a, b, c, d, op, n, den) in &cases {
            let code = Executable::new(&make(a, b, c, d, op.clone(), n, den), mode).unwrap();
            for budget in [1, 100] {
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
fn rational_zero_divisors_are_reported_without_library_panics() {
    for mode in [Mode::Cold, Mode::Jit] {
        let p = make(
            "1",
            "0",
            "1",
            "1",
            I::RatAdd {
                dst: Dst(R(6)),
                left: R(2),
                right: R(5),
            },
            "0",
            "1",
        );
        assert!(
            Executable::new(&p, mode)
                .unwrap()
                .run()
                .unwrap_err()
                .0
                .contains("Division by zero")
        );
        let p = make(
            "1",
            "2",
            "0",
            "1",
            I::RatMod {
                dst: Dst(R(6)),
                left: R(2),
                right: R(5),
            },
            "0",
            "1",
        );
        assert!(
            Executable::new(&p, mode)
                .unwrap()
                .run()
                .unwrap_err()
                .0
                .contains("Modulo by zero")
        );
    }
}
