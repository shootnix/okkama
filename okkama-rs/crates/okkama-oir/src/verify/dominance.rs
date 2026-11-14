use super::{graph::Dominators, read::{Read, Reader}, EntityLocation as L, RuleId as R, State};
use crate::*;
use std::collections::BTreeSet;

#[derive(Default)]
struct References(Vec<OperandRef>);
impl Reader for References {
    fn value(&mut self, value: &ValueId) { self.0.push(OperandRef::Value(*value)); }
    fn dependency(&mut self, id: &DepExprId) { self.0.push(OperandRef::Dependency(*id)); }
    fn key(&mut self, id: &KeyExprId) { self.0.push(OperandRef::Key(*id)); }
    fn selection(&mut self, id: &SelectionId) { self.0.push(OperandRef::Selection(*id)); }
}

pub(super) fn leaves(item: &impl Read, body: &Body, state: &mut State<'_>) -> Vec<ValueId> {
    let mut refs = References::default();
    item.read(&mut refs);
    let mut pending = refs.0;
    let mut seen = BTreeSet::new();
    let mut values = Vec::new();
    while let Some(reference) = pending.pop() {
        if !state.step(1) || !state.memory(48) { break; }
        if !seen.insert(reference) { continue; }
        if let OperandRef::Value(value) = reference {
            values.push(value);
            if let Ok(data) = body.values.get(value) {
                let mut facts = References::default();
                data.facts.read(&mut facts);
                pending.extend(facts.0);
            }
        } else if let Ok(children) = cfg::expression_children(reference, body) {
            if !state.array(children.len(), 24) { break; }
            pending.extend(children);
        }
    }
    values
}

pub(super) fn check(module: &Module, selected: Option<FuncId>, state: &mut State<'_>) {
    for (function, data) in module.functions.iter() {
        if selected.is_some_and(|id| id != function) { continue; }
        let Some(body) = &data.body else { continue; };
        if !super::graph::eligible(function, body, state) { continue; }
        let Some(dom) = Dominators::new(body, state) else { continue; };
        let checker = Uses { function, body, dom: &dom };
        for (block_id, block) in body.blocks.iter() {
            let entry = Dominators::entry(block_id);
            for &value in &block.params {
                state.at(L::Value { function, value }, None);
                if let Ok(data) = body.values.get(value) {
                    checker.check(&data.facts, entry, None, true, false, state);
                }
            }
            if let Some(unwind) = &block.unwind {
                state.at(L::Block { function, block: block_id }, None);
                checker.check(unwind, entry, None, false, true, state);
            }
            for (position, &instruction) in block.insts.iter().enumerate() {
                let Ok(inst) = body.insts.get(instruction) else { continue; };
                state.at(L::Instruction { function, instruction }, inst.loc);
                checker.check(&inst.op, entry + 1, Some(position), false, false, state);
                checker.check(&inst.facts, entry + 1, Some(position), true, false, state);
                for &value in &inst.results {
                    state.at(L::Value { function, value }, inst.loc);
                    if let Ok(data) = body.values.get(value) {
                        checker.check(&data.facts, entry + 1, Some(position), true, false, state);
                    }
                }
            }
            state.at(L::Terminator { function, block: block_id }, block.term.loc);
            checker.check(&block.term, entry + 1, Some(block.insts.len()), false, false, state);
            for record in body.high_results.iter().filter(|record| record.block == block_id) {
                checker.check(record, entry + 1, Some(block.insts.len()), false, false, state);
                if let Some(prepared) = record.prepared
                    && let Some(&(source, position)) = state.positions.get(&(function, prepared)) {
                    let definition = Dominators::entry(source) + 1;
                    if !dom.dominates(definition, entry + 1, state)
                        || source == block_id && position >= block.insts.len() {
                        state.error(R::SSA2, "result preparation does not dominate its edge");
                    }
                }
            }
        }
    }
}

struct Uses<'a> {
    function: FuncId,
    body: &'a Body,
    dom: &'a Dominators,
}
impl Uses<'_> {
    fn check(
        &self,
        item: &impl Read,
        node: usize,
        position: Option<usize>,
        after: bool,
        unwind: bool,
        state: &mut State<'_>,
    ) {
        for value in leaves(item, self.body, state) {
            let Ok(data) = self.body.values.get(value) else { continue; };
            let (definition, index) = match data.def {
                Definition::BlockParam { block, .. } => (Dominators::entry(block), None),
                Definition::InstResult { inst, .. } => {
                    let Some(&(block, position)) = state.positions.get(&(self.function, inst)) else { continue; };
                    (Dominators::entry(block) + 1, Some(position))
                }
            };
            let ordered = definition != node || index.is_none_or(|index| {
                position.is_some_and(|position| index < position || (after && index == position))
            });
            if ordered && self.dom.dominates(definition, node, state) { continue; }
            let rule = if unwind || (ordered && self.unwind_bypass(definition, node, state)) {
                R::B61
            } else if definition / 2 == node / 2 {
                R::SSA1
            } else {
                R::SSA2
            };
            state.error(rule, format!("{value:?} is not visible at this use"));
        }
    }
    fn unwind_bypass(&self, definition: usize, node: usize, state: &mut State<'_>) -> bool {
        self.dom.edges.iter().any(|edge| {
            edge.entry && definition == Dominators::entry(edge.source) + 1
                && self.dom.reachable_avoiding(Dominators::entry(edge.target), node, definition, state)
        })
    }
}
