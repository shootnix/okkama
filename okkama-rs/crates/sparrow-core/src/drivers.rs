//! Drivers: the bodies of async natives and intrinsics (see `tasks`).
//!
//! A driver runs on each (re)entry of its frame and returns a [`Step`]. Simple
//! waits keep their progress in the frame's resume word (`primitives`
//! codes); structured ones (groups, collections, timeouts) keep a payload in
//! the driver word after the registers. Semantics follow the previous
//! engine's lowered plans (`group.rs`, `collect.rs`, `timeout.rs`, `channel.rs`,
//! `barrier.rs`, `semaphore.rs`, the `sleep_nanoseconds` native).
#![allow(unsafe_code)]

use crate::heap::Payload;
use crate::intrinsics::{Intrinsic, call_function, call_role, thrown};
use crate::primitives::{COMPLETED, REFUSED, State, WAITING};
use crate::program::Kind;
use crate::runtime::{Ctx, FAILED, FailureKind, OK, Runtime};
use crate::tasks::{self, DELIVER_NOW, DONE, FAILURE_STATE, FLAGS, PARKED, REGISTERS, RESULT, RESUME};
use crate::word::{Repr, UNBOUND, Word};

#[derive(Debug)]
pub enum Step {
    Pending,
    Done(Word),
    Failed(Word),
    /// The engine stopped (status FATAL is set).
    Stop,
}
use Step::{Done, Failed, Pending, Stop};

/// Run driver `frame`; true while it is pending.
pub fn drive(runtime: &mut Runtime, ctx: *mut Ctx, frame: Word) -> bool {
    let flags = tasks::get(frame, FLAGS);
    let deliver = flags & DELIVER_NOW != 0;
    tasks::set(frame, FLAGS, flags & !(DELIVER_NOW | PARKED));
    let function = runtime.function_of(frame);
    let source = &runtime.loaded.program.functions[function];
    let step = if source.native.is_some() {
        native(runtime, ctx, frame, function, deliver)
    } else {
        match runtime.intrinsics[function] {
            Some(Intrinsic::Worker) => crate::worker::drive(runtime, frame, deliver),
            Some(intrinsic) => intrinsic_step(runtime, ctx, frame, function, intrinsic, deliver),
            None => {
                let message = format!(
                    "intrinsic {} is not available in sparrow-core yet",
                    source.intrinsic.as_ref().map_or("", |i| i.name.as_str())
                );
                crate::runtime::fatal(unsafe { &mut *ctx }, runtime, message);
                Stop
            }
        }
    };
    match step {
        Pending => {
            tasks::set(frame, FLAGS, tasks::get(frame, FLAGS) | PARKED);
            true
        }
        Done(value) => {
            runtime.settle(frame, DONE, value);
            false
        }
        Failed(failure) => {
            runtime.settle(frame, FAILURE_STATE, failure);
            false
        }
        Stop => false,
    }
}

fn param(runtime: &Runtime, frame: Word, function: usize, index: usize) -> Word {
    let register = runtime.loaded.program.functions[function].params[index].0;
    tasks::get(frame, REGISTERS + u64::from(register) * 8)
}

fn resume(frame: Word) -> u64 {
    tasks::get(frame, RESUME) & 0xff
}

fn wait(frame: Word, extra: u64) {
    tasks::set(frame, RESUME, WAITING | (extra << 8));
}

fn fail(runtime: &mut Runtime, kind: FailureKind) -> Step {
    Failed(runtime.failure_new(kind))
}

fn cancelled(runtime: &mut Runtime) -> Step {
    Failed(runtime.cancelled_failure())
}

/// The failure a call left in the context (or a stop).
fn call_failed(ctx: *mut Ctx) -> Result<Word, ()> {
    let ctx = unsafe { &mut *ctx };
    if ctx.status != FAILED {
        return Err(());
    }
    ctx.status = OK;
    Ok(std::mem::replace(&mut ctx.failure, UNBOUND))
}

fn native(runtime: &mut Runtime, ctx: *mut Ctx, frame: Word, function: usize, deliver: bool) -> Step {
    let symbol = runtime.loaded.program.functions[function].native.clone().unwrap_or_default();
    let Some(native) = okkama_runtime_abi::Native::resolve(&symbol) else {
        return fail(runtime, FailureKind::Program(crate::natives::missing_native(&symbol)));
    };
    if let Err(message) = runtime.limits.permits(native) {
        return fail(runtime, FailureKind::Program(message));
    }
    if native != okkama_runtime_abi::Native::TimeSleepNanoseconds {
        if !crate::host::supported(native) {
            let message = format!("native provider {symbol} is not available in sparrow-core yet");
            crate::runtime::fatal(unsafe { &mut *ctx }, runtime, message);
            return Stop;
        }
        return native_io(runtime, frame, function, native, deliver);
    }
    match resume(frame) {
        0 => {
            let source = runtime.heap.text(param(runtime, frame, function, 0));
            let canonical = source == "0"
                || (!source.is_empty() && !source.starts_with('0') && source.bytes().all(|b| b.is_ascii_digit()));
            let message = if !canonical {
                "Wait duration must be a canonical non-negative integer"
            } else {
                match source.parse::<i64>() {
                    Ok(0) => return Done(1),
                    Ok(nanos) => {
                        let deadline = std::time::Instant::now() + std::time::Duration::from_nanos(nanos as u64);
                        runtime.timers.push((deadline, frame));
                        wait(frame, 0);
                        return Pending;
                    }
                    Err(_) => "Wait duration exceeds the runtime timer range",
                }
            };
            // Typed native failures arrive with the provider port (stage 4).
            fail(runtime, FailureKind::Program(message.into()))
        }
        _ if deliver => {
            runtime.timers.retain(|&(_, f)| f != frame);
            cancelled(runtime)
        }
        _ => Done(1),
    }
}

/// An async native provider: start its request, then poll it on each wake.
fn native_io(runtime: &mut Runtime, frame: Word, function: usize, native: okkama_runtime_abi::Native, deliver: bool) -> Step {
    use crate::host::Start;
    let started = match runtime.io.requests.get(&frame) {
        Some(&id) => {
            if deliver && !crate::host::cancel(runtime, id) {
                runtime.io.requests.remove(&frame);
                return cancelled(runtime);
            }
            let waker = runtime.io.waker(frame);
            match crate::host::poll(runtime, native, id, &waker) {
                Some(outcome) => outcome,
                None => return Pending,
            }
        }
        None => {
            let count = runtime.loaded.program.functions[function].params.len();
            let arguments: Vec<Word> = (0..count).map(|i| param(runtime, frame, function, i)).collect();
            match crate::host::start(runtime, native, &arguments) {
                Start::Pending(id) => {
                    // An external request always suspends its caller once, even
                    // if its reply is already there (a fast worker): `spawn`
                    // returns before it, so a cancellation right after `spawn`
                    // is observed, as on the other engines. The frame is woken
                    // at once and polls the request then.
                    runtime.io.requests.insert(frame, id);
                    runtime.io.waker(frame).wake();
                    return Pending;
                }
                outcome => outcome,
            }
        }
    };
    runtime.io.requests.remove(&frame);
    match started {
        Start::Done(value) => Done(value),
        Start::Raise(failure) => Failed(failure),
        Start::Refused(message) => fail(runtime, FailureKind::Program(message)),
        Start::Pending(_) => Pending,
    }
}

fn intrinsic_step(runtime: &mut Runtime, ctx: *mut Ctx, frame: Word, function: usize, intrinsic: Intrinsic, deliver: bool) -> Step {
    use Intrinsic::*;
    let receiver = param(runtime, frame, function, 0);
    match intrinsic {
        EventWait | TokenWait => {
            let Some(event) = runtime.event_of(receiver) else {
                let message = if intrinsic == EventWait { "Invalid async event" } else { "Invalid cancellation capability" };
                return fail(runtime, FailureKind::Program(message.into()));
            };
            if deliver {
                runtime.event_unwait(&event, frame);
                return cancelled(runtime);
            }
            if runtime.event_ready(&event) {
                return Done(receiver);
            }
            runtime.event_wait(&event, frame);
            Pending
        }
        LockGuard | SemaphorePermit => {
            let Some(pool) = runtime.pool_of(receiver, intrinsic == SemaphorePermit) else {
                let message = if intrinsic == LockGuard { "Invalid async lock" } else { "Cannot read field amount" };
                return fail(runtime, FailureKind::Program(message.into()));
            };
            let granted = match resume(frame) {
                0 => {
                    let mut p = pool.borrow_mut();
                    if p.remaining > 0 && p.queue.is_empty() {
                        p.remaining -= 1;
                        true
                    } else {
                        p.queue.push_back(frame);
                        false
                    }
                }
                COMPLETED => true,
                _ if deliver => {
                    pool.borrow_mut().queue.retain(|&w| w != frame);
                    return cancelled(runtime);
                }
                _ => false,
            };
            if !granted {
                wait(frame, 0);
                return Pending;
            }
            let owner = runtime.loaded.program.functions[function].intrinsic.as_ref().expect("intrinsic").objects[0].0;
            let guard = runtime.heap.object_new(owner, &[]);
            runtime.state_new(guard, State::Grant { pool, closed: false });
            Done(guard)
        }
        BarrierWait => {
            let Some(barrier) = runtime.barrier_of(receiver) else {
                return fail(runtime, FailureKind::Program("Cannot read field amount".into()));
            };
            match resume(frame) {
                0 => {
                    let mut b = barrier.borrow_mut();
                    if b.broken {
                        drop(b);
                        return Failed(failure_of(runtime, function, 0));
                    }
                    let index = b.waiting.len() as i64;
                    b.waiting.push(frame);
                    if b.waiting.len() as i64 == b.parties {
                        b.generation += 1;
                        let waiting = std::mem::take(&mut b.waiting);
                        drop(b);
                        for (i, waiter) in waiting.into_iter().enumerate() {
                            if waiter != frame {
                                let arrival = runtime.heap.int(i as i64);
                                runtime.complete_wait(waiter, COMPLETED, Some(arrival));
                            }
                        }
                        return Done(runtime.heap.int(index));
                    }
                    let generation = b.generation as u64;
                    drop(b);
                    tasks::set(frame, RESULT, runtime.heap.int(index));
                    wait(frame, generation);
                    Pending
                }
                COMPLETED => Done(tasks::get(frame, RESULT)),
                REFUSED => Failed(failure_of(runtime, function, 0)),
                _ if deliver => {
                    let generation = (tasks::get(frame, RESUME) >> 8) as i64;
                    barrier.borrow_mut().waiting.retain(|&w| w != frame);
                    if barrier.borrow().generation == generation {
                        runtime.barrier_break(&barrier);
                    }
                    cancelled(runtime)
                }
                _ => Pending,
            }
        }
        Send | Receive => channel(runtime, frame, function, intrinsic == Send, deliver),
        GroupWait | ScopeResult => group_wait(runtime, ctx, frame, function, intrinsic == ScopeResult, deliver),
        CollectAll | CollectRace | CollectFirst | TimeoutBefore | TimeoutWithin => {
            collect(runtime, ctx, frame, function, intrinsic, deliver)
        }
        StreamCompletion => completion(runtime, ctx, frame, function, deliver),
        _ => {
            crate::runtime::fatal(unsafe { &mut *ctx }, runtime, format!("intrinsic {intrinsic:?} is not async"));
            Stop
        }
    }
}

/// A failure throwing the `index`th related object of `function`.
fn failure_of(runtime: &mut Runtime, function: usize, index: usize) -> Word {
    let kind = thrown(runtime, function, index, &[]);
    runtime.failure_new(kind)
}

fn channel(runtime: &mut Runtime, frame: Word, function: usize, send: bool, deliver: bool) -> Step {
    let endpoint = param(runtime, frame, function, 0);
    let Some(channel) = runtime.channel_of(endpoint) else {
        return fail(runtime, FailureKind::Program("Invalid channel endpoint".into()));
    };
    let send_function = |runtime: &Runtime, sender: Word| {
        let function = runtime.function_of(sender);
        param(runtime, sender, function, 1)
    };
    match resume(frame) {
        0 => {}
        COMPLETED if send => return Done(endpoint),
        COMPLETED => return Done(tasks::get(frame, RESULT)),
        REFUSED => return Failed(failure_of(runtime, function, 0)),
        _ if deliver => {
            let mut c = channel.borrow_mut();
            c.senders.retain(|&w| w != frame);
            c.receivers.retain(|&w| w != frame);
            drop(c);
            return cancelled(runtime);
        }
        _ => return Pending,
    }
    // Sending<T> and Receiving<T> of one channel agree on T only by the
    // program's types; the core holds them to one representation.
    let repr = if send {
        param_kind(runtime, function, 1).repr()
    } else {
        runtime.loaded.functions[function].result.map_or(Repr::Raw, Kind::repr)
    };
    if *channel.borrow_mut().repr.get_or_insert(repr) != repr {
        return fail(runtime, FailureKind::Program("Channel endpoints disagree on the message type".into()));
    }
    if send {
        let value = param(runtime, frame, function, 1);
        let mut c = channel.borrow_mut();
        if c.closed {
            drop(c);
            return Failed(failure_of(runtime, function, 0));
        }
        if let Some(receiver) = Runtime::pop_parked(&mut c.receivers) {
            drop(c);
            runtime.complete_wait(receiver, COMPLETED, Some(value));
            return Done(endpoint);
        }
        if c.messages.len() < c.capacity {
            c.messages.push_back(value);
            return Done(endpoint);
        }
        c.senders.push_back(frame);
        drop(c);
        wait(frame, 0);
        return Pending;
    }
    let mut c = channel.borrow_mut();
    if let Some(value) = c.messages.pop_front() {
        if let Some(sender) = Runtime::pop_parked(&mut c.senders) {
            let payload = send_function(runtime, sender);
            c.messages.push_back(payload);
            drop(c);
            runtime.complete_wait(sender, COMPLETED, None);
        }
        return Done(value);
    }
    if let Some(sender) = Runtime::pop_parked(&mut c.senders) {
        drop(c);
        let value = send_function(runtime, sender);
        runtime.complete_wait(sender, COMPLETED, None);
        return Done(value);
    }
    if c.closed {
        drop(c);
        return Failed(failure_of(runtime, function, 0));
    }
    c.receivers.push_back(frame);
    drop(c);
    wait(frame, 0);
    Pending
}

fn param_kind(runtime: &Runtime, function: usize, index: usize) -> Kind {
    let f = &runtime.loaded.program.functions[function];
    runtime.loaded.functions[function].kinds[f.params[index].0 as usize]
}

// ---- structured drivers: state payloads ----

fn state<T: Payload>(runtime: &mut Runtime, frame: Word) -> Option<&mut T> {
    let slot = tasks::get(frame, runtime.driver_slot(frame));
    if slot == UNBOUND {
        return None;
    }
    runtime.heap.payload_mut::<T>(slot)
}

fn set_state(runtime: &mut Runtime, frame: Word, payload: Box<dyn Payload>) {
    let object = runtime.heap.allocate_payload(payload);
    let slot = runtime.driver_slot(frame);
    tasks::set(frame, slot, object);
}

#[derive(Debug)]
struct GroupWait {
    /// 0 start, 1 body pending (scope), 2 waiting for members.
    phase: u8,
    group: Word,
    body: Word,
    value: Word,
    value_repr: Repr,
    external: Option<Word>,
    entries: Vec<Word>,
    index: usize,
}
impl Payload for GroupWait {
    fn trace(&self, visit: &mut dyn FnMut(Word, Repr)) {
        visit(self.group, Repr::Ref);
        visit(self.body, Repr::Ref);
        visit(self.value, self.value_repr);
        if let Some(external) = self.external {
            visit(external, Repr::Ref);
        }
        self.entries.iter().for_each(|&e| visit(e, Repr::Ref));
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// `FutureGroup.wait` and `FutureScope.result`: run the scope body (scope
/// only), then wait for every member; a failure from outside (the body's, a
/// cancellation) cancels the group and is reported after it.
fn group_wait(runtime: &mut Runtime, ctx: *mut Ctx, frame: Word, function: usize, scope: bool, deliver: bool) -> Step {
    let receiver = param(runtime, frame, function, 0);
    if state::<GroupWait>(runtime, frame).is_none() {
        let value_repr = runtime.loaded.functions[function].result.map_or(Repr::Raw, Kind::repr);
        let group = if scope {
            let owner = runtime.loaded.program.functions[function].intrinsic.as_ref().expect("intrinsic").objects[0].0;
            let group = runtime.heap.object_new(owner, &[]);
            runtime.state_new(group, State::Group(Default::default()));
            group
        } else {
            receiver
        };
        let wait = GroupWait {
            phase: if scope { 0 } else { 2 },
            group,
            body: UNBOUND,
            value: if value_repr == Repr::Raw { crate::word::UNBOUND_RAW } else { UNBOUND },
            value_repr,
            external: None,
            entries: Vec::new(),
            index: 0,
        };
        set_state(runtime, frame, Box::new(wait));
        if !scope {
            seal(runtime, frame);
        }
    }
    let group = state::<GroupWait>(runtime, frame).expect("group wait").group;
    if state::<GroupWait>(runtime, frame).expect("group wait").phase == 0 {
        let action = runtime.heap.field(receiver, 0);
        let body = crate::runtime::rt_call_closure(ctx, action, &group, 1);
        let s = state::<GroupWait>(runtime, frame).expect("group wait");
        s.phase = 1;
        match unsafe { (*ctx).status } {
            OK => s.body = body,
            _ => match call_failed(ctx) {
                Ok(failure) => {
                    s.external = Some(failure);
                    body_done(runtime, ctx, frame, group);
                }
                Err(()) => return Stop,
            },
        }
    }
    if state::<GroupWait>(runtime, frame).expect("group wait").phase == 1 {
        let body = state::<GroupWait>(runtime, frame).expect("group wait").body;
        let outcome = if deliver {
            runtime.remove_waiter(body, frame);
            tasks::set(frame, tasks::AWAITING, UNBOUND);
            Some(Err(runtime.cancelled_failure()))
        } else {
            runtime.driver_await(ctx, frame, body)
        };
        if unsafe { (*ctx).status } != OK {
            return Stop;
        }
        match outcome {
            None => return Pending,
            Some(Ok(value)) => state::<GroupWait>(runtime, frame).expect("group wait").value = value,
            Some(Err(failure)) => state::<GroupWait>(runtime, frame).expect("group wait").external = Some(failure),
        }
        body_done(runtime, ctx, frame, group);
        return group_members(runtime, ctx, frame, scope, false);
    }
    group_members(runtime, ctx, frame, scope, deliver)
}

/// After the scope body: cancel the group when it failed, then wait.
fn body_done(runtime: &mut Runtime, ctx: *mut Ctx, frame: Word, group: Word) {
    if state::<GroupWait>(runtime, frame).expect("group wait").external.is_some() {
        runtime.group_cancel(ctx, group);
    }
    seal(runtime, frame);
}

fn seal(runtime: &mut Runtime, frame: Word) {
    let group = state::<GroupWait>(runtime, frame).expect("group wait").group;
    let entries = match runtime.group_mut(group) {
        Some(g) => {
            g.sealed = true;
            g.entries.clone()
        }
        None => Vec::new(),
    };
    for &entry in &entries {
        Runtime::observe(entry);
    }
    let s = state::<GroupWait>(runtime, frame).expect("group wait");
    s.entries = entries;
    s.index = 0;
    s.phase = 2;
}

fn group_members(runtime: &mut Runtime, ctx: *mut Ctx, frame: Word, scope: bool, deliver: bool) -> Step {
    let group = state::<GroupWait>(runtime, frame).expect("group wait").group;
    if runtime.group_mut(group).is_none() {
        return fail(runtime, FailureKind::Program("Invalid future group".into()));
    }
    let mut deliver = deliver;
    loop {
        let s = state::<GroupWait>(runtime, frame).expect("group wait");
        let Some(&child) = s.entries.get(s.index) else {
            break;
        };
        let outcome = if deliver {
            deliver = false;
            runtime.remove_waiter(child, frame);
            tasks::set(frame, tasks::AWAITING, UNBOUND);
            Some(Err(runtime.cancelled_failure()))
        } else {
            runtime.driver_await(ctx, frame, child)
        };
        if unsafe { (*ctx).status } != OK {
            return Stop;
        }
        match outcome {
            None => return Pending,
            Some(Err(failure)) if tasks::get(child, tasks::FAILURE) != failure || tasks::get(child, tasks::STATE) != FAILURE_STATE => {
                // A failure from outside the member: cancel the group, keep waiting.
                runtime.group_cancel(ctx, group);
                let s = state::<GroupWait>(runtime, frame).expect("group wait");
                if !scope || s.external.is_none() {
                    s.external = Some(failure);
                }
            }
            Some(_) => state::<GroupWait>(runtime, frame).expect("group wait").index += 1,
        }
    }
    // Finish: the first unexpected member failure is primary, others attach.
    let cached = runtime.group_mut(group).expect("group").result;
    let result = match cached {
        Some(result) => result,
        None => {
            let (entries, owned) = {
                let g = runtime.group_mut(group).expect("group");
                (g.entries.clone(), g.owned.clone())
            };
            let mut primary: Option<Word> = None;
            for (entry, owned) in entries.into_iter().zip(owned) {
                match tasks::get(entry, tasks::STATE) {
                    DONE => {}
                    FAILURE_STATE => {
                        let failure = tasks::get(entry, tasks::FAILURE);
                        let quiet = runtime.failure(failure).is_some_and(|f| matches!(f.kind, FailureKind::Cancelled) && f.secondary.is_empty());
                        if owned && quiet {
                            continue;
                        }
                        match primary {
                            Some(first) if first != failure => {
                                runtime.failure_link(first, failure, true);
                            }
                            Some(_) => {}
                            None => primary = Some(failure),
                        }
                    }
                    _ => {
                        crate::runtime::fatal(unsafe { &mut *ctx }, runtime, "group finished with running child".into());
                        return Stop;
                    }
                }
            }
            let result = primary.map_or(Ok(group), Err);
            runtime.group_mut(group).expect("group").result = Some(result);
            result
        }
    };
    let s = state::<GroupWait>(runtime, frame).expect("group wait");
    let (external, value) = (s.external, s.value);
    if let Some(external) = external {
        if let Err(failed) = result {
            if failed != external {
                runtime.failure_link(external, failed, true);
            }
        }
        return Failed(external);
    }
    match result {
        Err(failure) => Failed(failure),
        Ok(_) if scope => Done(value),
        Ok(_) => Done(param(runtime, frame, runtime.function_of(frame), 0)),
    }
}

// ---- collections: AllFutures, Raced, FirstSuccessful, Before/Within ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Strategy {
    All,
    Race,
    First,
    Timeout,
}

#[derive(Debug)]
struct Collection {
    strategy: Strategy,
    started: bool,
    entries: Vec<Word>,
    consumed: Vec<bool>,
    cancelled: Vec<bool>,
    initial_order: u64,
    winner: Option<Word>,
    winner_repr: Repr,
    failures: Vec<Word>,
    external: Option<Word>,
    finishing: bool,
}
impl Payload for Collection {
    fn trace(&self, visit: &mut dyn FnMut(Word, Repr)) {
        self.entries.iter().chain(&self.failures).chain(&self.external).for_each(|&w| visit(w, Repr::Ref));
        if let Some(winner) = self.winner {
            visit(winner, self.winner_repr);
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn collection(runtime: &mut Runtime, frame: Word) -> &mut Collection {
    state::<Collection>(runtime, frame).expect("collection")
}

fn collect(runtime: &mut Runtime, ctx: *mut Ctx, frame: Word, function: usize, intrinsic: Intrinsic, deliver: bool) -> Step {
    let receiver = param(runtime, frame, function, 0);
    if state::<Collection>(runtime, frame).is_none() {
        let strategy = match intrinsic {
            Intrinsic::CollectAll => Strategy::All,
            Intrinsic::CollectRace => Strategy::Race,
            Intrinsic::CollectFirst => Strategy::First,
            _ => Strategy::Timeout,
        };
        let winner_repr = runtime.loaded.functions[function].result.map_or(Repr::Raw, Kind::repr);
        let payload = Collection {
            strategy,
            started: false,
            entries: Vec::new(),
            consumed: Vec::new(),
            cancelled: Vec::new(),
            initial_order: 0,
            winner: None,
            winner_repr,
            failures: Vec::new(),
            external: None,
            finishing: false,
        };
        set_state(runtime, frame, Box::new(payload));
        let step = if strategy == Strategy::Timeout {
            timeout_start(runtime, ctx, frame, function, intrinsic == Intrinsic::TimeoutBefore)
        } else {
            collect_start(runtime, ctx, frame, function, receiver)
        };
        if let Some(step) = step {
            return step;
        }
        let entries = collection(runtime, frame).entries.clone();
        for &entry in &entries {
            Runtime::observe(entry);
            runtime.spawn(ctx, entry);
            if unsafe { (*ctx).status } != OK {
                return Stop;
            }
        }
        let completions = runtime.completions;
        let c = collection(runtime, frame);
        c.consumed = vec![false; entries.len()];
        c.cancelled = vec![false; entries.len()];
        c.initial_order = completions;
        c.started = true;
    }
    let entries = collection(runtime, frame).entries.clone();
    // Whatever woke this frame, stop listening to the other entries.
    for &entry in &entries {
        runtime.remove_waiter(entry, frame);
    }
    if deliver {
        let failure = runtime.cancelled_failure();
        collection(runtime, frame).external = Some(failure);
        cancel_collection(runtime, ctx, frame);
    }
    loop {
        let c = collection(runtime, frame);
        let mut best: Option<(u8, u64, usize)> = None;
        let mut remaining = false;
        for (index, &entry) in c.entries.iter().enumerate() {
            if c.consumed[index] {
                continue;
            }
            remaining = true;
            if matches!(tasks::get(entry, tasks::STATE), DONE | FAILURE_STATE) {
                let order = Runtime::completion_order(entry);
                let key = if order <= c.initial_order { (0, 0, index) } else { (1, order, index) };
                if best.is_none_or(|b| key < b) {
                    best = Some(key);
                }
            }
        }
        match best {
            Some((_, _, index)) => consume(runtime, ctx, frame, function, index),
            None if remaining => {
                let c = collection(runtime, frame);
                let pending: Vec<Word> = c
                    .entries
                    .iter()
                    .zip(&c.consumed)
                    .filter(|&(_, &consumed)| !consumed)
                    .map(|(&e, _)| e)
                    .collect();
                for entry in pending {
                    runtime.listen(entry, frame);
                }
                return Pending;
            }
            None => break,
        }
    }
    collect_finish(runtime, frame, function)
}

/// Read the origins of a collection through their sequence role; None when
/// the entries are ready to start.
fn collect_start(runtime: &mut Runtime, ctx: *mut Ctx, frame: Word, function: usize, receiver: Word) -> Option<Step> {
    let program = &runtime.loaded.program;
    let receiver_type = program.functions[function].registers[program.functions[function].params[0].0 as usize];
    let Some(okkama_sparrow::Type::Object(owner)) = program.ty(receiver_type) else {
        return Some(fail(runtime, FailureKind::Program("invalid collection receiver".into())));
    };
    let Some(okkama_sparrow::Type::Role(role)) = program.ty(program.objects[owner.0 as usize].fields[0].1) else {
        return Some(fail(runtime, FailureKind::Program("invalid collection origins".into())));
    };
    let role = role.0;
    let methods = &program.roles[role as usize].methods;
    let size_slot = methods.iter().position(|m| m.name == "size")? as u32;
    let item_slot = methods.iter().position(|m| m.name == "item")? as u32;
    let sequence = runtime.heap.field(receiver, 0);
    let Some(size) = call_role(ctx, runtime, sequence, role, size_slot, &[]) else {
        return Some(call_failed(ctx).map_or(Stop, Failed));
    };
    let size = runtime.small_or_big(size).unwrap_or(0).max(0);
    if size == 0 && collection(runtime, frame).strategy != Strategy::All {
        return Some(Failed(failure_of(runtime, function, 0)));
    }
    for index in 0..size {
        let position = runtime.heap.int(index);
        let Some(item) = call_role(ctx, runtime, sequence, role, item_slot, &[position]) else {
            return Some(call_failed(ctx).map_or(Stop, Failed));
        };
        collection(runtime, frame).entries.push(item);
    }
    None
}

/// Before/Within: race the origin against a timer future from the clock.
fn timeout_start(runtime: &mut Runtime, ctx: *mut Ctx, frame: Word, function: usize, before: bool) -> Option<Step> {
    let receiver = param(runtime, frame, function, 0);
    let origin = runtime.heap.field(receiver, 0);
    runtime.spawn(ctx, origin);
    if unsafe { (*ctx).status } != OK {
        return Some(Stop);
    }
    Runtime::observe(origin);
    match tasks::get(origin, tasks::STATE) {
        DONE => return Some(Done(tasks::get(origin, RESULT))),
        FAILURE_STATE => return Some(Failed(tasks::get(origin, tasks::FAILURE))),
        _ => {}
    }
    collection(runtime, frame).entries.push(origin);
    let helpers: Vec<usize> = runtime.loaded.program.functions[function]
        .intrinsic
        .as_ref()
        .expect("intrinsic")
        .helpers
        .iter()
        .map(|h| h.0 as usize)
        .collect();
    let offset = if before { 2 } else { 0 };
    let clock = runtime.heap.field(receiver, 2);
    let timer = (|| {
        let mut limit = runtime.heap.field(receiver, 1);
        if before {
            let now = call_function(ctx, runtime, helpers[0], &[clock])?;
            runtime.roots.push((now, Repr::Ref));
            let remaining = call_function(ctx, runtime, helpers[1], &[limit, now]);
            runtime.roots.pop();
            limit = remaining?;
        }
        runtime.roots.push((limit, Repr::Ref));
        let amount = call_function(ctx, runtime, helpers[offset], &[limit]);
        let result = amount.and_then(|amount| {
            use num_traits::Signed;
            if runtime.rational(amount).is_positive() {
                call_function(ctx, runtime, helpers[offset + 1], &[clock, limit]).map(Some)
            } else {
                Some(None)
            }
        });
        runtime.roots.pop();
        result
    })();
    let timer = match timer {
        Some(Some(timer)) => timer,
        // Already expired: the timer has won.
        Some(None) => runtime.settled_new(function, DONE, timer_value(runtime, function)),
        None => match call_failed(ctx) {
            Ok(failure) => runtime.settled_new(function, FAILURE_STATE, failure),
            Err(()) => return Some(Stop),
        },
    };
    collection(runtime, frame).entries.push(timer);
    None
}

/// An inert result word for a settled timer stand-in of `function`.
fn timer_value(runtime: &Runtime, function: usize) -> Word {
    match runtime.loaded.functions[function].result {
        Some(Kind::Float | Kind::Raw | Kind::Byte) => crate::word::UNBOUND_RAW,
        _ => UNBOUND,
    }
}

fn cancel_collection(runtime: &mut Runtime, ctx: *mut Ctx, frame: Word) {
    let c = collection(runtime, frame);
    c.finishing = true;
    let entries = c.entries.clone();
    let mut seen = std::collections::HashSet::new();
    for (index, entry) in entries.into_iter().enumerate() {
        let settled = matches!(tasks::get(entry, tasks::STATE), DONE | FAILURE_STATE);
        if seen.contains(&entry) || !settled {
            seen.insert(entry);
            collection(runtime, frame).cancelled[index] = true;
            runtime.cancel(ctx, entry);
        }
    }
}

fn consume(runtime: &mut Runtime, ctx: *mut Ctx, frame: Word, function: usize, index: usize) {
    let entry = collection(runtime, frame).entries[index];
    let mut result = match tasks::get(entry, tasks::STATE) {
        DONE => Ok(tasks::get(entry, RESULT)),
        _ => Err(tasks::get(entry, tasks::FAILURE)),
    };
    let c = collection(runtime, frame);
    if c.strategy == Strategy::Timeout && index == 1 && !c.finishing && result.is_ok() {
        result = Err(failure_of(runtime, function, 0));
    }
    let ignore = match result {
        Err(failure) => {
            let quiet = runtime.failure(failure).is_some_and(|f| matches!(f.kind, FailureKind::Cancelled) && f.secondary.is_empty());
            let c = collection(runtime, frame);
            (c.cancelled[index] && quiet)
                || (c.finishing && c.external.is_none() && c.strategy != Strategy::All && !c.cancelled[index])
        }
        Ok(_) => false,
    };
    let c = collection(runtime, frame);
    c.consumed[index] = true;
    let mut cancel = false;
    match result {
        Ok(value) if !c.finishing && c.strategy != Strategy::All => {
            if c.strategy == Strategy::First {
                c.failures.clear();
            }
            c.winner = Some(value);
            cancel = true;
        }
        Err(failure) if !ignore => {
            if !c.failures.contains(&failure) {
                c.failures.push(failure);
            }
            if !c.finishing && c.strategy != Strategy::First {
                cancel = true;
            }
        }
        _ => {}
    }
    if cancel {
        cancel_collection(runtime, ctx, frame);
    }
}

fn collect_finish(runtime: &mut Runtime, frame: Word, function: usize) -> Step {
    let c = collection(runtime, frame);
    let (strategy, entries, winner, mut failures, external) =
        (c.strategy, c.entries.clone(), c.winner, c.failures.clone(), c.external);
    if let Some(primary) = external.or_else(|| failures.first().copied()) {
        // AllFutures keeps the first observed failure as primary and attaches
        // the others in input order.
        if strategy == Strategy::All {
            let position = |failure: Word| {
                entries.iter().position(|&e| tasks::get(e, tasks::STATE) == FAILURE_STATE && tasks::get(e, tasks::FAILURE) == failure)
            };
            failures.sort_by_key(|&f| position(f));
        }
        for failure in failures {
            if failure != primary {
                runtime.failure_link(primary, failure, true);
            }
        }
        return Failed(primary);
    }
    if strategy == Strategy::All {
        let values: Vec<Word> = entries.iter().map(|&e| tasks::get(e, RESULT)).collect();
        let program = &runtime.loaded.program;
        let owner = program.functions[function].intrinsic.as_ref().expect("intrinsic").objects[0].0;
        let items = program.objects[owner as usize].fields[0].1;
        let Some(okkama_sparrow::Type::Array(element)) = program.ty(items) else {
            return fail(runtime, FailureKind::Program("invalid collection result".into()));
        };
        let kind = Kind::of(program, *element);
        let array = if kind == Kind::Byte {
            let bytes: Vec<u8> = values.iter().map(|&w| w as u8).collect();
            runtime.heap.allocate_bytes(&bytes)
        } else {
            runtime.heap.allocate_array(kind.repr(), &values)
        };
        return Done(runtime.heap.object_new(owner, &[array]));
    }
    match winner {
        Some(winner) => Done(winner),
        None => fail(runtime, FailureKind::Program("collection has no winner".into())),
    }
}

// ---- FutureCompletions.completion ----

/// Resume code of a request that cancels the stream and waits for its inputs.
const DRAINING: u64 = 4;

fn completion(runtime: &mut Runtime, ctx: *mut Ctx, frame: Word, function: usize, deliver: bool) -> Step {
    let receiver = param(runtime, frame, function, 0);
    match crate::intrinsics::stream_ready(ctx, runtime, function, receiver) {
        Ok(()) => {}
        Err(Some(failure)) => return Failed(failure),
        Err(None) => return Stop,
    }
    let entries = runtime.stream_mut(receiver).expect("stream").entries.clone();
    for &entry in &entries {
        runtime.remove_waiter(entry, frame);
    }
    match resume(frame) {
        COMPLETED => return Done(tasks::get(frame, RESULT)),
        REFUSED => return Failed(tasks::get(frame, RESULT)),
        DRAINING => return drain(runtime, ctx, frame, receiver),
        0 => {
            let stream = runtime.stream_mut(receiver).expect("stream");
            if let (Some(aborted), true) = (stream.aborted, stream.abort_done) {
                return Failed(aborted);
            }
            if stream.reserved >= stream.entries.len() {
                return Failed(failure_of(runtime, function, 0));
            }
            stream.reserved += 1;
            stream.waiters.push_back(frame);
            if !stream.started && !stream.aborting {
                stream.started = true;
                for &entry in &entries {
                    Runtime::observe(entry);
                    runtime.spawn(ctx, entry);
                    if unsafe { (*ctx).status } != OK {
                        return Stop;
                    }
                }
                let completions = runtime.completions;
                runtime.stream_mut(receiver).expect("stream").initial_order = completions;
            }
        }
        _ if deliver => {
            let failure = runtime.cancelled_failure();
            let stream = runtime.stream_mut(receiver).expect("stream");
            stream.waiters.retain(|&w| w != frame);
            if stream.aborting {
                // Another request is cleaning up; its outcome is this one's.
                stream.waiters.push_back(frame);
                wait(frame, 0);
                return Pending;
            }
            stream.aborting = true;
            stream.aborted = Some(failure);
            for (index, &entry) in entries.iter().enumerate() {
                if !matches!(tasks::get(entry, tasks::STATE), DONE | FAILURE_STATE) {
                    runtime.stream_mut(receiver).expect("stream").cancelled[index] = true;
                    runtime.cancel(ctx, entry);
                }
            }
            tasks::set(frame, RESUME, DRAINING);
            return drain(runtime, ctx, frame, receiver);
        }
        _ => {}
    }
    runtime.stream_poll(receiver, frame);
    if resume(frame) == COMPLETED {
        return Done(tasks::get(frame, RESULT));
    }
    if !runtime.stream_mut(receiver).expect("stream").aborting {
        let stream = runtime.stream_mut(receiver).expect("stream");
        let pending: Vec<Word> =
            stream.entries.iter().zip(&stream.emitted).filter(|&(_, &emitted)| !emitted).map(|(&e, _)| e).collect();
        for entry in pending {
            runtime.listen(entry, frame);
        }
    }
    wait(frame, 0);
    Pending
}

/// Wait for every input after cancelling the stream, then fail every pending
/// request with the cancellation (cleanup failures attached).
fn drain(runtime: &mut Runtime, ctx: *mut Ctx, frame: Word, receiver: Word) -> Step {
    let entries = runtime.stream_mut(receiver).expect("stream").entries.clone();
    let mut index = (tasks::get(frame, RESUME) >> 8) as usize;
    while let Some(&entry) = entries.get(index) {
        let outcome = runtime.driver_await(ctx, frame, entry);
        if unsafe { (*ctx).status } != OK {
            return Stop;
        }
        if outcome.is_none() {
            tasks::set(frame, RESUME, DRAINING | ((index as u64) << 8));
            return Pending;
        }
        index += 1;
    }
    let stream = runtime.stream_mut(receiver).expect("stream");
    let primary = stream.aborted.expect("stream cancellation");
    let cancelled = stream.cancelled.clone();
    let mut seen = std::collections::HashSet::from([primary]);
    for (entry, owned) in entries.iter().zip(cancelled) {
        if tasks::get(*entry, tasks::STATE) != FAILURE_STATE {
            continue;
        }
        let failure = tasks::get(*entry, tasks::FAILURE);
        let quiet = runtime.failure(failure).is_some_and(|f| matches!(f.kind, FailureKind::Cancelled) && f.secondary.is_empty());
        if owned && quiet {
            continue;
        }
        if seen.insert(failure) {
            runtime.failure_link(primary, failure, true);
        }
    }
    let stream = runtime.stream_mut(receiver).expect("stream");
    stream.queue.clear();
    stream.delivered = stream.entries.len();
    stream.reserved = stream.entries.len();
    stream.abort_done = true;
    let waiters: Vec<Word> = stream.waiters.drain(..).collect();
    for waiter in waiters {
        if waiter != frame {
            runtime.complete_wait(waiter, REFUSED, Some(primary));
        }
    }
    Failed(primary)
}
