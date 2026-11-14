//! Independent boundary liveness and dependency closure; no producer plans are trusted.
use super::{EntityLocation as L, RuleId as R, State, read::{Read, Reader}};
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
struct References(Vec<OperandRef>);
impl Reader for References {
    fn value(&mut self, value: &ValueId) { self.0.push(OperandRef::Value(*value)); }
    fn dependency(&mut self, value: &DepExprId) { self.0.push(OperandRef::Dependency(*value)); }
    fn selection(&mut self, value: &SelectionId) { self.0.push(OperandRef::Selection(*value)); }
    fn key(&mut self, value: &KeyExprId) { self.0.push(OperandRef::Key(*value)); }
}
fn uses(item: &impl Read, body: &Body, state: &mut State<'_>) -> BTreeSet<ValueId> {
    let mut references = References::default();
    item.read(&mut references);
    let mut pending = references.0;
    let mut seen = BTreeSet::new();
    let mut values = BTreeSet::new();
    while let Some(reference) = pending.pop() {
        if !state.step(1) || !state.memory(48) {
            break;
        }
        if !seen.insert(reference) {
            continue;
        }
        if let OperandRef::Value(value) = reference {
            values.insert(value);
        } else if let Ok(children) = cfg::expression_children(reference, body) {
            pending.extend(children);
        }
    }
    values
}
fn operands(op: &Operation, body: &Body, state: &mut State<'_>) -> BTreeSet<ValueId> {
    let mut values = uses(op, body, state);
    if let Operation::Cleanup(CleanupOp::Begin { close, .. }) = op {
        for (_, schedule) in body.cleanup.schedules.iter().filter(|(_, schedule)| schedule.close == *close) {
            values.extend(schedule.dictionary);
        }
    }
    values
}
fn traced(module: &Module, body: &Body, value: ValueId) -> bool {
    body.values.get(value).ok().and_then(|data| module.types.get(data.ty).ok()).is_some_and(|ty| {
        match ty {
            Type::Bool | Type::Byte | Type::Float | Type::Char | Type::Never => false,
            Type::Service(service) => matches!(service, ServiceType::CleanupState | ServiceType::Scope(_)
                | ServiceType::Own | ServiceType::Pending | ServiceType::Failure | ServiceType::FailureState
                | ServiceType::ExitState | ServiceType::Deps),
            _ => true,
        }
    })
}
fn boundary_key(boundary: &RootBoundary) -> (u8, u32) {
    match *boundary {
        RootBoundary::Entry(block) => (0, block.as_u32()),
        RootBoundary::BeforeInstruction(inst) => (1, inst.as_u32()),
        RootBoundary::AfterInstruction(inst) => (2, inst.as_u32()),
        RootBoundary::Terminator(block) => (3, block.as_u32()),
    }
}
type Maps<'a> = BTreeMap<(u8, u32), Vec<&'a RootRequirement>>;
fn boundary(
    module: &Module, function: FuncId, body: &Body, boundary: RootBoundary,
    live: &BTreeSet<ValueId>, maps: &Maps<'_>, state: &mut State<'_>,
) {
    let location = match boundary {
        RootBoundary::BeforeInstruction(instruction) | RootBoundary::AfterInstruction(instruction) => {
            L::Instruction { function, instruction }
        }
        RootBoundary::Entry(block) => L::Block { function, block },
        RootBoundary::Terminator(block) => L::Terminator { function, block },
    };
    state.at(location, None);
    let maps = maps.get(&boundary_key(&boundary)).map_or(&[][..], Vec::as_slice);
    if maps.len() != 1 {
        state.error(R::B105, "Low requires exactly one root map at every SSA boundary");
        return;
    }
    let map = maps[0];
    if !state.step(live.len() + map.bindings.len()) || !state.array(live.len() + map.bindings.len(), 64) {
        return;
    }
    let bindings: BTreeSet<_> = map.bindings.iter().map(|binding| binding.value).collect();
    if bindings.len() != map.bindings.len() {
        state.error(R::B105, "root map repeats a concrete SSA holder");
    }
    let mut required = live.clone();
    let mut pending: Vec<_> = live.iter().copied().collect();
    while let Some(value) = pending.pop() {
        if !state.step(1) {
            return;
        }
        let Ok(data) = body.values.get(value) else {
            continue;
        };
        for dependency in super::dominance::leaves(&data.facts, body, state) {
            if required.insert(dependency) {
                pending.push(dependency);
            }
        }
    }
    if required.iter().any(|value| traced(module, body, *value) && !bindings.contains(value)) {
        state.error(R::B105, "root map omits a live value, ownership holder or full dependency operand");
    }
}
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
        if !super::graph::eligible(function, body, state) || !state.array(body.blocks.len(), 96) {
            continue;
        }
        let mut maps = Maps::new();
        for (_, map) in body.cleanup.roots.iter() {
            if !state.step(1) || !state.memory(64) {
                return;
            }
            maps.entry(boundary_key(&map.boundary)).or_default().push(map);
        }
        let edges = super::graph::edges(body, state);
        let mut successors = vec![BTreeSet::new(); body.blocks.len()];
        for edge in &edges {
            if !state.step(1) || !state.memory(48) {
                return;
            }
            successors[edge.source.as_u32() as usize].insert(edge.target);
        }
        let blocks: Vec<_> = body.blocks.iter().collect();
        let mut incoming = vec![BTreeSet::new(); body.blocks.len()];
        let mut outgoing = incoming.clone();
        let mut inst_uses = BTreeMap::new();
        let mut term_uses = BTreeMap::new();
        for (id, block) in body.blocks.iter() {
            term_uses.insert(id, uses(&block.term.kind, body, state));
            for instruction in &block.insts {
                if let Ok(inst) = body.insts.get(*instruction) {
                    inst_uses.insert(*instruction, operands(&inst.op, body, state));
                }
            }
        }
        loop {
            let mut changed = false;
            for &(id, block) in blocks.iter().rev() {
                if !state.step(block.insts.len() + 1) {
                    return;
                }
                let mut live = term_uses[&id].clone();
                for target in &successors[id.as_u32() as usize] {
                    live.extend(&incoming[target.as_u32() as usize]);
                }
                outgoing[id.as_u32() as usize] = live.clone();
                for instruction in block.insts.iter().rev() {
                    let inst = body.insts.get(*instruction).unwrap();
                    for result in &inst.results {
                        live.remove(result);
                    }
                    live.extend(&inst_uses[instruction]);
                }
                for param in &block.params {
                    live.remove(param);
                }
                if let Some(unwind) = &block.unwind {
                    live.extend(&unwind.target.args);
                }
                if !state.array(live.len(), 48) {
                    return;
                }
                let target = &mut incoming[id.as_u32() as usize];
                if *target != live {
                    *target = live;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for (id, block) in body.blocks.iter() {
            let mut live = outgoing[id.as_u32() as usize].clone();
            boundary(module, function, body, RootBoundary::Terminator(id), &live, &maps, state);
            for instruction in block.insts.iter().rev() {
                let inst = body.insts.get(*instruction).unwrap();
                let mut after = live.clone();
                after.extend(&inst.results);
                boundary(module, function, body, RootBoundary::AfterInstruction(*instruction), &after, &maps, state);
                for result in &inst.results {
                    live.remove(result);
                }
                live.extend(&inst_uses[instruction]);
                if ContractRegistry::new(module).lookup(&inst.op).is_ok_and(|contract| contract.unwind.is_some())
                    && let Some(unwind) = &block.unwind {
                    live.extend(&unwind.target.args);
                }
                boundary(module, function, body, RootBoundary::BeforeInstruction(*instruction), &live, &maps, state);
            }
            live.extend(&block.params);
            boundary(module, function, body, RootBoundary::Entry(id), &live, &maps, state);
        }
    }
}
