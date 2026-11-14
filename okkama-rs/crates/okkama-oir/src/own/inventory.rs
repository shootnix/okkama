//! A finite pre-solve may-offer inventory, not the ownership domain/rewrite.
//! False proves that a reference alone cannot bring an obligation into a scope.
//! True is deliberately conservative; it never claims that a packet is present.
use super::*;

pub(super) fn may_offer(
    module: &Module,
    function: &Function,
    body: &Body,
    types: &BTreeMap<TypeId, TypeClassification>,
    external: &BTreeMap<FuncId, Summary>,
    budget: &mut Budget<'_>,
) -> Result<Vec<bool>, OwnError> {
    budget.memory(body.values.len() * 16 + (body.blocks.len() + body.insts.len()) * 64)?;
    let mut offered = vec![false; body.values.len()];
    let entry = body.blocks.get(body.entry).map_err(arena)?;
    let boundary = function.signature.ownership_abi != OwnershipAbi::Observation
        && !function.summary.and_then(|id| module.summaries.get(id).ok())
            .is_some_and(|summary| summary.entry_kind == EntryKind::Internal);
    let destroy = function.attrs.iter().any(|attr| matches!(attr, FunctionAttr::Destroy(_)));
    let mut index = 0;
    for value in &entry.params {
        let ty = body.values.get(*value).map_err(arena)?.ty;
        if language(module, &[ty]).is_empty() {
            continue;
        }
        offered[value.as_u32() as usize] = boundary && (!destroy || index != 0) && types[&ty].may_own();
        index += 1;
    }
    loop {
        let mut changed = false;
        for (_, block) in body.blocks.iter() {
            for instruction in &block.insts {
                budget.work(1)?;
                let instruction = body.insts.get(*instruction).map_err(arena)?;
                let possible = operation(module, &instruction.op, &offered, external);
                for value in &instruction.results {
                    changed |= admit(body, types, &mut offered, *value, possible)?;
                }
            }
            let produced = match &block.term.kind {
                TermKind::InvokeOwn { op, .. } => operation(module, op, &offered, external),
                TermKind::Lookup { collection, .. } => offered[collection.as_u32() as usize],
                TermKind::InvokeDefer { .. } => true,
                _ => false,
            };
            let mut forward = |target: BlockId, arguments: Vec<EdgeArg>| -> Result<(), OwnError> {
                budget.work(arguments.len() + 1)?;
                for (parameter, argument) in body.blocks.get(target).map_err(arena)?.params.iter().zip(arguments) {
                    let possible = match argument {
                        EdgeArg::Value(value) => offered[value.as_u32() as usize],
                        EdgeArg::Produced(result) => {
                            matches!(result.outcome, Outcome::Normal | Outcome::DeferReplace(_)) && produced
                        }
                    };
                    changed |= admit(body, types, &mut offered, *parameter, possible)?;
                }
                Ok(())
            };
            let target_args = |target: &Target| target.args.iter().copied().map(EdgeArg::Value).collect();
            match &block.term.kind {
                TermKind::Br(target) => forward(target.block, target_args(target))?,
                TermKind::Cond { yes, no, .. } => {
                    forward(yes.block, target_args(yes))?;
                    forward(no.block, target_args(no))?;
                }
                TermKind::Switch { cases, default, .. } => {
                    for case in cases {
                        forward(case.target.block, target_args(&case.target))?;
                    }
                    forward(default.block, target_args(default))?;
                }
                TermKind::InvokeOwn { normal, unwind, .. } => {
                    forward(normal.block, normal.args.clone())?;
                    forward(unwind.block, unwind.args.clone())?;
                }
                TermKind::InvokeDefer { continue_edge, replacements, unwind, .. } => {
                    forward(continue_edge.block, continue_edge.args.clone())?;
                    for replacement in replacements {
                        forward(replacement.edge.block, replacement.edge.args.clone())?;
                    }
                    if let Some(unwind) = unwind {
                        forward(unwind.block, unwind.args.clone())?;
                    }
                }
                TermKind::Catch { handler, next, .. } => {
                    forward(handler.block, handler.args.clone())?;
                    forward(next.block, target_args(next))?;
                }
                TermKind::Lookup { found, missing, .. } => {
                    forward(found.block, found.args.clone())?;
                    forward(missing.block, target_args(missing))?;
                }
                _ => {}
            }
            if let Some(unwind) = &block.unwind {
                forward(unwind.target.block, target_args(&unwind.target))?;
            }
        }
        if !changed {
            break;
        }
    }
    Ok(offered)
}

fn admit(
    body: &Body,
    types: &BTreeMap<TypeId, TypeClassification>,
    offered: &mut [bool],
    value: ValueId,
    possible: bool,
) -> Result<bool, OwnError> {
    let ty = body.values.get(value).map_err(arena)?.ty;
    let next = possible && types[&ty].may_own();
    let previous = &mut offered[value.as_u32() as usize];
    let changed = next && !*previous;
    *previous |= next;
    Ok(changed)
}

fn operation(
    module: &Module,
    op: &Operation,
    offered: &[bool],
    external: &BTreeMap<FuncId, Summary>,
) -> bool {
    if matches!(op, Operation::Core(CoreOp::ObjNew { construction: ConstructionStatus::Complete, .. }
        | CoreOp::ObjNext { .. } | CoreOp::VariantNew { .. } | CoreOp::FakeNew { .. }) | Operation::Async(_)) {
        return true;
    }
    if let Some((call, receiver)) = summaries::call(op) {
        let target = match op {
            Operation::Call(CallOp::Direct { function, .. } | CallOp::Native { function, .. }
                | CallOp::Intrinsic { function, .. } | CallOp::Resume { function, .. }) => Some(*function),
            _ => None,
        };
        let summary = target.and_then(|target| external.get(&target));
        if let Some(summary) = summary {
            let services = summary.entry_kind == EntryKind::CallBoundary
                && summary.contract.results.iter().any(|ty| {
                    matches!(module.types.get(*ty), Ok(Type::Future(_) | Type::Frame(_)))
                });
            if services || summary.results.iter().any(|result| !result.fresh.is_empty()) {
                return true;
            }
            return receiver.iter().chain(&call.args).any(|argument| offered[argument.value.as_u32() as usize])
                && summary.results.iter().any(|result| !result.aliases.is_empty() || !result.external_deps.is_empty());
        }
        let observation = target.and_then(|id| module.functions.get(id).ok())
            .is_some_and(|function| function.signature.ownership_abi == OwnershipAbi::Observation);
        if !observation {
            return true;
        }
    }
    op.operand_refs().iter().any(|operand| match operand.reference {
        OperandRef::Value(value) => offered[value.as_u32() as usize],
        _ => false,
    })
}
