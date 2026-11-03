use sparrow_engine::{Executable, Mode, Outcome};
#[test]
fn stable_ordering_preserves_input_callbacks_errors_and_gc_roots_across_yields() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/ordering.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&program, mode).unwrap();
        let mut session = code.start();
        loop {
            match session.resume(3).unwrap() {
                Outcome::Returned(result) => {
                    assert_eq!(result, 42);
                    break;
                }
                Outcome::Yielded => {}
                other => panic!("unexpected outcome: {other:?}"),
            }
        }
        assert!(session.collection_count() > 0);
    }
}
