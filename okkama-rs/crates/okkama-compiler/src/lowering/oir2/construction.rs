//! Independent producer self-check. This is not the semantic verifier from #564.
use std::collections::{HashMap, HashSet, VecDeque};

use super::{LowerError, Result};
use crate::oir2 as ir;

fn error(message: &str) -> LowerError {
    LowerError::invalid(crate::source::Span::default(), format!("OIR-2 construction: {message}"))
}

fn targets(term: &ir::TermKind) -> Vec<&ir::Target> {
    match term {
        ir::TermKind::Br(target) => vec![target],
        ir::TermKind::Cond { yes, no, .. } => vec![yes, no],
        ir::TermKind::Switch { cases, default, .. } => {
            cases.iter().map(|case| &case.target).chain([default]).collect()
        }
        _ => vec![],
    }
}

pub(super) fn undefined_values(body: &ir::Body) -> Result<HashSet<ir::ValueId>> {
    let mut undefined = HashSet::new();
    for (_, inst) in body.insts.iter() {
        if matches!(inst.op, ir::Operation::Core(ir::CoreOp::Undef { .. })) {
            undefined.extend(inst.results.iter().copied());
        }
    }
    loop {
        let count = undefined.len();
        for (_, block) in body.blocks.iter() {
            for edge in targets(&block.term.kind) {
                for (&value, &param) in edge.args.iter().zip(&body.blocks.get(edge.block)?.params) {
                    if undefined.contains(&value) {
                        undefined.insert(param);
                    }
                }
            }
        }
        if undefined.len() == count {
            break;
        }
    }
    Ok(undefined)
}

pub(super) fn check(module: &ir::Module, body: &ir::Body) -> Result<()> {
    let undefined = undefined_values(body)?;
    let mut local_objects = HashSet::new();
    for (allocation, inst) in body.insts.iter() {
        let ir::Operation::Core(ir::CoreOp::ObjNew { declaration, fields, construction, .. }) = &inst.op else {
            continue;
        };
        let ir::DeclarationKind::Object { fields: layout, .. } = &module.declarations.get(*declaration)?.kind else {
            return Err(error("obj.new has no object layout"));
        };
        if fields.len() != layout.len() {
            return Err(error("allocation fields do not match layout"));
        }
        let pending = layout.last().is_some_and(|field| field.name == "$pending"
            && module.types.get(field.ty) == Ok(&ir::Type::Bool));
        if *construction == ir::ConstructionStatus::Incomplete
            && (!pending || !fields.last().is_some_and(|value| undefined.contains(value))) {
            return Err(error("incomplete self requires trailing $pending: bool = undef"));
        }
        if *construction == ir::ConstructionStatus::Complete
            && (fields.iter().any(|value| undefined.contains(value))
                || (pending && !fields.last().is_some_and(|value| is_false(body, *value)))) {
            return Err(error("complete self has uninitialized fields or non-false $pending"));
        }
        let mut aliases: HashSet<_> = inst.results.iter().copied().collect();
        loop {
            let count = aliases.len();
            for (_, block) in body.blocks.iter() {
                for edge in targets(&block.term.kind) {
                    for (&value, &param) in edge.args.iter().zip(&body.blocks.get(edge.block)?.params) {
                        if aliases.contains(&value) {
                            aliases.insert(param);
                        }
                    }
                }
            }
            if aliases.len() == count {
                break;
            }
        }
        local_objects.extend(aliases.iter().copied());
        check_identity(body, allocation, fields, *construction, &aliases, &undefined, pending)?;
    }
    for (_, inst) in body.insts.iter() {
        if let ir::Operation::Core(ir::CoreOp::ObjInit { object, .. }) = inst.op
            && !local_objects.contains(&object) {
            return Err(error("obj.init does not target constructor-local self"));
        }
    }
    Ok(())
}

fn is_false(body: &ir::Body, value: ir::ValueId) -> bool {
    let Ok(data) = body.values.get(value) else {
        return false;
    };
    let ir::Definition::InstResult { inst, .. } = data.def else {
        return false;
    };
    body.insts.get(inst).is_ok_and(|inst| inst.op == ir::Operation::Const(ir::Literal::Bool(false)))
}

fn check_identity(
    body: &ir::Body,
    allocation: ir::InstId,
    fields: &[ir::ValueId],
    construction: ir::ConstructionStatus,
    aliases: &HashSet<ir::ValueId>,
    undefined: &HashSet<ir::ValueId>,
    pending: bool,
) -> Result<()> {
    // Each cell is a set: absent (0), uninitialized (1), initialized (2).
    // Union at joins makes reads and completion valid on every incoming path.
    let mut incoming = HashMap::from([(body.entry, vec![0u8; fields.len() + 1])]);
    let mut queue = VecDeque::from([body.entry]);
    while let Some(id) = queue.pop_front() {
        let block = body.blocks.get(id)?;
        let mut state = incoming[&id].clone();
        for &instruction in &block.insts {
            let inst = body.insts.get(instruction)?;
            if instruction == allocation {
                for (slot, field) in state.iter_mut().zip(fields) {
                    *slot = if undefined.contains(field) { 1 } else { 2 };
                }
                state[fields.len()] = if construction == ir::ConstructionStatus::Complete { 2 } else { 1 };
            }
            if state[fields.len()] == 0 {
                continue;
            }
            match &inst.op {
                ir::Operation::Core(ir::CoreOp::ObjGet { object, field }) if aliases.contains(object) => {
                    if state[field.index as usize] != 2 {
                        return Err(error("read of an uninitialized field"));
                    }
                }
                ir::Operation::Core(ir::CoreOp::ObjInit { object, field, value, completion })
                    if aliases.contains(object) => {
                    let index = field.index as usize;
                    if index >= fields.len() || completion.is_some() || state[fields.len()] != 1
                        || state[index] != 1 || undefined.contains(value) {
                        return Err(error("duplicate initialization or completion of self"));
                    }
                    if pending && index + 1 == fields.len() {
                        if state[..index].iter().any(|slot| *slot != 2) || !is_false(body, *value) {
                            return Err(error("completion before field initialization"));
                        }
                        state[fields.len()] = 2;
                    }
                    state[index] = 2;
                }
                _ => {}
            }
        }
        if let ir::TermKind::Ret { values } = &block.term.kind
            && values.iter().any(|value| aliases.contains(value)) && state[fields.len()] != 2 {
            return Err(error("return of incomplete self"));
        }
        for edge in targets(&block.term.kind) {
            let changed = if let Some(old) = incoming.get_mut(&edge.block) {
                let previous = old.clone();
                for (slot, next) in old.iter_mut().zip(&state) {
                    *slot |= next;
                }
                *old != previous
            } else {
                incoming.insert(edge.block, state.clone());
                true
            };
            if changed {
                queue.push_back(edge.block);
            }
        }
    }
    Ok(())
}
