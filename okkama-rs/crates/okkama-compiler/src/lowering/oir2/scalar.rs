//! S1 instruction allowlist. Unsupported THIR variants have no OIR-1 fallback.
use super::body::BodyLowerer;
use super::{LowerError, Result};
use crate::analysis::thir::{self, ArithOp, BinaryOp, CastTarget, CompareKind, CompareOp, NumericKind, UnaryOp};
use crate::oir2 as ir;
use crate::source::Span;

macro_rules! value {
    ($this:expr, $expr:expr) => {
        match $this.value($expr)? {
            Some(value) => value,
            None => return Ok(None),
        }
    };
}

impl BodyLowerer<'_, '_> {
    pub(super) fn core(&mut self, op: ir::CoreOp) -> Result<ir::ValueId> {
        let operation = ir::Operation::Core(op);
        let contract = ir::ContractRegistry::new(self.b.module())
            .lookup(&operation).map_err(ir::BuildError::from)?;
        if !contract.effects.failures.is_empty() || contract.effects.suspends {
            self.sync_failure()?;
        }
        Ok(self.b.emit_schema(operation)?[0])
    }

    pub(super) fn collection(&mut self, op: ir::CollectionOp) -> Result<ir::ValueId> {
        // Only std support arrays of plain strings/bytes are interned by declarations.
        self.sync_failure()?;
        Ok(self.b.emit_schema(ir::Operation::Collection(op))?[0])
    }

    pub(super) fn constant(&mut self, value: ir::Literal) -> Result<ir::ValueId> {
        Ok(self.b.emit_schema(ir::Operation::Const(value))?[0])
    }

    pub(super) fn scalar(&mut self, expr: &thir::Expr) -> Result<Option<ir::ValueId>> {
        use thir::ExprKind as E;
        let result = match &expr.kind {
            E::Constant(constant) => self.constant(literal(constant, expr.span)?)?,
            E::Global(_) => return Err(LowerError::unsupported(expr.span, "module constant reference")),
            E::Unary { op, operand } => {
                let value = value!(self, operand);
                let op = match op {
                    UnaryOp::Not => ir::CoreOp::BoolNot { value },
                    UnaryOp::Negate(NumericKind::Int) => ir::CoreOp::IntNeg { value },
                    _ => return Err(LowerError::unsupported(expr.span, format!("unary {op:?}"))),
                };
                self.core(op)?
            }
            E::Binary { op, left, right } => {
                let left = value!(self, left);
                let right = value!(self, right);
                let operation = match op {
                    BinaryOp::Arith(NumericKind::Int, ArithOp::Add) => ir::CoreOp::IntAdd { left, right },
                    BinaryOp::Arith(NumericKind::Int, ArithOp::Sub) => ir::CoreOp::IntSub { left, right },
                    BinaryOp::Arith(NumericKind::Int, ArithOp::Mul) => ir::CoreOp::IntMul { left, right },
                    BinaryOp::Arith(NumericKind::Int, ArithOp::Mod) => ir::CoreOp::IntMod { left, right },
                    BinaryOp::Concat => ir::CoreOp::StrConcat { left, right },
                    BinaryOp::Compare(kind, op) => compare(*kind, *op, left, right, expr.span)?,
                    _ => return Err(LowerError::unsupported(expr.span, format!("binary {op:?}"))),
                };
                self.core(operation)?
            }
            E::Cast { target, value, .. } => {
                let value = value!(self, value);
                let to = match target {
                    CastTarget::Int => ir::NumericType::Int,
                    CastTarget::Str => ir::NumericType::Str,
                    _ => return Err(LowerError::unsupported(expr.span, format!("cast {target:?}"))),
                };
                self.convert(value, to, expr.span)?
            }
            E::Call { callee: thir::Callee::Builtin { receiver, op }, args } => {
                let value = value!(self, receiver);
                match (op, args.as_slice()) {
                    (thir::Builtin::ArraySize, []) => {
                        self.collection(ir::CollectionOp::ArrayLen { array: value })?
                    }
                    (thir::Builtin::StrLength, []) => self.core(ir::CoreOp::StrLen { value })?,
                    (thir::Builtin::Convert(target), []) => {
                        let to = match target {
                            CastTarget::Int => ir::NumericType::Int,
                            CastTarget::Str => ir::NumericType::Str,
                            _ => return Err(LowerError::unsupported(expr.span, format!("conversion {target:?}"))),
                        };
                        self.convert(value, to, expr.span)?
                    }
                    (thir::Builtin::Compare(kind, op), [other]) => {
                        let right = value!(self, other);
                        self.core(compare(*kind, *op, value, right, expr.span)?)?
                    }
                    (thir::Builtin::StrCompare(op), [other]) => {
                        let right = value!(self, other);
                        self.core(compare(CompareKind::Str, *op, value, right, expr.span)?)?
                    }
                    _ => return Err(LowerError::unsupported(expr.span, format!("builtin {op:?}"))),
                }
            }
            E::Range { start, end, inclusive } => {
                let start = value!(self, start);
                let end = value!(self, end);
                self.core(ir::CoreOp::RangeNew { start, end, inclusive: *inclusive })?
            }
            _ => return Err(LowerError::unsupported(expr.span, construct(&expr.kind))),
        };
        Ok(Some(result))
    }

    fn convert(&mut self, value: ir::ValueId, to: ir::NumericType, span: Span) -> Result<ir::ValueId> {
        let ty = self.b.body().values.get(value)?.ty;
        let from = match self.b.module().types.get(ty)? {
            ir::Type::Int => ir::NumericType::Int,
            ir::Type::Bool => ir::NumericType::Bool,
            ir::Type::Str => ir::NumericType::Str,
            _ => return Err(LowerError::unsupported(span, "conversion from a non-S1 type")),
        };
        if from == to {
            return Ok(value);
        }
        self.core(ir::CoreOp::Convert { value, from, to })
    }

    pub(super) fn equal(&mut self, left: ir::ValueId, right: ir::ValueId, span: Span) -> Result<ir::ValueId> {
        let ty = self.b.body().values.get(left)?.ty;
        let kind = match self.b.module().types.get(ty)? {
            ir::Type::Int => CompareKind::Int,
            ir::Type::Bool => CompareKind::Bool,
            ir::Type::Str => CompareKind::Str,
            _ => return Err(LowerError::unsupported(span, "non-scalar match subject")),
        };
        self.core(compare(kind, CompareOp::Eq, left, right, span)?)
    }
}

fn literal(constant: &thir::Constant, span: Span) -> Result<ir::Literal> {
    match constant {
        thir::Constant::Int(value) => Ok(ir::Literal::Int(value.clone())),
        thir::Constant::Bool(value) => Ok(ir::Literal::Bool(*value)),
        thir::Constant::Str(value) => Ok(ir::Literal::Str(value.clone())),
        _ => Err(LowerError::unsupported(span, format!("constant {constant:?}"))),
    }
}

fn construct(expr: &thir::ExprKind) -> &'static str {
    use thir::ExprKind as E;
    match expr {
        E::Call { .. } => "direct, constructor or role call (S2)",
        E::SelfValue | E::Field(_) | E::PendingField(_) => "object/field access (S2)",
        E::Try(_) | E::Panic(_) => "try/panic (#567)",
        E::Coerce { .. } => "coercion/successor (S2)",
        E::Await(_) | E::Spawn(_) => "async operation",
        E::Closure { .. } | E::CallValue { .. } | E::BoundAction { .. } => "closure or action value",
        E::Array(_) | E::Map(_) | E::Index { .. } | E::Functional(_) => "collection operation",
        _ => "expression outside the S1 allowlist",
    }
}

fn compare(
    kind: CompareKind,
    op: CompareOp,
    left: ir::ValueId,
    right: ir::ValueId,
    span: Span,
) -> Result<ir::CoreOp> {
    use ir::CoreOp as C;
    use CompareKind as K;
    use CompareOp as O;
    Ok(match (kind, op) {
        (K::Int, O::Eq) => C::IntEq { left, right },
        (K::Int, O::Ne) => C::IntNe { left, right },
        (K::Int, O::Lt) => C::IntLt { left, right },
        (K::Int, O::Le) => C::IntLe { left, right },
        (K::Int, O::Gt) => C::IntGt { left, right },
        (K::Int, O::Ge) => C::IntGe { left, right },
        (K::Bool, O::Eq) => C::BoolEq { left, right },
        (K::Bool, O::Ne) => C::BoolNe { left, right },
        (K::Str, O::Eq) => C::StrEq { left, right },
        (K::Str, O::Ne) => C::StrNe { left, right },
        (K::Str, O::Lt) => C::StrLt { left, right },
        (K::Str, O::Le) => C::StrLe { left, right },
        (K::Str, O::Gt) => C::StrGt { left, right },
        (K::Str, O::Ge) => C::StrGe { left, right },
        _ => return Err(LowerError::unsupported(span, format!("comparison {kind:?} {op:?}"))),
    })
}
