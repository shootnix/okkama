//! Producer normalization and checks in addition to independent High verification.
use std::collections::HashSet;

use super::{LowerError, Result};
use crate::oir2 as ir;
use ir::visit::Rewrite;

fn invalid(message: impl Into<String>) -> LowerError {
    LowerError::invalid(crate::source::Span::default(), message)
}

fn reachable(module: &ir::Module, body: &ir::Body) -> Result<HashSet<ir::BlockId>> {
    let mut seen = HashSet::new();
    let mut pending = vec![body.entry];
    while let Some(block) = pending.pop() {
        if seen.insert(block) {
            let next = ir::cfg::successors(block, body, ir::ContractRegistry::new(module))
                .map_err(ir::BuildError::from)?;
            pending.extend(next);
        }
    }
    Ok(seen)
}

/// Dead joins/steps are placeholders, never executable continuations. Remove
/// their unused parameters and unwind edges without leaving orphan SSA values.
pub(super) fn finish(module: &mut ir::Module) -> Result<()> {
    let mut functions = vec![];
    for (id, function) in module.functions.iter() {
        if let Some(body) = &function.body {
            functions.push((id, reachable(module, body)?));
        }
    }
    for (id, live) in functions {
        let body = module.functions.get_mut(id)?.body.as_mut().expect("source body");
        let mut removed = HashSet::new();
        for (block, data) in body.blocks.iter_mut() {
            if !live.contains(&block) {
                if !data.insts.is_empty() || data.term.kind != ir::TermKind::Unreachable {
                    return Err(invalid(format!("non-empty unreachable block {block:?}")));
                }
                removed.extend(data.params.drain(..));
                data.unwind = None;
            }
        }
        if !removed.is_empty() {
            let mut values = ir::Arena::new();
            let mut remap = ir::IdRemap::default();
            for (old, data) in body.values.iter() {
                if !removed.contains(&old) {
                    remap.values.insert(old, values.alloc(data.clone())?);
                }
            }
            body.values = values;
            body.rewrite(&mut remap);
        }
    }
    Ok(())
}

/// Semantic structure, function forms and SSA are checked by verify_high.
/// Retain the producer's additional construction and catch restrictions.
pub(super) fn check(module: &ir::Module) -> Result<()> {
    for (_, function) in module.functions.iter() {
        if let Some(body) = &function.body {
            super::construction::check(module, body)?;
            super::high::check(module, body)?;
        }
    }
    Ok(())
}
