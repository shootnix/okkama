use super::*;
use super::summaries::Universe;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ElisionBlocker {
    AcceptedInput,
    Obligation,
    Journal,
    Staging,
    Defer,
    ServiceDuty,
    Reserve,
    ExplicitTransport,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScopeFacts {
    /// Includes journal history, staging and replacement/reserve parts.
    pub potential_owned_types: BTreeSet<TypeId>,
    pub defers: BTreeSet<FuncId>,
    pub service_protocols: BTreeSet<ProtocolId>,
    pub blockers: BTreeSet<ElisionBlocker>,
    pub effects: ScopeEffectEnvelope,
    /// Eligibility for the later rewrite; commit 1 erases no scopes.
    pub empty_scope_eligible: bool,
}

fn add_type(
    scopes: &mut BTreeMap<ScopeId, ScopeFacts>,
    scope: ScopeId,
    ty: TypeId,
    types: &BTreeMap<TypeId, TypeClassification>,
    blocker: ElisionBlocker,
) {
    if types.get(&ty).is_some_and(TypeClassification::may_own) {
        let facts = scopes.get_mut(&scope).unwrap();
        facts.potential_owned_types.insert(ty);
        facts.blockers.insert(blocker);
    }
}

pub(super) fn inventory(
    module: &Module,
    types: &BTreeMap<TypeId, TypeClassification>,
    external: &BTreeMap<FuncId, Summary>,
    budget: &mut Budget<'_>,
) -> Result<BTreeMap<ScopeId, ScopeFacts>, OwnError> {
    budget.memory(module.scopes.len() * 512)?;
    let mut scopes: BTreeMap<_, _> = module.scopes.iter().map(|(id, _)| (id, ScopeFacts::default())).collect();
    for (_, function) in module.functions.iter() {
        let Some(body) = &function.body else {
            continue;
        };
        let offered = inventory::may_offer(module, function, body, types, external, budget)?;
        let entry = body.blocks.get(body.entry).map_err(arena)?;
        let destroy = function.attrs.iter().any(|attr| matches!(attr, FunctionAttr::Destroy(_)));
        let mut language_index = 0;
        let internal = function.summary.and_then(|id| module.summaries.get(id).ok())
            .is_some_and(|summary| summary.entry_kind == EntryKind::Internal);
        if function.signature.ownership_abi != OwnershipAbi::Observation && !internal {
            for value in &entry.params {
                budget.work(1)?;
                let ty = body.values.get(*value).map_err(arena)?.ty;
                if language(module, &[ty]).is_empty() {
                    continue;
                }
                // self@destroy is a borrow, not a newly accepted obligation.
                if !destroy || language_index != 0 {
                    add_type(&mut scopes, entry.lexical_scope, ty, types, ElisionBlocker::AcceptedInput);
                }
                language_index += 1;
            }
        }
        for (_, block) in body.blocks.iter() {
            budget.work(1)?;
            let mut ops = vec![];
            for instruction in &block.insts {
                let inst = body.insts.get(*instruction).map_err(arena)?;
                ops.push((&inst.op, inst.results.clone()));
            }
            if let TermKind::InvokeOwn { op, normal, .. } = &block.term.kind {
                let results = body.blocks.get(normal.block).map_err(arena)?.params.iter().zip(&normal.args)
                    .filter_map(|(parameter, argument)| matches!(argument, EdgeArg::Produced(_)).then_some(*parameter))
                    .collect();
                ops.push((op, results));
            }
            budget.memory(ops.len() * 64)?;
            for (op, results) in ops {
                budget.work(1)?;
                let can_own = matches!(op, Operation::Core(CoreOp::ObjNew { .. } | CoreOp::ObjNext { .. }
                    | CoreOp::VariantNew { .. } | CoreOp::CapMake { .. } | CoreOp::FnClosure { .. }
                    | CoreOp::FnBind { .. } | CoreOp::FnBindRole { .. } | CoreOp::AnyBox { .. }
                    | CoreOp::FakeNew { .. }) | Operation::Collection(_) | Operation::Call(_)
                    | Operation::Async(_));
                if can_own {
                    let observation = summaries::call(op).is_some_and(|(call, _)| {
                        call.summary.and_then(|id| module.summaries.get(id).ok()).is_some_and(|summary| {
                            summary.entry_kind == EntryKind::ObservationAdmissionStaysCaller
                        })
                    });
                    if !observation {
                        for value in results {
                            let ty = body.values.get(value).map_err(arena)?.ty;
                            if offered[value.as_u32() as usize] && !language(module, &[ty]).is_empty() {
                                add_type(&mut scopes, block.lexical_scope, ty, types, ElisionBlocker::Obligation);
                            }
                        }
                    }
                }
                match op {
                    Operation::Core(CoreOp::ObjInit { object, field, .. }) => {
                        if module.declarations.get(field.declaration).is_ok_and(|declaration| {
                            matches!(&declaration.kind, DeclarationKind::Object { fields, .. }
                                if fields.get(field.index as usize).is_some_and(|field| field.name == "$pending"))
                        }) {
                            let ty = body.values.get(*object).map_err(arena)?.ty;
                            add_type(&mut scopes, block.lexical_scope, ty, types, ElisionBlocker::Obligation);
                        }
                    }
                    Operation::Lexical(LexicalOp::Bind { binding, value }) => {
                        let scope = body.bindings.get(*binding).map_err(arena)?.scope;
                        let ty = body.values.get(*value).map_err(arena)?.ty;
                        if offered[value.as_u32() as usize] {
                            add_type(&mut scopes, scope, ty, types, ElisionBlocker::Obligation);
                        }
                    }
                    Operation::Lexical(LexicalOp::Rebind { target, value, .. }) => {
                        let ty = body.values.get(*value).map_err(arena)?.ty;
                        if offered[value.as_u32() as usize] {
                            add_type(&mut scopes, *target, ty, types, ElisionBlocker::Journal);
                        }
                    }
                    Operation::Lexical(LexicalOp::Stage { action, value, .. }) => {
                        let ty = body.values.get(*value).map_err(arena)?.ty;
                        if offered[value.as_u32() as usize] {
                            add_type(&mut scopes, *action, ty, types, ElisionBlocker::Staging);
                        }
                    }
                    Operation::Lexical(LexicalOp::Temp { scope, value }) => {
                        let ty = body.values.get(*value).map_err(arena)?.ty;
                        if offered[value.as_u32() as usize] {
                            add_type(&mut scopes, *scope, ty, types, ElisionBlocker::Obligation);
                        }
                    }
                    Operation::Lexical(LexicalOp::DeferRegister { scope, action }) => {
                        let facts = scopes.get_mut(scope).unwrap();
                        facts.defers.insert(action.function);
                        facts.blockers.insert(ElisionBlocker::Defer);
                    }
                    Operation::Ownership(OwnershipOp::Bind { binding }
                        | OwnershipOp::Rebind { binding }) => {
                        let ty = body.values.get(binding.resource.value).map_err(arena)?.ty;
                        add_type(&mut scopes, binding.original_scope, ty, types, ElisionBlocker::Journal);
                    }
                    Operation::Ownership(OwnershipOp::Stage { stage }) => {
                        let ty = body.values.get(stage.resource.value).map_err(arena)?.ty;
                        add_type(&mut scopes, stage.action, ty, types, ElisionBlocker::Staging);
                    }
                    Operation::Ownership(OwnershipOp::ScopeEnd { state, reserve: Some(_), .. }
                        | OwnershipOp::ScopeUnwind { state, reserve: Some(_), .. }) => {
                        let scope = state_scope(module, body, *state).unwrap_or(block.lexical_scope);
                        scopes.get_mut(&scope).unwrap().blockers.insert(ElisionBlocker::Reserve);
                    }
                    Operation::Ownership(OwnershipOp::Admit { state, .. } | OwnershipOp::Accept { state, .. }) => {
                        let scope = state_scope(module, body, *state).unwrap_or(block.lexical_scope);
                        scopes.get_mut(&scope).unwrap().blockers.insert(ElisionBlocker::ExplicitTransport);
                    }
                    Operation::Ownership(OwnershipOp::Dynamic { target, .. }) => {
                        scopes.get_mut(target).unwrap().blockers.insert(ElisionBlocker::ExplicitTransport);
                    }
                    Operation::Ownership(OwnershipOp::ScopeTransfer { states, .. }) => {
                        for state in states.values() {
                            let scope = state_scope(module, body, state).unwrap_or(block.lexical_scope);
                            scopes.get_mut(&scope).unwrap().blockers.insert(ElisionBlocker::ExplicitTransport);
                        }
                    }
                    Operation::Ownership(OwnershipOp::End { boundary, .. }) => {
                        let scope = body.boundaries.get(*boundary).map_err(arena)?.scope;
                        scopes.get_mut(&scope).unwrap().blockers.insert(ElisionBlocker::ExplicitTransport);
                    }
                    Operation::Exit(ExitOp::Extend { .. }) => {
                        scopes.get_mut(&block.lexical_scope).unwrap().blockers.insert(ElisionBlocker::Reserve);
                    }
                    Operation::Async(_) | Operation::Carrier(_) => {
                        scopes.get_mut(&block.lexical_scope).unwrap().blockers.insert(ElisionBlocker::ServiceDuty);
                    }
                    Operation::Exit(ExitOp::Prepare { exit } | ExitOp::Replace { replacement: exit, .. }) => {
                        if let Some(value) = exit.value.filter(|value| offered[value.as_u32() as usize]) {
                            let ty = body.values.get(value).map_err(arena)?.ty;
                            let boundary = body.boundaries.get(exit.boundary).map_err(arena)?.scope;
                            reserve(module, &mut scopes, block.lexical_scope, boundary, ty, types, budget)?;
                        }
                        if exit.packet.is_some() {
                            scopes.get_mut(&block.lexical_scope).unwrap().blockers.insert(ElisionBlocker::Reserve);
                        }
                    }
                    _ => {}
                }
            }
        }
        for result in &body.high_results {
            if result.demand == ResultDemand::Want {
                if !offered[result.value.as_u32() as usize] {
                    continue;
                }
                let ty = body.values.get(result.value).map_err(arena)?.ty;
                let scope = body.blocks.get(result.block).map_err(arena)?.lexical_scope;
                if let Some(instruction) = result.prepared {
                    if let Operation::Exit(ExitOp::Prepare { exit }) = &body.insts.get(instruction).map_err(arena)?.op {
                        let boundary = body.boundaries.get(exit.boundary).map_err(arena)?.scope;
                        reserve(module, &mut scopes, scope, boundary, ty, types, budget)?;
                    }
                } else if types[&ty].may_own() {
                    scopes.get_mut(&scope).unwrap().blockers.insert(ElisionBlocker::Reserve);
                }
            }
        }
        for schedule in &body.service_schedules {
            let facts = scopes.get_mut(&schedule.owner).unwrap();
            facts.blockers.insert(ElisionBlocker::ServiceDuty);
            facts.service_protocols.insert(schedule.protocol);
        }
    }
    // Until commit 2 partitions packet equations, explicit packet/reserve
    // transport uses the complete function inventory. This includes obligations
    // no longer reachable from a current SSA reference and closed-inner reserve.
    let mut function_types: BTreeMap<FuncId, BTreeSet<TypeId>> = BTreeMap::new();
    for (scope, facts) in &scopes {
        let function = module.scopes.get(*scope).map_err(arena)?.function;
        function_types.entry(function).or_default().extend(&facts.potential_owned_types);
    }
    for (scope, facts) in &mut scopes {
        if facts.blockers.contains(&ElisionBlocker::ExplicitTransport)
            || facts.blockers.contains(&ElisionBlocker::Reserve) {
            let function = module.scopes.get(*scope).map_err(arena)?.function;
            let additions = &function_types[&function];
            budget.work(additions.len())?;
            budget.memory(additions.len() * 64)?;
            facts.potential_owned_types.extend(additions);
        }
    }
    Ok(scopes)
}

fn state_scope(module: &Module, body: &Body, state: ValueId) -> Option<ScopeId> {
    let value = body.values.get(state).ok()?;
    match module.types.get(value.ty).ok()? {
        Type::Service(ServiceType::Scope(scope)) => Some(*scope),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn reserve(
    module: &Module,
    scopes: &mut BTreeMap<ScopeId, ScopeFacts>,
    mut scope: ScopeId,
    boundary: ScopeId,
    ty: TypeId,
    types: &BTreeMap<TypeId, TypeClassification>,
    budget: &mut Budget<'_>,
) -> Result<(), OwnError> {
    if !types[&ty].may_own() {
        return Ok(());
    }
    loop {
        budget.work(1)?;
        add_type(scopes, scope, ty, types, ElisionBlocker::Reserve);
        if scope == boundary {
            break;
        }
        let Some(parent) = module.scopes.get(scope).map_err(arena)?.parent else {
            return Err(budget.fail(OwnDiagnosticKind::InvalidEquation, "exit boundary is not an ancestor"));
        };
        scope = parent;
    }
    Ok(())
}

pub(super) fn analyze(
    module: &Module,
    types: &BTreeMap<TypeId, TypeClassification>,
    universe: &Universe,
    budget: &mut Budget<'_>,
) -> Result<OwnAnalysis, OwnError> {
    let mut result = OwnAnalysis::default();
    for (ty, facts) in types {
        if facts.unresolved.is_empty() && facts.service_types.iter().all(|ty| universe.services.contains_key(ty)) {
            result.cleanup_effects.insert(*ty, universe.cleanup(module, facts));
        }
    }
    for (scope, facts) in &universe.inventories {
        budget.work(1)?;
        let mut facts = facts.clone();
        for ty in &facts.potential_owned_types {
            let effects = &result.cleanup_effects[ty];
            normal::failures(&mut facts.effects.failures, &effects.failures);
            facts.effects.suspends |= effects.suspends;
        }
        for defer in &facts.defers {
            summaries::project(&mut facts.effects, universe.summary(*defer));
        }
        for protocol in &facts.service_protocols {
            summaries::project(&mut facts.effects, universe.protocol_summary(*protocol));
        }
        if facts.blockers.contains(&ElisionBlocker::ServiceDuty) && facts.service_protocols.is_empty()
            && facts.potential_owned_types.iter().all(|ty| types[ty].service_types.is_empty()) {
            return Err(budget.fail(OwnDiagnosticKind::MissingBoundary, format!(
                "scope {scope:?} has a service duty without a registered settlement descriptor",
            )));
        }
        if facts.effects.suspends {
            let function = module.scopes.get(*scope).map_err(arena)?.function;
            return Err(budget.fail(OwnDiagnosticKind::UnsupportedAsync, format!(
                "scope {scope:?} in {function:?} may own suspending cleanup",
            )));
        }
        facts.empty_scope_eligible = facts.blockers.is_empty() && facts.potential_owned_types.is_empty();
        budget.stats.empty_scope_candidates += usize::from(facts.empty_scope_eligible);
        result.scopes.insert(*scope, facts);
    }
    Ok(result)
}
