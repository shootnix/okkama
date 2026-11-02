//! VM Sparrow over its independent `sparrow-runtime`; no compiler or IR dependency.
//!
//! Кадр — регистры функции и номер инструкции; вызовы — собственный стек
//! кадров, без рекурсии Rust; задачи `async` — сопрограммы планировщика.

mod host;
mod jit;
mod liveness;
mod machine;
mod tests;

pub use sparrow_runtime::IntValue;
pub use machine::{execute, execute_traced, execute_with_arguments};
pub use sparrow_runtime::failure::Site;
pub use tests::run_tests;

use sparrow_runtime::failure::Failure;
use okkama_sparrow::VerifyError;
use std::fmt;
use std::rc::Rc;

/// Команда запуска той же программы для процесса-воркера `spawn`.
pub fn set_worker_command(command: Vec<String>) {
    sparrow_runtime::worker::set_command(command);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidProgram(VerifyError),
    MissingEntry,
    Operation {
        function: String,
        instruction: usize,
        error: OperationError,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProgram(error) => write!(f, "invalid Sparrow program: {error}"),
            Self::MissingEntry => f.write_str("Sparrow program has no entry function"),
            Self::Operation { error, .. } => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperationError {
    Unsupported { message: String },
    /// Неперехваченный отказ (описанный: тексты Error вычислены).
    Unwind(Rc<Failure>),
    Panic { message: String },
    NativeFailure(sparrow_runtime::abi::NativeFailure),
    InvalidState { message: String },
    Trap { message: String },
}

impl fmt::Display for OperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { message } => write!(f, "Sparrow: {message}"),
            Self::Unwind(failure) => failure.fmt(f),
            Self::Panic { message } => write!(f, "Panic: {message}"),
            Self::NativeFailure(failure) => failure.fmt(f),
            Self::InvalidState { message } => write!(f, "invalid interpreter state: {message}"),
            Self::Trap { message } => write!(f, "RuntimeError: {message}"),
        }
    }
}

impl std::error::Error for OperationError {}

/// `Rt.report` JVM: диагностика неперехваченного отказа программы (stderr,
/// код выхода 1) или `None` для ошибок реализации.
pub fn report(error: &Error) -> Option<String> {
    let Error::Operation { error, .. } = error else {
        return None;
    };
    let mut text = match error {
        OperationError::Unwind(failure) => {
            let mut text = sparrow_runtime::failure::describe(failure);
            for secondary in failure.secondaries() {
                text.push_str("\nSecondary cleanup failure:\n  ");
                text.push_str(&sparrow_runtime::failure::describe(&secondary));
            }
            text
        }
        OperationError::Panic { message } => format!("Panic: {message}"),
        OperationError::Trap { message } => format!("RuntimeError: {message}"),
        OperationError::NativeFailure(failure) => {
            sparrow_runtime::failure::describe(&Failure::new(sparrow_runtime::failure::Kind::Native(Rc::new(failure.clone()))))
        }
        _ => return None,
    };
    text.push('\n');
    Some(text)
}

/// Стек вызовов отказа в терминах Sparrow, без исходников: функция, номер и
/// текст инструкции каждого кадра. Пустая строка — стек пуст.
pub fn render_trace(program: &okkama_sparrow::Program, trace: &[Site]) -> String {
    if trace.is_empty() {
        return String::new();
    }
    let frames: Vec<String> = trace
        .iter()
        .map(|site| match program.functions.get(site.function as usize) {
            Some(function) => {
                let name = sparrow_runtime::failure::frame_name(&function.symbol);
                match function.code.get(site.pc as usize) {
                    Some(instruction) => format!("{name} [f{} @{}: {}]", site.function, site.pc, instruction.text()),
                    None => format!("{name} [f{} @{}]", site.function, site.pc),
                }
            }
            None => format!("? [f{} @{}]", site.function, site.pc),
        })
        .collect();
    sparrow_runtime::failure::stack_text("Stack trace (Sparrow, most recent call first):", &frames)
}
