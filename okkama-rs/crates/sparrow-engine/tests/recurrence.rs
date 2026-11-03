use okkama_sparrow::{
    Cmp, Constant, Dst, FuncRef, Function, Instruction as I, Program, Register as R, Target, Type,
    TypeRef,
};
use sparrow_engine::{Executable, Mode, Outcome};

fn c(dst: u32, n: i64) -> I {
    I::Const {
        dst: Dst(R(dst)),
        value: Constant::Int(n.to_string()),
    }
}
fn factorial(n: i64, unit: i64) -> Program {
    let make = |name: &str, registers, params, code| Function {
        symbol: name.into(),
        is_async: false,
        native: None,
        intrinsic: None,
        registers,
        params,
        result: Some(TypeRef(0)),
        code,
        handlers: vec![],
    };
    Program {
        module: "recurrence".into(),
        version: "1.0.0".into(),
        types: vec![Type::Int, Type::Bool],
        entry: Some(FuncRef(0)),
        functions: vec![
            make(
                "main",
                vec![TypeRef(0); 2],
                vec![],
                vec![
                    c(0, n),
                    I::Call {
                        dst: Some(Dst(R(1))),
                        function: FuncRef(1),
                        args: vec![R(0)],
                    },
                    I::Return { value: Some(R(1)) },
                ],
            ),
            make(
                "product",
                vec![
                    TypeRef(0),
                    TypeRef(0),
                    TypeRef(1),
                    TypeRef(0),
                    TypeRef(0),
                    TypeRef(0),
                ],
                vec![R(0)],
                vec![
                    c(1, unit),
                    I::IntCmp {
                        op: Cmp::Le,
                        dst: Dst(R(2)),
                        left: R(0),
                        right: R(1),
                    },
                    I::Branch {
                        condition: R(2),
                        then_target: Target {
                            pc: 3,
                            moves: vec![],
                        },
                        else_target: Target {
                            pc: 4,
                            moves: vec![],
                        },
                    },
                    I::Return { value: Some(R(1)) },
                    I::IntSub {
                        dst: Dst(R(3)),
                        left: R(0),
                        right: R(1),
                    },
                    I::Call {
                        dst: Some(Dst(R(4))),
                        function: FuncRef(1),
                        args: vec![R(3)],
                    },
                    I::IntMul {
                        dst: Dst(R(5)),
                        left: R(0),
                        right: R(4),
                    },
                    I::Return { value: Some(R(5)) },
                ],
            ),
        ],
        ..Program::default()
    }
}

#[test]
fn exact_factorial_becomes_an_accumulator_loop() {
    for n in [-2, 0, 1, 2, 20, 100, 300] {
        let expected = (2..=n).fold(num_bigint::BigInt::from(1), |a, v| a * v);
        for mode in [Mode::Cold, Mode::Jit] {
            let code = Executable::new(&factorial(n, 1), mode).unwrap();
            assert_eq!(code.recurrence_loop_count(), 1);
            let mut session = code.start();
            loop {
                let result = session.resume(7).unwrap();
                assert_eq!(session.continuation_depth(), 0);
                assert_eq!(session.saved_value_count(), 0);
                if let Outcome::Returned(value) = result {
                    assert_eq!(value.to_string(), expected.to_string());
                    break;
                }
            }
        }
    }
}

#[test]
fn non_unit_base_and_step_are_preserved_in_the_fold() {
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&factorial(4, 2), mode).unwrap();
        assert_eq!(code.recurrence_loop_count(), 1);
        assert_eq!(code.run().unwrap(), 8);
    }
}

#[test]
fn non_associative_recurrence_keeps_its_original_continuations() {
    let mut program = factorial(4, 1);
    program.functions[1].code[6] = I::IntSub {
        dst: Dst(R(5)),
        left: R(0),
        right: R(4),
    };
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&program, mode).unwrap();
        assert_eq!(code.recurrence_loop_count(), 0);
        assert_eq!(code.run().unwrap(), 2);
    }
}

#[test]
fn compiler_cfg_with_receiver_and_phi_return_folds_a_million_calls() {
    let mut program =
        okkama_sparrow::assemble(include_str!("fixtures/recurrence-sum.asm")).unwrap();
    // Keep the compiler's method CFG intact; return its result instead of printing.
    let main = &mut program.functions[2];
    main.code.truncate(4);
    main.code.push(I::Return { value: Some(R(3)) });
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&program, mode).unwrap();
        assert_eq!(code.recurrence_loop_count(), 1);
        let mut session = code.start();
        loop {
            let outcome = session.resume(10_000).unwrap();
            assert!(session.continuation_depth() <= 1);
            if let Outcome::Returned(value) = outcome {
                assert_eq!(value.to_string(), "500000500000");
                break;
            }
        }
    }
}
