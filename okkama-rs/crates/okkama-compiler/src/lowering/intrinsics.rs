//! Intrinsics std `async/*` (OIR-1 §14.2): действия и конструкторы объектов,
//! чьё состояние держит рантайм. Lowering выпускает для них функции без тела
//! `[intrinsic "okkama.<модуль>.<Объект>.<действие>"]` со списком объектов,
//! которые рантайм создаёт (результаты, Error std); тело даёт backend.

use super::ctx::Ctx;
use super::{LoweringError, Result};
use crate::analysis::registry::{CtorShape, TypeRef};
use crate::analysis::ty::Ty;
use crate::ir::{self, ObjectId, ResultType, Type};
use crate::source::Span;

/// Объекты std с intrinsic-действиями: модуль и имя.
const OBJECTS: [(&str, &str); 21] = [
    ("async/scope", "FutureScope"),
    ("async/lock", "Lock"),
    ("async/lock", "LockGuard"),
    ("async/semaphore", "Semaphore"),
    ("async/semaphore", "Permit"),
    ("async/barrier", "Barrier"),
    ("async/barrier", "BarrierState"),
    ("async/channel", "Channel"),
    ("async/channel", "Sending"),
    ("async/channel", "Receiving"),
    ("async/event", "Event"),
    ("async/atomic", "Atomic"),
    ("async/cancellation", "Cancellation"),
    ("async/cancellation", "CancellationToken"),
    ("async/scope", "FutureGroup"),
    ("async/collect", "AllFutures"),
    ("async/collect", "Raced"),
    ("async/collect", "FirstSuccessful"),
    ("async/collect", "FutureCompletions"),
    ("async/timeout", "Within"),
    ("async/timeout", "Before"),
];

/// Действие или конструктор объекта.
#[derive(Copy, Clone)]
pub(super) enum Member {
    Method(usize),
    Ctor(usize),
}

/// Объекты, которые рантайм создаёт для intrinsic: `Self` — сам объект,
/// `X<T>` — объект модуля с аргументами receiver, `X` — без аргументов,
/// `core::ArraySequence` — адаптер `@T` как `Sequencing<T>`.
fn related(name: &str) -> &'static [&'static str] {
    match name {
        "okkama.channel.Channel.new_capacity" => &["Self", "InvalidChannelCapacity"],
        "okkama.channel.Channel.sending" => &["Sending<T>"],
        "okkama.channel.Channel.receiving" => &["Receiving<T>"],
        "okkama.channel.Sending.send"
        | "okkama.channel.Receiving.item"
        | "okkama.channel.Sending.close"
        | "okkama.channel.Receiving.close" => &["ChannelClosed"],
        "okkama.channel.Sending.new"
        | "okkama.channel.Receiving.new"
        | "okkama.event.Event.new"
        | "okkama.event.Event.new_ready"
        | "okkama.atomic.Atomic.new_value"
        | "okkama.cancellation.Cancellation.new"
        | "okkama.cancellation.CancellationToken.new"
        | "okkama.scope.FutureGroup.new" => &["Self"],
        "okkama.atomic.Atomic.value"
        | "okkama.atomic.Atomic.replace"
        | "okkama.atomic.Atomic.compare_and_replace" => &["AtomicClosed"],
        "okkama.cancellation.Cancellation.cancelling" => &["CancellationToken"],
        "okkama.scope.FutureGroup.start" => &["FinishedFutureGroup"],
        "okkama.collect.AllFutures.results" => &["core::ArraySequence"],
        "okkama.collect.Raced.result"
        | "okkama.collect.FirstSuccessful.result"
        | "okkama.collect.FutureCompletions.completion" => &["NoFutures"],
        "okkama.scope.FutureScope.result" => &["FutureGroup<T0>"],
        "okkama.lock.Lock.new" => &["Self"],
        "okkama.lock.Lock.guard" => &["LockGuard"],
        "okkama.semaphore.Semaphore.permit" => &["Permit"],
        "okkama.barrier.Barrier.wait" => &["BrokenBarrier"],
        "okkama.barrier.Barrier.state" => &["BarrierState"],
        "okkama.timeout.Within.result" | "okkama.timeout.Before.result" => &["TimedOut"],
        _ => &[],
    }
}

/// Действия программы, которые вызывает intrinsic: (значение, действие,
/// арность). Значение — поле receiver или `^N`: тип результата N-го.
fn helpers(name: &str) -> &'static [(&'static str, &'static str, usize)] {
    match name {
        "okkama.timeout.Within.result" => &[("limit", "seconds", 0), ("clock", "wait", 1)],
        "okkama.timeout.Before.result" => &[
            ("clock", "monotonic_instant", 0),
            ("deadline", "minus", 1),
            ("^1", "seconds", 0),
            ("clock", "wait", 1),
        ],
        _ => &[],
    }
}

impl Ctx<'_, '_> {
    /// Имя intrinsic члена объекта, если его даёт рантайм.
    pub(super) fn intrinsic_name(&self, object: ObjectId, member: Member) -> Option<String> {
        let decl = self.objects[object.0].decl;
        let info = &self.reg.objects[decl.0];
        let module = self.reg.modules[info.module].name.as_str();
        if !OBJECTS.contains(&(module, info.name.as_str())) {
            return None;
        }
        let family = module.rsplit('/').next().unwrap_or(module);
        let action = match member {
            Member::Method(method) => info.methods[method].name.clone(),
            Member::Ctor(ctor) => {
                let names: Vec<String> = match &info.ctors[ctor].shape {
                    CtorShape::Full { params, .. } => {
                        params.iter().map(|(name, _)| name.clone()).collect()
                    }
                    CtorShape::Shorthand { fields, .. } => fields
                        .iter()
                        .map(|field| info.fields[*field].name.clone())
                        .collect(),
                };
                std::iter::once("new".to_string())
                    .chain(names)
                    .collect::<Vec<_>>()
                    .join("_")
            }
        };
        let name = format!("okkama.{family}.{}.{action}", info.name);
        ir::INTRINSICS.contains(&name.as_str()).then_some(name)
    }

    /// Объекты, которые рантайм создаёт для intrinsic `name` объекта `object`.
    fn intrinsic_objects(
        &mut self,
        object: ObjectId,
        name: &str,
        span: Span,
    ) -> Result<Vec<ObjectId>> {
        let decl = self.objects[object.0].decl;
        let module = self.reg.objects[decl.0].module;
        let args: Vec<Ty> = self.objects[object.0]
            .env
            .iter()
            .map(|(_, ty)| ty.clone())
            .collect();
        let mut objects = Vec::new();
        for entry in related(name) {
            let id = match *entry {
                "Self" => object,
                "core::ArraySequence" => {
                    let adapter = self.reg.adapters.array_sequence.ok_or_else(|| {
                        LoweringError::invalid(span, "internal compiler error: no Sequencing role")
                    })?;
                    let element = args.first().cloned().unwrap_or(Ty::Never);
                    self.object_instance(adapter, vec![element.clone(), element], span)?
                }
                other => {
                    // `X<T>` — все аргументы receiver, `X<T0>` — только первый.
                    let (name, arity) =
                        match (other.strip_suffix("<T>"), other.strip_suffix("<T0>")) {
                            (Some(name), _) => (name, args.len()),
                            (_, Some(name)) => (name, 1),
                            _ => (other, 0),
                        };
                    let Some(TypeRef::Object(found)) =
                        self.reg.modules[module].own.types.get(name).copied()
                    else {
                        return Err(LoweringError::invalid(
                            span,
                            format!("internal compiler error: intrinsic object {name} is missing"),
                        ));
                    };
                    let type_args = args.iter().take(arity).cloned().collect();
                    self.object_instance(found, type_args, span)?
                }
            };
            objects.push(id);
        }
        Ok(objects)
    }

    /// Переходники действий программы для intrinsic `name` (см. [`helpers`]).
    fn intrinsic_helpers(
        &mut self,
        object: ObjectId,
        name: &str,
        span: Span,
    ) -> Result<Vec<ir::FuncId>> {
        let decl = self.objects[object.0].decl;
        let env = self.objects[object.0].env.clone();
        let reg = self.reg;
        let info = &reg.objects[decl.0];
        let mut found: Vec<(ir::FuncId, Type)> = Vec::new();
        for (path, action, arity) in helpers(name) {
            let receiver = if let Some(index) = path.strip_prefix('^') {
                let index: usize = index.parse().expect("helper index");
                found[index].1.clone()
            } else {
                let field = info
                    .fields
                    .iter()
                    .find(|field| field.name == *path)
                    .ok_or_else(|| {
                        LoweringError::invalid(
                            span,
                            format!("internal compiler error: no field {path}"),
                        )
                    })?;
                self.ir_type(&field.ty.subst(&env), span)?
            };
            let func = self.member_thunk(receiver.clone(), action, *arity, false);
            // Тип результата действия: для цепочки `^N`.
            let result = match &receiver {
                Type::Object(target) => {
                    let target_env = self.objects[target.0].env.clone();
                    let target_info = &reg.objects[self.objects[target.0].decl.0];
                    let method = target_info
                        .methods
                        .iter()
                        .find(|method| method.name == *action && method.params.len() == *arity);
                    match method.map(|method| method.result.clone()) {
                        Some(result) => match self.ir_result(&result, &target_env, span)? {
                            ResultType::Value(ty) => ty,
                            ResultType::Void => Type::Never,
                        },
                        None => Type::Never,
                    }
                }
                Type::Role(role) => match self.roles[role.0]
                    .methods
                    .iter()
                    .find(|method| method.name == *action)
                    .map(|method| method.result.clone())
                {
                    Some(ResultType::Value(ty)) => ty,
                    _ => Type::Never,
                },
                _ => Type::Never,
            };
            found.push((func, result));
        }
        Ok(found.into_iter().map(|(func, _)| func).collect())
    }

    /// Функция intrinsic: параметры (receiver первым у действия), результат,
    /// владелец; тела нет.
    pub(super) fn lower_intrinsic(
        &mut self,
        symbol: String,
        object: ObjectId,
        member: Member,
        name: String,
    ) -> Result<ir::Function> {
        let decl = self.objects[object.0].decl;
        let env = self.objects[object.0].env.clone();
        let span = self.objects[object.0].span;
        let reg = self.reg;
        let info = &reg.objects[decl.0];
        let mut values = Vec::new();
        let mut attrs = Vec::new();
        let result = match member {
            Member::Method(method) => {
                let target = &info.methods[method];
                values.push(Type::Object(object));
                for (_, ty) in &target.params {
                    values.push(self.ir_type(&ty.subst(&env), span)?);
                }
                attrs.push(ir::Attr::Method(object));
                if target.is_async {
                    attrs.push(ir::Attr::Async);
                }
                self.ir_result(&target.result, &env, span)?
            }
            Member::Ctor(ctor) => {
                let types: Vec<Ty> = match &info.ctors[ctor].shape {
                    CtorShape::Full { params, .. } => {
                        params.iter().map(|(_, ty)| ty.clone()).collect()
                    }
                    CtorShape::Shorthand { fields, .. } => fields
                        .iter()
                        .map(|field| info.fields[*field].ty.clone())
                        .collect(),
                };
                for ty in types {
                    values.push(self.ir_type(&ty.subst(&env), span)?);
                }
                attrs.push(ir::Attr::Ctor(object));
                ResultType::Value(Type::Object(object))
            }
        };
        let objects = self.intrinsic_objects(object, &name, span)?;
        let helpers = self.intrinsic_helpers(object, &name, span)?;
        attrs.push(ir::Attr::Intrinsic {
            name,
            objects,
            helpers,
        });
        let params = (0..values.len()).map(ir::ValueId).collect();
        Ok(ir::Function {
            symbol,
            params,
            result,
            attrs,
            values,
            blocks: Vec::new(),
            span,
        })
    }
}
