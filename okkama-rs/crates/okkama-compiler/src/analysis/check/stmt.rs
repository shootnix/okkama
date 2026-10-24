//! Операторы: `let`, связывание полей и rebind, `return`, `confirm`,
//! `throw`, `next`/`last`, `defer`, постфиксное условие.

use super::{Checker, Kind};
use crate::analysis::thir::{Expr, ExprKind, Fallback, Stmt, StmtKind};
use crate::analysis::ty::Ty;
use crate::source::Span;
use crate::syntax::ast;

/// Итог оператора для потока блока: значение и уход управления.
#[derive(Clone, Debug)]
pub(in crate::analysis) struct Flow {
    pub ty: Option<Ty>,
    pub diverges: bool,
}

impl Flow {
    fn unit() -> Self {
        Flow {
            ty: None,
            diverges: false,
        }
    }

    fn diverged() -> Self {
        Flow {
            ty: Some(Ty::Never),
            diverges: true,
        }
    }

    fn of(expr: &Expr) -> Self {
        Flow {
            ty: expr.ty.clone(),
            diverges: expr.diverges,
        }
    }
}

/// `true`/`false`, в том числе в скобках.
pub(super) fn literal_condition(expr: &ast::Expr) -> Option<bool> {
    match strip_grouping(expr).kind {
        ast::ExprKind::Literal(ast::Literal::Bool(value)) => Some(value),
        _ => None,
    }
}

pub(super) fn strip_grouping(expr: &ast::Expr) -> &ast::Expr {
    match &expr.kind {
        ast::ExprKind::Grouping(inner) => strip_grouping(inner),
        _ => expr,
    }
}

impl Checker<'_, '_> {
    pub(super) fn stmt(
        &mut self,
        stmt: &ast::Stmt,
        want: bool,
        expected: Option<&Ty>,
    ) -> (Stmt, Flow) {
        let span = stmt.span;
        let (kind, flow) = match &stmt.kind {
            ast::StmtKind::Let { name, ty, value } => self.let_stmt(name, ty.as_ref(), value),
            ast::StmtKind::Expr {
                expr,
                condition: None,
            } => {
                let value = self.expr(expr, want, expected);
                let flow = Flow::of(&value);
                (StmtKind::Expr(value), flow)
            }
            ast::StmtKind::Expr {
                expr,
                condition: Some(condition),
            } => self.guarded(condition, "Postfix if condition", span, |checker| {
                let value = checker.expr(expr, false, None);
                let flow = Flow::of(&value);
                (StmtKind::Expr(value), flow)
            }),
            ast::StmtKind::Return { value } => self.return_stmt(value.as_ref(), span),
            ast::StmtKind::Confirm {
                condition,
                fallback,
            } => self.confirm(condition, fallback.as_ref()),
            ast::StmtKind::Block(block) => {
                let value = self.block(block, want, expected);
                let flow = Flow::of(&value);
                (StmtKind::Expr(value), flow)
            }
            ast::StmtKind::If(if_stmt) => {
                let value = self.if_stmt(if_stmt, want, expected);
                let flow = Flow::of(&value);
                (StmtKind::Expr(value), flow)
            }
            ast::StmtKind::Match(body) => {
                let value = self.match_body(body, want, expected, span);
                let flow = Flow::of(&value);
                (StmtKind::Expr(value), flow)
            }
            ast::StmtKind::Try(body) => {
                let value = self.try_body(body, want, expected, span);
                let flow = Flow::of(&value);
                (StmtKind::Expr(value), flow)
            }
            ast::StmtKind::For {
                source,
                bindings,
                body,
            } => self.for_stmt(source, bindings, body, want, expected, span),
            ast::StmtKind::Next { condition } => self.next(condition.as_ref(), span),
            ast::StmtKind::Last { value, condition } => {
                self.last(value.as_ref(), condition.as_ref(), span)
            }
            ast::StmtKind::FieldBind { name, value } => {
                // В конструкторе имя поля важнее одноимённой локальной привязки.
                if self.kind == Kind::Ctor {
                    if let Some(field) = self.own_field(&name.text) {
                        let (kind, flow) = self.bind_field(field, value, false);
                        return (Stmt { kind, span }, flow);
                    }
                }
                self.rebind(name, value)
            }
            ast::StmtKind::SelfFieldBind {
                receiver,
                name,
                value,
            } => {
                if self.kind != Kind::Ctor {
                    self.error(*receiver, "Fields can be bound only in constructors");
                    let _ = self.value(value, None);
                    (StmtKind::Expr(Self::poisoned(span)), Flow::unit())
                } else {
                    match self.own_field(&name.text) {
                        Some(field) => self.bind_field(field, value, false),
                        None => {
                            self.no_field(name);
                            let _ = self.value(value, None);
                            (StmtKind::Expr(Self::poisoned(span)), Flow::unit())
                        }
                    }
                }
            }
            ast::StmtKind::SuccessorAssign { name, value } => {
                let allowed = match self.kind {
                    Kind::Destroy => {
                        self.error(span, "Successor assignment is not allowed inside destroy");
                        false
                    }
                    Kind::Successor => true,
                    _ => {
                        self.error(
                            span,
                            "`^field = value` is allowed only in successor actions",
                        );
                        false
                    }
                };
                match self.own_field(&name.text) {
                    Some(field) if allowed => self.bind_field(field, value, true),
                    found => {
                        if allowed && found.is_none() {
                            self.no_field(name);
                        }
                        let _ = self.value(value, None);
                        (StmtKind::Expr(Self::poisoned(span)), Flow::unit())
                    }
                }
            }
            ast::StmtKind::Defer { value } => {
                let value = self.expr(value, false, None);
                (StmtKind::Defer(value), Flow::unit())
            }
            ast::StmtKind::Throw { value } => self.throw(value.as_ref(), span),
        };
        (Stmt { kind, span }, flow)
    }

    pub(super) fn own_field(&self, name: &str) -> Option<usize> {
        let object = self.object?;
        self.reg.objects[object.0]
            .fields
            .iter()
            .position(|field| field.name == name)
    }

    pub(super) fn no_field(&mut self, name: &ast::Name) {
        let object = self.object.expect("fields belong to an object");
        let message = format!(
            "Object \"{}\" has no field \"{}\"",
            self.reg.objects[object.0].name, name.text
        );
        self.error(name.span, message);
    }

    /// Тип поля объекта тела (в терминах его параметров).
    pub(super) fn field_type(&self, field: usize) -> Ty {
        let object = self.object.expect("fields belong to an object");
        self.reg.objects[object.0].fields[field].ty.clone()
    }

    fn let_stmt(
        &mut self,
        name: &ast::Name,
        ty: Option<&ast::TypeExpr>,
        value: &ast::Expr,
    ) -> (StmtKind, Flow) {
        let declared = ty.map(|ty| {
            let generics = self.generics.clone();
            self.reg.resolve(ty, &generics)
        });
        // `let xs = values...;` — новый массив из распаковки (§11.8.1).
        let checked = match &strip_grouping(value).kind {
            ast::ExprKind::Unpack { operand, .. } => self.unpacked(operand, value.span),
            _ => self.value(value, declared.as_ref()),
        };
        let checked = match &declared {
            // Типизированная привязка `C` не гасит `^C`; роль — граница (`role.wrap`).
            Some(declared) if checked.ty.as_ref().map(Ty::taken).as_ref() != Some(declared) => {
                let actual = Self::ty_of(&checked);
                let message = format!(
                    "Cannot initialize variable \"{}\" of type {} with {}",
                    name.text,
                    self.type_name(declared),
                    self.type_name(&actual)
                );
                self.narrow_message = Some(message.clone());
                let coerced = self.try_coerce(checked, declared, "Variable");
                self.narrow_message = None;
                match coerced {
                    Ok(value) => value,
                    Err(_) => {
                        if !actual.is_error() && !declared.is_error() {
                            self.error(value.span, message);
                        }
                        Self::poisoned(value.span)
                    }
                }
            }
            _ => checked,
        };
        if checked.diverges {
            let local = self.declare(name, Ty::Error, false);
            return (
                StmtKind::Let {
                    local,
                    value: checked,
                },
                Flow::diverged(),
            );
        }
        let ty = match &checked.ty {
            Some(ty) => ty.clone(),
            None => {
                self.error(value.span, "Expression does not produce a value");
                Ty::Error
            }
        };
        let local = self.declare(name, ty.clone(), false);
        if declared.is_none() && matches!(checked.kind, ExprKind::BoundAction { .. }) {
            self.unbounded.push(local);
        }
        (
            StmtKind::Let {
                local,
                value: checked,
            },
            Flow {
                ty: Some(ty),
                diverges: false,
            },
        )
    }

    /// Связать поле строящегося объекта или staged-поле successor.
    fn bind_field(&mut self, field: usize, value: &ast::Expr, staged: bool) -> (StmtKind, Flow) {
        let ty = self.field_type(field);
        let checked = match &strip_grouping(value).kind {
            ast::ExprKind::Unpack { operand, .. } => self.unpacked(operand, value.span),
            _ => self.value(value, Some(&ty)),
        };
        let object = self.object.expect("field binds belong to an object");
        let name = self.reg.objects[object.0].fields[field].name.clone();
        let diverges = checked.diverges;
        let checked = self.coerce(checked, &ty, value.span, &format!("Field \"{name}\""));
        let kind = if staged {
            StmtKind::StageField {
                field,
                value: checked,
            }
        } else {
            StmtKind::BindField {
                field,
                value: checked,
            }
        };
        (
            kind,
            if diverges {
                Flow::diverged()
            } else {
                Flow::unit()
            },
        )
    }

    fn rebind(&mut self, name: &ast::Name, value: &ast::Expr) -> (StmtKind, Flow) {
        let Some(local) = self.local(&name.text) else {
            self.error(name.span, format!("Undefined variable \"{}\"", name.text));
            let _ = self.value(value, None);
            return (StmtKind::Expr(Self::poisoned(name.span)), Flow::unit());
        };
        let current = self.locals[local.0].ty.taken();
        if !self.locals[local.0].rebindable && !current.is_error() {
            let message = format!(
                "Cannot rebind variable \"{}\": type \"{}\" has no successor actions",
                name.text,
                self.type_name(&current)
            );
            self.error(name.span, message);
            let _ = self.value(value, None);
            return (StmtKind::Expr(Self::poisoned(name.span)), Flow::unit());
        }
        let checked = self.value(value, Some(&current));
        let diverges = checked.diverges;
        let checked = self.coerce(
            checked,
            &current,
            value.span,
            &format!("Variable \"{}\"", name.text),
        );
        (
            StmtKind::Rebind {
                local,
                value: checked,
            },
            if diverges {
                Flow::diverged()
            } else {
                Flow::unit()
            },
        )
    }

    /// `stmt if condition`; литеральное условие известно заранее.
    pub(super) fn guarded(
        &mut self,
        condition: &ast::Expr,
        what: &str,
        span: Span,
        then: impl FnOnce(&mut Self) -> (StmtKind, Flow),
    ) -> (StmtKind, Flow) {
        let check = self.value(condition, Some(&Ty::Bool));
        self.expect(&check, &Ty::Bool, condition.span, what);
        let (kind, flow) = then(self);
        let flow = match literal_condition(condition) {
            Some(true) => Flow {
                ty: None,
                diverges: flow.diverges,
            },
            _ if check.diverges => Flow::diverged(),
            _ => Flow::unit(),
        };
        (
            StmtKind::Guarded {
                condition: check,
                stmt: Box::new(Stmt { kind, span }),
            },
            flow,
        )
    }

    fn return_stmt(&mut self, value: Option<&ast::Expr>, span: Span) -> (StmtKind, Flow) {
        match self.kind {
            Kind::Ctor => {
                self.error(span, "`return` is not allowed in a constructor");
                return (StmtKind::Return(None), Flow::diverged());
            }
            Kind::Successor => {
                if let Some(expr) = value {
                    self.error(
                        expr.span,
                        "A successor action cannot return a replacement value; use bare `return;`",
                    );
                }
                return (StmtKind::Return(None), Flow::diverged());
            }
            Kind::Destroy | Kind::Test => {
                self.error(span, "return is only allowed inside an action");
                return (StmtKind::Return(None), Flow::diverged());
            }
            Kind::Entry | Kind::Method | Kind::Default | Kind::CapField | Kind::Closure => {}
        }
        let result = self.result.clone();
        let checked = match (value, &result) {
            (Some(expr), Some(ty)) => {
                let checked = self.value(expr, Some(ty));
                Some(self.coerce(checked, ty, expr.span, "Return value"))
            }
            (None, None) => None,
            (Some(expr), None) => {
                self.error(expr.span, "A void action cannot return a value");
                let _ = self.expr(expr, false, None);
                None
            }
            (None, Some(ty)) => {
                let message = format!("`return;` requires a value of type {}", self.type_name(ty));
                self.error(span, message);
                None
            }
        };
        (StmtKind::Return(checked), Flow::diverged())
    }

    fn confirm(
        &mut self,
        condition: &ast::Expr,
        fallback: Option<&ast::ConfirmFallback>,
    ) -> (StmtKind, Flow) {
        let check = self.value(condition, Some(&Ty::Bool));
        self.expect(&check, &Ty::Bool, condition.span, "Confirm condition");
        let fallback = fallback.map(|fallback| match fallback {
            ast::ConfirmFallback::Panic { value, .. } => Fallback::Panic(self.panic_message(value)),
            ast::ConfirmFallback::Throw { value, .. } => Fallback::Throw(self.thrown(value)),
        });
        let flow = if check.diverges {
            Flow::diverged()
        } else {
            Flow::unit()
        };
        (
            StmtKind::Confirm {
                condition: check,
                fallback,
            },
            flow,
        )
    }

    pub(super) fn panic_message(&mut self, value: &ast::Expr) -> Expr {
        let message = self.value(value, Some(&Ty::Str));
        self.expect(&message, &Ty::Str, value.span, "Panic message");
        message
    }

    /// Можно ли бросить и поймать значение этого типа (`does Error`).
    pub(super) fn throwable(&self, ty: &Ty) -> bool {
        let error = self.reg.system.error;
        match ty.taken() {
            Ty::Role(role, _) => role == error,
            other @ Ty::Object(..) => self
                .reg
                .roles_of(&other)
                .iter()
                .any(|role| matches!(role, Ty::Role(found, _) if *found == error)),
            Ty::Param(_) => self
                .reg
                .roles_of(&ty.taken())
                .iter()
                .any(|role| matches!(role, Ty::Role(found, _) if *found == error)),
            _ => false,
        }
    }

    fn thrown(&mut self, value: &ast::Expr) -> Expr {
        let error = self.value(value, None);
        let ty = Self::ty_of(&error);
        if !error.diverges && !ty.is_error() && !self.throwable(&ty) {
            let message = format!(
                "Type \"{}\" cannot be thrown; only objects that do Error are catchable",
                self.type_name(&ty.taken())
            );
            self.error(value.span, message);
        }
        // `throw` потребляет successor, как `return`.
        Self::consume(error)
    }

    fn throw(&mut self, value: Option<&ast::Expr>, span: Span) -> (StmtKind, Flow) {
        match value {
            None => {
                if self.catch_depth == 0 {
                    self.error(span, "\"throw;\" is available only inside a catch body");
                }
                (StmtKind::Rethrow, Flow::diverged())
            }
            Some(value) => {
                let error = self.thrown(value);
                (StmtKind::Throw(error), Flow::diverged())
            }
        }
    }

    fn innermost_loop(&mut self, span: Span, keyword: &str) -> Option<usize> {
        let Some(frame) = self.loops.len().checked_sub(1) else {
            self.error(span, format!("`{keyword}` used outside of a loop"));
            return None;
        };
        if self.loops[frame].functional {
            self.error(
                span,
                format!("`{keyword}` cannot be used directly in a functional block"),
            );
            return None;
        }
        Some(frame)
    }

    fn next(&mut self, condition: Option<&ast::Expr>, span: Span) -> (StmtKind, Flow) {
        let _ = self.innermost_loop(span, "next");
        match condition {
            None => (StmtKind::Next, Flow::diverged()),
            Some(condition) => self.guarded(condition, "Next condition", span, |_| {
                (StmtKind::Next, Flow::diverged())
            }),
        }
    }

    fn last(
        &mut self,
        value: Option<&ast::Expr>,
        condition: Option<&ast::Expr>,
        span: Span,
    ) -> (StmtKind, Flow) {
        let frame = self.innermost_loop(span, "last");
        let leave = |checker: &mut Self| {
            let want = frame.is_some_and(|frame| checker.loops[frame].want);
            let target = frame.and_then(|frame| checker.loops[frame].expected.clone());
            let checked = value.map(|expr| {
                let checked = checker.expr(expr, want, target.as_ref());
                let checked = match (&target, want) {
                    (Some(target), true) if checked.ty.is_some() && !checked.diverges => {
                        checker.coerce(checked, target, expr.span, "Loop value")
                    }
                    _ => checked,
                };
                if !checked.diverges {
                    match (&checked.ty, want) {
                        (Some(ty), true) => {
                            if let Some(frame) = frame {
                                checker.loops[frame].exits.push(ty.clone());
                            }
                        }
                        (None, true) => {
                            checker.error(expr.span, "Expression does not produce a value");
                        }
                        _ => {}
                    }
                }
                checked
            });
            if value.is_none() {
                let keeps = frame.is_some_and(|frame| checker.loops[frame].bare_last_ok);
                if want && !keeps {
                    checker.error(span, "Bare `last` cannot produce the value of this loop");
                }
                if let Some(frame) = frame {
                    checker.loops[frame].plain_exit = true;
                }
            }
            (StmtKind::Last(checked), Flow::diverged())
        };
        match condition {
            None => leave(self),
            Some(condition) => self.guarded(condition, "Last condition", span, leave),
        }
    }

    /// Блок `if` в позиции оператора: `else if` — вложенный `if`.
    pub(super) fn if_stmt(
        &mut self,
        stmt: &ast::IfStmt,
        want: bool,
        expected: Option<&Ty>,
    ) -> Expr {
        let else_part = match &stmt.else_branch {
            None => None,
            Some(ast::ElseBranch::Block(block)) => Some(ElsePart::Block(block)),
            Some(ast::ElseBranch::If(nested)) => Some(ElsePart::If(nested)),
        };
        self.if_parts(
            &stmt.condition,
            &stmt.then_block,
            else_part,
            want,
            expected,
            stmt.span,
        )
    }

    pub(super) fn else_part(
        &mut self,
        part: ElsePart<'_>,
        want: bool,
        expected: Option<&Ty>,
    ) -> Expr {
        match part {
            ElsePart::Block(block) => self.block(block, want, expected),
            ElsePart::If(nested) => {
                let value = self.if_stmt(nested, want, expected);
                let span = value.span;
                let (ty, diverges) = (value.ty.clone(), value.diverges);
                Expr {
                    kind: ExprKind::Block(crate::analysis::thir::Block {
                        stmts: vec![Stmt {
                            kind: StmtKind::Expr(value),
                            span,
                        }],
                        span,
                    }),
                    ty,
                    diverges,
                    span,
                }
            }
        }
    }
}

#[derive(Copy, Clone)]
pub(super) enum ElsePart<'a> {
    Block(&'a ast::Block),
    If(&'a ast::IfStmt),
}
