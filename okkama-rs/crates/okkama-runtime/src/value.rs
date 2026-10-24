//! Source-located adapter over compiler-independent runtime values.
use crate::{RuntimeError, invalid_state};
use okkama_compiler::{ir::IntValue, source::Span};
pub(crate) use okkama_runtime_core::Value;

pub(crate) trait RuntimeValue {
    fn int(&self, span: Span) -> Result<&IntValue, RuntimeError>;
    fn boolean(&self, span: Span) -> Result<bool, RuntimeError>;
    fn string(&self, span: Span) -> Result<&str, RuntimeError>;
}
impl RuntimeValue for Value {
    fn int(&self, span: Span) -> Result<&IntValue, RuntimeError> {
        self.as_int().map_err(|e| invalid_state(span, e.message))
    }
    fn boolean(&self, span: Span) -> Result<bool, RuntimeError> {
        self.as_bool().map_err(|e| invalid_state(span, e.message))
    }
    fn string(&self, span: Span) -> Result<&str, RuntimeError> {
        self.as_string().map_err(|e| invalid_state(span, e.message))
    }
}
