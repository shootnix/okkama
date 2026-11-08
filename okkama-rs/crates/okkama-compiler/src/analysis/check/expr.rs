//! Выражения: литералы, имена, операторы, условные и составные формы.

use super::stmt::{ElsePart, literal_condition, strip_grouping};
use super::{Checker, Kind, LoopCtx};
use crate::analysis::thir::{
    ArithOp, ArrayPart, BinaryOp, BitOp, Catch, CatchAlt, CatchType, CompareKind, CompareOp,
    Constant, Expr, ExprKind, Match, NumericKind, Stmt, StmtKind, Try, UnaryOp,
};
use crate::analysis::ty::{EnumId, ParamId, RoleId, Ty};
use crate::ir::IntValue;
use crate::source::Span;
use crate::syntax::ast;
use crate::syntax::token::Radix;

impl Checker<'_, '_> {
    /// Выражение в позиции значения: ошибка, если значения нет.
    pub(super) fn value(&mut self, expr: &ast::Expr, expected: Option<&Ty>) -> Expr {
        let checked = self.expr(expr, true, expected);
        if checked.ty.is_none() && !checked.diverges {
            let message = self
                .missing_cases(&checked)
                .unwrap_or_else(|| "Expression does not produce a value".to_string());
            self.error(expr.span, message);
            return Self::poisoned(expr.span);
        }
        checked
    }

    /// Why a `match` over an enum has no value: the cases it leaves out
    /// (#229), named as the IDE lint `non-exhaustive-match` names them.
    fn missing_cases(&self, checked: &Expr) -> Option<String> {
        let ExprKind::Match(matched) = &checked.kind else { return None };
        let Some(Ty::Enum(id)) = &matched.subject.ty else { return None };
        if matched.exhaustive || matched.else_block.is_some() {
            return None;
        }
        let declaration = &self.reg.enums[id.0];
        let mut seen = vec![false; declaration.cases.len()];
        for pattern in matched.cases.iter().flat_map(|(patterns, _)| patterns) {
            match &pattern.kind {
                ExprKind::EnumCase { ty, case } if ty == id => seen[*case] = true,
                _ => return None,
            }
        }
        let missing: Vec<&str> = declaration.cases.iter().zip(&seen).filter(|(_, seen)| !**seen).map(|(name, _)| name.as_str()).collect();
        (!missing.is_empty()).then(|| {
            format!(
                "match over enum `{}` does not cover {}, so it has no value here; add the cases or an `else` branch",
                declaration.name,
                missing.join(", ")
            )
        })
    }

    /// Выражение; `want` — значение используется, `expected` — тип границы,
    /// которого ждут (вывод generic-конструирования, тег пустых коллекций).
    pub(super) fn expr(&mut self, expr: &ast::Expr, want: bool, expected: Option<&Ty>) -> Expr {
        let span = expr.span;
        match &expr.kind {
            ast::ExprKind::Throw(value) => {
                let value = self.thrown(value);
                Self::diverged(
                    ExprKind::Block(crate::analysis::thir::Block {
                        stmts: vec![crate::analysis::thir::Stmt {
                            kind: crate::analysis::thir::StmtKind::Throw(value),
                            span,
                        }],
                        span,
                    }),
                    span,
                )
            }
            ast::ExprKind::TypeValue(_) => {
                self.error(span, "A type is not a value");
                Self::poisoned(span)
            }
            ast::ExprKind::Coalesce { value, fallback } => {
                self.optional_fallback(value, fallback, expected, span)
            }
            ast::ExprKind::Literal(literal) => self.literal(literal, span),
            ast::ExprKind::Name(name) => self.read_name(name),
            ast::ExprKind::SelfValue => self.self_expr(span),
            ast::ExprKind::Choice(inner) => {
                if let ast::ExprKind::If {
                    condition,
                    then_block,
                    else_block,
                } = &inner.kind
                {
                    let check = self.value(condition, Some(&Ty::Bool));
                    self.expect(&check, &Ty::Bool, condition.span, "Choice condition");
                    let yes = self.block(then_block, true, expected);
                    let no = self.block(else_block, true, expected);
                    let (ty, diverges) = self.join(&[&yes, &no], true, span);
                    if ty.is_none() && !diverges {
                        self.error(
                            span,
                            "Compact choice branches must produce a value or diverge",
                        );
                    }
                    return Expr {
                        diverges: check.diverges || diverges,
                        kind: ExprKind::If {
                            condition: Box::new(check),
                            then_block: self.block_of(yes),
                            else_block: Some(self.block_of(no)),
                        },
                        ty,
                        span,
                    };
                }
                let result = self.expr(inner, true, expected);
                if result.ty.is_none() && !result.diverges {
                    self.error(
                        span,
                        "Compact choice branches must produce a value or diverge",
                    );
                }
                if let ast::ExprKind::Match(body) = &inner.kind {
                    let mut names = std::collections::HashSet::new();
                    for arm in &body.cases {
                        if let ast::ExprKind::Get { name, .. } = &arm.patterns[0].kind
                            && !names.insert(&name.text) {
                                self.error(name.span, "Duplicate variant pattern");
                            }
                    }
                    if let ExprKind::Match(found) = &result.kind {
                        if !matches!(Self::ty_of(&found.subject), Ty::Enum(_)) {
                            self.error(span, "Compact patterns require an enum value");
                        }
                        if !found.exhaustive && found.else_block.is_none() {
                            self.error(span, "Non-exhaustive compact match");
                        }
                    }
                }
                result
            }
            ast::ExprKind::Grouping(inner) => self.expr(inner, want, expected),
            ast::ExprKind::Unary { op, operand, .. } => self.unary(*op, operand, span),
            ast::ExprKind::Binary {
                op: op @ (ast::BinaryOp::And | ast::BinaryOp::Or),
                left,
                right,
                ..
            } => self.logical(*op, left, right, span),
            ast::ExprKind::Binary {
                op, left, right, ..
            } => {
                // `fake.storage.get(k)` операнд получает тип другого операнда.
                if is_fake_get(left) && !is_fake_get(right) {
                    let rhs = self.value(right, None);
                    let ty = Self::ty_of(&rhs).taken();
                    let lhs = self.value(left, Some(&ty));
                    return self.binary_values(*op, lhs, rhs, span);
                }
                let lhs = self.value(left, None);
                // An object, a role or a role-bounded type on the left: the
                // operator role it declares (#215).
                if Self::operator_operand(*op, &lhs) {
                    return self.operator_call(*op, lhs, right, span);
                }
                let rhs = if is_fake_get(right) || matches!(lhs.ty, Some(Ty::Variant(..))) {
                    let ty = Self::ty_of(&lhs).taken();
                    self.value(right, Some(&ty))
                } else {
                    self.value(right, None)
                };
                self.binary_values(*op, lhs, rhs, span)
            }
            ast::ExprKind::If {
                condition,
                then_block,
                else_block,
            } => self.if_parts(
                condition,
                then_block,
                Some(ElsePart::Block(else_block)),
                want,
                expected,
                span,
            ),
            ast::ExprKind::Loop { body } => self.loop_expr(body, want, expected, span),
            ast::ExprKind::Panic { value } => {
                let message = self.panic_message(value);
                Self::diverged(ExprKind::Panic(Box::new(message)), span)
            }
            ast::ExprKind::Call { callee, args } => self.call(callee, args, span, expected),
            ast::ExprKind::Get { object, name } => self.get(object, name, span, expected),
            ast::ExprKind::Try(body) => self.try_body(body, want, expected, span),
            ast::ExprKind::TryMark { force, value } => {
                self.try_mark(*force, value, want, expected, span)
            }
            ast::ExprKind::Match(body) => self.match_body(body, want, expected, span),
            ast::ExprKind::Array { elements } => self.array_literal(elements, expected, span),
            ast::ExprKind::Map { entries } => self.map_literal(entries, expected, span),
            ast::ExprKind::Range {
                inclusive,
                start,
                end,
                ..
            } => {
                let (start, end) = self.range_bounds(start, end);
                Self::typed(
                    ExprKind::Range {
                        start: Box::new(start),
                        end: Box::new(end),
                        inclusive: *inclusive,
                    },
                    Ty::Range,
                    span,
                )
            }
            ast::ExprKind::Index {
                object,
                index,
                fallback,
            } => self.index(object, index, fallback.as_deref(), want, expected, span),
            ast::ExprKind::Functional {
                operation,
                receiver,
                args,
                bindings,
                body,
                ..
            } => self.functional(*operation, receiver, args, bindings, body, expected, span),
            ast::ExprKind::ArrayCast { target, args, .. } => self.array_cast(target, args, span),
            ast::ExprKind::Unpack { .. } => {
                self.error(
                    span,
                    "Unpack is allowed only as a let value, a field value, a for source, a variadic argument or an array cast operand",
                );
                Self::poisoned(span)
            }
            ast::ExprKind::Await { value } => self.await_expr(value, want, span),
            ast::ExprKind::Spawn { call } => self.spawn(call, span),
            ast::ExprKind::AnonymousAction {
                params,
                result,
                body,
            } => self.closure(params, result, body, span),
            ast::ExprKind::StreamWrite { sink, source, .. } => {
                self.stream_write(sink, source, span)
            }
            ast::ExprKind::Select { object, name } => self.select(object, name, span),
            ast::ExprKind::OwnerField { owner, name } => self.owner_field(owner, name, span),
        }
    }

    // ----- литералы и имена -----

    fn literal(&mut self, literal: &ast::Literal, span: Span) -> Expr {
        let (constant, ty) = match literal {
            ast::Literal::Int(int) => match int_value(int) {
                Some(value) => (Constant::Int(value.to_string().to_string()), Ty::Int),
                None => {
                    self.error(span, "Integer literal cannot be represented exactly");
                    return Self::poisoned(span);
                }
            },
            ast::Literal::Float(float) => match float_value(float) {
                // A literal that is not zero but rounds to it lost its value (#57).
                Some(value) if value == 0.0 && float.nonzero() => {
                    self.error(span, "Float literal is too small: it would round to 0.0");
                    return Self::poisoned(span);
                }
                Some(value) => (Constant::Float(value.to_bits()), Ty::Float),
                None => {
                    self.error(span, "Float value must contain a finite number");
                    return Self::poisoned(span);
                }
            },
            ast::Literal::String(text) => (Constant::Str(text.value.clone()), Ty::Str),
            ast::Literal::Char(ch) => (Constant::Char(*ch), Ty::Char),
            ast::Literal::Bool(value) => (Constant::Bool(*value), Ty::Bool),
        };
        Self::typed(ExprKind::Constant(constant), ty, span)
    }

    /// Чтение имени: локальная привязка, затем поле `self` (в actions).
    fn read_name(&mut self, name: &ast::Name) -> Expr {
        if let Some(local) = self.local(&name.text) {
            if self.locals[local.0].variadic {
                self.error(
                    name.span,
                    format!(
                        "Variadic parameter \"{0}\" must be unpacked with \"{0}...\"",
                        name.text
                    ),
                );
                return Self::poisoned(name.span);
            }
            let ty = self.locals[local.0].ty.clone();
            return Self::typed(ExprKind::Local(local), ty, name.span);
        }
        if self.kind == Kind::CapField && self.cap_external && self.own_field(&name.text).is_some()
        {
            let owner = self
                .object
                .map(|object| self.reg.objects[object.0].name.clone())
                .unwrap_or_default();
            self.error(
                name.span,
                format!("External cap owner fields require {owner}:: qualification"),
            );
            return Self::poisoned(name.span);
        }
        if (matches!(
            self.kind,
            Kind::Method | Kind::Successor | Kind::Destroy | Kind::CapField
        ) || (self.kind == Kind::Closure && self.self_ty.is_some()))
            && let Some(field) = self.own_field(&name.text) {
                let ty = self.field_type(field);
                return Self::typed(ExprKind::Field(field), ty, name.span);
            }
        if let Some(id) = self.reg.find_const(self.module, &name.text) {
            return self.const_value(id, name.span);
        }
        if name.text == "test" {
            return Self::typed(ExprKind::TestCapability, Ty::Test, name.span);
        }
        // Выходы из цикла в других языках здесь читаются как имена (#256).
        let hint = match name.text.as_str() {
            "break" => "; Okkama leaves a loop with `last`",
            "continue" => "; Okkama starts the next iteration with `next`",
            _ => "",
        };
        self.error(name.span, format!("Undefined variable \"{}\"{hint}", name.text));
        Self::poisoned(name.span)
    }

    fn self_expr(&mut self, span: Span) -> Expr {
        match self.kind {
            Kind::Method | Kind::Successor | Kind::Destroy => {
                let ty = self.self_ty.clone().expect("actions have a receiver");
                Self::typed(ExprKind::SelfValue, ty, span)
            }
            Kind::Closure if self.self_ty.is_some() => {
                let ty = self.self_ty.clone().expect("checked");
                Self::typed(ExprKind::SelfValue, ty, span)
            }
            // Незавершённый receiver можно передать дальше; вызывать его
            // действия нельзя (§11.4).
            Kind::Ctor => {
                let ty = self.self_ty.clone().expect("constructors have a receiver");
                Self::typed(ExprKind::SelfValue, ty, span)
            }
            Kind::CapField => {
                self.error(span, "self is not available inside a cap field");
                Self::poisoned(span)
            }
            _ => {
                self.error(span, "`self` is available only inside object actions");
                Self::poisoned(span)
            }
        }
    }

    // ----- операторы -----

    fn unary(&mut self, op: ast::UnaryOp, operand: &ast::Expr, span: Span) -> Expr {
        // `-2.5` и `-7` — константы (сохраняют `-0.0`, точный минимум int).
        if op == ast::UnaryOp::Negate {
            match &operand.kind {
                ast::ExprKind::Literal(ast::Literal::Float(float)) => {
                    return match float_value(float) {
                        Some(value) if value == 0.0 && float.nonzero() => {
                            self.error(operand.span, "Float literal is too small: it would round to 0.0");
                            Self::poisoned(span)
                        }
                        Some(value) => Self::typed(
                            ExprKind::Constant(Constant::Float((-value).to_bits())),
                            Ty::Float,
                            span,
                        ),
                        None => {
                            self.error(operand.span, "Float value must contain a finite number");
                            Self::poisoned(span)
                        }
                    };
                }
                ast::ExprKind::Literal(ast::Literal::Int(int)) => {
                    return match int_value(int) {
                        Some(value) => Self::typed(
                            ExprKind::Constant(Constant::Int(
                                value.negated().to_string().to_string(),
                            )),
                            Ty::Int,
                            span,
                        ),
                        None => {
                            self.error(
                                operand.span,
                                "Integer literal cannot be represented exactly",
                            );
                            Self::poisoned(span)
                        }
                    };
                }
                _ => {}
            }
        }
        let value = self.value(operand, None);
        if value.diverges {
            return value;
        }
        let ty = Self::ty_of(&value).taken();
        if ty.is_error() {
            return Self::poisoned(span);
        }
        if op == ast::UnaryOp::Negate && matches!(ty, Ty::Object(..) | Ty::Role(..) | Ty::Param(_)) {
            return self.negation_call(value, span);
        }
        let (unary, result) = match (op, &ty) {
            (ast::UnaryOp::Not, Ty::Bool) => (UnaryOp::Not, Ty::Bool),
            (ast::UnaryOp::Negate, Ty::Int) => (UnaryOp::Negate(NumericKind::Int), Ty::Int),
            (ast::UnaryOp::Negate, Ty::Float) => (UnaryOp::Negate(NumericKind::Float), Ty::Float),
            (ast::UnaryOp::Negate, Ty::Rat) => (UnaryOp::Negate(NumericKind::Rat), Ty::Rat),
            // Арифметика над `byte` даёт `int` (§8.3).
            (ast::UnaryOp::Negate, Ty::Byte) => (UnaryOp::Negate(NumericKind::Byte), Ty::Int),
            (ast::UnaryOp::BitNot, Ty::Int) => (UnaryOp::BitNot { byte: false }, Ty::Int),
            (ast::UnaryOp::BitNot, Ty::Byte) => (UnaryOp::BitNot { byte: true }, Ty::Byte),
            (op, ty) => {
                let message = format!(
                    "Operator {} cannot be applied to {}",
                    op.symbol(),
                    self.type_name(ty)
                );
                self.error(span, message);
                return Self::poisoned(span);
            }
        };
        Self::typed(
            ExprKind::Unary {
                op: unary,
                operand: Box::new(value),
            },
            result,
            span,
        )
    }

    fn logical(
        &mut self,
        op: ast::BinaryOp,
        left: &ast::Expr,
        right: &ast::Expr,
        span: Span,
    ) -> Expr {
        let what = format!("Operand of {}", op.symbol());
        let lhs = self.value(left, Some(&Ty::Bool));
        self.expect(&lhs, &Ty::Bool, left.span, &what);
        let rhs = self.value(right, Some(&Ty::Bool));
        self.expect(&rhs, &Ty::Bool, right.span, &what);
        let diverges = lhs.diverges;
        let mut expr = Self::typed(
            ExprKind::Logical {
                and: op == ast::BinaryOp::And,
                left: Box::new(lhs),
                right: Box::new(rhs),
            },
            Ty::Bool,
            span,
        );
        expr.diverges = diverges;
        expr
    }

    /// Бинарный оператор над проверенными операндами. Разные числовые
    /// категории без явного каста не смешиваются (R02, D-046).
    pub(super) fn binary_values(
        &mut self,
        op: ast::BinaryOp,
        lhs: Expr,
        rhs: Expr,
        span: Span,
    ) -> Expr {
        use ast::BinaryOp as B;
        if lhs.diverges || rhs.diverges {
            let mut out = Self::typed(ExprKind::Unreachable, Ty::Error, span);
            out.diverges = true;
            return out;
        }
        // `[]`/`[:]` в сравнении получают тип типизированной коллекции другой
        // стороны, как ветви `join`.
        let (lhs, rhs) = if matches!(op, B::Equal | B::NotEqual) {
            self.retag_operands(lhs, rhs)
        } else {
            (lhs, rhs)
        };
        let (left, right) = (Self::ty_of(&lhs).taken(), Self::ty_of(&rhs).taken());
        if left == right && matches!(op, B::Equal | B::NotEqual)
            && let Ty::Variant(id, types) = &left {
                return self.variant_compare(lhs, rhs, *id, types.clone(), op == B::NotEqual, span);
            }
        if left.is_error() || right.is_error() {
            return Self::poisoned(span);
        }
        let arith = match op {
            B::Add => Some(ArithOp::Add),
            B::Subtract => Some(ArithOp::Sub),
            B::Multiply => Some(ArithOp::Mul),
            B::Divide => Some(ArithOp::Div),
            B::Remainder => Some(ArithOp::Mod),
            _ => None,
        };
        let bits = match op {
            B::BitAnd => Some(BitOp::And),
            B::BitOr => Some(BitOp::Or),
            B::BitXor => Some(BitOp::Xor),
            B::ShiftLeft => Some(BitOp::Shl),
            B::ShiftRight => Some(BitOp::Shr),
            B::ShiftRightUnsigned => Some(BitOp::Ushr),
            _ => None,
        };
        let compare = match op {
            B::Equal => Some(CompareOp::Eq),
            B::NotEqual => Some(CompareOp::Ne),
            B::Less => Some(CompareOp::Lt),
            B::LessEqual => Some(CompareOp::Le),
            B::Greater => Some(CompareOp::Gt),
            B::GreaterEqual => Some(CompareOp::Ge),
            _ => None,
        };
        let numeric = |ty: &Ty| match ty {
            Ty::Int => Some(NumericKind::Int),
            Ty::Byte => Some(NumericKind::Byte),
            Ty::Float => Some(NumericKind::Float),
            Ty::Rat => Some(NumericKind::Rat),
            _ => None,
        };
        let result: Result<(BinaryOp, Ty), String> = if let Some(arith) = arith {
            match (numeric(&left), numeric(&right), &left, &right) {
                (Some(kind), Some(other), ..) if kind == other => {
                    let ty = match (kind, arith) {
                        (NumericKind::Int | NumericKind::Byte, ArithOp::Div) => Ty::Rat,
                        (NumericKind::Byte, _) => Ty::Int,
                        (NumericKind::Int, _) => Ty::Int,
                        (NumericKind::Float, _) => Ty::Float,
                        (NumericKind::Rat, _) => Ty::Rat,
                    };
                    Ok((BinaryOp::Arith(kind, arith), ty))
                }
                (_, _, Ty::Str, Ty::Str) if arith == ArithOp::Add => {
                    Ok((BinaryOp::Concat, Ty::Str))
                }
                // Массивы и словари: `+` склеивает, `-` вычитает; оба операнда
                // одного типа (пустой литерал `[]`/`[:]` принимает тип другого).
                (_, _, Ty::Array(_), Ty::Array(_)) | (_, _, Ty::Map(..), Ty::Map(..))
                    if matches!(arith, ArithOp::Add | ArithOp::Sub) =>
                {
                    self.collection_operator(op, arith, &left, &right)
                }
                _ => Err(self.operator_error(op, &left, &right)),
            }
        } else if let Some(bits) = bits {
            let shift = matches!(bits, BitOp::Shl | BitOp::Shr | BitOp::Ushr);
            match (&left, &right) {
                (Ty::Int, Ty::Int) => Ok((
                    BinaryOp::Bits {
                        byte: false,
                        op: bits,
                    },
                    Ty::Int,
                )),
                (Ty::Byte, Ty::Byte) if !shift => Ok((
                    BinaryOp::Bits {
                        byte: true,
                        op: bits,
                    },
                    Ty::Byte,
                )),
                (Ty::Byte, Ty::Int) if shift => Ok((
                    BinaryOp::Bits {
                        byte: true,
                        op: bits,
                    },
                    Ty::Byte,
                )),
                (Ty::Int | Ty::Byte, Ty::Byte) | (Ty::Byte, Ty::Int) if shift => {
                    Err("Shift count must have type int".to_string())
                }
                (Ty::Int | Ty::Byte, Ty::Int | Ty::Byte) => {
                    Err("Bitwise operands must have the same int or byte category".to_string())
                }
                _ => Err(self.operator_error(op, &left, &right)),
            }
        } else {
            let compare = compare.expect("binary operators are arithmetic, bitwise or comparisons");
            let kind = if left == right { equality_kind(&left) } else { None };
            match kind {
                Some(kind) if compare.is_equality() => {
                    Ok((BinaryOp::Compare(kind, compare), Ty::Bool))
                }
                Some(
                    kind @ (CompareKind::Int
                    | CompareKind::Byte
                    | CompareKind::Float
                    | CompareKind::Rat),
                ) => Ok((BinaryOp::Compare(kind, compare), Ty::Bool)),
                _ if compare.is_equality() => Err(format!(
                    "Cannot compare {} and {}",
                    self.type_name(&left),
                    self.type_name(&right)
                )),
                _ => Err(self.operator_error(op, &left, &right)),
            }
        };
        match result {
            Ok((op, ty)) => Self::typed(
                ExprKind::Binary {
                    op,
                    left: Box::new(Self::consume(lhs)),
                    right: Box::new(Self::consume(rhs)),
                },
                ty,
                span,
            ),
            Err(message) => {
                self.error(span, message);
                Self::poisoned(span)
            }
        }
    }

    /// Whether `op` with this left operand goes through an operator role (#215):
    /// arithmetic or ordering with an object, a role or a type parameter.
    fn operator_operand(op: ast::BinaryOp, lhs: &Expr) -> bool {
        use ast::BinaryOp as B;
        matches!(
            op,
            B::Add | B::Subtract | B::Multiply | B::Divide | B::Remainder | B::Less | B::LessEqual | B::Greater | B::GreaterEqual
        ) && !lhs.diverges
            && matches!(Self::ty_of(lhs).taken(), Ty::Object(..) | Ty::Role(..) | Ty::Param(_))
    }

    /// The instances of `role` that `ty` declares, as role arguments: an
    /// object's `does`, a role reference of that role, or a type parameter
    /// bounded by it.
    fn declared_roles(&self, ty: &Ty, role: RoleId) -> Vec<Vec<Ty>> {
        match ty {
            Ty::Object(object, type_args) => {
                let info = &self.reg.objects[object.0];
                let env: Vec<(ParamId, Ty)> = info.params.iter().copied().zip(type_args.iter().cloned()).collect();
                info.does
                    .iter()
                    .filter(|(found, _)| *found == role)
                    .map(|(_, args)| args.iter().map(|arg| arg.subst(&env)).collect())
                    .collect()
            }
            Ty::Role(found, args) if *found == role => vec![args.clone()],
            Ty::Param(param) => match &self.reg.params[param.0].bound {
                Some(Ty::Role(found, args)) if *found == role => vec![args.clone()],
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    /// The action call an operator stands for, on an already checked left
    /// operand; `None` for the role name when the type has no such action.
    fn role_action(&mut self, target: Expr, action: &str, args: &[ast::Argument], span: Span) -> Expr {
        let name = ast::Name { text: action.to_string(), span };
        match Self::ty_of(&target).taken() {
            Ty::Object(object, type_args) => self.object_method(target, object, type_args, &name, args, false, span),
            Ty::Role(role, role_args) => self.role_method(target, role, role_args, &name, args, span),
            Ty::Param(param) => match self.reg.params[param.0].bound.clone() {
                Some(Ty::Role(role, role_args)) => self.role_method(target, role, role_args, &name, args, span),
                _ => Self::poisoned(span),
            },
            _ => Self::poisoned(span),
        }
    }

    /// `a op b` through the operator role of `a` (#215): `a.plus(b)`, …;
    /// `<`, `<=`, `>`, `>=` compare `a.order(b)` with an `Order` case. The
    /// right operand is checked by the action's own overload selection, and
    /// the chosen overload must be one the declared role instances name.
    fn operator_call(&mut self, op: ast::BinaryOp, lhs: Expr, right: &ast::Expr, span: Span) -> Expr {
        use ast::BinaryOp as B;
        let operators = self.reg.standard.operators;
        let (role, action, role_name) = match op {
            B::Add => (operators.adding, "plus", "Adding"),
            B::Subtract => (operators.subtracting, "minus", "Subtracting"),
            B::Multiply => (operators.multiplying, "times", "Multiplying"),
            B::Divide => (operators.dividing, "divided", "Dividing"),
            B::Remainder => (operators.remaindering, "remainder", "Remaindering"),
            _ => (self.reg.standard.comparing, "order", "Comparing"),
        };
        let left = Self::ty_of(&lhs).taken();
        let instances = role.map(|role| self.declared_roles(&left, role)).unwrap_or_default();
        if instances.is_empty() {
            let rhs = self.value(right, None);
            let right_ty = Self::ty_of(&rhs).taken();
            if !right_ty.is_error() {
                let message = format!(
                    "Operator {} cannot be applied to {} and {}: {} does not do {}",
                    op.symbol(),
                    self.type_name(&left),
                    self.type_name(&right_ty),
                    self.type_name(&left),
                    role_name
                );
                self.error(span, message);
            }
            return Self::poisoned(span);
        }
        let args = [ast::Argument { name: None, value: right.clone(), span: right.span }];
        let call = self.role_action(lhs, action, &args, span);
        let (operand, result) = match &call.kind {
            ExprKind::Call { args, .. } if !call.diverges => (
                args.first().map(|arg| Self::ty_of(arg).taken()),
                Self::ty_of(&call).taken(),
            ),
            _ => return call,
        };
        let comparison = !matches!(op, B::Add | B::Subtract | B::Multiply | B::Divide | B::Remainder);
        let declared = instances.iter().any(|args| {
            operand.as_ref() == args.first() && (comparison || Some(&result) == args.get(1))
        });
        if !declared {
            let message = format!(
                "Operator {} on {} calls {}({}), which is not one of the {} roles {} declares",
                op.symbol(),
                self.type_name(&left),
                action,
                operand.as_ref().map_or_else(String::new, |ty| self.type_name(ty)),
                role_name,
                self.type_name(&left)
            );
            self.error(span, message);
            return Self::poisoned(span);
        }
        if !comparison {
            return call;
        }
        let Some(order) = self.reg.standard.order else {
            return Self::poisoned(span);
        };
        let (case_name, negated) = match op {
            B::Less => ("less", false),
            B::LessEqual => ("greater", true),
            B::Greater => ("greater", false),
            _ => ("less", true),
        };
        let Some(case) = self.reg.enums[order.0].cases.iter().position(|case| case == case_name) else {
            return Self::poisoned(span);
        };
        let constant = Self::typed(ExprKind::EnumCase { ty: order, case }, Ty::Enum(order), span);
        self.binary_values(if negated { B::NotEqual } else { B::Equal }, call, constant, span)
    }

    /// `-a` through `Negating<R>` (#215).
    fn negation_call(&mut self, value: Expr, span: Span) -> Expr {
        let ty = Self::ty_of(&value).taken();
        let instances = self
            .reg
            .standard
            .operators
            .negating
            .map(|role| self.declared_roles(&ty, role))
            .unwrap_or_default();
        if instances.is_empty() {
            let message = format!(
                "Operator - cannot be applied to {}: {} does not do Negating",
                self.type_name(&ty),
                self.type_name(&ty)
            );
            self.error(span, message);
            return Self::poisoned(span);
        }
        let call = self.role_action(value, "negated", &[], span);
        if call.diverges || !matches!(call.kind, ExprKind::Call { .. }) {
            return call;
        }
        let result = Self::ty_of(&call).taken();
        if !instances.iter().any(|args| args.first() == Some(&result)) {
            let message = format!(
                "Operator - on {} gives {}, which is not the result its Negating role declares",
                self.type_name(&ty),
                self.type_name(&result)
            );
            self.error(span, message);
            return Self::poisoned(span);
        }
        call
    }

    /// `+`/`-` над массивами и словарями: тип результата — общий тип операндов.
    fn collection_operator(&self, op: ast::BinaryOp, arith: ArithOp, left: &Ty, right: &Ty) -> Result<(BinaryOp, Ty), String> {
        let ty = if left.is_untagged() {
            right.clone()
        } else if right.is_untagged() || left == right {
            left.clone()
        } else {
            return Err(format!(
                "Operator {} needs two collections of the same type, got {} and {}",
                op.symbol(),
                self.type_name(left),
                self.type_name(right)
            ));
        };
        let array = matches!(ty, Ty::Array(_));
        if arith == ArithOp::Sub && holds_any(&ty) {
            return Err(format!(
                "Operator - compares elements, and {} holds any, which cannot be compared",
                self.type_name(&ty)
            ));
        }
        let kind = match (array, arith) {
            (true, ArithOp::Add) => BinaryOp::ArrayConcat,
            (true, _) => BinaryOp::ArrayDifference,
            (false, ArithOp::Add) => BinaryOp::MapMerge,
            (false, _) => BinaryOp::MapDifference,
        };
        Ok((kind, ty))
    }

    fn operator_error(&self, op: ast::BinaryOp, left: &Ty, right: &Ty) -> String {
        format!(
            "Operator {} cannot be applied to {} and {}",
            op.symbol(),
            self.type_name(left),
            self.type_name(right)
        )
    }

    // ----- условия и циклы -----

    pub(super) fn if_parts(
        &mut self,
        condition: &ast::Expr,
        then_block: &ast::Block,
        else_part: Option<ElsePart<'_>>,
        want: bool,
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        let check = self.value(condition, Some(&Ty::Bool));
        self.expect(&check, &Ty::Bool, condition.span, "If condition");
        let then_value = self.block(then_block, want, expected);
        let else_value = else_part.map(|part| self.else_part(part, want, expected));
        // Литеральное условие: исполняемая ветвь известна, другая только проверяется.
        let (ty, diverges) = if check.diverges {
            (Some(Ty::Never), true)
        } else {
            match literal_condition(condition) {
                Some(true) => (then_value.ty.clone(), then_value.diverges),
                Some(false) => match &else_value {
                    Some(value) => (value.ty.clone(), value.diverges),
                    None => (None, false),
                },
                None => {
                    let absent = Self::unit(ExprKind::Unreachable, span);
                    let other = else_value.as_ref().unwrap_or(&absent);
                    self.join(&[&then_value, other], want, span)
                }
            }
        };
        let then_block = self.block_of(then_value);
        let else_block = else_value.map(|value| self.block_of(value));
        Expr {
            kind: ExprKind::If {
                condition: Box::new(check),
                then_block,
                else_block,
            },
            ty,
            diverges,
            span,
        }
    }

    fn loop_expr(
        &mut self,
        body: &ast::Block,
        want: bool,
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        self.loops.push(LoopCtx {
            functional: false,
            want,
            exits: Vec::new(),
            plain_exit: false,
            bare_last_ok: false,
            expected: expected.cloned(),
        });
        // Значение последнего оператора тела `loop` отбрасывается.
        let checked = self.block(body, false, None);
        let frame = self.loops.pop().expect("loop frame pushed above");
        let block = self.block_of(checked);
        if frame.exits.is_empty() && !frame.plain_exit {
            return Self::diverged(ExprKind::Loop(block), span);
        }
        let ty = if want && !frame.exits.is_empty() && !frame.plain_exit {
            let first = frame.exits[0].clone();
            if let Some(other) = frame
                .exits
                .iter()
                .find(|ty| **ty != first && !ty.is_error())
            {
                if !first.is_error() {
                    let message = format!(
                        "Branches produce different types: {} and {}",
                        self.type_name(&first),
                        self.type_name(other)
                    );
                    self.error(span, message);
                }
                Some(Ty::Error)
            } else {
                Some(first)
            }
        } else {
            None
        };
        Expr {
            kind: ExprKind::Loop(block),
            ty,
            diverges: false,
            span,
        }
    }

    // ----- match и try -----

    pub(super) fn match_body(
        &mut self,
        body: &ast::MatchBody,
        want: bool,
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        let subject = Self::consume(self.value(&body.subject, None));
        let subject_ty = Self::ty_of(&subject).taken();
        if let Ty::Variant(id, args) = &subject_ty {
            return self.variant_match(body, subject, *id, args.clone(), want, expected, span);
        }
        let mut cases = Vec::new();
        let mut arms: Vec<Expr> = Vec::new();
        for case in &body.cases {
            let mut patterns = Vec::new();
            for pattern in &case.patterns {
                let value = if let (Ty::Enum(id), ast::ExprKind::Get { object, name }) =
                    (&subject_ty, &pattern.kind)
                {
                    if matches!(&object.kind, ast::ExprKind::Name(n) if n.text == "$choice") {
                        if let Some(case) = self.reg.enums[id.0]
                            .cases
                            .iter()
                            .position(|c| c == &name.text)
                        {
                            Self::typed(
                                ExprKind::EnumCase { ty: *id, case },
                                subject_ty.clone(),
                                pattern.span,
                            )
                        } else {
                            self.error(pattern.span, "Unknown enum variant");
                            Self::poisoned(pattern.span)
                        }
                    } else {
                        self.value(pattern, Some(&subject_ty))
                    }
                } else {
                    self.value(pattern, Some(&subject_ty))
                };
                // Совпадение — тот же `==`, что и в выражениях.
                let probe =
                    Self::typed(ExprKind::Unreachable, subject_ty.clone(), body.subject.span);
                let _ =
                    self.binary_values(ast::BinaryOp::Equal, probe, value.clone(), pattern.span);
                patterns.push(value);
            }
            let out = self.block(&case.body, want, expected);
            arms.push(out.clone());
            cases.push((patterns, self.block_of(out)));
        }
        let exhaustive = body.else_block.is_none() && self.covers(&subject_ty, &body.cases);
        let else_value = body
            .else_block
            .as_ref()
            .map(|block| self.block(block, want, expected));
        let absent = Self::unit(ExprKind::Unreachable, span);
        let mut refs: Vec<&Expr> = arms.iter().collect();
        match &else_value {
            Some(value) => refs.push(value),
            None if !exhaustive => refs.push(&absent),
            None => {}
        }
        let (ty, diverges) = if subject.diverges {
            (Some(Ty::Never), true)
        } else {
            self.join(&refs, want, span)
        };
        let else_block = else_value.map(|value| self.block_of(value));
        Expr {
            kind: ExprKind::Match(Box::new(Match {
                subject,
                cases,
                else_block,
                exhaustive,
            })),
            ty,
            diverges,
            span,
        }
    }

    /// Образцы-константы покрывают все значения типа субъекта.
    fn covers(&mut self, ty: &Ty, cases: &[ast::MatchCase]) -> bool {
        let patterns: Vec<&ast::Expr> = cases.iter().flat_map(|case| &case.patterns).collect();
        match ty {
            Ty::Enum(id) => {
                let total = self.reg.enums[id.0].cases.len();
                let mut seen = vec![false; total];
                for pattern in patterns {
                    if let ast::ExprKind::Get { object, name } = &strip_grouping(pattern).kind
                        && let ast::ExprKind::Name(enum_name) = &strip_grouping(object).kind
                            && self.local(&enum_name.text).is_none()
                                && (enum_name.text == "$choice"
                                    || self.enum_named(&enum_name.text) == Some(*id))
                                && let Some(case) = self.reg.enums[id.0]
                                    .cases
                                    .iter()
                                    .position(|case| *case == name.text)
                                {
                                    seen[case] = true;
                                }
                }
                seen.iter().all(|seen| *seen)
            }
            Ty::Bool => {
                let mut seen = [false; 2];
                for pattern in patterns {
                    if let ast::ExprKind::Literal(ast::Literal::Bool(value)) =
                        strip_grouping(pattern).kind
                    {
                        seen[usize::from(value)] = true;
                    }
                }
                seen[0] && seen[1]
            }
            _ => false,
        }
    }

    pub(super) fn enum_named(&self, name: &str) -> Option<crate::analysis::ty::EnumId> {
        match self.reg.find_type(self.module, name) {
            Some(crate::analysis::registry::TypeRef::Enum(id)) => Some(id),
            _ => None,
        }
    }

    /// Тип `catch`: объект, делающий `Error`, error enum или роль `Error`.
    fn catch_type(&mut self, ty: &ast::TypeExpr) -> Option<CatchType> {
        let generics = self.generics.clone();
        let resolved = self.reg.resolve(ty, &generics);
        if resolved.is_error() {
            return None;
        }
        let catch = match &resolved {
            Ty::Object(object, args) if self.throwable(&resolved) => {
                Some(CatchType::Object(*object, args.clone()))
            }
            Ty::Role(role, args) if self.throwable(&resolved) => {
                Some(CatchType::Role(*role, args.clone()))
            }
            Ty::Variant(id, _) if self.throwable(&resolved) => Some(CatchType::Variant(*id)),
            _ => None,
        };
        if catch.is_none() {
            let message = format!(
                "Catch type \"{}\" does not satisfy Error; catch an error enum or Error",
                self.type_name(&resolved)
            );
            self.error(ty.span, message);
        }
        catch
    }

    /// Образец catch: вариант error enum (`Enum.case`, `Enum.case(x, _)`) или
    /// тип ошибки; привязки payload — для тела.
    #[allow(clippy::type_complexity)] // alternative and the enum case it binds
    fn catch_pattern(
        &mut self,
        pattern: &ast::Expr,
    ) -> Option<(CatchAlt, Option<(EnumId, usize, Vec<ast::Argument>)>)> {
        let (callee, args) = match &strip_grouping(pattern).kind {
            ast::ExprKind::Call { callee, args } => (&**callee, Some(args)),
            _ => (pattern, None),
        };
        if let ast::ExprKind::Get { object, name } = &strip_grouping(callee).kind
            && let Some((id, _)) = self.variant_receiver(object) {
                let decl = self.reg.enums[id.0].clone();
                if !decl.error {
                    self.error(
                        pattern.span,
                        format!("Catch pattern belongs to \"{}\", which is not an error enum", decl.name),
                    );
                    return None;
                }
                let Some(case) = decl.cases.iter().position(|c| *c == name.text) else {
                    self.error(name.span, "Unknown enum variant");
                    return None;
                };
                let given = args.map_or(0, Vec::len);
                if given != decl.payloads[case].len() {
                    self.error(pattern.span, "Wrong number of variant pattern bindings");
                    return None;
                }
                let alt = CatchAlt {
                    ty: CatchType::Variant(id),
                    cases: Some(vec![case]),
                };
                return Some((alt, Some((id, case, args.cloned().unwrap_or_default()))));
            }
        let path = args.is_none().then(|| expr_path(callee)).flatten();
        let Some(path) = path else {
            self.error(
                pattern.span,
                "Expected an error enum case or an error type in a catch pattern",
            );
            return None;
        };
        let ty = ast::TypeExpr {
            kind: ast::TypeKind::Named {
                path,
                args: Vec::new(),
            },
            span: pattern.span,
        };
        let ty = self.catch_type(&ty)?;
        Some((CatchAlt { ty, cases: None }, None))
    }

    /// Тип привязки catch как тип анализа.
    fn catch_binding_ty(ty: &CatchType) -> Ty {
        match ty {
            CatchType::Object(object, args) => Ty::Object(*object, args.clone()),
            CatchType::Role(role, args) => Ty::Role(*role, args.clone()),
            CatchType::Variant(id) => Ty::Variant(*id, Vec::new()),
        }
    }

    /// `try? value` — `Maybe<T>`: `some(value)` или `none`, если брошена ошибка;
    /// `try! value` — значение, брошенная ошибка становится panic. Это обычный
    /// `try` с `catch` по `Error`: panic и программные ошибки проходят насквозь.
    fn try_mark(
        &mut self,
        force: bool,
        value: &ast::Expr,
        want: bool,
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        let maybe = crate::analysis::ty::EnumId(0);
        let inner = match expected {
            Some(ty) if force => Some(ty.clone()),
            Some(Ty::Variant(id, args)) if *id == maybe => args.first().cloned(),
            _ => None,
        };
        self.push_scope();
        let checked = self.expr(value, want || !force, inner.as_ref());
        self.pop_scope();
        let operand = checked.ty.clone();
        let diverges = checked.diverges;
        let result = if force {
            operand.clone()
        } else {
            match &operand {
                None if !diverges => {
                    self.error(
                        value.span,
                        "try? needs a value; a void call is handled with try/catch",
                    );
                    return Self::poisoned(span);
                }
                Some(ty) if ty.is_successor() => {
                    self.error(
                        value.span,
                        "try? cannot wrap a successor result: its must-use obligation would be lost",
                    );
                    return Self::poisoned(span);
                }
                Some(ty) if ty.is_error() => return Self::poisoned(span),
                Some(Ty::Never) | None => Some(Ty::Error),
                Some(ty) => Some(Ty::Variant(maybe, vec![ty.clone()])),
            }
        };
        if result.as_ref().is_some_and(Ty::is_error) {
            return Self::poisoned(span);
        }
        let body_value = match &result {
            Some(maybe_ty) if !force => Self::typed(
                ExprKind::VariantNew {
                    case: 0,
                    values: vec![checked],
                },
                maybe_ty.clone(),
                span,
            ),
            _ => checked,
        };
        let body = crate::analysis::thir::Block {
            stmts: vec![crate::analysis::thir::Stmt {
                kind: crate::analysis::thir::StmtKind::Expr(body_value),
                span,
            }],
            span,
        };
        let error = self.reg.system.error;
        self.push_scope();
        let name = ast::Name {
            text: "$try".into(),
            span,
        };
        let local = self.declare(&name, Ty::role(error), false);
        let handled = if force {
            // panic "Unexpected error: " + $try.text()
            let text = ast::Expr {
                kind: ast::ExprKind::Call {
                    callee: Box::new(ast::Expr {
                        kind: ast::ExprKind::Get {
                            object: Box::new(ast::Expr {
                                kind: ast::ExprKind::Name(name.clone()),
                                span,
                            }),
                            name: ast::Name {
                                text: "text".into(),
                                span,
                            },
                        },
                        span,
                    }),
                    args: Vec::new(),
                },
                span,
            };
            let message = ast::Expr {
                kind: ast::ExprKind::Binary {
                    op: ast::BinaryOp::Add,
                    op_span: span,
                    left: Box::new(ast::Expr {
                        kind: ast::ExprKind::Literal(ast::Literal::String(
                            crate::syntax::token::StrLiteral {
                                value: "Unexpected error: ".into(),
                                form: crate::syntax::token::StringForm::Quoted,
                            },
                        )),
                        span,
                    }),
                    right: Box::new(text),
                },
                span,
            };
            let message = self.panic_message(&message);
            Self::diverged(ExprKind::Panic(Box::new(message)), span)
        } else {
            Self::typed(
                ExprKind::VariantNew {
                    case: 1,
                    values: Vec::new(),
                },
                result.clone().expect("try? has a Maybe result"),
                span,
            )
        };
        self.pop_scope();
        let catch = Catch {
            local,
            ty: CatchType::Role(error, Vec::new()),
            alternatives: vec![CatchAlt {
                ty: CatchType::Role(error, Vec::new()),
                cases: None,
            }],
            body: crate::analysis::thir::Block {
                stmts: vec![crate::analysis::thir::Stmt {
                    kind: crate::analysis::thir::StmtKind::Expr(handled),
                    span,
                }],
                span,
            },
            span,
        };
        Expr {
            kind: ExprKind::Try(Box::new(Try {
                body,
                catches: vec![catch],
            })),
            ty: result,
            diverges,
            span,
        }
    }

    pub(super) fn try_body(
        &mut self,
        body: &ast::TryBody,
        want: bool,
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        // Типы и образцы catch проверяются до тела.
        type Payload = Option<(EnumId, usize, Vec<ast::Argument>)>;
        let mut plans: Vec<Option<(Vec<CatchAlt>, Payload)>> = Vec::new();
        for clause in &body.catches {
            let mut alternatives = Vec::new();
            let mut payload = None;
            let mut valid = true;
            if clause.name.is_some() {
                for ty in &clause.types {
                    match self.catch_type(ty) {
                        Some(ty) => alternatives.push(CatchAlt { ty, cases: None }),
                        None => valid = false,
                    }
                }
            } else {
                for pattern in &clause.patterns {
                    match self.catch_pattern(pattern) {
                        Some((alt, found)) => {
                            let named = found.as_ref().is_some_and(|(_, _, args)| {
                                args.iter().any(|arg| {
                                    !matches!(&arg.value.kind, ast::ExprKind::Name(n) if n.text == "_")
                                })
                            });
                            if named && clause.patterns.len() > 1 {
                                self.error(
                                    pattern.span,
                                    "Payload bindings in a catch need a single pattern",
                                );
                                valid = false;
                            }
                            if clause.patterns.len() == 1 {
                                payload = found;
                            }
                            alternatives.push(alt);
                        }
                        None => valid = false,
                    }
                }
            }
            plans.push(valid.then_some((alternatives, payload)));
        }
        let main = self.block(&body.body, want, expected);
        let mut arms = vec![main.clone()];
        let mut catches = Vec::new();
        let error = self.reg.system.error;
        for (clause, plan) in body.catches.iter().zip(plans) {
            self.push_scope();
            let binding = match &plan {
                Some((alternatives, _)) if alternatives.len() == 1 => alternatives[0].ty.clone(),
                Some(_) => CatchType::Role(error, Vec::new()),
                None => CatchType::Role(error, Vec::new()),
            };
            let ty = if plan.is_some() {
                Self::catch_binding_ty(&binding)
            } else {
                Ty::Error
            };
            let name = clause.name.clone().unwrap_or_else(|| ast::Name {
                text: "$caught".into(),
                span: clause.span,
            });
            let local = self.declare(&name, ty.clone(), false);
            // Привязки payload образца `Enum.case(x, _)`.
            let mut bindings = Vec::new();
            if let Some((_, Some((id, case, args)))) = &plan {
                let decl = self.reg.enums[id.0].clone();
                let offset = 1 + decl.payloads[..*case].iter().map(Vec::len).sum::<usize>();
                let subject = Self::typed(ExprKind::Local(local), ty.clone(), clause.span);
                for (index, (arg, (_, field_ty))) in args.iter().zip(&decl.payloads[*case]).enumerate() {
                    let ast::ExprKind::Name(binding_name) = &arg.value.kind else {
                        self.error(arg.span, "Expected a binding or _ in variant pattern");
                        continue;
                    };
                    if arg.name.is_some() {
                        self.error(arg.span, "Pattern bindings are positional");
                    }
                    if binding_name.text == "_" {
                        continue;
                    }
                    let binding_local = self.declare(binding_name, field_ty.clone(), false);
                    let value = Self::typed(
                        ExprKind::VariantGet {
                            value: Box::new(subject.clone()),
                            field: offset + index,
                        },
                        field_ty.clone(),
                        arg.span,
                    );
                    bindings.push(Stmt {
                        kind: StmtKind::Let {
                            local: binding_local,
                            value,
                        },
                        span: arg.span,
                    });
                }
            }
            self.catch_depth += 1;
            let out = self.block(&clause.body, want, expected);
            self.catch_depth -= 1;
            self.pop_scope();
            arms.push(out.clone());
            if let Some((alternatives, _)) = plan {
                let mut block = self.block_of(out);
                bindings.append(&mut block.stmts);
                block.stmts = bindings;
                catches.push(Catch {
                    local,
                    ty: binding,
                    alternatives,
                    body: block,
                    span: clause.span,
                });
            }
        }
        let refs: Vec<&Expr> = arms.iter().collect();
        let (ty, diverges) = self.join(&refs, want, span);
        Expr {
            kind: ExprKind::Try(Box::new(Try {
                body: self.block_of(main),
                catches,
            })),
            ty,
            diverges,
            span,
        }
    }

    // ----- коллекции -----

    /// Границы диапазона — `int`; `byte` требует явного `.int()` (R02).
    pub(super) fn range_bounds(&mut self, start: &ast::Expr, end: &ast::Expr) -> (Expr, Expr) {
        let first = self.value(start, Some(&Ty::Int));
        self.expect(&first, &Ty::Int, start.span, "Range start");
        let last = self.value(end, Some(&Ty::Int));
        self.expect(&last, &Ty::Int, end.span, "Range end");
        (first, last)
    }

    /// `[a, b, 1..=3]`: элементы одного точного типа; диапазоны раскрываются.
    /// С целевым `@E` каждый элемент пересекает границу `E`; разнородные
    /// элементы без цели — `@any` с явной упаковкой каждого (R08).
    fn array_literal(&mut self, elements: &[ast::Expr], expected: Option<&Ty>, span: Span) -> Expr {
        // Цель — `@E` или роль массивов `Sequencing<E>`/`Iterating<E>`.
        let target = match expected {
            Some(Ty::Array(element)) if **element != Ty::Never => Some((**element).clone()),
            Some(Ty::Role(role, args))
                if args.len() == 1
                    && (Some(*role) == self.reg.standard.sequencing
                        || Some(*role) == self.reg.standard.iterating) =>
            {
                Some(args[0].clone())
            }
            _ => None,
        };
        enum Part {
            Value(Expr),
            Range(Expr, Expr, bool),
        }
        let mut parts = Vec::new();
        let mut diverges = false;
        for element in elements {
            if let ast::ExprKind::Range {
                inclusive,
                start,
                end,
                ..
            } = &element.kind
            {
                let (first, last) = self.range_bounds(start, end);
                diverges |= first.diverges || last.diverges;
                parts.push(Part::Range(first, last, *inclusive));
                continue;
            }
            let value = self.value(element, target.as_ref());
            diverges |= value.diverges;
            parts.push(Part::Value(value));
        }
        let types: Vec<Ty> = parts
            .iter()
            .filter_map(|part| match part {
                Part::Value(value) if !value.diverges => Some(Self::ty_of(value).taken()),
                Part::Value(_) => None,
                Part::Range(..) => Some(Ty::Int),
            })
            .collect();
        let element_type = match &target {
            Some(target) => Some(target.clone()),
            None if types.iter().any(Ty::is_error) => Some(Ty::Error),
            None => match types.first() {
                None => None,
                Some(first) if types.iter().all(|ty| ty == first) => Some(first.clone()),
                Some(_) => Some(Ty::Any),
            },
        };
        let Some(element_type) = element_type else {
            let mut out = Self::typed(ExprKind::Array(Vec::new()), Ty::array(Ty::Never), span);
            out.diverges = diverges;
            return out;
        };
        let mut checked = Vec::new();
        for (part, element) in parts.into_iter().zip(elements) {
            checked.push(match part {
                Part::Range(start, end, inclusive) => {
                    if element_type != Ty::Int
                        && element_type != Ty::Any
                        && !element_type.is_error()
                    {
                        let message = format!(
                            "Array element must be {}, got int",
                            self.type_name(&element_type)
                        );
                        self.error(element.span, message);
                    }
                    ArrayPart::Range {
                        start,
                        end,
                        inclusive,
                    }
                }
                // Сохранение в агрегат потребляет successor.
                Part::Value(value) => ArrayPart::Value(self.coerce(
                    value,
                    &element_type,
                    element.span,
                    "Array element",
                )),
            });
        }
        let mut out = Self::typed(ExprKind::Array(checked), Ty::array(element_type), span);
        out.diverges = diverges;
        out
    }

    /// `[k: v, …]`: ключи и значения — по одному точному типу; повтор ключа —
    /// последнее значение (R05). Разнородные значения без цели — `any` (R08).
    fn map_literal(
        &mut self,
        entries: &[ast::MapEntry],
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        let target = match expected {
            Some(Ty::Map(key, value)) if **key != Ty::Never => {
                Some(((**key).clone(), (**value).clone()))
            }
            _ => None,
        };
        let mut pairs = Vec::new();
        let mut diverges = false;
        for entry in entries {
            let key = self.value(&entry.key, target.as_ref().map(|(key, _)| key));
            let value = self.value(&entry.value, target.as_ref().map(|(_, value)| value));
            diverges |= key.diverges || value.diverges;
            pairs.push((key, value));
        }
        let keys: Vec<Ty> = pairs
            .iter()
            .map(|(key, _)| Self::ty_of(key).taken())
            .collect();
        let values: Vec<Ty> = pairs
            .iter()
            .map(|(_, value)| Self::ty_of(value).taken())
            .collect();
        let (key_type, value_type) = match &target {
            Some((key, value)) => (key.clone(), value.clone()),
            None if pairs.is_empty() => {
                let mut out = Self::typed(
                    ExprKind::Map(Vec::new()),
                    Ty::map(Ty::Never, Ty::Never),
                    span,
                );
                out.diverges = diverges;
                return out;
            }
            None => {
                let key = if keys.iter().any(Ty::is_error) {
                    Ty::Error
                } else if keys.iter().all(|ty| *ty == keys[0]) {
                    keys[0].clone()
                } else {
                    // Ключи — одного типа: `any` ключом не бывает (§6.2, D-047).
                    let other = keys
                        .iter()
                        .find(|ty| **ty != keys[0])
                        .expect("keys differ")
                        .clone();
                    let message = format!(
                        "Map keys have different types: {} and {}",
                        self.type_name(&keys[0]),
                        self.type_name(&other)
                    );
                    self.error(span, message);
                    Ty::Error
                };
                let value = if values.iter().any(Ty::is_error) {
                    Ty::Error
                } else if values.iter().all(|ty| *ty == values[0]) {
                    values[0].clone()
                } else {
                    Ty::Any
                };
                (key, value)
            }
        };
        // A literal keyed by a parameter bounds its argument like `%T:V` (#242).
        if let Ty::Param(param) = key_type {
            self.reg.params[param.0].key = true;
        }
        if let Some(message) = self.reg.key_problem(&key_type) {
            let at = entries.first().map_or(span, |entry| entry.key.span);
            self.error(at, message);
            return Self::poisoned(span);
        }
        let mut checked = Vec::new();
        for ((key, value), entry) in pairs.into_iter().zip(entries) {
            let key = self.coerce(key, &key_type, entry.key.span, "Map key");
            let value = self.coerce(value, &value_type, entry.value.span, "Map value");
            checked.push((key, value));
        }
        let mut out = Self::typed(ExprKind::Map(checked), Ty::map(key_type, value_type), span);
        out.diverges = diverges;
        out
    }

    /// `xs[i]` и `xs[i] else fallback` (fallback — только при промахе).
    fn index(
        &mut self,
        object: &ast::Expr,
        index: &ast::Expr,
        fallback: Option<&ast::Expr>,
        want: bool,
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        let target = Self::consume(self.value(object, None));
        let target_ty = Self::ty_of(&target).taken();
        let (key_type, element) = match &target_ty {
            Ty::Array(element) if **element != Ty::Never => (Ty::Int, (**element).clone()),
            Ty::Array(_) => {
                self.error(
                    object.span,
                    "An empty untyped array has no elements to index",
                );
                (Ty::Error, Ty::Error)
            }
            Ty::Map(key, _) if **key == Ty::Never => {
                // `[:][k] else fallback` — всегда fallback.
                if fallback.is_none() {
                    self.error(span, "An empty untyped map has no entries to index");
                }
                (Ty::Error, Ty::Never)
            }
            Ty::Map(key, value) => ((**key).clone(), (**value).clone()),
            Ty::Error => (Ty::Error, Ty::Error),
            Ty::Any => {
                self.error(span, "Cannot index any without narrowing");
                (Ty::Error, Ty::Error)
            }
            other => {
                let message = format!("Type {} cannot be indexed", self.type_name(other));
                self.error(object.span, message);
                (Ty::Error, Ty::Error)
            }
        };
        let position = self.value(index, Some(&key_type));
        let what = if matches!(target_ty, Ty::Array(_)) {
            "Array index"
        } else {
            "Map key"
        };
        if !key_type.is_error() {
            self.expect(&position, &key_type, index.span, what);
        }
        let found = Self::typed(ExprKind::Unreachable, element.clone(), span);
        let other = fallback.map(|fallback| {
            let hint = if element == Ty::Never {
                expected
            } else {
                Some(&element)
            };
            self.expr(fallback, true, hint)
        });
        let (ty, diverges) = match &other {
            None => (Some(element), false),
            Some(other) if element == Ty::Never => (other.ty.clone(), other.diverges),
            Some(other) => self.join(&[&found, other], want || true, span),
        };
        let diverges = diverges || target.diverges || position.diverges;
        Expr {
            kind: ExprKind::Index {
                object: Box::new(target),
                index: Box::new(position),
                fallback: other.map(Box::new),
            },
            ty,
            diverges,
            span,
        }
    }

    // ----- async -----

    fn await_expr(&mut self, value: &ast::Expr, want: bool, span: Span) -> Expr {
        if !self.is_async {
            self.error(span, "await is available only inside an async action");
        }
        let operand = self.value(value, None);
        let ty = Self::ty_of(&operand).taken();
        let result = match &ty {
            Ty::Future(Some(inner)) => Some((**inner).clone()),
            Ty::Future(None) => {
                if want {
                    self.error(
                        span,
                        "Await of an async void action cannot be used as a value.",
                    );
                }
                None
            }
            Ty::Error => Some(Ty::Error),
            other => {
                let message = format!(
                    "await operand must have type Future<T>, got {}",
                    self.type_name(other)
                );
                self.error(span, message);
                Some(Ty::Error)
            }
        };
        let diverges = operand.diverges;
        Expr {
            kind: ExprKind::Await(Box::new(operand)),
            ty: result,
            diverges,
            span,
        }
    }

    fn spawn(&mut self, call: &ast::Expr, span: Span) -> Expr {
        let direct = match &strip_grouping(call).kind {
            ast::ExprKind::Call { callee, .. } => matches!(
                &strip_grouping(callee).kind,
                ast::ExprKind::Get { object, .. }
                    if matches!(strip_grouping(object).kind, ast::ExprKind::Call { .. })
            ),
            _ => false,
        };
        let value = self.value(call, None);
        let bound = matches!(value.kind, ExprKind::CallValue { .. });
        let worker = !matches!(value.ty, Some(Ty::Future(_)));
        if bound && worker {
            self.reg.worker_actions = true;
        }
        if worker {
            self.check_transferable(&value, span);
        }
        if !direct && !bound {
            self.error(
                span,
                "spawn requires a direct object action or a typed bound action value",
            );
        }
        let ty = match Self::ty_of(&value) {
            future @ Ty::Future(_) => future,
            Ty::Error => Ty::Error,
            other => Ty::Future(Some(Box::new(other.taken()))),
        };
        Self::typed(ExprKind::Spawn(Box::new(value)), ty, span)
    }

    /// Синхронный `spawn` (§18.1): аргументы конструктора и действия и
    /// результат пересекают границу процесса — только скаляры, массивы и map
    /// из них. Параметры типа проверяет lowering после подстановки.
    fn check_transferable(&mut self, value: &Expr, span: Span) {
        let mut types: Vec<Ty> = Vec::new();
        match &value.kind {
            ExprKind::Call {
                callee: crate::analysis::thir::Callee::Method { receiver, .. },
                args,
            } => {
                if let ExprKind::Call { args: ctor, .. } = &receiver.kind {
                    types.extend(ctor.iter().map(Self::ty_of));
                }
                types.extend(args.iter().map(Self::ty_of));
            }
            ExprKind::CallValue { args, .. } => types.extend(args.iter().map(Self::ty_of)),
            _ => {}
        }
        if let Some(result) = &value.ty {
            types.push(result.taken());
        }
        for ty in types {
            if let Some(bad) = untransferable(&ty) {
                let message = format!(
                    "spawn cannot transfer value of type {}",
                    self.type_name(&bad)
                );
                self.error(span, message);
                return;
            }
        }
    }

    // ----- потоки и caps -----

    fn stream_write(&mut self, sink: &ast::Expr, source: &ast::Expr, span: Span) -> Expr {
        let sink_value = self.value(sink, None);
        let sink_ty = Self::ty_of(&sink_value).taken();
        let writing = self.reg.standard.writing.map(crate::analysis::ty::Ty::role);
        if !sink_ty.is_error() {
            let ok = writing
                .as_ref()
                .is_some_and(|role| self.reg.assignable(&sink_ty, role));
            if !ok {
                let message = format!(
                    "The left operand of <- must perform role \"Writing\", got {}",
                    self.type_name(&sink_ty)
                );
                self.error(sink.span, message);
            }
        }
        let (source_expr, unpack) = match &strip_grouping(source).kind {
            ast::ExprKind::Unpack { operand, .. } => (operand.as_ref(), true),
            _ => (source, false),
        };
        let source_value = self.value(source_expr, None);
        let source_ty = Self::ty_of(&source_value).taken();
        let item = if unpack {
            match &source_ty {
                Ty::Array(element) | Ty::Varargs(element) => (**element).clone(),
                Ty::Error => Ty::Error,
                other => {
                    let message = format!(
                        "Only arrays and variadic bindings can be unpacked with `...`, got {}",
                        self.type_name(other)
                    );
                    self.error(source_expr.span, message);
                    Ty::Error
                }
            }
        } else {
            source_ty
        };
        let byte_source = self.reg.standard.byte_source.map(Ty::role);
        let accepted = item.is_error()
            || item == Ty::Str
            || item == Ty::array(Ty::Byte)
            || byte_source
                .as_ref()
                .is_some_and(|role| self.reg.assignable(&item, role));
        if !accepted {
            let message = format!(
                "Stream source must be string, @byte or perform role \"ByteSource\", got {}; use an explicit conversion",
                self.type_name(&item)
            );
            self.error(source.span, message);
        }
        Self::typed(
            ExprKind::StreamWrite {
                sink: Box::new(sink_value),
                source: Box::new(source_value),
                unpack,
            },
            sink_ty,
            span,
        )
    }

    /// `Owner::field` в теле внешнего cap.
    fn owner_field(&mut self, owner: &ast::Name, name: &ast::Name, span: Span) -> Expr {
        let own = self
            .object
            .map(|object| self.reg.objects[object.0].name.clone());
        if self.kind != Kind::CapField
            || !self.cap_external
            || own.as_deref() != Some(owner.text.as_str())
        {
            self.error(
                owner.span,
                format!(
                    "\"{}::{}\" is available only in an external cap of {}",
                    owner.text, name.text, owner.text
                ),
            );
            return Self::poisoned(span);
        }
        match self.own_field(&name.text) {
            Some(field) => {
                let ty = self.field_type(field);
                Self::typed(ExprKind::OwnerField(field), ty, span)
            }
            None => {
                self.error(name.span, format!("Owner has no field \"{}\"", name.text));
                Self::poisoned(span)
            }
        }
    }

    /// `owner::Cap` и `cap::field`.
    fn select(&mut self, object: &ast::Expr, name: &ast::Name, span: Span) -> Expr {
        let owner = self.value(object, None);
        match Self::ty_of(&owner).taken() {
            Ty::Object(id, args) => {
                if self.kind == Kind::Ctor
                    && matches!(strip_grouping(object).kind, ast::ExprKind::SelfValue)
                {
                    self.error(
                        name.span,
                        "Cannot obtain a cap before construction completes",
                    );
                    return Self::poisoned(span);
                }
                match self.reg.objects[id.0]
                    .caps
                    .iter()
                    .position(|cap| cap.name == name.text)
                {
                    // Получение cap — использование владельца, как вызов на receiver.
                    Some(cap) => Self::typed(
                        ExprKind::CapSelect {
                            owner: Box::new(Self::consume(owner)),
                            object: id,
                            cap,
                        },
                        Ty::Cap(id, args, cap),
                        span,
                    ),
                    None => {
                        self.error(name.span, format!("Object has no cap \"{}\"", name.text));
                        Self::poisoned(span)
                    }
                }
            }
            Ty::Cap(id, args, cap) => {
                let info = &self.reg.objects[id.0];
                let Some(field) = info.caps[cap]
                    .fields
                    .iter()
                    .position(|(field, _, _)| *field == name.text)
                else {
                    self.error(
                        name.span,
                        format!("Cap has no computed field \"{}\"", name.text),
                    );
                    return Self::poisoned(span);
                };
                let env: Vec<_> = info
                    .params
                    .iter()
                    .copied()
                    .zip(args.iter().cloned())
                    .collect();
                let ty = info.caps[cap].fields[field].1.subst(&env);
                Self::typed(
                    ExprKind::CapRead {
                        cap: Box::new(owner),
                        object: id,
                        index: cap,
                        field,
                    },
                    ty,
                    span,
                )
            }
            Ty::Error => Self::poisoned(span),
            _ => {
                self.error(name.span, "Cap selection requires a full object reference");
                Self::poisoned(span)
            }
        }
    }
}

fn int_value(literal: &crate::syntax::token::IntLiteral) -> Option<IntValue> {
    match literal.radix {
        Radix::Decimal => IntValue::from_decimal_digits(&literal.digits),
        Radix::Binary | Radix::Hexadecimal => literal.to_u128().map(IntValue::from_u128),
    }
}

/// Конечный binary64 литерала `whole.fraction[e±exponent]`; экспонента любой
/// длины разбирается без раскрытия в цифры (`1e999999999` — бесконечность).
pub(super) fn float_value(literal: &crate::syntax::token::FloatLiteral) -> Option<f64> {
    literal
        .text()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

/// Тип, чьё значение при `==` всегда встречает `any`: `any` и коллекции из него.
fn holds_any(ty: &Ty) -> bool {
    match ty {
        Ty::Any => true,
        Ty::Array(element) => holds_any(element),
        Ty::Map(key, value) => holds_any(key) || holds_any(value),
        _ => false,
    }
}

/// `fake.storage.get(...)`: тип чтения хранилища fake даёт контекст.
fn is_fake_get(expr: &ast::Expr) -> bool {
    let ast::ExprKind::Call { callee, .. } = &strip_grouping(expr).kind else {
        return false;
    };
    let ast::ExprKind::Get { object, name } = &strip_grouping(callee).kind else {
        return false;
    };
    let ast::ExprKind::Get {
        object: inner,
        name: view,
    } = &strip_grouping(object).kind
    else {
        return false;
    };
    name.text == "get"
        && view.text == "storage"
        && matches!(&strip_grouping(inner).kind, ast::ExprKind::Name(fake) if fake.text == "fake")
}

/// Первый тип, не пересекающий границу процесса-воркера (§18.1).
fn untransferable(ty: &Ty) -> Option<Ty> {
    match ty {
        Ty::Int
        | Ty::Float
        | Ty::Str
        | Ty::Char
        | Ty::Byte
        | Ty::Bool
        | Ty::Param(_)
        | Ty::Error
        | Ty::Never => None,
        Ty::Array(element) => untransferable(element),
        Ty::Map(key, value) => untransferable(key).or_else(|| untransferable(value)),
        Ty::Varargs(element) => untransferable(element),
        other => Some(other.clone()),
    }
}

/// `a.b.C` как путь имени типа.
fn expr_path(expr: &ast::Expr) -> Option<Vec<ast::Name>> {
    match &strip_grouping(expr).kind {
        ast::ExprKind::Name(name) => Some(vec![name.clone()]),
        ast::ExprKind::Get { object, name } => {
            let mut path = expr_path(object)?;
            path.push(name.clone());
            Some(path)
        }
        _ => None,
    }
}

/// Как сравниваются на равенство два значения типа `ty` (`==`, `contains`).
/// `any` не распаковывается: коллекция из `any` не сравнима никогда.
pub(super) fn equality_kind(ty: &Ty) -> Option<CompareKind> {
    if holds_any(ty) {
        return None;
    }
    match ty {
        Ty::Int => Some(CompareKind::Int),
        Ty::Byte => Some(CompareKind::Byte),
        Ty::Float => Some(CompareKind::Float),
        Ty::Rat => Some(CompareKind::Rat),
        Ty::Bool => Some(CompareKind::Bool),
        Ty::Char => Some(CompareKind::Char),
        Ty::Str => Some(CompareKind::Str),
        Ty::Enum(_) => Some(CompareKind::Enum),
        Ty::Object(..)
        | Ty::Role(..)
        | Ty::Array(_)
        | Ty::Map(..)
        | Ty::Range
        | Ty::Param(_)
        | Ty::Future(_)
        | Ty::Action(_) => Some(CompareKind::Value),
        _ => None,
    }
}
