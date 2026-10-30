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
    ///
    /// Вывод идёт по каждому конструктору, которому вызов подходит по форме
    /// (сначала фиксированные и shorthand, затем variadic, §11.3). Разные
    /// специализации от разных конструкторов сужаются до тех, при которых
    /// конструктор принимает аргументы по типам.
    pub(super) fn infer_specialization(
        &mut self,
        object: ObjectId,
        call: &CallArgs,
        expected: Option<&Ty>,
        span: Span,
    ) -> Option<Vec<Ty>> {
        let count = self.reg.objects[object.0].ctors.len();
        let variadic = |ctor: &CtorShape<'_>| matches!(ctor, CtorShape::Full { variadic: true, .. });
        let mut order: Vec<usize> = (0..count)
            .filter(|ctor| !variadic(&self.reg.objects[object.0].ctors[*ctor].shape))
            .collect();
        let tails = order.len();
        order.extend((0..count).filter(|ctor| variadic(&self.reg.objects[object.0].ctors[*ctor].shape)));
        let mut found: Vec<Vec<Ty>> = Vec::new();
        let mut failure = None;
        let mut shaped = false;
        for (position, ctor) in order.iter().enumerate() {
            if position == tails && !found.is_empty() {
                break;
            }
            let Some((patterns, tail)) = self.ctor_patterns(object, *ctor, call) else {
                continue;
            };
            shaped = true;
            match self.infer_with(object, call, expected, &patterns, tail.as_ref()) {
                Ok(resolved) if !found.contains(&resolved) => found.push(resolved),
                Ok(_) => {}
                Err(message) => {
                    failure.get_or_insert(message);
                }
            }
        }
        if !shaped {
            // Ни одной подходящей формы: специализацию задаёт только ожидаемый
            // тип, а отказ конструирования сообщит выбор конструктора.
            let none = vec![None; call.values.len()];
            match self.infer_with(object, call, expected, &none, None) {
                Ok(resolved) => found.push(resolved),
                Err(message) => failure = Some(message),
            }
        }
        if found.len() > 1 {
            found.retain(|resolved| {
                let env: Vec<(ParamId, Ty)> = self.reg.objects[object.0]
                    .params
                    .iter()
                    .copied()
                    .zip(resolved.iter().cloned())
                    .collect();
                let shapes = self.ctor_shapes(object, &env);
                !self.ctor_candidates(object, &shapes, call, &env).1.is_empty()
            });
        }
        match found.len() {
            1 => found.pop(),
            0 => {
                let message = failure.unwrap_or_else(|| {
                    format!(
                        "Cannot infer generic parameters for object \"{}\"",
                        self.reg.objects[object.0].name
                    )
                });
                self.error(span, message);
                None
            }
            _ => {
                let message = format!(
                    "Ambiguous construction of \"{}\": constructors infer different generic arguments",
                    self.reg.objects[object.0].name
                );
                self.error(span, message);
                None
            }
        }
    }

    /// Специализация по образцам аргументов одного конструктора.
    fn infer_with(
        &self,
        object: ObjectId,
        call: &CallArgs,
        expected: Option<&Ty>,
        patterns: &[Option<Ty>],
        tail: Option<&Ty>,
    ) -> Result<Vec<Ty>, String> {
        let mut inference = self.inference(object);
        if let Some(expected) = expected {
            self.bind_expected(object, &mut inference, expected);
        }
        for (arg, pattern) in call.values.iter().zip(patterns) {
            if let Some(pattern) = pattern {
                let actual = Self::ty_of(&arg.value).taken();
                self.unify(&mut inference, pattern, &actual);
            }
        }
        // Финальная распаковка `xs...` — массив элементов variadic-хвоста.
        if let (Some(element), Some((array, _))) = (tail, &call.spread) {
            if let Ty::Array(item) = Self::ty_of(array).taken() {
                self.unify(&mut inference, element, &item);
            }
        }
        if let Some(message) = inference.conflict.take() {
            return Err(message);
        }
        inference
            .params
            .iter()
            .zip(inference.bindings)
            .map(|(param, binding)| {
                binding.ok_or_else(|| {
                    format!(
                        "Cannot infer generic parameter \"{}\" for object \"{}\"",
                        self.reg.params[param.0].name, inference.object
                    )
                })
            })
            .collect()
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

    /// Образцы типов аргументов для конструктора `ctor`, если вызов подходит
    /// ему по форме: shorthand — типы полей по именам, полный конструктор —
    /// типы параметров (хвост — тип элемента). Второе значение — образец
    /// элемента хвоста для финальной распаковки.
    fn ctor_patterns(
        &self,
        object: ObjectId,
        ctor: usize,
        call: &CallArgs,
    ) -> Option<(Vec<Option<Ty>>, Option<Ty>)> {
        let info = &self.reg.objects[object.0];
        let count = call.values.len();
        if call.named() {
            let fields: Vec<(String, Ty)> = match &info.ctors[ctor].shape {
                CtorShape::Shorthand { fields, .. } => fields
                    .iter()
                    .map(|field| {
                        (
                            info.fields[*field].name.clone(),
                            info.fields[*field].ty.clone(),
                        )
                    })
                    .collect(),
                CtorShape::Full {
                    params, variadic, ..
                } if !variadic && params.len() == count => params.clone(),
                CtorShape::Full { .. } => return None,
            };
            let names = call.names();
            if !names
                .iter()
                .all(|name| fields.iter().any(|(field, _)| field == name))
            {
                return None;
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
            return Some((patterns, None));
        }
        match &info.ctors[ctor].shape {
            CtorShape::Full {
                params, variadic, ..
            } if *variadic && count + 1 >= params.len() => {
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
                Some((patterns, Some(element)))
            }
            CtorShape::Full {
                params, variadic, ..
            } if !variadic && params.len() == count && call.spread.is_none() => Some((
                params.iter().map(|(_, ty)| Some(ty.clone())).collect(),
                None,
            )),
            // `Box()` у shorthand: всё — из ожидаемого типа и значений по умолчанию.
            CtorShape::Shorthand { .. } if count == 0 => Some((Vec::new(), None)),
            _ => None,
        }
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
