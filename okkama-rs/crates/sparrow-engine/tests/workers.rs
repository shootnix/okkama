use sparrow_engine::{Executable, Mode};

#[test]
#[ignore = "launches worker processes; run only under external process-tree limits"]
fn process_workers_transfer_bound_fields_arrays_and_marker_text_and_reap_on_cancel() {
    for (index, source) in [
        include_str!("fixtures/async-worker.asm"),
        include_str!("fixtures/async-worker-framing.asm"),
        include_str!("fixtures/async-worker-cancel.asm"),
    ]
    .into_iter()
    .enumerate()
    {
        let program = okkama_sparrow::assemble(source).unwrap();
        let path = std::env::temp_dir().join(format!(
            "sparrow-workers-{}-{index}.spb",
            std::process::id()
        ));
        std::fs::write(&path, okkama_sparrow::encode(&program).unwrap()).unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            let mut command = vec![env!("CARGO_BIN_EXE_sparrow-next").into()];
            if matches!(mode, Mode::Cold) {
                command.push("--cold".into());
            }
            command.push(path.as_os_str().to_owned());
            let executable = Executable::new(&program, mode)
                .unwrap()
                .with_worker_command(command)
                .unwrap();
            assert_eq!(executable.run().unwrap(), 42);
        }
        std::fs::remove_file(path).unwrap();
    }
}
