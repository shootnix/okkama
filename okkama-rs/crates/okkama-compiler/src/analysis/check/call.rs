//! Вызовы: конструкторы, actions объектов и ролей, natives, action-значения,
//! доступ к членам, проекции и анонимные действия.
//!
//! Аргументы вычисляются один раз, слева направо. Если имя и форма вызова
//! однозначно определяют сигнатуру, её типы параметров — ожидаемые типы
//! аргументов (вывод generic-конструирования, тег `[]`); иначе аргументы
//! проверяются сами по себе, а перегрузка выбирается по ним (§11.8).

use std::collections::HashSet;

use super::stmt::strip_grouping;
use super::{Checker, Kind, Outer};
use crate::analysis::registry::{CtorShape, TypeRef};
use crate::analysis::thir::{BodyOwner, Callee, CastTarget, Expr, ExprKind};
use crate::analysis::ty::{ActionTy, ObjectId, ParamId, RoleId, Ty};
use crate::source::Span;
use crate::syntax::ast;

/// Проверенный аргумент вызова.
#[derive(Clone, Debug)]
pub(super) struct Arg {
    pub name: Option<String>,
    pub value: Expr,
    pub span: Span,
}

/// Проверенные аргументы: значения и финальная распаковка `xs...`.
#[derive(Clone, Debug, Default)]
pub(super) struct CallArgs {
    pub values: Vec<Arg>,
    pub spread: Option<(Expr, Span)>,
    pub diverges: bool,
}

/// Сигнатура для выбора перегрузки (типы уже подставлены).
#[derive(Clone, Debug)]
pub(super) struct Shape {
    pub params: Vec<(String, Ty)>,
    /// Последний параметр — variadic-хвост (`@T`).
    pub variadic: bool,
}

impl CallArgs {
    pub fn named(&self) -> bool {
        self.values.iter().any(|arg| arg.name.is_some())
    }

    pub fn names(&self) -> HashSet<String> {
        self.values
            .iter()
            .filter_map(|arg| arg.name.clone())
            .collect()
    }
}

impl Checker<'_, '_> {
    // ----- аргументы -----

    /// Аргументы слева направо; `expected` — типы параметров, если сигнатура
    /// уже известна. Распаковка допустима только последней.
    pub(super) fn call_arguments(
        &mut self,
        args: &[ast::Argument],
        expected: Option<&Shape>,
    ) -> CallArgs {
        let mut call = CallArgs::default();
        let mut seen: HashSet<String> = HashSet::new();
        for (index, arg) in args.iter().enumerate() {
            if let Some(name) = &arg.name {
                if !seen.insert(name.text.clone()) {
                    self.error(name.span, format!("Duplicate argument \"{}\"", name.text));
                }
            }
            if let ast::ExprKind::Unpack { operand, .. } = &strip_grouping(&arg.value).kind {
                if index + 1 != args.len() || arg.name.is_some() {
                    self.error(
                        arg.value.span,
                        "Unpack is allowed only as the final positional argument of a variadic call",
                    );
                }
                let value = self.unpacked(operand, arg.value.span);
                call.diverges |= value.diverges;
                call.spread = Some((value, arg.value.span));
                continue;
            }
            let hint = expected.and_then(|shape| {
                param_hint(
                    shape,
                    index,
                    arg.name.as_ref().map(|name| name.text.as_str()),
                )
            });
            let value = self.value(&arg.value, hint.as_ref());
            call.diverges |= value.diverges;
            call.values.push(Arg {
                name: arg.name.as_ref().map(|name| name.text.clone()),
                value,
                span: arg.value.span,
            });
        }
        call
    }

    /// Операнд `xs...`: массив или variadic-привязка; результат — новый массив.
    pub(super) fn unpacked(&mut self, operand: &ast::Expr, span: Span) -> Expr {
        let value = match &strip_grouping(operand).kind {
            ast::ExprKind::Name(name) => match self.local(&name.text) {
                Some(local) if self.locals[local.0].variadic => {
                    let ty = self.locals[local.0].ty.clone();
                    Self::typed(ExprKind::Local(local), ty, operand.span)
                }
                _ => self.value(operand, None),
            },
            _ => self.value(operand, None),
        };
        let element = match Self::ty_of(&value).taken() {
            Ty::Array(element) | Ty::Varargs(element) => *element,
            Ty::Error => Ty::Error,
            _ => {
                self.error(
                    operand.span,
                    "Only arrays and variadic bindings can be unpacked with `...`",
                );
                Ty::Error
            }
        };
        let diverges = value.diverges;
        let mut out = Self::typed(
            ExprKind::Unpack(Box::new(Self::consume(value))),
            Ty::array(element),
            span,
        );
        out.diverges = diverges;
        out
    }

    pub(super) fn type_list(&self, call: &CallArgs) -> String {
        call.values
            .iter()
            .map(|arg| {
                let ty = self.type_name(&Self::ty_of(&arg.value).taken());
                match &arg.name {
                    Some(name) => format!("{name}: {ty}"),
                    None => ty,
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Специфичность аргумента для параметра: точный тип > роль > `any`.
    fn rank(&self, actual: &Ty, param: &Ty) -> u8 {
        if actual.taken() == *param {
            3
        } else if mentions_any(param) {
            0
        } else if matches!(param, Ty::Role(..)) {
            1
        } else {
            2
        }
    }

    /// Подходит ли вызов сигнатуре; `Some(ranks)` — специфичность по позициям.
    pub(super) fn fits(&self, call: &CallArgs, shape: &Shape) -> Option<Vec<u8>> {
        let accepts = |actual: &Ty, param: &Ty| self.reg.assignable(&actual.taken(), param);
        let named = call.named();
        if !shape.variadic {
            if call.spread.is_some() || shape.params.len() != call.values.len() {
                return None;
            }
            let mut ranks = Vec::new();
            for (index, (name, param)) in shape.params.iter().enumerate() {
                let arg = if named {
                    call.values
                        .iter()
                        .find(|arg| arg.name.as_deref() == Some(name.as_str()))?
                } else {
                    &call.values[index]
                };
                let actual = Self::ty_of(&arg.value);
                if !accepts(&actual, param) {
                    return None;
                }
                ranks.push(self.rank(&actual, param));
            }
            return Some(ranks);
        }
        let (fixed, tail) = shape.params.split_at(shape.params.len() - 1);
        let Ty::Array(element) = &tail[0].1 else {
            return None;
        };
        if named || call.values.len() < fixed.len() {
            return None;
        }
        let mut ranks = Vec::new();
        for ((_, param), arg) in fixed.iter().zip(&call.values) {
            let actual = Self::ty_of(&arg.value);
            if !accepts(&actual, param) {
                return None;
            }
            ranks.push(self.rank(&actual, param));
        }
        for arg in &call.values[fixed.len()..] {
            let actual = Self::ty_of(&arg.value);
            if !accepts(&actual, element) {
                return None;
            }
            ranks.push(self.rank(&actual, element));
        }
        if let Some((array, _)) = &call.spread {
            match Self::ty_of(array).taken() {
                Ty::Array(item) if *item == **element || *item == Ty::Never || item.is_error() => {}
                Ty::Error => {}
                _ => return None,
            }
        }
        Some(ranks)
    }

    /// Единственный самый специфичный кандидат (§11.8): `Ok(index)`,
    /// `Err(true)` — неоднозначно, `Err(false)` — никто не подходит.
    pub(super) fn select_overload(&self, call: &CallArgs, shapes: &[Shape]) -> Result<usize, bool> {
        let fitting: Vec<(usize, Vec<u8>)> = shapes
            .iter()
            .enumerate()
            .filter_map(|(index, shape)| self.fits(call, shape).map(|ranks| (index, ranks)))
            .collect();
        match fitting.as_slice() {
            [] => Err(false),
            [(index, _)] => Ok(*index),
            _ => {
                let dominates = |a: &[u8], b: &[u8]| {
                    a.len() == b.len()
                        && a.iter().zip(b).all(|(x, y)| x >= y)
                        && a.iter().zip(b).any(|(x, y)| x > y)
                };
                let best: Vec<usize> = fitting
                    .iter()
                    .filter(|(index, ranks)| {
                        fitting
                            .iter()
                            .filter(|(other, _)| other != index)
                            .all(|(_, other)| dominates(ranks, other))
                    })
                    .map(|(index, _)| *index)
                    .collect();
                match best.as_slice() {
                    [index] => Ok(*index),
                    _ => Err(true),
                }
            }
        }
    }

    /// Упорядочить аргументы по параметрам и привести их к типам параметров;
    /// variadic-хвост собирается узлом `Pack`.
    pub(super) fn arrange(
        &mut self,
        call: CallArgs,
        shape: &Shape,
        what: &str,
        span: Span,
    ) -> Vec<Expr> {
        if shape.variadic && call.named() {
            let subject = what
                .strip_prefix("Action ")
                .map_or(what.to_string(), |rest| format!("action {rest}"));
            self.error(
                span,
                format!("Variadic {subject} does not accept named arguments"),
            );
            return Vec::new();
        }
        let fixed_count = if shape.variadic {
            shape.params.len() - 1
        } else {
            shape.params.len()
        };
        let mut values = call.values;
        let rest = if shape.variadic {
            values.split_off(fixed_count.min(values.len()))
        } else {
            Vec::new()
        };
        let ordered: Vec<Arg> = if values.iter().any(|arg| arg.name.is_some()) {
            let mut ordered = Vec::new();
            for (name, _) in &shape.params[..fixed_count] {
                if let Some(position) = values
                    .iter()
                    .position(|arg| arg.name.as_deref() == Some(name.as_str()))
                {
                    ordered.push(values.remove(position));
                }
            }
            ordered
        } else {
            values
        };
        let mut args = Vec::new();
        for (arg, (name, ty)) in ordered.into_iter().zip(&shape.params[..fixed_count]) {
            let what = format!("{what} argument \"{name}\"");
            args.push(self.coerce(arg.value, ty, arg.span, &what));
        }
        if shape.variadic {
            let element = match &shape.params[fixed_count].1 {
                Ty::Array(element) => (**element).clone(),
                _ => Ty::Error,
            };
            let mut items = Vec::new();
            for arg in rest {
                items.push(self.coerce(arg.value, &element, arg.span, &format!("{what} argument")));
            }
            let spread = call.spread.map(|(array, _)| Box::new(array));
            args.push(Self::typed(
                ExprKind::Pack { items, spread },
                Ty::array(element),
                span,
            ));
        }
        args
    }

    fn call_result(
        callee: Callee,
        args: Vec<Expr>,
        result: Option<Ty>,
        is_async: bool,
        span: Span,
        diverges: bool,
    ) -> Expr {
        let ty = if is_async {
            Some(Ty::Future(result.map(Box::new)))
        } else {
            result
        };
        Expr {
            kind: ExprKind::Call { callee, args },
            ty,
            diverges,
            span,
        }
    }

    // ----- формы вызова -----

    pub(super) fn call(
        &mut self,
        callee: &ast::Expr,
        args: &[ast::Argument],
        span: Span,
        expected: Option<&Ty>,
    ) -> Expr {
        let named = args.iter().filter(|arg| arg.name.is_some()).count();
        if named != 0 && named != args.len() {
            self.error(span, "A call cannot mix positional and named arguments");
            return Self::poisoned(span);
        }
        match &strip_grouping(callee).kind {
            // Примитивные приведения важнее одноимённых локальных привязок (§6.6.1).
            ast::ExprKind::Name(name)
                if matches!(
                    name.text.as_str(),
                    "int" | "string" | "byte" | "float" | "rat"
                ) =>
            {
                self.primitive_cast(&name.text, args, span)
            }
            ast::ExprKind::Name(name) => {
                match self.reg.find_type(self.module, &name.text) {
                    Some(TypeRef::Object(object)) => {
                        return self.construct(object, None, args, span, expected);
                    }
                    Some(TypeRef::Role(role)) => {
                        let message = format!(
                            "Role \"{}\" cannot be constructed",
                            self.reg.roles[role.0].name
                        );
                        self.error(span, message);
                        return Self::poisoned(span);
                    }
                    _ => {}
                }
                if let Some(local) = self.local(&name.text) {
                    let ty = self.locals[local.0].ty.clone();
                    let value = Self::typed(ExprKind::Local(local), ty, name.span);
                    return self.call_value(value, args, span);
                }
                if let Some(native) = self.reg.find_native(self.module, &name.text) {
                    return self.native_call(native, args, span);
                }
                // Поле с action-значением: `condition(item)`.
                let fields_visible =
                    matches!(self.kind, Kind::Method | Kind::Successor | Kind::Destroy)
                        || (self.kind == Kind::Closure && self.self_ty.is_some());
                if fields_visible {
                    if let Some(field) = self.own_field(&name.text) {
                        let ty = self.field_type(field);
                        let value = Self::typed(ExprKind::Field(field), ty, name.span);
                        return self.call_value(value, args, span);
                    }
                }
                if matches!(name.text.as_str(), "NativeFailure" | "Cancelled") {
                    self.error(span, format!("Source cannot construct \"{}\"", name.text));
                    return Self::poisoned(span);
                }
                self.error(name.span, format!("Undefined action \"{}\"", name.text));
                Self::poisoned(span)
            }
            ast::ExprKind::Get { object, name } => {
                if let Some(call) = self.fake_call(object, name, args, span, expected) {
                    return call;
                }
                // `ns.Object(...)` и `ns.native(...)`: имя пространства, не локальная.
                if let ast::ExprKind::Name(space) = &strip_grouping(object).kind {
                    if self.local(&space.text).is_none()
                        && self.reg.is_namespace(self.module, &space.text)
                    {
                        return self.qualified_call(&space.text, name, args, span, expected);
                    }
                }
                self.method_call(object, name, args, span)
            }
            _ => {
                let value = self.value(callee, None);
                self.call_value(value, args, span)
            }
        }
    }

    /// `Role.fake()` — объект fake роли (тестовый режим) и
    /// `fake.storage.op(...)` в action fake.
    fn fake_call(
        &mut self,
        object: &ast::Expr,
        name: &ast::Name,
        args: &[ast::Argument],
        span: Span,
        expected: Option<&Ty>,
    ) -> Option<Expr> {
        match &strip_grouping(object).kind {
            ast::ExprKind::Name(role_name)
                if name.text == "fake" && self.local(&role_name.text).is_none() =>
            {
                let Some(TypeRef::Role(role)) = self.reg.find_type(self.module, &role_name.text)
                else {
                    return None;
                };
                let info = &self.reg.roles[role.0];
                let fake = crate::syntax::prune::fake_object_name(&info.name);
                let found = self.reg.modules[info.module].own.types.get(&fake).copied();
                match found {
                    Some(TypeRef::Object(fake)) if self.reg.test_mode => {
                        Some(self.construct(fake, None, args, span, expected))
                    }
                    _ => {
                        let message = if self.reg.test_mode {
                            format!("Role \"{}\" has no fake", role_name.text)
                        } else {
                            format!("{}.fake() is available only in test mode", role_name.text)
                        };
                        self.error(span, message);
                        Some(Self::poisoned(span))
                    }
                }
            }
            ast::ExprKind::Get {
                object: inner,
                name: view,
            } if view.text == "storage"
                && matches!(&strip_grouping(inner).kind, ast::ExprKind::Name(fake) if fake.text == "fake")
                && self.local("fake").is_none() =>
            {
                Some(self.fake_storage(name, args, span, expected))
            }
            _ => None,
        }
    }

    /// Операция хранилища fake: ключ — скаляр, строка или enum; значение
    /// `get` — тип из контекста (проверяется при чтении).
    fn fake_storage(
        &mut self,
        op: &ast::Name,
        args: &[ast::Argument],
        span: Span,
        expected: Option<&Ty>,
    ) -> Expr {
        let in_fake = self
            .object
            .is_some_and(|object| self.reg.objects[object.0].name.ends_with("$fake"))
            && matches!(self.kind, Kind::Method | Kind::Closure);
        if !in_fake {
            self.error(span, "fake.storage is available only inside a role fake");
            return Self::poisoned(span);
        }
        let arity = match op.text.as_str() {
            "set" => 2,
            "get" | "has" | "remove" => 1,
            "clear" | "size" => 0,
            other => {
                self.error(
                    op.span,
                    format!("Fake storage operation \"{other}\" does not exist"),
                );
                return Self::poisoned(span);
            }
        };
        if args.len() != arity || args.iter().any(|arg| arg.name.is_some()) {
            self.error(
                span,
                format!(
                    "Fake storage operation \"{}\" expects {arity} positional argument(s)",
                    op.text
                ),
            );
            return Self::poisoned(span);
        }
        let mut values = Vec::new();
        if let Some(key) = args.first() {
            let value = self.value(&key.value, None);
            let key_ty = Self::ty_of(&value).taken();
            let valid = matches!(
                key_ty,
                Ty::Int | Ty::Bool | Ty::Str | Ty::Char | Ty::Enum(_) | Ty::Error
            );
            if !valid {
                let message = if key_ty == Ty::Byte {
                    "Fake storage key cannot be byte".to_string()
                } else {
                    format!(
                        "Fake storage key must be int, bool, string, char or enum, got {}",
                        self.type_name(&key_ty)
                    )
                };
                self.error(key.value.span, message);
            }
            values.push(Self::consume(value));
        }
        if let Some(stored) = args.get(1) {
            let value = self.value(&stored.value, None);
            values.push(Self::consume(value));
        }
        let result = match op.text.as_str() {
            "has" => Some(Ty::Bool),
            "size" => Some(Ty::Int),
            "get" => match expected {
                Some(ty) if !matches!(ty, Ty::Error) => Some(ty.clone()),
                _ => {
                    self.error(
                        span,
                        "fake.storage.get() needs an expected type from its context",
                    );
                    return Self::poisoned(span);
                }
            },
            _ => None,
        };
        let callee = Callee::FakeStorage(op.text.clone());
        match result {
            Some(ty) => Self::typed(
                ExprKind::Call {
                    callee,
                    args: values,
                },
                ty,
                span,
            ),
            None => Self::unit(
                ExprKind::Call {
                    callee,
                    args: values,
                },
                span,
            ),
        }
    }

    fn qualified_call(
        &mut self,
        space: &str,
        name: &ast::Name,
        args: &[ast::Argument],
        span: Span,
        expected: Option<&Ty>,
    ) -> Expr {
        match self.reg.find_qualified(self.module, space, &name.text) {
            Some(TypeRef::Object(object)) => {
                return self.construct(object, None, args, span, expected);
            }
            Some(_) => {
                self.error(
                    name.span,
                    format!("\"{space}.{}\" is not callable", name.text),
                );
                return Self::poisoned(span);
            }
            None => {}
        }
        if let Some(native) = self
            .reg
            .find_qualified_native(self.module, space, &name.text)
        {
            return self.native_call(native, args, span);
        }
        self.error(
            name.span,
            format!("Namespace \"{space}\" has no declaration \"{}\"", name.text),
        );
        Self::poisoned(span)
    }

    /// Вызов `native`: аргументы приводятся к типам параметров (§5.3).
    fn native_call(&mut self, native: usize, args: &[ast::Argument], span: Span) -> Expr {
        let info = &self.reg.natives[native];
        let shape = Shape {
            params: info.params.clone(),
            variadic: info.variadic,
        };
        let (result, is_async, name) = (info.result.clone(), info.is_async, info.name.clone());
        let call = self.call_arguments(args, Some(&shape));
        let diverges = call.diverges;
        let arity_ok = if shape.variadic {
            call.values.len() + 1 >= shape.params.len()
        } else {
            call.values.len() == shape.params.len() && call.spread.is_none()
        };
        if !arity_ok {
            let message = format!(
                "Native action \"{name}\" expects {} argument(s), got {}",
                shape.params.len(),
                call.values.len()
            );
            self.error(span, message);
            return Self::poisoned(span);
        }
        if call.named() {
            let missing = shape
                .params
                .iter()
                .find(|(param, _)| !call.names().contains(param));
            if let Some((param, _)) = missing {
                let message = format!("Native action \"{name}\" requires argument \"{param}\"");
                self.error(span, message);
                return Self::poisoned(span);
            }
        }
        let args = self.arrange(call, &shape, &format!("Native action \"{name}\""), span);
        Self::call_result(
            Callee::Native(native),
            args,
            result,
            is_async,
            span,
            diverges,
        )
    }

    /// Вызов action-значения: только позиционные аргументы, точная арность.
    fn call_value(&mut self, callee: Expr, args: &[ast::Argument], span: Span) -> Expr {
        let unbounded = match &callee.kind {
            ExprKind::BoundAction { .. } => true,
            ExprKind::Local(local) => self.unbounded.contains(local),
            _ => false,
        };
        if unbounded {
            self.error(
                span,
                "Bound action requires an exact action-type boundary before it can be called",
            );
            for arg in args {
                let _ = self.expr(&arg.value, true, None);
            }
            return Self::poisoned(span);
        }
        let ty = Self::ty_of(&callee).taken();
        let Ty::Action(action) = ty else {
            if !ty.is_error() {
                let message = format!("Value of type {} is not callable", self.type_name(&ty));
                self.error(span, message);
            }
            for arg in args {
                let _ = self.expr(&arg.value, true, None);
            }
            return Self::poisoned(span);
        };
        if args.iter().any(|arg| arg.name.is_some()) {
            self.error(span, "Action values take positional arguments only");
            return Self::poisoned(span);
        }
        let shape = Shape {
            params: action
                .params
                .iter()
                .enumerate()
                .map(|(index, ty)| (format!("{}", index + 1), ty.clone()))
                .collect(),
            variadic: false,
        };
        let call = self.call_arguments(args, Some(&shape));
        if call.values.len() != shape.params.len() || call.spread.is_some() {
            let message = format!(
                "Action value expects {} argument(s), got {}",
                shape.params.len(),
                call.values.len()
            );
            self.error(span, message);
            return Self::poisoned(span);
        }
        let diverges = call.diverges || callee.diverges;
        let args = self.arrange(call, &shape, "Action value", span);
        let result = if action.is_async {
            Some(Ty::Future(action.result.clone().map(Box::new)))
        } else {
            action.result.clone()
        };
        Expr {
            kind: ExprKind::CallValue {
                callee: Box::new(callee),
                args,
            },
            ty: result,
            diverges,
            span,
        }
    }

    // ----- конструирование -----

    /// Формы конструкторов объекта (типы — после подстановки `env`).
    pub(super) fn ctor_shapes(
        &self,
        object: ObjectId,
        env: &[(ParamId, Ty)],
    ) -> Vec<(Shape, bool)> {
        let info = &self.reg.objects[object.0];
        info.ctors
            .iter()
            .map(|ctor| match &ctor.shape {
                CtorShape::Full {
                    params, variadic, ..
                } => (
                    Shape {
                        params: params
                            .iter()
                            .map(|(name, ty)| (name.clone(), ty.subst(env)))
                            .collect(),
                        variadic: *variadic,
                    },
                    false,
                ),
                CtorShape::Shorthand { fields, .. } => (
                    Shape {
                        params: fields
                            .iter()
                            .map(|field| {
                                (
                                    info.fields[*field].name.clone(),
                                    info.fields[*field].ty.subst(env),
                                )
                            })
                            .collect(),
                        variadic: false,
                    },
                    true,
                ),
            })
            .collect()
    }

    /// `Object(args)`; `type_args` — известная специализация generic-объекта.
    pub(super) fn construct(
        &mut self,
        object: ObjectId,
        type_args: Option<Vec<Ty>>,
        args: &[ast::Argument],
        span: Span,
        expected: Option<&Ty>,
    ) -> Expr {
        if self.reg.is_system_object(object) {
            let message = format!(
                "Source cannot construct \"{}\"",
                self.reg.objects[object.0].name
            );
            self.error(span, message);
            return Self::poisoned(span);
        }
        let params = self.reg.objects[object.0].params.clone();
        // Известная из ожидаемого типа специализация задаёт типы аргументов.
        let type_args_given = type_args.clone();
        let known = type_args.or_else(|| {
            if params.is_empty() {
                Some(Vec::new())
            } else {
                self.expected_specialization(object, expected)
            }
        });
        let hint = known.as_ref().and_then(|args_known| {
            let env: Vec<(ParamId, Ty)> = params
                .iter()
                .copied()
                .zip(args_known.iter().cloned())
                .collect();
            let shapes: Vec<(usize, Shape)> = self
                .ctor_shapes(object, &env)
                .into_iter()
                .map(|(shape, _)| shape)
                .enumerate()
                .collect();
            Self::unique_shape(&shapes, args)
        });
        let call = self.call_arguments(args, hint.as_ref());
        // Ожидаемый тип — подсказка аргументам; специализацию задают и он, и
        // фактические аргументы: противоречие — конфликт вывода (§6.5).
        let resolved = match (type_args_given, params.is_empty()) {
            (Some(args_known), _) => args_known,
            (None, true) => Vec::new(),
            (None, false) => match self.infer_specialization(object, &call, expected, span) {
                Some(args_inferred) => args_inferred,
                None => return Self::poisoned(span),
            },
        };
        if !self.check_specialization(object, &resolved, span) {
            return Self::poisoned(span);
        }
        self.construct_values(object, resolved, call, span)
    }

    /// Конструирование по вычисленным аргументам.
    pub(super) fn construct_values(
        &mut self,
        object: ObjectId,
        type_args: Vec<Ty>,
        call: CallArgs,
        span: Span,
    ) -> Expr {
        let params = self.reg.objects[object.0].params.clone();
        let env: Vec<(ParamId, Ty)> = params
            .iter()
            .copied()
            .zip(type_args.iter().cloned())
            .collect();
        let shapes = self.ctor_shapes(object, &env);
        let object_ty = Ty::Object(object, type_args.clone());
        let name = self.type_name(&object_ty);
        let named = call.named();
        let names = call.names();
        let mut candidates = Vec::new();
        for (index, (shape, shorthand)) in shapes.iter().enumerate() {
            let fits = if *shorthand {
                let info = &self.reg.objects[object.0];
                let CtorShape::Shorthand { fields, .. } = &info.ctors[index].shape else {
                    unreachable!("shorthand shapes come from shorthand constructors")
                };
                let allowed: HashSet<&str> = fields
                    .iter()
                    .map(|field| info.fields[*field].name.as_str())
                    .collect();
                let required = fields
                    .iter()
                    .filter(|field| info.fields[**field].default.is_none())
                    .all(|field| names.contains(&info.fields[*field].name));
                call.spread.is_none()
                    && (named || call.values.is_empty())
                    && names.iter().all(|name| allowed.contains(name.as_str()))
                    && required
            } else if shape.variadic {
                self.fits(&call, shape).is_some()
            } else {
                call.spread.is_none()
                    && shape.params.len() == call.values.len()
                    && (!named || shape.params.iter().all(|(name, _)| names.contains(name)))
            };
            if fits {
                candidates.push(index);
            }
        }
        // Несколько подходящих форм с именами: меньше пропущенных полей shorthand (как Perl).
        if candidates.len() > 1 && named {
            let omitted = |index: &usize| match &self.reg.objects[object.0].ctors[*index].shape {
                CtorShape::Shorthand { fields, .. } => fields
                    .iter()
                    .filter(|field| {
                        !names.contains(&self.reg.objects[object.0].fields[**field].name)
                    })
                    .count(),
                CtorShape::Full { .. } => 0,
            };
            let fewest = candidates.iter().map(omitted).min().unwrap_or(0);
            candidates.retain(|index| omitted(index) == fewest);
        }
        let ctor = match candidates.as_slice() {
            [ctor] => *ctor,
            [] => {
                let message = format!(
                    "No constructor of \"{name}\" accepts ({})",
                    self.type_list(&call)
                );
                self.error(span, message);
                return Self::poisoned(span);
            }
            _ => {
                self.error(span, format!("Ambiguous construction of \"{name}\""));
                return Self::poisoned(span);
            }
        };
        let diverges = call.diverges;
        let (shape, shorthand) = shapes[ctor].clone();
        let (shape, given) = if shorthand {
            let info = &self.reg.objects[object.0];
            let mut given: Vec<usize> = names
                .iter()
                .map(|name| {
                    info.fields
                        .iter()
                        .position(|field| field.name == *name)
                        .expect("allowed")
                })
                .collect();
            given.sort_unstable();
            let params = given
                .iter()
                .map(|field| {
                    (
                        info.fields[*field].name.clone(),
                        info.fields[*field].ty.subst(&env),
                    )
                })
                .collect();
            (
                Shape {
                    params,
                    variadic: false,
                },
                given,
            )
        } else {
            (shape, Vec::new())
        };
        let args = self.arrange(call, &shape, &format!("Constructor \"{name}\""), span);
        Expr {
            kind: ExprKind::Call {
                callee: Callee::Ctor {
                    object,
                    type_args,
                    ctor,
                    given,
                },
                args,
            },
            ty: Some(object_ty),
            diverges,
            span,
        }
    }

    // ----- actions -----

    /// Формы действий объекта с данным именем (типы подставлены).
    fn method_shapes(&self, object: ObjectId, args: &[Ty], name: &str) -> Vec<(usize, Shape)> {
        let info = &self.reg.objects[object.0];
        let env: Vec<(ParamId, Ty)> = info
            .params
            .iter()
            .copied()
            .zip(args.iter().cloned())
            .collect();
        info.methods
            .iter()
            .enumerate()
            .filter(|(_, method)| method.name == name)
            .map(|(index, method)| {
                (
                    index,
                    Shape {
                        params: method
                            .params
                            .iter()
                            .map(|(name, ty)| (name.clone(), ty.subst(&env)))
                            .collect(),
                        variadic: method.variadic,
                    },
                )
            })
            .collect()
    }

    fn role_shapes(&self, role: RoleId, args: &[Ty], name: &str) -> Vec<(usize, Shape)> {
        let info = &self.reg.roles[role.0];
        let env: Vec<(ParamId, Ty)> = info
            .params
            .iter()
            .copied()
            .zip(args.iter().cloned())
            .collect();
        info.methods
            .iter()
            .enumerate()
            .filter(|(_, method)| method.name == name)
            .map(|(index, method)| {
                (
                    index,
                    Shape {
                        params: method
                            .params
                            .iter()
                            .map(|(name, ty)| (name.clone(), ty.subst(&env)))
                            .collect(),
                        variadic: method.variadic,
                    },
                )
            })
            .collect()
    }

    /// Единственная форма по арности (или именам аргументов): её типы —
    /// ожидаемые типы аргументов.
    fn unique_shape(shapes: &[(usize, Shape)], args: &[ast::Argument]) -> Option<Shape> {
        let named = args.iter().any(|arg| arg.name.is_some());
        let matching: Vec<&Shape> = shapes
            .iter()
            .map(|(_, shape)| shape)
            .filter(|shape| {
                // Параметров по умолчанию нет: именованный вызов называет все.
                if named {
                    shape.params.len() == args.len()
                        && args.iter().all(|arg| {
                            arg.name.as_ref().is_some_and(|name| {
                                shape.params.iter().any(|(param, _)| *param == name.text)
                            })
                        })
                } else {
                    shape.variadic || shape.params.len() == args.len()
                }
            })
            .collect();
        match matching.as_slice() {
            [shape] => Some((*shape).clone()),
            _ => None,
        }
    }

    pub(super) fn method_call(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Name,
        args: &[ast::Argument],
        span: Span,
    ) -> Expr {
        let on_self = matches!(strip_grouping(receiver).kind, ast::ExprKind::SelfValue);
        if on_self && self.kind == Kind::Ctor {
            self.error(
                receiver.span,
                "Cannot call actions on self before construction completes",
            );
            for arg in args {
                let _ = self.expr(&arg.value, true, None);
            }
            return Self::poisoned(span);
        }
        // `Enum.case(...)`, `Type.action(...)` — не значения.
        let target = self.value(receiver, None);
        let target_ty = Self::ty_of(&target).taken();
        match target_ty.clone() {
            Ty::Object(object, type_args) => {
                self.object_method(target, object, type_args, name, args, on_self, span)
            }
            Ty::Role(role, role_args) => {
                self.role_method(target, role, role_args, name, args, span)
            }
            Ty::Param(param) => match self.reg.params[param.0].bound.clone() {
                Some(Ty::Role(role, role_args)) => {
                    self.role_method(target, role, role_args, name, args, span)
                }
                _ => {
                    let message = format!(
                        "Type parameter \"{}\" has no action \"{}\"; declare a role bound",
                        self.reg.params[param.0].name, name.text
                    );
                    self.error(name.span, message);
                    for arg in args {
                        let _ = self.expr(&arg.value, true, None);
                    }
                    Self::poisoned(span)
                }
            },
            Ty::Error => {
                for arg in args {
                    let _ = self.expr(&arg.value, true, None);
                }
                Self::poisoned(span)
            }
            Ty::Test => self.test_call(name, args, span),
            _ => self.builtin_call(target, name, args, span),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn object_method(
        &mut self,
        target: Expr,
        object: ObjectId,
        type_args: Vec<Ty>,
        name: &ast::Name,
        args: &[ast::Argument],
        on_self: bool,
        span: Span,
    ) -> Expr {
        let object_name = self.type_name(&Ty::Object(object, type_args.clone()));
        let shapes = self.method_shapes(object, &type_args, &name.text);
        if shapes.is_empty() {
            let message = format!("Object \"{object_name}\" has no action \"{}\"", name.text);
            self.error(name.span, message);
            for arg in args {
                let _ = self.expr(&arg.value, true, None);
            }
            return Self::poisoned(span);
        }
        let hint = Self::unique_shape(&shapes, args);
        let call = self.call_arguments(args, hint.as_ref());
        let plain: Vec<Shape> = shapes.iter().map(|(_, shape)| shape.clone()).collect();
        let chosen = match self.select_overload(&call, &plain) {
            Ok(index) => index,
            Err(ambiguous) => {
                let message = if ambiguous {
                    format!(
                        "Ambiguous call of action \"{}\" on object \"{object_name}\"",
                        name.text
                    )
                } else {
                    format!(
                        "No action \"{}\" on object \"{object_name}\" accepts ({})",
                        name.text,
                        self.type_list(&call)
                    )
                };
                self.error(span, message);
                return Self::poisoned(span);
            }
        };
        let (method, shape) = shapes[chosen].clone();
        let info = &self.reg.objects[object.0].methods[method];
        if info.private && !(on_self && self.object == Some(object)) {
            let message = format!(
                "Action \"{}\" on object \"{object_name}\" is private",
                name.text
            );
            self.error(name.span, message);
            return Self::poisoned(span);
        }
        let env: Vec<(ParamId, Ty)> = self.reg.objects[object.0]
            .params
            .iter()
            .copied()
            .zip(type_args.iter().cloned())
            .collect();
        let result = info.result.as_ref().map(|ty| ty.subst(&env));
        let is_async = info.is_async;
        let diverges = call.diverges || target.diverges;
        let args = self.arrange(call, &shape, &format!("Action \"{}\"", name.text), span);
        // Вызов на receiver потребляет successor-результат.
        Self::call_result(
            Callee::Method {
                receiver: Box::new(Self::consume(target)),
                object,
                method,
            },
            args,
            result,
            is_async,
            span,
            diverges,
        )
    }

    /// Вызов слота роли на ограниченной ссылке: доступны только действия роли.
    fn role_method(
        &mut self,
        target: Expr,
        role: RoleId,
        role_args: Vec<Ty>,
        name: &ast::Name,
        args: &[ast::Argument],
        span: Span,
    ) -> Expr {
        let role_name = self.type_name(&Ty::Role(role, role_args.clone()));
        let shapes = self.role_shapes(role, &role_args, &name.text);
        if shapes.is_empty() {
            let message = format!("Role \"{role_name}\" has no action \"{}\"", name.text);
            self.error(name.span, message);
            for arg in args {
                let _ = self.expr(&arg.value, true, None);
            }
            return Self::poisoned(span);
        }
        let hint = Self::unique_shape(&shapes, args);
        let call = self.call_arguments(args, hint.as_ref());
        let plain: Vec<Shape> = shapes.iter().map(|(_, shape)| shape.clone()).collect();
        let chosen = match self.select_overload(&call, &plain) {
            Ok(index) => index,
            Err(ambiguous) => {
                let message = if ambiguous {
                    format!(
                        "Ambiguous call of action \"{}\" on role \"{role_name}\"",
                        name.text
                    )
                } else {
                    format!(
                        "No action \"{}\" on role \"{role_name}\" accepts ({})",
                        name.text,
                        self.type_list(&call)
                    )
                };
                self.error(span, message);
                return Self::poisoned(span);
            }
        };
        let (slot, shape) = shapes[chosen].clone();
        let info = &self.reg.roles[role.0];
        let env: Vec<(ParamId, Ty)> = info
            .params
            .iter()
            .copied()
            .zip(role_args.iter().cloned())
            .collect();
        let result = info.methods[slot].result.as_ref().map(|ty| ty.subst(&env));
        let is_async = info.methods[slot].is_async;
        let diverges = call.diverges || target.diverges;
        let args = self.arrange(call, &shape, &format!("Action \"{}\"", name.text), span);
        // Значение `self` в позиции receiver роли: граница роли.
        let target = match Self::ty_of(&target).taken() {
            Ty::Role(..) => target,
            _ => {
                let to = Ty::Role(role, role_args.clone());
                self.coerce(target, &to, span, "Receiver")
            }
        };
        Self::call_result(
            Callee::Role {
                receiver: Box::new(Self::consume(target)),
                role,
                slot,
            },
            args,
            result,
            is_async,
            span,
            diverges,
        )
    }

    /// `test.ok(bool)`, `test.is(a, b)`, `test.isnt(a, b)`, `test.fail(string)` (§14).
    fn test_call(&mut self, name: &ast::Name, args: &[ast::Argument], span: Span) -> Expr {
        let params: Vec<Option<Ty>> = match name.text.as_str() {
            "ok" => vec![Some(Ty::Bool)],
            "is" | "isnt" => vec![None, None],
            "fail" => vec![Some(Ty::Str)],
            other => {
                self.error(
                    name.span,
                    format!("Test capability has no action \"{other}\""),
                );
                return Self::poisoned(span);
            }
        };
        if args.len() != params.len() || args.iter().any(|arg| arg.name.is_some()) {
            let message = format!(
                "test.{}() takes {} positional argument(s)",
                name.text,
                params.len()
            );
            self.error(span, message);
            return Self::poisoned(span);
        }
        let mut values = Vec::new();
        for (arg, param) in args.iter().zip(&params) {
            let value = self.value(&arg.value, param.as_ref());
            let value = match param {
                Some(ty) => self.coerce(
                    value,
                    ty,
                    arg.value.span,
                    &format!("test.{} argument", name.text),
                ),
                None => value,
            };
            values.push(value);
        }
        Self::unit(
            ExprKind::Call {
                callee: Callee::Test(name.text.clone()),
                args: values,
            },
            span,
        )
    }

    // ----- доступ к членам -----

    /// `receiver.name` без вызова: поле `self`, case enum, bound action.
    pub(super) fn get(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Name,
        span: Span,
        expected: Option<&Ty>,
    ) -> Expr {
        if let ast::ExprKind::SelfValue = strip_grouping(receiver).kind {
            let available = matches!(
                self.kind,
                Kind::Method | Kind::Successor | Kind::Ctor | Kind::Destroy
            ) || (self.kind == Kind::Closure && self.self_ty.is_some());
            if !available {
                if self.kind == Kind::CapField {
                    self.error(receiver.span, "self is not available inside a cap field");
                } else {
                    self.error(
                        receiver.span,
                        "`self` is available only inside object actions",
                    );
                }
                return Self::poisoned(span);
            }
            if let Some(field) = self.own_field(&name.text) {
                let ty = self.field_type(field);
                let kind = if self.kind == Kind::Ctor {
                    ExprKind::PendingField(field)
                } else {
                    ExprKind::Field(field)
                };
                return Self::typed(kind, ty, span);
            }
            if self.kind != Kind::Ctor {
                let target = self.self_expr_value(receiver.span);
                if let Some(bound) = self.bound_action(target, name, expected, true, span) {
                    return bound;
                }
            }
            self.no_field(name);
            return Self::poisoned(span);
        }
        // `Light.red`: case enum, если имя не перекрыто локальной привязкой.
        if let Some(id) = self.enum_receiver(receiver) {
            let decl = &self.reg.enums[id.0];
            return match decl.cases.iter().position(|case| *case == name.text) {
                Some(case) => Self::typed(ExprKind::EnumCase { ty: id, case }, Ty::Enum(id), span),
                None => {
                    let message = format!("Enum \"{}\" has no case \"{}\"", decl.name, name.text);
                    self.error(name.span, message);
                    Self::poisoned(span)
                }
            };
        }
        if let ast::ExprKind::Name(space) = &strip_grouping(receiver).kind {
            if self.local(&space.text).is_none() {
                if self.reg.is_namespace(self.module, &space.text) {
                    let message = format!(
                        "\"{}.{}\" is not a value; call it or use it as a type",
                        space.text, name.text
                    );
                    self.error(span, message);
                    return Self::poisoned(span);
                }
                if self.reg.find_type(self.module, &space.text).is_some() {
                    let message =
                        format!("Type \"{}\" has no member \"{}\"", space.text, name.text);
                    self.error(span, message);
                    return Self::poisoned(span);
                }
            }
        }
        let target = self.value(receiver, None);
        let target_ty = Self::ty_of(&target).taken();
        match &target_ty {
            Ty::Object(object, _) => {
                let object = *object;
                if let Some(bound) = self.bound_action(target, name, expected, false, span) {
                    return bound;
                }
                if self.reg.objects[object.0]
                    .fields
                    .iter()
                    .any(|field| field.name == name.text)
                {
                    let message = format!(
                        "Field \"{}\" of object \"{}\" is private; call an action instead",
                        name.text,
                        self.type_name(&target_ty)
                    );
                    self.error(name.span, message);
                } else {
                    let message = format!(
                        "Object \"{}\" has no action \"{}\"",
                        self.type_name(&target_ty),
                        name.text
                    );
                    self.error(name.span, message);
                }
                Self::poisoned(span)
            }
            Ty::Role(role, role_args) => {
                let (role, role_args) = (*role, role_args.clone());
                let actions: Vec<(usize, ActionTy)> = self
                    .reg
                    .role_actions(&target_ty)
                    .into_iter()
                    .enumerate()
                    .filter(|(_, (action, _, _, variadic, _))| *action == name.text && !variadic)
                    .map(|(slot, (_, params, result, _, is_async))| {
                        (
                            slot,
                            ActionTy {
                                is_async,
                                params,
                                result,
                            },
                        )
                    })
                    .filter(|(_, ty)| match expected {
                        Some(Ty::Action(want)) => ty == &**want,
                        _ => true,
                    })
                    .collect();
                match actions.as_slice() {
                    [(slot, ty)] => Self::typed(
                        ExprKind::BoundRoleAction {
                            receiver: Box::new(Self::consume(target)),
                            role,
                            slot: *slot,
                        },
                        Ty::Action(Box::new(ty.clone())),
                        span,
                    ),
                    [] => {
                        let message = format!(
                            "Role \"{}\" has no action \"{}\"",
                            self.type_name(&Ty::Role(role, role_args)),
                            name.text
                        );
                        self.error(name.span, message);
                        Self::poisoned(span)
                    }
                    _ => {
                        let message = format!(
                            "Bound action \"{}\" is overloaded; an exact action type must select one",
                            name.text
                        );
                        self.error(span, message);
                        Self::poisoned(span)
                    }
                }
            }
            Ty::Error => Self::poisoned(span),
            other => {
                let message = format!(
                    "Value of type {} has no member \"{}\"",
                    self.type_name(other),
                    name.text
                );
                self.error(name.span, message);
                Self::poisoned(span)
            }
        }
    }

    fn self_expr_value(&self, span: Span) -> Expr {
        let ty = self.self_ty.clone().expect("checked by the caller");
        Self::typed(ExprKind::SelfValue, ty, span)
    }

    /// `Color` или `ns.Color` как receiver case enum.
    fn enum_receiver(&mut self, receiver: &ast::Expr) -> Option<crate::analysis::ty::EnumId> {
        match &strip_grouping(receiver).kind {
            ast::ExprKind::Name(name) if self.local(&name.text).is_none() => {
                self.enum_named(&name.text)
            }
            ast::ExprKind::Get { object, name } => match &strip_grouping(object).kind {
                ast::ExprKind::Name(space) if self.local(&space.text).is_none() => {
                    match self
                        .reg
                        .find_qualified(self.module, &space.text, &name.text)
                    {
                        Some(TypeRef::Enum(id)) => Some(id),
                        _ => None,
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// `receiver.action` как значение (§8.10.1): перегрузку выбирает
    /// ожидаемый тип action; приватные действия значениями не бывают.
    fn bound_action(
        &mut self,
        target: Expr,
        name: &ast::Name,
        expected: Option<&Ty>,
        on_self: bool,
        span: Span,
    ) -> Option<Expr> {
        let Ty::Object(object, type_args) = Self::ty_of(&target).taken() else {
            return None;
        };
        let info = &self.reg.objects[object.0];
        let env: Vec<(ParamId, Ty)> = info
            .params
            .iter()
            .copied()
            .zip(type_args.iter().cloned())
            .collect();
        let candidates: Vec<(usize, ActionTy, bool)> = info
            .methods
            .iter()
            .enumerate()
            .filter(|(_, method)| method.name == name.text && !method.successor && !method.variadic)
            .map(|(index, method)| {
                (
                    index,
                    ActionTy {
                        is_async: method.is_async,
                        params: method.params.iter().map(|(_, ty)| ty.subst(&env)).collect(),
                        result: method.result.as_ref().map(|ty| ty.subst(&env)),
                    },
                    method.private,
                )
            })
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let chosen: Vec<&(usize, ActionTy, bool)> = match expected {
            Some(Ty::Action(want)) => candidates
                .iter()
                .filter(|(_, ty, _)| ty == &**want)
                .collect(),
            _ => candidates.iter().collect(),
        };
        let (method, ty, private) = match chosen.as_slice() {
            [one] => (*one).clone(),
            [] => {
                let message = format!(
                    "No action \"{}\" matches the expected action type",
                    name.text
                );
                self.error(span, message);
                return Some(Self::poisoned(span));
            }
            _ => {
                let message = format!(
                    "Bound action \"{}\" is overloaded; an exact action type must select one",
                    name.text
                );
                self.error(span, message);
                return Some(Self::poisoned(span));
            }
        };
        if private && !on_self {
            let message = format!("Private action \"{}\" cannot be used as a value", name.text);
            self.error(name.span, message);
            return Some(Self::poisoned(span));
        }
        Some(Self::typed(
            ExprKind::BoundAction {
                receiver: Box::new(Self::consume(target)),
                object,
                method,
            },
            Ty::Action(Box::new(ty)),
            span,
        ))
    }

    // ----- проекции -----

    /// `prefix Object(...)`: конструирование, затем действие `#projected(prefix)`.
    pub(super) fn projection(
        &mut self,
        prefix: &ast::Name,
        construction: &ast::Expr,
        span: Span,
    ) -> Expr {
        let target = match &construction.kind {
            ast::ExprKind::Call { callee, .. } => match &strip_grouping(callee).kind {
                ast::ExprKind::Name(name) => match self.reg.find_type(self.module, &name.text) {
                    Some(TypeRef::Object(object)) => Some(object),
                    _ => None,
                },
                ast::ExprKind::Get { object, name } => match &strip_grouping(object).kind {
                    ast::ExprKind::Name(space) if self.local(&space.text).is_none() => {
                        match self
                            .reg
                            .find_qualified(self.module, &space.text, &name.text)
                        {
                            Some(TypeRef::Object(object)) => Some(object),
                            _ => None,
                        }
                    }
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };
        let Some(object) = target else {
            self.error(
                prefix.span,
                "Constructor projection target must resolve to an object",
            );
            let _ = self.expr(construction, true, None);
            return Self::poisoned(span);
        };
        let Some(method) = self.reg.objects[object.0]
            .methods
            .iter()
            .position(|method| method.projection.as_deref() == Some(prefix.text.as_str()))
        else {
            let message = format!(
                "Object \"{}\" has no projection prefix \"{}\"",
                self.reg.objects[object.0].name, prefix.text
            );
            self.error(prefix.span, message);
            let _ = self.expr(construction, true, None);
            return Self::poisoned(span);
        };
        let built = self.value(construction, None);
        let type_args = match Self::ty_of(&built).taken() {
            Ty::Object(_, args) => args,
            _ => return Self::poisoned(span),
        };
        let info = &self.reg.objects[object.0];
        let env: Vec<(ParamId, Ty)> = info.params.iter().copied().zip(type_args).collect();
        let result = info.methods[method]
            .result
            .as_ref()
            .map_or(Ty::Error, |ty| ty.subst(&env));
        let diverges = built.diverges;
        let mut out = Self::typed(
            ExprKind::Projection {
                construction: Box::new(built),
                object,
                method,
            },
            result,
            span,
        );
        out.diverges = diverges;
        out
    }

    // ----- анонимные действия -----

    /// `(a is int) is int { … }`: значение с захваченными привязками (§8.10.2).
    pub(super) fn closure(
        &mut self,
        params: &[ast::Param],
        result: &ast::ActionResult,
        body: &ast::Block,
        span: Span,
    ) -> Expr {
        let generics = self.generics.clone();
        let mut param_types = Vec::new();
        for param in params {
            if param.variadic {
                self.error(
                    param.span,
                    "Anonymous actions cannot declare variadic parameters",
                );
            }
            param_types.push(self.reg.resolve(&param.ty, &generics));
        }
        let result_ty = match result {
            ast::ActionResult::Void(_) => None,
            ast::ActionResult::Type(ty) => Some(self.reg.resolve(ty, &generics)),
        };
        let visible = self.visible();
        let outer = Outer {
            visible,
            self_ty: if matches!(
                self.kind,
                Kind::Method | Kind::Successor | Kind::Destroy | Kind::Closure
            ) {
                self.self_ty.clone()
            } else {
                None
            },
        };
        let (object, self_ty, module) = (self.object, outer.self_ty.clone(), self.module);
        let checked = {
            let mut inner = Checker::new(&mut *self.reg, Kind::Closure, object, result_ty.clone());
            inner.module = module;
            inner.self_ty = self_ty;
            inner.outer = Some(outer);
            let mut locals = Vec::new();
            for (param, ty) in params.iter().zip(&param_types) {
                locals.push(inner.declare(&param.name, ty.clone(), false));
            }
            let block = inner.body(body, "<anonymous>");
            let captures = std::mem::take(&mut inner.captures);
            let body = inner.finish(BodyOwner::Closure, locals, block, span);
            (body, captures)
        };
        self.reg.set_module(self.module);
        let (body, captures) = checked;
        let ty = Ty::Action(Box::new(ActionTy {
            is_async: false,
            params: param_types,
            result: result_ty,
        }));
        Self::typed(
            ExprKind::Closure {
                body: Box::new(body),
                captures,
            },
            ty,
            span,
        )
    }

    /// `T(x)`: приведение одного значения (§6.6.1); `rat(n, d)` — конструктор.
    pub(super) fn primitive_cast(
        &mut self,
        target: &str,
        args: &[ast::Argument],
        span: Span,
    ) -> Expr {
        if target == "rat" {
            let [numerator, denominator] = args else {
                self.error(span, "`rat(...)` takes a numerator and a denominator");
                return Self::poisoned(span);
            };
            if numerator.name.is_some() || denominator.name.is_some() {
                self.error(span, "`rat(...)` takes positional arguments only");
                return Self::poisoned(span);
            }
            let mut parts = Vec::new();
            for arg in [numerator, denominator] {
                let value = self.value(&arg.value, Some(&Ty::Int));
                let ty = Self::ty_of(&value).taken();
                if !matches!(ty, Ty::Int | Ty::Byte | Ty::Error) {
                    let message = format!(
                        "rat() arguments must be int or byte, got {}",
                        self.type_name(&ty)
                    );
                    self.error(arg.value.span, message);
                }
                parts.push(value);
            }
            let denominator = parts.pop().expect("two parts");
            let numerator = parts.pop().expect("two parts");
            return Self::typed(
                ExprKind::RatNew {
                    numerator: Box::new(numerator),
                    denominator: Box::new(denominator),
                },
                Ty::Rat,
                span,
            );
        }
        let [arg] = args else {
            self.error(
                span,
                format!("`{target}(...)` takes exactly one positional argument"),
            );
            return Self::poisoned(span);
        };
        if arg.name.is_some() || matches!(arg.value.kind, ast::ExprKind::Unpack { .. }) {
            self.error(
                arg.span,
                format!("`{target}(...)` takes exactly one positional argument"),
            );
            return Self::poisoned(span);
        }
        let value = self.value(&arg.value, None);
        let cast = match target {
            "int" => CastTarget::Int,
            "byte" => CastTarget::Byte,
            "float" => CastTarget::Float,
            _ => CastTarget::Str,
        };
        self.convert(value, cast, arg.value.span, None, span)
    }
}

/// Тип параметра для аргумента `index` (или по имени).
fn param_hint(shape: &Shape, index: usize, name: Option<&str>) -> Option<Ty> {
    if let Some(name) = name {
        return shape
            .params
            .iter()
            .find(|(param, _)| param == name)
            .map(|(_, ty)| ty.clone());
    }
    if shape.variadic && index + 1 >= shape.params.len() {
        return match &shape.params.last()?.1 {
            Ty::Array(element) => Some((**element).clone()),
            _ => None,
        };
    }
    shape.params.get(index).map(|(_, ty)| ty.clone())
}

fn mentions_any(ty: &Ty) -> bool {
    match ty {
        Ty::Any => true,
        Ty::Array(inner) => mentions_any(inner),
        Ty::Map(key, value) => mentions_any(key) || mentions_any(value),
        _ => false,
    }
}
