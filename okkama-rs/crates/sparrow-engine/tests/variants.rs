//! Fixtures are real compiler output, not handwritten synthetic bytecode.
//! Sources and compiler provenance are recorded alongside the assembly and in
//! docs/new-runtime/compiler-variants-smoke.json.
use okkama_sparrow::{Constant, Dst, Instruction, Register, Target, assemble};
use sparrow_engine::{Executable, Mode, Outcome};
#[test]
fn compiled_maybe_presence_fold_and_fallback_execute_in_both_engines() {
    for asm in [
        include_str!("fixtures/maybe-some.asm"),
        include_str!("fixtures/maybe-none.asm"),
        include_str!("fixtures/presence-missing.asm"),
        include_str!("fixtures/presence-fold.asm"),
    ] {
        let p = assemble(asm).unwrap();
        for mode in [Mode::Cold, Mode::Jit] {
            let code = Executable::new(&p, mode).unwrap();
            for budget in [1, 4096] {
                let mut session = code.start();
                loop {
                    if let Outcome::Returned(value) = session.resume(budget).unwrap() {
                        assert_eq!(value, 42);
                        break;
                    }
                }
            }
        }
    }
}
#[test]
fn inactive_variant_payload_traps_without_host_panic() {
    let mut p = assemble(include_str!("fixtures/maybe-some.asm")).unwrap();
    // Deliberately request the Some payload while the tag is None. This is
    // invalid language behavior but valid SPB that must produce its trap.
    p.functions[0].code[0] = Instruction::Const {
        dst: Dst(Register(0)),
        value: Constant::Int("1".into()),
    };
    p.functions[0].code[7] = Instruction::Jump {
        target: Target {
            pc: 8,
            moves: vec![],
        },
    };
    for mode in [Mode::Cold, Mode::Jit] {
        assert_eq!(
            Executable::new(&p, mode).unwrap().run().unwrap_err().0,
            "Cannot extract payload of an inactive enum variant"
        );
    }
}

#[test]
fn terminal_failure_is_memoized_even_with_zero_fuel() {
    let mut p = assemble(include_str!("fixtures/maybe-some.asm")).unwrap();
    p.functions[0].code[0] = Instruction::Trap {
        code: okkama_sparrow::TrapCode::Empty,
        message: "failure".into(),
    };
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&p, mode).unwrap();
        let mut session = code.start();
        let error = session.resume(1).unwrap_err();
        assert_eq!(session.live_registers().count(), 0);
        for fuel in [0, 1, 4096] {
            assert_eq!(session.resume(fuel).unwrap_err(), error);
        }
    }
}
