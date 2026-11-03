use okkama_sparrow::{
    Cmp, Constant, Dst, FuncRef, Function, Instruction as I, Program, Register as R, Target, Type,
    TypeRef,
};
use sparrow_engine::{Executable, Mode, Outcome};
fn target(pc: u32, moves: Vec<(R, R)>) -> Target {
    Target { pc, moves }
}
fn constant(dst: u32, n: &str) -> I {
    I::Const {
        dst: Dst(R(dst)),
        value: Constant::Int(n.into()),
    }
}
#[test]
fn region_edges_preserve_parallel_moves_big_tags_and_exact_instruction_budgets() {
    let mut code = vec![
        constant(0, "0"),
        constant(1, "5"),
        constant(2, "9223372036854775808"),
        constant(3, "17"),
        constant(4, "1"),
        I::Jump {
            target: target(62, vec![]),
        },
    ];
    while code.len() < 62 {
        code.push(constant(6, "99"));
    }
    code.push(I::IntAdd {
        dst: Dst(R(0)),
        left: R(0),
        right: R(4),
    });
    code.push(I::Jump {
        target: target(130, vec![(R(2), R(3)), (R(3), R(2))]),
    });
    while code.len() < 130 {
        code.push(constant(6, "88"));
    }
    code.push(I::IntCmp {
        op: Cmp::Lt,
        dst: Dst(R(5)),
        left: R(0),
        right: R(1),
    });
    code.push(I::Branch {
        condition: R(5),
        then_target: target(62, vec![]),
        else_target: target(132, vec![]),
    });
    code.push(constant(6, "100"));
    code.push(I::IntMul {
        dst: Dst(R(2)),
        left: R(2),
        right: R(6),
    });
    code.push(I::IntAdd {
        dst: Dst(R(2)),
        left: R(2),
        right: R(3),
    });
    code.push(I::Return { value: Some(R(2)) });
    let program = Program {
        module: "regions".into(),
        version: "1.0.0".into(),
        types: vec![Type::Int, Type::Bool],
        entry: Some(FuncRef(0)),
        functions: vec![Function {
            symbol: "main".into(),
            is_async: false,
            native: None,
            intrinsic: None,
            registers: vec![
                TypeRef(0),
                TypeRef(0),
                TypeRef(0),
                TypeRef(0),
                TypeRef(0),
                TypeRef(1),
                TypeRef(0),
            ],
            params: vec![],
            result: Some(TypeRef(0)),
            handlers: vec![],
            code,
        }],
        ..Program::default()
    };
    let cold = Executable::new(&program, Mode::Cold).unwrap();
    let jit = Executable::new(&program, Mode::Jit).unwrap();
    let mut cold = cold.start();
    let mut jit = jit.start();
    assert_eq!(cold.resume(0).unwrap(), jit.resume(0).unwrap());
    for _ in 0..100 {
        let expected = cold.resume(1).unwrap();
        let actual = jit.resume(1).unwrap();
        assert_eq!(actual, expected);
        if let Outcome::Returned(n) = actual {
            assert_eq!(n.to_string(), "9223372036854777508");
            return;
        }
    }
    panic!("region dispatcher did not finish within the expected instruction count");
}
