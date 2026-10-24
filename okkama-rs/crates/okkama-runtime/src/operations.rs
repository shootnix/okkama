//! Операции над значениями исполнителя IR: перевод перечислений IR в общие
//! операции `okkama_runtime_core::ops` (их же исполняет VM Sparrow) и места
//! ошибок в `RuntimeError`.
use crate::{RuntimeError, invalid_state, unsupported, value::Value};
use okkama_compiler::{
    ir::{self, Constant},
    source::Span,
};
use okkama_runtime_core::ops::{self, Cmp, Fault};
use std::rc::Rc;

/// Место для отказа общей операции.
pub(crate) fn fault(span: Span) -> impl Fn(Fault) -> RuntimeError {
    move |fault| match fault {
        Fault::Trap(message) => RuntimeError::Trap { span, message },
        Fault::Invalid(message) => invalid_state(span, message),
        Fault::Unsupported(message) => unsupported(span, message),
    }
}

pub(crate) fn cmp(op: ir::CompareOp) -> Cmp {
    match op {
        ir::CompareOp::Eq => Cmp::Eq,
        ir::CompareOp::Ne => Cmp::Ne,
        ir::CompareOp::Lt => Cmp::Lt,
        ir::CompareOp::Le => Cmp::Le,
        ir::CompareOp::Gt => Cmp::Gt,
        ir::CompareOp::Ge => Cmp::Ge,
    }
}

pub(crate) fn int_op(op: ir::IntBinaryOp) -> ops::IntOp {
    match op {
        ir::IntBinaryOp::Add => ops::IntOp::Add,
        ir::IntBinaryOp::Sub => ops::IntOp::Sub,
        ir::IntBinaryOp::Mul => ops::IntOp::Mul,
        ir::IntBinaryOp::Mod => ops::IntOp::Mod,
        ir::IntBinaryOp::And => ops::IntOp::And,
        ir::IntBinaryOp::Or => ops::IntOp::Or,
        ir::IntBinaryOp::Xor => ops::IntOp::Xor,
        ir::IntBinaryOp::Shl => ops::IntOp::Shl,
        ir::IntBinaryOp::Shr => ops::IntOp::Shr,
        ir::IntBinaryOp::Ushr => ops::IntOp::Ushr,
    }
}

fn float_op(op: ir::FloatOp) -> ops::FloatOp {
    match op {
        ir::FloatOp::Add => ops::FloatOp::Add,
        ir::FloatOp::Sub => ops::FloatOp::Sub,
        ir::FloatOp::Mul => ops::FloatOp::Mul,
        ir::FloatOp::Div => ops::FloatOp::Div,
        ir::FloatOp::Mod => ops::FloatOp::Mod,
        ir::FloatOp::Pow => ops::FloatOp::Pow,
        ir::FloatOp::Neg => ops::FloatOp::Neg,
        ir::FloatOp::Abs => ops::FloatOp::Abs,
        ir::FloatOp::Sqrt => ops::FloatOp::Sqrt,
    }
}

fn rat_op(op: ir::RatOp) -> ops::RatOp {
    match op {
        ir::RatOp::New => ops::RatOp::New,
        ir::RatOp::Add => ops::RatOp::Add,
        ir::RatOp::Sub => ops::RatOp::Sub,
        ir::RatOp::Mul => ops::RatOp::Mul,
        ir::RatOp::Div => ops::RatOp::Div,
        ir::RatOp::Mod => ops::RatOp::Mod,
        ir::RatOp::Neg => ops::RatOp::Neg,
        ir::RatOp::Abs => ops::RatOp::Abs,
    }
}

fn str_op(op: ir::StrOp) -> ops::StrOp {
    match op {
        ir::StrOp::Len => ops::StrOp::Len,
        ir::StrOp::Slice => ops::StrOp::Slice,
        ir::StrOp::Split => ops::StrOp::Split,
        ir::StrOp::Replace => ops::StrOp::Replace,
        ir::StrOp::ReplaceFirst => ops::StrOp::ReplaceFirst,
        ir::StrOp::Trim => ops::StrOp::Trim,
        ir::StrOp::Chomp => ops::StrOp::Chomp,
        ir::StrOp::Utf8 => ops::StrOp::Utf8,
    }
}

pub(crate) fn scalar(target: &ir::Type) -> ops::Scalar {
    match target {
        ir::Type::Int => ops::Scalar::Int,
        ir::Type::Float => ops::Scalar::Float,
        ir::Type::Rat => ops::Scalar::Rat,
        ir::Type::Str => ops::Scalar::Str,
        _ => ops::Scalar::Other,
    }
}

pub(crate) fn constant(value: &Constant, span: Span) -> Result<Value, RuntimeError> {
    match value {
        Constant::Int(value) => Ok(Value::Int(value.clone())),
        Constant::Bool(value) => Ok(Value::Bool(*value)),
        Constant::Byte(value) => Ok(Value::Byte(*value)),
        Constant::Char(value) => Ok(Value::Char(*value)),
        Constant::Str(value) => Ok(Value::Str(Rc::from(value.as_str()))),
        Constant::Float(bits) => ops::float_constant(*bits).map_err(fault(span)),
    }
}

pub(crate) fn float(op: ir::FloatOp, args: &[&Value], span: Span) -> Result<Value, RuntimeError> {
    ops::float_op(float_op(op), args).map_err(fault(span))
}

pub(crate) fn float_compare(op: ir::CompareOp, left: &Value, right: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::float_compare(cmp(op), left, right).map_err(fault(span))
}

pub(crate) fn rat(op: ir::RatOp, args: &[&Value], span: Span) -> Result<Value, RuntimeError> {
    ops::rat_op(rat_op(op), args).map_err(fault(span))
}

pub(crate) fn rat_compare(op: ir::CompareOp, left: &Value, right: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::rat_compare(cmp(op), left, right).map_err(fault(span))
}

pub(crate) fn rat_part(value: &Value, denominator: bool, span: Span) -> Result<Value, RuntimeError> {
    ops::rat_part(value, denominator).map_err(fault(span))
}

pub(crate) fn sign(value: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::sign(value).map_err(fault(span))
}

pub(crate) fn int_abs(value: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::int_abs(value).map_err(fault(span))
}

pub(crate) fn bits(op: ir::BitOp, byte: bool, value: &Value, count: Option<&Value>, span: Span) -> Result<Value, RuntimeError> {
    let code = okkama_runtime_core::integer::BitOp::from_code(op as u8)
        .ok_or_else(|| invalid_state(span, "unknown bit operation"))?;
    ops::bits(code, op.counts(), byte, value, count).map_err(fault(span))
}

pub(crate) fn convert(value: &Value, target: &ir::Type, span: Span) -> Result<Value, RuntimeError> {
    ops::convert(value, scalar(target)).map_err(|error| match error {
        Fault::Unsupported(_) => unsupported(span, format!("conversion to {target:?} is not supported yet")),
        other => fault(span)(other),
    })
}

pub(crate) fn byte_from(value: &Value, context: Option<&str>, span: Span) -> Result<Value, RuntimeError> {
    ops::byte_from(value, context).map_err(fault(span))
}

pub(crate) fn byte_binary(op: ir::IntBinaryOp, left: &Value, right: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::byte_binary(int_op(op), left, right).map_err(fault(span))
}

pub(crate) fn scalar_compare(op: ir::CompareOp, left: &Value, right: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::scalar_compare(cmp(op), left, right).map_err(fault(span))
}

pub(crate) fn int_binary(op: ir::IntBinaryOp, left: &Value, right: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::int_binary(int_op(op), left, right).map_err(fault(span))
}

pub(crate) fn int_unary(op: ir::IntUnaryOp, value: &Value, span: Span) -> Result<Value, RuntimeError> {
    let op = match op {
        ir::IntUnaryOp::Neg => ops::IntUnOp::Neg,
        ir::IntUnaryOp::Not => ops::IntUnOp::Not,
    };
    ops::int_unary(op, value).map_err(fault(span))
}

pub(crate) fn int_compare(op: ir::CompareOp, left: &Value, right: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::int_compare(cmp(op), left, right).map_err(fault(span))
}

pub(crate) fn bool_not(value: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::bool_not(value).map_err(fault(span))
}

pub(crate) fn bool_compare(op: ir::CompareOp, left: &Value, right: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::bool_compare(cmp(op), left, right).map_err(fault(span))
}

pub(crate) fn concat(left: &Value, right: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::concat(left, right).map_err(fault(span))
}

pub(crate) fn string_compare(op: ir::CompareOp, left: &Value, right: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::string_compare(cmp(op), left, right).map_err(fault(span))
}

pub(crate) fn string(op: ir::StrOp, args: &[&Value], span: Span) -> Result<Value, RuntimeError> {
    ops::string(str_op(op), args).map_err(fault(span))
}

pub(crate) fn map_new(entries: impl IntoIterator<Item = (Value, Value)>, span: Span) -> Result<Value, RuntimeError> {
    ops::map_new(entries).map_err(fault(span))
}

pub(crate) fn map_len(map: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::map_len(map).map_err(fault(span))
}

pub(crate) fn map_lookup<'a>(map: &'a Value, key: &Value, span: Span) -> Result<Option<&'a Value>, RuntimeError> {
    ops::map_lookup(map, key).map_err(fault(span))
}

/// Отсутствующий ключ — ошибка с ключом, как `Rt.get` JVM (`formatKey`).
pub(crate) fn map_get(module: &ir::Module, map: &Value, key: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::map_get(map, key, &|declaration, case| module.enums[declaration].cases[case].clone()).map_err(fault(span))
}

pub(crate) fn map_keys(map: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::map_keys(map).map_err(fault(span))
}

pub(crate) fn map_values(map: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::map_values(map).map_err(fault(span))
}

pub(crate) fn map_builder_put(builder: &Value, key: &Value, value: Value, span: Span) -> Result<Value, RuntimeError> {
    ops::map_builder_put(builder, key, value).map_err(fault(span))
}

pub(crate) fn map_builder_finish(builder: &Value, span: Span) -> Result<Value, RuntimeError> {
    ops::map_builder_finish(builder).map_err(fault(span))
}
