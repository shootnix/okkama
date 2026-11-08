//! Вызовы: actions объектов и ролей, конструкторы, natives, builtins,
//! проекции и variadic-хвосты.
//!
//! Операнды вычисляются слева направо, затем — границы параметров (`succ.take`,
//! `role.wrap`, тег пустой коллекции) в порядке параметров, как в Perl.

use super::matching::compare_inst;
use super::{BodyLowerer, Out, Typed, unsupported};
use crate::analysis::thir::{self, Builtin, Callee, CastTarget, CompareOp, ExprKind};
use crate::analysis::ty::Ty;
use crate::ir::{
    self, BitOp, CompareOp as IrCompare, Constant, InstKind, IntBinaryOp, IntUnaryOp, IntValue, RatOp, StrOp, Target,
    TermKind, TrapCode, Type, ValueId,
};
use crate::lowering::{LoweringError, Result};
use crate::source::Span;

/// Вычисленный операнд, граница которого ещё не применена.
enum Pending<'r> {
    Value(Typed, Option<&'r thir::Expr>),
    /// Variadic-хвост: значения элементов и распаковка.
    Pack {
        items: Vec<(Typed, Option<&'r thir::Expr>)>,
        spread: Option<Typed>,
        expr: &'r thir::Expr,
    },
}

impl<'r> BodyLowerer<'_, 'r, '_> {
    /// Значение операнда без его внешней границы (`Coerce`).
    fn operand(&mut self, expr: &'r thir::Expr) -> Result<Option<(Typed, Option<&'r thir::Expr>)>> {
        match &expr.kind {
            ExprKind::Coerce { value, .. } => {
                Ok(self.value(value)?.map(|typed| (typed, Some(expr))))
            }
            _ => Ok(self.value(expr)?.map(|typed| (typed, None))),
        }
    }

    /// Применить отложенную границу операнда.
    fn bound(&mut self, typed: Typed, coercion: Option<&'r thir::Expr>) -> Result<Typed> {
        let Some(expr) = coercion else {
            return Ok(typed);
        };
        let ExprKind::Coerce { coercion, .. } = &expr.kind else {
            return Ok(typed);
        };
        let target = self.expr_type(expr)?;
        let span = expr.span;
        Ok(match coercion {
            thir::Coercion::Take => self.take(typed, span),
            thir::Coercion::Retag => match &target {
                Type::Map(..) => self.emit(InstKind::MapRetag(typed.value), target, span),
                _ => self.emit(InstKind::Retag(typed.value), target, span),
            },
            thir::Coercion::RoleWrap => {
                let ExprKind::Coerce { value, .. } = &expr.kind else {
                    unreachable!("matched above");
                };
                self.role_wrap(typed, value.ty.as_ref(), expr.ty.as_ref(), target, span)?
            }
            thir::Coercion::RoleNarrow { message } => self.emit(
                InstKind::RoleNarrow {
                    value: typed.value,
                    message: message.clone(),
                },
                target,
                span,
            ),
            thir::Coercion::Box => {
                let typed = self.take(typed, span);
                self.emit(InstKind::AnyBox(typed.value), Type::Any, span)
            }
        })
    }

    /// Аргументы вызова: значения слева направо, затем границы.
    pub(super) fn arguments(
        &mut self,
        args: &'r [thir::Expr],
        span: Span,
    ) -> Result<Option<Vec<ir::ValueId>>> {
        let mut pending = Vec::new();
        for arg in args {
            match &arg.kind {
                ExprKind::Pack { items, spread } => {
                    let mut values = Vec::new();
                    for item in items {
                        let Some(value) = self.operand(item)? else {
                            return Ok(None);
                        };
                        values.push(value);
                    }
                    let spread = match spread {
                        Some(spread) => match self.value(spread)? {
                            Some(value) => Some(self.take(value, spread.span)),
                            None => return Ok(None),
                        },
                        None => None,
                    };
                    pending.push(Pending::Pack {
                        items: values,
                        spread,
                        expr: arg,
                    });
                }
                _ => {
                    let Some((value, coercion)) = self.operand(arg)? else {
                        return Ok(None);
                    };
                    pending.push(Pending::Value(value, coercion));
                }
            }
        }
        let mut values = Vec::new();
        for item in pending {
            match item {
                Pending::Value(value, coercion) => values.push(self.bound(value, coercion)?.value),
                Pending::Pack {
                    items,
                    spread,
                    expr,
                } => {
                    let array_type = self.expr_type(expr)?;
                    let mut elements = Vec::new();
                    for (value, coercion) in items {
                        elements.push(self.bound(value, coercion)?.value);
                    }
                    values.push(self.build_pack(elements, spread, array_type, span).value);
                }
            }
        }
        Ok(Some(values))
    }

    /// Массив variadic-хвоста: `array.new` или сборщик с распаковкой.
    fn build_pack(
        &mut self,
        items: Vec<ir::ValueId>,
        spread: Option<Typed>,
        array_type: Type,
        span: Span,
    ) -> Typed {
        let Some(array) = spread else {
            return self.emit(InstKind::ArrayNew(items), array_type, span);
        };
        let Type::Array(element) = &array_type else {
            unreachable!("a variadic tail is an array")
        };
        let builder_type = Type::Builder(element.clone());
        let mut builder = self.emit(InstKind::BuilderNew, builder_type.clone(), span);
        for item in items {
            builder = self.emit(
                InstKind::BuilderPush {
                    builder: builder.value,
                    value: item,
                },
                builder_type.clone(),
                span,
            );
        }
        builder = self.emit(
            InstKind::BuilderAppend {
                builder: builder.value,
                array: array.value,
            },
            builder_type,
            span,
        );
        self.emit(InstKind::BuilderFinish(builder.value), array_type, span)
    }

    /// Variadic-хвост вне вызова (не встречается: хвост — аргумент).
    pub(super) fn pack(
        &mut self,
        items: &'r [thir::Expr],
        spread: Option<&'r thir::Expr>,
        expr: &'r thir::Expr,
        span: Span,
    ) -> Result<Out> {
        let mut elements = Vec::new();
        for item in items {
            let Some(value) = self.value(item)? else {
                return Ok(Out::Diverged);
            };
            elements.push(value.value);
        }
        let spread = match spread {
            Some(spread) => match self.value(spread)? {
                Some(value) => Some(value),
                None => return Ok(Out::Diverged),
            },
            None => None,
        };
        let array_type = self.expr_type(expr)?;
        Ok(Out::Value(
            self.build_pack(elements, spread, array_type, span),
        ))
    }

    pub(super) fn finish_call(
        &mut self,
        kind: InstKind,
        expr: &thir::Expr,
        span: Span,
    ) -> Result<Out> {
        match &expr.ty {
            None => {
                self.b.emit_void(kind, span);
                Ok(Out::Unit)
            }
            Some(_) => {
                let ty = self.expr_type(expr)?;
                Ok(Out::Value(self.emit(kind, ty, span)))
            }
        }
    }

    pub(super) fn call(
        &mut self,
        callee: &'r Callee,
        args: &'r [thir::Expr],
        expr: &'r thir::Expr,
        span: Span,
    ) -> Result<Out> {
        match callee {
            Callee::Method {
                receiver, method, ..
            } => {
                let Some(target) = self.value(receiver)? else {
                    return Ok(Out::Diverged);
                };
                let Type::Object(object) = target.ty.taken() else {
                    return Err(LoweringError::invalid(
                        span,
                        "internal compiler error: a method receiver is not an object",
                    ));
                };
                let target = self.take(target, receiver.span);
                if self.ctx.is_intrinsic(object, *method) {
                    let name = self.ctx.objects[object.0].name.clone();
                    return Err(unsupported(span, &format!("VM intrinsics of `{name}`")));
                }
                let func = self.ctx.method_func(object, *method, span)?;
                let Some(values) = self.arguments(args, span)? else {
                    return Ok(Out::Diverged);
                };
                let mut call_args = vec![target.value];
                call_args.extend(values);
                self.finish_call(
                    InstKind::Call {
                        func,
                        args: call_args,
                    },
                    expr,
                    span,
                )
            }
            Callee::Role { receiver, slot, .. } => {
                let Some(target) = self.value(receiver)? else {
                    return Ok(Out::Diverged);
                };
                let Some(values) = self.arguments(args, span)? else {
                    return Ok(Out::Diverged);
                };
                self.finish_call(
                    InstKind::CallRole {
                        receiver: target.value,
                        slot: *slot,
                        args: values,
                    },
                    expr,
                    span,
                )
            }
            Callee::Ctor {
                object,
                type_args,
                ctor,
                given,
            } => {
                let type_args: Vec<Ty> = type_args.iter().map(|arg| arg.subst(&self.env)).collect();
                let instance = self.ctx.object_instance(*object, type_args, span)?;
                let Some(values) = self.arguments(args, span)? else {
                    return Ok(Out::Diverged);
                };
                let func = self.ctx.ctor_func(instance, *ctor, given.clone());
                Ok(Out::Value(self.emit(
                    InstKind::Call { func, args: values },
                    Type::Object(instance),
                    span,
                )))
            }
            Callee::Native(native) => {
                let func = self.ctx.native_func(*native, span)?;
                let Some(values) = self.arguments(args, span)? else {
                    return Ok(Out::Diverged);
                };
                self.finish_call(InstKind::Call { func, args: values }, expr, span)
            }
            Callee::Builtin { receiver, op } => self.builtin(receiver, op, args, expr, span),
            Callee::Test(name) => {
                let Some(values) = self.arguments(args, span)? else {
                    return Ok(Out::Diverged);
                };
                let types = values
                    .iter()
                    .map(|value| self.b.value_type(*value).clone())
                    .collect();
                let func = self.ctx.runtime_intrinsic(
                    &format!("okkama.test.{name}"),
                    types,
                    ir::ResultType::Void,
                );
                self.b
                    .emit_void(InstKind::Call { func, args: values }, span);
                Ok(Out::Unit)
            }
            Callee::FakeStorage(op) => {
                let (Some(receiver), Some(object)) = (self.self_value, self.object) else {
                    return Err(LoweringError::invalid(
                        span,
                        "internal compiler error: fake.storage outside a fake action",
                    ));
                };
                let Some(values) = self.arguments(args, span)? else {
                    return Ok(Out::Diverged);
                };
                let mut types = vec![Type::Object(object)];
                types.extend(values.iter().map(|value| self.b.value_type(*value).clone()));
                let result = match &expr.ty {
                    Some(_) => ir::ResultType::Value(self.expr_type(expr)?),
                    None => ir::ResultType::Void,
                };
                let func =
                    self.ctx
                        .runtime_intrinsic(&format!("okkama.fake.storage.{op}"), types, result);
                let mut call_args = vec![receiver];
                call_args.extend(values);
                self.finish_call(
                    InstKind::Call {
                        func,
                        args: call_args,
                    },
                    expr,
                    span,
                )
            }
        }
    }

    // ----- builtins -----

    fn builtin(
        &mut self,
        receiver: &'r thir::Expr,
        op: &Builtin,
        args: &'r [thir::Expr],
        expr: &'r thir::Expr,
        span: Span,
    ) -> Result<Out> {
        let Some(target) = self.value(receiver)? else {
            return Ok(Out::Diverged);
        };
        let target = self.take(target, receiver.span);
        let mut operands = Vec::new();
        for arg in args {
            let Some(value) = self.value(arg)? else {
                return Ok(Out::Diverged);
            };
            operands.push(self.take(value, arg.span));
        }
        let ids: Vec<ir::ValueId> = operands.iter().map(|value| value.value).collect();
        let receiver_id = target.value;
        let (kind, ty) = match op {
            Builtin::ArraySize => (InstKind::ArrayLen(receiver_id), Type::Int),
            Builtin::ArraySlice => (
                InstKind::ArraySlice {
                    array: receiver_id,
                    start: ids[0],
                    end: ids[1],
                },
                target.ty.clone(),
            ),
            Builtin::ArrayJoin => (
                InstKind::ArrayJoin {
                    array: receiver_id,
                    separator: ids[0],
                },
                Type::Str,
            ),
            Builtin::ArrayItem => {
                let element = self.expr_type(expr)?;
                (
                    InstKind::ArrayGet {
                        array: receiver_id,
                        index: ids[0],
                    },
                    element,
                )
            }
            Builtin::ArrayFirst | Builtin::ArrayLast => {
                let first = *op == Builtin::ArrayFirst;
                let index = if first {
                    self.constant(Constant::Int(IntValue::from_i64(0)), span)
                        .value
                } else {
                    let size = self.emit(InstKind::ArrayLen(receiver_id), Type::Int, span);
                    let one = self.constant(Constant::Int(IntValue::from_i64(1)), span);
                    self.emit(
                        InstKind::IntBinary {
                            op: IntBinaryOp::Sub,
                            left: size.value,
                            right: one.value,
                        },
                        Type::Int,
                        span,
                    )
                    .value
                };
                // Пустой массив — свой текст ошибки, как в Perl-интерпретаторе.
                let element = self.expr_type(expr)?;
                let (found, found_params) = self.block_with(&[], std::slice::from_ref(&element));
                let missing = self.b.new_block();
                self.b.terminate(
                    TermKind::ArrayLookup {
                        array: receiver_id,
                        index,
                        found: Target::new(found),
                        missing: Target::new(missing),
                    },
                    span,
                );
                self.b.switch_to(missing);
                self.sync_unwind();
                let text = if first { "first" } else { "last" };
                self.b.terminate(
                    TermKind::Trap {
                        code: TrapCode::Empty,
                        message: format!("Cannot read {text} item of an empty sequence"),
                    },
                    span,
                );
                self.b.switch_to(found);
                self.sync_unwind();
                return Ok(Out::Value(Typed {
                    value: found_params[0],
                    ty: element,
                }));
            }
            Builtin::StrLength => (str_op(StrOp::Len, receiver_id, &ids), Type::Int),
            Builtin::StrEmpty => {
                let length = self.emit(str_op(StrOp::Len, receiver_id, &ids), Type::Int, span);
                let zero = self.constant(Constant::Int(IntValue::from_i64(0)), span);
                (
                    InstKind::IntCompare {
                        op: ir::CompareOp::Eq,
                        left: length.value,
                        right: zero.value,
                    },
                    Type::Bool,
                )
            }
            Builtin::StrSlice => (str_op(StrOp::Slice, receiver_id, &ids), Type::Str),
            Builtin::StrSplit => (
                str_op(StrOp::Split, receiver_id, &ids),
                Type::Array(Box::new(Type::Str)),
            ),
            Builtin::StrReplace => (str_op(StrOp::Replace, receiver_id, &ids), Type::Str),
            Builtin::StrReplaceFirst => (str_op(StrOp::ReplaceFirst, receiver_id, &ids), Type::Str),
            Builtin::StrTrim => (str_op(StrOp::Trim, receiver_id, &ids), Type::Str),
            Builtin::StrChomp => (str_op(StrOp::Chomp, receiver_id, &ids), Type::Str),
            // Searches give an index or -1 (#210); `contains` is a found index.
            Builtin::StrContains => {
                let index = self.emit(str_op(StrOp::Find, receiver_id, &ids), Type::Int, span);
                let zero = self.constant(Constant::Int(IntValue::from_i64(0)), span);
                (
                    InstKind::IntCompare { op: IrCompare::Ge, left: index.value, right: zero.value },
                    Type::Bool,
                )
            }
            Builtin::StrFind | Builtin::StrFindLast => {
                let op = if *op == Builtin::StrFind { StrOp::Find } else { StrOp::FindLast };
                let index = self.emit(str_op(op, receiver_id, &ids), Type::Int, span);
                return self.maybe_index(index.value, expr, span);
            }
            Builtin::StrStartsWith => (str_op(StrOp::StartsWith, receiver_id, &ids), Type::Bool),
            Builtin::StrEndsWith => (str_op(StrOp::EndsWith, receiver_id, &ids), Type::Bool),
            Builtin::StrTrimStart => (str_op(StrOp::TrimStart, receiver_id, &ids), Type::Str),
            Builtin::StrTrimEnd => (str_op(StrOp::TrimEnd, receiver_id, &ids), Type::Str),
            Builtin::StrLower => (str_op(StrOp::Lower, receiver_id, &ids), Type::Str),
            Builtin::StrUpper => (str_op(StrOp::Upper, receiver_id, &ids), Type::Str),
            Builtin::StrRepeat => (str_op(StrOp::Repeat, receiver_id, &ids), Type::Str),
            Builtin::StrCharacters => (
                str_op(StrOp::Characters, receiver_id, &ids),
                Type::Array(Box::new(Type::Char)),
            ),
            Builtin::CharCode => (InstKind::Cvt(receiver_id), Type::Int),
            Builtin::ArrayContains(kind) => {
                let index = self.array_search(&target, ids[0], *kind, span);
                let zero = self.constant(Constant::Int(IntValue::from_i64(0)), span);
                (
                    InstKind::IntCompare { op: IrCompare::Ge, left: index, right: zero.value },
                    Type::Bool,
                )
            }
            Builtin::ArrayIndexOf(kind) => {
                let index = self.array_search(&target, ids[0], *kind, span);
                return self.maybe_index(index, expr, span);
            }
            Builtin::ArrayReversed => return Ok(Out::Value(self.array_reversed(&target, span))),
            Builtin::ArrayTake | Builtin::ArrayDrop => {
                let drop = *op == Builtin::ArrayDrop;
                return Ok(Out::Value(self.array_take(&target, ids[0], drop, span)));
            }
            // Сравнение строк по длине, как в Perl-интерпретаторе.
            Builtin::StrCompare(compare) => {
                let left = self.emit(str_op(StrOp::Len, receiver_id, &[]), Type::Int, span);
                let right = self.emit(str_op(StrOp::Len, ids[0], &[]), Type::Int, span);
                (
                    InstKind::IntCompare {
                        op: ir_compare(*compare),
                        left: left.value,
                        right: right.value,
                    },
                    Type::Bool,
                )
            }
            Builtin::MapSize => (InstKind::MapLen(receiver_id), Type::Int),
            Builtin::MapHas => match &target.ty {
                // У `[:]` нет ключей: `has` — всегда `false` (ключ уже вычислен).
                Type::Map(key, _) if **key == Type::Never => {
                    (InstKind::Const(Constant::Bool(false)), Type::Bool)
                }
                _ => (
                    InstKind::MapHas {
                        map: receiver_id,
                        key: ids[0],
                    },
                    Type::Bool,
                ),
            },
            Builtin::MapKeys => {
                let ty = self.expr_type(expr)?;
                (InstKind::MapKeys(receiver_id), ty)
            }
            Builtin::MapValues => {
                let ty = self.expr_type(expr)?;
                (InstKind::MapValues(receiver_id), ty)
            }
            Builtin::Compare(kind, compare) => (
                compare_inst(*kind, ir_compare(*compare), receiver_id, ids[0]),
                Type::Bool,
            ),
            Builtin::Abs => match target.ty {
                Type::Int => (InstKind::IntAbs(receiver_id), Type::Int),
                Type::Float => (
                    InstKind::Float {
                        op: ir::FloatOp::Abs,
                        args: vec![receiver_id],
                    },
                    Type::Float,
                ),
                _ => (
                    InstKind::Rat {
                        op: ir::RatOp::Abs,
                        args: vec![receiver_id],
                    },
                    Type::Rat,
                ),
            },
            Builtin::Sign => match target.ty {
                Type::Int => (InstKind::IntSign(receiver_id), Type::Int),
                Type::Float => (InstKind::FloatSign(receiver_id), Type::Int),
                _ => (InstKind::RatSign(receiver_id), Type::Int),
            },
            Builtin::Sqrt => {
                let value = match target.ty {
                    Type::Int => {
                        self.emit(InstKind::Cvt(receiver_id), Type::Float, span)
                            .value
                    }
                    _ => receiver_id,
                };
                (
                    InstKind::Float {
                        op: ir::FloatOp::Sqrt,
                        args: vec![value],
                    },
                    Type::Float,
                )
            }
            Builtin::Pow if matches!(target.ty, Type::Int | Type::Rat) => {
                return Ok(Out::Value(self.exact_power(receiver_id, target.ty.clone(), ids[0], span)));
            }
            Builtin::Pow => (
                InstKind::Float {
                    op: ir::FloatOp::Pow,
                    args: vec![receiver_id, ids[0]],
                },
                Type::Float,
            ),
            Builtin::Bits { name, byte } => {
                let op = BitOp::from_action(name).ok_or_else(|| {
                    LoweringError::invalid(
                        span,
                        format!("internal compiler error: bit action {name}"),
                    )
                })?;
                let count = op.rotates().then(|| ids[0]);
                let ty = if op.counts() {
                    Type::Int
                } else {
                    target.ty.clone()
                };
                (
                    InstKind::Bits {
                        op,
                        byte: *byte,
                        value: receiver_id,
                        count,
                    },
                    ty,
                )
            }
            Builtin::Convert(cast) => {
                return Ok(Out::Value(self.convert(target, *cast, span, None)?));
            }
            Builtin::ArrayCursor => {
                // `xs.cursor()` — адаптер `ArrayCursor<E, E>` с позиции 0.
                let element = match receiver.ty.as_ref().map(|ty| ty.subst(&self.env)) {
                    Some(Ty::Array(element)) => *element,
                    _ => return Err(unsupported(span, "array cursors and orderings")),
                };
                let Some(decl) = self.ctx.reg.adapters.array_cursor else {
                    return Err(unsupported(span, "array cursors and orderings"));
                };
                let cursor =
                    self.ctx
                        .object_instance(decl, vec![element.clone(), element], span)?;
                let items = self.ctx.objects[cursor.0].fields[0].1.clone();
                let items = if target.ty == items {
                    receiver_id
                } else {
                    self.emit(InstKind::Retag(receiver_id), items, span).value
                };
                let zero = self
                    .constant(Constant::Int(IntValue::from_i64(0)), span)
                    .value;
                let ctor = self.ctx.adapter_ctor(cursor);
                let value = self.emit(
                    InstKind::Call {
                        func: ctor,
                        args: vec![items, zero],
                    },
                    Type::Object(cursor),
                    span,
                );
                let ty = self.expr_type(expr)?;
                return Ok(Out::Value(self.emit(
                    InstKind::RoleWrap(value.value),
                    ty,
                    span,
                )));
            }
            Builtin::ArrayOrdering => {
                // Устойчивая сортировка по `order()` элементов — в рантайме.
                let ty = target.ty.clone();
                let func = self.ctx.runtime_intrinsic(
                    "okkama.array.ordering",
                    vec![ty.clone()],
                    ir::ResultType::Value(ty.clone()),
                );
                return Ok(Out::Value(self.emit(
                    InstKind::Call {
                        func,
                        args: vec![receiver_id],
                    },
                    ty,
                    span,
                )));
            }
            Builtin::Order(kind) => {
                let Type::Enum(order) = self.expr_type(expr)? else {
                    return Err(LoweringError::invalid(
                        span,
                        "internal compiler error: order() result is not compare.Order",
                    ));
                };
                return self.order(*kind, receiver_id, ids[0], order, span);
            }
            Builtin::Numerator | Builtin::Denominator => (
                InstKind::RatPart {
                    value: receiver_id,
                    denominator: *op == Builtin::Denominator,
                },
                Type::Str,
            ),
            Builtin::FutureCancel => {
                self.b.emit_void(
                    InstKind::Fut {
                        op: ir::FutOp::Cancel,
                        future: receiver_id,
                    },
                    span,
                );
                return Ok(Out::Unit);
            }
            Builtin::FutureDone | Builtin::FutureCancelled => (
                InstKind::Fut {
                    op: if *op == Builtin::FutureDone {
                        ir::FutOp::Done
                    } else {
                        ir::FutOp::Cancelled
                    },
                    future: receiver_id,
                },
                Type::Bool,
            ),
        };
        Ok(Out::Value(self.emit(kind, ty, span)))
    }

    /// Явное приведение одного значения (`T(x)`, `x.T()`, элемент `@T(...)`).
    pub(super) fn convert(
        &mut self,
        value: Typed,
        target: CastTarget,
        span: Span,
        position: Option<usize>,
    ) -> Result<Typed> {
        let value = self.take(value, span);
        Ok(match (target, &value.ty) {
            (CastTarget::Int, Type::Int)
            | (CastTarget::Str, Type::Str)
            | (CastTarget::Byte, Type::Byte)
            | (CastTarget::Float, Type::Float)
            | (CastTarget::Rat, Type::Rat)
            | (CastTarget::Char, Type::Char) => value,
            (
                CastTarget::Str,
                Type::Int | Type::Bool | Type::Byte | Type::Float | Type::Rat | Type::Char,
            ) => self.emit(InstKind::Cvt(value.value), Type::Str, span),
            // A code point outside the Unicode scalar values fails at run time (#231).
            (CastTarget::Char, Type::Int) => self.emit(InstKind::Cvt(value.value), Type::Char, span),
            (CastTarget::Float, Type::Int | Type::Byte | Type::Rat) => {
                self.emit(InstKind::Cvt(value.value), Type::Float, span)
            }
            (CastTarget::Int, Type::Float | Type::Byte | Type::Rat) => {
                self.emit(InstKind::Cvt(value.value), Type::Int, span)
            }
            (CastTarget::Rat, Type::Int | Type::Byte | Type::Float) => {
                self.emit(InstKind::Cvt(value.value), Type::Rat, span)
            }
            // `int/float/rat → byte` проверяет диапазон 0..255 во время исполнения.
            (CastTarget::Byte, Type::Int | Type::Float | Type::Rat) => self.emit(
                InstKind::ByteFrom {
                    value: value.value,
                    context: position.map(|position| format!("@byte() argument {position}")),
                },
                Type::Byte,
                span,
            ),
            _ => return Err(unsupported(span, "`rat` values")),
        })
    }
}

fn str_op(op: StrOp, receiver: ir::ValueId, rest: &[ir::ValueId]) -> InstKind {
    let mut args = vec![receiver];
    args.extend_from_slice(rest);
    InstKind::Str { op, args }
}

pub(super) fn ir_compare(op: CompareOp) -> ir::CompareOp {
    match op {
        CompareOp::Eq => ir::CompareOp::Eq,
        CompareOp::Ne => ir::CompareOp::Ne,
        CompareOp::Lt => ir::CompareOp::Lt,
        CompareOp::Le => ir::CompareOp::Le,
        CompareOp::Gt => ir::CompareOp::Gt,
        CompareOp::Ge => ir::CompareOp::Ge,
    }
}

impl BodyLowerer<'_, '_, '_> {
    /// `base.pow(exponent)` exactly (#228), by squaring: an `int` base gives
    /// an `int` (a negative exponent is an error), a `rat` base a `rat` (a
    /// negative exponent inverts the result). Products are the ordinary exact
    /// multiplications, measured against the memory budget like any other.
    fn exact_power(&mut self, base: ValueId, ty: Type, exponent: ValueId, span: Span) -> Typed {
        let int = |value: i64| Constant::Int(IntValue::from_i64(value));
        let zero = self.constant(int(0), span).value;
        let one_int = self.constant(int(1), span).value;
        let negative = self.emit(InstKind::IntCompare { op: IrCompare::Lt, left: exponent, right: zero }, Type::Bool, span).value;
        let rat = ty == Type::Rat;
        // The exponent to square with: |exponent| for a rat, exponent after
        // the sign check for an int.
        let (start, start_params) = self.block_with(&[], &[Type::Int]);
        let flip = self.b.new_block();
        let keep = self.b.new_block();
        self.b.terminate(
            TermKind::Cond { condition: negative, then_target: Target::new(flip), else_target: Target::new(keep) },
            span,
        );
        self.b.switch_to(flip);
        self.sync_unwind();
        if rat {
            let positive = self.emit(InstKind::IntUnary { op: IntUnaryOp::Neg, operand: exponent }, Type::Int, span).value;
            self.b.terminate(TermKind::Br(Target { block: start, args: vec![positive] }), span);
        } else {
            self.b.terminate(
                TermKind::Trap {
                    code: TrapCode::Range,
                    message: "int.pow needs a non-negative exponent; a rat base takes a negative one".to_string(),
                },
                span,
            );
        }
        self.b.switch_to(keep);
        self.sync_unwind();
        self.b.terminate(TermKind::Br(Target { block: start, args: vec![exponent] }), span);
        self.b.switch_to(start);
        self.sync_unwind();
        let count = start_params[0];
        let one = if rat {
            self.emit(InstKind::Rat { op: RatOp::New, args: vec![one_int, one_int] }, Type::Rat, span).value
        } else {
            one_int
        };
        let multiply = |left: ValueId, right: ValueId| {
            if rat {
                InstKind::Rat { op: RatOp::Mul, args: vec![left, right] }
            } else {
                InstKind::IntBinary { op: IntBinaryOp::Mul, left, right }
            }
        };
        // head(result, square, count): while count > 0, multiply in the
        // square for each set bit of count.
        let (head, head_params) = self.block_with(&[], &[ty.clone(), ty.clone(), Type::Int]);
        let (done, done_params) = self.block_with(&[], std::slice::from_ref(&ty));
        let (join, join_params) = self.block_with(&[], std::slice::from_ref(&ty));
        self.b.terminate(TermKind::Br(Target { block: head, args: vec![one, base, count] }), span);
        self.b.switch_to(head);
        self.sync_unwind();
        let (result, square, left) = (head_params[0], head_params[1], head_params[2]);
        let more = self.emit(InstKind::IntCompare { op: IrCompare::Gt, left, right: zero }, Type::Bool, span).value;
        let step = self.b.new_block();
        self.b.terminate(
            TermKind::Cond {
                condition: more,
                then_target: Target::new(step),
                else_target: Target { block: done, args: vec![result] },
            },
            span,
        );
        self.b.switch_to(step);
        self.sync_unwind();
        let bit = self.emit(InstKind::IntBinary { op: IntBinaryOp::And, left, right: one_int }, Type::Int, span).value;
        let odd = self.emit(InstKind::IntCompare { op: IrCompare::Ne, left: bit, right: zero }, Type::Bool, span).value;
        let times = self.b.new_block();
        self.b.terminate(
            TermKind::Cond {
                condition: odd,
                then_target: Target::new(times),
                else_target: Target { block: join, args: vec![result] },
            },
            span,
        );
        self.b.switch_to(times);
        self.sync_unwind();
        let multiplied = self.emit(multiply(result, square), ty.clone(), span).value;
        self.b.terminate(TermKind::Br(Target { block: join, args: vec![multiplied] }), span);
        self.b.switch_to(join);
        self.sync_unwind();
        let squared = self.emit(multiply(square, square), ty.clone(), span).value;
        let halved = self.emit(InstKind::IntBinary { op: IntBinaryOp::Shr, left, right: one_int }, Type::Int, span).value;
        self.b.terminate(TermKind::Br(Target { block: head, args: vec![join_params[0], squared, halved] }), span);
        self.b.switch_to(done);
        self.sync_unwind();
        let power = done_params[0];
        if !rat {
            return Typed { value: power, ty };
        }
        // A negative exponent: the reciprocal (0 to a negative power fails
        // as a division by zero).
        let (out, out_params) = self.block_with(&[], &[Type::Rat]);
        let invert = self.b.new_block();
        self.b.terminate(
            TermKind::Cond {
                condition: negative,
                then_target: Target::new(invert),
                else_target: Target { block: out, args: vec![power] },
            },
            span,
        );
        self.b.switch_to(invert);
        self.sync_unwind();
        let inverted = self.emit(InstKind::Rat { op: RatOp::Div, args: vec![one, power] }, Type::Rat, span).value;
        self.b.terminate(TermKind::Br(Target { block: out, args: vec![inverted] }), span);
        self.b.switch_to(out);
        self.sync_unwind();
        Typed { value: out_params[0], ty: Type::Rat }
    }
}
