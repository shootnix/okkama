//! Eliminate a pure exact-integer fold recurrence across compiler CFG edges.
//! No function names, source spelling or unit base/step values are assumed.
use crate::{
    Op,
    value::{Arithmetic, Slot},
};
use okkama_sparrow::{Register, Target};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Origin {
    Before(usize),
    Recursive,
    Combined,
}

/// Track parallel copies after the call. The only computation permitted on the
/// return path is one associative exact add/multiply of the recursive result.
fn continuation(
    plan: &[Op],
    call: usize,
    dst: usize,
    registers: usize,
) -> Option<(Arithmetic, usize)> {
    let mut origins: Vec<_> = (0..registers).map(Origin::Before).collect();
    origins[dst] = Origin::Recursive;
    let mut visited = vec![false; plan.len()];
    let mut pc = call + 1;
    let mut fold = None;
    loop {
        if pc >= plan.len() || std::mem::replace(&mut visited[pc], true) {
            return None;
        }
        match &plan[pc] {
            Op::Copy(dst, src) => origins[*dst] = origins[*src],
            Op::Capture(moves) => {
                let values: Vec<_> = moves.iter().map(|(_, source)| origins[*source]).collect();
                for ((dst, _), value) in moves.iter().zip(values) {
                    origins[*dst] = value;
                }
            }
            Op::Jump(target) => {
                let values: Vec<_> = target
                    .moves
                    .iter()
                    .map(|(_, source)| origins[source.0 as usize])
                    .collect();
                for ((dst, _), value) in target.moves.iter().zip(values) {
                    origins[dst.0 as usize] = value;
                }
                pc = target.pc as usize;
                continue;
            }
            Op::Arithmetic(op @ (Arithmetic::Add | Arithmetic::Multiply), dst, left, right)
                if fold.is_none() =>
            {
                let term = match (origins[*left], origins[*right]) {
                    (Origin::Recursive, Origin::Before(term))
                    | (Origin::Before(term), Origin::Recursive) => term,
                    _ => return None,
                };
                fold = Some((*op, term));
                origins[*dst] = Origin::Combined;
            }
            Op::Return(value) if origins[*value] == Origin::Combined => return fold,
            _ => return None,
        }
        pc += 1;
    }
}

pub(crate) fn lower(
    plan: &mut Vec<Op>,
    function: usize,
    params: &[usize],
    registers: &mut usize,
) -> bool {
    let mut recursive = None;
    // Other calls, heap mutations/reads, native effects, exceptions, resources,
    // floating arithmetic and async are deliberately excluded from this proof.
    for (pc, op) in plan.iter().enumerate() {
        match op {
            Op::Call {
                dst: Some(dst),
                function: callee,
                args,
                tail: false,
            } if *callee == function && args.len() == params.len() && recursive.is_none() => {
                recursive = Some((pc, *dst))
            }
            Op::Const(..)
            | Op::Copy(..)
            | Op::Capture(..)
            | Op::Compare(..)
            | Op::Not(..)
            | Op::Branch(..)
            | Op::Jump(..)
            | Op::Arithmetic(..)
            | Op::Return(..) => {}
            _ => return false,
        }
    }
    let Some((call, dst)) = recursive else {
        return false;
    };
    let Some((combine, term)) = continuation(plan, call, dst, *registers) else {
        return false;
    };
    let accumulator = *registers;
    let result = accumulator + 1;
    *registers += 2;
    let mut next_pc = 1u32;
    let mapping: Vec<_> = plan
        .iter()
        .enumerate()
        .map(|(pc, op)| {
            let mapped = next_pc;
            next_pc += if pc == call || matches!(op, Op::Return(_)) {
                2
            } else {
                1
            };
            mapped
        })
        .collect();
    let mut rewritten = Vec::with_capacity(next_pc as usize);
    rewritten.push(Op::Const(
        accumulator,
        Slot::small(if matches!(combine, Arithmetic::Multiply) {
            1
        } else {
            0
        }),
    ));
    for (pc, mut op) in std::mem::take(plan).into_iter().enumerate() {
        if pc == call {
            let Op::Call { args, .. } = op else {
                unreachable!()
            };
            rewritten.push(Op::Arithmetic(combine, accumulator, accumulator, term));
            rewritten.push(Op::Jump(Target {
                pc: mapping[0],
                moves: params
                    .iter()
                    .zip(args)
                    .map(|(&dst, src)| (Register(dst as u32), Register(src as u32)))
                    .collect(),
            }));
        } else if let Op::Return(value) = op {
            rewritten.push(Op::Arithmetic(combine, result, accumulator, value));
            rewritten.push(Op::Return(result));
        } else {
            match &mut op {
                Op::Jump(target) => target.pc = mapping[target.pc as usize],
                Op::Branch(_, yes, no) => {
                    yes.pc = mapping[yes.pc as usize];
                    no.pc = mapping[no.pc as usize];
                }
                _ => {}
            }
            rewritten.push(op);
        }
    }
    *plan = rewritten;
    true
}
