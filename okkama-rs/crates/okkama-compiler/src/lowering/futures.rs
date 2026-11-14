//! Conservative unique-cache proofs, independent of optional OIR passes (#581).
//!
//! Async calls allocate frames; a plain action may also return a fresh frame.
//! A single SSA use alone is insufficient: require fresh provenance, reject
//! every publication/alias, and keep bound extraction at its original owner.
use super::ctx::Ctx;
use crate::ir::{Function, Inst, InstKind, TermKind, Type};

/// Re-entering an await without allocating again would consume twice, even
/// with only one syntactic use. Cutting the definition block must break every
/// cycle through the await (including unwind edges).
fn once_per_definition(function: &Function, definition: usize, awaiting: usize) -> bool {
    if definition == awaiting { return true; }
    let mut seen = vec![false; function.blocks.len()];
    let mut pending = crate::ir::cfg::all_successors(function, crate::ir::BlockId(awaiting));
    while let Some(block) = pending.pop() {
        if block.0 == definition { continue; }
        if block.0 == awaiting { return false; }
        if !std::mem::replace(&mut seen[block.0], true) {
            pending.extend(crate::ir::cfg::all_successors(function, block));
        }
    }
    true
}

fn bookkeeping(kind: &InstKind) -> bool {
    matches!(kind, InstKind::RegionTrack { .. } | InstKind::RegionAdopt { .. }
        | InstKind::RegionTransfer { .. } | InstKind::RegionLeave { .. }
        | InstKind::RegionSettle { .. } | InstKind::RegionUnwind { .. }
        | InstKind::RegionSettleUnwind { .. })
}

fn uses(function: &Function) -> Vec<usize> {
    let mut uses = vec![0; function.values.len()];
    for block in &function.blocks {
        for inst in &block.insts {
            if !bookkeeping(&inst.kind) {
                for value in inst.kind.operands() { uses[value.0] += 1; }
            }
        }
        for value in block.term.kind.operands() { uses[value.0] += 1; }
        for target in block.term.kind.targets().into_iter().chain(block.unwind.as_ref()) {
            for value in &target.args { uses[value.0] += 1; }
        }
    }
    uses
}

/// A definition is fresh only if its producer cannot return a stored alias.
/// Spawn preserves the proof only when it is the input's sole real use.
fn fresh_values(function: &Function, fresh_calls: &[bool], fresh_results: &[bool], async_calls: &[bool], uses: &[usize]) -> Vec<Option<usize>> {
    let mut fresh = vec![None; function.values.len()];
    let mut fresh_outcome = vec![false; function.values.len()];
    for (block_index, block) in function.blocks.iter().enumerate() {
        for inst in &block.insts {
            let Some(value) = inst.result else { continue };
            if !matches!(function.values[value.0], Type::Future(_)) { continue; }
            let unique = match inst.kind {
                InstKind::Call { func, .. } => {
                    fresh_outcome[value.0] = async_calls[func.0] && fresh_results[func.0];
                    fresh_calls[func.0]
                }
                InstKind::FutSpawn(input) => {
                    fresh_outcome[value.0] = fresh_outcome[input.0];
                    fresh[input.0] == Some(block_index) && uses[input.0] == 1
                }
                // Only extraction exposes an exclusive outcome. Ordinary
                // await of even a fresh-producing action can have other views.
                InstKind::AwaitTake(input) => fresh_outcome[input.0],
                _ => false,
            };
            if unique { fresh[value.0] = Some(block_index); }
        }
    }
    fresh
}

pub(super) fn take_unique(ctx: &mut Ctx<'_, '_>, functions: &mut [Function]) {
    let all_uses: Vec<_> = functions.iter().map(uses).collect();
    let async_calls: Vec<_> = functions.iter().map(Function::is_async).collect();
    let mut fresh_calls = async_calls.clone();
    let mut fresh_results = vec![false; functions.len()];
    loop {
        let mut changed = false;
        for (index, function) in functions.iter().enumerate() {
            if fresh_results[index] || !matches!(function.result, crate::ir::ResultType::Value(Type::Future(_))) { continue; }
            let fresh = fresh_values(function, &fresh_calls, &fresh_results, &async_calls, &all_uses[index]);
            let mut returned = false;
            let proven = function.blocks.iter().all(|body| {
                if let TermKind::Ret(Some(value)) = body.term.kind {
                    returned = true;
                    fresh[value.0].is_some() && all_uses[index][value.0] == 1
                } else { true }
            });
            if returned && proven {
                fresh_results[index] = true;
                fresh_calls[index] = true;
                changed = true;
            }
        }
        if !changed { break; }
    }
    for (index, function) in functions.iter_mut().enumerate() {
        if !function.blocks.iter().flat_map(|block| &block.insts).any(|inst| matches!(inst.kind, InstKind::Await(_))) {
            continue;
        }
        let fresh = fresh_values(function, &fresh_calls, &fresh_results, &async_calls, &all_uses[index]);
        let eligible: Vec<_> = function.blocks.iter().enumerate().map(|(block_index, block)| {
            block.insts.iter().filter_map(|inst| match inst.kind {
                InstKind::Await(future) if all_uses[index][future.0] == 1
                    && fresh[future.0].is_some_and(|definition| once_per_definition(function, definition, block_index)) => Some(future),
                _ => None,
            }).collect::<std::collections::HashSet<_>>()
        }).collect();
        for (block_index, block) in function.blocks.iter_mut().enumerate() {
            let mut insts = Vec::with_capacity(block.insts.len());
            for mut inst in std::mem::take(&mut block.insts) {
                let extraction = if let InstKind::Await(future) = inst.kind
                    && eligible[block_index].contains(&future)
                {
                    ctx.await_regions.get(index).and_then(|sites| sites.get(&future)).copied().map(|(region, bound)| {
                        inst.kind = InstKind::AwaitTake(future);
                        (region, bound)
                    })
                } else { None };
                let result = inst.result;
                let span = inst.span;
                insts.push(inst);
                if let (Some((region, bound)), Some(value)) = (extraction, result) {
                    let ty = &function.values[value.0];
                    if !ctx.is_plain(ty) {
                        // Bound results never gain fresh call admission: adopt
                        // them immediately at the binding's cleanup boundary.
                        let kind = if bound { InstKind::RegionAdopt { region, value } }
                            else { InstKind::RegionTrack { region, value } };
                        insts.push(Inst { result: None, kind, span });
                        if let Some(Some(types)) = ctx.settle_types.get_mut(index)
                            && !types.contains(ty) { types.push(ty.clone()); }
                    }
                }
            }
            block.insts = insts;
        }
    }
}
