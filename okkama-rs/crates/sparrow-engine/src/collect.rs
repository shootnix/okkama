//! Structured collection policy; execution, suspension and cleanup use VM
//! continuations on Tokio. Completion notifications are shared, not polled.
use crate::plan::Plan;
use crate::{
    Error,
    failure::FailureKind,
    future::State,
    value::{Heap, Slot},
};
#[derive(Debug, Clone, Copy)]
pub(crate) enum Strategy {
    All(u32),
    Race,
    Timeout(u32),
    First,
}
#[derive(Debug, Clone, Copy)]
pub(crate) enum Operation {
    Size,
    Settled(bool),
    Init(Strategy),
    Begin,
    Next,
    Consume,
    Abort,
    Finish,
}
#[derive(Debug)]
pub(crate) struct Collection {
    strategy: Strategy,
    entries: Vec<Slot>,
    consumed: Vec<bool>,
    cancelled: Vec<bool>,
    initial_order: u64,
    winner: Option<Slot>,
    failures: Vec<Slot>,
    external: Option<Slot>,
    finishing: bool,
}
impl Collection {
    pub(crate) fn roots(&self, roots: &mut Vec<Slot>) {
        roots.extend_from_slice(&self.entries);
        roots.extend_from_slice(&self.failures);
        roots.extend(self.winner);
        roots.extend(self.external);
    }
}
impl Heap {
    fn cancel_collection(&mut self, receiver: Slot) -> Result<(), Error> {
        let state = self
            .collectors
            .get_mut(&receiver.bits)
            .ok_or_else(|| Error("invalid collection".into()))?;
        state.finishing = true;
        let entries = state.entries.clone();
        let mut cancelled = std::collections::HashSet::new();
        for (index, entry) in entries.into_iter().enumerate() {
            if cancelled.contains(&entry.bits)
                || !matches!(self.future(entry)?.state, State::Ready(_))
            {
                cancelled.insert(entry.bits);
                self.collectors
                    .get_mut(&receiver.bits)
                    .expect("collection")
                    .cancelled[index] = true;
                self.future_control(okkama_sparrow::FutOp::Cancel, entry)?;
            }
        }
        Ok(())
    }
    pub(crate) fn poll_collection(&mut self, future: Slot) -> Result<(), Error> {
        let State::Collect(receiver) = self.future(future)?.state else {
            return Ok(());
        };
        let state = self
            .collectors
            .get(&receiver.bits)
            .ok_or_else(|| Error("invalid collection".into()))?;
        let mut best = None;
        let mut remaining = false;
        for (index, entry) in state.entries.iter().enumerate() {
            if state.consumed[index] {
                continue;
            }
            remaining = true;
            let entry = self.future(*entry)?;
            if matches!(entry.state, State::Ready(_)) {
                let order = if entry.ready_order <= state.initial_order {
                    (0, 0, index)
                } else {
                    (1, entry.ready_order, index)
                };
                if best.is_none_or(|before| order < before) {
                    best = Some(order);
                }
            }
        }
        if let Some((_, _, index)) = best {
            self.future_state(future, State::Ready(Ok(Slot::small(index as i64))))?;
        } else if !remaining {
            self.future_state(future, State::Ready(Ok(Slot::small(-1))))?;
        }
        Ok(())
    }
    pub(crate) fn collect_operation(
        &mut self,
        operation: Operation,
        args: [Option<Slot>; 3],
    ) -> Result<Slot, Error> {
        let receiver = args[0].ok_or_else(|| Error("missing collection receiver".into()))?;
        if let Operation::Settled(success) = operation {
            let value = args[1].ok_or_else(|| Error("missing settled value".into()))?;
            if !success {
                self.failure(value)?;
            }
            return self.future_with_state(State::Ready(if success {
                Ok(value)
            } else {
                Err(value)
            }));
        }
        if let Operation::Size = operation {
            let count = self
                .integer(args[1].ok_or_else(|| Error("missing sequence size".into()))?)?
                .to_i64()
                .ok_or_else(|| Error("Cannot read origins: size is not an int".into()))?;
            return Ok(Slot::small(count.max(0)));
        }
        if let Operation::Init(strategy) = operation {
            let entries = self
                .elements(args[1].ok_or_else(|| Error("missing collection entries".into()))?)?
                .to_vec();
            for &entry in &entries {
                self.future(entry)?;
                self.futures.get_mut(entry.bits)?.observed = true;
            }
            let count = entries.len();
            self.collectors.insert(
                receiver.bits,
                Collection {
                    strategy,
                    entries,
                    consumed: vec![false; count],
                    cancelled: vec![false; count],
                    initial_order: 0,
                    winner: None,
                    failures: vec![],
                    external: None,
                    finishing: false,
                },
            );
            return Ok(Slot::default());
        }
        match operation {
            Operation::Begin => {
                self.collectors
                    .get_mut(&receiver.bits)
                    .ok_or_else(|| Error("invalid collection".into()))?
                    .initial_order = self.completion_clock;
                Ok(Slot::default())
            }
            Operation::Next => {
                let future = self.future_with_state(State::Collect(receiver))?;
                self.poll_collection(future)?;
                Ok(future)
            }
            Operation::Consume => {
                let index = self
                    .integer(args[1].ok_or_else(|| Error("missing completion index".into()))?)?
                    .to_i64()
                    .and_then(|i| usize::try_from(i).ok())
                    .ok_or_else(|| Error("invalid completion index".into()))?;
                let state = self
                    .collectors
                    .get(&receiver.bits)
                    .ok_or_else(|| Error("invalid collection".into()))?;
                let entry = *state
                    .entries
                    .get(index)
                    .ok_or_else(|| Error("invalid completion index".into()))?;
                let State::Ready(mut result) = self.future(entry)?.state else {
                    return Err(Error("unsettled completion".into()));
                };
                if let Strategy::Timeout(owner) = state.strategy {
                    if index == 1 && !state.finishing && result.is_ok() {
                        let error = self.object_new(owner, std::iter::empty())?;
                        result = Err(self.failure_new(FailureKind::Thrown(error))?);
                    }
                }
                let state = &self.collectors[&receiver.bits];
                let ignore = if let Err(failure) = result {
                    let record = self.failure(failure)?;
                    (state.cancelled[index]
                        && matches!(record.kind, FailureKind::Cancelled)
                        && record.secondary.is_empty())
                        || (state.finishing
                            && state.external.is_none()
                            && !matches!(state.strategy, Strategy::All(_))
                            && !state.cancelled[index])
                } else {
                    false
                };
                let state = self.collectors.get_mut(&receiver.bits).expect("collection");
                if std::mem::replace(&mut state.consumed[index], true) {
                    return Err(Error("duplicate completion".into()));
                }
                let mut cancel = false;
                match result {
                    Ok(value)
                        if !state.finishing && !matches!(state.strategy, Strategy::All(_)) =>
                    {
                        if matches!(state.strategy, Strategy::First) {
                            state.failures.clear();
                        }
                        state.winner = Some(value);
                        cancel = true;
                    }
                    Err(failure) if !ignore => {
                        if !state.failures.iter().any(|f| f.bits == failure.bits) {
                            state.failures.push(failure);
                        }
                        if !state.finishing && !matches!(state.strategy, Strategy::First) {
                            cancel = true;
                        }
                    }
                    _ => {}
                }
                if cancel {
                    self.cancel_collection(receiver)?;
                }
                Ok(Slot::default())
            }
            Operation::Abort => {
                let failure =
                    args[1].ok_or_else(|| Error("missing collection cancellation".into()))?;
                self.failure(failure)?;
                self.collectors
                    .get_mut(&receiver.bits)
                    .ok_or_else(|| Error("invalid collection".into()))?
                    .external = Some(failure);
                self.cancel_collection(receiver)?;
                Ok(Slot::default())
            }
            Operation::Finish => {
                let state = self
                    .collectors
                    .get(&receiver.bits)
                    .ok_or_else(|| Error("invalid collection".into()))?;
                if state.consumed.iter().any(|c| !*c) {
                    return Err(Error("collection finished before cleanup".into()));
                }
                let (strategy, entries, winner, mut failures, external) = (
                    state.strategy,
                    state.entries.clone(),
                    state.winner,
                    state.failures.clone(),
                    state.external,
                );
                // Failed attempts preceding a FirstSuccessful winner are expected;
                // failures from loser cleanup are not. Keep these separately below.
                let primary = external.or_else(|| failures.first().copied());
                let result = if let Some(primary) = primary {
                    // AllFutures keeps the first observed error as primary, but
                    // attaches other errors in input order, independent of wake order.
                    if matches!(strategy, Strategy::All(_)) {
                        let mut positions = std::collections::HashMap::new();
                        for (index, entry) in entries.iter().enumerate() {
                            if let State::Ready(Err(failure)) = self.future(*entry)?.state {
                                positions.entry(failure.bits).or_insert(index);
                            }
                        }
                        failures.sort_by_key(|failure| positions.get(&failure.bits).copied());
                    }
                    for failure in failures {
                        if failure.bits != primary.bits {
                            self.failure_link(primary, failure, true)?;
                        }
                    }
                    Err(primary)
                } else if let Strategy::All(owner) = strategy {
                    let values = entries
                        .into_iter()
                        .map(|entry| match self.future(entry)?.state {
                            State::Ready(Ok(value)) => Ok(value),
                            _ => Err(Error("missing successful collection value".into())),
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let array = self.allocate_array(values)?;
                    Ok(self.object_new(owner, [array].into_iter())?)
                } else {
                    Ok(winner.ok_or_else(|| Error("collection has no winner".into()))?)
                };
                self.future_with_state(State::Ready(result))
            }
            Operation::Init(_) | Operation::Size | Operation::Settled(_) => unreachable!(),
        }
    }
}

pub(crate) fn lower(program: &crate::Program, index: usize) -> Result<crate::FunctionPlan, Error> {
    use crate::{Op, Type, array::ArrayOp, value::Arithmetic};
    use okkama_sparrow::{Cmp, Handler, Register, Target};
    let f = &program.functions[index];
    let intrinsic = f.intrinsic.as_ref().expect("collection intrinsic");
    let invalid = || {
        Error(format!(
            "invalid intrinsic signature/layout: {}",
            intrinsic.name
        ))
    };
    let strategy = match intrinsic.name.as_str() {
        "okkama.collect.AllFutures.results" => {
            Strategy::All(intrinsic.objects.first().ok_or_else(invalid)?.0)
        }
        "okkama.collect.Raced.result" => Strategy::Race,
        "okkama.collect.FirstSuccessful.result" => Strategy::First,
        name => return Err(Error(format!("unsupported intrinsic {name} in new engine"))),
    };
    if !f.is_async
        || f.params.len() != 1
        || intrinsic.objects.len() != 1
        || !intrinsic.helpers.is_empty()
    {
        return Err(invalid());
    }
    let receiver = f.params[0].0 as usize;
    let Some(Type::Object(owner)) = program.ty(f.registers[receiver]) else {
        return Err(invalid());
    };
    let object = &program.objects[owner.0 as usize];
    let prefix = match strategy {
        Strategy::All(_) => "async.collect::AllFutures<",
        Strategy::Race => "async.collect::Raced<",
        Strategy::First => "async.collect::FirstSuccessful<",
        Strategy::Timeout(_) => return Err(invalid()),
    };
    if !object.name.starts_with(prefix)
        || object.destroy.is_some()
        || object.fields.len() != 1
        || object.fields[0].0 != "origins"
    {
        return Err(invalid());
    }
    let Some(Type::Role(role)) = program.ty(object.fields[0].1) else {
        return Err(invalid());
    };
    let declaration = &program.roles[role.0 as usize];
    let size_slot = declaration
        .methods
        .iter()
        .position(|m| m.name == "size")
        .ok_or_else(invalid)?;
    let item_slot = declaration
        .methods
        .iter()
        .position(|m| m.name == "item")
        .ok_or_else(invalid)?;
    let size_method = &declaration.methods[size_slot];
    let item_method = &declaration.methods[item_slot];
    if !size_method.params.is_empty()
        || size_method.result.and_then(|t| program.ty(t)) != Some(&Type::Int)
        || item_method.params.len() != 1
        || program.ty(item_method.params[0]) != Some(&Type::Int)
    {
        return Err(invalid());
    }
    let Some(Type::Future(Some(element))) = item_method.result.and_then(|t| program.ty(t)) else {
        return Err(invalid());
    };
    if crate::kind(program, *element).is_none() {
        return Err(invalid());
    }
    let related = &program.objects[intrinsic.objects[0].0 as usize];
    if related.destroy.is_some() {
        return Err(invalid());
    }
    if let Strategy::All(_) = strategy {
        let Some(Type::Role(result_role)) = f.result.and_then(|t| program.ty(t)) else {
            return Err(invalid());
        };
        if !related.name.starts_with("core::ArraySequence<")
            || related.fields.len() != 1
            || related.fields[0].0 != "items"
            || program.ty(related.fields[0].1) != Some(&Type::Array(*element))
            || !related.does.iter().any(|w| w.role == *result_role)
        {
            return Err(invalid());
        }
        // The returned role's item contract must agree with the stored elements.
        if !program.roles[result_role.0 as usize]
            .methods
            .iter()
            .any(|m| m.name == "item" && m.result == Some(*element))
        {
            return Err(invalid());
        }
    } else if f.result != Some(*element)
        || related.name != "async.collect::NoFutures"
        || !related.fields.is_empty()
    {
        return Err(invalid());
    }
    let n = f.registers.len();
    let (
        seq,
        size,
        i,
        one,
        cond,
        builder,
        entries,
        child,
        selected,
        selection,
        zero,
        failure,
        result,
    ) = (
        n,
        n + 1,
        n + 2,
        n + 3,
        n + 4,
        n + 5,
        n + 6,
        n + 7,
        n + 8,
        n + 9,
        n + 10,
        n + 11,
        n + 12,
    );
    let mut p = Plan::default();
    let op = |operation, dst, args| {
        Op::Intrinsic(crate::intrinsic::Operation::Collect(operation), dst, args)
    };
    let first = [Some(receiver), None, None];
    p.push(Op::ObjectGet(seq, receiver, 0));
    p.push(Op::CallRole {
        tail: false,
        dst: Some(size),
        receiver: seq,
        role: role.0,
        slot: size_slot,
        args: vec![],
    });
    p.push(op(
        Operation::Size,
        size,
        [Some(receiver), Some(size), None],
    ));
    p.push(Op::Const(i, Slot::small(0)));
    p.push(Op::Const(one, Slot::small(1)));
    p.push(Op::Const(zero, Slot::small(0)));
    p.push(Op::Array(ArrayOp::BuilderNew, builder, [None; 3]));
    p.label("read");
    p.push(Op::Compare(Cmp::Lt, cond, i, size));
    p.branch(cond, "item", "read_done");
    p.label("item");
    p.push(Op::CallRole {
        tail: false,
        dst: Some(child),
        receiver: seq,
        role: role.0,
        slot: item_slot,
        args: vec![i],
    });
    p.push(Op::Array(
        ArrayOp::Push,
        builder,
        [Some(builder), Some(child), None],
    ));
    p.push(Op::Arithmetic(Arithmetic::Add, i, i, one));
    p.jump("read");
    p.label("read_done");
    p.push(Op::Array(
        ArrayOp::Finish,
        entries,
        [Some(builder), None, None],
    ));
    if !matches!(strategy, Strategy::All(_)) {
        p.push(Op::Compare(Cmp::Eq, cond, size, zero));
        p.branch(cond, "empty", "init");
    }
    p.label("init");
    p.push(op(
        Operation::Init(strategy),
        result,
        [Some(receiver), Some(entries), None],
    ));
    p.push(Op::Const(i, Slot::small(0)));
    p.label("start");
    p.push(Op::Compare(Cmp::Lt, cond, i, size));
    p.branch(cond, "spawn", "started");
    p.label("spawn");
    p.push(Op::Array(
        ArrayOp::Get,
        child,
        [Some(entries), Some(i), None],
    ));
    p.push(Op::FutureSpawn(child, child));
    p.push(Op::Arithmetic(Arithmetic::Add, i, i, one));
    p.jump("start");
    p.label("started");
    p.push(op(Operation::Begin, result, first));
    p.label("next");
    p.push(op(Operation::Next, selection, first));
    let await_pc = p.ops.len() as u32;
    p.push(Op::Await {
        dst: Some(selected),
        future: selection,
    });
    p.push(Op::Compare(Cmp::Lt, cond, selected, zero));
    p.branch(cond, "finish", "consume");
    p.label("consume");
    p.push(op(
        Operation::Consume,
        result,
        [Some(receiver), Some(selected), None],
    ));
    p.jump("next");
    p.label("abort");
    let abort = p.ops.len() as u32;
    p.push(op(
        Operation::Abort,
        result,
        [Some(receiver), Some(failure), None],
    ));
    p.jump("next");
    p.label("finish");
    p.push(op(Operation::Finish, result, first));
    p.push(Op::Await {
        dst: Some(result),
        future: result,
    });
    p.push(Op::Return(result));
    if !matches!(strategy, Strategy::All(_)) {
        p.label("empty");
        p.push(Op::ObjectNew(result, intrinsic.objects[0].0, vec![]));
        p.push(Op::Throw(result));
    }
    let plan = p.finish();
    let handlers = vec![Handler {
        start: await_pc,
        end: await_pc + 1,
        target: Target {
            pc: abort,
            moves: vec![],
        },
        failure: Register(failure as u32),
    }];
    let live = crate::liveness::analyze(&plan, &handlers);
    let call_saved = crate::liveness::call_saved(&plan, &live, &handlers);
    Ok(crate::FunctionPlan {
        native: None,
        is_async: true,
        cold_entry: 0,
        cached: true,
        symbol: f.symbol.clone(),
        recurrence_loop: false,
        elided_objects: 0,
        params: vec![receiver],
        plan,
        registers: n + 13,
        compiled: Default::default(),
        jit_kinds: None,
        live,
        call_saved,
        handlers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_preserves_primary_completion_order_and_secondary_input_order() {
        let mut heap = Heap::new(&[], &[]);
        let owner = heap.object_new(0, std::iter::empty()).unwrap();
        let mut failures = Vec::new();
        let mut entries = Vec::new();
        for _ in 0..3 {
            let failure = heap.failure_new(FailureKind::Cancelled).unwrap();
            failures.push(failure);
            entries.push(heap.future_with_state(State::Ready(Err(failure))).unwrap());
        }
        // Simulate three completed inputs observed in order 2, 1, 0.
        // The primary remains 2; secondary diagnostics must be 0, 1.
        heap.collectors.insert(
            owner.bits,
            Collection {
                strategy: Strategy::All(0),
                entries,
                consumed: vec![true; 3],
                cancelled: vec![false; 3],
                initial_order: 0,
                winner: None,
                failures: vec![failures[2], failures[1], failures[0]],
                external: None,
                finishing: true,
            },
        );
        let result = heap
            .collect_operation(Operation::Finish, [Some(owner), None, None])
            .unwrap();
        assert!(
            matches!(heap.future(result).unwrap().state, State::Ready(Err(v)) if v.bits == failures[2].bits)
        );
        let secondary = &heap.failure(failures[2]).unwrap().secondary;
        assert_eq!(
            secondary.iter().map(|v| v.bits).collect::<Vec<_>>(),
            vec![failures[0].bits, failures[1].bits]
        );
    }
}
