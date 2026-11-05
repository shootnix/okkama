use sparrow_engine::{AsyncHost, Executable, Mode, Outcome};
#[test]
fn compiler_cold_future_is_lazy_and_repeated_await_caches_value_or_failure() {
    for source in [
        include_str!("fixtures/async-cached.asm"),
        include_str!("fixtures/async-failure.asm"),
    ] {
        let p = okkama_sparrow::assemble(source).unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            let code = Executable::new(&p, mode).unwrap();
            let mut session = code.start();
            loop {
                if let Outcome::Returned(n) = session.resume(1).unwrap() {
                    assert_eq!(n, 42);
                    break;
                }
            }
            let host = AsyncHost::new().unwrap();
            assert_eq!(host.run(&code).unwrap(), 42);
        }
    }
}
#[test]
fn session_keeps_machine_code_alive_after_the_executable_handle_is_dropped() {
    let p = okkama_sparrow::assemble(include_str!("fixtures/async-cached.asm")).unwrap();
    let code = Executable::new(&p, Mode::Jit).unwrap();
    let mut session = code.start();
    drop(code);
    for _ in 0..128 {
        match session.resume(1).unwrap() {
            Outcome::Returned(n) => {
                assert_eq!(n, 42);
                return;
            }
            Outcome::Yielded => {}
            Outcome::Waiting => {
                panic!("cached in-process fixture unexpectedly requires external work")
            }
        }
    }
    panic!("cached fixture exceeded 128 instruction slices");
}

#[test]
fn ten_thousand_nested_awaits_use_flat_continuations_with_gc() {
    let p = okkama_sparrow::assemble(include_str!("fixtures/async-deep.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut session = code.start();
        session.stress_collection();
        let mut peak = 0;
        loop {
            let result = session.resume(1).unwrap();
            peak = peak.max(session.continuation_depth());
            if let Outcome::Returned(n) = result {
                assert_eq!(n, 42);
                break;
            }
        }
        assert_eq!(peak, 10001);
        assert!(session.collection_count() > 50);
    }
}
#[test]
fn repeated_await_executes_a_captured_builder_effect_once() {
    let p = okkama_sparrow::assemble(
        r#"sparrow 9
module "future_cache" "1.0.0"
type t0 int
type t1 builder t0
type t2 array t0
type t3 future t0
function f0 "main" async {
 registers [t1,t0,t3,t0,t0,t2,t0]
 params []
 result t0
 0: builder.new r0
 1: const r1, int 21
 2: call r2, f1, [r0,r1]
 3: await r3, r2
 4: await r4, r2
 5: builder.finish r5, r0
 6: arr.len r6, r5
 7: int.add r3, r3, r4
 8: int.add r3, r3, r6
 9: ret r3
}
function f1 "once" async {
 registers [t1,t0]
 params [r0,r1]
 result t0
 0: builder.push r0, r0, r1
 1: ret r1
}
entry f0
"#,
    )
    .unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(
            AsyncHost::new()
                .unwrap()
                .run(&Executable::new(&p, mode).unwrap())
                .unwrap(),
            43
        );
    }
}

#[test]
fn cancel_is_idempotent_for_cold_bodies_and_native_timers() {
    for source in [
        include_str!("fixtures/async-cancel-cold.asm"),
        include_str!("fixtures/async-cancel-timer.asm"),
    ] {
        let p = okkama_sparrow::assemble(source).unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            let code = Executable::new(&p, mode).unwrap();
            assert_eq!(code.run().unwrap(), 42);
            assert_eq!(AsyncHost::new().unwrap().run(&code).unwrap(), 42);
        }
    }
}

#[test]
fn cancellation_waits_for_actual_suspension_and_runs_cleanup_before_propagating() {
    let ready =
        okkama_sparrow::assemble(include_str!("fixtures/async-self-cancel-ready.asm")).unwrap();
    let pending =
        okkama_sparrow::assemble(include_str!("fixtures/async-self-cancel-cleanup.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&ready, mode).unwrap().run().unwrap(), 42);
        let code = Executable::new(&pending, mode).unwrap();
        let mut session = code.start();
        let host = AsyncHost::new().unwrap();
        let error = host
            .block_on(session.complete_async())
            .unwrap()
            .unwrap_err();
        assert_eq!(
            error.0,
            "Cancelled: Operation was cancelled\nSecondary cleanup failure:\n  CleanupFailure: cleanup"
        );
        let report = session.failure_report().unwrap().unwrap();
        assert_eq!(report.nodes[0].name, "Cancelled");
        assert_eq!(report.nodes[0].secondary.len(), 1);
        assert_eq!(
            report.nodes[report.nodes[0].secondary[0]].name,
            "CleanupFailure"
        );
    }
}

#[test]
fn expired_positive_timer_still_delivers_pending_cancellation_and_cleanup() {
    let source = include_str!("fixtures/async-self-cancel-cleanup.asm").replace("20000000", "1");
    let program = okkama_sparrow::assemble(&source).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        let error = Executable::new(&program, mode).unwrap().run().unwrap_err();
        assert_eq!(
            error.0,
            "Cancelled: Operation was cancelled\nSecondary cleanup failure:\n  CleanupFailure: cleanup"
        );
    }
}
