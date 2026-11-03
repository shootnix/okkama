use sparrow_engine::{Executable, Mode};
#[test]
fn synchronous_process_acquisition_installs_tokio_before_first_instruction() {
    let program =
        okkama_sparrow::assemble(include_str!("fixtures/process-sync-native.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
    }
}
#[test]
fn public_command_capture_and_pipeline_use_exact_arguments_and_os_pipes() {
    let program =
        okkama_sparrow::assemble(include_str!("fixtures/async-process-native.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(Executable::new(&program, mode).unwrap().run().unwrap(), 42);
    }
}
