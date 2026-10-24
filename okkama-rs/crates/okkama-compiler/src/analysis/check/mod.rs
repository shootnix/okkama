//! Проверка тел: имена, типы, вызовы, управление потоком.
//!
//! [`Checker`] обходит одно тело и строит его THIR. Каждая конструкция даёт
//! [`Expr`]: значение типа `ty`, отсутствие значения (`ty: None`) или уход
//! управления (`diverges`). Ошибка сообщается один раз и превращает значение
//! в `Ty::Error`, с которым дальнейшие проверки молчат, — так находятся все
//! независимые ошибки тела, а не только первая.
//!
//! Код после ухода управления проверяется так же, как живой, но его типы не
//! участвуют в слиянии ветвей.

mod builtin;
mod call;
mod collections;
mod expr;
mod generics;
mod stmt;

use std::collections::HashMap;

use super::registry::{CtorShape, Registry};
use super::thir::{
    Block, Body, BodyOwner, Coercion, Expr, ExprKind, Local, LocalId, Stmt, StmtKind,
};
use super::ty::{ObjectId, ParamId, Ty};
use crate::source::Span;
use crate::syntax::ast;

/// Вид тела: от него зависят `self`, `return`, связывание полей.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Entry,
    Method,
    Successor,
    Ctor,
    Default,
    Destroy,
    CapField,
    Closure,
    Test,
}

/// Активный цикл: цель `next`/`last` и типы значений `last`.
#[derive(Clone, Debug)]
pub(super) struct LoopCtx {
    /// Тело `map`/`filter`/`reduce`: `next`/`last` запрещены.
    pub functional: bool,
    /// Значение цикла используется (`loop` в позиции значения).
    pub want: bool,
    /// Типы значений `last value` (по порядку).
    pub exits: Vec<Ty>,
    /// Цикл имеет `last` без значения или выход по исчерпанию.
    pub plain_exit: bool,
    /// Bare `last` сохраняет накопленное значение (`for`), а не запрещён (`loop`).
    pub bare_last_ok: bool,
    /// Ожидаемый тип значения цикла: граница значений `last`.
    pub expected: Option<Ty>,
}

/// Видимые снаружи локальные для тела анонимного действия.
#[derive(Clone, Debug, Default)]
pub(super) struct Outer {
    /// Имя → (локальная внешнего тела, тип).
    pub visible: HashMap<String, (LocalId, Ty)>,
    pub self_ty: Option<Ty>,
}

pub(super) struct Checker<'r, 'a> {
    pub reg: &'r mut Registry<'a>,
    pub module: usize,
    pub kind: Kind,
    pub object: Option<ObjectId>,
    /// Видимые параметры типа (generic-объявление).
    pub generics: Vec<(String, ParamId)>,
    /// Тип `self` (объект с параметрами как аргументами).
    pub self_ty: Option<Ty>,
    /// Объявленный результат тела; `None` — `void`.
    pub result: Option<Ty>,
    pub is_async: bool,
    pub locals: Vec<Local>,
    pub scopes: Vec<HashMap<String, LocalId>>,
    pub loops: Vec<LoopCtx>,
    /// Глубина тел `catch`: `throw;` допустим только внутри.
    pub catch_depth: usize,
    /// Текст ошибки `role.narrow` для текущей границы (`let x is R = …`).
    pub narrow_message: Option<String>,
    /// Локальные `let x = obj.action;` без типа: bound action ещё не пересёк
    /// границу типа action и не вызывается (§8.10.1).
    pub unbounded: Vec<LocalId>,
    /// Внешнее тело анонимного действия.
    pub outer: Option<Outer>,
    pub captures: Vec<(LocalId, LocalId)>,
    /// Поля, которые тело shorthand-конструктора читает по имени.
    pub shorthand_fields: Vec<usize>,
    /// Тело поля внешнего cap: поля владельца читаются как `Owner::field`.
    pub cap_external: bool,
}

/// Проверить все тела программы: зависимости раньше корня, в порядке исходника.
pub(super) fn check_bodies(reg: &mut Registry<'_>) -> Vec<Body> {
    let mut bodies = Vec::new();
    for module in 0..reg.modules.len() {
        reg.set_module(module);
        let objects: Vec<ObjectId> = (0..reg.objects.len())
            .map(ObjectId)
            .filter(|id| reg.objects[id.0].module == module && reg.objects[id.0].decl.is_some())
            .collect();
        for object in objects {
            check_object(reg, object, &mut bodies);
        }
        for index in 0..reg.tests.len() {
            let test = reg.tests[index].clone();
            if test.module != module {
                continue;
            }
            let mut checker = Checker::new(reg, Kind::Test, None, None);
            checker.module = module;
            checker.reg.set_module(module);
            checker.is_async = test.is_async;
            let block = checker.body(test.body, &test.name);
            bodies.push(checker.finish(BodyOwner::Test(index), Vec::new(), block, test.span));
        }
        if module == reg.entry_module {
            if let Some(entry) = reg.entry {
                let mut checker = Checker::new(reg, Kind::Entry, None, Some(Ty::Int));
                checker.is_async = entry.is_async;
                let block = checker.body(&entry.body, &entry.name.text);
                bodies.push(checker.finish(BodyOwner::Entry, Vec::new(), block, entry.span));
            }
        }
    }
    bodies
}

fn check_object(reg: &mut Registry<'_>, object: ObjectId, bodies: &mut Vec<Body>) {
    // Defaults: в области объявления, без локальных вызывающего (§10).
    for field in 0..reg.objects[object.0].fields.len() {
        let info = &reg.objects[object.0].fields[field];
        let (Some(default), ty) = (info.default, info.ty.clone()) else {
            continue;
        };
        let mut checker = Checker::new(reg, Kind::Default, Some(object), Some(ty.clone()));
        let value = checker.value(default, Some(&ty));
        let value = checker.coerce(value, &ty, default.span, "Field default");
        let block = Block {
            stmts: vec![Stmt {
                kind: StmtKind::Expr(value),
                span: default.span,
            }],
            span: default.span,
        };
        bodies.push(checker.finish(
            BodyOwner::Default { object, field },
            Vec::new(),
            block,
            default.span,
        ));
    }
    for ctor in 0..reg.objects[object.0].ctors.len() {
        let body = check_ctor(reg, object, ctor);
        bodies.push(body);
    }
    for method in 0..reg.objects[object.0].methods.len() {
        let info = &reg.objects[object.0].methods[method];
        let Some(decl) = info.decl else {
            continue;
        };
        let (params, result, successor, is_async) = (
            info.params.clone(),
            info.result.clone(),
            info.successor,
            info.is_async,
        );
        let kind = if successor {
            Kind::Successor
        } else {
            Kind::Method
        };
        let mut checker = Checker::new(reg, kind, Some(object), result);
        checker.is_async = is_async;
        let mut locals = Vec::new();
        for ((_, ty), param) in params.iter().zip(&decl.params) {
            let local = checker.declare_param(&param.name, ty.clone(), param.variadic);
            locals.push(local);
        }
        let block = if successor {
            let out = checker.block(&decl.body, false, None);
            checker.block_of(out)
        } else {
            checker.body(&decl.body, &decl.name.text)
        };
        bodies.push(checker.finish(
            BodyOwner::Method { object, method },
            locals,
            block,
            decl.span,
        ));
    }
    if let Some(destroy) = reg.objects[object.0].destroy {
        let mut checker = Checker::new(reg, Kind::Destroy, Some(object), None);
        let block = checker.body(&destroy.body, "destroy");
        bodies.push(checker.finish(
            BodyOwner::Destroy { object },
            Vec::new(),
            block,
            destroy.span,
        ));
    }
    for cap in 0..reg.objects[object.0].caps.len() {
        for field in 0..reg.objects[object.0].caps[cap].fields.len() {
            let (name, ty, body) = reg.objects[object.0].caps[cap].fields[field].clone();
            let external = reg.objects[object.0].caps[cap].external;
            let mut checker = Checker::new(reg, Kind::CapField, Some(object), Some(ty));
            checker.cap_external = external;
            let block = checker.body(body, &name);
            bodies.push(checker.finish(
                BodyOwner::CapField { object, cap, field },
                Vec::new(),
                block,
                body.span,
            ));
        }
    }
}

fn check_ctor(reg: &mut Registry<'_>, object: ObjectId, ctor: usize) -> Body {
    let info = &reg.objects[object.0].ctors[ctor];
    let span = info.span;
    let shape = info.shape.clone();
    let self_ty = reg.self_type(object);
    let mut checker = Checker::new(reg, Kind::Ctor, Some(object), Some(self_ty));
    let mut locals = Vec::new();
    let body = match shape {
        CtorShape::Full {
            params,
            names,
            variadic,
            body,
        } => {
            let count = params.len();
            for (index, ((_, ty), name)) in params.into_iter().zip(names).enumerate() {
                let local = checker.declare_param(name, ty, variadic && index + 1 == count);
                locals.push(local);
            }
            Some(body)
        }
        CtorShape::Shorthand { fields, body } => {
            // В теле shorthand поля списка читаются по имени (§11.3).
            let named: Vec<(String, Ty)> = fields
                .iter()
                .map(|field| {
                    let info = &checker.reg.objects[object.0].fields[*field];
                    (info.name.clone(), info.ty.clone())
                })
                .collect();
            for (name, ty) in named {
                let name = ast::Name { text: name, span };
                locals.push(checker.declare_param(&name, ty, false));
            }
            checker.shorthand_fields = fields;
            body
        }
    };
    let block = match body {
        Some(body) => {
            let out = checker.block(body, false, None);
            checker.block_of(out)
        }
        None => Block {
            stmts: Vec::new(),
            span,
        },
    };
    checker.finish(BodyOwner::Ctor { object, ctor }, locals, block, span)
}

impl<'r, 'a> Checker<'r, 'a> {
    pub fn new(
        reg: &'r mut Registry<'a>,
        kind: Kind,
        object: Option<ObjectId>,
        result: Option<Ty>,
    ) -> Self {
        let module = match object {
            Some(object) => reg.objects[object.0].module,
            None => reg.entry_module,
        };
        reg.set_module(module);
        let generics = object
            .map(|object| reg.object_scope(object))
            .unwrap_or_default();
        let self_ty = object.map(|object| reg.self_type(object));
        Checker {
            reg,
            module,
            kind,
            object,
            generics,
            self_ty,
            result,
            is_async: false,
            locals: Vec::new(),
            scopes: vec![HashMap::new()],
            loops: Vec::new(),
            catch_depth: 0,
            narrow_message: None,
            unbounded: Vec::new(),
            outer: None,
            captures: Vec::new(),
            shorthand_fields: Vec::new(),
            cap_external: false,
        }
    }

    pub fn finish(self, owner: BodyOwner, params: Vec<LocalId>, block: Block, span: Span) -> Body {
        let returns_value =
            self.result.is_some() && !matches!(self.kind, Kind::Successor | Kind::Ctor);
        let body = Body {
            owner,
            module: self.module,
            locals: self.locals,
            params,
            block,
            returns_value,
            span,
        };
        super::flow::check(self.reg, &body);
        body
    }

    // ----- ошибки -----

    pub fn error(&mut self, span: Span, message: impl Into<String>) {
        self.reg.set_module(self.module);
        self.reg.error(span, message);
    }

    pub fn warn(&mut self, span: Span, message: impl Into<String>) {
        self.reg.set_module(self.module);
        self.reg.warn(span, message);
    }

    pub fn unsupported(&mut self, span: Span, message: impl Into<String>) {
        self.reg.set_module(self.module);
        self.reg.unsupported(span, message);
    }

    pub fn type_name(&self, ty: &Ty) -> String {
        self.reg.type_name(ty)
    }

    // ----- конструкторы выражений -----

    pub fn typed(kind: ExprKind, ty: Ty, span: Span) -> Expr {
        Expr {
            kind,
            ty: Some(ty),
            diverges: false,
            span,
        }
    }

    pub fn unit(kind: ExprKind, span: Span) -> Expr {
        Expr {
            kind,
            ty: None,
            diverges: false,
            span,
        }
    }

    pub fn diverged(kind: ExprKind, span: Span) -> Expr {
        Expr {
            kind,
            ty: Some(Ty::Never),
            diverges: true,
            span,
        }
    }

    /// Значение в потребляющей позиции (receiver, `throw`, распаковка, …):
    /// обязательство `^C` гасится явным `Take`.
    pub fn consume(value: Expr) -> Expr {
        match &value.ty {
            Some(Ty::Successor(inner)) if !value.diverges => {
                let ty = (**inner).clone();
                let span = value.span;
                Self::typed(
                    ExprKind::Coerce {
                        coercion: Coercion::Take,
                        value: Box::new(value),
                    },
                    ty,
                    span,
                )
            }
            _ => value,
        }
    }

    /// Значение после уже сообщённой ошибки.
    pub fn poisoned(span: Span) -> Expr {
        Self::typed(ExprKind::Unreachable, Ty::Error, span)
    }

    /// Тип значения выражения (`Error` — нет значения после ошибки).
    pub fn ty_of(expr: &Expr) -> Ty {
        expr.ty.clone().unwrap_or(Ty::Error)
    }

    // ----- локальные -----

    pub fn rebindable(&self, ty: &Ty) -> bool {
        match ty.taken() {
            Ty::Object(object, _) => self.reg.objects[object.0]
                .methods
                .iter()
                .any(|method| method.successor),
            _ => false,
        }
    }

    fn new_local(&mut self, name: &ast::Name, ty: Ty, variadic: bool) -> LocalId {
        let rebindable = self.rebindable(&ty);
        self.locals.push(Local {
            name: name.text.clone(),
            ty,
            rebindable,
            variadic,
            span: name.span,
        });
        LocalId(self.locals.len() - 1)
    }

    pub fn declare_param(&mut self, name: &ast::Name, ty: Ty, variadic: bool) -> LocalId {
        let ty = match (variadic, ty) {
            (true, Ty::Array(element)) => Ty::Varargs(element),
            (_, ty) => ty,
        };
        self.declare(name, ty, variadic)
    }

    /// Новая привязка в текущей области.
    pub fn declare(&mut self, name: &ast::Name, ty: Ty, variadic: bool) -> LocalId {
        if self
            .scopes
            .last()
            .is_some_and(|scope| scope.contains_key(&name.text))
        {
            self.error(
                name.span,
                format!(
                    "Variable \"{}\" is already defined in this scope",
                    name.text
                ),
            );
        }
        let local = self.new_local(name, ty, variadic);
        self.scopes
            .last_mut()
            .expect("statements run inside a scope")
            .insert(name.text.clone(), local);
        local
    }

    /// Локальная по имени; во внешнем теле анонимного действия — захват.
    pub fn local(&mut self, name: &str) -> Option<LocalId> {
        if let Some(local) = self
            .scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
        {
            return Some(local);
        }
        let (outer_local, ty) = self.outer.as_ref()?.visible.get(name).cloned()?;
        if let Some((_, inner)) = self
            .captures
            .iter()
            .find(|(outer, _)| *outer == outer_local)
        {
            return Some(*inner);
        }
        let inner = self.new_local(
            &ast::Name {
                text: name.to_string(),
                span: Span::empty(0),
            },
            ty,
            false,
        );
        self.scopes[0].insert(name.to_string(), inner);
        self.captures.push((outer_local, inner));
        Some(inner)
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Все видимые сейчас локальные (для тела анонимного действия). Имена
    /// внешнего тела сначала захватываются этим телом: вложенное действие
    /// ссылается только на локальные своего непосредственного окружения.
    pub fn visible(&mut self) -> HashMap<String, (LocalId, Ty)> {
        let inherited: Vec<String> = self
            .outer
            .as_ref()
            .map(|outer| outer.visible.keys().cloned().collect())
            .unwrap_or_default();
        for name in inherited {
            let _ = self.local(&name);
        }
        let mut visible: HashMap<String, (LocalId, Ty)> = HashMap::new();
        for scope in &self.scopes {
            for (name, local) in scope {
                visible.insert(name.clone(), (*local, self.locals[local.0].ty.clone()));
            }
        }
        visible
    }

    // ----- границы типов -----

    /// Значение в позиции с ожидаемым типом: явная граница (§12.1, §6.2).
    pub fn coerce(&mut self, value: Expr, expected: &Ty, span: Span, what: &str) -> Expr {
        let Some(actual) = value.ty.clone() else {
            if value.diverges {
                return value;
            }
            self.error(span, "Expression does not produce a value");
            return Self::poisoned(span);
        };
        match self.try_coerce(value, expected, what) {
            Ok(value) => value,
            Err(_) => {
                self.mismatch(&actual, expected, span, what);
                Self::poisoned(span)
            }
        }
    }

    /// Граница без сообщения: `Err` — значение не пересекает границу.
    pub fn try_coerce(
        &mut self,
        value: Expr,
        expected: &Ty,
        what: &str,
    ) -> Result<Expr, Box<Expr>> {
        let Some(actual) = value.ty.clone() else {
            return if value.diverges {
                Ok(value)
            } else {
                Err(Box::new(value))
            };
        };
        if value.diverges || actual.is_error() || expected.is_error() || actual == *expected {
            return Ok(value);
        }
        let wrap = |coercion: Coercion, value: Expr, ty: Ty| {
            let span = value.span;
            Self::typed(
                ExprKind::Coerce {
                    coercion,
                    value: Box::new(value),
                },
                ty,
                span,
            )
        };
        if actual.taken() == *expected {
            return Ok(wrap(Coercion::Take, value, expected.clone()));
        }
        let taken = actual.taken();
        let convertible = (taken.is_untagged() && self.same_shape(&taken, expected))
            || match expected {
                Ty::Role(..) => match taken {
                    Ty::Role(..) => self.narrowing(&taken, expected, what).is_some(),
                    _ => self.reg.assignable(&taken, expected),
                },
                Ty::Any => self.reg.assignable(&taken, &Ty::Any),
                Ty::Successor(target) => {
                    actual.is_successor() && self.reg.assignable(&taken, target)
                }
                _ => false,
            };
        if !convertible {
            return Err(Box::new(value));
        }
        if let Ty::Successor(_) = expected {
            return Ok(wrap(Coercion::RoleWrap, value, expected.clone()));
        }
        let value = if actual.is_successor() {
            wrap(Coercion::Take, value, taken.clone())
        } else {
            value
        };
        if taken.is_untagged() && self.same_shape(&taken, expected) {
            return Ok(wrap(Coercion::Retag, value, expected.clone()));
        }
        Ok(match expected {
            Ty::Role(..) => match taken {
                Ty::Role(..) => {
                    let message = self
                        .narrowing(&taken, expected, what)
                        .expect("checked as convertible");
                    wrap(Coercion::RoleNarrow { message }, value, expected.clone())
                }
                _ => wrap(Coercion::RoleWrap, value, expected.clone()),
            },
            _ => wrap(Coercion::Box, value, Ty::Any),
        })
    }

    /// Пустой нетипизированный литерал одной стороны получает тип
    /// типизированной коллекции другой стороны той же формы.
    pub fn retag_operands(&mut self, lhs: Expr, rhs: Expr) -> (Expr, Expr) {
        let (left, right) = (Self::ty_of(&lhs).taken(), Self::ty_of(&rhs).taken());
        let retag =
            |this: &mut Self, value: Expr, target: &Ty| match this.try_coerce(value, target, "") {
                Ok(value) => value,
                Err(value) => *value,
            };
        if left.is_untagged() && !right.is_untagged() && self.same_shape(&left, &right) {
            (retag(self, lhs, &right), rhs)
        } else if right.is_untagged() && !left.is_untagged() && self.same_shape(&right, &left) {
            let rhs = retag(self, rhs, &left);
            (lhs, rhs)
        } else {
            (lhs, rhs)
        }
    }

    fn same_shape(&self, untagged: &Ty, target: &Ty) -> bool {
        matches!(
            (untagged, target),
            (Ty::Array(_), Ty::Array(_)) | (Ty::Map(..), Ty::Map(..))
        )
    }

    /// Роль → роль сужением (§12.1): текст ошибки проверки во время исполнения.
    fn narrowing(&self, source: &Ty, target: &Ty, what: &str) -> Option<String> {
        self.reg.narrows(source, target).then(|| {
            self.narrow_message.clone().unwrap_or_else(|| {
                format!(
                    "{what} must be {}, got {}",
                    self.type_name(target),
                    self.type_name(source)
                )
            })
        })
    }

    pub fn mismatch(&mut self, actual: &Ty, expected: &Ty, span: Span, what: &str) {
        if actual.is_error() || expected.is_error() {
            return;
        }
        let message = format!(
            "{what} must be {}, got {}",
            self.type_name(expected),
            self.type_name(actual)
        );
        self.error(span, message);
    }

    /// Точный тип без границ (условия, индексы, операнды builtins).
    pub fn expect(&mut self, value: &Expr, expected: &Ty, span: Span, what: &str) -> bool {
        match &value.ty {
            Some(actual) if actual.taken() == *expected => true,
            Some(actual) if actual.is_error() || value.diverges => false,
            Some(actual) => {
                let actual = actual.clone();
                self.mismatch(&actual, expected, span, what);
                false
            }
            None if value.diverges => false,
            None => {
                self.error(span, "Expression does not produce a value");
                false
            }
        }
    }

    // ----- слияние ветвей -----

    /// Тип значения ветвей `if`/`match`/`try`/индекса с fallback: ветви,
    /// ушедшие из потока, не участвуют; `[]`/`[:]` получают тип типизированной
    /// ветви той же коллекции.
    pub fn join(&mut self, arms: &[&Expr], want: bool, span: Span) -> (Option<Ty>, bool) {
        let live: Vec<&&Expr> = arms.iter().filter(|arm| !arm.diverges).collect();
        if live.is_empty() {
            return (Some(Ty::Never), true);
        }
        if !want || live.iter().any(|arm| arm.ty.is_none()) {
            return (None, false);
        }
        let types: Vec<Ty> = live.iter().map(|arm| Self::ty_of(arm)).collect();
        if types.iter().any(Ty::is_error) {
            return (Some(Ty::Error), false);
        }
        let tagged = types
            .iter()
            .find(|ty| matches!(ty, Ty::Array(_) | Ty::Map(..)) && !ty.is_untagged())
            .cloned();
        let normalized: Vec<Ty> = types
            .iter()
            .map(|ty| match &tagged {
                Some(target) if ty.is_untagged() && self.same_shape(ty, target) => target.clone(),
                _ => ty.clone(),
            })
            .collect();
        let first = normalized[0].clone();
        for other in &normalized[1..] {
            if *other != first {
                let message = format!(
                    "Branches produce different types: {} and {}",
                    self.type_name(&first),
                    self.type_name(other)
                );
                self.error(span, message);
                return (Some(Ty::Error), false);
            }
        }
        (Some(first), false)
    }

    // ----- тела и блоки -----

    /// Тело действия: значение последнего оператора — результат.
    pub fn body(&mut self, block: &ast::Block, name: &str) -> Block {
        let result = self.result.clone();
        let want = result.is_some();
        let out = self.block(block, want, result.as_ref());
        let tail_span = block.stmts.last().map_or(block.span, |stmt| stmt.span);
        let mut thir = self.block_of(out.clone());
        match (&out.ty, &result) {
            _ if out.diverges => {}
            (Some(_), Some(ty)) => {
                // Результат — граница типа действия: последний оператор приводится.
                if let Some(last) = thir.stmts.pop() {
                    let value = match last.kind {
                        StmtKind::Expr(value) => value,
                        StmtKind::Let { local, value } => {
                            // `let` последним: значение — значение привязки.
                            thir.stmts.push(Stmt {
                                kind: StmtKind::Let { local, value },
                                span: last.span,
                            });
                            let ty = self.locals[local.0].ty.clone();
                            Self::typed(ExprKind::Local(local), ty, last.span)
                        }
                        other => {
                            thir.stmts.push(Stmt {
                                kind: other,
                                span: last.span,
                            });
                            return thir;
                        }
                    };
                    let value = self.coerce(value, ty, tail_span, "Action result");
                    thir.stmts.push(Stmt {
                        kind: StmtKind::Expr(value),
                        span: last.span,
                    });
                }
            }
            (None, Some(ty)) => {
                let message = format!(
                    "Action \"{name}\" must produce a return value of type {}.",
                    self.type_name(ty)
                );
                self.error(block.span, message);
            }
            (_, None) => {}
        }
        thir
    }

    /// Блок как выражение; `expected` — тип, которого ждут от значения.
    pub fn block(&mut self, block: &ast::Block, want: bool, expected: Option<&Ty>) -> Expr {
        self.push_scope();
        let out = self.stmts(&block.stmts, want, expected, block.span);
        self.pop_scope();
        out
    }

    /// THIR-блок из результата [`Checker::block`].
    pub fn block_of(&self, expr: Expr) -> Block {
        match expr.kind {
            ExprKind::Block(block) => block,
            _ => Block {
                stmts: vec![Stmt {
                    span: expr.span,
                    kind: StmtKind::Expr(expr),
                }],
                span: Span::empty(0),
            },
        }
    }

    fn stmts(
        &mut self,
        stmts: &[ast::Stmt],
        want: bool,
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        let mut out: Vec<Stmt> = Vec::new();
        let mut ty = None;
        let mut diverged = false;
        for (index, stmt) in stmts.iter().enumerate() {
            let last = index + 1 == stmts.len();
            let (mut checked, mut value) =
                self.stmt(stmt, want && last, if last { expected } else { None });
            // Ожидаемый тип блока — граница его последнего значения: ветви
            // `if`/`match`/`try` приводятся к нему каждая (§6.5).
            if let (true, Some(target), Some(actual)) = (want && last, expected, value.ty.clone()) {
                if !value.diverges && actual != *target && !actual.is_error() {
                    if let StmtKind::Expr(expr) = checked.kind {
                        checked.kind = match self.try_coerce(expr, target, "Value") {
                            Ok(coerced) => {
                                value.ty = Some(target.clone());
                                StmtKind::Expr(coerced)
                            }
                            Err(original) => StmtKind::Expr(*original),
                        };
                    }
                }
            }
            if !diverged {
                if value.diverges {
                    diverged = true;
                } else if last {
                    ty = value.ty;
                }
            }
            out.push(checked);
        }
        Expr {
            kind: ExprKind::Block(Block { stmts: out, span }),
            ty: if diverged { Some(Ty::Never) } else { ty },
            diverges: diverged,
            span,
        }
    }
}
