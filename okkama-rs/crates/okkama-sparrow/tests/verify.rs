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
    function.registers.push(okkama_sparrow::TypeRef(0));
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
    program.functions[1].native = Some("runtime/stdio@1::stdout_write/1".into());
    assert!(message(&program).contains("native and intrinsic functions have no code"));
    program.functions[1].code.clear();
    assert_eq!(program.verify(), Ok(()));
}
