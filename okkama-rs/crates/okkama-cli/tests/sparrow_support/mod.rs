#![allow(dead_code)]

use okkama_compiler::{codegen, ir};

/// How a run on Sparrow ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ending {
    /// `main`'s result.
    Value(i64),
    /// An uncaught failure: its category (`runtime`, `panic`,
    /// `uncaught-error`) and message.
    Failure { category: String, message: String },
    /// The program did not load, or `main`'s result is not an `i64`.
    Error(String),
}

/// `module` compiled to SPB, encoded, and run on Sparrow.
pub fn sparrow(module: &ir::Module) -> Ending {
    let program = codegen::sparrow::generate(module).expect("Sparrow bytecode");
    run(okkama_sparrow::encode(&program).expect("SPB"))
}

/// An encoded SPB program run on Sparrow.
pub fn run(bytes: Vec<u8>) -> Ending {
    match sparrow_core::run_described(bytes, Vec::new(), false, Vec::new(), sparrow_core::budget::Limits::default()) {
        Err(error) | Ok(Err((error, None))) => Ending::Error(error.0),
        Ok(Ok(value)) => Ending::Value(value),
        Ok(Err((_, Some(uncaught)))) => Ending::Failure {
            category: uncaught.category.to_string(),
            message: uncaught.message,
        },
    }
}

/// `main`'s result on Sparrow as text, for assertions.
pub fn value(module: &ir::Module) -> String {
    match sparrow(module) {
        Ending::Value(value) => value.to_string(),
        other => panic!("Sparrow: {other:?}"),
    }
}

/// `main` returns `expected` on Sparrow; a result beyond `i64` is not an
/// exit status there and must be refused.
pub fn assert_value(module: &ir::Module, expected: &str) {
    check_value(sparrow(module), expected);
}

/// A ready SPB program, encoded and run on Sparrow; a program the encoder
/// refuses ends as an error, like one the loader refuses.
pub fn run_program(program: &okkama_sparrow::Program) -> Ending {
    match okkama_sparrow::encode(program) {
        Ok(bytes) => run(bytes),
        Err(error) => Ending::Error(error.to_string()),
    }
}

/// `assert_value` for a ready SPB program.
pub fn assert_program_value(program: &okkama_sparrow::Program, expected: &str) {
    check_value(run_program(program), expected);
}

fn check_value(actual: Ending, expected: &str) {
    match expected.parse::<i64>() {
        Ok(value) => assert_eq!(actual, Ending::Value(value), "expected {expected}"),
        Err(_) => assert!(
            matches!(&actual, Ending::Error(message) if message.contains("entry result outside")),
            "expected {expected}, Sparrow {actual:?}"
        ),
    }
}

/// A trap (a runtime error) with `message` on Sparrow.
pub fn assert_trap(module: &ir::Module, message: &str) {
    assert_eq!(sparrow(module), Ending::Failure { category: "runtime".into(), message: message.into() });
}

/// A panic with `message` on Sparrow.
pub fn assert_panic(module: &ir::Module, message: &str) {
    assert_eq!(sparrow(module), Ending::Failure { category: "panic".into(), message: message.into() });
}

/// The report Sparrow prints for `module`'s uncaught failure, without the
/// place and the call stack that follow it.
pub fn report(module: &ir::Module) -> String {
    let program = codegen::sparrow::generate(module).expect("Sparrow bytecode");
    let bytes = okkama_sparrow::encode(&program).expect("SPB");
    match sparrow_core::run_described(bytes, Vec::new(), false, Vec::new(), sparrow_core::budget::Limits::default()) {
        Ok(Err((error, _))) => {
            let text = error.0;
            let end = [text.find("\n --> "), text.find("\nStack trace")].into_iter().flatten().min().unwrap_or(text.len());
            text[..end].trim_end().to_string()
        }
        other => panic!("expected an uncaught failure, got {other:?}"),
    }
}

/// The uncaught failure of `module` on Sparrow, with its call stack.
pub fn uncaught(module: &ir::Module) -> (okkama_sparrow::Program, sparrow_core::Uncaught) {
    let program = codegen::sparrow::generate(module).expect("Sparrow bytecode");
    let bytes = okkama_sparrow::encode(&program).expect("SPB");
    match sparrow_core::run_described(bytes, Vec::new(), false, Vec::new(), sparrow_core::budget::Limits::default()) {
        Ok(Err((_, Some(uncaught)))) => (program, uncaught),
        other => panic!("expected an uncaught failure, got {other:?}"),
    }
}
