use sparrow_engine::{Mode, run_tests};
#[test]
fn fake_storage_gc_aliases_and_isolation_match_compiler_contracts() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/fake-storage.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        let mut output = Vec::new();
        assert!(
            run_tests(&program, mode, &mut output).unwrap(),
            "{}",
            String::from_utf8_lossy(&output)
        );
    }
}
