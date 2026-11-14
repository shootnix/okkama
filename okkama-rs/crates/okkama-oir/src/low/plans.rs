//! Conservative finite inventories. Cyclic state equations and ambiguous joins stay with the executor.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) type FreshSummaries = BTreeMap<FuncId, SummaryId>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Seed {
    identity: ValueId,
    origin: CreationWitness,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Inventory {
    entries: Vec<Seed>,
    preserves: Vec<PreserveWitness>,
    joins: Vec<JoinWitness>,
}
struct Recipe {
    inputs: Vec<ValueId>,
    seed: Option<Seed>,
    alternatives: bool,
    partition: Option<(SelectionId, bool)>,
}
fn incoming(body: &Body, block: BlockId, index: usize) -> Vec<(BlockId, EdgeArg)> {
    let mut args = vec![];
    for (id, data) in body.blocks.iter() {
        let mut edge = |target: BlockId, values: Vec<EdgeArg>| {
            if target == block && let Some(value) = values.get(index) {
                args.push((id, *value));
            }
        };
        match &data.term.kind {
            TermKind::Br(target) => edge(target.block, target.args.iter().copied().map(EdgeArg::Value).collect()),
            TermKind::Cond { yes, no, .. } => {
                for target in [yes, no] {
                    edge(target.block, target.args.iter().copied().map(EdgeArg::Value).collect());
                }
            }
            TermKind::Switch { cases, default, .. } => {
                for target in cases.iter().map(|case| &case.target).chain([default]) {
                    edge(target.block, target.args.iter().copied().map(EdgeArg::Value).collect());
                }
            }
            TermKind::InvokeOwn { normal, unwind, .. } => {
                edge(normal.block, normal.args.clone());
                edge(unwind.block, unwind.args.clone());
            }
            _ => {
                // A non-supported predecessor invalidates finite inventory inference.
                if graph::successors(body, id).is_ok_and(|targets| targets.contains(&block)) {
                    return vec![];
                }
            }
        }
        if data.unwind.as_ref().is_some_and(|unwind| unwind.target.block == block) {
            return vec![];
        }
    }
    args
}
fn outputs(body: &Body, site: &HighOperationSite) -> Option<Vec<ValueId>> {
    match site {
        HighOperationSite::Instruction(inst) => Some(body.insts.get(*inst).ok()?.results.clone()),
        HighOperationSite::Terminator(block) => {
            let TermKind::InvokeOwn { normal, .. } = &body.blocks.get(*block).ok()?.term.kind else {
                return None;
            };
            let params = &body.blocks.get(normal.block).ok()?.params;
            let mut results = BTreeMap::new();
            for (arg, param) in normal.args.iter().zip(params) {
                if let EdgeArg::Produced(EdgeResult { outcome: Outcome::Normal, slot }) = arg
                    && results.insert(*slot, *param).is_some() {
                    return None;
                }
            }
            let mut values = vec![];
            for slot in 0..results.len() {
                values.push(*results.get(&(slot as u16))?);
            }
            Some(values)
        }
    }
}
fn operation_recipe(
    body: &Body, op: &Operation, site: HighOperationSite, index: u16, fresh: &FreshSummaries,
) -> Option<Recipe> {
    let mut recipe = Recipe { inputs: vec![], seed: None, alternatives: false, partition: None };
    match op {
        Operation::Ownership(OwnershipOp::Empty {} | OwnershipOp::ScopeBegin { .. }) if index == 0 => {}
        Operation::Ownership(OwnershipOp::Move { packet }) if index == 0 => recipe.inputs.push(*packet),
        Operation::Ownership(OwnershipOp::Join { packets }) if index == 0 => recipe.inputs.clone_from(packets),
        Operation::Ownership(OwnershipOp::Admit { state, packet } | OwnershipOp::Accept { state, packet })
            if index == 0 => recipe.inputs.extend([*state, *packet]),
        Operation::Ownership(OwnershipOp::Bind { binding } | OwnershipOp::Rebind { binding }) if index == 0 => {
            recipe.inputs.extend([binding.state, binding.resource.packet]);
        }
        Operation::Ownership(OwnershipOp::Split { packet, selection }
            | OwnershipOp::SplitChecked { packet, selection }) if index < 2 => {
            recipe.inputs.push(*packet);
            recipe.partition = Some((*selection, index == 1));
        }
        Operation::Ownership(OwnershipOp::ScopeSplit { state, selection }) if index < 2 => {
            recipe.inputs.push(*state);
            recipe.partition = Some((*selection, index == 1));
        }
        Operation::Ownership(OwnershipOp::ScopeTransfer { states: TransferStates::Same(state), .. })
            if index == 0 => recipe.inputs.push(*state),
        Operation::Core(CoreOp::ObjNew { packet, construction: ConstructionStatus::Incomplete, .. })
            if index == 1 => recipe.inputs.extend(packet),
        Operation::Core(CoreOp::ObjNew { packet, construction: ConstructionStatus::Complete, .. })
            if index == 1 => {
            recipe.inputs.extend(packet);
            recipe.seed = Some(Seed {
                identity: *outputs(body, &site)?.first()?, origin: CreationWitness::Create { site },
            });
        }
        Operation::Core(CoreOp::ObjInit { object, completion: Some(completion), .. }) if index == 0 => {
            recipe.inputs.push(completion.state);
            recipe.seed = Some(Seed { identity: *object, origin: CreationWitness::Create { site } });
        }
        Operation::Call(CallOp::Direct { function, call }) if index == 1 => {
            let summary = *fresh.get(function)?;
            if call.summary != Some(summary) {
                return None;
            }
            recipe.seed = Some(Seed {
                identity: *outputs(body, &site)?.first()?,
                origin: CreationWitness::FreshResult { site, callee: *function, summary },
            });
        }
        _ => return None,
    }
    Some(recipe)
}
fn recipe(body: &Body, value: ValueId, fresh: &FreshSummaries) -> Option<Recipe> {
    match body.values.get(value).ok()?.def {
        Definition::InstResult { inst, index } => {
            operation_recipe(body, &body.insts.get(inst).ok()?.op, HighOperationSite::Instruction(inst), index, fresh)
        }
        Definition::BlockParam { block, index } => {
            if block == body.entry {
                return None;
            }
            let args = incoming(body, block, index as usize);
            if args.is_empty() {
                return None;
            }
            let mut inputs = vec![];
            for (source, arg) in args {
                match arg {
                    EdgeArg::Value(value) => inputs.push(value),
                    EdgeArg::Produced(EdgeResult { outcome: Outcome::Normal, slot }) if inputs.is_empty() => {
                        let TermKind::InvokeOwn { op, normal, .. } = &body.blocks.get(source).ok()?.term.kind else {
                            return None;
                        };
                        if normal.block != block || incoming(body, block, index as usize).len() != 1 {
                            return None;
                        }
                        return operation_recipe(body, op, HighOperationSite::Terminator(source), slot, fresh);
                    }
                    _ => return None,
                }
            }
            Some(Recipe { inputs, seed: None, alternatives: true, partition: None })
        }
    }
}
fn inventory(
    body: &Body, root: ValueId, fresh: &FreshSummaries, budget: &mut Budget<'_>,
) -> Result<Option<Inventory>, LowError> {
    let mut pending = vec![(root, false)];
    let mut visiting = BTreeSet::new();
    let mut known = BTreeMap::<ValueId, Inventory>::new();
    while let Some((value, finish)) = pending.pop() {
        budget.step(body.blocks.len() + 1)?;
        if known.contains_key(&value) {
            continue;
        }
        let Some(recipe) = recipe(body, value, fresh) else {
            return Ok(None);
        };
        if !finish {
            if !visiting.insert(value) {
                return Ok(None);
            }
            pending.push((value, true));
            for input in recipe.inputs.iter().rev() {
                if !known.contains_key(input) {
                    pending.push((*input, false));
                }
            }
            continue;
        }
        let mut result = Inventory::default();
        for (index, input) in recipe.inputs.iter().enumerate() {
            let Some(source) = known.get(input) else {
                return Ok(None);
            };
            budget.step(source.entries.len() + source.preserves.len() + 1)?;
            if recipe.alternatives && index != 0 {
                if result.entries != source.entries {
                    return Ok(None);
                }
            } else {
                result.entries.extend(source.entries.clone());
            }
            result.preserves.extend(source.preserves.clone());
            result.joins.extend(source.joins.clone());
            result.preserves.push(PreserveWitness { from: *input, to: value });
        }
        if let Some((selection, selected)) = recipe.partition {
            let SelectionExpr::Value(identity) = body.selections.get(selection)? else {
                return Ok(None);
            };
            if !result.entries.iter().any(|entry| entry.identity == *identity) {
                return Ok(None);
            }
            result.entries.retain(|entry| (entry.identity == *identity) == selected);
        }

        if recipe.alternatives && recipe.inputs.len() > 1 {
            result.joins.push(JoinWitness { state: value, predecessors: recipe.inputs.clone() });
        }
        if let Some(seed) = recipe.seed {
            result.entries.push(seed);
        }
        let mut identities = BTreeSet::new();
        if result.entries.iter().any(|entry| !identities.insert(entry.identity)) {
            return Ok(None);
        }
        visiting.remove(&value);
        known.insert(value, result);
    }
    Ok(known.remove(&root))
}
/// Verify the actual output packet at every return, in addition to the final summary.
/// Native/opaque contracts need an explicit trusted singleton contract in a later integration.
pub(super) fn fresh_summaries(module: &Module, budget: &mut Budget<'_>) -> Result<FreshSummaries, LowError> {
    let mut fresh = BTreeMap::new();
    for (id, function) in module.functions.iter() {
        budget.step(1)?;
        let (Some(body), Some(summary_id)) = (&function.body, function.summary) else {
            continue;
        };
        let summary = module.summaries.get(summary_id)?;
        let [relation] = summary.results.as_slice() else {
            continue;
        };
        if relation.fresh.len() != 1 || !relation.aliases.is_empty()
            || relation.provenance != AdmissionProvenance::UnboundResult {
            continue;
        }
        let mut returns = 0;
        let mut valid = true;
        for (_, block) in body.blocks.iter() {
            let TermKind::Ret { values } = &block.term.kind else {
                continue;
            };
            returns += 1;
            let [identity, packet, _deps] = values.as_slice() else {
                valid = false;
                break;
            };
            let output = inventory(body, *packet, &BTreeMap::new(), budget)?;
            let Some(output) = output else {
                valid = false;
                break;
            };
            if output.entries.len() != 1 || output.entries[0].identity != *identity {
                valid = false;
                break;
            }
        }
        if valid && returns != 0 {
            fresh.insert(id, summary_id);
        }
    }
    Ok(fresh)
}
fn location(body: &Body, site: &HighOperationSite) -> Option<(BlockId, usize)> {
    match site {
        HighOperationSite::Instruction(inst) => body.blocks.iter().find_map(|(block, data)| {
            data.insts.iter().position(|id| id == inst).map(|index| (block, index))
        }),
        HighOperationSite::Terminator(block) => Some((*block, body.blocks.get(*block).ok()?.insts.len())),
    }
}
fn before(
    body: &Body, first: (BlockId, usize), second: (BlockId, usize), budget: &mut Budget<'_>,
) -> Result<bool, LowError> {
    if first.0 == second.0 {
        return Ok(first.1 < second.1);
    }
    let mut pending = vec![body.entry];
    let mut seen = BTreeSet::new();
    let mut reaches_first = false;
    while let Some(block) = pending.pop() {
        budget.step(1)?;
        if !seen.insert(block) {
            continue;
        }
        if block == first.0 {
            reaches_first = true;
            continue;
        }
        if block == second.0 {
            return Ok(false);
        }
        pending.extend(graph::successors(body, block)?);
    }
    Ok(reaches_first)
}
fn origin_site(origin: &CreationWitness) -> &HighOperationSite {
    match origin {
        CreationWitness::Singleton => unreachable!("producer always records completions"),
        CreationWitness::Create { site } | CreationWitness::FreshResult { site, .. } => site,
    }
}
fn pending_empty(body: &Body, value: ValueId, budget: &mut Budget<'_>) -> Result<bool, LowError> {
    let Definition::InstResult { inst, .. } = body.values.get(value)?.def else {
        return Ok(false);
    };
    let Operation::Exit(ExitOp::Prepare { exit }) = &body.insts.get(inst)?.op else {
        return Ok(false);
    };
    let Some(packet) = exit.packet else {
        return Ok(true);
    };
    Ok(inventory(body, packet, &BTreeMap::new(), budget)?.is_some_and(|value| value.entries.is_empty()))
}
pub(super) fn prove(
    module: &Module, body: &Body, state: ValueId, pending: Option<ValueId>, close: &CleanupClose,
    fresh: &FreshSummaries, budget: &mut Budget<'_>,
) -> Result<Option<CleanupOrderProof>, LowError> {
    let reserve_empty = match pending {
        None => true,
        Some(value) => pending_empty(body, value, budget)?,
    };
    if (!close.effects.failures.is_empty() || close.active_failure) && !reserve_empty {
        return Ok(None);
    }
    let Some(mut inventory) = inventory(body, state, fresh, budget)? else {
        return Ok(None);
    };
    if inventory.entries.is_empty() {
        return Ok(None);
    }
    // Insertion order is not serial evidence: establish every adjacent relation from the CFG.
    for index in 1..inventory.entries.len() {
        let mut current = index;
        while current > 0 {
            let Some(a) = location(body, origin_site(&inventory.entries[current - 1].origin)) else {
                return Ok(None);
            };
            let Some(b) = location(body, origin_site(&inventory.entries[current].origin)) else {
                return Ok(None);
            };
            if before(body, a, b, budget)? {
                break;
            }
            if !before(body, b, a, budget)? {
                return Ok(None);
            }
            inventory.entries.swap(current - 1, current);
            current -= 1;
        }
    }
    let at = (close.after_defers, body.blocks.get(close.after_defers)?.insts.len() + 1);
    let mut slots = vec![];
    for seed in inventory.entries.into_iter().rev() {
        let Some(created) = location(body, origin_site(&seed.origin)) else {
            return Ok(None);
        };
        if !before(body, created, at, budget)? {
            return Ok(None);
        }
        let mut target = None;
        for (id, inst) in body.insts.iter() {
            budget.step(1)?;
            let Operation::Core(CoreOp::TypeOf { ty, descriptor }) = &inst.op else {
                continue;
            };
            if *ty != body.values.get(seed.identity)?.ty || descriptor.managed != DescriptorManaged::Managed
                || descriptor.async_destroy {
                continue;
            }
            let Some(position) = location(body, &HighOperationSite::Instruction(id)) else {
                continue;
            };
            let contract = &module.summaries.get(descriptor.summary)?.contract;
            if contract.suspends || contract.async_destroy
                || contract.failures.iter().any(|kind| !close.effects.failures.contains(kind)) {
                continue;
            }
            if before(body, position, at, budget)? {
                target = inst.results.first().map(|value| (*value, descriptor.cleanup));
                break;
            }
        }
        let Some((descriptor, destroy)) = target else {
            return Ok(None);
        };
        slots.push(DirectSlot {
            identity: seed.identity, descriptor, destroy, origin: seed.origin, active_at: close.after_defers,
        });
    }
    let mut suffixes = vec![];
    for completed in 1..=slots.len() {
        budget.step(slots.len() - completed + 1)?;
        let remaining = slots[completed..].iter().map(|slot| slot.identity).collect();
        let roots = slots[completed..].iter().flat_map(|slot| [slot.identity, slot.descriptor]).collect();
        suffixes.push(CleanupSuffix { completed: completed as u32, remaining, roots });
    }
    Ok(Some(CleanupOrderProof {
        state, slots, preserves: inventory.preserves, joins: inventory.joins, suffixes, reserve_empty,
    }))
}

/// Producer consistency check. Does not replace the independent semantic verifier (#564).
/// Reconstructs evidence from operations and successful edges, without trusting the supplied witnesses.
pub fn check_cleanup_plans(module: &Module, options: &LowOptions) -> Result<(), LowError> {
    let mut budget = Budget { options, steps: 0, blocks: 0, roots: 0 };
    check(module, &mut budget)
}
pub(super) fn check(module: &Module, budget: &mut Budget<'_>) -> Result<(), LowError> {
    let fresh = fresh_summaries(module, budget)?;
    for (_, function) in module.functions.iter() {
        let Some(body) = &function.body else {
            continue;
        };
        for (_, plan) in body.cleanup.plans.iter() {
            budget.step(1)?;
            let close = body.cleanup.closes.get(plan.close)?;
            let mut sources = vec![match close.source {
                CleanupSource::Scope(_) => MergeSource::ScopeInventory,
                CleanupSource::Packet(_) => MergeSource::PacketInventory,
            }];
            if close.reserve {
                sources.push(MergeSource::WholeReserveOnFailure);
            }
            if plan.sources != sources {
                return Err(LowError::InvalidOutput("plan omits a close or reserve source".into()));
            }
            let Some(proof_id) = plan.direct else {
                continue;
            };
            let proof = body.cleanup.proofs.get(proof_id)?;
            budget.step(body.insts.len() + body.blocks.len())?;
            let begins: Vec<_> = body.insts.iter().filter_map(|(instruction, inst)| match &inst.op {
                Operation::Cleanup(CleanupOp::Begin { state, pending, close: id, effects, failure })
                    if *id == plan.close && *effects == close.effects
                        && failure.is_some() == close.active_failure && pending.is_some() == close.reserve
                        && location(body, &HighOperationSite::Instruction(instruction))
                            .is_some_and(|(block, _)| block == close.after_defers) => Some((*state, *pending)),
                _ => None,
            }).collect();
            let [(state, pending)] = begins.as_slice() else {
                return Err(LowError::InvalidOutput("direct plan has no unique matching begin".into()));
            };
            let source_matches = match close.source {
                CleanupSource::Scope(scope) => module.types.get(body.values.get(*state)?.ty)?
                    == &Type::Service(ServiceType::Scope(scope)),
                CleanupSource::Packet(_) => module.types.get(body.values.get(*state)?.ty)?
                    == &Type::Service(ServiceType::Own),
            };
            if !source_matches {
                return Err(LowError::InvalidOutput("plan has the wrong owner or packet source".into()));
            }
            let reconstructed = prove(module, body, *state, *pending, close, &fresh, budget)?;
            if reconstructed.as_ref() != Some(proof) {
                return Err(LowError::InvalidOutput(
                    "direct order, active inventory, suffix or B2 evidence invalid".into(),
                ));
            }
        }
    }
    Ok(())
}
