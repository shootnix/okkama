//! VM без компилятора: программы SPB 9, собранные вручную.
use okkama_sparrow::{
    Cmp, Constant, Dst, FuncRef, Function, Handler, Instruction as I, Program, Register as R, Target, TrapCode, Type,
    TypeRef,
};
use sparrow_vm::{Error, OperationError, execute};

fn d(index: u32) -> Dst {
    Dst(R(index))
}

fn int(text: &str) -> Constant {
    Constant::Int(text.into())
}

fn to(pc: u32) -> Target {
    Target { pc, moves: Vec::new() }
}

fn function(symbol: &str, registers: usize, params: Vec<u32>, code: Vec<I>) -> Function {
    Function {
        symbol: symbol.into(),
        is_async: false,
        native: None,
        intrinsic: None,
        registers: vec![TypeRef(0); registers],
        params: params.into_iter().map(R).collect(),
        result: Some(TypeRef(0)),
        code,
        handlers: Vec::new(),
    }
}

fn program(functions: Vec<Function>) -> Program {
    Program {
        module: "app".into(),
        version: "1.0.0".into(),
        types: vec![Type::Int, Type::Failure],
        functions,
        entry: Some(FuncRef(0)),
        ..Program::default()
    }
}

/// `sum(n) = n == 0 ? 0 : n + sum(n - 1)` — глубокая рекурсия без рекурсии Rust.
fn recursion(depth: &str) -> Program {
    program(vec![
        function(
            "main",
            2,
            vec![],
            vec![
                I::Const { dst: d(0), value: int(depth) },
                I::Call { dst: Some(d(1)), function: FuncRef(1), args: vec![R(0)] },
                I::Return { value: Some(R(1)) },
            ],
        ),
        function(
            "sum",
            5,
            vec![0],
            vec![
                I::Const { dst: d(1), value: int("0") },
                I::IntCmp { op: Cmp::Eq, dst: d(2), left: R(0), right: R(1) },
                I::Branch { condition: R(2), then_target: to(3), else_target: to(4) },
                I::Return { value: Some(R(1)) },
                I::Const { dst: d(3), value: int("1") },
                I::IntSub { dst: d(3), left: R(0), right: R(3) },
                I::Call { dst: Some(d(4)), function: FuncRef(1), args: vec![R(3)] },
                I::IntAdd { dst: d(4), left: R(0), right: R(4) },
                I::Return { value: Some(R(4)) },
            ],
        ),
    ])
}

#[test]
fn recursion_and_serialization_work_without_the_compiler() {
    let program = recursion("200000");
    let bytes = okkama_sparrow::encode(&program).unwrap();
    let loaded = okkama_sparrow::decode(&bytes).unwrap();
    assert_eq!(execute(&loaded).unwrap().to_string(), "20000100000");
    let text = okkama_sparrow::disassemble(&loaded);
    assert_eq!(execute(&okkama_sparrow::assemble(&text).unwrap()).unwrap().to_string(), "20000100000");
}

#[test]
fn exact_values_traps_and_handlers_are_independent_of_source_locations() {
    let mut exact = program(vec![function(
        "main",
        1,
        vec![],
        vec![
            I::Const { dst: d(0), value: int("99999999999999999999999999999999999999999") },
            I::IntAdd { dst: d(0), left: R(0), right: R(0) },
            I::Return { value: Some(R(0)) },
        ],
    )]);
    // R01: точное сложение, без переполнения signed-64.
    assert_eq!(execute(&exact).unwrap().to_string(), "199999999999999999999999999999999999999998");

    exact.functions[0].code = vec![I::Trap { code: TrapCode::Confirm, message: "test trap".into() }];
    let error = execute(&exact).unwrap_err();
    assert_eq!(error.to_string(), "RuntimeError: test trap");
    assert_eq!(sparrow_vm::report(&error).unwrap(), "RuntimeError: test trap\n");
    assert!(matches!(error, Error::Operation { instruction: 0, ref function, .. } if function == "main"));

    // Отказ деления перехватывает обработчик диапазона и продолжает программу.
    let mut caught = program(vec![function(
        "main",
        3,
        vec![],
        vec![
            I::Const { dst: d(0), value: int("7") },
            I::Const { dst: d(1), value: int("0") },
            I::IntMod { dst: d(2), left: R(0), right: R(1) },
            I::Return { value: Some(R(2)) },
            I::Const { dst: d(2), value: int("42") },
            I::Return { value: Some(R(2)) },
        ],
    )]);
    caught.functions[0].registers[1] = TypeRef(0);
    caught.functions[0].handlers = vec![Handler { start: 2, end: 4, target: to(4), failure: R(1) }];
    assert_eq!(execute(&caught).unwrap().to_string(), "42");
    caught.functions[0].handlers.clear();
    assert!(matches!(
        execute(&caught),
        Err(Error::Operation { instruction: 2, error: OperationError::Trap { ref message }, .. }) if message == "Modulo by zero"
    ));

    exact.functions[0].code = vec![I::Return { value: Some(R(0)) }];
    assert!(matches!(execute(&exact), Err(Error::InvalidProgram(_))));
}

/// `main`: `acc = Σ f(g(i))` для `i` в `0..3000`; после 1000 вызовов и
/// обратных переходов функции исполняет машинный код (вход в цикл — OSR).
fn hot_loop(callee: Function, argument: Vec<I>) -> Program {
    let mut code = vec![
        I::Const { dst: d(0), value: int("0") },
        I::Const { dst: d(1), value: int("0") },
        I::Const { dst: d(2), value: int("3000") },
        I::Jump { target: to(4) },
        I::IntCmp { op: Cmp::Lt, dst: d(3), left: R(0), right: R(2) },
        I::Branch { condition: R(3), then_target: to(6), else_target: to(7 + argument.len() as u32 + 4) },
    ];
    code.extend(argument);
    code.extend([
        I::Call { dst: Some(d(4)), function: FuncRef(1), args: vec![R(7)] },
        I::IntAdd { dst: d(6), left: R(1), right: R(4) },
        I::Const { dst: d(5), value: int("1") },
        I::IntAdd { dst: d(0), left: R(0), right: R(5) },
        I::Jump { target: Target { pc: 4, moves: vec![(R(1), R(6))] } },
        I::Return { value: Some(R(1)) },
    ]);
    let mut main = function("main", 9, vec![], code);
    main.registers[3] = TypeRef(2);
    let mut program = program(vec![main, callee]);
    program.types.push(Type::Bool);
    program
}

#[test]
fn hot_code_falls_back_to_exact_integers_on_overflow() {
    // f(x) = x * 2^62: переполнение signed-64 — точный результат (R01).
    let callee = function(
        "f",
        3,
        vec![0],
        vec![
            I::Const { dst: d(1), value: int("4611686018427387904") },
            I::IntMul { dst: d(2), left: R(0), right: R(1) },
            I::Return { value: Some(R(2)) },
        ],
    );
    let program = hot_loop(callee, vec![I::Move { dst: d(7), source: R(0) }]);
    assert_eq!(execute(&program).unwrap().to_string(), "20745669553895604486144000");
}

#[test]
fn hot_code_handlers_catch_failures() {
    // f(x) = 7 % x, деление на 0 перехватывает обработчик: 42.
    let mut callee = function(
        "f",
        4,
        vec![0],
        vec![
            I::Const { dst: d(1), value: int("7") },
            I::Jump { target: to(2) },
            I::IntMod { dst: d(2), left: R(1), right: R(0) },
            I::Return { value: Some(R(2)) },
            I::Const { dst: d(2), value: int("42") },
            I::Return { value: Some(R(2)) },
        ],
    );
    callee.registers[3] = TypeRef(1);
    callee.handlers = vec![Handler { start: 2, end: 4, target: to(4), failure: R(3) }];
    let program = hot_loop(
        callee,
        vec![I::Const { dst: d(8), value: int("3") }, I::IntMod { dst: d(7), left: R(0), right: R(8) }],
    );
    assert_eq!(execute(&program).unwrap().to_string(), "43000");
}
