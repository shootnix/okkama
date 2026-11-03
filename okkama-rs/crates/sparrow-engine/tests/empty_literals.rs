use sparrow_engine::{Executable, Mode};

#[test]
fn compiler_empty_array_bottom_type_can_widen_to_an_int_array() {
    let mut program = okkama_sparrow::assemble(include_str!("fixtures/empty-literal.asm")).unwrap();
    // Retain the empty-array construction, widening move and ordinary method call.
    let main = &mut program.functions[4];
    main.code.drain(0..10);
    main.code.truncate(4);
    main.code.push(okkama_sparrow::Instruction::Return {
        value: Some(okkama_sparrow::Register(1)),
    });
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 0);
    }
}
