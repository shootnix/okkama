use sparrow_engine::{Executable, Mode};
#[test]
fn cyclic_barrier_break_reset_snapshots_and_cancellation() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/async-barrier.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
    }
}
