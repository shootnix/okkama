//! Исполнение OIR в вызывающем Rust-процессе, без JVM и файлов артефактов.
//!
//! Синхронный main: целые, булевы и строковые значения, сложение, сравнения,
//! переходы с параметрами блоков, прямые вызовы и возврат. Результат — точное целое, а не код завершения процесса.
//! Объекты хранятся в куче запуска; значения передают ссылки по индексу.
//! Неподдерживаемые конструкции возвращают ошибку; CLI использует IR по умолчанию.

mod host;
mod interpreter;
mod operations;
pub mod sparrow;
mod worker;
mod state;
mod tests;
mod value;

pub use interpreter::{execute, execute_with_arguments};
pub use tests::run_tests;
pub use worker::set_worker_command;
pub use okkama_runtime_core::failure::Kind as FailureKind;
pub use okkama_runtime_core::host::default_environment;

use std::fmt;

use okkama_compiler::ir;
use okkama_compiler::source::Span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    Unwind {
        span: Span,
        failure: std::rc::Rc<okkama_runtime_core::failure::Failure>,
    },
    Panic {
        span: Span,
        message: String,
    },
    NativeFailure {
        span: Span,
        failure: okkama_runtime_core::abi::NativeFailure,
    },
    InvalidIr(ir::VerifyError),
    MissingEntry,
    Unsupported {
        span: Span,
        message: String,
    },
    InvalidState {
        span: Span,
        message: String,
    },
    /// Ошибка программы при исполнении (в отличие от ограничения реализации).
    Trap {
        span: Span,
        message: String,
    },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unwind { failure, .. } => failure.fmt(f),
            Self::Panic { message, .. } => write!(f, "Panic: {message}"),
            Self::NativeFailure { failure, .. } => failure.fmt(f),
            Self::InvalidIr(error) => write!(f, "invalid IR: {error}"),
            Self::MissingEntry => f.write_str("IR module has no entry function"),
            Self::Unsupported { message, .. } => write!(f, "IR interpreter: {message}"),
            Self::InvalidState { message, .. } => write!(f, "invalid interpreter state: {message}"),
            Self::Trap { message, .. } => write!(f, "RuntimeError: {message}"),
        }
    }
}

impl std::error::Error for RuntimeError {}

pub(crate) fn unsupported(span: Span, message: impl Into<String>) -> RuntimeError {
    RuntimeError::Unsupported {
        span,
        message: message.into(),
    }
}

pub(crate) fn invalid_state(span: Span, message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidState {
        span,
        message: message.into(),
    }
}

/// `Rt.describe` JVM: текст одного отказа.
pub fn describe(failure: &okkama_runtime_core::failure::Failure) -> String {
    use okkama_runtime_core::failure::Kind;
    match &failure.kind {
        Kind::Native(native) => native_text(native),
        Kind::Thrown { name, message, .. } => {
            format!("{name}: {}", message.as_deref().unwrap_or(""))
        }
        Kind::Panic(message) => format!("Panic: {message}"),
        Kind::Program(message) => format!("RuntimeError: {message}"),
    }
}

fn native_text(native: &okkama_runtime_core::abi::NativeFailure) -> String {
    format!(
        "NativeError: {}\ncode: {}\ndomain: {}\noperation: {}",
        native.message, native.code, native.domain, native.operation
    )
}

/// `Rt.report` JVM: диагностика неперехваченного отказа программы (stderr,
/// код выхода 1) или None для ошибок реализации.
pub fn report(error: &RuntimeError) -> Option<String> {
    let mut text = match error {
        RuntimeError::Unwind { failure, .. } => {
            let mut text = describe(failure);
            for secondary in failure.secondaries() {
                text.push_str("\nSecondary cleanup failure:\n  ");
                text.push_str(&describe(&secondary));
            }
            text
        }
        RuntimeError::Panic { message, .. } => format!("Panic: {message}"),
        RuntimeError::Trap { message, .. } => format!("RuntimeError: {message}"),
        RuntimeError::NativeFailure { failure, .. } => native_text(failure),
        _ => return None,
    };
    text.push('\n');
    Some(text)
}
