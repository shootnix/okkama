use sparrow_engine::{Executable, Mode};
#[test]
fn scope_caches_the_body_and_waits_for_children_on_success_failure_and_cancel() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/async-scope.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
    }
}
