//! `const NAME = value;` модуля (#153): значение вычисляет компилятор.
//!
//! Инициализатор проверяется как выражение модуля без локальных, `self` и
//! вызовов и вычисляется операциями IR-движка (`okkama_runtime_core::ops`),
//! поэтому константа равна тому, что программа получила бы во время
//! исполнения. Допустимы литералы, варианты enum без данных, операторы над
//! ними и над другими константами, массивы и словари таких значений. На месте
//! использования скаляр становится литералом, а массив или словарь — ссылкой
//! на одно значение, построенное движком при загрузке (`ExprKind::Global`).

use std::rc::Rc;

use okkama_runtime_core::{IntValue, Value, heap::ArrayHeap, ops};

use super::{Checker, Kind};
use crate::analysis::registry::{ConstResult, ConstState, Registry};
use crate::analysis::thir::{
    ArithOp, ArrayPart, BinaryOp, BitOp, CastTarget, CompareKind, CompareOp, ConstValue, Constant, Expr,
    ExprKind, NumericKind, UnaryOp,
};
use crate::analysis::ty::{EnumId, Ty};
use crate::source::Span;

impl Checker<'_, '_> {
    /// `NAME` на месте использования: литерал или ссылка на значение.
    pub(super) fn const_value(&mut self, id: usize, span: Span) -> Expr {
        let Some(result) = evaluate(self.reg, id) else {
            return Self::poisoned(span);
        };
        let kind = match &result.value {
            ConstValue::Scalar(constant) => ExprKind::Constant(constant.clone()),
            ConstValue::Rat(numerator, denominator) => {
                let int = |text: &String| Box::new(Self::typed(ExprKind::Constant(Constant::Int(text.clone())), Ty::Int, span));
                ExprKind::RatNew {
                    numerator: int(numerator),
                    denominator: int(denominator),
                }
            }
            ConstValue::Enum(ty, case) => ExprKind::EnumCase { ty: *ty, case: *case },
            ConstValue::Array(_) | ConstValue::Map(_) => ExprKind::Global(id),
        };
        Self::typed(kind, result.ty, span)
    }
}

/// Значение константы `id`, вычисленное один раз; `None` — ошибка сообщена.
pub(super) fn evaluate(reg: &mut Registry<'_>, id: usize) -> Option<ConstResult> {
    if reg.const_state.len() < reg.consts.len() {
        reg.const_state.resize(reg.consts.len(), ConstState::Pending);
    }
    let info = reg.consts[id].clone();
    match &reg.const_state[id] {
        ConstState::Done(result) => return result.clone(),
        ConstState::Busy => {
            let previous = reg.current_module();
            reg.set_module(info.module);
            reg.error(info.decl.name.span, format!("Constant \"{}\" depends on itself", info.name));
            reg.set_module(previous);
            return None;
        }
        ConstState::Pending => {}
    }
    reg.const_state[id] = ConstState::Busy;
    let previous = reg.current_module();
    let result = {
        let mut checker = Checker::new(reg, Kind::Const, None, None);
        checker.module = info.module;
        checker.reg.set_module(info.module);
        let declared = info.decl.ty.as_ref().map(|ty| checker.reg.resolve(ty, &[]));
        let value = checker.value(&info.decl.value, declared.as_ref());
        let value = match &declared {
            Some(ty) if !value.diverges => checker.coerce(value, ty, info.decl.value.span, "Constant"),
            _ => value,
        };
        let ty = Checker::ty_of(&value).taken();
        if ty.is_error() || value.diverges {
            None
        } else {
            match (Eval { reg: checker.reg }).expr(&value) {
                Ok(runtime) => Some(ConstResult {
                    value: tree(&runtime),
                    ty,
                    runtime,
                }),
                Err(problem) => {
                    let message = format!("Constant \"{}\" cannot be computed by the compiler: {problem}", info.name);
                    checker.error(info.decl.value.span, message);
                    None
                }
            }
        }
    };
    reg.set_module(previous);
    reg.const_state[id] = ConstState::Done(result.clone());
    result
}

/// Вычисление проверенного инициализатора операциями IR-движка.
struct Eval<'r, 'a> {
    reg: &'r Registry<'a>,
}

type Outcome = Result<Value, String>;

fn fault(error: impl std::fmt::Debug) -> String {
    let text = format!("{error:?}");
    // `Trap("Division by zero")` → `Division by zero`.
    match (text.find('"'), text.rfind('"')) {
        (Some(start), Some(end)) if end > start => text[start + 1..end].to_string(),
        _ => text,
    }
}

impl Eval<'_, '_> {
    fn expr(&self, expr: &Expr) -> Outcome {
        match &expr.kind {
            ExprKind::Constant(constant) => scalar(constant),
            ExprKind::EnumCase { ty, case } => Ok(Value::Enum {
                declaration: ty.0,
                case: *case,
            }),
            ExprKind::Global(id) => match &self.reg.const_state.get(*id) {
                Some(ConstState::Done(Some(result))) => Ok(result.runtime.clone()),
                _ => Err("it depends on a constant that has no value".into()),
            },
            ExprKind::RatNew { numerator, denominator } => {
                let (numerator, denominator) = (self.expr(numerator)?, self.expr(denominator)?);
                ops::rat_op(ops::RatOp::New, &[&numerator, &denominator]).map_err(fault)
            }
            ExprKind::Unary { op, operand } => {
                let value = self.expr(operand)?;
                match op {
                    UnaryOp::Not => ops::bool_not(&value),
                    UnaryOp::Negate(NumericKind::Int) => ops::int_unary(ops::IntUnOp::Neg, &value),
                    UnaryOp::Negate(NumericKind::Byte) => {
                        let int = ops::convert(&value, ops::Scalar::Int).map_err(fault)?;
                        ops::int_unary(ops::IntUnOp::Neg, &int)
                    }
                    UnaryOp::Negate(NumericKind::Float) => ops::float_op(ops::FloatOp::Neg, &[&value]),
                    UnaryOp::Negate(NumericKind::Rat) => ops::rat_op(ops::RatOp::Neg, &[&value]),
                    UnaryOp::BitNot { byte: false } => ops::int_unary(ops::IntUnOp::Not, &value),
                    UnaryOp::BitNot { byte: true } => ops::byte_not(&value),
                }
                .map_err(fault)
            }
            ExprKind::Logical { and, left, right } => {
                let left = self.expr(left)?;
                match (left, and) {
                    (Value::Bool(false), true) => Ok(Value::Bool(false)),
                    (Value::Bool(true), false) => Ok(Value::Bool(true)),
                    _ => self.expr(right),
                }
            }
            ExprKind::Binary { op, left, right } => {
                let (left, right) = (self.expr(left)?, self.expr(right)?);
                binary(*op, &left, &right)
            }
            ExprKind::Cast { target, value, .. } => {
                let value = self.expr(value)?;
                match target {
                    CastTarget::Byte => ops::byte_from(&value, None),
                    CastTarget::Int => ops::convert(&value, ops::Scalar::Int),
                    CastTarget::Float => ops::convert(&value, ops::Scalar::Float),
                    CastTarget::Rat => ops::convert(&value, ops::Scalar::Rat),
                    CastTarget::Str => ops::convert(&value, ops::Scalar::Str),
                    CastTarget::Char => ops::convert(&value, ops::Scalar::Char),
                }
                .map_err(fault)
            }
            ExprKind::Coerce { coercion, value } => match coercion {
                crate::analysis::thir::Coercion::Retag => self.expr(value),
                _ => Err("its value has to cross a role or `any` boundary".into()),
            },
            ExprKind::Array(parts) => {
                let mut items = Vec::new();
                for part in parts {
                    match part {
                        ArrayPart::Value(value) => items.push(self.expr(value)?),
                        ArrayPart::Range { start, end, inclusive } => {
                            let (start, end) = (self.expr(start)?, self.expr(end)?);
                            let (start, end) = (word(&start)?, word(&end)?);
                            if start > end {
                                return Err("Range start must be less than or equal to range end".into());
                            }
                            let last = if *inclusive { end } else { end - 1 };
                            if last - start >= MAX_ELEMENTS {
                                return Err(format!("a range of more than {MAX_ELEMENTS} elements"));
                            }
                            items.extend((start..=last).map(|n| Value::Int(IntValue::from_i64(n))));
                        }
                    }
                }
                Ok(ArrayHeap.allocate(items))
            }
            ExprKind::Map(entries) => {
                let mut pairs = Vec::new();
                for (key, value) in entries {
                    pairs.push((self.expr(key)?, self.expr(value)?));
                }
                ops::map_new(pairs).map_err(fault)
            }
            ExprKind::Local(_) | ExprKind::SelfValue | ExprKind::Field(_) | ExprKind::PendingField(_) => {
                Err("it reads a variable".into())
            }
            ExprKind::Call { .. } => {
                Err("it calls an action or a constructor; a constant is made of literals, operators and other constants".into())
            }
            _ => Err("only literals, enum cases, operators, arrays, maps and other constants are allowed".into()),
        }
    }
}

/// The most elements a range may add to a constant array.
const MAX_ELEMENTS: i64 = 1 << 20;

fn word(value: &Value) -> Result<i64, String> {
    okkama_runtime_core::integer::word(value.as_int().map_err(fault)?).map_err(str::to_string)
}

fn scalar(constant: &Constant) -> Outcome {
    Ok(match constant {
        Constant::Int(text) => Value::Int(IntValue::parse(text).ok_or("an invalid integer literal")?),
        Constant::Float(bits) => return ops::float_constant(*bits).map_err(fault),
        Constant::Str(text) => Value::Str(Rc::from(text.as_str())),
        Constant::Char(value) => Value::Char(*value),
        Constant::Bool(value) => Value::Bool(*value),
        Constant::Byte(value) => Value::Byte(*value),
    })
}

fn binary(op: BinaryOp, left: &Value, right: &Value) -> Outcome {
    let arith_int = |op: ArithOp| match op {
        ArithOp::Add => Some(ops::IntOp::Add),
        ArithOp::Sub => Some(ops::IntOp::Sub),
        ArithOp::Mul => Some(ops::IntOp::Mul),
        ArithOp::Mod => Some(ops::IntOp::Mod),
        ArithOp::Div => None,
    };
    let bits = |op: BitOp| match op {
        BitOp::And => ops::IntOp::And,
        BitOp::Or => ops::IntOp::Or,
        BitOp::Xor => ops::IntOp::Xor,
        BitOp::Shl => ops::IntOp::Shl,
        BitOp::Shr => ops::IntOp::Shr,
        BitOp::Ushr => ops::IntOp::Ushr,
    };
    let cmp = |op: CompareOp| match op {
        CompareOp::Eq => ops::Cmp::Eq,
        CompareOp::Ne => ops::Cmp::Ne,
        CompareOp::Lt => ops::Cmp::Lt,
        CompareOp::Le => ops::Cmp::Le,
        CompareOp::Gt => ops::Cmp::Gt,
        CompareOp::Ge => ops::Cmp::Ge,
    };
    match op {
        BinaryOp::Arith(NumericKind::Int, arith) => match arith_int(arith) {
            Some(op) => ops::int_binary(op, left, right),
            None => ops::rat_op(ops::RatOp::New, &[left, right]),
        },
        BinaryOp::Arith(NumericKind::Byte, arith) => {
            let (left, right) = (
                ops::convert(left, ops::Scalar::Int).map_err(fault)?,
                ops::convert(right, ops::Scalar::Int).map_err(fault)?,
            );
            match arith_int(arith) {
                Some(op) => ops::int_binary(op, &left, &right),
                None => ops::rat_op(ops::RatOp::New, &[&left, &right]),
            }
        }
        BinaryOp::Arith(NumericKind::Float, arith) => {
            let op = match arith {
                ArithOp::Add => ops::FloatOp::Add,
                ArithOp::Sub => ops::FloatOp::Sub,
                ArithOp::Mul => ops::FloatOp::Mul,
                ArithOp::Div => ops::FloatOp::Div,
                ArithOp::Mod => ops::FloatOp::Mod,
            };
            ops::float_op(op, &[left, right])
        }
        BinaryOp::Arith(NumericKind::Rat, arith) => {
            let op = match arith {
                ArithOp::Add => ops::RatOp::Add,
                ArithOp::Sub => ops::RatOp::Sub,
                ArithOp::Mul => ops::RatOp::Mul,
                ArithOp::Div => ops::RatOp::Div,
                ArithOp::Mod => ops::RatOp::Mod,
            };
            ops::rat_op(op, &[left, right])
        }
        BinaryOp::Bits { byte: false, op } => ops::int_binary(bits(op), left, right),
        BinaryOp::Bits { byte: true, op } => ops::byte_binary(bits(op), left, right),
        BinaryOp::Concat => ops::concat(left, right),
        BinaryOp::ArrayConcat => ops::array_concat(&[left, right]),
        BinaryOp::ArrayDifference => {
            let removed = ArrayHeap.elements(right).map_err(fault)?.to_vec();
            ops::array_difference(left, &removed.iter().collect::<Vec<_>>())
        }
        BinaryOp::MapMerge => ops::map_merge(&[left, right]),
        BinaryOp::MapDifference => ops::map_difference(left, &[right]),
        BinaryOp::Compare(kind, op) => match kind {
            CompareKind::Int => ops::int_compare(cmp(op), left, right),
            CompareKind::Float => ops::float_compare(cmp(op), left, right),
            CompareKind::Rat => ops::rat_compare(cmp(op), left, right),
            CompareKind::Bool => ops::bool_compare(cmp(op), left, right),
            CompareKind::Byte | CompareKind::Char => ops::scalar_compare(cmp(op), left, right),
            CompareKind::Str => ops::string_compare(cmp(op), left, right),
            CompareKind::Enum | CompareKind::Value => {
                let equal = ops::equal(left, right).map_err(fault)?;
                return Ok(Value::Bool(if op == CompareOp::Eq { equal } else { !equal }));
            }
        },
    }
    .map_err(fault)
}

/// Значение рантайма как дерево для lowering.
fn tree(value: &Value) -> ConstValue {
    match value {
        Value::Int(value) => ConstValue::Scalar(Constant::Int(value.to_string())),
        Value::Float(value) => ConstValue::Scalar(Constant::Float(value.to_bits())),
        Value::Str(text) => ConstValue::Scalar(Constant::Str(text.to_string())),
        Value::Char(value) => ConstValue::Scalar(Constant::Char(*value)),
        Value::Bool(value) => ConstValue::Scalar(Constant::Bool(*value)),
        Value::Byte(value) => ConstValue::Scalar(Constant::Byte(*value)),
        Value::Rat(rat) => {
            let numerator = ops::rat_part(value, false).ok().and_then(|text| text.as_string().ok().map(str::to_string));
            let denominator = ops::rat_part(value, true).ok().and_then(|text| text.as_string().ok().map(str::to_string));
            let _ = rat;
            ConstValue::Rat(numerator.unwrap_or_default(), denominator.unwrap_or_else(|| "1".into()))
        }
        Value::Enum { declaration, case } => ConstValue::Enum(EnumId(*declaration), *case),
        Value::Array(_) => ConstValue::Array(
            ArrayHeap.elements(value).map(|items| items.iter().map(tree).collect()).unwrap_or_default(),
        ),
        Value::Map(_) => {
            let keys = ops::map_keys(value).ok();
            let values = ops::map_values(value).ok();
            match (keys, values) {
                (Some(keys), Some(values)) => {
                    let keys = ArrayHeap.elements(&keys).map(|items| items.to_vec()).unwrap_or_default();
                    let values = ArrayHeap.elements(&values).map(|items| items.to_vec()).unwrap_or_default();
                    ConstValue::Map(keys.iter().map(tree).zip(values.iter().map(tree)).collect())
                }
                _ => ConstValue::Map(Vec::new()),
            }
        }
        _ => ConstValue::Array(Vec::new()),
    }
}
