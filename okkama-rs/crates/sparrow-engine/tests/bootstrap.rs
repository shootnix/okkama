use okkama_sparrow::Target;
use okkama_sparrow::{
    Constant, Dst, FuncRef, Function, Instruction as I, Program, Register as R, Type, TypeRef,
};
use sparrow_engine::{Executable, Mode, Outcome};

fn program(value: &str) -> Program {
    Program {
        module: "new_engine_test".into(),
        version: "1.0.0".into(),
        types: vec![Type::Int],
        entry: Some(FuncRef(0)),
        functions: vec![Function {
            symbol: "main".into(),
            is_async: false,
            native: None,
            intrinsic: None,
            registers: vec![TypeRef(0); 2],
            params: vec![],
            result: Some(TypeRef(0)),
            handlers: vec![],
            code: vec![
                I::Const {
                    dst: Dst(R(0)),
                    value: Constant::Int(value.into()),
                },
                I::Move {
                    dst: Dst(R(1)),
                    source: R(0),
                },
                I::Return { value: Some(R(1)) },
            ],
        }],
        ..Program::default()
    }
}

#[test]
fn decoded_spb_has_matching_cold_and_native_results() {
    for value in [i64::MIN, -1, 0, 42, i64::MAX] {
        let bytes = okkama_sparrow::encode(&program(&value.to_string())).unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            let code = Executable::decode(&bytes, mode).unwrap();
            for _ in 0..3 {
                assert_eq!(code.run().unwrap(), value);
            }
        }
    }
}

#[test]
fn malformed_and_unsupported_programs_do_not_fall_back() {
    assert!(Executable::decode(b"not SPB", Mode::Jit).is_err());
    let mut invalid = program("1");
    invalid.functions[0].code[2] = I::Return { value: Some(R(99)) };
    assert!(Executable::new(&invalid, Mode::Jit).is_err());
    let mut arithmetic = program("1");
    arithmetic.functions[0].code[1] = I::Await {
        dst: Some(Dst(R(1))),
        future: R(0),
    };
    for mode in [Mode::Cold, Mode::Jit] {
        assert!(
            Executable::new(&arithmetic, mode)
                .unwrap_err()
                .0
                .contains("invalid operand types")
        );
        assert_eq!(
            Executable::new(&program("9223372036854775808"), mode)
                .unwrap()
                .run()
                .unwrap()
                .to_string(),
            "9223372036854775808"
        );
    }
}

#[test]
fn overwritten_source_does_not_change_copied_value() {
    let mut p = program("17");
    p.functions[0].code.insert(
        2,
        I::Const {
            dst: Dst(R(0)),
            value: Constant::Int("99".into()),
        },
    );
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&p, mode).unwrap().run().unwrap(), 17);
    }
}

#[test]
fn standalone_cli_executes_spb_and_preserves_exit_status() {
    let path =
        std::env::temp_dir().join(format!("sparrow-next-bootstrap-{}.spb", std::process::id()));
    std::fs::write(&path, okkama_sparrow::encode(&program("42")).unwrap()).unwrap();
    for args in [vec![], vec!["--cold"]] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_sparrow-next"))
            .args(args)
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(42),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn suspension_preserves_backedges_and_parallel_edge_moves() {
    let mut p = program("17");
    p.types.push(Type::Bool);
    p.functions[0].registers.push(TypeRef(1));
    let swap = Target {
        pc: 4,
        moves: vec![(R(0), R(1)), (R(1), R(0))],
    };
    p.functions[0].code = vec![
        I::Const {
            dst: Dst(R(0)),
            value: Constant::Int("17".into()),
        },
        I::Const {
            dst: Dst(R(1)),
            value: Constant::Int("99".into()),
        },
        I::Const {
            dst: Dst(R(2)),
            value: Constant::Bool(false),
        },
        I::Jump {
            target: swap.clone(),
        },
        I::BoolNot {
            dst: Dst(R(2)),
            operand: R(2),
        },
        I::Branch {
            condition: R(2),
            then_target: swap,
            else_target: Target {
                pc: 6,
                moves: vec![],
            },
        },
        I::Return { value: Some(R(0)) },
    ];
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        for budget in [1, 2, 3, 4, 8, 9, 32, u64::MAX] {
            let mut session = code.start();
            let mut yields = 0;
            loop {
                assert_eq!(session.resume(0).unwrap(), Outcome::Yielded);
                match session.resume(budget).unwrap() {
                    Outcome::Returned(value) => {
                        assert_eq!(value, 17);
                        break;
                    }
                    Outcome::Waiting => panic!("pure scalar code requested external I/O"),
                    Outcome::Yielded => {
                        yields += 1;
                        assert!(yields < 10);
                    }
                }
            }
            if budget == 1 {
                assert_eq!(yields, 8);
            }
            assert_eq!(session.resume(0).unwrap(), Outcome::Returned(17.into()));
        }
    }
}

#[test]
fn nonterminating_bytecode_yields_without_growing_the_host_stack() {
    let mut p = program("0");
    p.functions[0].code = vec![I::Jump {
        target: Target {
            pc: 0,
            moves: vec![],
        },
    }];
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut session = code.start();
        for _ in 0..1000 {
            assert_eq!(session.resume(1000).unwrap(), Outcome::Yielded);
        }
    }
}

#[test]
fn only_live_values_cross_a_suspension_boundary() {
    let mut p = program("17");
    p.functions[0].code.insert(
        2,
        I::Const {
            dst: Dst(R(0)),
            value: Constant::Int("999".into()),
        },
    );
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut session = code.start();
        assert_eq!(session.resume(2).unwrap(), Outcome::Yielded);
        assert_eq!(session.live_registers().collect::<Vec<_>>(), vec![1]);
        assert_eq!(session.resume(2).unwrap(), Outcome::Returned(17.into()));
        assert!(session.live_registers().next().is_none());
    }
}
