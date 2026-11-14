use super::*;
use std::collections::BTreeSet;

fn trace(module: &Module, body: &Body, value: ValueId) -> Result<Option<RootTrace>, LowError> {
    let data = body.values.get(value)?;
    let trace = match module.types.get(data.ty)? {
        Type::Service(ServiceType::CleanupState) => RootTrace::CleanupActivation,
        Type::Service(ServiceType::Scope(_)) => RootTrace::ScopeInventory,
        Type::Service(ServiceType::Own) => RootTrace::PacketInventory,
        Type::Service(ServiceType::Pending) => RootTrace::PendingValueReserveDeps,
        Type::Service(ServiceType::Failure | ServiceType::FailureState) => RootTrace::FailureGraph,
        Type::Service(ServiceType::ExitState) => RootTrace::ExitReserve,
        Type::Service(ServiceType::Deps) => RootTrace::FullDependencies,
        Type::Service(_) => return Ok(None),
        Type::Bool | Type::Byte | Type::Float | Type::Char | Type::Never => return Ok(None),
        _ => RootTrace::Value(data.ty),
    };
    Ok(Some(trace))
}
fn record(
    module: &Module, body: &mut Body, boundary: RootBoundary, live: &BTreeSet<ValueId>, budget: &mut Budget<'_>,
) -> Result<(), LowError> {
    let mut expanded = live.clone();
    for value in live {
        budget.step(1)?;
        match &body.values.get(*value)?.facts {
            ValueFacts::Managed { deps } => {
                let mut pending = vec![OperandRef::Dependency(*deps)];
                let mut seen = BTreeSet::new();
                while let Some(reference) = pending.pop() {
                    budget.step(1)?;
                    if !seen.insert(reference) {
                        continue;
                    }
                    if let OperandRef::Value(value) = reference {
                        expanded.insert(value);
                    } else {
                        pending.extend(cfg::expression_children(reference, body)
                            .map_err(|error| LowError::InvalidOutput(error.to_string()))?);
                    }
                }
            }
            ValueFacts::Plain { certificate: PlainCertificate::Descriptor(dictionary) } => {
                expanded.insert(*dictionary);
            }
            _ => {}
        }
    }
    let mut bindings = vec![];
    for value in expanded {
        if let Some(trace) = trace(module, body, value)? {
            budget.roots(1)?;
            bindings.push(RootBinding { value, trace });
        }
    }
    body.cleanup.roots.alloc(RootRequirement { boundary, bindings })?;
    Ok(())
}

pub(super) fn collect(module: &Module, body: &mut Body, budget: &mut Budget<'_>) -> Result<(), LowError> {
    let count = body.blocks.len();
    let mut incoming = vec![BTreeSet::<ValueId>::new(); count];
    let mut outgoing = incoming.clone();
    let blocks: Vec<_> = body.blocks.iter().map(|(id, _)| id).collect();
    loop {
        let mut changed = false;
        for &id in blocks.iter().rev() {
            budget.step(1)?;
            let data = body.blocks.get(id)?;
            let mut live = graph::terminator(body, &data.term.kind, budget)?;
            for successor in graph::successors(body, id)? {
                budget.step(incoming[successor.as_u32() as usize].len())?;
                live.extend(incoming[successor.as_u32() as usize].iter().copied());
            }
            outgoing[id.as_u32() as usize] = live.clone();
            for instruction in data.insts.iter().rev() {
                let inst = body.insts.get(*instruction)?;
                for result in &inst.results {
                    live.remove(result);
                }
                live.extend(graph::operation(body, &inst.op, budget)?);
            }
            for param in &data.params {
                live.remove(param);
            }
            if let Some(unwind) = &data.unwind {
                live.extend(unwind.target.args.iter().copied());
            }
            budget.step(live.len())?;
            if incoming[id.as_u32() as usize] != live {
                budget.roots(live.len())?;
                incoming[id.as_u32() as usize] = live;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for id in blocks {
        let data = body.blocks.get(id)?.clone();
        let mut live = outgoing[id.as_u32() as usize].clone();
        record(module, body, RootBoundary::Terminator(id), &live, budget)?;
        for instruction in data.insts.iter().rev() {
            let inst = body.insts.get(*instruction)?.clone();
            let contract = ContractRegistry::new(module).construction_schema(&inst.op, body)
                .map_err(|error| LowError::InvalidOutput(error.to_string()))?;
            let mut after = live.clone();
            after.extend(inst.results.iter().copied());
            record(module, body, RootBoundary::AfterInstruction(*instruction), &after, budget)?;
            for result in &inst.results {
                live.remove(result);
            }
            live.extend(graph::operation(body, &inst.op, budget)?);
            if contract.unwind.is_some()
                && let Some(unwind) = &data.unwind {
                live.extend(unwind.target.args.iter().copied());
            }
            record(module, body, RootBoundary::BeforeInstruction(*instruction), &live, budget)?;
        }
        live.extend(data.params);
        record(module, body, RootBoundary::Entry(id), &live, budget)?;
    }
    Ok(())
}
