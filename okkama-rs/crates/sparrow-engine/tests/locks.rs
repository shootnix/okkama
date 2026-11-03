use sparrow_engine::{Executable, Mode};
#[test]
fn compiler_lock_is_fifo_and_cancelled_waiter_cannot_take_a_grant() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/async-lock.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
    }
}
