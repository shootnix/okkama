use super::{EntityLocation as L, RuleId as R, State};
use crate::*;
use std::collections::{BTreeSet, VecDeque};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Origin { Instruction(InstId), Invoke(BlockId) }
struct Creation<'a> {
    origin: Origin,
    initial: &'a [ValueId],
    status: ConstructionStatus,
    layout: &'a [Field],
    pending_layout: bool,
    aliases: BTreeSet<ValueId>,
}
fn undef(body: &Body, value: ValueId) -> bool {
    body.values.get(value).is_ok_and(|v| match v.def {
        Definition::InstResult { inst, .. } => body.insts.get(inst)
            .is_ok_and(|i| matches!(i.op, Operation::Core(CoreOp::Undef { .. }))),
        _ => false,
    })
}
pub(super) fn check(module: &Module, function: FuncId, body: &Body, state: &mut State<'_>) {
    let edges = super::graph::edges(body, state);
    let instructions = body.insts.iter().filter_map(|(id, inst)| {
        inst.results.first().map(|value| (Origin::Instruction(id), &inst.op, vec![*value]))
    });
    let invokes = body.blocks.iter().filter_map(|(id, block)| {
        let TermKind::InvokeOwn { op, normal, .. } = &block.term.kind else { return None; };
        let target = body.blocks.get(normal.block).ok()?;
        let aliases = normal.args.iter().zip(&target.params).filter_map(|(arg, param)| {
            matches!(arg, EdgeArg::Produced(EdgeResult { outcome: Outcome::Normal, slot: 0 })).then_some(*param)
        }).collect();
        Some((Origin::Invoke(id), op.as_ref(), aliases))
    });
    let mut seeds = BTreeSet::new();
    for (origin, op, origin_values) in instructions.chain(invokes) {
        let Operation::Core(CoreOp::ObjNew { declaration, fields, construction, .. }) = op else { continue; };
        let Ok(decl) = module.declarations.get(*declaration) else { continue; };
        let DeclarationKind::Object { fields: layout, .. } = &decl.kind else { continue; };
        seeds.extend(&origin_values);
        let pending_layout = layout.last().is_some_and(|field| field.name == "$pending"
            && module.types.get(field.ty) == Ok(&Type::Bool));
        let mut creation = Creation {
            origin, initial: fields, status: *construction, layout, pending_layout,
            aliases: origin_values.into_iter().collect(),
        };
        let mut changed = true;
        while changed {
            changed = false;
            for edge in &edges {
                let Ok(target) = body.blocks.get(edge.target) else { continue; };
                for (arg, param) in edge.args.iter().zip(&target.params) {
                    if !state.step(1) || !state.memory(32) { return; }
                    if let EdgeArg::Value(value) = arg
                        && creation.aliases.contains(value) && creation.aliases.insert(*param) {
                        changed = true;
                    }
                }
            }
        }
        analyze(module, function, body, &edges, &creation, state);
    }
    let valid_self = definite_self(body, &edges, seeds, state);
    for (instruction, inst) in body.insts.iter() {
        state.at(L::Instruction { function, instruction }, inst.loc);
        valid_init(&inst.op, &valid_self, state);
    }
    for (block_id, block) in body.blocks.iter() {
        if let TermKind::InvokeOwn { op, .. } = &block.term.kind {
            state.at(L::Terminator { function, block: block_id }, block.term.loc);
            valid_init(op, &valid_self, state);
        }
    }
}
fn valid_init(op: &Operation, valid: &BTreeSet<ValueId>, state: &mut State<'_>) {
    if let Operation::Core(CoreOp::ObjInit { object, .. }) = op
        && !valid.contains(object) {
        state.error(R::A2, "obj.init requires an early self created by this constructor");
    }
}
fn analyze(
    module: &Module,
    function: FuncId,
    body: &Body,
    edges: &[super::graph::Edge],
    c: &Creation<'_>,
    state: &mut State<'_>,
) {
    let width = c.layout.len() + 1;
    if !state.array(body.blocks.len(), width.saturating_mul(4).saturating_add(64)) { return; }
    // 0 = this origin absent, 1 = uninitialized/incomplete, 2 = initialized/completed.
    // Union ignores absent alternatives; each SSA phi is also checked for definite local origins.
    // The final cell is the construction state. Creation resets each incarnation.
    let mut inputs: Vec<Option<Vec<u8>>> = vec![None; body.blocks.len()];
    inputs[body.entry.as_u32() as usize] = Some(vec![0; width]);
    let mut queue = VecDeque::from([body.entry]);
    let mut queued = vec![false; body.blocks.len()];
    queued[body.entry.as_u32() as usize] = true;
    while let Some(block_id) = queue.pop_front() {
        if !state.step(1) { return; }
        queued[block_id.as_u32() as usize] = false;
        let Some(input) = inputs[block_id.as_u32() as usize].clone() else { continue; };
        let Ok(block) = body.blocks.get(block_id) else { continue; };
        let mut output = input.clone();
        let mut exceptional: Option<Vec<u8>> = None;
        for &id in &block.insts {
            if !state.step(width) { return; }
            if let Ok(inst) = body.insts.get(id) {
                if ContractRegistry::new(module).construction_schema(&inst.op, body)
                    .is_ok_and(|contract| !contract.effects.failures.is_empty() || contract.effects.suspends) {
                    join_failure(&mut exceptional, &output);
                }
                transfer(body, &inst.op, Origin::Instruction(id), c, &mut output, None);
            }
        }
        if matches!(block.term.kind, TermKind::Throw(_) | TermKind::Raise(_) | TermKind::Panic(_)
            | TermKind::Trap { .. }) {
            join_failure(&mut exceptional, &output);
        }
        for edge in edges.iter().filter(|edge| edge.source == block_id) {
            if !state.step(width) { return; }
            let mut next = if edge.entry {
                let Some(exceptional) = &exceptional else { continue; };
                exceptional.clone()
            } else {
                output.clone()
            };
            if let TermKind::InvokeOwn { op, normal, .. } = &block.term.kind
                && !edge.entry && edge.target == normal.block && edge.args == normal.args {
                transfer(body, op, Origin::Invoke(block_id), c, &mut next, None);
            }
            let slot = &mut inputs[edge.target.as_u32() as usize];
            let changed = match slot {
                None => {
                    *slot = Some(next);
                    true
                }
                Some(old) => {
                    let mut changed = false;
                    for (old, new) in old.iter_mut().zip(next) {
                        let merged = *old | new;
                        changed |= merged != *old;
                        *old = merged;
                    }
                    changed
                }
            };
            let target = edge.target.as_u32() as usize;
            if changed && !queued[target] {
                queued[target] = true;
                queue.push_back(edge.target);
            }
        }
    }
    // Diagnose only after reaching the fixpoint.
    for (block_id, block) in body.blocks.iter() {
        let Some(mut values) = inputs[block_id.as_u32() as usize].clone() else { continue; };
        for &id in &block.insts {
            if !state.step(width) { return; }
            if let Ok(inst) = body.insts.get(id) {
                state.at(L::Instruction { function, instruction: id }, inst.loc);
                transfer(body, &inst.op, Origin::Instruction(id), c, &mut values, Some(state));
            }
        }
        state.at(L::Terminator { function, block: block_id }, block.term.loc);
        if let TermKind::InvokeOwn { op, .. } = &block.term.kind {
            transfer(body, op, Origin::Invoke(block_id), c, &mut values, Some(state));
        }
        if let TermKind::Ret { values: returned } = &block.term.kind
            && returned.iter().any(|v| c.aliases.contains(v))
            && values[c.layout.len()] != 0 && values[c.layout.len()] != 2 {
            state.error(R::A2, "constructor returns an identity before completion");
        }
    }
}
fn transfer(
    body: &Body,
    op: &Operation,
    origin: Origin,
    c: &Creation<'_>,
    values: &mut [u8],
    mut state: Option<&mut State<'_>>,
) {
    if origin == c.origin {
        let incomplete = c.initial.iter().any(|value| undef(body, *value));
        let pending_unbound = c.initial.last().is_some_and(|value| undef(body, *value));
        let pending_false = c.initial.last().is_some_and(|value| is_false(body, *value));
        let invalid = match c.status {
            ConstructionStatus::Complete => incomplete || (c.pending_layout && !pending_false),
            ConstructionStatus::Incomplete => !c.pending_layout || !pending_unbound,
        };
        if invalid && let Some(state) = state.as_mut() {
            state.error(R::A2, "construction status does not match complete fields or the early-self layout");
        }
        for (slot, value) in values.iter_mut().zip(c.initial) {
            *slot = if undef(body, *value) { 1 } else { 2 };
        }
        values[c.layout.len()] = if c.status == ConstructionStatus::Complete { 2 } else { 1 };
    }
    if values[c.layout.len()] == 0 { return; }
    if let Operation::Core(CoreOp::ObjGet { object, field } | CoreOp::OnceLoad { object, field }) = op
        && c.aliases.contains(object) && values.get(field.index as usize).is_some_and(|v| *v != 2)
        && let Some(state) = state.as_mut() {
        state.error(R::A2, "constructor reads an uninitialized field of its early self");
    }
    let Operation::Core(CoreOp::ObjInit { object, field, value, completion }) = op else { return; };
    if !c.aliases.contains(object) { return; }
    let index = field.index as usize;
    let Some(slot) = values.get(index).copied() else { return; };
    let is_pending = c.layout.get(index).is_some_and(|field| field.name == "$pending");
    let invalid = values[c.layout.len()] != 1 || slot != 1
        || (completion.is_some() && !is_pending)
        || (is_pending && (values[..c.layout.len()].iter().enumerate().any(|(i, v)| i != index && *v != 2)
            || !is_false(body, *value)));
    if invalid && let Some(state) = state.as_mut() {
        state.error(R::A2, "field already initialized, self complete, or completion precedes field initialization");
    }
    values[index] = 2;
    if is_pending { values[c.layout.len()] = 2; }
}
pub(super) fn is_false(body: &Body, value: ValueId) -> bool {
    body.values.get(value).is_ok_and(|v| match v.def {
        Definition::InstResult { inst, .. } => body.insts.get(inst)
            .is_ok_and(|i| i.op == Operation::Const(Literal::Bool(false))),
        _ => false,
    })
}

fn definite_self(
    body: &Body,
    edges: &[super::graph::Edge],
    seeds: BTreeSet<ValueId>,
    state: &mut State<'_>,
) -> BTreeSet<ValueId> {
    if !state.array(body.values.len(), 64) { return seeds; }
    let mut valid = seeds.clone();
    // Greatest fixpoint keeps loop-carried self, but removes any phi with an
    // incoming value that is not a constructor-local identity.
    for (block, data) in body.blocks.iter() {
        if block != body.entry { valid.extend(&data.params); }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for edge in edges {
            let Ok(target) = body.blocks.get(edge.target) else { continue; };
            for (arg, param) in edge.args.iter().zip(&target.params) {
                if !state.step(1) { return valid; }
                let local = match arg {
                    EdgeArg::Value(value) => valid.contains(value),
                    EdgeArg::Produced(result) => result.outcome == Outcome::Normal && result.slot == 0
                        && body.blocks.get(edge.source).is_ok_and(|block| match &block.term.kind {
                            TermKind::InvokeOwn { op, normal, .. } => normal.block == edge.target
                                && normal.args == edge.args && matches!(**op, Operation::Core(CoreOp::ObjNew { .. })),
                            _ => false,
                        }),
                };
                if !local && valid.remove(param) { changed = true; }
            }
        }
    }
    valid
}

fn join_failure(target: &mut Option<Vec<u8>>, values: &[u8]) {
    match target {
        None => *target = Some(values.to_vec()),
        Some(old) => {
            for (old, new) in old.iter_mut().zip(values) { *old |= new; }
        }
    }
}
