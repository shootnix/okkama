//! Intrinsics std `async/*` (OIR-1 §14.2), тестового режима и `xs.ordering()`
//! — перенос `okkama.rt.Intrinsics` JVM. Состояние объекта держит рантайм
//! (`Object::intrinsic`); операции ожидания — холодные Future, чьё тело — код
//! рантайма, ждущий обещаний планировщика.
//!
//! Intrinsic получает объявления объектов, которые он создаёт (результаты,
//! Error std), функции-переходники программы и аргументы — как заглушка JVM.

use crate::failure::{Failure, Kind};
use crate::heap::Object;
use crate::sched::{self, Future, Outcome};
use crate::{IntValue, Value};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

/// Исполнитель программы, которому intrinsic отдаёт работу с её значениями.
pub trait Host {
    /// Объект объявления `declaration` с полями в порядке объявления.
    fn make(&self, declaration: usize, fields: Vec<Value>) -> Value;
    /// Отказ с Error-объектом объявления `declaration`.
    fn thrown(&self, declaration: usize, fields: Vec<Value>) -> Rc<Failure>;
    /// Поле объекта по имени.
    fn field(&self, object: &Value, name: &str) -> std::result::Result<Value, Rc<Failure>>;
    /// Функция программы над аргументами.
    fn call(&self, function: usize, args: Vec<Value>) -> Outcome;
    /// Вызов значения-функции.
    fn call_fn(&self, callee: &Value, args: Vec<Value>) -> Outcome;
    /// Действие по имени и арности на значении роли или объекта.
    fn call_method(&self, receiver: &Value, name: &str, args: Vec<Value>) -> Outcome;
    /// Объявление объекта модуля `object` с именем `name`.
    fn sibling(&self, object: &Value, name: &str) -> Option<usize>;
    /// Равенство значений (`val.eq`).
    fn equal(&self, left: &Value, right: &Value) -> std::result::Result<bool, Rc<Failure>>;
    /// Текст значения в сообщении теста (`Tests.show`).
    fn show(&self, value: &Value) -> String;
    /// Неудача утверждения текущего теста (`Tests.fail`).
    fn test_failure(&self, message: String) -> std::result::Result<(), Rc<Failure>>;
}

pub type HostRef = Rc<dyn Host>;
type Result<T> = std::result::Result<T, Rc<Failure>>;

fn trap(message: impl Into<String>) -> Rc<Failure> {
    Failure::new(Kind::Program(message.into()))
}

fn int(value: i64) -> Value {
    Value::Int(IntValue::from_i64(value))
}

fn object(value: &Value) -> Result<&Rc<Object>> {
    match value {
        Value::Object(object) => Ok(object),
        _ => Err(trap("intrinsic receiver is not an object")),
    }
}

fn future(value: &Value) -> Result<Future> {
    match value {
        Value::Future(future) => Ok(future.clone()),
        _ => Err(trap("intrinsic expected a future")),
    }
}

fn state<T: 'static>(value: &Value, what: &str) -> Result<Rc<T>> {
    let object = object(value)?;
    let state = object.intrinsic();
    state
        .and_then(|state| state.downcast::<T>().ok())
        .ok_or_else(|| trap(what))
}

fn attach<T: 'static>(value: Value, state: Rc<T>) -> Result<Value> {
    object(&value)?.set_intrinsic(state as Rc<dyn Any>);
    Ok(value)
}

/// Сильные рёбра состояния intrinsic для сборщика циклов; ожидающие
/// (`Waiter`) и замыкания непрозрачны. `false` — состояние заимствовано.
pub(crate) fn state_edges(state: &dyn Any, out: &mut crate::gc::Edges) -> bool {
    if let Some(value) = state.downcast_ref::<Value>() {
        out.value(value);
    } else if let Some(atomic) = state.downcast_ref::<AtomicState>() {
        let Ok(value) = atomic.value.try_borrow() else { return false };
        out.value(&value);
    } else if let Some(retained) = state.downcast_ref::<Retained>() {
        let Ok(result) = retained.result.try_borrow() else { return false };
        result.iter().for_each(|value| out.value(value));
    } else if let Some(storage) = state.downcast_ref::<Storage>() {
        let Ok(entries) = storage.entries.try_borrow() else { return false };
        for (key, value) in entries.iter() {
            out.value(key);
            out.value(value);
        }
    } else if let Some(channel) = state.downcast_ref::<ChannelState>() {
        let Ok(messages) = channel.messages.try_borrow() else { return false };
        messages.iter().for_each(|value| out.value(value));
    } else if let Some(group) = state.downcast_ref::<GroupState>() {
        let (Ok(entries), Ok(completion)) = (group.entries.try_borrow(), group.completion.try_borrow()) else {
            return false;
        };
        entries.iter().chain(completion.iter()).for_each(|future| out.future(future));
    } else if let Some(stream) = state.downcast_ref::<Completions>() {
        let (Ok(queue), Ok(waiters), Ok(aborted)) =
            (stream.queue.try_borrow(), stream.waiters.try_borrow(), stream.aborted.try_borrow())
        else {
            return false;
        };
        stream.origins.iter().chain(queue.iter()).chain(waiters.iter()).for_each(|future| out.future(future));
        aborted.iter().for_each(|failure| out.failure(failure));
    }
    true
}

/// Тело холодного Future операции: код рантайма над захватами.
fn op(retained: Vec<Value>, body: impl FnOnce() -> Outcome + 'static) -> Value {
    Value::Future(sched::cold(Box::new(body), retained))
}

/// Ожидающий: обещание и, у отправителя, значение.
struct Waiter {
    promise: Future,
    value: Value,
    active: Cell<bool>,
}

impl Waiter {
    fn new(value: Value) -> Rc<Self> {
        Rc::new(Self {
            promise: sched::promise(),
            value,
            active: Cell::new(true),
        })
    }
}

/// Ждать обещание ожидающего; отмена снимает его с очереди.
fn await_waiter(waiter: &Waiter) -> Outcome {
    sched::await_future(&waiter.promise).inspect_err(|_| waiter.active.set(false))
}

fn take(queue: &RefCell<VecDeque<Rc<Waiter>>>) -> Option<Rc<Waiter>> {
    loop {
        let waiter = queue.borrow_mut().pop_front()?;
        if waiter.active.get() {
            waiter.active.set(false);
            return Some(waiter);
        }
    }
}

/// Продолжить всех ожидающих очереди по порядку.
fn release_all(queue: &RefCell<VecDeque<Rc<Waiter>>>) {
    while let Some(waiter) = take(queue) {
        sched::settle(&waiter.promise, Ok(Value::Undef));
    }
}

/// Ждать, пока очередь событий откроется: `wait()` Event/Cancelling.
fn wait_until(open: bool, queue: &RefCell<VecDeque<Rc<Waiter>>>, receiver: Value) -> Outcome {
    if !open {
        let waiter = Waiter::new(Value::Undef);
        queue.borrow_mut().push_back(waiter.clone());
        await_waiter(&waiter)?;
    }
    Ok(receiver)
}

// ----- async/channel -----

struct ChannelState {
    capacity: usize,
    closed: Cell<bool>,
    messages: RefCell<VecDeque<Value>>,
    senders: RefCell<VecDeque<Rc<Waiter>>>,
    receivers: RefCell<VecDeque<Rc<Waiter>>>,
}

impl ChannelState {
    fn admit_waiting_sender(&self) {
        let Some(sender) = take(&self.senders) else {
            return;
        };
        match take(&self.receivers) {
            Some(receiver) => sched::complete(&receiver.promise, Ok(sender.value.clone())),
            None => self.messages.borrow_mut().push_back(sender.value.clone()),
        }
        sched::settle(&sender.promise, Ok(Value::Undef));
    }

    fn close(&self, host: &HostRef, closed: usize) {
        if self.closed.get() {
            return;
        }
        self.closed.set(true);
        while let Some(sender) = take(&self.senders) {
            if sender.promise.running() {
                sched::complete(&sender.promise, Err(host.thrown(closed, Vec::new())));
            }
        }
        if self.messages.borrow().is_empty() {
            while let Some(receiver) = take(&self.receivers) {
                if receiver.promise.running() {
                    sched::complete(&receiver.promise, Err(host.thrown(closed, Vec::new())));
                }
            }
        }
    }
}

fn channel(value: &Value) -> Result<Rc<ChannelState>> {
    state(value, "Invalid channel endpoint")
}

// ----- async/event, async/cancellation -----

struct EventState {
    ready: Cell<bool>,
    waiting: RefCell<VecDeque<Rc<Waiter>>>,
}

fn event(value: &Value) -> Result<Rc<EventState>> {
    state(value, "Invalid async event")
}

struct CancelState {
    requested: Cell<bool>,
    waiting: RefCell<VecDeque<Rc<Waiter>>>,
}

fn cancellation(value: &Value) -> Result<Rc<CancelState>> {
    state(value, "Invalid cancellation capability")
}

// ----- async/atomic -----

struct AtomicState {
    value: RefCell<Value>,
    closed: Cell<bool>,
}

fn atomic(value: &Value) -> Result<Rc<AtomicState>> {
    state(value, "Invalid atomic state")
}

fn open_atomic(host: &HostRef, closed: usize, value: &Value) -> Result<Rc<AtomicState>> {
    let state = atomic(value)?;
    if state.closed.get() {
        return Err(host.thrown(closed, Vec::new()));
    }
    Ok(state)
}

// ----- async/scope -----

#[derive(Default)]
struct GroupState {
    sealed: Cell<bool>,
    cancel_requested: Cell<bool>,
    active: Cell<i64>,
    entries: RefCell<Vec<Future>>,
    group_cancelled: RefCell<Vec<bool>>,
    completion: RefCell<Option<Future>>,
}

fn group(value: &Value) -> Result<Rc<GroupState>> {
    state(value, "Invalid future group")
}

fn cancel_group(state: &GroupState) {
    state.sealed.set(true);
    if state.cancel_requested.get() {
        return;
    }
    state.cancel_requested.set(true);
    let entries = state.entries.borrow().clone();
    for (index, future) in entries.iter().enumerate() {
        if future.done() {
            if future.cancelled() {
                state.group_cancelled.borrow_mut()[index] = true;
            }
            continue;
        }
        state.group_cancelled.borrow_mut()[index] = true;
        sched::cancel(future);
    }
}

fn group_wait(receiver: Value) -> Value {
    let retained = vec![receiver.clone()];
    op(retained, move || {
        let state = group(&receiver)?;
        state.sealed.set(true);
        if state.completion.borrow().is_none() {
            let completion = sched::promise();
            *state.completion.borrow_mut() = Some(completion.clone());
            let entries = state.entries.borrow().clone();
            for future in &entries {
                future.observe();
            }
            let finished = state.clone();
            when_all(&entries, move || {
                let entries = finished.entries.borrow().clone();
                let mut primary: Option<Rc<Failure>> = None;
                for (index, future) in entries.iter().enumerate() {
                    if future.state() != sched::State::Failed
                        || (finished.group_cancelled.borrow()[index] && future.cancelled())
                    {
                        continue;
                    }
                    let failure = future.failure().expect("failed future has a failure");
                    match &primary {
                        None => primary = Some(failure),
                        Some(primary) => {
                            primary.with_secondary(failure);
                        }
                    }
                }
                sched::complete(&completion, primary.map_or(Ok(Value::Undef), Err));
            });
        }
        let completion = state.completion.borrow().clone().expect("completion set above");
        if let Err(failure) = sched::await_future(&completion) {
            if sched::is_cancelled(&failure) {
                cancel_group(&state);
            }
            return Err(failure);
        }
        Ok(receiver)
    })
}

/// `then` — когда завершатся все `futures` (сразу, если уже).
fn when_all(futures: &[Future], then: impl FnOnce() + 'static) {
    let pending = futures.iter().filter(|future| !future.done()).count();
    if pending == 0 {
        then();
        return;
    }
    let counter = Rc::new(Cell::new(pending));
    let then = Rc::new(RefCell::new(Some(then)));
    for future in futures.iter().filter(|future| !future.done()) {
        let (counter, then) = (counter.clone(), then.clone());
        future.listen(move || {
            counter.set(counter.get() - 1);
            if counter.get() == 0 {
                if let Some(then) = then.borrow_mut().take() {
                    then();
                }
            }
        });
    }
}

// ----- async/collect -----

/// Элементы `Sequencing<Future<T>>`: поле `origins` объекта.
fn origins(host: &HostRef, receiver: &Value) -> Result<Vec<Future>> {
    let sequence = host.field(receiver, "origins")?;
    let size = host.call_method(&sequence, "size", Vec::new())?;
    let count = size
        .as_int()
        .ok()
        .and_then(IntValue::to_i64)
        .ok_or_else(|| trap("Cannot read origins: size is not an int"))?;
    (0..count)
        .map(|index| future(&host.call_method(&sequence, "item", vec![int(index)])?))
        .collect()
}

/// Повторный `results()`/`result()` даёт тот же Future.
#[derive(Default)]
struct Retained {
    result: RefCell<Option<Value>>,
}

fn retained(value: &Value) -> Result<Rc<Retained>> {
    let object = object(value)?;
    let existing = object.intrinsic();
    if let Some(state) = existing.and_then(|state| state.downcast::<Retained>().ok()) {
        return Ok(state);
    }
    let state = Rc::new(Retained::default());
    object.set_intrinsic(state.clone() as Rc<dyn Any>);
    Ok(state)
}

fn cached(receiver: &Value, make: impl FnOnce() -> Value) -> Result<Value> {
    let state = retained(receiver)?;
    if state.result.borrow().is_none() {
        *state.result.borrow_mut() = Some(make());
    }
    Ok(state.result.borrow().clone().expect("result set above"))
}

fn start_all(futures: &[Future]) {
    for future in futures {
        future.observe();
        sched::start(future, None);
        sched::register(future);
    }
}

fn cancel_others(futures: &[Future], keep: Option<&Future>) -> Vec<Future> {
    let mut cancelled = Vec::new();
    for other in futures {
        if keep.is_some_and(|keep| Rc::ptr_eq(keep, other)) || other.done() {
            continue;
        }
        cancelled.push(other.clone());
        sched::cancel(other);
    }
    cancelled
}

fn contains(futures: &[Future], future: &Future) -> bool {
    futures.iter().any(|other| Rc::ptr_eq(other, future))
}

/// Все Future по порядку входа; отказ одного отменяет незавершённых, ждёт их
/// и отдаёт первый отказ с остальными вторичными.
fn all_futures(host: &HostRef, sequence: usize, receiver: &Value) -> Outcome {
    let futures = origins(host, receiver)?;
    let count = futures.len();
    let values = Rc::new(RefCell::new(vec![Value::Undef; count]));
    if count == 0 {
        return Ok(host.make(sequence, vec![crate::heap::ArrayHeap.allocate(Vec::new())]));
    }
    start_all(&futures);
    let done = sched::promise();
    let remaining = Rc::new(Cell::new(count));
    let finishing = Rc::new(Cell::new(false));
    let futures = Rc::new(futures);
    for (position, future) in futures.iter().enumerate() {
        let (done, remaining, finishing, futures, values, this) = (
            done.clone(),
            remaining.clone(),
            finishing.clone(),
            futures.clone(),
            values.clone(),
            future.clone(),
        );
        future.listen(move || {
            if finishing.get() || !done.running() {
                return;
            }
            if this.state() == sched::State::Failed {
                finishing.set(true);
                let cancelled_here = cancel_others(&futures, Some(&this));
                let all = futures.clone();
                when_all(&futures, move || {
                    let failure = this.failure().expect("failed future has a failure");
                    for other in all.iter() {
                        if Rc::ptr_eq(other, &this)
                            || other.state() != sched::State::Failed
                            || (contains(&cancelled_here, other) && other.cancelled())
                        {
                            continue;
                        }
                        failure.with_secondary(other.failure().expect("failed future"));
                    }
                    sched::complete(&done, Err(failure));
                });
                return;
            }
            values.borrow_mut()[position] = this.value().unwrap_or(Value::Undef);
            remaining.set(remaining.get() - 1);
            if remaining.get() == 0 {
                sched::complete(&done, Ok(Value::Undef));
            }
        });
    }
    if let Err(failure) = sched::await_future(&done) {
        if sched::is_cancelled(&failure) && !finishing.get() {
            finishing.set(true);
            cancel_others(&futures, None);
        }
        return Err(failure);
    }
    let values = values.borrow().clone();
    Ok(host.make(sequence, vec![crate::heap::ArrayHeap.allocate(values)]))
}

/// Исход первого завершившегося; проигравшие отменяются и дожидаются.
fn raced(host: &HostRef, none: usize, receiver: &Value) -> Outcome {
    let futures = origins(host, receiver)?;
    if futures.is_empty() {
        return Err(host.thrown(none, Vec::new()));
    }
    start_all(&futures);
    let done = sched::promise();
    let winner: Rc<RefCell<Option<Future>>> = Rc::new(RefCell::new(None));
    let futures = Rc::new(futures);
    for future in futures.iter() {
        let (done, winner, futures, this) =
            (done.clone(), winner.clone(), futures.clone(), future.clone());
        future.listen(move || {
            if winner.borrow().is_some() {
                return;
            }
            *winner.borrow_mut() = Some(this.clone());
            cancel_others(&futures, Some(&this));
            when_all(&futures, move || {
                let outcome = match this.state() {
                    sched::State::Failed => Err(this.failure().expect("failed future")),
                    _ => Ok(this.value().unwrap_or(Value::Undef)),
                };
                sched::complete(&done, outcome);
            });
        });
    }
    sched::await_future(&done)
}

/// Первый успешный исход; проигравшие отменяются и дожидаются. Если все
/// отказали — первый отказ с остальными вторичными.
fn first_successful(host: &HostRef, none: usize, receiver: &Value) -> Outcome {
    let futures = origins(host, receiver)?;
    if futures.is_empty() {
        return Err(host.thrown(none, Vec::new()));
    }
    start_all(&futures);
    let done = sched::promise();
    let finishing = Rc::new(Cell::new(false));
    let remaining = Rc::new(Cell::new(futures.len()));
    let failures: Rc<RefCell<Vec<Rc<Failure>>>> = Rc::new(RefCell::new(Vec::new()));
    let futures = Rc::new(futures);
    for future in futures.iter() {
        let (done, finishing, remaining, failures, futures, this) = (
            done.clone(),
            finishing.clone(),
            remaining.clone(),
            failures.clone(),
            futures.clone(),
            future.clone(),
        );
        future.listen(move || {
            if finishing.get() || !done.running() {
                return;
            }
            if this.state() == sched::State::Done {
                finishing.set(true);
                cancel_others(&futures, Some(&this));
                when_all(&futures, move || {
                    sched::complete(&done, Ok(this.value().unwrap_or(Value::Undef)));
                });
                return;
            }
            failures
                .borrow_mut()
                .push(this.failure().expect("failed future"));
            remaining.set(remaining.get() - 1);
            if remaining.get() == 0 {
                finishing.set(true);
                let failures = failures.borrow().clone();
                let primary = failures[0].clone();
                for other in &failures[1..] {
                    primary.with_secondary(other.clone());
                }
                sched::complete(&done, Err(primary));
            }
        });
    }
    match sched::await_future(&done) {
        Err(failure) => {
            if sched::is_cancelled(&failure) && !finishing.get() {
                finishing.set(true);
                cancel_others(&futures, None);
            }
            Err(failure)
        }
        ok => ok,
    }
}

/// Поток завершений: входы, очередь завершившихся и ожидающие (FIFO).
struct Completions {
    origins: Vec<Future>,
    started: Cell<bool>,
    reserved: Cell<usize>,
    delivered: Cell<usize>,
    queue: RefCell<VecDeque<Future>>,
    waiters: RefCell<VecDeque<Future>>,
    aborting: Cell<bool>,
    aborted: RefCell<Option<Rc<Failure>>>,
}

fn completions(host: &HostRef, receiver: &Value) -> Result<Rc<Completions>> {
    let object = object(receiver)?;
    let existing = object.intrinsic();
    if let Some(state) = existing.and_then(|state| state.downcast::<Completions>().ok()) {
        return Ok(state);
    }
    let state = Rc::new(Completions {
        origins: origins(host, receiver)?,
        started: Cell::new(false),
        reserved: Cell::new(0),
        delivered: Cell::new(0),
        queue: RefCell::new(VecDeque::new()),
        waiters: RefCell::new(VecDeque::new()),
        aborting: Cell::new(false),
        aborted: RefCell::new(None),
    });
    object.set_intrinsic(state.clone() as Rc<dyn Any>);
    Ok(state)
}

fn emit(state: &Completions, future: &Future) {
    if state.aborting.get() {
        return;
    }
    let waiter = state.waiters.borrow_mut().pop_front();
    match waiter {
        Some(waiter) => {
            state.delivered.set(state.delivered.get() + 1);
            sched::complete(&waiter, Ok(Value::Future(future.clone())));
        }
        None => state.queue.borrow_mut().push_back(future.clone()),
    }
}

fn abort(state: &Rc<Completions>, cancelled: Rc<Failure>) {
    if state.aborting.get() {
        return;
    }
    state.aborting.set(true);
    let mine = cancel_others(&state.origins, None);
    let finished = state.clone();
    when_all(&state.origins, move || {
        for future in &finished.origins {
            if future.state() == sched::State::Failed
                && !(contains(&mine, future) && future.cancelled())
            {
                cancelled.with_secondary(future.failure().expect("failed future"));
            }
        }
        *finished.aborted.borrow_mut() = Some(cancelled.clone());
        let waiters: Vec<Future> = finished.waiters.borrow_mut().drain(..).collect();
        for waiter in waiters {
            if waiter.running() {
                sched::complete(&waiter, Err(cancelled.clone()));
            }
        }
        finished.queue.borrow_mut().clear();
        finished.delivered.set(finished.origins.len());
    });
}

fn completion(host: &HostRef, none: usize, state: Rc<Completions>) -> Outcome {
    if let Some(aborted) = state.aborted.borrow().clone() {
        return Err(aborted);
    }
    if state.reserved.get() >= state.origins.len() {
        return Err(host.thrown(none, Vec::new()));
    }
    state.reserved.set(state.reserved.get() + 1);
    let next = state.queue.borrow_mut().pop_front();
    if let Some(next) = next {
        state.delivered.set(state.delivered.get() + 1);
        return Ok(Value::Future(next));
    }
    let waiter = sched::promise();
    state.waiters.borrow_mut().push_back(waiter.clone());
    if !state.started.get() {
        state.started.set(true);
        start_all(&state.origins);
        for future in &state.origins {
            let (state, future) = (state.clone(), future.clone());
            let target = future.clone();
            target.listen(move || emit(&state, &future));
        }
    }
    match sched::await_future(&waiter) {
        Err(failure) => {
            if sched::is_cancelled(&failure) && waiter.running() {
                state
                    .waiters
                    .borrow_mut()
                    .retain(|other| !Rc::ptr_eq(other, &waiter));
                abort(&state, failure.clone());
            }
            Err(failure)
        }
        ok => ok,
    }
}

// ----- async/lock, async/semaphore -----

/// Владение с очередью: замок (одно разрешение) и семафор (N разрешений).
struct PermitState {
    available: Cell<i64>,
    waiting: RefCell<VecDeque<Rc<Waiter>>>,
}

/// Разрешение: закрытие возвращает его следующему ожидающему или в запас.
struct Grant {
    origin: Rc<PermitState>,
    closed: Cell<bool>,
}

fn acquire(host: &HostRef, state: &Rc<PermitState>, grant: usize) -> Outcome {
    if state.available.get() > 0 {
        state.available.set(state.available.get() - 1);
        return attach(
            host.make(grant, Vec::new()),
            Rc::new(Grant {
                origin: state.clone(),
                closed: Cell::new(false),
            }),
        );
    }
    let waiter = Waiter::new(Value::Undef);
    state.waiting.borrow_mut().push_back(waiter.clone());
    await_waiter(&waiter)
}

fn release(host: &HostRef, grant_type: usize, grant: &Grant) -> Result<()> {
    if grant.closed.get() {
        return Ok(());
    }
    grant.closed.set(true);
    let state = &grant.origin;
    match take(&state.waiting) {
        Some(waiter) if waiter.promise.running() => {
            // Следующий ожидающий получает новое разрешение сразу.
            let next = attach(
                host.make(grant_type, Vec::new()),
                Rc::new(Grant {
                    origin: state.clone(),
                    closed: Cell::new(false),
                }),
            )?;
            sched::complete(&waiter.promise, Ok(next));
        }
        _ => state.available.set(state.available.get() + 1),
    }
    Ok(())
}

fn declaration(value: &Value) -> Result<usize> {
    Ok(object(value)?.declaration)
}

/// Состояние семафора или барьера создаётся при первом использовании из
/// поля `amount`.
fn lazy_state<T: 'static>(
    host: &HostRef,
    value: &Value,
    what: &str,
    make: impl FnOnce(i64) -> T,
) -> Result<Rc<T>> {
    let object = object(value)?;
    if object.intrinsic().is_none() {
        let amount = host.field(value, "amount")?;
        let amount = amount
            .as_int()
            .ok()
            .and_then(IntValue::to_i64)
            .ok_or_else(|| trap("Cannot read field amount"))?;
        object.set_intrinsic(Rc::new(make(amount)) as Rc<dyn Any>);
    }
    state(value, what)
}

fn semaphore(host: &HostRef, value: &Value) -> Result<Rc<PermitState>> {
    lazy_state(host, value, "Invalid async semaphore", |amount| PermitState {
        available: Cell::new(amount),
        waiting: RefCell::new(VecDeque::new()),
    })
}

// ----- async/barrier -----

struct BarrierState {
    parties: i64,
    generation: Cell<i64>,
    broken: Cell<bool>,
    waiting: RefCell<Vec<Rc<Waiter>>>,
    broken_type: Cell<Option<usize>>,
}

impl BarrierState {
    fn active(&self) -> i64 {
        self.waiting
            .borrow()
            .iter()
            .filter(|waiter| waiter.active.get())
            .count() as i64
    }

    fn break_generation(&self, host: &HostRef) {
        if self.broken.get() {
            return;
        }
        self.broken.set(true);
        let live: Vec<Rc<Waiter>> = self.waiting.borrow_mut().drain(..).collect();
        for waiter in live {
            if waiter.active.get() && waiter.promise.running() {
                waiter.active.set(false);
                let failure = match self.broken_type.get() {
                    Some(broken) => host.thrown(broken, Vec::new()),
                    None => trap("BrokenBarrier is not available"),
                };
                sched::complete(&waiter.promise, Err(failure));
            }
        }
    }
}

fn barrier(host: &HostRef, value: &Value) -> Result<Rc<BarrierState>> {
    lazy_state(host, value, "Invalid async barrier", |amount| BarrierState {
        parties: amount,
        generation: Cell::new(0),
        broken: Cell::new(false),
        waiting: RefCell::new(Vec::new()),
        broken_type: Cell::new(None),
    })
}

/// Снимок состояния барьера: (участники, ожидающие, поколение, сломан).
struct BarrierSnapshot {
    parties: i64,
    waiters: i64,
    generation: i64,
    broken: bool,
}

fn snapshot(value: &Value) -> Result<Rc<BarrierSnapshot>> {
    state(value, "Invalid barrier state")
}

fn broken_type(host: &HostRef, state: &BarrierState, receiver: &Value) -> Result<()> {
    if state.broken_type.get().is_none() {
        let found = host
            .sibling(receiver, "BrokenBarrier")
            .ok_or_else(|| trap("BrokenBarrier is not available"))?;
        state.broken_type.set(Some(found));
    }
    Ok(())
}

// ----- async/timeout -----

/// `result()` ограниченного Future: уже завершённый origin выигрывает без
/// часов; иначе гонка origin и `clock.wait`; проигравший отменяется и
/// дожидается; победа часов — `TimedOut`.
fn race_timeout(
    host: &HostRef,
    timed_out: usize,
    receiver: &Value,
    duration: impl FnOnce(&Value) -> Outcome,
    seconds: usize,
    wait: usize,
) -> Outcome {
    let origin = future(&host.field(receiver, "origin")?)?;
    let clock = host.field(receiver, "clock")?;
    origin.observe();
    sched::register(&origin);
    sched::start(&origin, sched::current_task());
    if origin.done() {
        return origin.outcome();
    }
    let mut timer: Option<Future> = None;
    let mut timer_failure = None;
    let started = (|| -> Result<()> {
        let duration = duration(&clock)?;
        let amount = host.call(seconds, vec![duration.clone()])?;
        let positive = match &amount {
            Value::Rat(rat) => rat.sign() > 0,
            _ => return Err(trap("timeout duration seconds() is not a rat")),
        };
        if positive {
            let started = future(&host.call(wait, vec![clock.clone(), duration])?)?;
            started.observe();
            sched::register(&started);
            sched::start(&started, sched::current_task());
            timer = Some(started);
        }
        Ok(())
    })();
    if let Err(failure) = started {
        timer_failure = Some(failure);
    }
    let first = sched::promise();
    let winner: Rc<RefCell<Option<&'static str>>> = Rc::new(RefCell::new(None));
    {
        let (first, winner) = (first.clone(), winner.clone());
        origin.listen(move || {
            if winner.borrow().is_none() {
                *winner.borrow_mut() = Some("origin");
                sched::settle(&first, Ok(Value::Undef));
            }
        });
    }
    if winner.borrow().is_none() {
        match &timer {
            None => {
                *winner.borrow_mut() = Some("timer");
                sched::settle(&first, Ok(Value::Undef));
            }
            Some(timer) => {
                let (first, winner) = (first.clone(), winner.clone());
                timer.listen(move || {
                    if winner.borrow().is_none() {
                        *winner.borrow_mut() = Some("timer");
                        sched::settle(&first, Ok(Value::Undef));
                    }
                });
            }
        }
    }
    if let Err(cancelled) = sched::await_future(&first) {
        if !sched::is_cancelled(&cancelled) {
            return Err(cancelled);
        }
        *winner.borrow_mut() = Some("parent");
    }
    let winner = winner.borrow().expect("a winner is decided");
    let mut origin_cancelled = false;
    let mut timer_cancelled = false;
    if winner != "origin" && !origin.done() {
        origin_cancelled = true;
        sched::cancel(&origin);
    }
    if let Some(timer) = &timer {
        if winner != "timer" && !timer.done() {
            timer_cancelled = true;
            sched::cancel(timer);
        }
    }
    let _ = sched::await_future(&origin);
    if let Some(timer) = &timer {
        let _ = sched::await_future(timer);
    }
    let mut related = Vec::new();
    if winner != "origin"
        && origin.state() == sched::State::Failed
        && !(origin_cancelled && origin.cancelled())
    {
        related.push(origin.failure().expect("failed origin"));
    }
    if let Some(timer) = &timer {
        if winner != "timer"
            && timer.state() == sched::State::Failed
            && !(timer_cancelled && timer.cancelled())
        {
            related.push(timer.failure().expect("failed timer"));
        }
    }
    if winner == "parent" {
        return Err(sched::cancelled_failure());
    }
    let primary = if winner == "origin" {
        origin.failure().filter(|_| origin.state() == sched::State::Failed)
    } else {
        Some(
            timer_failure
                .or_else(|| {
                    timer
                        .as_ref()
                        .filter(|timer| timer.state() == sched::State::Failed)
                        .and_then(|timer| timer.failure())
                })
                .unwrap_or_else(|| host.thrown(timed_out, Vec::new())),
        )
    };
    if let Some(primary) = primary {
        return Err(primary);
    }
    if let Some(first) = related.into_iter().next() {
        return Err(first);
    }
    Ok(origin.value().unwrap_or(Value::Undef))
}

fn timeout_result(
    host: &HostRef,
    receiver: &Value,
    body: impl FnOnce() -> Outcome + 'static,
) -> Result<Value> {
    let object = object(receiver)?;
    let existing = object.intrinsic();
    if let Some(result) = existing.and_then(|state| state.downcast::<Value>().ok()) {
        return Ok((*result).clone());
    }
    let _ = host;
    let result = op(vec![receiver.clone()], body);
    object.set_intrinsic(Rc::new(result.clone()) as Rc<dyn Any>);
    Ok(result)
}

// ----- тестовый режим и массивы -----

/// Строгое глубокое равенство утверждений: `any` сравнивается по содержимому.
pub fn same(host: &dyn Host, left: &Value, right: &Value) -> Result<bool> {
    match (left, right) {
        (Value::Any(a), Value::Any(b)) => same(host, a, b),
        (Value::Array(a), Value::Array(b)) => {
            if a.0.len() != b.0.len() {
                return Ok(false);
            }
            for (x, y) in a.0.iter().zip(&b.0) {
                if !same(host, x, y)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (Value::Map(a), Value::Map(b)) => {
            if a.0.len() != b.0.len() || a.0.keys().zip(b.0.keys()).any(|(x, y)| x != y) {
                return Ok(false);
            }
            for (x, y) in a.0.values().zip(b.0.values()) {
                if !same(host, x, y)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => host.equal(left, right),
    }
}

/// Порядок двух значений: встроенные — сами, объекты — их `order(other)`.
fn order(host: &HostRef, left: &Value, right: &Value) -> Result<std::cmp::Ordering> {
    use std::cmp::Ordering;
    Ok(match (left, right) {
        (Value::Int(a), Value::Int(b)) => crate::integer::compare(a, b),
        (Value::Str(a), Value::Str(b)) => a.cmp(b),
        // `Double.compare`: -0.0 < 0.0.
        (Value::Float(a), Value::Float(b)) => a.total_cmp(b),
        (Value::Rat(a), Value::Rat(b)) => a.compare(b),
        (Value::Byte(a), Value::Byte(b)) => a.cmp(b),
        (Value::Char(a), Value::Char(b)) => a.cmp(b),
        _ => {
            let result = host.call_method(left, "order", vec![right.clone()])?;
            match result {
                Value::Enum { case, .. } => (case as i64 - 1).cmp(&0),
                _ => return Err(trap("ordering failed: order() is not an Ordering")),
            }
        }
    })
    .map(|order: Ordering| order)
}

fn array_ordering(host: &HostRef, array: &Value) -> Result<Value> {
    let mut sorted = crate::heap::ArrayHeap
        .elements(array)
        .map_err(|e| trap(e.message))?
        .to_vec();
    let mut failure = None;
    sorted.sort_by(|left, right| {
        if failure.is_some() {
            return std::cmp::Ordering::Equal;
        }
        order(host, left, right).unwrap_or_else(|error| {
            failure = Some(error);
            std::cmp::Ordering::Equal
        })
    });
    match failure {
        Some(failure) => Err(failure),
        None => Ok(crate::heap::ArrayHeap.allocate(sorted)),
    }
}

/// Хранилище экземпляра fake: ключи — значения рантайма.
#[derive(Default)]
struct Storage {
    entries: RefCell<Vec<(Value, Value)>>,
}

fn storage(value: &Value) -> Result<Rc<Storage>> {
    let object = object(value)?;
    let existing = object.intrinsic();
    if let Some(state) = existing.and_then(|state| state.downcast::<Storage>().ok()) {
        return Ok(state);
    }
    let state = Rc::new(Storage::default());
    object.set_intrinsic(state.clone() as Rc<dyn Any>);
    Ok(state)
}

fn storage_position(host: &HostRef, storage: &Storage, key: &Value) -> Result<Option<usize>> {
    for (index, (known, _)) in storage.entries.borrow().iter().enumerate() {
        if host.equal(known, key)? {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

fn bool_arg(value: &Value) -> Result<bool> {
    value.as_bool().map_err(|e| trap(e.message))
}

/// Вызов intrinsic `name`: `objects` — объявления, которые он создаёт,
/// `helpers` — функции-переходники программы, `args` — параметры функции.
pub fn call(
    host: &HostRef,
    name: &str,
    objects: &[usize],
    helpers: &[usize],
    args: Vec<Value>,
) -> Result<Value> {
    let name = name.strip_prefix("okkama.").unwrap_or(name);
    let arg = |index: usize| -> Result<Value> {
        args.get(index)
            .cloned()
            .ok_or_else(|| trap(format!("intrinsic {name} is missing argument {index}")))
    };
    let kind = |index: usize| -> Result<usize> {
        objects
            .get(index)
            .copied()
            .ok_or_else(|| trap(format!("intrinsic {name} is missing object {index}")))
    };
    let host = host.clone();
    let unit = Ok(Value::Undef);
    match name {
        // ----- channel -----
        "channel.Channel.new_capacity" => {
            let capacity = arg(0)?;
            let capacity = capacity.as_int().map_err(|e| trap(e.message))?;
            if capacity.is_negative() {
                return Err(host.thrown(kind(1)?, vec![Value::Int(capacity.clone())]));
            }
            let capacity = capacity
                .to_i64()
                .filter(|value| *value <= i64::from(i32::MAX))
                .ok_or_else(|| trap("Channel capacity is too large"))?;
            attach(
                host.make(kind(0)?, Vec::new()),
                Rc::new(ChannelState {
                    capacity: capacity as usize,
                    closed: Cell::new(false),
                    messages: RefCell::new(VecDeque::new()),
                    senders: RefCell::new(VecDeque::new()),
                    receivers: RefCell::new(VecDeque::new()),
                }),
            )
        }
        "channel.Channel.sending" | "channel.Channel.receiving" => {
            attach(host.make(kind(0)?, Vec::new()), channel(&arg(0)?)?)
        }
        "channel.Sending.new"
        | "channel.Receiving.new"
        | "cancellation.CancellationToken.new" => Ok(host.make(kind(0)?, Vec::new())),
        "channel.Sending.send" => {
            let (closed, receiver, value) = (kind(0)?, arg(0)?, arg(1)?);
            let retained = vec![receiver.clone(), value.clone()];
            Ok(op(retained, move || {
                let state = channel(&receiver)?;
                if state.closed.get() {
                    return Err(host.thrown(closed, Vec::new()));
                }
                if let Some(waiting) = take(&state.receivers) {
                    sched::complete(&waiting.promise, Ok(value));
                    return Ok(receiver);
                }
                if state.messages.borrow().len() < state.capacity {
                    state.messages.borrow_mut().push_back(value);
                    return Ok(receiver);
                }
                let waiter = Waiter::new(value);
                state.senders.borrow_mut().push_back(waiter.clone());
                await_waiter(&waiter)?;
                Ok(receiver)
            }))
        }
        "channel.Receiving.item" => {
            let (closed, receiver) = (kind(0)?, arg(0)?);
            Ok(op(vec![receiver.clone()], move || {
                let state = channel(&receiver)?;
                let message = state.messages.borrow_mut().pop_front();
                if let Some(value) = message {
                    if !state.closed.get() {
                        state.admit_waiting_sender();
                    }
                    return Ok(value);
                }
                if let Some(sender) = take(&state.senders) {
                    sched::settle(&sender.promise, Ok(Value::Undef));
                    return Ok(sender.value.clone());
                }
                if state.closed.get() {
                    return Err(host.thrown(closed, Vec::new()));
                }
                let waiter = Waiter::new(Value::Undef);
                state.receivers.borrow_mut().push_back(waiter.clone());
                await_waiter(&waiter)
            }))
        }
        "channel.Sending.close" | "channel.Receiving.close" => {
            channel(&arg(0)?)?.close(&host, kind(0)?);
            unit
        }
        // ----- event -----
        "event.Event.new" => attach(
            host.make(kind(0)?, vec![Value::Bool(false)]),
            Rc::new(EventState {
                ready: Cell::new(false),
                waiting: RefCell::new(VecDeque::new()),
            }),
        ),
        "event.Event.new_ready" => {
            let ready = arg(0)?;
            let flag = bool_arg(&ready)?;
            attach(
                host.make(kind(0)?, vec![ready]),
                Rc::new(EventState {
                    ready: Cell::new(flag),
                    waiting: RefCell::new(VecDeque::new()),
                }),
            )
        }
        "event.Event.ready" => Ok(Value::Bool(event(&arg(0)?)?.ready.get())),
        "event.Event.set" => {
            let state = event(&arg(0)?)?;
            if !state.ready.get() {
                state.ready.set(true);
                release_all(&state.waiting);
            }
            unit
        }
        "event.Event.wait" => {
            let receiver = arg(0)?;
            Ok(op(vec![receiver.clone()], move || {
                let state = event(&receiver)?;
                wait_until(state.ready.get(), &state.waiting, receiver)
            }))
        }
        // ----- atomic -----
        "atomic.Atomic.new_value" => attach(
            host.make(kind(0)?, Vec::new()),
            Rc::new(AtomicState {
                value: RefCell::new(arg(0)?),
                closed: Cell::new(false),
            }),
        ),
        "atomic.Atomic.value" => Ok(open_atomic(&host, kind(0)?, &arg(0)?)?.value.borrow().clone()),
        "atomic.Atomic.replace" => {
            *open_atomic(&host, kind(0)?, &arg(0)?)?.value.borrow_mut() = arg(1)?;
            unit
        }
        "atomic.Atomic.compare_and_replace" => {
            let state = open_atomic(&host, kind(0)?, &arg(0)?)?;
            let current = state.value.borrow().clone();
            if !host.equal(&current, &arg(1)?)? {
                return Ok(Value::Bool(false));
            }
            *state.value.borrow_mut() = arg(2)?;
            Ok(Value::Bool(true))
        }
        "atomic.Atomic.closed" => Ok(Value::Bool(atomic(&arg(0)?)?.closed.get())),
        "atomic.Atomic.close" => {
            let state = atomic(&arg(0)?)?;
            state.closed.set(true);
            *state.value.borrow_mut() = Value::Undef;
            unit
        }
        // ----- cancellation -----
        "cancellation.Cancellation.new" => attach(
            host.make(kind(0)?, Vec::new()),
            Rc::new(CancelState {
                requested: Cell::new(false),
                waiting: RefCell::new(VecDeque::new()),
            }),
        ),
        "cancellation.Cancellation.cancelling" => {
            attach(host.make(kind(0)?, Vec::new()), cancellation(&arg(0)?)?)
        }
        "cancellation.Cancellation.cancel" => {
            let state = cancellation(&arg(0)?)?;
            if !state.requested.get() {
                state.requested.set(true);
                release_all(&state.waiting);
            }
            unit
        }
        "cancellation.CancellationToken.requested" => {
            Ok(Value::Bool(cancellation(&arg(0)?)?.requested.get()))
        }
        "cancellation.CancellationToken.check" => {
            if cancellation(&arg(0)?)?.requested.get() {
                return Err(sched::cancelled_failure());
            }
            unit
        }
        "cancellation.CancellationToken.wait" => {
            let receiver = arg(0)?;
            Ok(op(vec![receiver.clone()], move || {
                let state = cancellation(&receiver)?;
                wait_until(state.requested.get(), &state.waiting, receiver)
            }))
        }
        // ----- scope -----
        "scope.FutureGroup.new" => attach(
            host.make(kind(0)?, Vec::new()),
            Rc::new(GroupState::default()),
        ),
        "scope.FutureGroup.start" => {
            let state = group(&arg(0)?)?;
            let origin = future(&arg(1)?)?;
            if contains(&state.entries.borrow(), &origin) {
                return unit;
            }
            if state.sealed.get() {
                return Err(host.thrown(kind(0)?, Vec::new()));
            }
            state.entries.borrow_mut().push(origin.clone());
            state.group_cancelled.borrow_mut().push(false);
            state.active.set(state.active.get() + 1);
            sched::register(&origin);
            sched::start(&origin, None);
            let counter = state.clone();
            origin.listen(move || counter.active.set(counter.active.get() - 1));
            unit
        }
        "scope.FutureGroup.active" => Ok(int(group(&arg(0)?)?.active.get())),
        "scope.FutureGroup.cancel" => {
            let state = group(&arg(0)?)?;
            cancel_group(&state);
            unit
        }
        "scope.FutureGroup.wait" => Ok(group_wait(arg(0)?)),
        "scope.FutureScope.result" => {
            let (group_type, receiver) = (kind(0)?, arg(0)?);
            let body_receiver = receiver.clone();
            cached(&receiver, move || {
                op(vec![body_receiver.clone()], move || {
                    let receiver = body_receiver;
                    let group = attach(
                        host.make(group_type, Vec::new()),
                        Rc::new(GroupState::default()),
                    )?;
                    let action = host.field(&receiver, "action")?;
                    let body = host
                        .call_fn(&action, vec![group.clone()])
                        .and_then(|started| sched::await_future(&future(&started)?));
                    let (value, body) = match body {
                        Ok(value) => (value, None),
                        Err(failure) => (Value::Undef, Some(failure)),
                    };
                    if body.is_some() {
                        let state = self::group(&group)?;
                        cancel_group(&state);
                    }
                    let waited = sched::await_future(&future(&group_wait(group))?).err();
                    if let Some(body) = body {
                        if let Some(waited) = waited {
                            if !Rc::ptr_eq(&waited, &body) {
                                body.with_secondary(waited);
                            }
                        }
                        return Err(body);
                    }
                    if let Some(waited) = waited {
                        return Err(waited);
                    }
                    Ok(value)
                })
            })
        }
        // ----- collect -----
        "collect.AllFutures.results" => {
            let (sequence, receiver) = (kind(0)?, arg(0)?);
            let body_receiver = receiver.clone();
            cached(&receiver, move || {
                op(vec![body_receiver.clone()], move || {
                    all_futures(&host, sequence, &body_receiver)
                })
            })
        }
        "collect.Raced.result" => {
            let (none, receiver) = (kind(0)?, arg(0)?);
            let body_receiver = receiver.clone();
            cached(&receiver, move || {
                op(vec![body_receiver.clone()], move || raced(&host, none, &body_receiver))
            })
        }
        "collect.FirstSuccessful.result" => {
            let (none, receiver) = (kind(0)?, arg(0)?);
            let body_receiver = receiver.clone();
            cached(&receiver, move || {
                op(vec![body_receiver.clone()], move || {
                    first_successful(&host, none, &body_receiver)
                })
            })
        }
        "collect.FutureCompletions.remaining_count" => {
            let state = completions(&host, &arg(0)?)?;
            Ok(int((state.origins.len() - state.delivered.get()) as i64))
        }
        "collect.FutureCompletions.completion" => {
            let (none, receiver) = (kind(0)?, arg(0)?);
            let state = completions(&host, &receiver)?;
            Ok(op(vec![receiver], move || completion(&host, none, state)))
        }
        // ----- lock, semaphore -----
        "lock.Lock.new" => attach(
            host.make(kind(0)?, Vec::new()),
            Rc::new(PermitState {
                available: Cell::new(1),
                waiting: RefCell::new(VecDeque::new()),
            }),
        ),
        "lock.Lock.locked" => Ok(Value::Bool(
            state::<PermitState>(&arg(0)?, "Invalid async lock")?
                .available
                .get()
                == 0,
        )),
        "lock.Lock.guard" => {
            let (guard, receiver) = (kind(0)?, arg(0)?);
            Ok(op(vec![receiver.clone()], move || {
                let state = state::<PermitState>(&receiver, "Invalid async lock")?;
                acquire(&host, &state, guard)
            }))
        }
        "lock.LockGuard.closed" => Ok(Value::Bool(
            state::<Grant>(&arg(0)?, "Invalid async lock guard")?
                .closed
                .get(),
        )),
        "lock.LockGuard.close" => {
            let receiver = arg(0)?;
            let grant = state::<Grant>(&receiver, "Invalid async lock guard")?;
            release(&host, declaration(&receiver)?, &grant)?;
            unit
        }
        "semaphore.Semaphore.permits" => Ok(int(semaphore(&host, &arg(0)?)?.available.get())),
        "semaphore.Semaphore.permit" => {
            let (permit, receiver) = (kind(0)?, arg(0)?);
            Ok(op(vec![receiver.clone()], move || {
                let state = semaphore(&host, &receiver)?;
                acquire(&host, &state, permit)
            }))
        }
        "semaphore.Permit.closed" => Ok(Value::Bool(
            state::<Grant>(&arg(0)?, "Invalid async semaphore permit")?
                .closed
                .get(),
        )),
        "semaphore.Permit.close" => {
            let receiver = arg(0)?;
            let grant = state::<Grant>(&receiver, "Invalid async semaphore permit")?;
            release(&host, declaration(&receiver)?, &grant)?;
            unit
        }
        // ----- barrier -----
        "barrier.Barrier.wait" => {
            let (broken, receiver) = (kind(0)?, arg(0)?);
            Ok(op(vec![receiver.clone()], move || {
                let state = barrier(&host, &receiver)?;
                state.broken_type.set(Some(broken));
                if state.broken.get() {
                    return Err(host.thrown(broken, Vec::new()));
                }
                let waiter = Waiter::new(int(state.active()));
                state.waiting.borrow_mut().push(waiter.clone());
                if state.active() == state.parties {
                    let arrivals: Vec<Rc<Waiter>> = state.waiting.borrow_mut().drain(..).collect();
                    state.generation.set(state.generation.get() + 1);
                    for arrival in arrivals {
                        if arrival.active.get() && arrival.promise.running() {
                            arrival.active.set(false);
                            sched::complete(&arrival.promise, Ok(arrival.value.clone()));
                        }
                    }
                }
                match sched::await_future(&waiter.promise) {
                    Err(failure) => {
                        if waiter.active.get() {
                            // Отменённый участник ломает поколение.
                            waiter.active.set(false);
                            state.break_generation(&host);
                        }
                        Err(failure)
                    }
                    ok => ok,
                }
            }))
        }
        "barrier.Barrier.state" => {
            let state = barrier(&host, &arg(0)?)?;
            attach(
                host.make(kind(0)?, Vec::new()),
                Rc::new(BarrierSnapshot {
                    parties: state.parties,
                    waiters: state.active(),
                    generation: state.generation.get(),
                    broken: state.broken.get(),
                }),
            )
        }
        "barrier.Barrier.abort" => {
            let receiver = arg(0)?;
            let state = barrier(&host, &receiver)?;
            broken_type(&host, &state, &receiver)?;
            state.break_generation(&host);
            unit
        }
        "barrier.Barrier.reset" => {
            let receiver = arg(0)?;
            let state = barrier(&host, &receiver)?;
            if state.active() > 0 {
                broken_type(&host, &state, &receiver)?;
                state.break_generation(&host);
            }
            state.generation.set(state.generation.get() + 1);
            state.broken.set(false);
            state.waiting.borrow_mut().clear();
            unit
        }
        "barrier.BarrierState.parties" => Ok(int(snapshot(&arg(0)?)?.parties)),
        "barrier.BarrierState.waiters" => Ok(int(snapshot(&arg(0)?)?.waiters)),
        "barrier.BarrierState.generation" => Ok(int(snapshot(&arg(0)?)?.generation)),
        "barrier.BarrierState.broken" => Ok(Value::Bool(snapshot(&arg(0)?)?.broken)),
        // ----- timeout -----
        "timeout.Within.result" => {
            let (timed_out, receiver) = (kind(0)?, arg(0)?);
            let (seconds, wait) = (helper(helpers, 0)?, helper(helpers, 1)?);
            let body_host = host.clone();
            let body_receiver = receiver.clone();
            timeout_result(&host, &receiver, move || {
                let limit_host = body_host.clone();
                let limit_receiver = body_receiver.clone();
                race_timeout(
                    &body_host,
                    timed_out,
                    &body_receiver,
                    move |_| limit_host.field(&limit_receiver, "limit"),
                    seconds,
                    wait,
                )
            })
        }
        "timeout.Before.result" => {
            let (timed_out, receiver) = (kind(0)?, arg(0)?);
            let (now, minus) = (helper(helpers, 0)?, helper(helpers, 1)?);
            let (seconds, wait) = (helper(helpers, 2)?, helper(helpers, 3)?);
            let body_host = host.clone();
            let body_receiver = receiver.clone();
            timeout_result(&host, &receiver, move || {
                let deadline_host = body_host.clone();
                let deadline_receiver = body_receiver.clone();
                race_timeout(
                    &body_host,
                    timed_out,
                    &body_receiver,
                    move |clock| {
                        let instant = deadline_host.call(now, vec![clock.clone()])?;
                        let deadline = deadline_host.field(&deadline_receiver, "deadline")?;
                        deadline_host.call(minus, vec![deadline, instant])
                    },
                    seconds,
                    wait,
                )
            })
        }
        // ----- тестовый режим -----
        "test.ok" => {
            if !bool_arg(&arg(0)?)? {
                host.test_failure("expected true, got false".into())?;
            }
            unit
        }
        "test.is" => {
            let (actual, expected) = (arg(0)?, arg(1)?);
            if !same(&*host, &actual, &expected)? {
                host.test_failure(format!(
                    "expected {}, got {}",
                    host.show(&expected),
                    host.show(&actual)
                ))?;
            }
            unit
        }
        "test.isnt" => {
            let (actual, expected) = (arg(0)?, arg(1)?);
            if same(&*host, &actual, &expected)? {
                host.test_failure(format!(
                    "expected values to differ, both were {}",
                    host.show(&actual)
                ))?;
            }
            unit
        }
        "test.fail" => {
            let message = arg(0)?;
            host.test_failure(message.as_string().map_err(|e| trap(e.message))?.to_owned())?;
            unit
        }
        "fake.storage.set" => {
            let (receiver, key, value) = (arg(0)?, arg(1)?, arg(2)?);
            let storage = storage(&receiver)?;
            match storage_position(&host, &storage, &key)? {
                Some(index) => storage.entries.borrow_mut()[index].1 = value,
                None => storage.entries.borrow_mut().push((key, value)),
            }
            unit
        }
        "fake.storage.get" | "fake.storage.remove" => {
            let (receiver, key) = (arg(0)?, arg(1)?);
            let storage = storage(&receiver)?;
            let Some(index) = storage_position(&host, &storage, &key)? else {
                return Err(trap(format!("Fake storage has no key {}", host.show(&key))));
            };
            if name == "fake.storage.get" {
                return Ok(storage.entries.borrow()[index].1.clone());
            }
            storage.entries.borrow_mut().remove(index);
            unit
        }
        "fake.storage.has" => {
            let (receiver, key) = (arg(0)?, arg(1)?);
            let storage = storage(&receiver)?;
            Ok(Value::Bool(storage_position(&host, &storage, &key)?.is_some()))
        }
        "fake.storage.clear" => {
            storage(&arg(0)?)?.entries.borrow_mut().clear();
            unit
        }
        "fake.storage.size" => Ok(int(storage(&arg(0)?)?.entries.borrow().len() as i64)),
        "array.ordering" => array_ordering(&host, &arg(0)?),
        other => Err(trap(format!("intrinsic okkama.{other} is not available"))),
    }
}

fn helper(helpers: &[usize], index: usize) -> Result<usize> {
    helpers
        .get(index)
        .copied()
        .ok_or_else(|| trap(format!("intrinsic helper {index} is missing")))
}
