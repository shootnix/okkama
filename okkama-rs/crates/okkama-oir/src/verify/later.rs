use super::State;
use crate::Module;

// These hooks describe the deliberately unimplemented semantic portion of #564 commit 4.
// Commit 0 checks payloads, types, declarations, edge slots and cancellation inventories only.
pub(super) fn async_cleanup_and_profiles(module: &Module, state: &mut State<'_>) {
    defer_settlement_and_replacement_semantics(module, state);
    shielded_cleanup_cancellation_semantics(module, state);
    reserve_extension_authority_semantics(module, state);
    scope_effect_envelope_soundness(module, state);
}
fn defer_settlement_and_replacement_semantics(_: &Module, _: &mut State<'_>) {}
fn shielded_cleanup_cancellation_semantics(_: &Module, _: &mut State<'_>) {}
fn reserve_extension_authority_semantics(_: &Module, _: &mut State<'_>) {}
fn scope_effect_envelope_soundness(_: &Module, _: &mut State<'_>) {}
