//! Адаптеры встроенных значений к стандартным ролям (§9.9, §12): массив как
//! `Sequencing<T>`/`Iterating<T>`/`Sizing`/`ByteSource`, скаляр как
//! `Comparing<T>`. Это системные объекты анализа (`Adapters`) без исходника:
//! их конструктор и actions строятся здесь прямо в OIR. Тексты ошибок —
//! Perl-интерпретатора (`BuiltinActions.pm`). Элемент массива `S` пересекает
//! границу элемента роли `T` при чтении (`role.wrap`, если `T` — роль).

use super::super::builder::FnBuilder;
use super::super::ctx::Ctx;
use super::super::{LoweringError, Result};
use super::matching::compare_inst;
use crate::analysis::thir::CompareKind;
use crate::analysis::ty::Ty;
use crate::ir::{
    self, Attr, CompareOp, Constant, InstKind, IntBinaryOp, IntValue, ObjectId, RatOp, ResultType, StrOp,
    Target, TermKind, TrapCode, Type, ValueId,
};
use crate::source::Span;

/// Тело action адаптера.
pub(super) fn lower_method(
    ctx: &mut Ctx<'_, '_>,
    symbol: String,
    object: ObjectId,
    method: usize,
) -> Result<ir::Function> {
    let instance = &ctx.objects[object.0];
    let decl = instance.decl;
    let span = instance.span;
    let env = instance.env.clone();
    let reg = ctx.reg;
    let info = &reg.objects[decl.0].methods[method];
    let result = ctx.ir_result(&info.result, &env, span)?;
    let ResultType::Value(result_ty) = result.clone() else {
        return Err(internal(span, "adapter action without a result"));
    };
    let mut params = Vec::new();
    for (_, ty) in &info.params {
        params.push(ctx.ir_type(&ty.subst(&env), span)?);
    }
    let mut a = Adapter {
        b: FnBuilder::new(),
        object,
        receiver: ValueId(0),
        span,
    };
    a.receiver = a.b.new_value(Type::Object(object));
    let mut values = vec![a.receiver];
    for ty in &params {
        values.push(a.b.new_value(ty.clone()));
    }
    let kind = AdapterKind::of(ctx, decl);
    match (kind, info.name.as_str()) {
        (AdapterKind::Sequence, "size") => {
            let items = a.field(ctx, 0);
            let size = a.emit(InstKind::ArrayLen(items), Type::Int);
            a.ret(size);
        }
        (AdapterKind::Sequence, "item") => {
            let items = a.field(ctx, 0);
            let item = a.get(ctx, items, values[1]);
            let item = a.cross(item, &result_ty)?;
            a.ret(item);
        }
        (AdapterKind::Sequence, name @ ("first" | "last")) => {
            let items = a.field(ctx, 0);
            let index = if name == "first" {
                a.int(0)
            } else {
                let size = a.emit(InstKind::ArrayLen(items), Type::Int);
                let one = a.int(1);
                a.emit(
                    InstKind::IntBinary {
                        op: IntBinaryOp::Sub,
                        left: size,
                        right: one,
                    },
                    Type::Int,
                )
            };
            let message = format!("Cannot read {name} item of an empty sequence");
            let item = a.lookup(ctx, items, index, message);
            let item = a.cross(item, &result_ty)?;
            a.ret(item);
        }
        (AdapterKind::Sequence, "cursor") => {
            let items = a.field(ctx, 0);
            let start = a.int(0);
            let cursor = a.cursor(ctx, items, start, &result_ty)?;
            a.ret(cursor);
        }
        (AdapterKind::Cursor, "available") => {
            let items = a.field(ctx, 0);
            let at = a.field(ctx, 1);
            let size = a.emit(InstKind::ArrayLen(items), Type::Int);
            let available = a.emit(
                compare_inst(CompareKind::Int, CompareOp::Lt, at, size),
                Type::Bool,
            );
            a.ret(available);
        }
        (AdapterKind::Cursor, "item") => {
            let items = a.field(ctx, 0);
            let at = a.field(ctx, 1);
            let item = a.lookup(ctx, items, at, "Array cursor is exhausted".to_string());
            let item = a.cross(item, &result_ty)?;
            a.ret(item);
        }
        (AdapterKind::Cursor, "successor") => {
            let items = a.field(ctx, 0);
            let at = a.field(ctx, 1);
            // Исчерпанный курсор не имеет преемника: сначала та же проверка, что у item().
            a.lookup(ctx, items, at, "Array cursor is exhausted".to_string());
            let one = a.int(1);
            let next = a.emit(
                InstKind::IntBinary {
                    op: IntBinaryOp::Add,
                    left: at,
                    right: one,
                },
                Type::Int,
            );
            let cursor = a.cursor(ctx, items, next, &result_ty)?;
            a.ret(cursor);
        }
        (AdapterKind::Order, "order") => {
            let value = a.field(ctx, 0);
            let kind = scalar_kind(&params[0])
                .ok_or_else(|| internal(span, "ScalarOrder over a non-ordered type"))?;
            let Type::Enum(order) = result_ty else {
                return Err(internal(span, "order() result is not compare.Order"));
            };
            // `less`, иначе `equal`, иначе `greater` (порядок случаев `enum Order`).
            for (case, op) in [(0, CompareOp::Lt), (1, CompareOp::Eq)] {
                let hit = a.emit(compare_inst(kind, op, value, values[1]), Type::Bool);
                let yes = a.b.new_block();
                let no = a.b.new_block();
                a.b.terminate(
                    TermKind::Cond {
                        condition: hit,
                        then_target: Target::new(yes),
                        else_target: Target::new(no),
                    },
                    span,
                );
                a.b.switch_to(yes);
                let found = a.emit(InstKind::EnumCase { ty: order, case }, Type::Enum(order));
                a.ret(found);
                a.b.switch_to(no);
            }
            let greater = a.emit(InstKind::EnumCase { ty: order, case: 2 }, Type::Enum(order));
            a.ret(greater);
        }
        (AdapterKind::Bytes, "bytes") => {
            let items = a.field(ctx, 0);
            a.ret(items);
        }
        (AdapterKind::Text, "bytes") => {
            let value = a.field(ctx, 0);
            let bytes = a.emit(
                InstKind::Str {
                    op: StrOp::Utf8,
                    args: vec![value],
                },
                Type::Array(Box::new(Type::Byte)),
            );
            a.ret(bytes);
        }
        (_, name) => {
            return Err(internal(span, &format!("adapter has no action {name}")));
        }
    }
    let (values_ty, blocks, params) =
        a.b.finish(&values)
            .map_err(|message| internal(span, &message))?;
    Ok(ir::Function {
        symbol,
        params,
        result,
        attrs: vec![Attr::Method(object)],
        values: values_ty,
        blocks,
        span,
    })
}

/// Конструктор адаптера: `obj.new` из параметров-полей.
pub(super) fn lower_new(
    ctx: &mut Ctx<'_, '_>,
    symbol: String,
    object: ObjectId,
) -> Result<ir::Function> {
    let span = ctx.objects[object.0].span;
    let mut b = FnBuilder::new();
    let fields: Vec<ValueId> = ctx.objects[object.0]
        .fields
        .iter()
        .map(|(_, ty, _)| b.new_value(ty.clone()))
        .collect();
    let value = b.emit(
        InstKind::ObjNew {
            object,
            fields: fields.clone(),
        },
        Type::Object(object),
        span,
    );
    b.terminate(TermKind::Ret(Some(value)), span);
    let (values, blocks, params) = b
        .finish(&fields)
        .map_err(|message| internal(span, &message))?;
    Ok(ir::Function {
        symbol,
        params,
        result: ResultType::Value(Type::Object(object)),
        attrs: vec![Attr::Ctor(object)],
        values,
        blocks,
        span,
    })
}

pub(super) fn lower_variant_get(
    ctx: &mut Ctx<'_, '_>,
    symbol: String,
    object: ObjectId,
    field: usize,
) -> Result<ir::Function> {
    let span = ctx.objects[object.0].span;
    let ty = ctx.objects[object.0].fields[field].1.clone();
    let mut b = FnBuilder::new();
    let receiver = b.new_value(Type::Object(object));
    if let Some(case) = ctx.variant_field_case(object, field) {
        let tag = b.emit(
            InstKind::ObjGet {
                object: receiver,
                field: 0,
            },
            Type::Int,
            span,
        );
        let expected = b.emit(
            InstKind::Const(Constant::Int(IntValue::parse(&case.to_string()).unwrap())),
            Type::Int,
            span,
        );
        let matches = b.emit(
            InstKind::IntCompare {
                op: CompareOp::Eq,
                left: tag,
                right: expected,
            },
            Type::Bool,
            span,
        );
        let found = b.new_block();
        let invalid = b.new_block();
        b.terminate(
            TermKind::Cond {
                condition: matches,
                then_target: Target::new(found),
                else_target: Target::new(invalid),
            },
            span,
        );
        b.switch_to(invalid);
        b.terminate(
            TermKind::Trap {
                code: TrapCode::Empty,
                message: "Cannot extract payload of an inactive enum variant".into(),
            },
            span,
        );
        b.switch_to(found);
    }
    let value = b.emit(
        InstKind::ObjGet {
            object: receiver,
            field,
        },
        ty.clone(),
        span,
    );
    let (value, ty) = if field > 0 {
        let Type::Array(inner) = ty else {
            return Err(internal(span, "variant payload slot must be an array"));
        };
        let zero = b.emit(
            InstKind::Const(Constant::Int(IntValue::from_i64(0))),
            Type::Int,
            span,
        );
        (
            b.emit(
                InstKind::ArrayGet {
                    array: value,
                    index: zero,
                },
                (*inner).clone(),
                span,
            ),
            *inner,
        )
    } else {
        (value, ty)
    };
    b.terminate(TermKind::Ret(Some(value)), span);
    let (values, blocks, params) = b
        .finish(&[receiver])
        .map_err(|message| internal(span, &message))?;
    Ok(ir::Function {
        symbol,
        params,
        result: ResultType::Value(ty),
        attrs: vec![Attr::Method(object)],
        values,
        blocks,
        span,
    })
}

#[derive(Copy, Clone)]
enum AdapterKind {
    Sequence,
    Cursor,
    Order,
    Bytes,
    Text,
}

impl AdapterKind {
    fn of(ctx: &Ctx<'_, '_>, decl: crate::analysis::ty::ObjectId) -> AdapterKind {
        let adapters = ctx.reg.adapters;
        if Some(decl) == adapters.array_cursor {
            AdapterKind::Cursor
        } else if Some(decl) == adapters.scalar_order {
            AdapterKind::Order
        } else if Some(decl) == adapters.byte_array_source {
            AdapterKind::Bytes
        } else if Some(decl) == adapters.string_source {
            AdapterKind::Text
        } else {
            AdapterKind::Sequence
        }
    }
}

/// Категория сравнения скаляра `Comparing<T>`.
pub(super) fn scalar_kind(ty: &Type) -> Option<CompareKind> {
    Some(match ty {
        Type::Int => CompareKind::Int,
        Type::Byte => CompareKind::Byte,
        Type::Float => CompareKind::Float,
        Type::Rat => CompareKind::Rat,
        Type::Str => CompareKind::Str,
        Type::Char => CompareKind::Char,
        _ => return None,
    })
}

struct Adapter {
    b: FnBuilder,
    object: ObjectId,
    receiver: ValueId,
    span: Span,
}

impl Adapter {
    fn emit(&mut self, kind: InstKind, ty: Type) -> ValueId {
        self.b.emit(kind, ty, self.span)
    }

    fn ret(&mut self, value: ValueId) {
        self.b.terminate(TermKind::Ret(Some(value)), self.span);
    }

    fn int(&mut self, value: i64) -> ValueId {
        self.emit(
            InstKind::Const(Constant::Int(IntValue::from_i64(value))),
            Type::Int,
        )
    }

    fn field(&mut self, ctx: &Ctx<'_, '_>, field: usize) -> ValueId {
        let ty = ctx.objects[self.object.0].fields[field].1.clone();
        self.emit(
            InstKind::ObjGet {
                object: self.receiver,
                field,
            },
            ty,
        )
    }

    fn element(ctx: &Ctx<'_, '_>, object: ObjectId) -> Type {
        match &ctx.objects[object.0].fields[0].1 {
            Type::Array(element) => (**element).clone(),
            other => other.clone(),
        }
    }

    /// `items[index]`; вне границ — ошибка `Array index out of bounds`.
    fn get(&mut self, ctx: &Ctx<'_, '_>, items: ValueId, index: ValueId) -> ValueId {
        let element = Self::element(ctx, self.object);
        self.emit(
            InstKind::ArrayGet {
                array: items,
                index,
            },
            element,
        )
    }

    /// `items[index]` или программная ошибка `message` (пустой/исчерпанный).
    fn lookup(
        &mut self,
        ctx: &Ctx<'_, '_>,
        items: ValueId,
        index: ValueId,
        message: String,
    ) -> ValueId {
        let element = Self::element(ctx, self.object);
        let found = self.b.new_block();
        let item = self.b.add_param(found, element);
        let missing = self.b.new_block();
        self.b.terminate(
            TermKind::ArrayLookup {
                array: items,
                index,
                found: Target::new(found),
                missing: Target::new(missing),
            },
            self.span,
        );
        self.b.switch_to(missing);
        self.b.terminate(
            TermKind::Trap {
                code: TrapCode::Empty,
                message,
            },
            self.span,
        );
        self.b.switch_to(found);
        item
    }

    /// Элемент массива на границе элемента роли.
    fn cross(&mut self, value: ValueId, target: &Type) -> Result<ValueId> {
        let ty = self.b.value_type(value).clone();
        if ty == *target {
            return Ok(value);
        }
        match (&ty, target) {
            (Type::Object(_), Type::Role(_)) => {
                Ok(self.emit(InstKind::RoleWrap(value), target.clone()))
            }
            _ => Err(LoweringError::unsupported(
                self.span,
                "IR lowering of builtin role adapters (arrays and scalars as roles) is not implemented yet.",
            )),
        }
    }

    /// Курсор `ArrayCursor<S, T>` с позиции `at`, как ссылка `Cursor<T>`.
    fn cursor(
        &mut self,
        ctx: &mut Ctx<'_, '_>,
        items: ValueId,
        at: ValueId,
        role: &Type,
    ) -> Result<ValueId> {
        let decl = ctx
            .reg
            .adapters
            .array_cursor
            .ok_or_else(|| internal(self.span, "no Cursor role for array cursors"))?;
        let args = ctx.objects[self.object.0]
            .env
            .iter()
            .map(|(_, ty)| ty.clone())
            .collect();
        let cursor = ctx.object_instance(decl, args, self.span)?;
        let ctor = ctx.adapter_ctor(cursor);
        let value = self.emit(
            InstKind::Call {
                func: ctor,
                args: vec![items, at],
            },
            Type::Object(cursor),
        );
        Ok(self.emit(InstKind::RoleWrap(value), role.clone()))
    }
}

fn internal(span: Span, message: &str) -> LoweringError {
    LoweringError::invalid(span, format!("internal compiler error: {message}"))
}

/// `text()` значения варианта: `Enum.case` или `Enum.case(payload, ...)`;
/// строки в кавычках, числа — `string()`, `bool` и enum — их имена,
/// вложенный вариант — его собственный текст.
pub(super) fn lower_variant_text(
    ctx: &mut Ctx<'_, '_>,
    symbol: String,
    object: ObjectId,
) -> Result<ir::Function> {
    let span = ctx.objects[object.0].span;
    let Some(Ty::Variant(id, args)) = ctx.variant_ty(object) else {
        return Err(internal(span, "variant text of a non-variant object"));
    };
    let reg = ctx.reg;
    let decl = &reg.enums[id.0];
    let env: Vec<_> = decl.params.iter().copied().zip(args).collect();
    let mut b = FnBuilder::new();
    let receiver = b.new_value(Type::Object(object));
    let tag = b.emit(
        InstKind::ObjGet {
            object: receiver,
            field: 0,
        },
        Type::Int,
        span,
    );
    let mut offset = 1;
    for (case, payload) in decl.payloads.iter().enumerate() {
        let expected = b.emit(
            InstKind::Const(Constant::Int(IntValue::from_i64(case as i64))),
            Type::Int,
            span,
        );
        let matches = b.emit(
            InstKind::IntCompare {
                op: CompareOp::Eq,
                left: tag,
                right: expected,
            },
            Type::Bool,
            span,
        );
        let found = b.new_block();
        let next = b.new_block();
        b.terminate(
            TermKind::Cond {
                condition: matches,
                then_target: Target::new(found),
                else_target: Target::new(next),
            },
            span,
        );
        b.switch_to(found);
        let mut text = text_const(&mut b, &format!("{}.{}", decl.name, decl.cases[case]), span);
        if !payload.is_empty() {
            text = append(&mut b, text, "(", span);
            for (index, (_, field_ty)) in payload.iter().enumerate() {
                if index > 0 {
                    text = append(&mut b, text, ", ", span);
                }
                let field = offset + index;
                let Type::Array(inner) = ctx.objects[object.0].fields[field].1.clone() else {
                    return Err(internal(span, "variant payload slot must be an array"));
                };
                let slot = b.emit(
                    InstKind::ObjGet {
                        object: receiver,
                        field,
                    },
                    Type::Array(inner.clone()),
                    span,
                );
                let zero = b.emit(
                    InstKind::Const(Constant::Int(IntValue::from_i64(0))),
                    Type::Int,
                    span,
                );
                let value = b.emit(
                    InstKind::ArrayGet {
                        array: slot,
                        index: zero,
                    },
                    (*inner).clone(),
                    span,
                );
                let shown = show(ctx, &mut b, value, &field_ty.subst(&env), span)?;
                text = concat(&mut b, text, shown, span);
            }
            text = append(&mut b, text, ")", span);
        }
        b.terminate(TermKind::Ret(Some(text)), span);
        b.switch_to(next);
        offset += payload.len();
    }
    b.terminate(
        TermKind::Trap {
            code: TrapCode::Empty,
            message: "Enum variant tag is out of range".into(),
        },
        span,
    );
    let (values, blocks, params) = b
        .finish(&[receiver])
        .map_err(|message| internal(span, &message))?;
    Ok(ir::Function {
        symbol,
        params,
        result: ResultType::Value(Type::Str),
        attrs: vec![Attr::Method(object)],
        values,
        blocks,
        span,
    })
}

/// Текст `bool` (`true`/`false`) или case enum без payload (`Enum.case`).
pub(super) fn lower_describe(
    ctx: &mut Ctx<'_, '_>,
    symbol: String,
    ty: &Ty,
) -> Result<ir::Function> {
    let span = Span::empty(0);
    let ir_ty = ctx.ir_type(ty, span)?;
    let mut b = FnBuilder::new();
    let value = b.new_value(ir_ty.clone());
    let cases: Vec<(InstKind, String)> = match ty {
        Ty::Bool => vec![
            (InstKind::Const(Constant::Bool(true)), "true".to_string()),
            (InstKind::Const(Constant::Bool(false)), "false".to_string()),
        ],
        Ty::Enum(id) => {
            let decl = &ctx.reg.enums[id.0];
            let ir_enum = ctx.enum_id(*id);
            decl.cases
                .iter()
                .enumerate()
                .map(|(case, name)| {
                    (
                        InstKind::EnumCase { ty: ir_enum, case },
                        format!("{}.{name}", decl.name),
                    )
                })
                .collect()
        }
        _ => return Err(internal(span, "describe needs bool or a payload-free enum")),
    };
    for (constant, name) in cases {
        let expected = b.emit(constant, ir_ty.clone(), span);
        let compare = match ty {
            Ty::Bool => InstKind::BoolCompare {
                op: CompareOp::Eq,
                left: value,
                right: expected,
            },
            _ => InstKind::EnumCompare {
                op: CompareOp::Eq,
                left: value,
                right: expected,
            },
        };
        let matches = b.emit(compare, Type::Bool, span);
        let found = b.new_block();
        let next = b.new_block();
        b.terminate(
            TermKind::Cond {
                condition: matches,
                then_target: Target::new(found),
                else_target: Target::new(next),
            },
            span,
        );
        b.switch_to(found);
        let text = text_const(&mut b, &name, span);
        b.terminate(TermKind::Ret(Some(text)), span);
        b.switch_to(next);
    }
    b.terminate(
        TermKind::Trap {
            code: TrapCode::Empty,
            message: "Enum case is out of range".into(),
        },
        span,
    );
    let (values, blocks, params) = b
        .finish(&[value])
        .map_err(|message| internal(span, &message))?;
    Ok(ir::Function {
        symbol,
        params,
        result: ResultType::Value(Type::Str),
        attrs: Vec::new(),
        values,
        blocks,
        span,
    })
}

/// Текст одного значения payload ошибки (типы ограничены анализом).
fn show(
    ctx: &mut Ctx<'_, '_>,
    b: &mut FnBuilder,
    value: ValueId,
    ty: &Ty,
    span: Span,
) -> Result<ValueId> {
    Ok(match ty {
        Ty::Str => {
            let quoted = append_to(b, "\"", value, span);
            append(b, quoted, "\"", span)
        }
        Ty::Int | Ty::Byte | Ty::Float | Ty::Rat => b.emit(InstKind::Cvt(value), Type::Str, span),
        Ty::Bool | Ty::Enum(_) => {
            let func = ctx.describe(ty);
            b.emit(
                InstKind::Call {
                    func,
                    args: vec![value],
                },
                Type::Str,
                span,
            )
        }
        Ty::Variant(..) => {
            let Type::Object(object) = ctx.ir_type(ty, span)? else {
                return Err(internal(span, "a variant lowers to an object"));
            };
            let func = ctx.variant_text(object);
            b.emit(
                InstKind::Call {
                    func,
                    args: vec![value],
                },
                Type::Str,
                span,
            )
        }
        _ => return Err(internal(span, "unsupported error payload type")),
    })
}

fn text_const(b: &mut FnBuilder, text: &str, span: Span) -> ValueId {
    b.emit(InstKind::Const(Constant::Str(text.to_string())), Type::Str, span)
}

fn concat(b: &mut FnBuilder, left: ValueId, right: ValueId, span: Span) -> ValueId {
    b.emit(InstKind::StrConcat { left, right }, Type::Str, span)
}

fn append(b: &mut FnBuilder, left: ValueId, text: &str, span: Span) -> ValueId {
    let right = text_const(b, text, span);
    concat(b, left, right, span)
}

fn append_to(b: &mut FnBuilder, text: &str, right: ValueId, span: Span) -> ValueId {
    let left = text_const(b, text, span);
    concat(b, left, right, span)
}

/// The init function of a constant array or map (#153): the value the
/// compiler computed, built from constants, rats, enum cases, arrays and maps.
pub(super) fn lower_global(ctx: &mut Ctx<'_, '_>, symbol: String, constant: usize) -> Result<ir::Function> {
    let span = ctx.reg.consts[constant].decl.span;
    let Some(crate::analysis::registry::ConstState::Done(Some(result))) = ctx.reg.const_state.get(constant).cloned() else {
        return Err(internal(span, "a constant without a value"));
    };
    let ty = ctx.ir_type(&result.ty, span)?;
    let mut b = FnBuilder::new();
    let value = build_constant(ctx, &mut b, &result.value, &result.ty, span)?;
    b.terminate(TermKind::Ret(Some(value)), span);
    let (values, blocks, params) = b.finish(&[]).map_err(|message| internal(span, &message))?;
    Ok(ir::Function {
        symbol,
        params,
        result: ResultType::Value(ty),
        attrs: Vec::new(),
        values,
        blocks,
        span,
    })
}

fn build_constant(
    ctx: &mut Ctx<'_, '_>,
    b: &mut FnBuilder,
    value: &crate::analysis::thir::ConstValue,
    ty: &Ty,
    span: Span,
) -> Result<ValueId> {
    use crate::analysis::thir::{ConstValue, Constant as Literal};
    let int = |text: &str| IntValue::parse(text).ok_or_else(|| internal(span, "a malformed integer constant"));
    Ok(match value {
        ConstValue::Scalar(literal) => {
            let constant = match literal {
                Literal::Int(text) => Constant::Int(int(text)?),
                Literal::Float(bits) => Constant::Float(*bits),
                Literal::Str(text) => Constant::Str(text.clone()),
                Literal::Char(ch) => Constant::Char(*ch),
                Literal::Bool(flag) => Constant::Bool(*flag),
                Literal::Byte(byte) => Constant::Byte(*byte),
            };
            let ty = constant.ty();
            b.emit(InstKind::Const(constant), ty, span)
        }
        ConstValue::Rat(numerator, denominator) => {
            let numerator = b.emit(InstKind::Const(Constant::Int(int(numerator)?)), Type::Int, span);
            let denominator = b.emit(InstKind::Const(Constant::Int(int(denominator)?)), Type::Int, span);
            b.emit(InstKind::Rat { op: RatOp::New, args: vec![numerator, denominator] }, Type::Rat, span)
        }
        ConstValue::Enum(id, case) => {
            let ir_enum = ctx.enum_id(*id);
            b.emit(InstKind::EnumCase { ty: ir_enum, case: *case }, Type::Enum(ir_enum), span)
        }
        ConstValue::Array(items) => {
            let element = match ty {
                Ty::Array(element) => (**element).clone(),
                _ => return Err(internal(span, "a constant array without an array type")),
            };
            let mut values = Vec::new();
            for item in items {
                values.push(build_constant(ctx, b, item, &element, span)?);
            }
            let array = ctx.ir_type(ty, span)?;
            b.emit(InstKind::ArrayNew(values), array, span)
        }
        ConstValue::Map(entries) => {
            let (key, entry) = match ty {
                Ty::Map(key, value) => ((**key).clone(), (**value).clone()),
                _ => return Err(internal(span, "a constant map without a map type")),
            };
            let mut pairs = Vec::new();
            for (k, v) in entries {
                pairs.push((build_constant(ctx, b, k, &key, span)?, build_constant(ctx, b, v, &entry, span)?));
            }
            let map = ctx.ir_type(ty, span)?;
            b.emit(InstKind::MapNew(pairs), map, span)
        }
    })
}
