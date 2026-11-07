//! Верификатор: ссылки, регистры, переходы, обработчики и инициализация.
mod common;
use common::*;
use okkama_sparrow::{FuncRef, Handler, Instruction as I, ObjectRef, Target};

fn message(program: &okkama_sparrow::Program) -> String {
    program.verify().expect_err("program must be rejected").to_string()
}

#[test]
fn accepts_recursion_branches_and_register_reuse() {
    assert_eq!(sample().verify(), Ok(()));
}

#[test]
fn rejects_out_of_bounds_references() {
    let mut program = sample();
    program.functions[0].code[0] = I::Const { dst: d(9), value: int("1") };
    assert!(message(&program).contains("register r9 is out of bounds"), "{}", message(&program));

    let mut program = sample();
    program.functions[0].code[1] = I::Call { dst: Some(d(1)), function: FuncRef(7), args: vec![r(0)] };
    assert!(message(&program).contains("function f7 is out of bounds"));

    let mut program = sample();
    program.functions[1].code[2] = I::Branch { condition: r(2), then_target: to(3), else_target: to(40) };
    assert!(message(&program).contains("jump target @40 is out of bounds"));

    let mut program = sample();
    program.functions[0].code[0] = I::ObjNew { dst: d(0), object: ObjectRef(0), fields: vec![] };
    assert!(message(&program).contains("object obj0 is out of bounds"));

    let mut program = sample();
    program.entry = Some(FuncRef(5));
    assert!(message(&program).contains("function f5 is out of bounds"));
}

#[test]
fn rejects_reads_of_registers_not_written_on_every_path() {
    let mut program = sample();
    // r3 записан только на ветке else, а читается после слияния.
    program.functions[1].code = vec![
        I::Const { dst: d(1), value: int("1") },
        I::IntCmp { op: okkama_sparrow::Cmp::Le, dst: d(2), left: r(0), right: r(1) },
        I::Branch { condition: r(2), then_target: to(4), else_target: to(3) },
        I::IntSub { dst: d(3), left: r(0), right: r(1) },
        I::Return { value: Some(r(3)) },
    ];
    assert!(message(&program).contains("register r3 may be read before it is written"), "{}", message(&program));
    // Присваивание на ребре делает регистр записанным.
    program.functions[1].code[2] = I::Branch {
        condition: r(2),
        then_target: Target { pc: 4, moves: vec![(r(3), r(0))] },
        else_target: to(3),
    };
    assert_eq!(program.verify(), Ok(()));
}

#[test]
fn rejects_falling_off_the_code_and_duplicate_edge_assignments() {
    let mut program = sample();
    program.functions[0].code.pop();
    assert!(message(&program).contains("falls off the end"));

    let mut program = sample();
    program.functions[1].code[2] = I::Branch {
        condition: r(2),
        then_target: Target { pc: 3, moves: vec![(r(3), r(0)), (r(3), r(1))] },
        else_target: to(4),
    };
    assert!(message(&program).contains("assigned twice"));
}

#[test]
fn handlers_must_be_ordered_and_define_their_failure_register() {
    let mut program = sample();
    let function = &mut program.functions[0];
    function.registers.push(okkama_sparrow::TypeRef(3));
    function.code.push(I::Raise { failure: r(3) });
    function.handlers = vec![Handler { start: 0, end: 3, target: to(3), failure: r(3) }];
    assert_eq!(program.verify(), Ok(()));
    program.functions[0].handlers = vec![
        Handler { start: 1, end: 3, target: to(3), failure: r(3) },
        Handler { start: 0, end: 2, target: to(3), failure: r(3) },
    ];
    assert!(message(&program).contains("sorted and disjoint"));
    program.functions[0].handlers = vec![Handler { start: 2, end: 9, target: to(3), failure: r(3) }];
    assert!(message(&program).contains("out of bounds"));
}

#[test]
fn bodiless_functions_have_no_code() {
    let mut program = sample();
    // Символ без provider: вызов — ошибка исполнения, сигнатуру сверять не с чем.
    program.functions[1].native = Some("app/missing@1::factorial/1".into());
    assert!(message(&program).contains("native and intrinsic functions have no code"));
    program.functions[1].code.clear();
    assert_eq!(program.verify(), Ok(()));
}

#[test]
fn rejects_false_register_types_and_signatures() {
    // Ложный тип регистра: int.cmp пишет bool в регистр str.
    let mut program = sample();
    program.functions[1].registers[2] = okkama_sparrow::TypeRef(2);
    assert!(message(&program).contains("str cannot hold the bool result of `int.cmp`"), "{}", message(&program));

    // Лишний и недостающий аргумент вызова.
    let mut program = sample();
    program.functions[0].code[1] = I::Call { dst: Some(d(1)), function: FuncRef(1), args: vec![r(0), r(0)] };
    assert!(message(&program).contains("passes 2 arguments for 1 parameters"), "{}", message(&program));
    program.functions[0].code[1] = I::Call { dst: Some(d(1)), function: FuncRef(1), args: vec![] };
    assert!(message(&program).contains("passes 0 arguments for 1 parameters"), "{}", message(&program));

    // Аргумент не того типа.
    let mut program = sample();
    program.functions[0].registers[0] = okkama_sparrow::TypeRef(2);
    program.functions[0].code[0] = I::Const { dst: d(0), value: okkama_sparrow::Constant::Str("10".into()) };
    assert!(message(&program).contains("r0 has type str, expected int"), "{}", message(&program));

    // Результат функции не того типа: `ret` bool из функции int.
    let mut program = sample();
    program.functions[1].code[3] = I::Return { value: Some(r(2)) };
    assert!(message(&program).contains("r2 has type bool, expected int"), "{}", message(&program));

    // Результат вызова в регистр другого типа.
    let mut program = sample();
    program.functions[0].registers[1] = okkama_sparrow::TypeRef(1);
    assert!(message(&program).contains("bool cannot hold the int result of `call of @factorial`"), "{}", message(&program));

    // Перемещение на ребре между регистрами разных типов.
    let mut program = sample();
    program.functions[1].code[2] = I::Branch {
        condition: r(2),
        then_target: Target { pc: 3, moves: vec![(r(1), r(2))] },
        else_target: to(4),
    };
    assert!(message(&program).contains("branch passes r2: bool for r1: int"), "{}", message(&program));

    // Условие перехода — не bool.
    let mut program = sample();
    program.functions[1].code[2] = I::Branch { condition: r(1), then_target: to(3), else_target: to(4) };
    assert!(message(&program).contains("r1 has type int, expected bool"), "{}", message(&program));

    // Регистр отказа обработчика — не failure.
    let mut program = sample();
    let function = &mut program.functions[0];
    function.registers.push(okkama_sparrow::TypeRef(0));
    function.code.push(I::Return { value: Some(r(3)) });
    function.handlers = vec![Handler { start: 0, end: 3, target: to(3), failure: r(3) }];
    assert!(message(&program).contains("r3 has type int, expected failure"), "{}", message(&program));
}

#[test]
fn rejects_runtime_tables_with_false_signatures() {
    // Native: сигнатура не совпадает с ABI provider (`(@byte) -> int`).
    let mut program = sample();
    let mut native = function("runtime.write", 1, vec![0], true, Vec::new());
    native.native = Some("runtime/stdio@1::stdout_write/1".into());
    program.functions.push(native);
    assert!(message(&program).contains("does not match the signature of native"), "{}", message(&program));

    // Точка входа с параметром и точка входа не int.
    let mut program = sample();
    program.entry = Some(FuncRef(1));
    assert!(message(&program).contains("entry @factorial takes parameters"), "{}", message(&program));
    let mut program = sample();
    let mut text = function("main", 1, vec![], true, vec![
        I::Const { dst: d(0), value: okkama_sparrow::Constant::Str("0".into()) },
        I::Return { value: Some(r(0)) },
    ]);
    text.registers[0] = okkama_sparrow::TypeRef(2);
    text.result = Some(okkama_sparrow::TypeRef(2));
    program.functions[0] = text;
    assert!(message(&program).contains("entry @main must return int"), "{}", message(&program));

    // destroy объекта принимает не его.
    let mut program = sample();
    program.types.push(okkama_sparrow::Type::Object(ObjectRef(0)));
    program.objects = vec![okkama_sparrow::ObjectDecl {
        name: "app::Thing".into(),
        fields: vec![],
        does: vec![],
        destroy: Some(FuncRef(1)),
        builtin: false,
    }];
    assert!(message(&program).contains("is not a destroy of app::Thing"), "{}", message(&program));
}
