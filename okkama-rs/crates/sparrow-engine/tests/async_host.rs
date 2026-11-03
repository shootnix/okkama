use sparrow_engine::{AsyncHost, Executable, Mode};
use std::{cell::Cell, rc::Rc, time::Duration};

#[test]
fn ready_spawn_transitions_yield_without_ending_eager_startup() {
    let source = r#"sparrow 9
module "ready_spawn_budget" "1.0.0"
type t0 int
type t1 bool
type t2 str
type t3 future t1
type t4 future t0
function f0 "main" async {
 registers [t0,t0,t0,t1,t2,t3,t3]
 params []
 result t0
 0: const r0, int 1024
 1: const r1, int 1
 2: const r2, int 0
 3: const r4, str "0"
 4: call r5, f1, [r4]
 5: fut.spawn r6, r5
 6: int.sub r0, r0, r1
 7: int.cmp gt, r3, r0, r2
 8: br r3, @4, @9
 9: const r0, int 42
 10: ret r0
}
function f1 "runtime.time::sleep_nanoseconds" async native "runtime/time@1::sleep_nanoseconds/1" {
 registers [t2]
 params [r0]
 result t1
}
function f2 "eager_entry" async {
 registers [t4,t4,t1,t0]
 params []
 result t0
 0: call r0, f0, []
 1: fut.spawn r1, r0
 2: fut done, r2, r1
 3: br r2, @4, @6
 4: await r3, r1
 5: ret r3
 6: const r3, int 0
 7: ret r3
}
entry f0
"#;
    let program = okkama_sparrow::assemble(source).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&program, mode).unwrap();
        let host = AsyncHost::new().unwrap();
        let finished = Rc::new(Cell::new(false));
        let observed = Rc::new(Cell::new(false));
        host.block_on(async {
            let done = finished.clone();
            let progress = observed.clone();
            let observer = tokio::task::spawn_local(async move {
                progress.set(!done.get());
            });
            assert_eq!(code.run_async().await.unwrap(), 42);
            finished.set(true);
            observer.await.unwrap();
        })
        .unwrap();
        assert!(
            observed.get(),
            "ready spawn transitions starved another local task"
        );

        // Fuel/host yields are not a language suspension: the parent still sees
        // eager completion after a child containing many ready transitions.
        let eager = okkama_sparrow::assemble(&source.replace("entry f0", "entry f2")).unwrap();
        assert_eq!(
            host.run(&Executable::new(&eager, mode).unwrap()).unwrap(),
            42
        );
    }
}

fn loop_program() -> okkama_sparrow::Program {
    okkama_sparrow::assemble(
        r#"sparrow 9
module "cooperative" "1.0.0"
type t0 int
type t1 bool
function f0 "main" {
 registers [t0,t0,t0,t1]
 params []
 result t0
 0: const r0, int 100000
 1: const r1, int 1
 2: const r2, int 0
 3: int.sub r0, r0, r1
 4: int.cmp gt, r3, r0, r2
 5: br r3, @3, @6
 6: const r0, int 42
 7: ret r0
}
entry f0
"#,
    )
    .unwrap()
}
#[test]
fn jit_and_cold_sessions_yield_to_tokio_local_tasks() {
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&loop_program(), mode).unwrap();
        let host = AsyncHost::new().unwrap();
        let finished = Rc::new(Cell::new(false));
        let observed = Rc::new(Cell::new(false));
        host.block_on(async {
            let done = finished.clone();
            let progress = observed.clone();
            let observer = tokio::task::spawn_local(async move {
                progress.set(!done.get());
            });
            assert_eq!(code.run_async().await.unwrap(), 42);
            finished.set(true);
            observer.await.unwrap();
        })
        .unwrap();
        assert!(
            observed.get(),
            "a runnable local task was starved by VM execution"
        );
    }
}
#[test]
fn host_owns_the_timer_driver_and_rejects_nested_blocking_without_panicking() {
    let host = AsyncHost::new().unwrap();
    host.block_on(async {
        tokio::time::sleep(Duration::from_millis(1)).await;
        assert!(
            host.block_on(async { 42 })
                .unwrap_err()
                .0
                .contains("active Tokio runtime")
        );
    })
    .unwrap();
}
#[test]
fn async_completion_preserves_cached_results_and_failures() {
    let host = AsyncHost::new().unwrap();
    let mut program = loop_program();
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&program, mode).unwrap();
        let mut session = code.start();
        host.block_on(async {
            assert_eq!(session.complete_async().await.unwrap(), 42);
            assert_eq!(session.complete_async().await.unwrap(), 42);
        })
        .unwrap();
    }
    program.functions[0].code = vec![okkama_sparrow::Instruction::Trap {
        code: okkama_sparrow::TrapCode::Empty,
        message: "failure".into(),
    }];
    let code = Executable::new(&program, Mode::Jit).unwrap();
    let mut session = code.start();
    host.block_on(async {
        let error = session.complete_async().await.unwrap_err();
        assert_eq!(session.complete_async().await.unwrap_err(), error);
    })
    .unwrap();
}
