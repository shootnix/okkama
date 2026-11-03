use sparrow_engine::{AsyncHost, Executable, Mode};

#[test]
fn compiler_spawn_caches_typed_results_and_observed_failures_for_multiple_readers() {
    for source in [
        include_str!("fixtures/async-spawn.asm"),
        include_str!("fixtures/async-spawn-observed.asm"),
        include_str!("fixtures/async-spawn-shutdown.asm"),
        include_str!("fixtures/async-spawn-short-timer.asm"),
    ] {
        let program = okkama_sparrow::assemble(source).unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            let code = Executable::new(&program, mode).unwrap();
            assert_eq!(code.run().unwrap(), 42);
            assert_eq!(AsyncHost::new().unwrap().run(&code).unwrap(), 42);
        }
    }
}

#[test]
fn shutdown_reports_an_unobserved_child_failure_using_its_error_text() {
    let program =
        okkama_sparrow::assemble(include_str!("fixtures/async-spawn-unobserved.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&program, mode).unwrap();
        let mut session = code.start();
        let host = AsyncHost::new().unwrap();
        let error = host
            .block_on(session.complete_async())
            .unwrap()
            .unwrap_err();
        assert_eq!(error.0, "Problem: problem");
        let report = session.failure_report().unwrap().unwrap();
        assert_eq!(report.nodes[0].message.as_deref(), Some("problem"));
        assert!(!report.nodes[0].trace.is_empty());
    }
}

#[test]
fn deep_nested_spawn_startup_uses_tokio_tasks_without_native_stack_recursion() {
    let source = include_str!("fixtures/async-spawn-deep.asm").replace("int 20", "int 2000");
    let program = okkama_sparrow::assemble(&source).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
    }
}

#[test]
fn fuel_yields_and_ready_awaits_do_not_end_eager_spawn_startup() {
    let source = include_str!("fixtures/async-spawn-eager.asm").replace("int 200", "int 10000");
    let program = okkama_sparrow::assemble(&source).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
    }
}
