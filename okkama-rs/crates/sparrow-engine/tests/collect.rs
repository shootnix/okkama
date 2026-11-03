use sparrow_engine::{Executable, Mode};
#[test]
fn collection_order_cached_identity_winners_and_loser_cleanup() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/async-collect.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
    }
}
#[test]
fn cancelling_a_collector_waits_for_every_child_cleanup() {
    let program =
        okkama_sparrow::assemble(include_str!("fixtures/async-collect-cancel.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
    }
}
#[test]
fn synthetic_sequence_dispatch_rejects_a_mismatched_witness() {
    let mut program = okkama_sparrow::assemble(include_str!("fixtures/async-collect.asm")).unwrap();
    let size = program
        .functions
        .iter_mut()
        .find(|f| f.symbol.contains("ArraySequence<Future<int>") && f.symbol.ends_with(".size"))
        .unwrap();
    size.is_async = true;
    for mode in [Mode::Cold, Mode::Jit] {
        let error = Executable::new(&program, mode).unwrap_err().to_string();
        assert!(error.contains("role witness"), "{error}");
    }
}
