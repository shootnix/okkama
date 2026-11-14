//! Bounded iterative expression and CFG utilities for the producer.
use super::*;
use crate::visit::{Rewrite, VisitorMut};
use std::collections::BTreeSet;

pub(super) fn leaves(
    body: &Body, roots: impl IntoIterator<Item = OperandRef>, budget: &mut Budget<'_>,
) -> Result<BTreeSet<ValueId>, LowError> {
    let mut pending: Vec<_> = roots.into_iter().collect();
    let mut seen = BTreeSet::new();
    let mut values = BTreeSet::new();
    while let Some(reference) = pending.pop() {
        budget.step(1)?;
        if !seen.insert(reference) {
            continue;
        }
        if let OperandRef::Value(value) = reference {
            body.values.get(value)?;
            values.insert(value);
        } else {
            let children = cfg::expression_children(reference, body)
                .map_err(|error| LowError::InvalidInput { function: None, message: error.to_string() })?;
            budget.step(children.len())?;
            pending.extend(children);
        }
    }
    Ok(values)
}
pub(super) fn operation(
    body: &Body, operation: &Operation, budget: &mut Budget<'_>,
) -> Result<BTreeSet<ValueId>, LowError> {
    let mut values = leaves(body, operation.operand_refs().into_iter().map(|operand| operand.reference), budget)?;
    if let Operation::Cleanup(CleanupOp::Begin { close, .. }) = operation {
        for (_, schedule) in body.cleanup.schedules.iter().filter(|(_, schedule)| schedule.close == *close) {
            budget.step(1)?;
            values.extend(schedule.dictionary);
        }
    }
    Ok(values)
}
#[derive(Default)]
struct References(Vec<OperandRef>);
impl VisitorMut for References {
    fn value(&mut self, value: &mut ValueId) { self.0.push(OperandRef::Value(*value)); }
    fn dependency(&mut self, value: &mut DepExprId) { self.0.push(OperandRef::Dependency(*value)); }
    fn selection(&mut self, value: &mut SelectionId) { self.0.push(OperandRef::Selection(*value)); }
    fn key(&mut self, value: &mut KeyExprId) { self.0.push(OperandRef::Key(*value)); }
}
pub(super) fn terminator(
    body: &Body, term: &TermKind, budget: &mut Budget<'_>,
) -> Result<BTreeSet<ValueId>, LowError> {
    let mut references = References::default();
    term.clone().rewrite(&mut references);
    leaves(body, references.0, budget)
}
pub(super) fn successors(body: &Body, block: BlockId) -> Result<BTreeSet<BlockId>, LowError> {
    let block = body.blocks.get(block)?;
    let mut targets = BTreeSet::new();
    if let Some(unwind) = &block.unwind {
        targets.insert(unwind.target.block);
    }
    match &block.term.kind {
        TermKind::Br(target) => { targets.insert(target.block); }
        TermKind::Cond { yes, no, .. } => { targets.extend([yes.block, no.block]); }
        TermKind::Switch { cases, default, .. } => {
            targets.extend(cases.iter().map(|case| case.target.block));
            targets.insert(default.block);
        }
        TermKind::InvokeOwn { normal, unwind, .. } => { targets.extend([normal.block, unwind.block]); }
        TermKind::InvokeDefer { continue_edge, replacements, unwind, .. } => {
            targets.insert(continue_edge.block);
            targets.extend(replacements.iter().map(|replacement| replacement.edge.block));
            targets.extend(unwind.iter().map(|edge| edge.block));
        }
        TermKind::FailureDispatch { none, some, .. } => { targets.extend([none.block, some.block]); }
        TermKind::CleanupNext { .. } | TermKind::ExitDispatch { .. } => {
            targets.extend(dispatch_edges(&block.term.kind).iter().map(|(_, edge)| edge.block));
        }
        TermKind::Catch { handler, next, .. } => { targets.extend([handler.block, next.block]); }
        TermKind::Lookup { found, missing, .. } => { targets.extend([found.block, missing.block]); }
        _ => {}
    }
    for target in &targets {
        body.blocks.get(*target)?;
    }
    Ok(targets)
}
