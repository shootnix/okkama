//! Builtin-действия значений (§6.6) и явные приведения (§6.6.1).
//!
//! Таблица действий повторяет `BuiltinActions.pm`: категория получателя,
//! точная арность, параметры той же категории (`same`) без расширения,
//! только позиционные аргументы.

use super::Checker;
use crate::analysis::thir::{
    Builtin, Callee, CastPart, CastTarget, CompareKind, CompareOp, Expr, ExprKind,
};
use crate::analysis::ty::Ty;
use crate::source::Span;
use crate::syntax::ast;

/// Параметр builtin-действия.
#[derive(Clone)]
enum Param {
    /// Тип категории получателя.
    Same,
    Exact(Ty),
    /// Любой скалярный ключ словаря.
    Key,
}

/// Битовые действия `int`/`byte`.
const BIT_ACTIONS: [&str; 7] = [
    "bit_count",
    "leading_zeros",
    "trailing_zeros",
    "rotate_left",
    "rotate_right",
    "reverse_bits",
    "swap_bytes",
];

fn compare_kind(ty: &Ty) -> Option<CompareKind> {
    match ty {
        Ty::Int => Some(CompareKind::Int),
        Ty::Byte => Some(CompareKind::Byte),
        Ty::Float => Some(CompareKind::Float),
        Ty::Rat => Some(CompareKind::Rat),
        Ty::Str => Some(CompareKind::Str),
        Ty::Char => Some(CompareKind::Char),
        _ => None,
    }
}

fn ordering(name: &str) -> Option<CompareOp> {
    match name {
        "less" => Some(CompareOp::Lt),
        "less_or_equal" => Some(CompareOp::Le),
        "more" => Some(CompareOp::Gt),
        "more_or_equal" => Some(CompareOp::Ge),
        _ => None,
    }
}

fn cast_target(name: &str) -> Option<CastTarget> {
    match name {
        "int" => Some(CastTarget::Int),
        "byte" => Some(CastTarget::Byte),
        "float" => Some(CastTarget::Float),
        "rat" => Some(CastTarget::Rat),
        "string" => Some(CastTarget::Str),
        _ => None,
    }
}

impl Checker<'_, '_> {
    /// Категория получателя для сообщений (`Type "string" has no …`).
    fn category(ty: &Ty) -> Option<&'static str> {
        Some(match ty {
            Ty::Array(_) => "array",
            Ty::Map(..) => "map",
            Ty::Str => "string",
            Ty::Int => "int",
            Ty::Float => "float",
            Ty::Rat => "rat",
            Ty::Byte => "byte",
            Ty::Bool => "bool",
            Ty::Char => "char",
            Ty::Future(_) => "Future",
            _ => return None,
        })
    }

    /// Сигнатура builtin-действия: параметры и результат (`None` — нет такого).
    fn builtin_signature(
        &self,
        receiver: &Ty,
        name: &str,
    ) -> Option<(Builtin, Vec<Param>, Option<Ty>)> {
        let int = || Param::Exact(Ty::Int);
        let string = || Param::Exact(Ty::Str);
        let order = self.reg.standard.order.map(Ty::Enum);
        Some(match (receiver, name) {
            (Ty::Array(_), "size") => (Builtin::ArraySize, vec![], Some(Ty::Int)),
            (Ty::Array(_), "slice") => (
                Builtin::ArraySlice,
                vec![int(), int()],
                Some(receiver.clone()),
            ),
            (Ty::Array(_), "join") => (Builtin::ArrayJoin, vec![string()], Some(Ty::Str)),
            (Ty::Array(element), "item") => {
                (Builtin::ArrayItem, vec![int()], Some((**element).clone()))
            }
            (Ty::Array(element), "first") => {
                (Builtin::ArrayFirst, vec![], Some((**element).clone()))
            }
            (Ty::Array(element), "last") => (Builtin::ArrayLast, vec![], Some((**element).clone())),
            (Ty::Array(element), "cursor") => {
                let cursor = self.reg.standard.cursor?;
                (
                    Builtin::ArrayCursor,
                    vec![],
                    Some(Ty::Role(cursor, vec![(**element).clone()])),
                )
            }
            (Ty::Array(_), "ordering") => (Builtin::ArrayOrdering, vec![], Some(receiver.clone())),
            (Ty::Map(..), "size") => (Builtin::MapSize, vec![], Some(Ty::Int)),
            (Ty::Map(..), "has") => (Builtin::MapHas, vec![Param::Key], Some(Ty::Bool)),
            (Ty::Map(key, _), "keys") => {
                (Builtin::MapKeys, vec![], Some(Ty::array((**key).clone())))
            }
            (Ty::Map(_, value), "values") => (
                Builtin::MapValues,
                vec![],
                Some(Ty::array((**value).clone())),
            ),
            (Ty::Str, "length") => (Builtin::StrLength, vec![], Some(Ty::Int)),
            (Ty::Str, "empty") => (Builtin::StrEmpty, vec![], Some(Ty::Bool)),
            (Ty::Str, "slice") => (Builtin::StrSlice, vec![int(), int()], Some(Ty::Str)),
            (Ty::Str, "split") => (
                Builtin::StrSplit,
                vec![Param::Same],
                Some(Ty::array(Ty::Str)),
            ),
            (Ty::Str, "replace") => (
                Builtin::StrReplace,
                vec![Param::Same, Param::Same],
                Some(Ty::Str),
            ),
            (Ty::Str, "replace_first") => (
                Builtin::StrReplaceFirst,
                vec![Param::Same, Param::Same],
                Some(Ty::Str),
            ),
            (Ty::Str, "trim") => (Builtin::StrTrim, vec![], Some(Ty::Str)),
            (Ty::Str, "chomp") => (Builtin::StrChomp, vec![], Some(Ty::Str)),
            // Строки упорядочиваются по длине (как Perl); `more` у строк нет.
            (Ty::Str, "less" | "less_or_equal") => (
                Builtin::StrCompare(ordering(name)?),
                vec![Param::Same],
                Some(Ty::Bool),
            ),
            (Ty::Str, "string") => (Builtin::Convert(CastTarget::Str), vec![], Some(Ty::Str)),
            (Ty::Str | Ty::Char | Ty::Int | Ty::Float | Ty::Rat | Ty::Byte, "order") => (
                Builtin::Order(compare_kind(receiver)?),
                vec![Param::Same],
                Some(order?),
            ),
            (
                Ty::Int | Ty::Float | Ty::Rat | Ty::Byte,
                "less" | "less_or_equal" | "more" | "more_or_equal",
            ) => (
                Builtin::Compare(compare_kind(receiver)?, ordering(name)?),
                vec![Param::Same],
                Some(Ty::Bool),
            ),
            (Ty::Int | Ty::Float | Ty::Rat | Ty::Byte | Ty::Bool, cast)
                if cast_target(cast).is_some() =>
            {
                let target = cast_target(cast)?;
                let allowed = match receiver {
                    Ty::Bool => target == CastTarget::Str,
                    _ => true,
                };
                if !allowed {
                    return None;
                }
                let ty = match target {
                    CastTarget::Int => Ty::Int,
                    CastTarget::Byte => Ty::Byte,
                    CastTarget::Float => Ty::Float,
                    CastTarget::Rat => Ty::Rat,
                    CastTarget::Str => Ty::Str,
                };
                (Builtin::Convert(target), vec![], Some(ty))
            }
            (Ty::Int | Ty::Float, "abs") => (Builtin::Abs, vec![], Some(receiver.clone())),
            (Ty::Rat, "abs") => (Builtin::Abs, vec![], Some(Ty::Rat)),
            (Ty::Int | Ty::Float | Ty::Rat, "sign") => (Builtin::Sign, vec![], Some(Ty::Int)),
            (Ty::Int | Ty::Float, "pow") => (Builtin::Pow, vec![Param::Same], Some(Ty::Float)),
            (Ty::Int | Ty::Float, "sqrt") => (Builtin::Sqrt, vec![], Some(Ty::Float)),
            (Ty::Rat, "numerator") => (Builtin::Numerator, vec![], Some(Ty::Str)),
            (Ty::Rat, "denominator") => (Builtin::Denominator, vec![], Some(Ty::Str)),
            (Ty::Int | Ty::Byte, bits) if BIT_ACTIONS.contains(&bits) => {
                let rotates = bits.starts_with("rotate");
                let counts = matches!(bits, "bit_count" | "leading_zeros" | "trailing_zeros");
                (
                    Builtin::Bits {
                        name: bits.to_string(),
                        byte: *receiver == Ty::Byte,
                    },
                    if rotates { vec![int()] } else { vec![] },
                    Some(if counts { Ty::Int } else { receiver.clone() }),
                )
            }
            (Ty::Future(_), "cancel") => (Builtin::FutureCancel, vec![], Some(receiver.clone())),
            (Ty::Future(_), "done") => (Builtin::FutureDone, vec![], Some(Ty::Bool)),
            (Ty::Future(_), "cancelled") => (Builtin::FutureCancelled, vec![], Some(Ty::Bool)),
            _ => return None,
        })
    }

    /// `value.name(args)` для builtin-получателя.
    pub(super) fn builtin_call(
        &mut self,
        target: Expr,
        name: &ast::Name,
        args: &[ast::Argument],
        span: Span,
    ) -> Expr {
        let receiver = Self::ty_of(&target).taken();
        let Some(category) = Self::category(&receiver) else {
            self.error(name.span, "Can only call actions on objects");
            for arg in args {
                let _ = self.expr(&arg.value, true, None);
            }
            return Self::poisoned(span);
        };
        let Some((op, params, result)) = self.builtin_signature(&receiver, &name.text) else {
            let message = format!(
                "Type \"{category}\" has no builtin action \"{}\"",
                name.text
            );
            self.error(name.span, message);
            for arg in args {
                let _ = self.expr(&arg.value, true, None);
            }
            return Self::poisoned(span);
        };
        if args.iter().any(|arg| arg.name.is_some()) {
            self.error(span, "Builtin actions take positional arguments only");
            return Self::poisoned(span);
        }
        if args.len() != params.len() {
            let message = if let Builtin::Bits { .. } = op {
                format!(
                    "Builtin action \"{}\" takes {} argument(s)",
                    name.text,
                    params.len()
                )
            } else {
                format!(
                    "Builtin action \"{}\" on type \"{category}\" expects {} argument(s), got {}",
                    name.text,
                    params.len(),
                    args.len()
                )
            };
            self.error(span, message);
            for arg in args {
                let _ = self.expr(&arg.value, true, None);
            }
            return Self::poisoned(span);
        }
        let mut values = Vec::new();
        let mut diverges = target.diverges;
        for (arg, param) in args.iter().zip(&params) {
            let hint = match param {
                Param::Same => Some(receiver.clone()),
                Param::Exact(ty) => Some(ty.clone()),
                Param::Key => None,
            };
            let value = self.value(&arg.value, hint.as_ref());
            diverges |= value.diverges;
            let actual = Self::ty_of(&value).taken();
            let expected = match param {
                Param::Same => Some(receiver.clone()),
                Param::Exact(ty) => Some(ty.clone()),
                Param::Key => match &receiver {
                    Ty::Map(key, _) if **key != Ty::Never => Some((**key).clone()),
                    _ => {
                        if !actual.is_map_key() && !actual.is_error() {
                            let message = format!(
                                "Map key type {} is not a scalar key type",
                                self.type_name(&actual)
                            );
                            self.error(arg.value.span, message);
                        }
                        None
                    }
                },
            };
            if let Some(expected) = expected {
                if actual != expected && !actual.is_error() && !value.diverges {
                    let message = if let Builtin::Bits { .. } = op {
                        format!("Builtin action \"{}\" requires an int count", name.text)
                    } else {
                        format!(
                            "Cannot pass {} to builtin action \"{}\" parameter of type {}",
                            self.type_name(&actual),
                            name.text,
                            self.type_name(&expected)
                        )
                    };
                    self.error(span, message);
                    return Self::poisoned(span);
                }
            }
            let value = if actual != Self::ty_of(&value) {
                self.coerce(value, &actual, arg.value.span, "Argument")
            } else {
                value
            };
            values.push(value);
        }
        if op == Builtin::ArrayJoin {
            if let Ty::Array(element) = &receiver {
                if **element != Ty::Str && **element != Ty::Never && !element.is_error() {
                    let message = format!(
                        "Builtin action \"join\" requires an array of string, got {}",
                        self.type_name(&receiver)
                    );
                    self.error(span, message);
                    return Self::poisoned(span);
                }
            }
        }
        if op == Builtin::ArrayOrdering {
            if let Ty::Array(element) = &receiver {
                let comparing = self
                    .reg
                    .standard
                    .comparing
                    .map(|role| Ty::Role(role, vec![(**element).clone()]));
                let ordered = **element == Ty::Never
                    || element.is_error()
                    || comparing.is_some_and(|role| self.reg.assignable(element, &role));
                if !ordered {
                    let message = format!(
                        "Builtin action \"ordering\" requires elements that do Comparing<{0}>, got {0}",
                        self.type_name(element)
                    );
                    self.error(span, message);
                    return Self::poisoned(span);
                }
            }
        }
        if matches!(
            op,
            Builtin::ArrayItem | Builtin::ArrayFirst | Builtin::ArrayLast
        ) && receiver.is_untagged_array()
        {
            self.error(span, "An empty untyped array has no elements to read");
            return Self::poisoned(span);
        }
        let target = if Self::ty_of(&target) != receiver {
            self.coerce(target, &receiver, span, "Receiver")
        } else {
            target
        };
        Expr {
            kind: ExprKind::Call {
                callee: Callee::Builtin {
                    receiver: Box::new(target),
                    op,
                },
                args: values,
            },
            ty: result,
            diverges,
            span,
        }
    }

    /// Явное приведение одного значения (`T(x)`, элемент `@T(...)`).
    pub(super) fn convert(
        &mut self,
        value: Expr,
        target: CastTarget,
        value_span: Span,
        position: Option<usize>,
        span: Span,
    ) -> Expr {
        let actual = Self::ty_of(&value).taken();
        let (result, accepted) = match target {
            CastTarget::Int => (
                Ty::Int,
                matches!(actual, Ty::Int | Ty::Byte | Ty::Float | Ty::Rat),
            ),
            CastTarget::Float => (
                Ty::Float,
                matches!(actual, Ty::Int | Ty::Byte | Ty::Float | Ty::Rat),
            ),
            CastTarget::Byte => (
                Ty::Byte,
                matches!(actual, Ty::Int | Ty::Byte | Ty::Float | Ty::Rat),
            ),
            CastTarget::Rat => (
                Ty::Rat,
                matches!(actual, Ty::Int | Ty::Byte | Ty::Float | Ty::Rat),
            ),
            CastTarget::Str => (
                Ty::Str,
                matches!(
                    actual,
                    Ty::Str | Ty::Int | Ty::Byte | Ty::Float | Ty::Rat | Ty::Bool
                ),
            ),
        };
        if !accepted && !actual.is_error() && !value.diverges {
            let name = match target {
                CastTarget::Int => "int",
                CastTarget::Byte => "byte",
                CastTarget::Float => "float",
                CastTarget::Rat => "rat",
                CastTarget::Str => "string",
            };
            let message = format!("Cannot convert {} to {name}", self.type_name(&actual));
            let message = match position {
                Some(position) => format!("@{name}() argument {position}: {message}"),
                None => message,
            };
            self.error(value_span, message);
            return Self::poisoned(span);
        }
        let diverges = value.diverges;
        let mut out = Self::typed(
            ExprKind::Cast {
                target,
                value: Box::new(Self::consume(value)),
                position,
            },
            result,
            span,
        );
        out.diverges = diverges;
        out
    }

    /// `@int(...)`, `@string(...)`: операнды и распаковки по порядку.
    pub(super) fn array_cast(
        &mut self,
        target: &ast::Name,
        args: &[ast::Argument],
        span: Span,
    ) -> Expr {
        let element = match target.text.as_str() {
            "int" => CastTarget::Int,
            "byte" => CastTarget::Byte,
            "float" => CastTarget::Float,
            "string" => CastTarget::Str,
            other => {
                self.error(target.span, format!("`@{other}(...)` is not an array cast"));
                return Self::poisoned(span);
            }
        };
        if args.iter().any(|arg| arg.name.is_some()) {
            self.error(span, "Array casts take positional arguments only");
            return Self::poisoned(span);
        }
        let mut parts = Vec::new();
        let mut diverges = false;
        for (index, arg) in args.iter().enumerate() {
            let position = Some(index + 1);
            match &arg.value.kind {
                ast::ExprKind::Unpack { operand, .. } => {
                    let array = self.unpacked(operand, arg.value.span);
                    diverges |= array.diverges;
                    if let Ty::Array(source) = Self::ty_of(&array) {
                        // Элементы приводятся по одному, с номером операнда.
                        let probe =
                            Self::typed(ExprKind::Unreachable, (*source).clone(), operand.span);
                        if *source != Ty::Never {
                            let _ = self.convert(probe, element, operand.span, position, span);
                        }
                    }
                    parts.push(CastPart::Spread(array));
                }
                _ => {
                    let value = self.value(&arg.value, None);
                    diverges |= value.diverges;
                    let converted =
                        self.convert(value, element, arg.value.span, position, arg.value.span);
                    parts.push(CastPart::One(converted));
                }
            }
        }
        let element_ty = match element {
            CastTarget::Int => Ty::Int,
            CastTarget::Byte => Ty::Byte,
            CastTarget::Float => Ty::Float,
            CastTarget::Rat => Ty::Rat,
            CastTarget::Str => Ty::Str,
        };
        let mut out = Self::typed(
            ExprKind::ArrayCast { element, parts },
            Ty::array(element_ty),
            span,
        );
        out.diverges = diverges;
        out
    }
}
