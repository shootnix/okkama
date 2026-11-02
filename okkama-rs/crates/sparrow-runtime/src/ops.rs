//! Операции над значениями, общие для исполнителя IR и VM Sparrow (как
//! `okkama.rt.Rt` для JVM-байткода): ни регистров, ни кадров, ни формата
//! программы. Отказ — [`Fault`]: программная ошибка, нарушение инварианта
//! исполнителя или неподдержанная операция; место ошибки добавляет вызывающий.
use crate::heap::{ArrayHeap, maps};
use crate::{IntValue, Value, ValueError, float, integer, rat, scalar, strings};
use std::cmp::Ordering;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    /// Программная ошибка (`RuntimeError: …`).
    Trap(String),
    /// Нарушение инварианта исполнителя (неверная программа).
    Invalid(String),
    Unsupported(String),
}

impl From<ValueError> for Fault {
    fn from(error: ValueError) -> Self {
        Fault::Invalid(error.message)
    }
}

pub type Result<T> = std::result::Result<T, Fault>;

fn trap(message: impl Into<String>) -> Fault {
    Fault::Trap(message.into())
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Cmp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Cmp {
    pub fn holds(self, order: Ordering) -> bool {
        match self {
            Cmp::Eq => order.is_eq(),
            Cmp::Ne => !order.is_eq(),
            Cmp::Lt => order.is_lt(),
            Cmp::Le => !order.is_gt(),
            Cmp::Gt => order.is_gt(),
            Cmp::Ge => !order.is_lt(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum IntOp {
    Add,
    Sub,
    Mul,
    Mod,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Ushr,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum IntUnOp {
    Neg,
    Not,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FloatOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Neg,
    Abs,
    Sqrt,
}

impl FloatOp {
    pub fn arity(self) -> usize {
        match self {
            FloatOp::Neg | FloatOp::Abs | FloatOp::Sqrt => 1,
            _ => 2,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum RatOp {
    New,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Abs,
}

impl RatOp {
    pub fn arity(self) -> usize {
        match self {
            RatOp::Neg | RatOp::Abs => 1,
            _ => 2,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StrOp {
    Len,
    Slice,
    Split,
    Replace,
    ReplaceFirst,
    Trim,
    Chomp,
    Utf8,
}

impl StrOp {
    pub fn arity(self) -> usize {
        match self {
            StrOp::Len | StrOp::Trim | StrOp::Chomp | StrOp::Utf8 => 1,
            StrOp::Split => 2,
            StrOp::Slice | StrOp::Replace | StrOp::ReplaceFirst => 3,
        }
    }
}

/// Целевой тип преобразования `cvt` (§16.1).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Scalar {
    Int,
    Float,
    Rat,
    Str,
    /// Любой другой тип: преобразование не поддержано.
    Other,
}

fn float_value(value: &Value) -> Result<f64> {
    Ok(value.as_float()?)
}

fn rat_value(value: &Value) -> Result<&rat::Rat> {
    Ok(value.as_rat()?)
}

pub fn float_constant(bits: u64) -> Result<Value> {
    let value = f64::from_bits(bits);
    if !value.is_finite() {
        return Err(Fault::Invalid("non-finite float constant".into()));
    }
    Ok(Value::Float(value))
}

pub fn float_op(op: FloatOp, args: &[&Value]) -> Result<Value> {
    if args.len() != op.arity() {
        return Err(Fault::Invalid("float operation argument count mismatch".into()));
    }
    let left = float_value(args[0])?;
    let right = || float_value(args[1]);
    let value = match op {
        FloatOp::Add => float::add(left, right()?),
        FloatOp::Sub => float::sub(left, right()?),
        FloatOp::Mul => float::mul(left, right()?),
        FloatOp::Div => float::div(left, right()?),
        FloatOp::Mod => float::modulo(left, right()?),
        FloatOp::Pow => float::pow(left, right()?),
        FloatOp::Neg => Ok(float::neg(left)),
        FloatOp::Abs => Ok(float::abs(left)),
        FloatOp::Sqrt => float::sqrt(left),
    };
    value.map(Value::Float).map_err(trap)
}

pub fn float_compare(op: Cmp, left: &Value, right: &Value) -> Result<Value> {
    Ok(Value::Bool(op.holds(float::compare(float_value(left)?, float_value(right)?))))
}

pub fn rat_op(op: RatOp, args: &[&Value]) -> Result<Value> {
    if args.len() != op.arity() {
        return Err(Fault::Invalid("rat operation argument count mismatch".into()));
    }
    if op == RatOp::New {
        return rat::Rat::new(args[0].as_int()?, args[1].as_int()?)
            .map(|value| Value::Rat(Rc::new(value)))
            .map_err(trap);
    }
    let left = rat_value(args[0])?;
    let right = || rat_value(args[1]);
    let value = match op {
        RatOp::Add => Ok(left.add(right()?)),
        RatOp::Sub => Ok(left.sub(right()?)),
        RatOp::Mul => Ok(left.mul(right()?)),
        RatOp::Div => left.div(right()?),
        RatOp::Mod => left.modulo(right()?),
        RatOp::Neg => Ok(left.neg()),
        RatOp::Abs => Ok(left.abs()),
        RatOp::New => unreachable!("handled above"),
    };
    value.map(|value| Value::Rat(Rc::new(value))).map_err(trap)
}

pub fn rat_compare(op: Cmp, left: &Value, right: &Value) -> Result<Value> {
    Ok(Value::Bool(op.holds(rat_value(left)?.compare(rat_value(right)?))))
}

pub fn rat_part(value: &Value, denominator: bool) -> Result<Value> {
    let value = rat_value(value)?;
    let part = if denominator { value.denominator() } else { value.numerator() };
    Ok(Value::Str(Rc::from(part.to_string())))
}

pub fn sign(value: &Value) -> Result<Value> {
    let sign = match value {
        Value::Int(value) => value.signum(),
        Value::Float(value) => float::sign(*value),
        Value::Rat(value) => value.sign(),
        _ => return Err(Fault::Invalid("expected int, float or rat for sign".into())),
    };
    Ok(Value::Int(IntValue::from_i64(sign)))
}

pub fn int_abs(value: &Value) -> Result<Value> {
    let value = value.as_int()?;
    Ok(Value::Int(if value.is_negative() { value.negated() } else { value.clone() }))
}

/// Битовое действие; `op` — код [`integer::BitOp`], `counts` — результат-счётчик.
pub fn bits(op: integer::BitOp, counts: bool, byte: bool, value: &Value, count: Option<&Value>) -> Result<Value> {
    let count = count.map(Value::as_int).transpose()?;
    if byte {
        let value = value.as_byte()?;
        let result = integer::byte_bits(op, value, count).map_err(trap)?;
        Ok(if counts { Value::Int(IntValue::from_i64(i64::from(result))) } else { Value::Byte(result) })
    } else {
        Ok(Value::Int(integer::int_bits(op, value.as_int()?, count).map_err(trap)?))
    }
}

pub fn convert(value: &Value, target: Scalar) -> Result<Value> {
    match (value, target) {
        (Value::Int(_) | Value::Bool(_) | Value::Byte(_), Scalar::Str) => Ok(Value::Str(Rc::from(scalar::string(value)?))),
        (Value::Byte(value), Scalar::Int) => Ok(Value::Int(IntValue::from_i64(i64::from(*value)))),
        (Value::Int(value), Scalar::Float) => float::from_int(value).map(Value::Float).map_err(trap),
        (Value::Byte(value), Scalar::Float) => Ok(Value::Float(f64::from(*value))),
        (Value::Float(value), Scalar::Int) => float::to_int(*value).map(Value::Int).map_err(trap),
        (Value::Float(value), Scalar::Str) => Ok(Value::Str(Rc::from(float::text(*value)))),
        (Value::Int(value), Scalar::Rat) => Ok(Value::Rat(Rc::new(rat::Rat::from_int(value)))),
        (Value::Byte(value), Scalar::Rat) => Ok(Value::Rat(Rc::new(rat::Rat::from_int(&IntValue::from_i64(i64::from(*value)))))),
        (Value::Float(value), Scalar::Rat) => Ok(Value::Rat(Rc::new(rat::Rat::from_float(*value)))),
        (Value::Rat(value), Scalar::Int) => Ok(Value::Int(value.to_int())),
        (Value::Rat(value), Scalar::Float) => value.to_float().map(Value::Float).map_err(trap),
        (Value::Rat(value), Scalar::Str) => Ok(Value::Str(Rc::from(value.to_string()))),
        (_, target) => Err(Fault::Unsupported(format!("conversion to {target:?} is not supported yet"))),
    }
}

pub fn byte_from(value: &Value, context: Option<&str>) -> Result<Value> {
    match value {
        Value::Int(value) => scalar::byte(value, context),
        Value::Float(value) => float::to_byte(*value, context),
        Value::Rat(value) => value.to_byte(context),
        _ => return Err(Fault::Invalid("expected int, float or rat for byte".into())),
    }
    .map(Value::Byte)
    .map_err(trap)
}

pub fn byte_binary(op: IntOp, left: &Value, right: &Value) -> Result<Value> {
    let left = left.as_byte()?;
    let value = match op {
        IntOp::Shl | IntOp::Shr | IntOp::Ushr => scalar::byte_shift(left, right.as_int()?, op == IntOp::Shl).map_err(trap)?,
        IntOp::And | IntOp::Or | IntOp::Xor => {
            let right = right.as_byte()?;
            match op {
                IntOp::And => left & right,
                IntOp::Or => left | right,
                _ => left ^ right,
            }
        }
        _ => return Err(Fault::Invalid("invalid byte binary operation".into())),
    };
    Ok(Value::Byte(value))
}

pub fn byte_not(value: &Value) -> Result<Value> {
    Ok(Value::Byte(!value.as_byte()?))
}

pub fn scalar_compare(op: Cmp, left: &Value, right: &Value) -> Result<Value> {
    let order = match (left, right) {
        (Value::Byte(a), Value::Byte(b)) => a.cmp(b),
        (Value::Char(a), Value::Char(b)) => a.cmp(b),
        _ => return Err(Fault::Invalid("expected matching byte or char operands".into())),
    };
    Ok(Value::Bool(op.holds(order)))
}

pub fn int_binary(op: IntOp, left: &Value, right: &Value) -> Result<Value> {
    let (left, right) = (left.as_int()?, right.as_int()?);
    Ok(Value::Int(match op {
        IntOp::Add => integer::add(left, right),
        IntOp::Mul => integer::multiply(left, right),
        IntOp::Mod => integer::remainder(left, right).ok_or_else(|| trap("Modulo by zero"))?,
        IntOp::Sub => integer::subtract(left, right),
        IntOp::And => integer::bit_and(left, right).map_err(trap)?,
        IntOp::Or => integer::bit_or(left, right).map_err(trap)?,
        IntOp::Xor => integer::bit_xor(left, right).map_err(trap)?,
        IntOp::Shl | IntOp::Shr | IntOp::Ushr => {
            integer::shift(left, right, op == IntOp::Shl, op == IntOp::Ushr).map_err(trap)?
        }
    }))
}

pub fn int_unary(op: IntUnOp, value: &Value) -> Result<Value> {
    match op {
        IntUnOp::Neg => Ok(Value::Int(integer::negate(value.as_int()?))),
        IntUnOp::Not => Ok(Value::Int(integer::bit_not(value.as_int()?).map_err(trap)?)),
    }
}

pub fn int_compare(op: Cmp, left: &Value, right: &Value) -> Result<Value> {
    Ok(Value::Bool(op.holds(integer::compare(left.as_int()?, right.as_int()?))))
}

pub fn bool_not(value: &Value) -> Result<Value> {
    Ok(Value::Bool(!value.as_bool()?))
}

pub fn bool_compare(op: Cmp, left: &Value, right: &Value) -> Result<Value> {
    Ok(Value::Bool(op.holds(left.as_bool()?.cmp(&right.as_bool()?))))
}

pub fn concat(left: &Value, right: &Value) -> Result<Value> {
    let mut joined = String::from(left.as_string()?);
    joined.push_str(right.as_string()?);
    Ok(Value::Str(Rc::from(joined)))
}

pub fn string_compare(op: Cmp, left: &Value, right: &Value) -> Result<Value> {
    let (left, right) = (left.as_string()?, right.as_string()?);
    Ok(Value::Bool(match op {
        Cmp::Eq => left == right,
        Cmp::Ne => left != right,
        // По Unicode code points (`KeyOrder.compareCodePoints`): для UTF-8 это
        // и есть побайтовый порядок.
        _ => op.holds(left.cmp(right)),
    }))
}

pub fn string(op: StrOp, args: &[&Value]) -> Result<Value> {
    if args.len() != op.arity() {
        return Err(Fault::Invalid("string operation argument count mismatch".into()));
    }
    match op {
        StrOp::Trim => Ok(Value::Str(Rc::from(strings::trim(args[0].as_string()?)))),
        StrOp::Chomp => Ok(Value::Str(Rc::from(strings::chomp(args[0].as_string()?)))),
        StrOp::Replace | StrOp::ReplaceFirst => Ok(Value::Str(Rc::from(strings::replace(
            args[0].as_string()?,
            args[1].as_string()?,
            args[2].as_string()?,
            op == StrOp::ReplaceFirst,
        )))),
        StrOp::Len => Ok(Value::Int(IntValue::from_u128(args[0].as_string()?.chars().count() as u128))),
        StrOp::Slice => Ok(Value::Str(Rc::from(
            strings::slice(args[0].as_string()?, args[1].as_int()?, args[2].as_int()?).map_err(trap)?,
        ))),
        StrOp::Split => Ok(ArrayHeap.split(args[0].as_string()?, args[1].as_string()?)),
        StrOp::Utf8 => Ok(ArrayHeap.utf8(args[0].as_string()?)),
    }
}

pub fn map_new(entries: impl IntoIterator<Item = (Value, Value)>) -> Result<Value> {
    Ok(maps::from_entries(entries)?)
}

pub fn map_len(map: &Value) -> Result<Value> {
    Ok(Value::Int(IntValue::from_u128(maps::entries(map)?.len() as u128)))
}

pub fn map_lookup<'a>(map: &'a Value, key: &Value) -> Result<Option<&'a Value>> {
    Ok(maps::get(map, key)?)
}

/// Отсутствующий ключ — ошибка с ключом, как `Rt.get` JVM (`formatKey`);
/// `case_name` даёт имя case ключа-enum.
pub fn map_get(map: &Value, key: &Value, case_name: &dyn Fn(usize, usize) -> String) -> Result<Value> {
    match map_lookup(map, key)? {
        Some(value) => Ok(value.clone()),
        None => Err(trap(format!("Map key not found: {}", key_text(key, case_name)))),
    }
}

fn key_text(key: &Value, case_name: &dyn Fn(usize, usize) -> String) -> String {
    match key {
        Value::Str(text) => format!("\"{text}\""),
        Value::Bool(value) => if *value { "1" } else { "0" }.into(),
        Value::Byte(value) => value.to_string(),
        Value::Float(value) => float::text(*value),
        Value::Char(value) => format!("\"{value}\""),
        Value::Enum { declaration, case } => format!("\"{}\"", case_name(*declaration, *case)),
        Value::Int(value) => value.to_string(),
        other => format!("{other:?}"),
    }
}

pub fn map_keys(map: &Value) -> Result<Value> {
    Ok(maps::keys(map)?)
}

pub fn map_values(map: &Value) -> Result<Value> {
    Ok(maps::values(map)?)
}

pub fn map_builder_put(builder: &Value, key: &Value, value: Value) -> Result<Value> {
    Ok(maps::builder_put(builder, key, value)?)
}

pub fn map_builder_finish(builder: &Value) -> Result<Value> {
    Ok(maps::builder_finish(builder)?)
}

// ---- массивы и сборщики ----

pub fn array_slice(array: &Value, start: &IntValue, end: &IntValue) -> Result<Value> {
    let elements = ArrayHeap.elements(array)?;
    let range = integer::slice_range(elements.len(), start, end).map_err(trap)?;
    Ok(ArrayHeap.allocate(elements[range].to_vec()))
}

pub fn array_join(array: &Value, separator: &str) -> Result<Value> {
    Ok(ArrayHeap.join(array, separator).map(|text| Value::Str(Rc::from(text)))?)
}

pub fn array_len(array: &Value) -> Result<Value> {
    Ok(Value::Int(IntValue::from_u128(ArrayHeap.elements(array)?.len() as u128)))
}

pub fn array_lookup<'a>(array: &'a Value, index: &IntValue) -> Result<Option<&'a Value>> {
    let elements = ArrayHeap.elements(array)?;
    Ok(index.to_i64().and_then(|i| usize::try_from(i).ok()).and_then(|i| elements.get(i)))
}

pub fn array_get(array: &Value, index: &IntValue) -> Result<Value> {
    array_lookup(array, index)?.cloned().ok_or_else(|| trap("Array index out of bounds"))
}

pub fn builder_range(builder: &Value, start: &IntValue, end: &IntValue, inclusive: bool) -> Result<Value> {
    let values = integer::range(start, end, inclusive).map_err(trap)?;
    Ok(ArrayHeap.builder_extend(builder, values.map(Value::Int))?)
}

pub fn range_new(start: &IntValue, end: &IntValue, inclusive: bool) -> Result<Value> {
    Ok(Value::Range(Rc::new(crate::range::Range::new(start.clone(), end.clone(), inclusive).map_err(trap)?)))
}

pub fn range_part(range: &Value, end: bool) -> Result<Value> {
    let range = range.as_range()?;
    Ok(Value::Int(if end { range.end_exclusive() } else { range.start() }))
}

/// Глубокое равенство (`val.eq`).
pub fn equal(left: &Value, right: &Value) -> Result<bool> {
    crate::heap::ObjectHeap.equal(&ArrayHeap, left, right).map_err(|error| match error {
        crate::heap::EqualityError::InvalidState(error) => Fault::Invalid(error.message),
        crate::heap::EqualityError::Any => trap("Cannot compare any without narrowing"),
        crate::heap::EqualityError::MixedTypes => trap("Cannot compare values of different types"),
    })
}

/// Поле объекта; у `NativeFailure` — его текстовые поля.
pub fn object_get(value: &Value, field: usize) -> Result<Value> {
    if let Value::NativeFailure(failure) = value {
        let text = match field {
            0 => &failure.domain,
            1 => &failure.code,
            2 => &failure.operation,
            3 => &failure.message,
            _ => return Err(Fault::Invalid("native failure field index out of bounds".into())),
        };
        return Ok(Value::Str(Rc::from(text.as_str())));
    }
    Ok(crate::heap::ObjectHeap.field(value, field)?)
}
