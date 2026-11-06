//! State of the async primitives (Atomic, Event and Cancellation, Lock and
//! Semaphore, Barrier, Channel, FutureGroup), kept by the address of the
//! language object that owns it. The object is watched by the heap, so its
//! state goes when the object dies. While the state lives, the frames waiting
//! in it and the values it holds are roots.
//!
//! Waiting frames are parked drivers (`tasks`). A completer records the
//! outcome in the waiter's resume word (and its result word) and wakes it; a
//! waiter that is no longer parked is being cancelled and is skipped.
use crate::runtime::Runtime;
use crate::tasks::{self, RESULT, RESUME};
use crate::word::{Repr, UNBOUND, Word};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// Resume codes of a waiting driver.
pub const WAITING: u64 = 1;
pub const COMPLETED: u64 = 2;
/// Completed with the primitive's own failure (closed channel, broken barrier).
pub const REFUSED: u64 = 3;

#[derive(Debug)]
pub enum State {
    /// Event, and Cancellation with its tokens.
    Event(Rc<RefCell<Event>>),
    /// Lock (one permit) and Semaphore.
    Pool(Rc<RefCell<Pool>>),
    Grant { pool: Rc<RefCell<Pool>>, closed: bool },
    Barrier(Rc<RefCell<Barrier>>),
    Snapshot([i64; 4]),
    /// A channel and both of its endpoints.
    Channel(Rc<RefCell<Channel>>),
    Group(Group),
    Stream(Box<Stream>),
    /// `fake.storage` of a role fake: key → (key word, value word, its type, repr).
    FakeStorage(std::collections::BTreeMap<crate::ops::Key, (Word, Word, okkama_sparrow::TypeRef, Repr)>),
    /// A bound action (`worker.bind`): its worker entry and receiver fields.
    WorkerBinding { entry: Word, fields: Vec<(Word, Repr)> },
}

/// `FutureCompletions`: settled inputs handed to `completion()` requests in
/// settlement order (inputs already settled at the start: in input order).
#[derive(Debug, Default)]
pub struct Stream {
    pub entries: Vec<Word>,
    pub emitted: Vec<bool>,
    pub cancelled: Vec<bool>,
    pub queue: VecDeque<usize>,
    /// Pending `completion()` frames, FIFO.
    pub waiters: VecDeque<Word>,
    pub reserved: usize,
    pub delivered: usize,
    pub started: bool,
    pub initial_order: u64,
    pub aborting: bool,
    pub aborted: Option<Word>,
    pub abort_done: bool,
}

/// An Event object (a cell: `[initial, set, waiters]`) or the shared event of
/// a Cancellation and its tokens.
#[derive(Debug)]
pub enum EventRef {
    Cell(Word),
    Shared(Rc<RefCell<Event>>),
}

#[derive(Debug, Default)]
pub struct Event {
    pub set: bool,
    pub waiters: Vec<Word>,
}

#[derive(Debug, Default)]
pub struct Pool {
    pub remaining: i64,
    pub queue: VecDeque<Word>,
}

#[derive(Debug, Default)]
pub struct Barrier {
    pub parties: i64,
    pub generation: i64,
    pub broken: bool,
    pub waiting: Vec<Word>,
}

#[derive(Debug)]
pub struct Channel {
    pub capacity: usize,
    pub closed: bool,
    pub repr: Repr,
    pub messages: VecDeque<Word>,
    /// Blocked `send` frames; the payload stays in the frame.
    pub senders: VecDeque<Word>,
    pub receivers: VecDeque<Word>,
}

#[derive(Debug, Default)]
pub struct Group {
    pub sealed: bool,
    pub cancelled: bool,
    pub entries: Vec<Word>,
    /// Cancelled by the group itself (its Cancelled failure is not reported).
    pub owned: Vec<bool>,
    pub result: Option<Result<Word, Word>>,
}

impl State {
    pub fn trace(&self, visit: &mut dyn FnMut(Word, Repr)) {
        match self {
            Self::Event(event) => event.borrow().waiters.iter().for_each(|&w| visit(w, Repr::Ref)),
            Self::Pool(pool) | Self::Grant { pool, .. } => {
                pool.borrow().queue.iter().for_each(|&w| visit(w, Repr::Ref))
            }
            Self::Barrier(barrier) => barrier.borrow().waiting.iter().for_each(|&w| visit(w, Repr::Ref)),
            Self::Channel(channel) => {
                let channel = channel.borrow();
                channel.messages.iter().for_each(|&w| visit(w, channel.repr));
                channel.senders.iter().chain(&channel.receivers).for_each(|&w| visit(w, Repr::Ref));
            }
            Self::WorkerBinding { entry, fields } => {
                visit(*entry, Repr::Ref);
                fields.iter().for_each(|&(w, repr)| visit(w, repr));
            }
            Self::FakeStorage(storage) => {
                for &(key, value, _, repr) in storage.values() {
                    visit(key, Repr::Ref);
                    visit(value, repr);
                }
            }
            Self::Stream(stream) => {
                stream.entries.iter().chain(&stream.waiters).chain(&stream.aborted).for_each(|&w| visit(w, Repr::Ref))
            }
            Self::Group(group) => {
                group.entries.iter().for_each(|&w| visit(w, Repr::Ref));
                if let Some(Ok(w) | Err(w)) = group.result {
                    visit(w, Repr::Ref);
                }
            }
            _ => {}
        }
    }
}

impl Runtime {
    /// Attach `state` to the language object `object`.
    pub fn state_new(&mut self, object: Word, state: State) {
        self.heap.watch(object);
        self.states.insert(object, state);
    }

    /// Record the outcome of a parked waiter and wake it; false when it is
    /// no longer parked (being cancelled).
    pub fn complete_wait(&mut self, frame: Word, code: u64, result: Option<Word>) -> bool {
        if !Runtime::parked(frame) {
            return false;
        }
        tasks::set(frame, RESUME, code | (tasks::get(frame, RESUME) & !0xff));
        if let Some(result) = result {
            tasks::set(frame, RESULT, result);
        }
        self.wake(frame);
        true
    }

    /// The event behind an Event, Cancellation or token object.
    pub fn event_of(&self, object: Word) -> Option<EventRef> {
        let header = self.heap.header_of(object);
        if crate::heap::kind(header) == crate::heap::OBJECT
            && self.loaded.is_event(crate::heap::payload(header) as usize)
        {
            return Some(EventRef::Cell(object));
        }
        match self.states.get(&object) {
            Some(State::Event(event)) => Some(EventRef::Shared(event.clone())),
            _ => None,
        }
    }

    pub fn event_ready(&self, event: &EventRef) -> bool {
        match event {
            EventRef::Cell(object) => self.heap.field(*object, 1) != 0,
            EventRef::Shared(event) => event.borrow().set,
        }
    }

    pub fn event_wait(&mut self, event: &EventRef, frame: Word) {
        match event {
            EventRef::Cell(object) => {
                let list = match self.heap.field(*object, 2) {
                    UNBOUND => {
                        let list = self.heap.allocate_payload(Box::new(tasks::Waiters::default()));
                        self.heap.set_field(*object, 2, list);
                        list
                    }
                    list => list,
                };
                let waiters = &mut self.heap.payload_mut::<tasks::Waiters>(list).expect("waiters").0;
                if !waiters.contains(&frame) {
                    waiters.push(frame);
                }
            }
            EventRef::Shared(event) => {
                let mut event = event.borrow_mut();
                if !event.waiters.contains(&frame) {
                    event.waiters.push(frame);
                }
            }
        }
    }

    pub fn event_unwait(&mut self, event: &EventRef, frame: Word) {
        match event {
            EventRef::Cell(object) => {
                let list = self.heap.field(*object, 2);
                if list != UNBOUND {
                    self.heap.payload_mut::<tasks::Waiters>(list).expect("waiters").0.retain(|&w| w != frame);
                }
            }
            EventRef::Shared(event) => event.borrow_mut().waiters.retain(|&w| w != frame),
        }
    }

    pub fn event_set(&mut self, event: &EventRef) {
        let waiters = match event {
            EventRef::Cell(object) => {
                if self.heap.field(*object, 1) != 0 {
                    return;
                }
                self.heap.set_field(*object, 1, 1);
                match self.heap.field(*object, 2) {
                    UNBOUND => Vec::new(),
                    list => std::mem::take(&mut self.heap.payload_mut::<tasks::Waiters>(list).expect("waiters").0),
                }
            }
            EventRef::Shared(event) => {
                let mut event = event.borrow_mut();
                if event.set {
                    return;
                }
                event.set = true;
                std::mem::take(&mut event.waiters)
            }
        };
        for waiter in waiters {
            self.complete_wait(waiter, COMPLETED, None);
        }
    }

    /// The pool of a Lock or Semaphore; a Semaphore gets its pool on first use
    /// from its `amount` field.
    pub fn pool_of(&mut self, object: Word, semaphore: bool) -> Option<Rc<RefCell<Pool>>> {
        match self.states.get(&object) {
            Some(State::Pool(pool)) => return Some(pool.clone()),
            Some(_) => return None,
            None if !semaphore => return None,
            None => {}
        }
        let remaining = self.small_or_big(self.heap.field(object, 0))?;
        let pool = Rc::new(RefCell::new(Pool { remaining, queue: VecDeque::new() }));
        self.state_new(object, State::Pool(pool.clone()));
        Some(pool)
    }

    /// Return one permit and hand free permits to queued waiters in order.
    pub fn pool_release(&mut self, pool: &Rc<RefCell<Pool>>) {
        pool.borrow_mut().remaining += 1;
        loop {
            let next = {
                let mut pool = pool.borrow_mut();
                if pool.remaining <= 0 {
                    return;
                }
                match pool.queue.pop_front() {
                    Some(frame) => frame,
                    None => return,
                }
            };
            if self.complete_wait(next, COMPLETED, None) {
                pool.borrow_mut().remaining -= 1;
            }
        }
    }

    /// Permits a new request could take now.
    pub fn pool_available(pool: &Pool) -> i64 {
        let queued = pool.queue.iter().filter(|&&f| Runtime::parked(f)).count() as i64;
        pool.remaining.saturating_sub(queued).max(0)
    }

    pub fn barrier_of(&mut self, object: Word) -> Option<Rc<RefCell<Barrier>>> {
        match self.states.get(&object) {
            Some(State::Barrier(barrier)) => return Some(barrier.clone()),
            Some(_) => return None,
            None => {}
        }
        let parties = self.small_or_big(self.heap.field(object, 0))?;
        let barrier = Rc::new(RefCell::new(Barrier { parties, ..Barrier::default() }));
        self.state_new(object, State::Barrier(barrier.clone()));
        Some(barrier)
    }

    /// Break the current generation: its waiters fail with BrokenBarrier.
    pub fn barrier_break(&mut self, barrier: &Rc<RefCell<Barrier>>) {
        let waiting = {
            let mut barrier = barrier.borrow_mut();
            barrier.broken = true;
            std::mem::take(&mut barrier.waiting)
        };
        for waiter in waiting {
            self.complete_wait(waiter, REFUSED, None);
        }
    }

    pub fn channel_of(&self, object: Word) -> Option<Rc<RefCell<Channel>>> {
        match self.states.get(&object) {
            Some(State::Channel(channel)) => Some(channel.clone()),
            _ => None,
        }
    }

    pub fn channel_close(&mut self, channel: &Rc<RefCell<Channel>>) {
        let blocked: Vec<Word> = {
            let mut channel = channel.borrow_mut();
            if std::mem::replace(&mut channel.closed, true) {
                return;
            }
            let senders = std::mem::take(&mut channel.senders);
            let receivers = std::mem::take(&mut channel.receivers);
            senders.into_iter().chain(receivers).collect()
        };
        for frame in blocked {
            self.complete_wait(frame, REFUSED, None);
        }
    }

    /// The first parked frame of a queue (cancelled ones are dropped).
    pub fn pop_parked(queue: &mut VecDeque<Word>) -> Option<Word> {
        while let Some(frame) = queue.pop_front() {
            if Runtime::parked(frame) {
                return Some(frame);
            }
        }
        None
    }

    pub fn group_mut(&mut self, object: Word) -> Option<&mut Group> {
        match self.states.get_mut(&object) {
            Some(State::Group(group)) => Some(group),
            _ => None,
        }
    }

    /// Cancel a group: seal it and cancel every unfinished member.
    pub fn group_cancel(&mut self, ctx: *mut crate::runtime::Ctx, object: Word) {
        let Some(group) = self.group_mut(object) else {
            return;
        };
        group.sealed = true;
        if std::mem::replace(&mut group.cancelled, true) {
            return;
        }
        let entries = group.entries.clone();
        for (index, entry) in entries.into_iter().enumerate() {
            let owned = match tasks::get(entry, tasks::STATE) {
                tasks::DONE => false,
                tasks::FAILURE_STATE => self.is_cancelled(tasks::get(entry, tasks::FAILURE)),
                _ => true,
            };
            self.group_mut(object).expect("group").owned[index] = owned;
            if owned {
                self.cancel(ctx, entry);
            }
        }
    }

    pub fn stream_mut(&mut self, object: Word) -> Option<&mut Stream> {
        match self.states.get_mut(&object) {
            Some(State::Stream(stream)) => Some(stream),
            _ => None,
        }
    }

    /// Queue newly settled inputs and pair them with requests in order;
    /// `current` (the running request, not parked) is served too.
    pub fn stream_poll(&mut self, object: Word, current: Word) {
        let completions = {
            let Some(stream) = self.stream_mut(object) else {
                return;
            };
            if !stream.started || stream.aborting {
                return;
            }
            let mut ready = Vec::new();
            for (index, &entry) in stream.entries.iter().enumerate() {
                if stream.emitted[index] || !matches!(tasks::get(entry, tasks::STATE), tasks::DONE | tasks::FAILURE_STATE) {
                    continue;
                }
                let order = Runtime::completion_order(entry);
                ready.push(if order <= stream.initial_order { (0, 0, index) } else { (1, order, index) });
            }
            ready.sort_unstable();
            for (_, _, index) in ready {
                stream.emitted[index] = true;
                stream.queue.push_back(index);
            }
            let mut completions = Vec::new();
            while !stream.queue.is_empty() {
                let Some(waiter) = stream.waiters.pop_front() else {
                    break;
                };
                if waiter != current && !Runtime::parked(waiter) {
                    continue;
                }
                let index = stream.queue.pop_front().expect("queued completion");
                stream.delivered += 1;
                completions.push((waiter, stream.entries[index]));
            }
            completions
        };
        for (waiter, entry) in completions {
            if waiter == current {
                tasks::set(waiter, RESUME, COMPLETED);
                tasks::set(waiter, RESULT, entry);
            } else {
                self.complete_wait(waiter, COMPLETED, Some(entry));
            }
        }
    }

    pub fn is_cancelled(&self, failure: Word) -> bool {
        matches!(self.failure(failure).map(|f| &f.kind), Some(crate::runtime::FailureKind::Cancelled))
    }
}
