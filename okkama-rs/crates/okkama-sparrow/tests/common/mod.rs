//! Небольшие программы SPB 9 для тестов модели.
#![allow(dead_code)]
use okkama_sparrow::{
    Constant, Dst, FuncRef, Function, Handler, Instruction as I, Program, Register as R, Target, Type, TypeRef,
};

pub fn r(index: u32) -> R {
    R(index)
}

pub fn d(index: u32) -> Dst {
    Dst(R(index))
}

pub fn int(text: &str) -> Constant {
    Constant::Int(text.into())
}

pub fn to(pc: u32) -> Target {
    Target { pc, moves: Vec::new() }
}

/// Функция над `registers` регистрами `int` (тип t0).
pub fn function(symbol: &str, registers: usize, params: Vec<u32>, result: bool, code: Vec<I>) -> Function {
    Function {
        symbol: symbol.into(),
        is_async: false,
        native: None,
        intrinsic: None,
        registers: vec![TypeRef(0); registers],
        params: params.into_iter().map(R).collect(),
        result: result.then_some(TypeRef(0)),
        code,
        handlers: Vec::<Handler>::new(),
    }
}

pub fn program(functions: Vec<Function>) -> Program {
    Program {
        module: "app".into(),
        version: "1.0.0".into(),
        types: vec![Type::Int, Type::Bool, Type::Str],
        functions,
        entry: Some(FuncRef(0)),
        ..Program::default()
    }
}

/// `main`: factorial(10) рекурсией и цикл с обменом на обратном ребре.
pub fn sample() -> Program {
    let main = function(
        "main",
        3,
        vec![],
        true,
        vec![
            I::Const { dst: d(0), value: int("10") },
            I::Call { dst: Some(d(1)), function: FuncRef(1), args: vec![r(0)] },
            I::Return { value: Some(r(1)) },
        ],
    );
    let factorial = function(
        "factorial",
        5,
        vec![0],
        true,
        vec![
            I::Const { dst: d(1), value: int("1") },
            I::IntCmp { op: okkama_sparrow::Cmp::Le, dst: d(2), left: r(0), right: r(1) },
            I::Branch { condition: r(2), then_target: to(3), else_target: to(4) },
            I::Return { value: Some(r(1)) },
            I::IntSub { dst: d(3), left: r(0), right: r(1) },
            I::Call { dst: Some(d(4)), function: FuncRef(1), args: vec![r(3)] },
            I::IntMul { dst: d(4), left: r(0), right: r(4) },
            I::Return { value: Some(r(4)) },
        ],
    );
    program(vec![main, factorial])
}
