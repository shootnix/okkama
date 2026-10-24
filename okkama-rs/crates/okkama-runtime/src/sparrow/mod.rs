//! Адаптер VM Sparrow к ошибкам исполнителя IR: те же `RuntimeError`, что и у
//! IR, — CLI и отчёты одинаковы для обоих движков. Сама VM не зависит от компилятора.
use crate::RuntimeError;
use okkama_compiler::{ir, source::Span};
use okkama_sparrow::{Program, VerifyError};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidProgram(VerifyError),
    Operation { instruction: usize, error: RuntimeError },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProgram(error) => write!(f, "invalid Sparrow program: {error}"),
            Self::Operation { error, .. } => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

const SPAN: Span = Span { start: 0, end: 0 };

fn convert(error: sparrow_vm::Error) -> Error {
    match error {
        sparrow_vm::Error::InvalidProgram(error) => Error::InvalidProgram(error),
        sparrow_vm::Error::MissingEntry => Error::Operation {
            instruction: 0,
            error: RuntimeError::MissingEntry,
        },
        sparrow_vm::Error::Operation { instruction, error, .. } => Error::Operation {
            instruction,
            error: match error {
                sparrow_vm::OperationError::Unsupported { message } => RuntimeError::Unsupported { span: SPAN, message },
                sparrow_vm::OperationError::Unwind(failure) => RuntimeError::Unwind { span: SPAN, failure },
                sparrow_vm::OperationError::Panic { message } => RuntimeError::Panic { span: SPAN, message },
                sparrow_vm::OperationError::NativeFailure(failure) => RuntimeError::NativeFailure { span: SPAN, failure },
                sparrow_vm::OperationError::Trap { message } => RuntimeError::Trap { span: SPAN, message },
                sparrow_vm::OperationError::InvalidState { message } => RuntimeError::InvalidState { span: SPAN, message },
            },
        },
    }
}

pub fn execute(program: &Program) -> Result<ir::IntValue, Error> {
    sparrow_vm::execute(program).map_err(convert)
}

pub fn execute_with_arguments(program: &Program, arguments: Vec<String>) -> Result<ir::IntValue, Error> {
    sparrow_vm::execute_with_arguments(program, arguments).map_err(convert)
}

pub fn run_tests(program: &Program, out: &mut dyn std::io::Write) -> Result<bool, Error> {
    sparrow_vm::run_tests(program, out).map_err(convert)
}

/// Ошибка запуска как ошибка исполнителя IR (для отчёта CLI).
pub fn runtime_error(error: Error) -> RuntimeError {
    match error {
        Error::InvalidProgram(error) => RuntimeError::InvalidState {
            span: SPAN,
            message: format!("invalid Sparrow program: {error}"),
        },
        Error::Operation { error, .. } => error,
    }
}
