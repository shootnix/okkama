use super::{EntityLocation as L, State};
use crate::*;

#[derive(Clone, Debug)]
pub(super) struct Edge {
    pub source: BlockId,
    pub target: BlockId,
    pub entry: bool,
    pub args: Vec<EdgeArg>,
}

pub(super) fn edges(body: &Body, state: &mut State<'_>) -> Vec<Edge> {
    let mut edges = Vec::new();
    for (source, block) in body.blocks.iter() {
        let mut add = |target, args: Vec<EdgeArg>, entry| {
            if state.array(args.len(), 16) && state.memory(64) {
                edges.push(Edge { source, target, entry, args });
            }
        };
        if let Some(unwind) = &block.unwind {
            add(unwind.target.block, unwind.target.args.iter().copied().map(EdgeArg::Value).collect(), true);
        }
        let mut target = |target: &Target| {
            add(target.block, target.args.iter().copied().map(EdgeArg::Value).collect(), false);
        };
        match &block.term.kind {
            TermKind::Br(t) => target(t),
            TermKind::Cond { yes, no, .. } => {
                target(yes);
                target(no);
            }
            TermKind::Switch { cases, default, .. } => {
                for case in cases { target(&case.target); }
                target(default);
            }
            TermKind::InvokeOwn { normal, unwind, .. } => {
                add(normal.block, normal.args.clone(), false);
                add(unwind.block, unwind.args.clone(), false);
            }
            TermKind::InvokeDefer { continue_edge, replacements, unwind, .. } => {
                add(continue_edge.block, continue_edge.args.clone(), false);
                for replacement in replacements {
                    add(replacement.edge.block, replacement.edge.args.clone(), false);
                }
                if let Some(unwind) = unwind {
                    add(unwind.block, unwind.args.clone(), false);
                }
            }
            TermKind::Catch { handler, next, .. } => {
                target(next);
                add(handler.block, handler.args.clone(), false);
            }
            TermKind::Lookup { found, missing, .. } => {
                target(missing);
                add(found.block, found.args.clone(), false);
            }
            _ => {}
        }
    }
    edges
}

pub(super) fn eligible(function: FuncId, body: &Body, state: &mut State<'_>) -> bool {
    if state.incomplete || state.poisoned.iter().any(|location| match location {
        L::Function(id) => *id == function,
        L::Block { function: id, .. } | L::Instruction { function: id, .. }
        | L::Terminator { function: id, .. } | L::Value { function: id, .. }
        | L::Edge { function: id, .. } => *id == function,
        _ => false,
    }) {
        return false;
    }
    super::index::intact(state.module, body, body, state)
}

/// Immediate dominators on Entry(B) -> Body(B), with ordinary unwind from Entry.
/// CHK uses linear storage and also handles loops and irreducible graphs.
pub(super) struct Dominators {
    pub edges: Vec<Edge>,
    enter: Vec<usize>,
    leave: Vec<usize>,
    successors: Vec<Vec<usize>>,
}
impl Dominators {
    pub fn new(body: &Body, state: &mut State<'_>) -> Option<Self> {
        let n = body.blocks.len().checked_mul(2)?;
        if !state.array(n, 112) { return None; }
        let edges = edges(body, state);
        let mut successors = vec![Vec::new(); n];
        let mut predecessors = vec![Vec::new(); n];
        for (id, _) in body.blocks.iter() {
            let entry = Self::entry(id);
            successors[entry].push(entry + 1);
            predecessors[entry + 1].push(entry);
        }
        for edge in &edges {
            if !state.step(1) || !state.memory(32) { return None; }
            let from = Self::entry(edge.source) + usize::from(!edge.entry);
            let to = Self::entry(edge.target);
            successors.get_mut(from)?.push(to);
            predecessors.get_mut(to)?.push(from);
        }
        let root = Self::entry(body.entry);
        let mut seen = vec![false; n];
        let mut postorder = Vec::new();
        let mut stack = vec![(root, false)];
        while let Some((node, leaving)) = stack.pop() {
            if !state.step(1) { return None; }
            if leaving {
                postorder.push(node);
            } else if !seen[node] {
                seen[node] = true;
                stack.push((node, true));
                stack.extend(successors[node].iter().rev().map(|node| (*node, false)));
            }
        }
        postorder.reverse();
        let mut rank = vec![usize::MAX; n];
        for (i, &node) in postorder.iter().enumerate() { rank[node] = i; }
        let mut idom = vec![usize::MAX; n];
        idom[root] = root;
        let mut changed = true;
        while changed {
            changed = false;
            for &node in postorder.iter().skip(1) {
                let mut incoming = predecessors[node].iter().copied().filter(|p| idom[*p] != usize::MAX);
                let Some(mut parent) = incoming.next() else { continue; };
                for mut other in incoming {
                    while parent != other {
                        if !state.step(1) { return None; }
                        if rank[parent] > rank[other] { parent = idom[parent]; }
                        else { other = idom[other]; }
                    }
                }
                if !state.step(1) { return None; }
                if idom[node] != parent {
                    idom[node] = parent;
                    changed = true;
                }
            }
        }
        if !state.array(n, 48) { return None; }
        let mut children = vec![Vec::new(); n];
        for &node in postorder.iter().skip(1) { children[idom[node]].push(node); }
        let mut enter = vec![usize::MAX; n];
        let mut leave = vec![usize::MAX; n];
        let mut clock = 0;
        let mut pending = vec![(root, false)];
        while let Some((node, leaving)) = pending.pop() {
            if !state.step(1) { return None; }
            if leaving {
                leave[node] = clock;
            } else {
                enter[node] = clock;
                pending.push((node, true));
                pending.extend(children[node].iter().rev().map(|node| (*node, false)));
            }
            clock += 1;
        }
        Some(Self { edges, enter, leave, successors })
    }
    pub fn reachable_avoiding(&self, source: usize, target: usize, avoid: usize, state: &mut State<'_>) -> bool {
        if !state.array(self.successors.len(), 16) { return false; }
        let mut seen = vec![false; self.successors.len()];
        let mut pending = vec![source];
        seen[source] = true;
        while let Some(node) = pending.pop() {
            if !state.step(1) { return false; }
            if node == avoid { continue; }
            if node == target { return true; }
            for &next in &self.successors[node] {
                if !state.step(1) { return false; }
                if !seen[next] {
                    seen[next] = true;
                    pending.push(next);
                }
            }
        }
        false
    }
    pub fn entry(block: BlockId) -> usize { block.as_u32() as usize * 2 }
    pub fn dominates(&self, a: usize, b: usize, state: &mut State<'_>) -> bool {
        if !state.step(1) { return false; }
        if self.enter[a] == usize::MAX || self.enter[b] == usize::MAX { return a == b; }
        self.enter[a] <= self.enter[b] && self.leave[b] <= self.leave[a]
    }
}
