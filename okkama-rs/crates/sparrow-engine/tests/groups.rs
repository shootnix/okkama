use sparrow_engine::{Executable, Mode};
#[test]
fn groups_start_eagerly_seal_cancel_and_wait_for_cleanup() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/async-group.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
    }
}
#[test]
fn group_failures_preserve_registration_order_and_primary_type() {
    let program =
        okkama_sparrow::assemble(include_str!("fixtures/async-group-failures.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        let error = Executable::new(&program, mode)
            .unwrap()
            .run()
            .unwrap_err()
            .to_string();
        assert_eq!(
            error,
            "Problem: first\nSecondary cleanup failure:\n  Problem: second"
        );
    }
}

// Known origin discrepancy: origin discards this cleanup failure. Keep it visible
// in the new engine as required by the stdlib contract; this is not an origin parity test.
#[test]
fn explicitly_cancelled_groups_do_not_hide_real_cleanup_failures() {
    let program =
        okkama_sparrow::assemble(include_str!("fixtures/async-group-cancel-cleanup.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        let error = Executable::new(&program, mode)
            .unwrap()
            .run()
            .unwrap_err()
            .to_string();
        assert!(error.contains("CleanupFailure: cleanup"), "{error}");
    }
}
