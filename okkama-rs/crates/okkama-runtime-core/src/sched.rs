//! Promise delivery for Sparrow's native bridge.
//! Providers post one-shot completions from host threads. Sparrow calls `pump`
//! on the program thread and owns all Okkama task execution itself.
use crate::Value;
use crate::failure::{Failure, Kind};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::rc::Rc;

pub type Outcome = Result<Value, Rc<Failure>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    Running,
    Done,
    Failed,
}

/// A native promise: the provider settles it and Sparrow observes the result.
pub struct FutureCell {
    state: Cell<State>,
    value: RefCell<Option<Value>>,
    failure: RefCell<Option<Rc<Failure>>>,
    waiters: RefCell<VecDeque<Box<dyn FnOnce()>>>,
    cancel_requested: Cell<bool>,
    canceller: RefCell<Option<Box<dyn FnOnce()>>>,
}

pub type Future = Rc<FutureCell>;

impl fmt::Debug for FutureCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Future").field("state", &self.state.get()).finish_non_exhaustive()
    }
}

impl FutureCell {
    pub fn on_cancel(&self, cancel: impl FnOnce() + 'static) {
        *self.canceller.borrow_mut() = Some(Box::new(cancel));
    }

    pub fn state(&self) -> State { self.state.get() }
    pub fn value(&self) -> Option<Value> { self.value.borrow().clone() }
    pub fn failure(&self) -> Option<Rc<Failure>> { self.failure.borrow().clone() }
    pub fn outcome(&self) -> Outcome {
        match self.state.get() {
            State::Failed => Err(self.failure().expect("failed promise has a failure")),
            _ => Ok(self.value().unwrap_or(Value::Undef)),
        }
    }

    pub fn listen(self: &Rc<Self>, listener: impl FnOnce() + 'static) {
        if self.done() { listener(); }
        else { self.waiters.borrow_mut().push_back(Box::new(listener)); }
    }

    pub fn done(&self) -> bool { self.state.get() != State::Running }
    pub fn running(&self) -> bool { self.state.get() == State::Running }
    pub fn cancelled(&self) -> bool {
        self.failure.borrow().as_ref().is_some_and(|failure| is_cancelled(failure))
    }
}

pub fn promise() -> Future {
    Rc::new(FutureCell {
        state: Cell::new(State::Running),
        value: RefCell::new(None),
        failure: RefCell::new(None),
        waiters: RefCell::new(VecDeque::new()),
        cancel_requested: Cell::new(false),
        canceller: RefCell::new(None),
    })
}

pub fn resolved(outcome: Outcome) -> Future {
    let promise = promise();
    settle(&promise, outcome);
    promise
}

pub fn cancelled_failure() -> Rc<Failure> {
    Failure::new(Kind::Thrown {
        object: crate::heap::Ref(Rc::new(crate::heap::Object::new(crate::heap::CANCELLED, Vec::new()))),
        name: "Cancelled".into(),
        message: Some("Operation was cancelled".into()),
    })
}

pub fn is_cancelled(failure: &Failure) -> bool {
    matches!(&failure.kind, Kind::Thrown { object, .. } if object.declaration == crate::heap::CANCELLED)
}

pub fn settle(future: &Future, outcome: Outcome) {
    if !future.running() { return; }
    match outcome {
        Ok(value) => {
            *future.value.borrow_mut() = Some(value);
            future.state.set(State::Done);
        }
        Err(failure) => {
            *future.failure.borrow_mut() = Some(failure);
            future.state.set(State::Failed);
        }
    }
    // Release outside the RefCell borrow: captured resources may reenter delivery.
    let canceller = future.canceller.borrow_mut().take();
    drop(canceller);
    loop {
        let next = future.waiters.borrow_mut().pop_front();
        match next { Some(listener) => listener(), None => break }
    }
}

pub fn cancel(future: &Future) {
    if !future.running() || future.cancel_requested.replace(true) { return; }
    let canceller = future.canceller.borrow_mut().take();
    match canceller {
        Some(cancel) => cancel(),
        None => settle(future, Err(cancelled_failure())),
    }
}

type Delivery = Box<dyn std::any::Any + Send>;
type Continuation = Box<dyn FnOnce(Delivery)>;

struct Scheduler {
    sender: std::sync::mpsc::Sender<(u64, Delivery)>,
    receiver: std::sync::mpsc::Receiver<(u64, Delivery)>,
    pending: RefCell<HashMap<u64, Continuation>>,
    next_op: Cell<u64>,
    timers: RefCell<std::collections::BinaryHeap<std::cmp::Reverse<(std::time::Instant, u64)>>>,
}

impl Scheduler {
    fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self {
            sender, receiver,
            pending: RefCell::new(HashMap::new()), next_op: Cell::new(0),
            timers: RefCell::new(std::collections::BinaryHeap::new()),
        }
    }
}

thread_local! { static SCHEDULER: Scheduler = Scheduler::new(); }
fn with<R>(f: impl FnOnce(&Scheduler) -> R) -> R { SCHEDULER.with(f) }

pub fn reset() {
    // Release callbacks outside the scheduler borrow, including reentrant drops.
    let pending = with(|s| {
        s.timers.borrow_mut().clear();
        std::mem::take(&mut *s.pending.borrow_mut())
    });
    drop(pending);
    with(|s| {
        while s.receiver.try_recv().is_ok() {}
    });
}

/// Сигнал внешнему циклу событий (ядро Sparrow): пришла доставка.
static WAKE: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>> = std::sync::OnceLock::new();

/// Встроить планировщик в чужой цикл событий: `wake` вызывается из рабочих
/// потоков после каждой доставки, цикл затем зовёт [`pump`].
pub fn set_wake(wake: impl Fn() + Send + Sync + 'static) {
    let _ = WAKE.set(Box::new(wake));
}

fn wake() {
    if let Some(wake) = WAKE.get() {
        wake();
    }
}

/// Выполнить доставки, которые уже пришли, не блокируясь.
pub fn pump() {
    fire_due_timers();
    while let Ok((id, delivery)) = with(|s| s.receiver.try_recv()) {
        let done = with(|s| s.pending.borrow_mut().remove(&id));
        if let Some(done) = done {
            done(delivery);
        }
    }
}

/// Run the continuations of timers that are due; true when one ran.
fn fire_due_timers() -> bool {
    let now = std::time::Instant::now();
    let mut fired = false;
    loop {
        let due = with(|s| {
            let mut timers = s.timers.borrow_mut();
            match timers.peek() {
                Some(entry) if entry.0.0 <= now => timers.pop().map(|entry| entry.0.1),
                _ => None,
            }
        });
        let Some(id) = due else {
            return fired;
        };
        let done = with(|s| s.pending.borrow_mut().remove(&id));
        if let Some(done) = done {
            done(Box::new(()));
            fired = true;
        }
    }
}

/// Run `done` on the event loop after `duration`, without a thread (#503).
/// Returns the operation and its id: `forget(id)` drops a cancelled timer's
/// continuation at once instead of at its deadline.
pub fn timer(duration: std::time::Duration, done: impl FnOnce() + 'static) -> (Op, u64) {
    let op = Op(Rc::new(Cell::new(true)));
    let attached = op.clone();
    let id = with(|s| {
        let id = s.next_op.get();
        s.next_op.set(id + 1);
        s.pending.borrow_mut().insert(
            id,
            Box::new(move |_delivery: Delivery| {
                attached.detach();
                done();
            }),
        );
        s.timers.borrow_mut().push(std::cmp::Reverse((std::time::Instant::now() + duration, id)));
        id
    });
    (op, id)
}

/// Drop the continuation of a pending operation (a cancelled timer).
pub fn forget(id: u64) {
    let _ = with(|s| s.pending.borrow_mut().remove(&id));
}

/// Attachment of a provider operation; late delivery still performs cleanup.
#[derive(Clone, Debug)]
pub struct Op(Rc<Cell<bool>>);

impl Op {
    /// Снова ждать операцию (её исход нужен новому ожиданию).
    pub fn attach(&self) {
        self.0.set(true);
    }

    pub fn detach(&self) {
        self.0.set(false);
    }
}

/// A one-shot delivery from an existing I/O worker to the program event loop.
/// Dropping the token reports a lost worker instead of leaving an await pending.
/// The token contains no program-thread Rc values and never runs `done` itself.
#[derive(Debug)]
pub struct Completion<R: Send + 'static> {
    sender: Option<std::sync::mpsc::Sender<(u64, Delivery)>>,
    id: u64,
    marker: std::marker::PhantomData<R>,
}

impl<R: Send + 'static> Completion<R> {
    pub fn deliver(mut self, result: R) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send((self.id, Box::new(Some(result))));
            wake();
        }
    }
}

impl<R: Send + 'static> Drop for Completion<R> {
    fn drop(&mut self) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send((self.id, Box::new(None::<R>)));
            wake();
        }
    }
}

/// Register completion without creating a thread. The provider moves the token
/// into its existing worker job. Cancellation may detach the Op, but delivery
/// still runs on the event loop, allowing late-result resource cleanup.
pub fn external<R: Send + 'static>(done: impl FnOnce(Option<R>) + 'static) -> (Op, Completion<R>) {
    let op = Op(Rc::new(Cell::new(true)));
    let attached = op.clone();
    let (id, sender) = with(|s| {
        let id = s.next_op.get();
        s.next_op.set(id + 1);
        s.pending.borrow_mut().insert(
            id,
            Box::new(move |delivery: Delivery| {
                attached.detach();
                done(
                    *delivery
                        .downcast::<Option<R>>()
                        .expect("external result type"),
                );
            }),
        );
        (id, s.sender.clone())
    });
    (
        op,
        Completion {
            sender: Some(sender),
            id,
            marker: std::marker::PhantomData,
        },
    )
}

/// `work` исполняется в рабочем потоке, `done` получает его результат в
/// цикле событий потока программы.
pub fn offload<R: Send + 'static>(
    work: impl FnOnce() -> R + Send + 'static,
    done: impl FnOnce(R) + 'static,
) -> Op {
    let op = Op(Rc::new(Cell::new(true)));
    let attached = op.clone();
    let (id, sender) = with(|s| {
        let id = s.next_op.get();
        s.next_op.set(id + 1);
        s.pending.borrow_mut().insert(
            id,
            Box::new(move |delivery: Delivery| {
                attached.detach();
                done(*delivery.downcast::<R>().expect("offloaded result type"));
            }),
        );
        (id, s.sender.clone())
    });
    std::thread::spawn(move || {
        let result = work();
        let _ = sender.send((id, Box::new(result)));
        wake();
    });
    op
}

#[cfg(test)]
mod tests {
    use super::*;
    fn int(n: i64) -> Value { Value::Int(crate::IntValue::from_i64(n)) }

    #[test]
    fn listeners_keep_order_and_cached_outcome() {
        reset();
        let future = promise();
        let log = Rc::new(RefCell::new(Vec::new()));
        for n in 0..3 {
            let log = log.clone();
            future.listen(move || log.borrow_mut().push(n));
        }
        settle(&future, Ok(int(42)));
        settle(&future, Ok(int(7)));
        assert_eq!(*log.borrow(), [0, 1, 2]);
        assert_eq!(future.outcome().unwrap().as_int().unwrap().to_i64(), Some(42));
        let called = Rc::new(Cell::new(false));
        let seen = called.clone();
        future.listen(move || seen.set(true));
        assert!(called.get());
    }

    #[test]
    fn cancellation_waits_for_provider_and_is_idempotent() {
        let future = promise();
        let count = Rc::new(Cell::new(0));
        let called = count.clone();
        future.on_cancel(move || called.set(called.get() + 1));
        cancel(&future); cancel(&future);
        assert_eq!(count.get(), 1);
        assert!(future.running());
        settle(&future, Err(cancelled_failure()));
        settle(&future, Ok(int(7)));
        assert!(future.cancelled());
    }

    #[test]
    fn external_delivery_and_lost_token_are_pumped() {
        reset();
        let result = Rc::new(RefCell::new(None));
        let delivered = result.clone();
        let (_op, token) = external(move |value: Option<i64>| *delivered.borrow_mut() = value);
        token.deliver(42);
        assert!(result.borrow().is_none());
        pump();
        assert_eq!(*result.borrow(), Some(42));
        let lost = Rc::new(Cell::new(false));
        let called = lost.clone();
        let (_op, token) = external(move |value: Option<i64>| called.set(value.is_none()));
        drop(token); pump();
        assert!(lost.get());
        reset();
    }

    #[test]
    fn due_timers_fire_and_forgotten_timers_do_not() {
        reset();
        let log = Rc::new(RefCell::new(Vec::new()));
        let first = log.clone();
        let (_op, _) = timer(std::time::Duration::ZERO, move || first.borrow_mut().push(1));
        let second = log.clone();
        let (_op, forgotten) = timer(std::time::Duration::ZERO, move || second.borrow_mut().push(2));
        forget(forgotten); pump();
        assert_eq!(*log.borrow(), [1]);
        reset();
    }
}
