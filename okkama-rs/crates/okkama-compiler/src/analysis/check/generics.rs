//! Вывод аргументов generic-объекта на месте конструирования (§6.5).
//!
//! `Box(value: 7)` называет объявление, а не специализацию: аргументы типа
//! выводятся из ожидаемого типа (`let b is Box<int> = …`, в том числе роли,
//! которую делает объект), затем из аргументов конструктора по типам его
//! параметров. Уравнения точные: первое свидетельство параметра — его тип,
//! другое — конфликт; роль совместима с объектом, который её делает.
//! Пустые нетегированные коллекции свидетельством не являются.

use super::Checker;
use super::call::CallArgs;
use crate::analysis::registry::CtorShape;
use crate::analysis::ty::{ObjectId, ParamId, Ty};
use crate::source::Span;

struct Inference {
    object: String,
    params: Vec<ParamId>,
    bindings: Vec<Option<Ty>>,
    conflict: Option<String>,
}

impl Checker<'_, '_> {
    /// Специализация, которую полностью задаёт ожидаемый тип.
    pub(super) fn expected_specialization(
        &mut self,
        object: ObjectId,
        expected: Option<&Ty>,
    ) -> Option<Vec<Ty>> {
        let expected = expected?;
        let mut inference = self.inference(object);
        self.bind_expected(object, &mut inference, expected);
        if inference.conflict.is_some() {
            return None;
        }
        inference.bindings.into_iter().collect()
    }

    fn inference(&self, object: ObjectId) -> Inference {
        let params = self.reg.objects[object.0].params.clone();
        Inference {
            object: self.reg.objects[object.0].name.clone(),
            bindings: vec![None; params.len()],
            params,
            conflict: None,
        }
    }

    fn bind_expected(&self, object: ObjectId, inference: &mut Inference, expected: &Ty) {
        match expected.taken() {
            Ty::Object(id, args) if id == object => {
                for (index, arg) in args.iter().enumerate() {
                    self.bind(inference, index, arg);
                }
            }
            role @ Ty::Role(..) => {
                // Роль, которую делает объявление: аргументы роли — образцы.
                let info = &self.reg.objects[object.0];
                let does = info.does.clone();
                let mut matching = does
                    .iter()
                    .filter(|(id, _)| matches!(&role, Ty::Role(target, _) if target == id));
                if let (Some((id, pattern)), None) = (matching.next(), matching.next()) {
                    let pattern = Ty::Role(*id, pattern.clone());
                    self.unify(inference, &pattern, &role);
                }
            }
            _ => {}
        }
    }

    /// Вывести специализацию по аргументам; ошибка сообщается здесь.
    pub(super) fn infer_specialization(
        &mut self,
        object: ObjectId,
        call: &CallArgs,
        expected: Option<&Ty>,
        span: Span,
    ) -> Option<Vec<Ty>> {
        let mut inference = self.inference(object);
        if let Some(expected) = expected {
            self.bind_expected(object, &mut inference, expected);
        }
        let (patterns, tail) = self.ctor_patterns(object, call);
        for (arg, pattern) in call.values.iter().zip(&patterns) {
            if let Some(pattern) = pattern {
                let actual = Self::ty_of(&arg.value).taken();
                self.unify(&mut inference, pattern, &actual);
            }
        }
        // Финальная распаковка `xs...` — массив элементов variadic-хвоста.
        if let (Some(element), Some((array, _))) = (&tail, &call.spread) {
            if let Ty::Array(item) = Self::ty_of(array).taken() {
                self.unify(&mut inference, element, &item);
            }
        }
        if let Some(message) = inference.conflict.take() {
            self.error(span, message);
            return None;
        }
        let mut resolved = Vec::new();
        for (param, binding) in inference.params.iter().zip(&inference.bindings) {
            match binding {
                Some(ty) => resolved.push(ty.clone()),
                None => {
                    let message = format!(
                        "Cannot infer generic parameter \"{}\" for object \"{}\"",
                        self.reg.params[param.0].name, inference.object
                    );
                    self.error(span, message);
                    return None;
                }
            }
        }
        Some(resolved)
    }

    /// Ограничения параметров специализации (§6.5): проверка до тела конструктора.
    pub(super) fn check_specialization(
        &mut self,
        object: ObjectId,
        args: &[Ty],
        span: Span,
    ) -> bool {
        let params = self.reg.objects[object.0].params.clone();
        let env: Vec<(ParamId, Ty)> = params.iter().copied().zip(args.iter().cloned()).collect();
        for (param, arg) in params.iter().zip(args) {
            let Some(bound) = self.reg.params[param.0].bound.clone() else {
                continue;
            };
            let required = bound.subst(&env);
            if arg.is_error() || self.reg.assignable(arg, &required) {
                continue;
            }
            let message = format!(
                "Generic parameter \"{}\" for object \"{}\" has type {}, which does not satisfy {}",
                self.reg.params[param.0].name,
                self.reg.objects[object.0].name,
                self.type_name(arg),
                self.type_name(&required)
            );
            self.error(span, message);
            return false;
        }
        true
    }

    /// Образцы типов аргументов: shorthand — типы полей по именам, полный
    /// конструктор — типы параметров (хвост — тип элемента). Второе значение
    /// — образец элемента хвоста для финальной распаковки.
    fn ctor_patterns(&self, object: ObjectId, call: &CallArgs) -> (Vec<Option<Ty>>, Option<Ty>) {
        let info = &self.reg.objects[object.0];
        let count = call.values.len();
        if call.named() {
            let names = call.names();
            for ctor in &info.ctors {
                let fields: Option<Vec<(String, Ty)>> = match &ctor.shape {
                    CtorShape::Shorthand { fields, .. } => Some(
                        fields
                            .iter()
                            .map(|field| {
                                (
                                    info.fields[*field].name.clone(),
                                    info.fields[*field].ty.clone(),
                                )
                            })
                            .collect(),
                    ),
                    CtorShape::Full {
                        params, variadic, ..
                    } if !variadic && params.len() == count => Some(params.clone()),
                    CtorShape::Full { .. } => None,
                };
                let Some(fields) = fields else {
                    continue;
                };
                if !names
                    .iter()
                    .all(|name| fields.iter().any(|(field, _)| field == name))
                {
                    continue;
                }
                let patterns = call
                    .values
                    .iter()
                    .map(|arg| {
                        let name = arg.name.as_deref()?;
                        fields
                            .iter()
                            .find(|(field, _)| field == name)
                            .map(|(_, ty)| ty.clone())
                    })
                    .collect();
                return (patterns, None);
            }
            return (vec![None; count], None);
        }
        for ctor in &info.ctors {
            if let CtorShape::Full {
                params, variadic, ..
            } = &ctor.shape
            {
                if *variadic && count + 1 >= params.len() {
                    let element = match &params[params.len() - 1].1 {
                        Ty::Array(element) => (**element).clone(),
                        other => other.clone(),
                    };
                    let patterns = (0..count)
                        .map(|index| {
                            Some(if index + 1 < params.len() {
                                params[index].1.clone()
                            } else {
                                element.clone()
                            })
                        })
                        .collect();
                    return (patterns, Some(element));
                }
                if !variadic && params.len() == count {
                    return (
                        params.iter().map(|(_, ty)| Some(ty.clone())).collect(),
                        None,
                    );
                }
            }
        }
        (vec![None; count], None)
    }

    /// Сопоставить образец (в параметрах объявления) с фактическим типом.
    fn unify(&self, inference: &mut Inference, pattern: &Ty, actual: &Ty) {
        if actual.is_error() {
            return;
        }
        match (pattern, actual) {
            (Ty::Param(param), _) => {
                if let Some(index) = inference.params.iter().position(|known| known == param) {
                    self.bind(inference, index, actual);
                }
            }
            (Ty::Array(element), Ty::Array(item)) if **item != Ty::Never => {
                self.unify(inference, element, item);
            }
            (Ty::Array(element), Ty::Varargs(item)) => self.unify(inference, element, item),
            (Ty::Map(key, value), Ty::Map(actual_key, actual_value))
                if **actual_key != Ty::Never =>
            {
                self.unify(inference, key, actual_key);
                self.unify(inference, value, actual_value);
            }
            (Ty::Successor(inner), Ty::Successor(other)) => self.unify(inference, inner, other),
            (Ty::Future(Some(inner)), Ty::Future(Some(other))) => {
                self.unify(inference, inner, other)
            }
            (Ty::Action(want), Ty::Action(have)) if want.params.len() == have.params.len() => {
                for (left, right) in want.params.iter().zip(&have.params) {
                    self.unify(inference, left, right);
                }
                if let (Some(left), Some(right)) = (&want.result, &have.result) {
                    self.unify(inference, left, right);
                }
            }
            (Ty::Object(id, args), Ty::Object(other, actual_args)) if id == other => {
                for (left, right) in args.iter().zip(actual_args) {
                    self.unify(inference, left, right);
                }
            }
            (Ty::Role(id, args), Ty::Role(other, actual_args)) if id == other => {
                for (left, right) in args.iter().zip(actual_args) {
                    self.unify(inference, left, right);
                }
            }
            // Роль-образец и другая роль, сужаемая до неё: по сигнатурам действий.
            (Ty::Role(..), Ty::Role(..)) => {
                let available = self.reg.role_actions(actual);
                for (name, params, result, _, _) in self.reg.role_actions(pattern) {
                    let Some((_, actual_params, actual_result, _, _)) =
                        available.iter().find(|(other, other_params, ..)| {
                            *other == name && other_params.len() == params.len()
                        })
                    else {
                        continue;
                    };
                    for (left, right) in params.iter().zip(actual_params) {
                        self.unify(inference, left, right);
                    }
                    if let (Some(left), Some(right)) = (&result, actual_result) {
                        self.unify(inference, left, right);
                    }
                }
            }
            // Роль-образец и объект (или builtin), который её делает.
            (Ty::Role(id, args), _) => {
                let roles = self.reg.roles_of(actual);
                let found = roles.iter().find_map(|role| match role {
                    Ty::Role(other, actual_args) if other == id => Some(actual_args.clone()),
                    _ => None,
                });
                if let Some(actual_args) = found {
                    for (left, right) in args.iter().zip(&actual_args) {
                        self.unify(inference, left, right);
                    }
                }
            }
            _ => {}
        }
    }

    fn bind(&self, inference: &mut Inference, index: usize, actual: &Ty) {
        // Пустые нетегированные коллекции не свидетельствуют о типе.
        if actual.is_untagged()
            || *actual == Ty::Never
            || actual.is_error()
            || inference.conflict.is_some()
        {
            return;
        }
        let actual = actual.taken();
        match inference.bindings[index].clone() {
            None => inference.bindings[index] = Some(actual),
            Some(existing) if existing == actual => {}
            // Роль совместима с объектом, который её делает: остаётся роль.
            Some(existing @ Ty::Role(..)) if self.reg.assignable(&actual, &existing) => {}
            Some(existing)
                if matches!(actual, Ty::Role(..)) && self.reg.assignable(&existing, &actual) =>
            {
                inference.bindings[index] = Some(actual);
            }
            Some(existing) => {
                inference.conflict = Some(format!(
                    "Generic parameter \"{}\" for object \"{}\" cannot be both {} and {}",
                    self.reg.params[inference.params[index].0].name,
                    inference.object,
                    self.type_name(&existing),
                    self.type_name(&actual)
                ));
            }
        }
    }
}
