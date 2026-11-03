use sparrow_engine::{Executable, Mode};
#[test]
fn completion_stream_order_duplicates_cancellation_and_fifo_reservations() {
    for source in [
        include_str!("fixtures/async-completions.asm"),
        include_str!("fixtures/async-completions-init.asm"),
        include_str!("fixtures/async-completions-fifo.asm"),
    ] {
        let program = okkama_sparrow::assemble(source).unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
        }
    }
}
