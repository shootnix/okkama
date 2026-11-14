use super::{RuleId, State};
use crate::*;

/// Three-colour DFS, visiting a shared metadata node once. Unlike CFG use
/// enumeration this check needs no expansion of all leaf occurrences.
pub(super) fn check(body: &Body, state: &mut State<'_>) {
    let Some(dep_end) = body.keys.len().checked_add(body.deps.len()) else {
        state.incomplete = true;
        return;
    };
    let Some(len) = dep_end.checked_add(body.selections.len()) else {
        state.incomplete = true;
        return;
    };
    if len > state.options.limits.expression_nodes_per_function || !state.array(len, 64) {
        state.incomplete = true;
        return;
    }
    let mut colours = vec![0_u8; len];
    let number = |reference| match reference {
        OperandRef::Key(id) => Some(id.as_u32() as usize),
        OperandRef::Dependency(id) => body.keys.len().checked_add(id.as_u32() as usize),
        OperandRef::Selection(id) => dep_end.checked_add(id.as_u32() as usize),
        OperandRef::Value(_) => None,
    };
    let roots = body.keys.iter().map(|(id, _)| OperandRef::Key(id))
        .chain(body.deps.iter().map(|(id, _)| OperandRef::Dependency(id)))
        .chain(body.selections.iter().map(|(id, _)| OperandRef::Selection(id)));
    for root in roots {
        let mut stack = vec![(root, false)];
        while let Some((reference, leaving)) = stack.pop() {
            if !state.step(1) { return; }
            let Some(index) = number(reference) else { continue; };
            let Some(colour) = colours.get_mut(index) else { continue; };
            if leaving {
                *colour = 2;
                continue;
            }
            match *colour {
                2 => continue,
                1 => {
                    state.error(RuleId::S2, "cyclic metadata expression");
                    continue;
                }
                _ => *colour = 1,
            }
            stack.push((reference, true));
            if let Ok(children) = cfg::expression_children(reference, body) {
                if !state.array(children.len(), 32) { return; }
                stack.extend(children.into_iter().rev().map(|child| (child, false)));
            }
        }
    }
}

/// Shared iterative cycle/depth check for structural TypeId graphs.
pub(super) fn types(module: &Module, state: &mut State<'_>) {
    let len = module.types.len();
    if !state.array(len, 64) { return; }
    let mut colours = vec![0_u8; len];
    let mut heights = vec![0_usize; len];
    for (root, _) in module.types.iter() {
        let mut stack = vec![(root, false)];
        while let Some((id, leaving)) = stack.pop() {
            if !state.step(1) { return; }
            let i = id.as_u32() as usize;
            let Some(colour) = colours.get(i).copied() else { continue; };
            let Ok(ty) = module.types.get(id) else { continue; };
            let children = super::types::children(ty);
            if !state.array(children.clone().count(), 32) { return; }
            if leaving {
                let height = children.clone().filter_map(|id| heights.get(id.as_u32() as usize)).max();
                let height = height.copied().unwrap_or(0).checked_add(1);
                if height.is_none_or(|height| height > state.options.limits.expression_depth) {
                    state.incomplete = true;
                    return;
                }
                heights[i] = height.unwrap_or(0);
                colours[i] = 2;
                continue;
            }
            match colour {
                2 => continue,
                1 => {
                    state.at(super::EntityLocation::Type(id), None);
                    state.error(RuleId::S1, "cyclic structural type");
                    continue;
                }
                _ => colours[i] = 1,
            }
            stack.push((id, true));
            stack.extend(children.into_iter().rev().map(|child| (child, false)));
        }
    }
}
