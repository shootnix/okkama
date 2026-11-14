//! Additional producer restrictions for failure dispatch and lexical catch bindings.
use std::collections::{HashMap, HashSet};

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

/// Keep producer restrictions that go beyond the independent verifier's High checks.
/// In particular, catch selection follows a Try exit, and ordinary control flow
/// cannot enter dispatch. Check failure routes even on blocks with D11 records.
pub(super) fn check(module: &ir::Module, body: &ir::Body) -> Result<()> {
    let entry = body.blocks.get(body.entry)?;
    if matches!(entry.term.kind, ir::TermKind::Catch { .. }) {
        return Err(invalid("H5: function entry cannot be catch dispatch without unwind"));
    }
    let root = entry.lexical_scope;
    let registry = ir::ContractRegistry::new(module);
    let mut seen = HashSet::new();
    let mut pending = vec![body.entry];
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let block = body.blocks.get(id)?;
        for &instruction in &block.insts {
            let inst = body.insts.get(instruction)?;
            let site = ir::HighOperationSite::Instruction(instruction);
            let contract = registry.high_site_schema(&inst.op, body, site).map_err(ir::BuildError::from)?;
            if !contract.effects.failures.is_empty() || contract.effects.suspends {
                let unwind = block.unwind.as_ref().ok_or_else(|| invalid("H5: missing high unwind"))?;
                pending.push(unwind.target.block);
            }
        }
        let failing = matches!(block.term.kind, ir::TermKind::Raise(_) | ir::TermKind::Throw(_)
            | ir::TermKind::Panic(_) | ir::TermKind::Trap { .. });
        if failing && block.lexical_scope != root {
            let unwind = block.unwind.as_ref().ok_or_else(|| invalid("H5: missing nested failure unwind"))?;
            pending.push(unwind.target.block);
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
            pending.push(target);
        }
    }
    check_catch_values(module, body)
}

/// The producer gives every catch payload a binding local to its handler.
/// SSA dominance itself is checked by verify_high.
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
    Ok(())
}
