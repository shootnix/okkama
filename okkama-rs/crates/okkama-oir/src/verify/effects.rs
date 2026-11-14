use super::State;
use crate::*;
use std::collections::BTreeSet;

pub(super) fn cleanup(module: &Module, body: &Body, ty: TypeId, state: &mut State<'_>) -> Effects {
    let mut result = Effects::default();
    let mut pending = vec![ty];
    let mut seen = BTreeSet::new();
    let mut bindings = Vec::<(TypeParameterRef, TypeId)>::new();
    let mut used = Vec::<TypeParameterRef>::new();
    while let Some(ty) = pending.pop() {
        if !state.step(1) {
            return result;
        }
        if !seen.insert(ty) {
            continue;
        }
        match module.types.get(ty) {
            Ok(Type::Object { decl, args }) => {
                for (index, argument) in args.iter().enumerate() {
                    let parameter = TypeParameterRef { owner: GenericOwner::Declaration(*decl), index: index as u32 };
                    if !bindings.contains(&(parameter, *argument)) {
                        bindings.push((parameter, *argument));
                        if used.contains(&parameter) {
                            pending.push(*argument);
                        }
                    }
                }
                if let Ok(Declaration { kind: DeclarationKind::Object { fields, destroy, .. }, .. }) =
                    module.declarations.get(*decl) {
                    pending.extend(fields.iter().map(|field| field.ty));
                    if let Some(destroy) = destroy {
                        add_function(module, *destroy, &mut result);
                    }
                }
            }
            Ok(Type::Variant(decl)) => {
                if let Ok(Declaration { kind: DeclarationKind::Variant { cases, .. }, .. }) =
                    module.declarations.get(*decl) {
                    for case in cases {
                        if let Ok(Declaration { kind: DeclarationKind::Object { fields, .. }, .. }) =
                            module.declarations.get(case.layout) {
                            pending.extend(fields.iter().map(|field| field.ty));
                        }
                    }
                }
            }
            Ok(Type::Array(item) | Type::Builder(item) | Type::Successor(item) | Type::LowCell(item)) => {
                pending.push(*item);
            }
            Ok(Type::Map(key, value) | Type::MapBuilder(key, value)) => pending.extend([*key, *value]),
            Ok(Type::Parameter(parameter)) => {
                used.push(*parameter);
                let actual: Vec<_> = bindings.iter().filter(|(formal, argument)| {
                    formal == parameter && module.types.get(*argument) != Ok(&Type::Parameter(*parameter))
                }).map(|(_, argument)| *argument).collect();
                if !actual.is_empty() {
                    pending.extend(actual);
                    continue;
                }
                let owner = match state.location {
                    super::EntityLocation::Instruction { function, .. }
                    | super::EntityLocation::Terminator { function, .. }
                    | super::EntityLocation::Value { function, .. } => Some(GenericOwner::Function(function)),
                    _ => None,
                };
                let dictionary = body.descriptor_bindings.iter().find(|binding| {
                    Some(parameter.owner) == owner &&
                    matches!(binding.contract.retention, SummarySelection::Descriptor(index)
                        if u32::from(index) == parameter.index)
                });
                if let Some(binding) = dictionary {
                    result.suspends |= binding.contract.async_destroy;
                    add_function(module, binding.contract.cleanup, &mut result);
                } else { unknown(&mut result); }
            }
            Ok(Type::Future(_) | Type::Frame(_) | Type::Any | Type::Role { .. } | Type::Cap { .. }
                | Type::Fn(_)) => unknown(&mut result),
            _ => {}
        }
    }
    result
}
fn add_function(module: &Module, function: FuncId, result: &mut Effects) {
    let Ok(function) = module.functions.get(function) else {
        unknown(result);
        return;
    };
    result.suspends |= function.attrs.contains(&FunctionAttr::Async);
    if let Some(summary) = function.summary.and_then(|id| module.summaries.get(id).ok()) {
        result.suspends |= summary.contract.suspends || summary.contract.async_destroy;
        for &kind in &summary.contract.failures {
            if kind != FailureKind::Cancelled && !result.failures.contains(&kind) {
                result.failures.push(kind);
            }
        }
    } else { unknown(result); }
}
fn unknown(result: &mut Effects) {
    result.suspends = true;
    for kind in [FailureKind::Thrown, FailureKind::Program, FailureKind::Panic, FailureKind::Native] {
        if !result.failures.contains(&kind) {
            result.failures.push(kind);
        }
    }
}
