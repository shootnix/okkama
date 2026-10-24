//! VM Sparrow: исполняет проверенную программу SPB 9 в этом процессе над
//! общим рантаймом `okkama-runtime-core` — значения, операции, планировщик
//! задач, регионы, natives и intrinsics те же, что у исполнителя IR (как
//! JVM-байткод над `okkama.rt`). Компилятор и исходники не нужны.
//!
//! Кадр — регистры функции и номер инструкции; вызовы — собственный стек
//! кадров, без рекурсии Rust; задачи `async` — сопрограммы планировщика.

mod host;
mod jit;
mod liveness;
mod machine;
mod tests;

pub use machine::{execute, execute_with_arguments};
pub use tests::run_tests;

use okkama_runtime_core::failure::Failure;
use okkama_sparrow::VerifyError;
use std::fmt;
use std::rc::Rc;

/// Команда запуска той же программы для процесса-воркера `spawn`.
pub fn set_worker_command(command: Vec<String>) {
    okkama_runtime_core::worker::set_command(command);
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
    NativeFailure(okkama_runtime_core::abi::NativeFailure),
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
            let mut text = okkama_runtime_core::failure::describe(failure);
            for secondary in failure.secondaries() {
                text.push_str("\nSecondary cleanup failure:\n  ");
                text.push_str(&okkama_runtime_core::failure::describe(&secondary));
            }
            text
        }
        OperationError::Panic { message } => format!("Panic: {message}"),
        OperationError::Trap { message } => format!("RuntimeError: {message}"),
        OperationError::NativeFailure(failure) => {
            okkama_runtime_core::failure::describe(&Failure::new(okkama_runtime_core::failure::Kind::Native(Rc::new(failure.clone()))))
        }
        _ => return None,
    };
    text.push('\n');
    Some(text)
}
