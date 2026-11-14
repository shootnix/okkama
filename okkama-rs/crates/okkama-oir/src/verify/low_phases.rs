//! Close provenance checks supplement the relational cleanup automaton.
//! Phase, selector, failure, direct cursor and drained-state checks live in linearity/cleanup.rs.
use super::{EntityLocation as L, RuleId as R, State};
use crate::*;

pub(super) fn check(module: &Module, selected: Option<FuncId>, state: &mut State<'_>) {
    if module.profile != Profile::Low {
        return;
    }
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
        if !body.service_schedules.is_empty() {
            state.at(L::Function(function), declaration.loc);
            state.error(R::B106, "Low service duties must use the close-local typed schedule arena");
        }
        for (_, schedule) in body.cleanup.schedules.iter() {
            if let Some(dictionary) = schedule.dictionary {
                let ty = body.values.get(dictionary).ok().and_then(|value| module.types.get(value.ty).ok());
                if ty != Some(&Type::Descriptor) {
                    state.at(L::Value { function, value: dictionary }, None);
                    state.error(R::B106, "service schedule dictionary must be descriptor-typed");
                }
            }
        }
        for (_, map) in body.cleanup.roots.iter() {
            for binding in &map.bindings {
                if !state.step(1) {
                    return;
                }
                let ty = body.values.get(binding.value).ok().and_then(|value| module.types.get(value.ty).ok());
                let valid = match (&binding.trace, ty) {
                    (RootTrace::Value(expected), Some(ty)) if !matches!(ty, Type::Service(_)) => {
                        body.values.get(binding.value).is_ok_and(|value| value.ty == *expected)
                    }
                    (RootTrace::Descriptor(_), Some(Type::Descriptor)) => true,
                    (RootTrace::FullDependencies, Some(Type::Service(ServiceType::Deps))) => true,
                    (RootTrace::CleanupActivation, Some(Type::Service(ServiceType::CleanupState))) => true,
                    (RootTrace::ScopeInventory, Some(Type::Service(ServiceType::Scope(_)))) => true,
                    (RootTrace::PacketInventory, Some(Type::Service(ServiceType::Own))) => true,
                    (RootTrace::PendingValueReserveDeps, Some(Type::Service(ServiceType::Pending))) => true,
                    (RootTrace::FailureGraph,
                        Some(Type::Service(ServiceType::Failure | ServiceType::FailureState))) => true,
                    (RootTrace::ExitReserve, Some(Type::Service(ServiceType::ExitState))) => true,
                    _ => false,
                };
                if !valid {
                    state.at(L::Value { function, value: binding.value }, None);
                    state.error(R::B105, "root binding trace must agree with its concrete SSA holder type");
                }
            }
        }
        for (block_id, block) in body.blocks.iter() {
            for &instruction in &block.insts {
                if !state.step(1) {
                    return;
                }
                let Ok(inst) = body.insts.get(instruction) else {
                    continue;
                };
                let Operation::Cleanup(CleanupOp::Begin { close, state: input, .. }) = inst.op else {
                    continue;
                };
                state.at(L::Instruction { function, instruction }, inst.loc);
                if body.cleanup.closes.get(close).is_ok_and(|close| close.after_defers != block_id) {
                    state.error(R::B101, "cleanup begin must follow its declared reached defer prefix");
                }
                for (_, plan) in body.cleanup.plans.iter().filter(|(_, plan)| plan.close == close) {
                    if let Some(proof) = plan.direct.and_then(|id| body.cleanup.proofs.get(id).ok())
                        && proof.state != input {
                        state.error(R::B102, "direct proof must describe the source consumed by this close");
                    }
                }
            }
        }
    }
}
