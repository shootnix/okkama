//! Local nonescaping object elimination. Preserve field snapshots in fresh
//! typed registers; never substitute mutable source registers at a later read.
use crate::{Op, scalar::Kind};
use okkama_sparrow::{Access, Function, Instruction};

pub(crate) fn lower(source: &Function, plan: &mut [Op], kinds: &mut Vec<Kind>) -> usize {
    let mut removed = 0;
    for pc in 0..plan.len() {
        let Op::ObjectNew(object, _, fields) = &plan[pc] else {
            continue;
        };
        let object = *object;
        if source.params.iter().any(|r| r.0 as usize == object) {
            continue;
        }
        let mut definitions = 0;
        let mut escapes = false;
        for instruction in &source.code {
            instruction.visit(&mut |access| match access {
                Access::Def(r) if r.0 as usize == object => definitions += 1,
                Access::Use(r) if r.0 as usize == object => {
                    if !matches!(instruction, Instruction::ObjGet { object: r, .. } if r.0 as usize == object) { escapes = true; }
                }
                Access::Target(t) => {
                    for (dst, src) in &t.moves {
                        if dst.0 as usize == object { definitions += 1; }
                        if src.0 as usize == object { escapes = true; }
                    }
                }
                _ => {}
            });
        }
        if definitions != 1 || escapes {
            continue;
        }
        let captures: Vec<_> = fields
            .iter()
            .map(|src| {
                let dst = kinds.len();
                kinds.push(kinds[*src]);
                (dst, *src)
            })
            .collect();
        for op in plan.iter_mut() {
            if let Op::ObjectGet(dst, receiver, field) = op {
                if *receiver == object {
                    *op = Op::Copy(*dst, captures[*field as usize].0);
                }
            }
        }
        plan[pc] = Op::Capture(captures);
        removed += 1;
    }
    removed
}
