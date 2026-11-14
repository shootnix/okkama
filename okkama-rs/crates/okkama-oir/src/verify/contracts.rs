use super::{EntityLocation as L, RuleId as R, State, VerifyEdgeId};
use crate::*;

pub(super) fn linear(module: &Module, ty: TypeId) -> bool {
    module.types.get(ty).is_ok_and(|ty| match ty {
        Type::Service(service) => service.is_linear(),
        Type::Builder(_) | Type::MapBuilder(..) | Type::Frame(_) => true,
        _ => false,
    })
}
fn linear_slot(module: &Module, body: &Body, ty: &ResultType) -> bool {
    match ty {
        ResultType::Service(service) => service.is_linear(),
        ResultType::BuilderOf(_) | ResultType::MapBuilderOf(..) | ResultType::Frame(_) => true,
        ResultType::Exact(ty) => linear(module, *ty),
        ResultType::SameAs(value) => body.values.get(*value).is_ok_and(|v| linear(module, v.ty)),
        _ => false,
    }
}

pub(super) fn check(module: &Module, selected: Option<FuncId>, state: &mut State<'_>) {
    for (function, data) in module.functions.iter() {
        if selected.is_some_and(|id| id != function) { continue; }
        let Some(body) = &data.body else { continue; };
        if !super::graph::eligible(function, body, state) { continue; }
        let mut edge = 0;
        for (_, block) in body.blocks.iter() {
            if block.unwind.is_some() { edge += 1; }
            if let TermKind::InvokeOwn { op, normal, unwind } = &block.term.kind {
                let Ok(contract) = ContractRegistry::new(module).lookup(op) else { continue; };
                state.at(L::Edge { function, edge: VerifyEdgeId(edge) }, block.term.loc);
                edge_slots(module, body, op, normal, Outcome::Normal, &contract.normal, state);
                state.at(L::Edge { function, edge: VerifyEdgeId(edge + 1) }, block.term.loc);
                if let Some(schema) = &contract.unwind {
                    edge_slots(module, body, op, unwind, Outcome::Unwind, schema, state);
                } else {
                    state.error(R::B62, "invoke requires an unwind schema");
                }
                if let Some(cancel) = &contract.cancellation {
                    // Cancellation shares the unwind target, with positional slots
                    // interpreted in its own outcome contract (decision 9.1).
                    if contract.unwind.as_ref().is_none_or(|schema| {
                        schema.results.len() != cancel.outcome.results.len()
                            || schema.results.iter().zip(&cancel.outcome.results).any(|(a, b)| a.ty != b.ty)
                    }) {
                        state.error(R::B62, "cancel and unwind must have identical positional slot types");
                    }
                    edge_slots(module, body, op, unwind, Outcome::Unwind, &cancel.outcome, state);
                }
                edge += 2;
            } else {
                edge += match &block.term.kind {
                    TermKind::Br(_) => 1,
                    TermKind::Cond { .. } | TermKind::Catch { .. } | TermKind::Lookup { .. } => 2,
                    TermKind::Switch { cases, .. } => cases.len() as u32 + 1,
                    _ => 0,
                };
            }
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn edge_slots(
    module: &Module,
    body: &Body,
    op: &Operation,
    edge: &InvokeEdge,
    outcome: Outcome,
    schema: &OutcomeContract,
    state: &mut State<'_>,
) {
    if !state.array(schema.results.len(), 8) { return; }
    let mut uses = vec![0_usize; schema.results.len()];
    for arg in &edge.args {
        if !state.step(1) { return; }
        if let EdgeArg::Produced(result) = arg {
            if result.outcome != outcome {
                state.error(R::B62, "edge result belongs to a different outcome");
            } else if let Some(count) = uses.get_mut(usize::from(result.slot)) {
                *count += 1;
            } else {
                state.error(R::B62, "edge result slot is absent from the outcome schema");
            }
        }
    }
    if let Ok(target) = body.blocks.get(edge.block) {
        for (arg, param) in edge.args.iter().zip(&target.params) {
            if let EdgeArg::Produced(result) = arg
                && result.outcome == outcome
                && let Some(slot) = schema.results.get(usize::from(result.slot))
                && let Ok(value) = body.values.get(*param)
                && !super::types::edge_slot_matches(module, body, op, &slot.ty, value.ty, state) {
                state.error(R::B62, "edge result type differs from its successor parameter type");
            }
        }
    }
    for (slot, count) in schema.results.iter().zip(uses) {
        if linear_slot(module, body, &slot.ty) && count != 1 {
            state.error(R::B62, "each linear edge result must be forwarded exactly once on its edge");
        }
    }
}
