//! Producer assertions for lexical scopes, lazy captures and reached defer prefixes.
//! The independent semantic verifier lives on #564; these checks do not replace it.
use std::collections::{HashMap, HashSet, VecDeque};

use super::{LowerError, Result};
use crate::oir2 as ir;

fn invalid(message: impl Into<String>) -> LowerError {
    LowerError::invalid(crate::source::Span::default(), message)
}

fn ancestors(module: &ir::Module, mut scope: ir::ScopeId) -> Result<Vec<ir::ScopeId>> {
    let mut path = vec![];
    loop {
        if path.contains(&scope) {
            return Err(invalid("cyclic lexical scope tree"));
        }
        path.push(scope);
        match module.scopes.get(scope)?.parent {
            Some(parent) => scope = parent,
            None => return Ok(path),
        }
    }
}

type Prefix = HashMap<ir::ScopeId, Vec<ir::InstId>>;

struct Flow<'a> {
    module: &'a ir::Module,
    body: &'a ir::Body,
    inputs: HashMap<ir::BlockId, Prefix>,
    pending: VecDeque<ir::BlockId>,
    entries: HashMap<ir::ScopeId, ir::BlockId>,
    ordered: HashSet<(ir::InstId, ir::InstId)>,
}

impl Flow<'_> {
    fn edge(&mut self, source: Option<ir::BlockId>, target: ir::BlockId, prefix: &Prefix) -> Result<()> {
        let target_path = ancestors(self.module, self.body.blocks.get(target)?.lexical_scope)?;
        let source_path = source.map(|id| {
            ancestors(self.module, self.body.blocks.get(id)?.lexical_scope)
        }).transpose()?.unwrap_or_default();
        for &scope in &target_path {
            if !source_path.contains(&scope)
                && self.entries.insert(scope, target).is_some_and(|previous| previous != target) {
                return Err(invalid("H1: lexical scope has multiple entries"));
            }
        }
        let mut incoming = prefix.clone();
        incoming.retain(|scope, _| target_path.contains(scope));
        match self.inputs.get(&target) {
            Some(previous) if *previous != incoming => {
                return Err(invalid("H3: merge observes conflicting defer prefixes"));
            }
            Some(_) => {}
            None => {
                self.inputs.insert(target, incoming);
                self.pending.push_back(target);
            }
        }
        Ok(())
    }
}

pub(super) fn check(module: &ir::Module, body: &ir::Body) -> Result<()> {
    let entry = body.blocks.get(body.entry)?;
    if matches!(entry.term.kind, ir::TermKind::Catch { .. }) {
        return Err(invalid("H5: function entry cannot be catch dispatch without unwind"));
    }
    let root = entry.lexical_scope;
    let declaration = module.scopes.get(root)?;
    if declaration.kind != ir::ScopeKind::Action || declaration.parent.is_some() {
        return Err(invalid("H1: function entry requires a root Action scope"));
    }
    let registry = ir::ContractRegistry::new(module);
    let mut flow = Flow {
        module, body, inputs: HashMap::new(), pending: VecDeque::new(),
        entries: HashMap::new(), ordered: HashSet::new(),
    };
    flow.edge(None, body.entry, &Prefix::new())?;
    while let Some(id) = flow.pending.pop_front() {
        let block = body.blocks.get(id)?;
        let mut prefix = flow.inputs[&id].clone();
        for &instruction in &block.insts {
            let inst = body.insts.get(instruction)?;
            if let ir::Operation::Lexical(ir::LexicalOp::DeferRegister { scope, action }) = &inst.op {
                if *scope != block.lexical_scope || !inst.results.is_empty() {
                    return Err(invalid("H3: defer registration must be a statement in its own scope"));
                }
                let reached = prefix.entry(*scope).or_default();
                if reached.contains(&instruction) {
                    return Err(invalid("H3: repeated defer registration in one scope instance"));
                }
                for &prior in reached.iter() {
                    flow.ordered.insert((prior, instruction));
                }
                reached.push(instruction);
                let mut types = action.captures.iter().map(|&value| {
                    body.values.get(value).map(|value| value.ty)
                }).collect::<std::result::Result<Vec<_>, _>>()?;
                for &binding in &action.bindings {
                    types.push(body.bindings.get(binding)?.ty);
                }
                let signature = &module.functions.get(action.function)?.signature;
                if signature.params != types || !signature.results.is_empty() {
                    return Err(invalid("defer signature must accept snapshots then latest binding values"));
                }
            }
            if let ir::Operation::Lexical(ir::LexicalOp::Temp { scope, .. }) = &inst.op
                && *scope != block.lexical_scope {
                return Err(invalid("H8: temporary must stay in its lexical scope"));
            }
            // Calls without inferred summaries retain a conservative high failure envelope.
            let may_fail = if matches!(inst.op, ir::Operation::Call(_)) {
                true
            } else {
                let contract = registry.lookup(&inst.op).map_err(ir::BuildError::from)?;
                !contract.effects.failures.is_empty() || contract.effects.suspends
            };
            if may_fail {
                let unwind = block.unwind.as_ref().ok_or_else(|| invalid("H5: missing high unwind"))?;
                flow.edge(Some(id), unwind.target.block, &prefix)?;
            }
        }
        let failing = matches!(block.term.kind, ir::TermKind::Raise(_) | ir::TermKind::Throw(_)
            | ir::TermKind::Panic(_) | ir::TermKind::Trap { .. });
        if failing && block.lexical_scope != root {
            let unwind = block.unwind.as_ref().ok_or_else(|| invalid("H5: missing nested failure unwind"))?;
            flow.edge(Some(id), unwind.target.block, &prefix)?;
        }
        if let Some(unwind) = &block.unwind {
            if !unwind.target.args.iter().all(|arg| block.params.contains(arg)) {
                return Err(invalid("H5: unwind argument is not an entry parameter"));
            }
            let destination = body.blocks.get(unwind.target.block)?;
            if matches!(destination.term.kind, ir::TermKind::Catch { .. }) {
                let source_path = ancestors(module, block.lexical_scope)?;
                let exited_try = source_path.iter().take_while(|&&scope| scope != destination.lexical_scope)
                    .any(|&scope| module.scopes.get(scope).is_ok_and(|scope| scope.kind == ir::ScopeKind::Try));
                if !source_path.contains(&destination.lexical_scope) || !exited_try {
                    return Err(invalid("H5: catch selection must follow exit from the Try scope"));
                }
            }
            if !matches!(destination.term.kind, ir::TermKind::Raise(_) | ir::TermKind::Catch { .. }) {
                return Err(invalid("H5: unwind does not lead to failure dispatch"));
            }
        }
        let mut targets = vec![];
        ir::cfg::walk_terminator(id, &block.term, body, registry, &mut |event| {
            if let ir::cfg::CfgEvent::Edge { target, kind, .. } = event {
                targets.push((target, kind));
            }
        }).map_err(ir::BuildError::from)?;
        for (target, kind) in targets {
            if matches!(body.blocks.get(target)?.term.kind, ir::TermKind::Catch { .. })
                && kind != ir::cfg::EdgeKind::CatchNext {
                return Err(invalid("H5: catch dispatch is reachable without unwind"));
            }
            flow.edge(Some(id), target, &prefix)?;
        }
    }
    let registrations: Vec<_> = body.insts.iter().filter_map(|(id, inst)| {
        match inst.op {
            ir::Operation::Lexical(ir::LexicalOp::DeferRegister { scope, .. }) => Some((scope, id)),
            _ => None,
        }
    }).collect();
    for (index, &(scope, first)) in registrations.iter().enumerate() {
        for &(other_scope, second) in &registrations[index + 1..] {
            if scope == other_scope && !flow.ordered.contains(&(first, second))
                && !flow.ordered.contains(&(second, first)) {
                return Err(invalid("H3: unordered defer registrations in one scope"));
            }
        }
    }
    check_bindings(module, body)?;
    check_catch_values(module, body)
}

fn check_bindings(module: &ir::Module, body: &ir::Body) -> Result<()> {
    let registry = ir::ContractRegistry::new(module);
    let mut predecessors = HashMap::<ir::BlockId, Vec<ir::BlockId>>::new();
    let mut live = HashSet::new();
    let mut pending = vec![body.entry];
    while let Some(source) = pending.pop() {
        if !live.insert(source) {
            continue;
        }
        for target in ir::cfg::successors(source, body, registry).map_err(ir::BuildError::from)? {
            predecessors.entry(target).or_default().push(source);
            pending.push(target);
        }
    }
    let mut dominators: HashMap<_, _> = live.iter().map(|&id| (id, live.clone())).collect();
    dominators.insert(body.entry, HashSet::from([body.entry]));
    let mut changed = true;
    while changed {
        changed = false;
        for &id in &live {
            if id == body.entry {
                continue;
            }
            let mut next = live.clone();
            for predecessor in &predecessors[&id] {
                next.retain(|block| dominators[predecessor].contains(block));
            }
            next.insert(id);
            if next != dominators[&id] {
                dominators.insert(id, next);
                changed = true;
            }
        }
    }
    let mut definitions = HashMap::new();
    for (block, data) in body.blocks.iter() {
        for (index, &id) in data.insts.iter().enumerate() {
            if let ir::Operation::Lexical(ir::LexicalOp::Bind { binding, .. }) = body.insts.get(id)?.op {
                if definitions.insert(binding, (block, index)).is_some() {
                    return Err(invalid("H7: multiple initial binding definitions"));
                }
                if !ancestors(module, data.lexical_scope)?.contains(&body.bindings.get(binding)?.scope) {
                    return Err(invalid("H7: initial bind outside its lexical scope"));
                }
            }
        }
    }
    for &block in &live {
        let data = body.blocks.get(block)?;
        for (index, &id) in data.insts.iter().enumerate() {
            let required = match &body.insts.get(id)?.op {
                ir::Operation::Lexical(ir::LexicalOp::Rebind { binding, target, .. }) => {
                    if *target != body.bindings.get(*binding)?.scope
                        || !ancestors(module, data.lexical_scope)?.contains(target) {
                        return Err(invalid("H8: rebind must target the original binding scope"));
                    }
                    std::slice::from_ref(binding)
                }
                ir::Operation::Lexical(ir::LexicalOp::DeferRegister { action, .. }) => &action.bindings,
                _ => continue,
            };
            for binding in required {
                let Some(&(definition, position)) = definitions.get(binding) else {
                    return Err(invalid("H7: missing initial binding definition"));
                };
                if !dominators[&block].contains(&definition) || definition == block && position >= index {
                    return Err(invalid("H7: initial bind does not dominate the binding use"));
                }
            }
        }
    }
    Ok(())
}

/// Catch payloads are edge-produced SSA values, available only below the handler.
fn check_catch_values(module: &ir::Module, body: &ir::Body) -> Result<()> {
    let mut payloads = HashSet::new();
    let registry = ir::ContractRegistry::new(module);
    ir::cfg::walk_body(body, registry, |event| {
        if let ir::cfg::CfgEvent::Edge { source, target, kind } = event
            && kind != ir::cfg::EdgeKind::BlockUnwind {
            let destination = body.blocks.get(target).expect("allocated catch target");
            let scope = destination.lexical_scope;
            if module.scopes.get(scope).expect("allocated scope").kind == ir::ScopeKind::Catch
                && ancestors(module, body.blocks.get(source).expect("allocated source").lexical_scope)
                    .is_ok_and(|path| !path.contains(&scope))
                && let Some(&value) = destination.params.last() {
                payloads.insert(value);
            }
        }
    }).map_err(ir::BuildError::from)?;
    let mut caught = HashMap::new();
    for (_, data) in body.blocks.iter() {
        for &id in &data.insts {
            if let ir::Operation::Lexical(ir::LexicalOp::Bind { binding, value }) = body.insts.get(id)?.op
                && payloads.contains(&value) {
                let ir::Definition::BlockParam { block: entry, .. } = body.values.get(value)?.def else {
                    return Err(invalid("catch binding must bind the handler payload parameter"));
                };
                if body.blocks.get(entry)?.lexical_scope != body.bindings.get(binding)?.scope
                    || data.lexical_scope != body.bindings.get(binding)?.scope {
                    return Err(invalid("catch binding must be local to its Catch scope"));
                }
                caught.insert(value, entry);
            }
        }
    }
    if caught.len() != payloads.len() {
        return Err(invalid("catch payload requires a lexical binding"));
    }
    if caught.is_empty() {
        return Ok(());
    }
    let registry = ir::ContractRegistry::new(module);
    // Removing the definition block must make every payload use unreachable.
    for (&value, &definition) in &caught {
        let mut bypass = HashSet::new();
        let mut pending = vec![body.entry];
        while let Some(block) = pending.pop() {
            if block == definition || !bypass.insert(block) {
                continue;
            }
            pending.extend(ir::cfg::successors(block, body, registry).map_err(ir::BuildError::from)?);
        }
        let mut violation = false;
        ir::cfg::walk_body(body, registry, |event| {
            let ir::cfg::CfgEvent::Use { site, operand } = event else {
                return;
            };
            if operand.value != value {
                return;
            }
            let block = match site {
                ir::cfg::UseSite::Instruction(id) => body.blocks.iter().find_map(|(block, data)| {
                    data.insts.contains(&id).then_some(block)
                }),
                ir::cfg::UseSite::Terminator(block) | ir::cfg::UseSite::BlockUnwind(block) => Some(block),
                _ => None,
            };
            violation |= block.is_some_and(|block| bypass.contains(&block));
        }).map_err(ir::BuildError::from)?;
        if violation {
            return Err(invalid("catch binding payload does not dominate its use"));
        }
    }
    Ok(())
}
