use sparrow_engine::{Executable, Mode};
#[test]
fn atomic_publication_and_typed_closed_failures_match_the_compiler_contract() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/async-atomic.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
    }
}
