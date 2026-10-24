//! Выражения: литералы, имена, операторы, условные и составные формы.

use super::stmt::{ElsePart, literal_condition, strip_grouping};
use super::{Checker, Kind, LoopCtx};
use crate::analysis::thir::{
    ArithOp, ArrayPart, BinaryOp, BitOp, Catch, CatchType, CompareKind, CompareOp, Constant, Expr,
    ExprKind, Match, NumericKind, Try, UnaryOp,
};
use crate::analysis::ty::Ty;
use crate::ir::IntValue;
use crate::source::Span;
use crate::syntax::ast;
use crate::syntax::token::Radix;

impl Checker<'_, '_> {
    /// Выражение в позиции значения: ошибка, если значения нет.
    pub(super) fn value(&mut self, expr: &ast::Expr, expected: Option<&Ty>) -> Expr {
        let checked = self.expr(expr, true, expected);
        if checked.ty.is_none() && !checked.diverges {
            self.error(expr.span, "Expression does not produce a value");
            return Self::poisoned(expr.span);
        }
        checked
    }

    /// Выражение; `want` — значение используется, `expected` — тип границы,
    /// которого ждут (вывод generic-конструирования, тег пустых коллекций).
    pub(super) fn expr(&mut self, expr: &ast::Expr, want: bool, expected: Option<&Ty>) -> Expr {
        let span = expr.span;
        match &expr.kind {
            ast::ExprKind::Literal(literal) => self.literal(literal, span),
            ast::ExprKind::Name(name) => self.read_name(name),
            ast::ExprKind::SelfValue => self.self_expr(span),
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
                let rhs = if is_fake_get(right) {
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
            ast::ExprKind::Projection {
                prefix,
                construction,
            } => self.projection(prefix, construction, span),
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
        if matches!(
            self.kind,
            Kind::Method | Kind::Successor | Kind::Destroy | Kind::CapField
        ) || (self.kind == Kind::Closure && self.self_ty.is_some())
        {
            if let Some(field) = self.own_field(&name.text) {
                let ty = self.field_type(field);
                return Self::typed(ExprKind::Field(field), ty, name.span);
            }
        }
        if name.text == "test" {
            return Self::typed(ExprKind::TestCapability, Ty::Test, name.span);
        }
        self.error(name.span, format!("Undefined variable \"{}\"", name.text));
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
                            ExprKind::Constant(Constant::Int(value.negated().to_string().to_string())),
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
            // `any` не распаковывается: коллекция из `any` не сравнима никогда.
            let kind = if left == right && !holds_any(&left) {
                match &left {
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
            } else {
                None
            };
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
        let mut cases = Vec::new();
        let mut arms: Vec<Expr> = Vec::new();
        for case in &body.cases {
            let mut patterns = Vec::new();
            for pattern in &case.patterns {
                let value = self.value(pattern, Some(&subject_ty));
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
                    if let ast::ExprKind::Get { object, name } = &strip_grouping(pattern).kind {
                        if let ast::ExprKind::Name(enum_name) = &strip_grouping(object).kind {
                            if self.local(&enum_name.text).is_none()
                                && self.enum_named(&enum_name.text) == Some(*id)
                            {
                                if let Some(case) = self.reg.enums[id.0]
                                    .cases
                                    .iter()
                                    .position(|case| *case == name.text)
                                {
                                    seen[case] = true;
                                }
                            }
                        }
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

    /// Тип `catch`: объект, делающий `Error`, или роль `Error`.
    fn catch_type(&mut self, clause: &ast::CatchClause) -> Option<CatchType> {
        let generics = self.generics.clone();
        let ty = self.reg.resolve(&clause.ty, &generics);
        if ty.is_error() {
            return None;
        }
        let catch = match &ty {
            Ty::Object(object, args) if self.throwable(&ty) => {
                Some(CatchType::Object(*object, args.clone()))
            }
            Ty::Role(role, args) if self.throwable(&ty) => {
                Some(CatchType::Role(*role, args.clone()))
            }
            _ => None,
        };
        if catch.is_none() {
            let message = format!(
                "Catch type \"{}\" does not satisfy Error",
                self.type_name(&ty)
            );
            self.error(clause.ty.span, message);
        }
        catch
    }

    pub(super) fn try_body(
        &mut self,
        body: &ast::TryBody,
        want: bool,
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        // Типы catch проверяются до тела.
        let types: Vec<Option<CatchType>> = body
            .catches
            .iter()
            .map(|clause| self.catch_type(clause))
            .collect();
        let main = self.block(&body.body, want, expected);
        let mut arms = vec![main.clone()];
        let mut catches = Vec::new();
        for (clause, catch_type) in body.catches.iter().zip(types) {
            self.push_scope();
            let ty = match &catch_type {
                Some(CatchType::Object(object, args)) => Ty::Object(*object, args.clone()),
                Some(CatchType::Role(role, args)) => Ty::Role(*role, args.clone()),
                None => Ty::Error,
            };
            let local = self.declare(&clause.name, ty, false);
            self.catch_depth += 1;
            let out = self.block(&clause.body, want, expected);
            self.catch_depth -= 1;
            self.pop_scope();
            arms.push(out.clone());
            if let Some(ty) = catch_type {
                catches.push(Catch {
                    local,
                    ty,
                    body: self.block_of(out),
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
        if !key_type.is_error() && !key_type.is_map_key() {
            let message = format!(
                "Map key type {} is not a scalar key type",
                self.type_name(&key_type)
            );
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

/// Конечный binary64 литерала `whole.fraction`.
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
