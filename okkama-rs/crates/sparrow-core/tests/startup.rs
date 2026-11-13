//! #480: byte-input execution verifies before constructing the JIT; no processes.
use sparrow_core::{Execution, budget::Limits};
use std::io::Write;
use std::sync::{Arc, Mutex};

fn main_program(body: &str) -> okkama_sparrow::Program {
    okkama_sparrow::assemble(&format!(
        "sparrow 15\nmodule \"startup\" \"v1.0.0\"\ntype t0 int\n\
         function f0 \"main\" {{\n registers [t0]\n params []\n result t0\n{body}\n}}\nentry f0\n"
    )).unwrap()
}

fn executable(bytes: Vec<u8>, trace: bool) -> Result<Execution, sparrow_core::ExecutionError> {
    sparrow_core::run_executable(bytes, Vec::new(), Vec::new(), Limits::default(),
        sparrow_core::ExecutionOptions { trace, ..Default::default() })
}

#[test]
fn invalid_spb_preserves_decode_and_verification_errors() {
    let mut invalid_entry = okkama_sparrow::encode(&main_program("0: const r0, int 7\n1: ret r0")).unwrap();
    let end = invalid_entry.len();
    assert_eq!(&invalid_entry[end - 3..], &[1, 0, 0]);
    invalid_entry[end - 2] = 5;
    for (bytes, pinned) in [
        (b"not SPB".to_vec(), "SPB byte 0: not a Sparrow bytecode file"),
        (invalid_entry, "SPB: invalid program: Sparrow program: function f5 is out of bounds"),
    ] {
        let expected = okkama_sparrow::decode(&bytes).unwrap_err().to_string();
        assert_eq!(expected, pinned);
        let sparrow_core::ExecutionError::Load(error) = executable(bytes.clone(), true).unwrap_err() else {
            panic!("invalid bytes must fail during loading");
        };
        assert_eq!(error.0, expected);
        assert_eq!(sparrow_core::run_described(bytes.clone(), Vec::new(), false, Vec::new(), Limits::default())
            .unwrap_err().0, expected);
        assert_eq!(sparrow_core::run_tests(bytes.clone(), Box::new(std::io::sink())).unwrap_err().0, expected);
        // Invalid bytes are refused before even parsing a worker request.
        assert_eq!(sparrow_core::run_worker(bytes, Vec::new(), Vec::new()).unwrap_err().0, expected);
    }
}

#[test]
fn execution_preserves_results_failure_text_and_trace_switch() {
    let bytes = okkama_sparrow::encode(&main_program("0: const r0, int 7\n1: ret r0")).unwrap();
    assert!(matches!(executable(bytes, true).unwrap(), Execution::Main(Ok(7))));
    let program = main_program("0: trap confirm, \"confirm failed\"");
    let bytes = okkama_sparrow::encode(&program).unwrap();
    let (old_error, old_uncaught) = sparrow_core::run_described(
        bytes.clone(), Vec::new(), false, Vec::new(), Limits::default(),
    ).unwrap().unwrap_err();
    let old_trace = sparrow_core::render_trace(&program, &old_uncaught.unwrap().trace);
    assert_eq!(old_trace, "Stack trace (Sparrow, most recent call first):\n  at main [f0 @0: trap confirm, \"confirm failed\"]\n");
    for trace in [false, true] {
        let Execution::Main(Err((error, rendered))) = executable(bytes.clone(), trace).unwrap() else {
            panic!("expected main failure");
        };
        assert_eq!(error, old_error);
        assert_eq!(error.0, "RuntimeError: confirm failed");
        assert_eq!(rendered, if trace { old_trace.as_str() } else { "" });
    }
}

#[derive(Clone)]
struct Output(Arc<Mutex<Vec<u8>>>);
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

#[test]
fn test_profile_dispatch_preserves_report_and_standalone_main() {
    let mut program = main_program("0: const r0, int 7\n1: ret r0");
    let mut test = program.functions[0].clone();
    test.symbol = "test".into();
    test.registers.clear();
    test.result = None;
    test.code = vec![okkama_sparrow::Instruction::Return { value: None }];
    program.functions.push(test);
    program.tests = Some(vec![("small".into(), okkama_sparrow::FuncRef(1))]);
    let bytes = okkama_sparrow::encode(&program).unwrap();
    assert!(matches!(executable(bytes.clone(), true).unwrap(), Execution::Main(Ok(7))));
    let old = Output(Arc::default());
    let new = Output(Arc::default());
    assert!(sparrow_core::run_tests(bytes.clone(), Box::new(old.clone())).unwrap());
    assert!(matches!(sparrow_core::run_executable(
        bytes, Vec::new(), Vec::new(), Limits::default(), sparrow_core::ExecutionOptions {
            tests: Some(Box::new(new.clone())), trace: true, ..Default::default()
        },
    ).unwrap(), Execution::Tests(true)));
    assert_eq!(*old.0.lock().unwrap(), *new.0.lock().unwrap());
    assert_eq!(&*new.0.lock().unwrap(), b"=== RUN   small\n--- PASS: small\nPASS\n");
}

#[test]
fn launcher_setup_runs_only_after_spb_verification() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let valid = okkama_sparrow::encode(&main_program("0: const r0, int 7\n1: ret r0")).unwrap();
    let mut invalid = valid.clone();
    let entry = invalid.len() - 2;
    invalid[entry] = 5;
    for (bytes, expected) in [(invalid, false), (valid, true)] {
        let prepared = Arc::new(AtomicBool::new(false));
        let flag = prepared.clone();
        let result = sparrow_core::run_executable(
            bytes, Vec::new(), Vec::new(), Limits::default(), sparrow_core::ExecutionOptions {
                on_verified: Some(Box::new(move || { flag.store(true, Ordering::Relaxed); })),
                ..Default::default()
            },
        );
        assert_eq!(result.is_ok(), expected);
        assert_eq!(prepared.load(Ordering::Relaxed), expected);
    }
}
