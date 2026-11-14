use super::{graph::Edge, high::inside, EntityLocation as L, RuleId as R, State};
use crate::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Defer registration is an execution fact, independent of SSA entry visibility.
/// Each scope has a finite prefix lattice: unreachable, a registered sequence,
/// or its common prefix after a conflicting merge (which is diagnosed).
pub(super) fn check(module: &Module, function: FuncId, body: &Body, edges: &[Edge], state: &mut State<'_>) {
    let mut registrations = BTreeMap::<ScopeId, Vec<InstId>>::new();
    for (id, inst) in body.insts.iter() {
        if let Operation::Lexical(LexicalOp::DeferRegister { scope, .. }) = inst.op {
            registrations.entry(scope).or_default().push(id);
        }
    }
    for (scope, ids) in registrations {
        if !state.array(ids.len(), 64) { return; }
        let width = ids.len().saturating_mul(8).saturating_add(64);
        if !state.array(body.blocks.len(), width) { return; }
        let mut flow = Prefixes {
            inputs: vec![None; body.blocks.len()], queue: VecDeque::new(),
            queued: vec![false; body.blocks.len()], conflicts: BTreeSet::new(), before: BTreeSet::new(),
        };
        if body.blocks.get(body.entry).is_ok_and(|b| inside(module, b.lexical_scope, scope, state)) {
            flow.join(body.entry, &[], state);
        }
        for edge in edges {
            let (Ok(source), Ok(target)) = (body.blocks.get(edge.source), body.blocks.get(edge.target)) else {
                continue;
            };
            if !inside(module, source.lexical_scope, scope, state)
                && inside(module, target.lexical_scope, scope, state) {
                flow.join(edge.target, &[], state);
            }
        }
        while let Some(block_id) = flow.queue.pop_front() {
            if !state.step(1) { return; }
            let index = block_id.as_u32() as usize;
            flow.queued[index] = false;
            let Some(mut prefix) = flow.inputs[index].clone() else { continue; };
            let Ok(block) = body.blocks.get(block_id) else { continue; };
            let unwind = block.unwind.as_ref().map(|unwind| unwind.target.block).filter(|target| {
                body.blocks.get(*target).is_ok_and(|b| inside(module, b.lexical_scope, scope, state))
            });
            for &id in &block.insts {
                if !state.step(prefix.len() + 1) { return; }
                let Ok(inst) = body.insts.get(id) else { continue; };
                if let Operation::Lexical(LexicalOp::DeferRegister { scope: registered, .. }) = inst.op
                    && registered == scope {
                    if prefix.contains(&id) {
                        flow.conflicts.insert(id);
                    } else {
                        for &prior in &prefix {
                            if !state.memory(32) { return; }
                            flow.before.insert((prior, id));
                        }
                        prefix.push(id);
                    }
                }
                if let Some(target) = unwind && ContractRegistry::new(module).construction_schema(&inst.op, body)
                    .is_ok_and(|c| !c.effects.failures.is_empty() || c.effects.suspends) {
                    flow.join(target, &prefix, state);
                }
            }
            if let Some(target) = unwind && matches!(block.term.kind,
                TermKind::Throw(_) | TermKind::Raise(_) | TermKind::Panic(_) | TermKind::Trap { .. }) {
                flow.join(target, &prefix, state);
            }
            for edge in edges.iter().filter(|edge| edge.source == block_id && !edge.entry) {
                if body.blocks.get(edge.target).is_ok_and(|b| inside(module, b.lexical_scope, scope, state)) {
                    flow.join(edge.target, &prefix, state);
                }
            }
        }
        // Registration points in one scope must be ordered on every path.
        for (i, &a) in ids.iter().enumerate() {
            for &b in ids.iter().skip(i + 1) {
                if !state.step(1) { return; }
                if !flow.before.contains(&(a, b)) && !flow.before.contains(&(b, a)) {
                    flow.conflicts.insert(b);
                }
            }
        }
        for instruction in flow.conflicts {
            let loc = body.insts.get(instruction).ok().and_then(|i| i.loc);
            state.at(L::Instruction { function, instruction }, loc);
            state.error(R::H3, "defer registrations are unordered or a merge observes different registered prefixes");
        }
    }
}
struct Prefixes {
    inputs: Vec<Option<Vec<InstId>>>,
    queue: VecDeque<BlockId>,
    queued: Vec<bool>,
    conflicts: BTreeSet<InstId>,
    before: BTreeSet<(InstId, InstId)>,
}
impl Prefixes {
    fn join(&mut self, block: BlockId, prefix: &[InstId], state: &mut State<'_>) {
        let index = block.as_u32() as usize;
        if !state.step(prefix.len() + 1) { return; }
        let changed = match &mut self.inputs[index] {
            None => {
                self.inputs[index] = Some(prefix.to_vec());
                true
            }
            Some(old) => {
                let common = old.iter().zip(prefix).take_while(|(a, b)| a == b).count();
                if (common != old.len() || common != prefix.len())
                    && let Some(id) = old.get(common).or_else(|| prefix.get(common)) {
                    self.conflicts.insert(*id);
                }
                let changed = common != old.len();
                old.truncate(common);
                changed
            }
        };
        if changed && !self.queued[index] {
            self.queued[index] = true;
            self.queue.push_back(block);
        }
    }
}
