//! Local synchronous phase checks. Full B10 order, conservation, D18 and roots remain #564-4.
use super::{EntityLocation as L, RuleId as R, State};
use crate::*;
use std::collections::BTreeSet;

const BEGUN: u8 = 1;
const SETTLED: u8 = 2;
const PLANNED: u8 = 4;
const SELECTED: u8 = 8;
const CURRENT: u8 = 16;
const CAPTURED: u8 = 32;
const RECORD: u8 = 64;
const DRAINED: u8 = 128;

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
        if !super::graph::eligible(function, body, state) || !state.array(body.values.len(), 32) {
            continue;
        }
        let mut phases = vec![0_u8; body.values.len()];
        let mut origins = vec![BTreeSet::new(); body.values.len()];
        let edges = super::graph::edges(body, state);
        loop {
            let mut changed = false;
            for (_, inst) in body.insts.iter() {
                if !state.step(1) {
                    return;
                }
                if let Some((_, phase)) = transition(&inst.op)
                    && let Some(result) = inst.results.first() {
                    changed |= join(&mut phases, *result, phase);
                    let source = match &inst.op {
                        Operation::Cleanup(CleanupOp::Begin { close, .. }) => BTreeSet::from([*close]),
                        _ => transition(&inst.op).and_then(|(input, _)| input)
                            .map(|(value, _)| origins[value.as_u32() as usize].clone()).unwrap_or_default(),
                    };
                    for close in source {
                        if !state.step(1) { return; }
                        if origins[result.as_u32() as usize].insert(close) {
                            if !state.memory(32) { return; }
                            changed = true;
                        }
                    }
                }
            }
            for edge in &edges {
                if !state.step(edge.args.len() + 1) {
                    return;
                }
                let Ok(source) = body.blocks.get(edge.source) else {
                    continue;
                };
                let Ok(target) = body.blocks.get(edge.target) else {
                    continue;
                };
                for (arg, parameter) in edge.args.iter().zip(&target.params) {
                    if let EdgeArg::Produced(result) = arg {
                        if let TermKind::CleanupNext { cleanup, .. } = source.term.kind
                            && result.slot == 0 {
                            let phase = if result.outcome == Outcome::CleanupSelected { CURRENT } else { DRAINED };
                            changed |= join(&mut phases, *parameter, phase);
                            for close in origins[cleanup.as_u32() as usize].clone() {
                                if !state.step(1) { return; }
                                if origins[parameter.as_u32() as usize].insert(close) {
                                    if !state.memory(32) { return; }
                                    changed = true;
                                }
                            }
                        }
                        continue;
                    }
                    let EdgeArg::Value(value) = arg else { unreachable!() };
                    let mut phase = phases[value.as_u32() as usize];
                    match &source.term.kind {
                        TermKind::Cond { condition, yes, no } => {
                            if let Ok(data) = body.values.get(*condition)
                                && let Definition::InstResult { inst, index: 2 } = data.def
                                && let Ok(inst) = body.insts.get(inst)
                                && matches!(inst.op, Operation::Cleanup(CleanupOp::Next { .. }))
                                && inst.results.first() == Some(value) {
                                if edge.target == yes.block && yes.block != no.block {
                                    phase = CURRENT;
                                } else if edge.target == no.block && yes.block != no.block {
                                    phase = DRAINED;
                                }
                            }
                        }
                        TermKind::FailureDispatch { none, some, .. } if phase == CAPTURED => {
                            if edge.target == none.block && none.block != some.block {
                                phase = PLANNED;
                            } else if edge.target == some.block && none.block != some.block {
                                phase = RECORD;
                            }
                        }
                        _ => {}
                    }
                    changed |= join(&mut phases, *parameter, phase);
                    for close in origins[value.as_u32() as usize].clone() {
                        if !state.step(1) { return; }
                        if origins[parameter.as_u32() as usize].insert(close) {
                            if !state.memory(32) { return; }
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
        for (block_id, block) in body.blocks.iter() {
            if let TermKind::CleanupNext { cleanup, .. } = block.term.kind
                && phases[cleanup.as_u32() as usize] != PLANNED {
                state.at(L::Terminator { function, block: block_id }, block.term.loc);
                state.error(R::B101, "runtime selection requires a settled serial plan");
            }
        }
        for (instruction, inst) in body.insts.iter() {
            if !state.step(1) {
                return;
            }
            let Some((input, _)) = transition(&inst.op) else {
                continue;
            };
            let Some((value, required)) = input else {
                continue;
            };
            let actual = &origins[value.as_u32() as usize];
            let expected = match &inst.op {
                Operation::Cleanup(CleanupOp::SettleEmpty { schedule, .. }) => {
                    body.cleanup.schedules.get(*schedule).ok().map(|item| item.close)
                }
                Operation::Cleanup(CleanupOp::Plan { plan, .. }) => {
                    body.cleanup.plans.get(*plan).ok().map(|item| item.close)
                }
                _ => None,
            };
            let valid_close = actual.len() == 1 && expected.is_none_or(|close| actual.contains(&close));
            if phases[value.as_u32() as usize] != required || !valid_close {
                state.at(L::Instruction { function, instruction }, inst.loc);
                state.error(R::B101, "cleanup step violates the synchronous phase or skips the remaining cleanup");
            }
        }
    }
}
fn join(phases: &mut [u8], value: ValueId, phase: u8) -> bool {
    let old = phases[value.as_u32() as usize];
    phases[value.as_u32() as usize] |= phase;
    old != phases[value.as_u32() as usize]
}
fn transition(op: &Operation) -> Option<(Option<(ValueId, u8)>, u8)> {
    let Operation::Cleanup(op) = op else {
        return None;
    };
    Some(match op {
        CleanupOp::Begin { .. } => (None, BEGUN),
        CleanupOp::SettleEmpty { cleanup, .. } => (Some((*cleanup, BEGUN)), SETTLED),
        CleanupOp::Plan { cleanup, .. } => (Some((*cleanup, SETTLED)), PLANNED),
        CleanupOp::Next { cleanup } => (Some((*cleanup, PLANNED)), SELECTED),
        CleanupOp::Destroy { cleanup, .. } => (Some((*cleanup, CURRENT)), CAPTURED),
        CleanupOp::RecordFailure { cleanup, .. } => (Some((*cleanup, RECORD)), PLANNED),
        CleanupOp::Finish { cleanup } => (Some((*cleanup, DRAINED)), 0),
        _ => return None,
    })
}
