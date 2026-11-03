use sparrow_engine::{Mode, run_tests};
#[test]
fn test_profile_accumulates_assertions_preserves_context_and_continues_with_async_cases() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/test-profile.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        let mut output = Vec::new();
        assert!(!run_tests(&program, mode, &mut output).unwrap());
        let output = String::from_utf8(output).unwrap();
        assert!(
            output.contains("expected true, got false\n    expected 2, got 1\n    third failure"),
            "{output}"
        );
        assert!(
            output.contains("--- PASS: async test continues after another failed test"),
            "{output}"
        );
        assert_eq!(output.matches("--- FAIL:").count(), 1);
    }
}
