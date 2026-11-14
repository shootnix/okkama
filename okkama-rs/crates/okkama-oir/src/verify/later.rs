use super::{EntityLocation as L, RuleId as R, State};
use crate::*;
use std::collections::BTreeSet;

pub(super) fn async_cleanup_and_profiles(module: &Module, selected: Option<FuncId>, state: &mut State<'_>) {
    protocols(module, state);
    for (function, declaration) in module.functions.iter() {
        if selected.is_some_and(|id| id != function) {
            continue;
        }
        let Some(body) = &declaration.body else {
            continue;
        };
        if !super::graph::eligible(function, body, state) {
            continue;
        }
        for (value, data) in body.values.iter() {
            if module.types.get(data.ty) == Ok(&Type::Service(ServiceType::CarrierState)) {
                state.at(L::Value { function, value }, None);
                state.error(R::B75, "owned carrier-state values are excluded from 0.6.0");
            }
        }
        for (block_id, block) in body.blocks.iter() {
            for &instruction in &block.insts {
                let Ok(inst) = body.insts.get(instruction) else {
                    continue;
                };
                state.at(L::Instruction { function, instruction }, inst.loc);
                operation(module, declaration, body, &inst.op, state);
            }
            state.at(L::Terminator { function, block: block_id }, block.term.loc);
            if declaration.attrs.iter().any(|attr| matches!(attr, FunctionAttr::DeferEntry { .. }))
                && matches!(block.term.kind, TermKind::Ret { .. }) {
                state.error(R::B101, "defer entry must return a checked Continue/Replace tag");
            }
            if let TermKind::InvokeOwn { op, .. } = &block.term.kind {
                operation(module, declaration, body, op, state);
            }
        }
    }
}

fn protocols(module: &Module, state: &mut State<'_>) {
    for (id, protocol) in module.protocols.iter() {
        state.at(L::Protocol(id), None);
        if !state.step(protocol.rules.len() + protocol.holders.len() + 1) {
            return;
        }
        if protocol.kind == ProtocolKind::Group {
            let holders = [HolderKind::Group, HolderKind::Frame, HolderKind::Cache, HolderKind::Pin];
            let transitions = [ProtocolTransition::Publish, ProtocolTransition::Settle, ProtocolTransition::Cancel,
                ProtocolTransition::Save, ProtocolTransition::Resume];
            if holders.iter().any(|holder| !protocol.holders.contains(holder))
                || transitions.iter().any(|transition| !protocol.rules.iter().any(|rule| {
                    rule.transition == *transition && rule.unwind.is_some() && rule.cancel.is_some()
                })) || !protocol.roots.contains(&RootKind::Cache) || !protocol.roots.contains(&RootKind::Pins) {
                state.error(R::B96,
                    "group/collector protocol needs unique holders, retention, settlement and cancellation");
            }
            if !state.stats.structural_checks.contains(&super::StructuralCheck::B96CollectorPositions) {
                state.stats.structural_checks.push(super::StructuralCheck::B96CollectorPositions);
            }
        }
        if protocol.holders.contains(&HolderKind::Carrier) {
            state.error(R::B75, "owned carrier graphs are excluded from 0.6.0");
        }
        for rule in &protocol.rules {
            if !protocol.holders.contains(&rule.from) || !protocol.holders.contains(&rule.to)
                || !protocol.holders.contains(&rule.normal.holder) {
                state.error(R::B71, "protocol transition refers to an undeclared holder");
            }
            for required in [ProtocolCheck::Owner, ProtocolCheck::LiveDependencies] {
                if !rule.checks.contains(&required) {
                    state.error(R::B71, "protocol transition needs owner and live-dependency checks");
                }
            }
            if matches!(protocol.kind, ProtocolKind::Atomic | ProtocolKind::Cell)
                && (rule.from != rule.to || rule.transition == ProtocolTransition::Attach) {
                state.error(R::B73, "Atomic/Cell publication borrows and cannot acquire a foreign packet");
            }
            if protocol.kind == ProtocolKind::Channel && rule.transition == ProtocolTransition::Publish
                && (!protocol.holders.contains(&HolderKind::Receipt) || rule.unwind.is_none()
                    || rule.cancel.is_none() || !protocol.roots.contains(&RootKind::Pins)) {
                state.error(R::B73, "channel handoff needs receipt, failure/cancel poststates and rooted pins");
            }
        }
    }
}

fn operation(module: &Module, function: &Function, body: &Body, op: &Operation, state: &mut State<'_>) {
    if matches!(op, Operation::Carrier(_)) {
        state.error(R::B75, "owned carrier graph operations are excluded from 0.6.0");
    }
    if let Operation::Async(AsyncOp::WorkerSpawn { args, .. } | AsyncOp::ProcessSpawn { args, .. }) = op {
        for &value in args {
            if let Some(ty) = body.values.get(value).ok().map(|value| value.ty)
                && !transferable(module, ty, state) {
                state.error(R::B113, "worker serialization cannot transport object, Future, closure or native handle");
            }
        }
    }
    let worker_inputs: &[ValueId] = match op {
        Operation::Async(AsyncOp::ProcessSpawn { constructor_args, .. }) => constructor_args,
        Operation::Async(AsyncOp::WorkerBind { fields, .. }) => fields,
        _ => &[],
    };
    for &value in worker_inputs {
        if let Some(ty) = body.values.get(value).ok().map(|value| value.ty)
            && !transferable(module, ty, state) {
            state.error(R::B113, "worker constructor/capture contains a nonserializable value");
        }
    }
    if let Operation::Async(AsyncOp::WorkerSpawn { result: Some(ty), .. }
        | AsyncOp::ProcessSpawn { result: Some(ty), .. }) = op
        && !transferable(module, *ty, state) {
        state.error(R::B113, "worker result is not serializable across the worker boundary");
    }
    if let Operation::Call(CallOp::Native { call, function: target }) = op {
        for argument in &call.args {
            if let Some(ty) = body.values.get(argument.value).ok().map(|value| value.ty)
            && !native_type(module, ty, state) {
                state.error(R::B113, "native ABI cannot accept language objects, closures, roles or Futures");
            }
        }
        if let Ok(target) = module.functions.get(*target) {
            for &ty in &target.signature.results {
                if !native_type(module, ty, state) {
                    state.error(R::B113, "native ABI cannot return a language ownership graph");
                }
            }
        }
    }
    if module.profile == Profile::High {
        return;
    }
    let mut structural = Vec::new();
    if matches!(op, Operation::Async(AsyncOp::FrameAlloc { .. } | AsyncOp::FrameSave { .. }
        | AsyncOp::FrameResume { .. } | AsyncOp::FrameCancel { .. })) {
        structural.push(super::StructuralCheck::B95FrameLayoutAndContinuations);
    }
    if function.attrs.contains(&FunctionAttr::Async)
        && ContractRegistry::new(module).lookup(op).is_ok_and(|contract| contract.effects.suspends)
        && matches!(op, Operation::Cleanup(_) | Operation::Async(_)) {
        structural.push(super::StructuralCheck::B105SuspensionRootMaps);
    }
    for check in structural {
        if !state.stats.structural_checks.contains(&check) {
            state.stats.structural_checks.push(check);
        }
    }
    state.stats.structural_checks.sort();
    if let Some((_, _, Some(target))) = super::summaries::call(op)
    && module.functions.get(target).is_ok_and(|function| {
        function.attrs.iter().any(|attr| matches!(attr, FunctionAttr::Destroy(_)))
    }) {
        state.error(R::B103, "destroy entry is entered only through the cleanup claim/invoke/finish step");
    }
    if let Operation::Cleanup(CleanupOp::DestroyDirect { destroy, identity, .. }) = op {
        let declared = match value_type(module, body, *identity) {
            Some(Type::Object { decl, .. }) => module.declarations.get(*decl).ok().and_then(|declaration| {
                if let DeclarationKind::Object { destroy, .. } = declaration.kind { destroy } else { None }
            }),
            _ => None,
        };
        if declared != Some(*destroy) {
            state.error(R::B103, "direct destroy must call the selected identity's declared destroy entry");
        }
    }
    if let Operation::Ownership(OwnershipOp::Dynamic { reason, protocol, target, state: owner, .. }) = op {
        let site = match state.location {
            L::Instruction { function, instruction } => super::OperationSite::Instruction { function, instruction },
            L::Terminator { function, block } => super::OperationSite::Terminator { function, block },
            _ => return,
        };
        if !state.memory(512) {
            return;
        }
        state.stats.dynamic.census.add(match site {
            super::OperationSite::Instruction { function, .. } | super::OperationSite::Terminator { function, .. } => {
                function
            }
        }, *reason, 1);
        state.stats.dynamic.sites.push(super::DynamicSite {
            site, target: *target, reason: *reason, protocol: *protocol,
        });
        if let Ok(declaration) = module.protocols.get(*protocol) {
            let expected = match reason {
                DynamicReason::Atomic => ProtocolKind::Atomic,
                DynamicReason::Cell => ProtocolKind::Cell,
                DynamicReason::Channel => ProtocolKind::Channel,
                DynamicReason::Group => ProtocolKind::Group,
                DynamicReason::Any => ProtocolKind::Any,
                DynamicReason::Closure => ProtocolKind::Closure,
                DynamicReason::Merge => ProtocolKind::Storage,
            };
            let rule = declaration.rules.iter().find(|rule| rule.transition == ProtocolTransition::Dynamic);
            if declaration.kind != expected || rule.is_none_or(|rule| {
                !rule.checks.contains(&ProtocolCheck::Snapshot)
                    || !rule.checks.contains(&ProtocolCheck::Transferable)
            }) {
                state.error(R::B71, "dynamic reason requires its protocol family and checked dynamic transition");
            }
        }
        if module.profile == Profile::Low
            && let Ok(declaration) = module.protocols.get(*protocol) {
            let dynamic: Vec<_> = declaration.rules.iter().filter(|rule| {
                rule.transition == ProtocolTransition::Dynamic
            }).collect();
            let supported = *reason == DynamicReason::Merge && declaration.kind == ProtocolKind::Storage
                && declaration.holders.contains(&HolderKind::Scope)
                && declaration.holders.contains(&HolderKind::Packet)
                && declaration.holders.iter().all(|holder| matches!(holder, HolderKind::Scope | HolderKind::Packet))
                && declaration.roots.contains(&RootKind::Operands)
                && dynamic.len() == 1 && dynamic[0].from == HolderKind::Packet
                && dynamic[0].to == HolderKind::Scope
                && dynamic[0].normal == ProtocolPoststate { holder: HolderKind::Scope, state: HolderState::Open }
                && dynamic[0].unwind == Some(ProtocolPoststate {
                    holder: HolderKind::Packet, state: HolderState::Unaccepted,
                }) && dynamic[0].cancel.is_none();
            if !supported {
                state.error(R::B71, "Low dynamic source requires the synchronous storage merge transition");
            }
        }
        if value_type(module, body, *owner) != Some(&Type::Service(ServiceType::Scope(*target))) {
            state.error(R::B71, "dynamic target must agree with the consumed owner state");
        }
    }
    if let Some((call, _, _)) = super::summaries::call(op)
        && call.cleanup == CallCleanupMode::Shielded
        && !function.attrs.iter().any(|attr| matches!(attr,
            FunctionAttr::Cleanup | FunctionAttr::Destroy(_) | FunctionAttr::DeferEntry { .. })) {
        state.error(R::B105, "shielded calls are restricted to cleanup execution");
    }
    if let Operation::Async(AsyncOp::AwaitShielded { .. }) = op
        && !function.attrs.iter().any(|attr| matches!(attr,
            FunctionAttr::Cleanup | FunctionAttr::Destroy(_) | FunctionAttr::DeferEntry { .. })) {
        state.error(R::B105, "shielded await is restricted to cleanup execution");
    }
    if module.profile == Profile::Low && matches!(op,
        Operation::Lexical(_)
        | Operation::Ownership(OwnershipOp::ScopeEnd { .. }
            | OwnershipOp::ScopeUnwind { .. } | OwnershipOp::End { .. })
        | Operation::Async(AsyncOp::Await { .. } | AsyncOp::AwaitTake { .. }
            | AsyncOp::AwaitShielded { .. })) {
        state.error(R::B114, "low profile contains an unlowered lexical close or suspension form");
    }
    if module.profile == Profile::Own && matches!(op,
        Operation::Cleanup(_) | Operation::Async(AsyncOp::FrameSave { .. } | AsyncOp::FrameResume { .. }
            | AsyncOp::FrameLoad { .. } | AsyncOp::FrameStore { .. } | AsyncOp::FrameCancel { .. })) {
        state.error(R::B114, "coroutine/cleanup implementation forms require coro or low");
    }

}

pub(super) fn value_type<'a>(module: &'a Module, body: &Body, value: ValueId) -> Option<&'a Type> {
    body.values.get(value).ok().and_then(|value| module.types.get(value.ty).ok())
}

fn transferable(module: &Module, ty: TypeId, state: &mut State<'_>) -> bool {
    let mut pending = vec![ty];
    let mut seen = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        if !state.step(1) {
            return false;
        }
        if !seen.insert(ty) {
            continue;
        }
        match module.types.get(ty) {
            Ok(Type::Int | Type::Bool | Type::Byte | Type::Float | Type::Rat | Type::Char | Type::Str
                | Type::Range | Type::Enum(_)) => {}
            Ok(Type::Array(item)) => pending.push(*item),
            Ok(Type::Map(key, value)) => pending.extend([*key, *value]),
            Ok(Type::Variant(id)) => {
                if let Ok(Declaration { kind: DeclarationKind::Variant { cases, .. }, .. }) =
                    module.declarations.get(*id) {
                    for case in cases {
                        if let Ok(Declaration { kind: DeclarationKind::Object { fields, .. }, .. }) =
                            module.declarations.get(case.layout) {
                            pending.extend(fields.iter().map(|field| field.ty));
                        }
                    }
                } else { return false; }
            }
            _ => return false,
        }
    }
    true
}

fn native_type(module: &Module, ty: TypeId, state: &mut State<'_>) -> bool {
    let mut pending = vec![ty];
    let mut seen = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        if !state.step(1) {
            return false;
        }
        if !seen.insert(ty) {
            continue;
        }
        match module.types.get(ty) {
            Ok(Type::Object { decl, .. }) => {
                if !module.declarations.get(*decl).is_ok_and(|declaration| {
                    matches!(declaration.kind, DeclarationKind::Object { builtin: true, .. })
                }) { return false; }
            }
            Ok(Type::Future(_) | Type::Fn(_) | Type::Role { .. } | Type::Cap { .. }
                | Type::Parameter(_) | Type::Any | Type::Service(_) | Type::Frame(_)) => return false,
            Ok(Type::Array(ty)) => pending.push(*ty),
            Ok(Type::Map(key, value)) => pending.extend([*key, *value]),
            Ok(Type::Variant(id)) => {
                if let Ok(Declaration { kind: DeclarationKind::Variant { cases, .. }, .. }) =
                    module.declarations.get(*id) {
                    for case in cases {
                        if let Ok(Declaration { kind: DeclarationKind::Object { fields, .. }, .. }) =
                            module.declarations.get(case.layout) {
                            pending.extend(fields.iter().map(|field| field.ty));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    true
}
