//! Channel policy (including Okkama's zero-capacity rendezvous) is local state.
//! Tokio watch supplies the wakeups; no executor, runnable queue or coroutine
//! stack lives here. Buffered values and queued payloads remain visible to GC.
use crate::{
    Error,
    value::{Heap, Slot},
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::{Rc, Weak},
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Operation {
    New(u32),
    Endpoint(u32),
    Unattached(u32),
    Send(u32),
    Receive(u32),
    Close(u32),
}

#[derive(Debug)]
pub(crate) struct Channel {
    capacity: usize,
    closed: bool,
    messages: VecDeque<Slot>,
    senders: VecDeque<Rc<Request>>,
    receivers: VecDeque<Rc<Request>>,
}
#[derive(Debug)]
pub(crate) struct Request {
    channel: Weak<RefCell<Channel>>,
    value: Cell<Option<Slot>>,
    receiver: Slot,
    result: Cell<Option<Result<Slot, u32>>>,
    cancelled: Cell<bool>,
    changed: tokio::sync::watch::Sender<bool>,
}
impl Request {
    fn new(channel: &Rc<RefCell<Channel>>, receiver: Slot, value: Option<Slot>) -> Rc<Self> {
        Rc::new(Self {
            channel: Rc::downgrade(channel),
            receiver,
            value: Cell::new(value),
            result: Cell::new(None),
            cancelled: Cell::new(false),
            changed: tokio::sync::watch::channel(false).0,
        })
    }
    fn complete(&self, result: Result<Slot, u32>) {
        debug_assert!(!self.cancelled.get() && self.result.get().is_none());
        self.value.set(None);
        self.result.set(Some(result));
        self.changed.send_replace(true);
    }
    pub(crate) fn cancel(&self) {
        if self.result.get().is_some() || self.cancelled.replace(true) {
            return;
        }
        self.value.set(None);
        if let Some(channel) = self.channel.upgrade() {
            let mut channel = channel.borrow_mut();
            channel.senders.retain(|r| !r.cancelled.get());
            channel.receivers.retain(|r| !r.cancelled.get());
        }
    }
    pub(crate) fn result(&self) -> Option<Result<Slot, u32>> {
        self.result.get()
    }
    pub(crate) fn subscribe(&self) -> tokio::sync::watch::Receiver<bool> {
        self.changed.subscribe()
    }
    pub(crate) fn roots(&self, roots: &mut Vec<Slot>) {
        if self.cancelled.get() {
            return;
        }
        roots.push(self.receiver);
        roots.extend(self.value.get());
        if let Some(Ok(value)) = self.result.get() {
            roots.push(value);
        }
    }
}
impl Channel {
    pub(crate) fn roots(&self, roots: &mut Vec<Slot>) {
        roots.extend(self.messages.iter().copied());
        for request in self.senders.iter().chain(&self.receivers) {
            request.roots(roots);
        }
    }
    fn send(&mut self, request: Rc<Request>, closed: u32) {
        if self.closed {
            request.complete(Err(closed));
        } else if let Some(receiver) = self.receivers.pop_front() {
            receiver.complete(Ok(request.value.get().expect("send payload")));
            request.complete(Ok(request.receiver));
        } else if self.messages.len() < self.capacity {
            self.messages
                .push_back(request.value.get().expect("send payload"));
            request.complete(Ok(request.receiver));
        } else {
            self.senders.push_back(request);
        }
    }
    fn receive(&mut self, request: Rc<Request>, closed: u32) {
        if let Some(value) = self.messages.pop_front() {
            if let Some(sender) = self.senders.pop_front() {
                debug_assert!(!self.closed);
                self.messages
                    .push_back(sender.value.get().expect("send payload"));
                sender.complete(Ok(sender.receiver));
            }
            request.complete(Ok(value));
        } else if let Some(sender) = self.senders.pop_front() {
            request.complete(Ok(sender.value.get().expect("send payload")));
            sender.complete(Ok(sender.receiver));
        } else if self.closed {
            request.complete(Err(closed));
        } else {
            self.receivers.push_back(request);
        }
    }
    fn close(&mut self, closed: u32) {
        if std::mem::replace(&mut self.closed, true) {
            return;
        }
        for request in self.senders.drain(..).chain(self.receivers.drain(..)) {
            request.complete(Err(closed));
        }
    }
}
impl Heap {
    pub(crate) fn channel_operation(
        &mut self,
        op: Operation,
        args: [Option<Slot>; 3],
    ) -> Result<Slot, Error> {
        if let Operation::Unattached(owner) = op {
            return self.object_new(owner, std::iter::empty());
        }
        let receiver = args[0].ok_or_else(|| Error("missing channel operand".into()))?;
        if let Operation::New(owner) = op {
            let capacity = self
                .integer(receiver)?
                .to_i64()
                .filter(|n| (0..=i32::MAX as i64).contains(n))
                .ok_or_else(|| Error("Channel capacity is too large".into()))?;
            let value = self.object_new(owner, std::iter::empty())?;
            self.channels.insert(
                value.bits,
                Rc::new(RefCell::new(Channel {
                    capacity: capacity as usize,
                    closed: false,
                    messages: VecDeque::new(),
                    senders: VecDeque::new(),
                    receivers: VecDeque::new(),
                })),
            );
            return Ok(value);
        }
        self.object_descriptor(receiver)?;
        let channel = self
            .channels
            .get(&receiver.bits)
            .cloned()
            .ok_or_else(|| Error("Invalid channel endpoint".into()))?;
        match op {
            Operation::Endpoint(owner) => {
                let value = self.object_new(owner, std::iter::empty())?;
                self.channels.insert(value.bits, channel);
                Ok(value)
            }
            Operation::Close(closed) => {
                channel.borrow_mut().close(closed);
                Ok(Slot::default())
            }
            Operation::Send(closed) | Operation::Receive(closed) => {
                let sending = matches!(op, Operation::Send(_));
                let value = if sending {
                    Some(args[1].ok_or_else(|| Error("missing channel payload".into()))?)
                } else {
                    None
                };
                let request = Request::new(&channel, receiver, value);
                if sending {
                    channel.borrow_mut().send(request.clone(), closed);
                } else {
                    channel.borrow_mut().receive(request.clone(), closed);
                }
                let future = self.future_with_state(crate::future::State::Signal(receiver))?;
                let cell = self.futures.get_mut(future.bits)?;
                cell.captures = args.into_iter().flatten().collect();
                cell.request = Some(crate::signal::Request::Channel(request));
                self.settle_signal(future)?;
                Ok(future)
            }
            _ => unreachable!(),
        }
    }
}

pub(crate) fn lower(program: &crate::Program, index: usize) -> Result<crate::FunctionPlan, Error> {
    use crate::{Op, Type};
    use okkama_sparrow::{Cmp, Target};
    let f = &program.functions[index];
    let intrinsic = f.intrinsic.as_ref().expect("channel intrinsic");
    let invalid = || {
        Error(format!(
            "invalid intrinsic signature/layout: {}",
            intrinsic.name
        ))
    };
    let params: Vec<_> = f.params.iter().map(|r| r.0 as usize).collect();
    let types: Vec<_> = params.iter().map(|&r| program.ty(f.registers[r])).collect();
    let result = f.result.and_then(|t| program.ty(t));
    // A single native channel transports one T. Check all emitted endpoint
    // contracts sharing that specialization, including synthetic native bodies.
    let receiver_type = types
        .first()
        .copied()
        .flatten()
        .filter(|t| matches!(t, Type::Object(_)))
        .or(result);
    if let Some(Type::Object(owner)) = receiver_type {
        let suffix = program.objects[owner.0 as usize]
            .name
            .split_once('<')
            .map(|(_, suffix)| suffix);
        let mut element = None;
        for peer in &program.functions {
            let Some(metadata) = &peer.intrinsic else {
                continue;
            };
            if !matches!(
                metadata.name.as_str(),
                "okkama.channel.Sending.send" | "okkama.channel.Receiving.item"
            ) {
                continue;
            }
            let Some(receiver) = peer.params.first() else {
                continue;
            };
            let Some(Type::Object(owner)) = program.ty(peer.registers[receiver.0 as usize]) else {
                continue;
            };
            if program.objects[owner.0 as usize]
                .name
                .split_once('<')
                .map(|(_, suffix)| suffix)
                != suffix
            {
                continue;
            }
            let ty = if metadata.name.ends_with(".send") {
                peer.params.get(1).map(|r| peer.registers[r.0 as usize])
            } else {
                peer.result
            }
            .ok_or_else(invalid)?;
            if let Some(expected) = element {
                if program.ty(expected) != program.ty(ty) {
                    return Err(invalid());
                }
            }
            element = Some(ty);
        }
    }
    let owner = |ty: Option<&Type>, prefix: &str| -> Result<u32, Error> {
        let Some(Type::Object(owner)) = ty else {
            return Err(invalid());
        };
        let object = &program.objects[owner.0 as usize];
        if !object.name.starts_with(prefix)
            || !object.name.ends_with('>')
            || object.destroy.is_some()
            || !object.fields.is_empty()
        {
            return Err(invalid());
        }
        Ok(owner.0)
    };
    let related = |at: usize, name: &str, fields: usize| -> Result<u32, Error> {
        let id = intrinsic.objects.get(at).ok_or_else(invalid)?.0;
        let object = &program.objects[id as usize];
        if object.name != name || object.fields.len() != fields || object.destroy.is_some() {
            return Err(invalid());
        }
        if fields == 1
            && (object.fields[0].0 != "capacity"
                || program.ty(object.fields[0].1) != Some(&Type::Int))
        {
            return Err(invalid());
        }
        Ok(id)
    };
    if !intrinsic.helpers.is_empty() {
        return Err(invalid());
    }
    let name = intrinsic.name.as_str();
    let (op, is_async) = match name {
        "okkama.channel.Channel.new_capacity" => {
            if types != vec![Some(&Type::Int)] || intrinsic.objects.len() != 2 {
                return Err(invalid());
            }
            let id = owner(result, "async.channel::Channel<")?;
            if intrinsic.objects[0].0 != id {
                return Err(invalid());
            }
            related(1, "async.channel::InvalidChannelCapacity", 1)?;
            (Operation::New(id), false)
        }
        "okkama.channel.Sending.new" | "okkama.channel.Receiving.new" => {
            if !params.is_empty() || intrinsic.objects.len() != 1 {
                return Err(invalid());
            }
            let prefix = if name.contains("Sending") {
                "async.channel::Sending<"
            } else {
                "async.channel::Receiving<"
            };
            let id = owner(result, prefix)?;
            if intrinsic.objects[0].0 != id {
                return Err(invalid());
            }
            (Operation::Unattached(id), false)
        }
        "okkama.channel.Channel.sending" | "okkama.channel.Channel.receiving" => {
            if params.len() != 1 || intrinsic.objects.len() != 1 {
                return Err(invalid());
            }
            let from = owner(types[0], "async.channel::Channel<")?;
            let prefix = if name.ends_with("sending") {
                "async.channel::Sending<"
            } else {
                "async.channel::Receiving<"
            };
            let id = owner(result, prefix)?;
            if intrinsic.objects[0].0 != id
                || program.objects[from as usize]
                    .name
                    .split_once('<')
                    .map(|p| p.1)
                    != program.objects[id as usize]
                        .name
                        .split_once('<')
                        .map(|p| p.1)
            {
                return Err(invalid());
            }
            (Operation::Endpoint(id), false)
        }
        "okkama.channel.Sending.send"
        | "okkama.channel.Receiving.item"
        | "okkama.channel.Sending.close"
        | "okkama.channel.Receiving.close" => {
            let send = name.ends_with(".send");
            let close = name.ends_with(".close");
            if params.len() != if send { 2 } else { 1 } || intrinsic.objects.len() != 1 {
                return Err(invalid());
            }
            let prefix = if name.contains("Sending") {
                "async.channel::Sending<"
            } else {
                "async.channel::Receiving<"
            };
            owner(types[0], prefix)?;
            let closed = related(0, "async.channel::ChannelClosed", 0)?;
            if close {
                if result.is_some() {
                    return Err(invalid());
                }
                (Operation::Close(closed), false)
            } else {
                if (send && result != types[0]) || (!send && result.is_none()) {
                    return Err(invalid());
                }
                for ty in f.registers.iter().chain(&f.result) {
                    if crate::kind(program, *ty).is_none() {
                        return Err(invalid());
                    }
                }
                (
                    if send {
                        Operation::Send(closed)
                    } else {
                        Operation::Receive(closed)
                    },
                    true,
                )
            }
        }
        _ => return Err(invalid()),
    };
    if f.is_async != is_async {
        return Err(invalid());
    }
    let output = f.registers.len();
    let args = std::array::from_fn(|i| params.get(i).copied());
    let invoke = Op::Intrinsic(crate::intrinsic::Operation::Channel(op), output, args);
    let (plan, registers) = if matches!(op, Operation::New(_)) {
        (
            vec![
                Op::Const(output, Slot::small(0)),
                Op::Compare(Cmp::Lt, output + 1, params[0], output),
                Op::Branch(
                    output + 1,
                    Target {
                        pc: 5,
                        moves: vec![],
                    },
                    Target {
                        pc: 3,
                        moves: vec![],
                    },
                ),
                invoke,
                Op::Return(output),
                Op::ObjectNew(output, intrinsic.objects[1].0, vec![params[0]]),
                Op::Throw(output),
            ],
            output + 2,
        )
    } else if is_async {
        (
            vec![
                invoke,
                Op::Await {
                    dst: Some(output + 1),
                    future: output,
                },
                Op::Return(output + 1),
            ],
            output + 2,
        )
    } else {
        (
            vec![
                invoke,
                if result.is_some() {
                    Op::Return(output)
                } else {
                    Op::ReturnVoid
                },
            ],
            output + 1,
        )
    };
    let live = crate::liveness::analyze(&plan, &[]);
    let call_saved = crate::liveness::call_saved(&plan, &live, &[]);
    Ok(crate::FunctionPlan {
        native: None,
        is_async,
        cold_entry: 0,
        cached: false,
        symbol: f.symbol.clone(),
        recurrence_loop: false,
        elided_objects: 0,
        params,
        plan,
        registers,
        compiled: Default::default(),
        jit_kinds: None,
        live,
        call_saved,
        handlers: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn call(heap: &mut Heap, op: Operation, a: Slot, b: Option<Slot>) -> Slot {
        heap.channel_operation(op, [Some(a), b, None]).unwrap()
    }
    #[test]
    fn channel_gc_retains_buffered_values_and_cancellation_dequeues_immediately() {
        let mut heap = Heap::new(&[], &[]);
        let owner = call(&mut heap, Operation::New(0), Slot::small(1), None);
        let input = call(&mut heap, Operation::Endpoint(1), owner, None);
        let output = call(&mut heap, Operation::Endpoint(2), owner, None);
        let payload = heap.object_new(3, [Slot::small(42)].into_iter()).unwrap();
        call(&mut heap, Operation::Send(4), output, Some(payload));
        // Only the endpoint remains. The buffer is a real GC edge.
        heap.collect([input].into_iter()).unwrap();
        assert_eq!(heap.object_get(payload, 0).unwrap().bits, 42);
        let found = call(&mut heap, Operation::Receive(4), input, None);
        assert!(
            matches!(heap.future(found).unwrap().state,crate::future::State::Ready(Ok(v)) if v.bits==payload.bits && v.tag==payload.tag)
        );
        for _ in 0..1000 {
            let waiting = call(&mut heap, Operation::Receive(4), input, None);
            heap.future_control(okkama_sparrow::FutOp::Cancel, waiting)
                .unwrap();
            assert!(heap.channels[&input.bits].borrow().receivers.is_empty());
        }
        heap.collect([input].into_iter()).unwrap();
        assert!(heap.object_get(payload, 0).is_err());
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.channels.is_empty());
    }
    #[test]
    fn completed_handoff_cannot_be_undone_by_cancelling_the_native_wait() {
        let mut heap = Heap::new(&[], &[]);
        let channel = call(&mut heap, Operation::New(0), Slot::small(0), None);
        let waiting = call(
            &mut heap,
            Operation::Send(1),
            channel,
            Some(Slot::small(42)),
        );
        let received = call(&mut heap, Operation::Receive(1), channel, None);
        heap.future_control(okkama_sparrow::FutOp::Cancel, waiting)
            .unwrap();
        assert!(
            matches!(heap.future(received).unwrap().state,crate::future::State::Ready(Ok(v)) if v.bits==42)
        );
        assert!(heap.channels[&channel.bits].borrow().messages.is_empty());
        assert!(heap.channels[&channel.bits].borrow().senders.is_empty());
    }
    #[test]
    fn closing_rejects_blocked_senders_but_preserves_already_buffered_values() {
        let mut heap = Heap::new(&[], &[]);
        let channel = call(&mut heap, Operation::New(0), Slot::small(1), None);
        call(
            &mut heap,
            Operation::Send(1),
            channel,
            Some(Slot::small(42)),
        );
        let blocked = call(
            &mut heap,
            Operation::Send(1),
            channel,
            Some(Slot::small(99)),
        );
        call(&mut heap, Operation::Close(1), channel, None);
        heap.settle_signal(blocked).unwrap();
        assert!(matches!(
            heap.future(blocked).unwrap().state,
            crate::future::State::Ready(Err(_))
        ));
        let value = call(&mut heap, Operation::Receive(1), channel, None);
        assert!(
            matches!(heap.future(value).unwrap().state,crate::future::State::Ready(Ok(v)) if v.bits==42)
        );
        let empty = call(&mut heap, Operation::Receive(1), channel, None);
        assert!(matches!(
            heap.future(empty).unwrap().state,
            crate::future::State::Ready(Err(_))
        ));
    }
}
