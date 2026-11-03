use sparrow_engine::{Executable, Mode};
#[test]
fn semaphore_fifo_cancellation_exact_counts_and_full_i64_capacity() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/async-semaphore.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
    }
}
