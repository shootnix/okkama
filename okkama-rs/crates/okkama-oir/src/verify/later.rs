use super::State;
use crate::Module;

// These hooks describe the deliberately unimplemented semantic portion of #564 commit 4.
// Commit 0 checks payloads, types, declarations, edge slots and cancellation inventories only.
pub(super) fn async_cleanup_and_profiles(
    module: &Module, selected: Option<crate::FuncId>, state: &mut State<'_>,
) {
    low_profile(module, selected, state);
    defer_settlement_and_replacement_semantics(module, state);
    shielded_cleanup_cancellation_semantics(module, state);
    reserve_extension_authority_semantics(module, state);
    scope_effect_envelope_soundness(module, state);
}
fn defer_settlement_and_replacement_semantics(_: &Module, _: &mut State<'_>) {}
fn shielded_cleanup_cancellation_semantics(_: &Module, _: &mut State<'_>) {}
fn reserve_extension_authority_semantics(_: &Module, _: &mut State<'_>) {}
fn scope_effect_envelope_soundness(_: &Module, _: &mut State<'_>) {}

fn low_profile(module: &Module, selected: Option<crate::FuncId>, state: &mut State<'_>) {
    use crate::*;
    if module.profile != Profile::Low { return; }
    for (function, data) in module.functions.iter() {
        if selected.is_some_and(|id| id != function) { continue; }
        let Some(body) = &data.body else {
            continue;
        };
        for (block_id, block) in body.blocks.iter() {
            for &instruction in &block.insts {
                if let Ok(inst) = body.insts.get(instruction) {
                    state.at(EntityLocation::Instruction { function, instruction }, inst.loc);
                    check_low_op(&inst.op, state);
                }
            }
            if let TermKind::InvokeOwn { op, .. } = &block.term.kind {
                state.at(EntityLocation::Terminator { function, block: block_id }, block.term.loc);
                check_low_op(op, state);
            }
        }
    }
}
fn check_low_op(op: &crate::Operation, state: &mut State<'_>) {
    use crate::*;
    if matches!(op, Operation::Lexical(_) | Operation::Ownership(OwnershipOp::ScopeEnd { .. }
        | OwnershipOp::ScopeUnwind { .. } | OwnershipOp::End { .. })
        | Operation::Cleanup(CleanupOp::Next { .. } | CleanupOp::Defer { .. }
            | CleanupOp::Shield { .. } | CleanupOp::DeliverCancel { .. } | CleanupOp::Settle { .. })) {
        state.error(RuleId::B114, "High/Own-only or legacy cleanup operation remains in P1 Low");
    }
}
