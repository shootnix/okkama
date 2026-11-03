//! Backward dataflow at SPB instruction boundaries, including parallel edges.
use crate::Op;
use okkama_sparrow::{Handler, Target};
use std::collections::BTreeSet;

fn edge(target: &Target, live: &[BTreeSet<usize>], defined: Option<usize>) -> BTreeSet<usize> {
    let mut after = live[target.pc as usize].clone();
    if let Some(defined) = defined {
        after.remove(&defined);
    }
    let mut before = after.clone();
    for (dst, _) in &target.moves {
        before.remove(&(dst.0 as usize));
    }
    for (dst, source) in &target.moves {
        if after.contains(&(dst.0 as usize)) {
            before.insert(source.0 as usize);
        }
    }
    before
}

pub(crate) fn analyze(plan: &[Op], handlers: &[Handler]) -> Vec<Vec<usize>> {
    let mut live = vec![BTreeSet::new(); plan.len()];
    loop {
        let mut changed = false;
        for pc in (0..plan.len()).rev() {
            let mut next = match &plan[pc] {
                Op::Lookup {
                    array,
                    index,
                    value,
                    found,
                    missing,
                    ..
                } => {
                    let mut set = edge(found, &live, Some(*value));
                    set.extend(edge(missing, &live, None));
                    set.extend([*array, *index]);
                    set
                }
                Op::Throw(src) | Op::Raise(src) => BTreeSet::from([*src]),
                Op::Catch {
                    failure,
                    value,
                    found,
                    missing,
                    ..
                } => {
                    let mut set = edge(found, &live, Some(*value));
                    set.extend(edge(missing, &live, None));
                    set.insert(*failure);
                    set
                }
                Op::Trap(..) | Op::ReturnVoid | Op::Unreachable => BTreeSet::new(),
                Op::Panic(src) => BTreeSet::from([*src]),
                Op::Return(source) => BTreeSet::from([*source]),
                Op::Jump(target) => edge(target, &live, None),
                Op::Branch(condition, yes, no) => {
                    let mut set = edge(yes, &live, None);
                    set.extend(edge(no, &live, None));
                    set.insert(*condition);
                    set
                }
                _ => live.get(pc + 1).cloned().unwrap_or_default(),
            };
            match &plan[pc] {
                Op::RegionNew(dst) => {
                    next.remove(dst);
                }
                Op::RegionRegister(region, value, _) => {
                    next.extend([*region, *value]);
                }
                Op::RegionTransfer(from, to, value) => {
                    next.extend([*from, *to, *value]);
                }
                Op::RegionClose {
                    region,
                    keep,
                    failure,
                } => {
                    if let Some((dst, src)) = failure {
                        next.remove(dst);
                        next.insert(*src);
                    }
                    next.insert(*region);
                    next.extend(keep);
                }
                Op::Text(_, dst, args) | Op::Array(_, dst, args) | Op::Map(_, dst, args) => {
                    next.remove(dst);
                    next.extend(args.iter().flatten().copied());
                }
                Op::FailureLink(dst, a, b, _) | Op::RangeNew(dst, a, b, _) => {
                    next.remove(dst);
                    next.extend([*a, *b]);
                }
                Op::BuilderRange(dst, a, b, c, _) => {
                    next.remove(dst);
                    next.extend([*a, *b, *c]);
                }
                Op::RatPart(dst, src, _) | Op::RangePart(dst, src, _) => {
                    next.remove(dst);
                    next.insert(*src);
                }
                Op::MapNew(dst, entries) => {
                    next.remove(dst);
                    for (k, v) in entries {
                        next.extend([*k, *v]);
                    }
                }
                Op::Capture(pairs) => {
                    for (dst, src) in pairs {
                        if next.remove(dst) {
                            next.insert(*src);
                        }
                    }
                }
                Op::ObjectNew(dst, _, fields) | Op::ArrayNew(dst, fields) => {
                    next.remove(dst);
                    next.extend(fields.iter().copied());
                }
                Op::ObjectNext(dst, object, updates) => {
                    next.remove(dst);
                    next.insert(*object);
                    next.extend(updates.iter().map(|(_, r)| *r));
                }
                Op::ObjectInit(object, _, value) => {
                    next.insert(*object);
                    next.insert(*value);
                }
                Op::AnyBox(dst, object) | Op::ObjectGet(dst, object, _) => {
                    next.remove(dst);
                    next.insert(*object);
                }
                Op::RatNew(dst, a, b) | Op::RatBinary(_, dst, a, b) => {
                    next.remove(dst);
                    next.insert(*a);
                    next.insert(*b);
                }
                Op::RatUnary(_, dst, src) => {
                    next.remove(dst);
                    next.insert(*src);
                }

                Op::Bits(_, _, dst, value, count) => {
                    next.remove(dst);
                    next.insert(*value);
                    next.extend(count.iter().copied());
                }
                Op::ByteNot(dst, source) => {
                    next.remove(dst);
                    next.insert(*source);
                }
                Op::Unary(_, dst, source)
                | Op::FloatUnary(_, dst, source)
                | Op::Convert(_, dst, source)
                | Op::ByteFrom(dst, source, _) => {
                    next.remove(dst);
                    next.insert(*source);
                }
                Op::Equal(_, dst, left, right)
                | Op::ByteBinary(_, dst, left, right)
                | Op::Arithmetic(_, dst, left, right)
                | Op::FloatBinary(_, dst, left, right)
                | Op::Compare(_, dst, left, right) => {
                    next.remove(dst);
                    next.insert(*left);
                    next.insert(*right);
                }
                Op::CallCap { dst, receiver, .. } => {
                    next.remove(dst);
                    next.insert(*receiver);
                }
                Op::Role(dst, src, _, _) => {
                    next.remove(dst);
                    next.insert(*src);
                }
                Op::CallRole {
                    dst,
                    receiver,
                    args,
                    ..
                } => {
                    if let Some(dst) = dst {
                        next.remove(dst);
                    }
                    next.insert(*receiver);
                    next.extend(args.iter().copied());
                }
                Op::Closure { dst, captures, .. } => {
                    next.remove(dst);
                    next.extend(captures.iter().copied());
                }
                Op::CallFn {
                    dst, callee, args, ..
                } => {
                    if let Some(dst) = dst {
                        next.remove(dst);
                    }
                    next.insert(*callee);
                    next.extend(args.iter().copied());
                }
                Op::WorkerSpawn {
                    dst, entry, args, ..
                } => {
                    next.remove(dst);
                    next.insert(*entry);
                    next.extend(args);
                }
                Op::WorkerBind {
                    dst,
                    action,
                    entry,
                    fields,
                } => {
                    next.remove(dst);
                    next.extend([*action, *entry]);
                    next.extend(fields);
                }
                Op::Intrinsic(_, dst, argument) => {
                    next.remove(dst);
                    next.extend(argument.iter().flatten().copied());
                }
                Op::FutureSpawn(dst, future) => {
                    next.remove(dst);
                    next.insert(*future);
                }
                Op::FutureControl(_, dst, future) | Op::Await { dst, future } => {
                    if let Some(dst) = dst {
                        next.remove(dst);
                    }
                    next.insert(*future);
                }
                Op::Call { dst, args, .. } => {
                    if let Some(dst) = dst {
                        next.remove(dst);
                    }
                    next.extend(args.iter().copied());
                }
                Op::Const(dst, _) => {
                    next.remove(dst);
                }
                Op::Copy(dst, source) | Op::Not(dst, source) if next.remove(dst) => {
                    next.insert(*source);
                }
                _ => {}
            }
            // Failure observes the pre-instruction state: merge after normal
            // definition kills, so the old destination can survive a throw.
            if let Some(handler) = handlers
                .iter()
                .find(|h| h.start as usize <= pc && pc < (h.end as usize))
            {
                next.extend(edge(
                    &handler.target,
                    &live,
                    Some(handler.failure.0 as usize),
                ));
            }
            if live[pc] != next {
                live[pc] = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    live.into_iter()
        .map(|set| set.into_iter().collect())
        .collect()
}

/// Caller values needed by either the ordinary continuation or a handler.
/// Arguments alone are not saved, and a normal result does not kill its old
/// value when an exceptional continuation still reads it.
pub(crate) fn call_saved(
    plan: &[Op],
    live: &[Vec<usize>],
    handlers: &[Handler],
) -> Vec<Vec<usize>> {
    let sets: Vec<BTreeSet<usize>> = live.iter().map(|v| v.iter().copied().collect()).collect();
    plan.iter()
        .enumerate()
        .map(|(pc, op)| {
            let dst = match op {
                Op::Call { dst, .. } | Op::CallFn { dst, .. } | Op::CallRole { dst, .. } => *dst,
                Op::CallCap { dst, .. } => Some(*dst),
                Op::Await { dst, .. } => *dst,
                _ => return Vec::new(),
            };
            let mut saved = sets.get(pc + 1).cloned().unwrap_or_default();
            if let Some(dst) = dst {
                saved.remove(&dst);
            }
            if let Some(handler) = handlers
                .iter()
                .find(|h| h.start as usize <= pc && pc < (h.end as usize))
            {
                saved.extend(edge(
                    &handler.target,
                    &sets,
                    Some(handler.failure.0 as usize),
                ));
            }
            saved.into_iter().collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use okkama_sparrow::Register as R;

    #[test]
    fn parallel_swap_reads_original_sources_and_drops_dead_values() {
        let plan = [
            Op::Const(0, crate::value::Slot::small(1)),
            Op::Const(1, crate::value::Slot::small(2)),
            Op::Const(2, crate::value::Slot::small(99)),
            Op::Jump(Target {
                pc: 4,
                moves: vec![(R(0), R(1)), (R(1), R(0))],
            }),
            Op::Return(0),
        ];
        let live = analyze(&plan, &[]);
        assert!(live[0].is_empty());
        assert_eq!(live[2], vec![1]);
        assert_eq!(live[3], vec![1]);
        assert_eq!(live[4], vec![0]);
    }
}

#[cfg(test)]
mod exceptional_tests {
    use super::*;
    use crate::value::Slot;
    use okkama_sparrow::Register as R;
    #[test]
    fn old_call_destination_is_preserved_when_only_handler_needs_it() {
        let plan = vec![
            Op::Const(0, Slot::small(7)),
            Op::Const(1, Slot::small(9)),
            Op::Call {
                dst: Some(0),
                function: 1,
                args: vec![1],
                tail: false,
            },
            Op::Return(0),
            Op::Return(0),
        ];
        let handlers = [Handler {
            start: 2,
            end: 3,
            target: Target {
                pc: 4,
                moves: vec![],
            },
            failure: R(2),
        }];
        let live = analyze(&plan, &handlers);
        assert_eq!(live[2], vec![0, 1]);
        assert_eq!(call_saved(&plan, &live, &handlers)[2], vec![0]);
        let normal = analyze(&plan, &[]);
        assert!(call_saved(&plan, &normal, &[])[2].is_empty());
    }
    #[test]
    fn handler_moves_read_the_old_failure_slot_before_it_is_defined() {
        let plan = vec![
            Op::Const(2, Slot::small(7)),
            Op::Call {
                dst: None,
                function: 1,
                args: vec![],
                tail: false,
            },
            Op::ReturnVoid,
            Op::Return(3),
        ];
        let handlers = [Handler {
            start: 1,
            end: 2,
            target: Target {
                pc: 3,
                moves: vec![(R(3), R(2))],
            },
            failure: R(2),
        }];
        let live = analyze(&plan, &handlers);
        assert_eq!(live[1], vec![2]);
        assert_eq!(call_saved(&plan, &live, &handlers)[1], vec![2]);
    }
}
