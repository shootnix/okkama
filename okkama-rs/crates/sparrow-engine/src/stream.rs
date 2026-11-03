//! FIFO completion streams. Tokio wakes requests; VM plans initialize sources,
//! start inputs and drain cancellation cleanup without a separate executor.
use crate::{
    Error,
    failure::FailureKind,
    future::State,
    value::{Heap, Slot},
};
use std::{cell::Cell, collections::VecDeque, rc::Rc};
#[derive(Debug, Clone, Copy)]
pub(crate) enum Operation {
    CreateFuture(usize),
    Cancelled,
    BeginInit,
    InitFuture,
    Init,
    InitFailed,
    Remaining,
    Reserve(u32),
    TryStart,
    Entries,
    Started,
    BeginAbort,
    AbortFuture,
    FinishAbort,
}
#[derive(Debug)]
pub(crate) struct Stream {
    init: Slot,
    init_failed: bool,
    entries: Vec<Slot>,
    started: bool,
    starting: bool,
    initial_order: u64,
    emitted: Vec<bool>,
    queue: VecDeque<usize>,
    waiters: VecDeque<Rc<Request>>,
    reserved: usize,
    delivered: usize,
    aborting: Rc<Cell<bool>>,
    abort_future: Option<Slot>,
    aborted: Option<Slot>,
    cancelled: Vec<bool>,
}
#[derive(Debug)]
pub(crate) struct Request {
    receiver: Slot,
    aborting: Rc<Cell<bool>>,
    result: Cell<Option<Result<Slot, Slot>>>,
    changed: tokio::sync::watch::Sender<bool>,
}
impl Request {
    pub fn cancel(&self) {
        if self.result.get().is_none() {
            self.aborting.set(true);
        }
    }
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<bool> {
        self.changed.subscribe()
    }
    pub fn result(&self) -> Option<Result<Slot, Slot>> {
        self.result.get()
    }
    fn complete(&self, result: Result<Slot, Slot>) {
        self.result.set(Some(result));
        self.changed.send_replace(true);
    }
    pub fn roots(&self, roots: &mut Vec<Slot>) {
        roots.push(self.receiver);
        if let Some(result) = self.result.get() {
            roots.push(match result {
                Ok(v) | Err(v) => v,
            });
        }
    }
}
impl Stream {
    pub(crate) fn roots(&self, roots: &mut Vec<Slot>) {
        roots.push(self.init);
        roots.extend_from_slice(&self.entries);
        roots.extend(self.abort_future);
        roots.extend(self.aborted);
        for request in &self.waiters {
            request.roots(roots);
        }
    }
}
impl Heap {
    fn stream_poll(&mut self, receiver: Slot) -> Result<(), Error> {
        let state = self
            .streams
            .get(&receiver.bits)
            .ok_or_else(|| Error("invalid completion stream".into()))?;
        if !state.started || state.starting || state.aborting.get() {
            return Ok(());
        }
        let mut ready = Vec::new();
        for (index, &entry) in state.entries.iter().enumerate() {
            if state.emitted[index] {
                continue;
            }
            let entry = self.future(entry)?;
            if matches!(entry.state, State::Ready(_)) {
                let order = if entry.ready_order <= state.initial_order {
                    (0, 0, index)
                } else {
                    (1, entry.ready_order, index)
                };
                ready.push(order);
            }
        }
        ready.sort_unstable();
        let state = self.streams.get_mut(&receiver.bits).expect("stream");
        for (_, _, index) in ready {
            state.emitted[index] = true;
            state.queue.push_back(index);
        }
        while !state.queue.is_empty() && !state.waiters.is_empty() {
            let index = state.queue.pop_front().expect("queued completion");
            let waiter = state.waiters.pop_front().expect("queued request");
            state.delivered += 1;
            waiter.complete(Ok(state.entries[index]));
        }
        Ok(())
    }
    pub(crate) fn poll_stream(&mut self, future: Slot) -> Result<(), Error> {
        if let State::Stream(receiver) = self.future(future)?.state {
            self.stream_poll(receiver)?;
            self.settle_signal(future)?;
        }
        Ok(())
    }
    pub(crate) fn stream_operation(
        &mut self,
        op: Operation,
        args: [Option<Slot>; 3],
    ) -> Result<Slot, Error> {
        let receiver = args[0].ok_or_else(|| Error("missing completion stream receiver".into()))?;
        if matches!(op, Operation::BeginInit) {
            if self
                .streams
                .get(&receiver.bits)
                .is_some_and(|s| !s.init_failed)
            {
                return Ok(Slot::boolean(false));
            }
            let init = self.future_with_state(State::Running)?;
            self.futures.get_mut(init.bits)?.captures = vec![receiver].into_boxed_slice();
            self.streams.insert(
                receiver.bits,
                Stream {
                    init,
                    init_failed: false,
                    entries: vec![],
                    started: false,
                    starting: false,
                    initial_order: 0,
                    emitted: vec![],
                    queue: VecDeque::new(),
                    waiters: VecDeque::new(),
                    reserved: 0,
                    delivered: 0,
                    aborting: Rc::new(Cell::new(false)),
                    abort_future: None,
                    aborted: None,
                    cancelled: vec![],
                },
            );
            return Ok(Slot::boolean(true));
        }
        if !self.streams.contains_key(&receiver.bits) {
            return Err(Error("uninitialized completion stream".into()));
        }
        match op {
            Operation::CreateFuture(function) => self.future_new(function, &[receiver]),
            Operation::Cancelled => Ok(Slot::boolean(matches!(
                self.failure(args[1].ok_or_else(|| Error("missing stream failure".into()))?)?
                    .kind,
                FailureKind::Cancelled
            ))),
            Operation::InitFuture => Ok(self.streams[&receiver.bits].init),
            Operation::Init => {
                let entries = self
                    .elements(args[1].ok_or_else(|| Error("missing stream entries".into()))?)?
                    .to_vec();
                for &entry in &entries {
                    self.future(entry)?;
                }
                let state = self.streams.get_mut(&receiver.bits).expect("stream");
                state.emitted = vec![false; entries.len()];
                state.cancelled = vec![false; entries.len()];
                state.entries = entries;
                let init = state.init;
                self.future_state(init, State::Ready(Ok(Slot::boolean(true))))?;
                Ok(Slot::default())
            }
            Operation::InitFailed => {
                let failure =
                    args[1].ok_or_else(|| Error("missing initialization failure".into()))?;
                self.failure(failure)?;
                let state = self.streams.get_mut(&receiver.bits).expect("stream");
                state.init_failed = true;
                let init = state.init;
                self.future_state(init, State::Ready(Err(failure)))?;
                Ok(Slot::default())
            }
            Operation::Remaining => {
                self.stream_poll(receiver)?;
                let state = &self.streams[&receiver.bits];
                Ok(Slot::small((state.entries.len() - state.delivered) as i64))
            }
            Operation::Reserve(none) => {
                let state = &self.streams[&receiver.bits];
                if let Some(aborted) = state.aborted {
                    if state.abort_future.is_some_and(|f| {
                        matches!(self.future(f).map(|c| c.state), Ok(State::Ready(_)))
                    }) {
                        return self.future_with_state(State::Ready(Err(aborted)));
                    }
                }
                if state.reserved >= state.entries.len() {
                    let error = self.object_new(none, std::iter::empty())?;
                    let failed = self.failure_new(FailureKind::Thrown(error))?;
                    return self.future_with_state(State::Ready(Err(failed)));
                }
                let request = Rc::new(Request {
                    receiver,
                    aborting: state.aborting.clone(),
                    result: Cell::new(None),
                    changed: tokio::sync::watch::channel(false).0,
                });
                let future = self.future_with_state(State::Stream(receiver))?;
                let cell = self.futures.get_mut(future.bits)?;
                cell.captures = vec![receiver].into_boxed_slice();
                cell.request = Some(crate::signal::Request::Stream(request.clone()));
                let state = self.streams.get_mut(&receiver.bits).expect("stream");
                state.reserved += 1;
                state.waiters.push_back(request);
                self.poll_stream(future)?;
                Ok(future)
            }
            Operation::TryStart => {
                let state = self.streams.get_mut(&receiver.bits).expect("stream");
                if state.started || state.aborting.get() {
                    return Ok(Slot::boolean(false));
                }
                state.started = true;
                state.starting = true;
                let entries = state.entries.clone();
                for entry in entries {
                    self.futures.get_mut(entry.bits)?.observed = true;
                }
                Ok(Slot::boolean(true))
            }
            Operation::Entries => self.allocate_array(self.streams[&receiver.bits].entries.clone()),
            Operation::Started => {
                let state = self.streams.get_mut(&receiver.bits).expect("stream");
                state.starting = false;
                state.initial_order = self.completion_clock;
                self.stream_poll(receiver)?;
                Ok(Slot::default())
            }
            Operation::BeginAbort => {
                let state = &self.streams[&receiver.bits];
                if state.abort_future.is_some() {
                    return Ok(Slot::boolean(false));
                }
                let failure = args[1].ok_or_else(|| Error("missing stream cancellation".into()))?;
                self.failure(failure)?;
                let abort = self.future_with_state(State::Running)?;
                self.futures.get_mut(abort.bits)?.captures = vec![receiver].into_boxed_slice();
                let state = self.streams.get_mut(&receiver.bits).expect("stream");
                state.aborting.set(true);
                state.abort_future = Some(abort);
                state.aborted = Some(failure);
                let entries = state.entries.clone();
                let mut cancelled = std::collections::HashSet::new();
                for (index, entry) in entries.into_iter().enumerate() {
                    if cancelled.contains(&entry.bits)
                        || !matches!(self.future(entry)?.state, State::Ready(_))
                    {
                        cancelled.insert(entry.bits);
                        self.streams
                            .get_mut(&receiver.bits)
                            .expect("stream")
                            .cancelled[index] = true;
                        self.future_control(okkama_sparrow::FutOp::Cancel, entry)?;
                    }
                }
                Ok(Slot::boolean(true))
            }
            Operation::AbortFuture => self.streams[&receiver.bits]
                .abort_future
                .ok_or_else(|| Error("missing stream abort".into())),
            Operation::FinishAbort => {
                let state = &self.streams[&receiver.bits];
                let primary = state
                    .aborted
                    .ok_or_else(|| Error("missing stream cancellation".into()))?;
                let entries = state.entries.clone();
                let cancelled = state.cancelled.clone();
                let mut seen = std::collections::HashSet::from([primary.bits]);
                for (entry, owned) in entries.into_iter().zip(cancelled) {
                    let State::Ready(result) = self.future(entry)?.state else {
                        return Err(Error("stream cleanup still running".into()));
                    };
                    if let Err(failure) = result {
                        let record = self.failure(failure)?;
                        if owned
                            && matches!(record.kind, FailureKind::Cancelled)
                            && record.secondary.is_empty()
                        {
                            continue;
                        }
                        if seen.insert(failure.bits) {
                            self.failure_link(primary, failure, true)?;
                        }
                    }
                }
                let state = self.streams.get_mut(&receiver.bits).expect("stream");
                state.queue.clear();
                state.delivered = state.entries.len();
                state.reserved = state.entries.len();
                for waiter in state.waiters.drain(..) {
                    waiter.complete(Err(primary));
                }
                let abort = state.abort_future.expect("stream abort");
                self.future_state(abort, State::Ready(Err(primary)))?;
                Ok(Slot::default())
            }
            Operation::BeginInit => unreachable!(),
        }
    }
}

pub(crate) fn lower(program: &crate::Program, index: usize) -> Result<crate::FunctionPlan, Error> {
    use crate::{Op, Type, array::ArrayOp, plan::Plan, value::Arithmetic};
    use okkama_sparrow::{Cmp, Handler, Register, Target};
    let f = &program.functions[index];
    let intrinsic = f.intrinsic.as_ref().expect("stream intrinsic");
    let invalid = || {
        Error(format!(
            "invalid intrinsic signature/layout: {}",
            intrinsic.name
        ))
    };
    let completion = match intrinsic.name.as_str() {
        "okkama.collect.FutureCompletions.completion" => true,
        "okkama.collect.FutureCompletions.remaining_count" => false,
        _ => return Err(invalid()),
    };
    if f.params.len() != 1
        || f.is_async != completion
        || !intrinsic.helpers.is_empty()
        || intrinsic.objects.len() != usize::from(completion)
    {
        return Err(invalid());
    }
    let receiver = f.params[0].0 as usize;
    let Some(Type::Object(owner)) = program.ty(f.registers[receiver]) else {
        return Err(invalid());
    };
    let object = &program.objects[owner.0 as usize];
    if !object.name.starts_with("async.collect::FutureCompletions<")
        || object.fields.len() != 1
        || object.fields[0].0 != "origins"
        || object.destroy.is_some()
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
    let size = &declaration.methods[size_slot];
    let item = &declaration.methods[item_slot];
    if !size.params.is_empty()
        || size.result.and_then(|t| program.ty(t)) != Some(&Type::Int)
        || item.params.len() != 1
        || program.ty(item.params[0]) != Some(&Type::Int)
        || !matches!(
            item.result.and_then(|t| program.ty(t)),
            Some(Type::Future(Some(_)))
        )
    {
        return Err(invalid());
    }
    if completion {
        if f.result != item.result {
            return Err(invalid());
        }
        let none = &program.objects[intrinsic.objects[0].0 as usize];
        if none.name != "async.collect::NoFutures"
            || !none.fields.is_empty()
            || none.destroy.is_some()
        {
            return Err(invalid());
        }
    } else if f.result.and_then(|t| program.ty(t)) != Some(&Type::Int) {
        return Err(invalid());
    }
    let n = f.registers.len();
    let (cond, seq, size, i, one, builder, entries, child, pending, result, failure, temporary) = (
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
    );
    let op = |operation, dst, args| {
        Op::Intrinsic(crate::intrinsic::Operation::Stream(operation), dst, args)
    };
    let first = [Some(receiver), None, None];
    let mut p = Plan::default();
    p.push(op(Operation::BeginInit, cond, first));
    p.branch(cond, "init", "initialized");
    p.label("init");
    let init_start = p.ops.len() as u32;
    p.push(Op::ObjectGet(seq, receiver, 0));
    p.push(Op::CallRole {
        dst: Some(size),
        receiver: seq,
        role: role.0,
        slot: size_slot,
        args: vec![],
        tail: false,
    });
    p.push(Op::Intrinsic(
        crate::intrinsic::Operation::Collect(crate::collect::Operation::Size),
        size,
        [Some(receiver), Some(size), None],
    ));
    p.push(Op::Const(i, Slot::small(0)));
    p.push(Op::Const(one, Slot::small(1)));
    p.push(Op::Array(ArrayOp::BuilderNew, builder, [None; 3]));
    p.label("read");
    p.push(Op::Compare(Cmp::Lt, cond, i, size));
    p.branch(cond, "item", "read_done");
    p.label("item");
    p.push(Op::CallRole {
        dst: Some(child),
        receiver: seq,
        role: role.0,
        slot: item_slot,
        args: vec![i],
        tail: false,
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
    p.push(op(
        Operation::Init,
        temporary,
        [Some(receiver), Some(entries), None],
    ));
    let init_end = p.ops.len() as u32;
    p.jump("initialized");
    let init_failed = p.ops.len() as u32;
    p.push(op(
        Operation::InitFailed,
        temporary,
        [Some(receiver), Some(failure), None],
    ));
    p.push(Op::Raise(failure));
    p.label("initialized");
    p.push(op(Operation::InitFuture, temporary, first));
    p.push(Op::Await {
        dst: None,
        future: temporary,
    });
    let mut handlers = vec![Handler {
        start: init_start,
        end: init_end,
        target: Target {
            pc: init_failed,
            moves: vec![],
        },
        failure: Register(failure as u32),
    }];
    let mut cold_entry = 0;
    if !completion {
        p.push(op(Operation::Remaining, result, first));
        p.push(Op::Return(result));
    } else {
        // Origins are read at the method call, while reservation/start stay cold.
        p.push(op(Operation::CreateFuture(index), result, first));
        p.push(Op::Return(result));
        cold_entry = p.ops.len() as u32;
        p.push(op(
            Operation::Reserve(intrinsic.objects[0].0),
            pending,
            first,
        ));
        p.push(op(Operation::TryStart, cond, first));
        p.branch(cond, "start_init", "wait_one");
        p.label("start_init");
        p.push(op(Operation::Entries, entries, first));
        p.push(Op::Array(
            ArrayOp::Length,
            size,
            [Some(entries), None, None],
        ));
        p.push(Op::Const(i, Slot::small(0)));
        p.push(Op::Const(one, Slot::small(1)));
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
        p.push(op(Operation::Started, temporary, first));
        p.label("wait_one");
        let wait_pc = p.ops.len() as u32;
        p.push(Op::Await {
            dst: Some(result),
            future: pending,
        });
        p.push(Op::Return(result));
        let abort = p.ops.len() as u32;
        p.push(op(
            Operation::Cancelled,
            cond,
            [Some(receiver), Some(failure), None],
        ));
        p.branch(cond, "abort", "raise");
        p.label("raise");
        p.push(Op::Raise(failure));
        p.label("abort");
        p.push(op(
            Operation::BeginAbort,
            cond,
            [Some(receiver), Some(failure), None],
        ));
        p.branch(cond, "drain_init", "abort_wait");
        p.label("drain_init");
        p.push(op(Operation::Entries, entries, first));
        p.push(Op::Array(
            ArrayOp::Length,
            size,
            [Some(entries), None, None],
        ));
        p.push(Op::Const(i, Slot::small(0)));
        p.push(Op::Const(one, Slot::small(1)));
        p.label("drain");
        p.push(Op::Compare(Cmp::Lt, cond, i, size));
        p.branch(cond, "drain_child", "drained");
        p.label("drain_child");
        p.push(Op::Array(
            ArrayOp::Get,
            child,
            [Some(entries), Some(i), None],
        ));
        let child_pc = p.ops.len() as u32;
        p.push(Op::Await {
            dst: None,
            future: child,
        });
        let advance = p.ops.len() as u32;
        p.push(Op::Arithmetic(Arithmetic::Add, i, i, one));
        p.jump("drain");
        p.label("drained");
        p.push(op(Operation::FinishAbort, temporary, first));
        p.label("abort_wait");
        p.push(op(Operation::AbortFuture, temporary, first));
        p.push(Op::Await {
            dst: None,
            future: temporary,
        });
        p.push(Op::Raise(failure));
        handlers.extend([
            Handler {
                start: wait_pc,
                end: wait_pc + 1,
                target: Target {
                    pc: abort,
                    moves: vec![],
                },
                failure: Register(failure as u32),
            },
            Handler {
                start: child_pc,
                end: child_pc + 1,
                target: Target {
                    pc: advance,
                    moves: vec![],
                },
                failure: Register(temporary as u32),
            },
        ]);
    }
    let plan = p.finish();
    let live = crate::liveness::analyze(&plan, &handlers);
    let call_saved = crate::liveness::call_saved(&plan, &live, &handlers);
    Ok(crate::FunctionPlan {
        native: None,
        is_async: false,
        cold_entry,
        cached: false,
        symbol: f.symbol.clone(),
        recurrence_loop: false,
        elided_objects: 0,
        params: vec![receiver],
        plan,
        registers: n + 12,
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

    fn call(heap: &mut Heap, owner: Slot, op: Operation, value: Option<Slot>) -> Slot {
        heap.stream_operation(op, [Some(owner), value, None])
            .unwrap()
    }

    #[test]
    fn pending_request_keeps_stream_inputs_alive_and_completed_cycles_are_collected() {
        let mut heap = Heap::new(&[], &[]);
        let owner = heap.object_new(0, std::iter::empty()).unwrap();
        let child = heap.future_with_state(State::Running).unwrap();
        call(&mut heap, owner, Operation::BeginInit, None);
        let entries = heap.allocate_array(vec![child, child]).unwrap();
        call(&mut heap, owner, Operation::Init, Some(entries));
        let first = call(&mut heap, owner, Operation::Reserve(1), None);
        let second = call(&mut heap, owner, Operation::Reserve(1), None);
        call(&mut heap, owner, Operation::TryStart, None);
        call(&mut heap, owner, Operation::Started, None);
        heap.collect([first, second].into_iter()).unwrap();
        heap.future_state(child, State::Ready(Ok(Slot::small(42))))
            .unwrap();
        for request in [first, second] {
            heap.poll_stream(request).unwrap();
            assert!(
                matches!(heap.future(request).unwrap().state, State::Ready(Ok(v)) if v.bits == child.bits)
            );
        }
        assert_eq!(call(&mut heap, owner, Operation::Remaining, None).bits, 0);
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.streams.is_empty());
        assert!(heap.future(child).is_err());
    }

    #[test]
    fn cancellation_blocks_handoff_until_every_input_has_finished_cleanup() {
        let mut heap = Heap::new(&[], &[]);
        let owner = heap.object_new(0, std::iter::empty()).unwrap();
        let child = heap.future_with_state(State::Running).unwrap();
        call(&mut heap, owner, Operation::BeginInit, None);
        let entries = heap.allocate_array(vec![child, child]).unwrap();
        call(&mut heap, owner, Operation::Init, Some(entries));
        let first = call(&mut heap, owner, Operation::Reserve(1), None);
        let second = call(&mut heap, owner, Operation::Reserve(1), None);
        call(&mut heap, owner, Operation::TryStart, None);
        call(&mut heap, owner, Operation::Started, None);
        heap.future_control(okkama_sparrow::FutOp::Cancel, first)
            .unwrap();
        let State::Ready(Err(primary)) = heap.future(first).unwrap().state else {
            panic!("cancelled request");
        };
        call(&mut heap, owner, Operation::BeginAbort, Some(primary));
        assert!(
            heap.stream_operation(Operation::FinishAbort, [Some(owner), None, None])
                .is_err()
        );
        assert!(matches!(
            heap.future(second).unwrap().state,
            State::Stream(_)
        ));
        let cleanup = heap.failure_new(FailureKind::Cancelled).unwrap();
        heap.future_state(child, State::Ready(Err(cleanup)))
            .unwrap();
        heap.poll_stream(second).unwrap();
        assert!(matches!(
            heap.future(second).unwrap().state,
            State::Stream(_)
        ));
        call(&mut heap, owner, Operation::FinishAbort, None);
        heap.poll_stream(second).unwrap();
        assert!(
            matches!(heap.future(second).unwrap().state, State::Ready(Err(v)) if v.bits == primary.bits)
        );
        assert!(heap.failure(primary).unwrap().secondary.is_empty());
    }
}
