use sparrow_engine::{Executable, Mode};
#[test]
fn compiler_event_is_level_triggered_and_cancelling_one_waiter_does_not_signal_it() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/async-event.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
    }
}
#[test]
fn malformed_intrinsic_signatures_and_event_layouts_are_rejected_before_execution() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/async-event.asm")).unwrap();
    let mut malformed = program.clone();
    let f = malformed
        .functions
        .iter_mut()
        .find(|f| {
            f.intrinsic
                .as_ref()
                .is_some_and(|i| i.name == "okkama.event.Event.wait")
        })
        .unwrap();
    f.is_async = false;
    let mut layout = program;
    layout
        .objects
        .iter_mut()
        .find(|o| o.name == "async.event::Event")
        .unwrap()
        .fields[0]
        .0 = "unexpected".into();
    for bad in [malformed, layout] {
        for mode in [Mode::Cold, Mode::Jit] {
            assert!(Executable::new(&bad, mode).is_err());
        }
    }
}

#[test]
fn cancellation_capability_is_separate_from_future_cancel_and_check_throws_cancelled() {
    let program =
        okkama_sparrow::assemble(include_str!("fixtures/async-cancellation-token.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
    }
}
