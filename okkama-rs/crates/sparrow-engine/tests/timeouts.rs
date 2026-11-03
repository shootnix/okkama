use sparrow_engine::{Executable, Mode};

#[test]
fn timeout_and_deadline_preserve_ready_results_clock_errors_and_structured_cancellation() {
    for source in [
        include_str!("fixtures/async-timeout.asm"),
        include_str!("fixtures/async-system-clock.asm"),
    ] {
        let program = okkama_sparrow::assemble(source).unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
        }
    }
}
