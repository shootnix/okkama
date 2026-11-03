use sparrow_engine::{AsyncHost, Executable, FailureCategory, Mode, Outcome};
#[test]
fn compiler_timer_and_native_error_contracts_match_in_cold_and_jit() {
    for source in [
        include_str!("fixtures/async-timer.asm"),
        include_str!("fixtures/async-native-failure.asm"),
        include_str!("fixtures/async-native-error-role.asm"),
        include_str!("fixtures/async-native-rethrow.asm"),
    ] {
        let p = okkama_sparrow::assemble(source).unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            let code = Executable::new(&p, mode).unwrap();
            assert_eq!(code.run().unwrap(), 42); // preserves the session when entering Tokio
            assert_eq!(AsyncHost::new().unwrap().run(&code).unwrap(), 42);
        }
    }
}
#[test]
fn uncaught_native_failure_retains_metadata_without_a_builtin_object_declaration() {
    let mut p = okkama_sparrow::assemble(include_str!("fixtures/async-timer.asm")).unwrap();
    p.functions[0].code[0] = okkama_sparrow::Instruction::Const {
        dst: okkama_sparrow::Dst(okkama_sparrow::Register(0)),
        value: okkama_sparrow::Constant::Str("01".into()),
    };
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut session = code.start();
        let error = loop {
            match session.resume(1) {
                Err(e) => break e,
                Ok(Outcome::Waiting) => panic!("invalid duration must already be failed"),
                _ => {}
            }
        };
        assert_eq!(
            error.0,
            "NativeFailure: Wait duration must be a canonical non-negative integer"
        );
        let report = session.failure_report().unwrap().unwrap();
        assert_eq!(report.nodes[0].category, FailureCategory::Native);
        let native = report.nodes[0].native.as_ref().unwrap();
        assert_eq!(native.domain, "core.time");
        assert_eq!(native.code, "invalid_input");
        assert_eq!(native.operation, "runtime/time@1::sleep_nanoseconds/1");
    }
}
#[test]
fn native_provider_signature_is_validated_before_machine_code_can_call_it() {
    let mut p = okkama_sparrow::assemble(include_str!("fixtures/async-timer.asm")).unwrap();
    let f = p.functions.iter_mut().find(|f| f.native.is_some()).unwrap();
    f.is_async = false;
    for mode in [Mode::Cold, Mode::Jit] {
        assert!(
            Executable::new(&p, mode)
                .unwrap_err()
                .0
                .contains("signature")
        );
    }
}
