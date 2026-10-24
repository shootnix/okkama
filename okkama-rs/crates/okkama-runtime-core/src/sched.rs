//! Кооперативный планировщик (OIR-1 §13, §18 спецификации) — перенос
//! `okkama.rt.Scheduler` JVM. Исполняется ровно одна задача: `transfer` отдаёт
//! управление и возвращается, когда его вернут. Запуск Future исполняет тело
//! до первой приостановки сразу; завершение сразу продолжает ожидающих по
//! порядку; `spawn` запускает задачу немедленно. Внешние события (таймеры,
//! ввод-вывод) приходят в очередь цикла `async main`.
//!
//! Задача — стековая корутина (`corosensei`) в том же потоке: JVM даёт каждой
//! задаче поток и передаёт управление семафорами, здесь та же передача —
//! `resume`/`suspend` без потоков. Поэтому тело задачи — обычный код
//! исполнителя: `await` приостанавливает корутину прямо внутри инструкции.
#![allow(unsafe_code)]

use crate::Value;
use crate::failure::{Failure, Kind};
use corosensei::stack::{DefaultStack, Stack};
use corosensei::{Coroutine, CoroutineResult, Yielder};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::rc::Rc;

/// Исход Future или тела задачи.
pub type Outcome = Result<Value, Rc<Failure>>;
/// Тело холодного Future: код исполнителя без аргументов.
pub type Body = Box<dyn FnOnce() -> Outcome>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    Cold,
    Running,
    Done,
    Failed,
}

/// Future Okkama: холодный до первого `await` или `spawn`; исход (значение,
/// отказ, отмена) кэшируется — повторный `await` не повторяет тело.
pub struct FutureCell {
    state: Cell<State>,
    value: RefCell<Option<Value>>,
    failure: RefCell<Option<Rc<Failure>>>,
    body: RefCell<Option<Body>>,
    /// Захваты тела: через них регион видит владеемые объекты.
    retained: RefCell<Vec<Value>>,
    /// Продолжения в порядке регистрации: ожидающие задачи и слушатели.
    waiters: RefCell<VecDeque<Waiter>>,
    task: RefCell<Option<Rc<Task>>>,
    /// Задача, чей `await` запустил этот Future (отмена идёт вглубь).
    starter: RefCell<Option<Rc<Task>>>,
    observed: Cell<bool>,
    cancel_requested: Cell<bool>,
}

pub type Future = Rc<FutureCell>;

impl fmt::Debug for FutureCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Future")
            .field("state", &self.state.get())
            .finish_non_exhaustive()
    }
}

enum Waiter {
    Task(Rc<Task>),
    Listener(Box<dyn FnOnce()>),
}

/// Задача планировщика: корутина, исполняющая тело одного Future (у задачи
/// запуска корутины нет).
pub struct Task {
    coroutine: RefCell<Option<Coroutine<(), (), (), DefaultStack>>>,
    yielder: Cell<*const Yielder<(), ()>>,
    future: Option<Future>,
    /// Future, которого задача сейчас ждёт.
    awaiting: RefCell<Option<Future>>,
    /// Отмена доставляется в ближайшей точке приостановки.
    cancel_pending: Cell<bool>,
    /// Нижняя граница стека корутины (0 — неизвестна).
    floor: Cell<usize>,
    /// Активные входы в машинный код на стеке задачи: такой стек нельзя
    /// раскручивать (у кода JIT нет таблиц раскрутки).
    native: Cell<u32>,
}

impl fmt::Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task").finish_non_exhaustive()
    }
}

impl Task {
    fn root() -> Rc<Self> {
        Rc::new(Self {
            coroutine: RefCell::new(None),
            yielder: Cell::new(std::ptr::null()),
            future: None,
            awaiting: RefCell::new(None),
            cancel_pending: Cell::new(false),
            floor: Cell::new(0),
            native: Cell::new(0),
        })
    }
}

/// Исход внешней операции хоста: вычислен в рабочем потоке, доставляется в
/// цикле событий потока программы.
type Delivery = Box<dyn std::any::Any + Send>;

struct Scheduler {
    current: RefCell<Rc<Task>>,
    /// Future, запущенные `spawn`: их исход проверяет завершение программы.
    registered: RefCell<Vec<Future>>,
    /// Внешние операции, исхода которых ждёт программа.
    external: Cell<usize>,
    /// Незавершённые задачи: их корутины освобождаются при завершении запуска.
    live: RefCell<HashMap<*const Task, Rc<Task>>>,
    /// Свободные стеки корутин.
    stacks: RefCell<Vec<DefaultStack>>,
    sender: std::sync::mpsc::Sender<(u64, Delivery)>,
    receiver: std::sync::mpsc::Receiver<(u64, Delivery)>,
    /// Продолжения внешних операций по номеру.
    pending: RefCell<HashMap<u64, Box<dyn FnOnce(Delivery)>>>,
    next_op: Cell<u64>,
}

impl Scheduler {
    fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self {
            current: RefCell::new(Task::root()),
            registered: RefCell::new(Vec::new()),
            external: Cell::new(0),
            live: RefCell::new(HashMap::new()),
            stacks: RefCell::new(Vec::new()),
            sender,
            receiver,
            pending: RefCell::new(HashMap::new()),
            next_op: Cell::new(0),
        }
    }
}

thread_local! {
    static SCHEDULER: Scheduler = Scheduler::new();
    /// Нижняя граница стека, на котором сейчас исполняется код (0 — неизвестна).
    static FLOOR: Cell<usize> = const { Cell::new(0) };
    /// Входы в машинный код текущей задачи; у приостановленной — в `Task::native`.
    static NATIVE: Cell<u32> = const { Cell::new(0) };
}

/// Нижняя граница текущего стека: машинный код (JIT) проверяет по ней запас
/// перед вложенным вызовом. 0 — граница неизвестна.
pub fn stack_floor() -> usize {
    FLOOR.with(Cell::get)
}

/// Адрес нижней границы текущего стека (читает машинный код).
pub fn floor_address() -> *const usize {
    FLOOR.with(|cell| cell.as_ptr().cast_const())
}

/// Вход в машинный код на стеке текущей задачи.
pub fn native_enter() {
    NATIVE.with(|native| native.set(native.get() + 1));
}

/// Выход из машинного кода.
pub fn native_leave() {
    NATIVE.with(|native| native.set(native.get().saturating_sub(1)));
}

/// Исполнить `f`, считая нижней границей стека `floor`.
fn on_floor<R>(floor: usize, f: impl FnOnce() -> R) -> R {
    let saved = FLOOR.with(|cell| cell.replace(floor));
    let result = f();
    FLOOR.with(|cell| cell.set(saved));
    result
}

/// Стек задачи: виртуальная память, физическая выделяется по мере роста.
const STACK: usize = 64 << 20;
const IDLE_STACKS: usize = 64;

fn with<R>(f: impl FnOnce(&Scheduler) -> R) -> R {
    SCHEDULER.with(f)
}

fn current() -> Rc<Task> {
    with(|s| s.current.borrow().clone())
}

/// Исполнить `f` на стеке 256 МБ, как поток задачи JVM: глубокая рекурсия
/// providers (разбор JSON, разметки) не зависит от стека вызывающего потока.
pub fn on_large_stack<R>(f: impl FnOnce() -> R) -> R {
    let stack = DefaultStack::new(256 << 20).expect("program stack");
    let floor = stack.limit().get();
    corosensei::on_stack(stack, move || on_floor(floor, f))
}

/// Задача, у которой сейчас управление (стартер для `start`).
pub fn current_task() -> Option<Rc<Task>> {
    Some(current())
}

/// Сбросить планировщик перед запуском программы и освободить корутины
/// незавершённых задач после него (их стеки раскручиваются).
pub fn reset() {
    let live: Vec<Rc<Task>> = with(|s| {
        *s.current.borrow_mut() = Task::root();
        s.registered.borrow_mut().clear();
        s.external.set(0);
        s.pending.borrow_mut().clear();
        while s.receiver.try_recv().is_ok() {}
        s.live.borrow_mut().drain().map(|(_, task)| task).collect()
    });
    for task in live {
        let mut coroutine = task.coroutine.borrow_mut().take();
        if task.native.get() > 0
            && let Some(coroutine) = coroutine.as_mut()
        {
            // SAFETY: стек с кадрами машинного кода не раскручивается: значения
            // на нём не освобождаются (запуск завершён), стек переиспользуется.
            unsafe { coroutine.force_reset() };
        }
        drop(coroutine);
        task.awaiting.borrow_mut().take();
        if let Some(future) = &task.future {
            future.task.borrow_mut().take();
            future.starter.borrow_mut().take();
            future.waiters.borrow_mut().clear();
        }
    }
}

impl FutureCell {
    fn new(state: State, body: Option<Body>, retained: Vec<Value>) -> Future {
        Rc::new(Self {
            state: Cell::new(state),
            value: RefCell::new(None),
            failure: RefCell::new(None),
            body: RefCell::new(body),
            retained: RefCell::new(retained),
            waiters: RefCell::new(VecDeque::new()),
            task: RefCell::new(None),
            starter: RefCell::new(None),
            observed: Cell::new(false),
            cancel_requested: Cell::new(false),
        })
    }

    pub fn state(&self) -> State {
        self.state.get()
    }

    pub fn observe(&self) {
        self.observed.set(true);
    }

    /// Значения, через которые регион видит объекты Future.
    pub fn retained(&self) -> Vec<Value> {
        let mut values = self.retained.borrow().clone();
        values.extend(self.value.borrow().iter().cloned());
        values
    }

    pub fn value(&self) -> Option<Value> {
        self.value.borrow().clone()
    }

    pub fn failure(&self) -> Option<Rc<Failure>> {
        self.failure.borrow().clone()
    }

    /// Исход завершённого Future.
    pub fn outcome(&self) -> Outcome {
        match self.state.get() {
            State::Failed => Err(self.failure().expect("failed future has a failure")),
            _ => Ok(self.value().unwrap_or(Value::Undef)),
        }
    }

    /// Слушатель исхода: сразу, если Future завершён, иначе при завершении.
    pub fn listen(self: &Rc<Self>, listener: impl FnOnce() + 'static) {
        if self.state.get() >= State::Done {
            listener();
        } else {
            self.waiters
                .borrow_mut()
                .push_back(Waiter::Listener(Box::new(listener)));
        }
    }

    /// Сильные рёбра для сборщика циклов (тело и ожидающие непрозрачны).
    pub(crate) fn edges(&self, out: &mut crate::gc::Edges) -> bool {
        let (Ok(value), Ok(failure), Ok(retained)) =
            (self.value.try_borrow(), self.failure.try_borrow(), self.retained.try_borrow())
        else {
            return false;
        };
        value.iter().for_each(|value| out.value(value));
        failure.iter().for_each(|failure| out.failure(failure));
        retained.iter().for_each(|value| out.value(value));
        true
    }

    /// Разорвать поздние рёбра недостижимого Future.
    pub(crate) fn clear(&self, sink: &mut Vec<Value>, failures: &mut Vec<Rc<Failure>>) {
        if let Ok(mut value) = self.value.try_borrow_mut() {
            sink.extend(value.take());
        }
        if let Ok(mut failure) = self.failure.try_borrow_mut() {
            failures.extend(failure.take());
        }
        if let Ok(mut retained) = self.retained.try_borrow_mut() {
            sink.append(&mut retained);
        }
    }

    pub fn done(&self) -> bool {
        self.state.get() >= State::Done
    }

    /// Завершён отменой: отказ — `Cancelled`.
    pub fn cancelled(&self) -> bool {
        self.state.get() == State::Failed
            && self.failure.borrow().as_ref().is_some_and(|f| is_cancelled(f))
    }

    pub fn running(&self) -> bool {
        self.state.get() == State::Running
    }
}

/// Вызов async-функции: холодный Future над телом.
pub fn cold(body: Body, retained: Vec<Value>) -> Future {
    FutureCell::new(State::Cold, Some(body), retained)
}

/// Обещание: исход задаёт рантайм (`complete`).
pub fn promise() -> Future {
    FutureCell::new(State::Running, None, Vec::new())
}

/// Завершённое обещание: исход, известный сразу.
pub fn resolved(outcome: Outcome) -> Future {
    let promise = promise();
    complete(&promise, outcome);
    promise
}

/// Отказ `Cancelled` (системный Error, OIR-1 §9).
pub fn cancelled_failure() -> Rc<Failure> {
    Failure::new(Kind::Thrown {
        object: crate::heap::Ref(Rc::new(crate::heap::Object::new(
            crate::heap::CANCELLED,
            Vec::new(),
        ))),
        name: "Cancelled".into(),
        message: Some("Operation was cancelled".into()),
    })
}

pub fn is_cancelled(failure: &Failure) -> bool {
    matches!(&failure.kind, Kind::Thrown { object, .. } if object.declaration == crate::heap::CANCELLED)
}

pub fn register(future: &Future) {
    with(|s| {
        let mut registered = s.registered.borrow_mut();
        if !registered.iter().any(|known| Rc::ptr_eq(known, future)) {
            registered.push(future.clone());
        }
    });
}

/// Отдать управление `target`; вернуться, когда оно его вернёт.
fn transfer(target: &Rc<Task>) {
    let me = current();
    with(|s| *s.current.borrow_mut() = target.clone());
    let mut coroutine = target
        .coroutine
        .borrow_mut()
        .take()
        .expect("a resumed task is suspended");
    let saved = NATIVE.with(|native| native.replace(target.native.get()));
    let finished = matches!(
        on_floor(target.floor.get(), || coroutine.resume(())),
        CoroutineResult::Return(())
    );
    target.native.set(NATIVE.with(|native| native.replace(saved)));
    if finished {
        with(|s| {
            s.live.borrow_mut().remove(&Rc::as_ptr(target));
            let stack = coroutine.into_stack();
            let mut stacks = s.stacks.borrow_mut();
            if stacks.len() < IDLE_STACKS {
                stacks.push(stack);
            }
        });
    } else {
        *target.coroutine.borrow_mut() = Some(coroutine);
    }
    with(|s| *s.current.borrow_mut() = me);
}

/// Приостановить текущую задачу: управление — тому, кто её продолжил.
fn suspend() {
    let me = current();
    let yielder = me.yielder.get();
    assert!(!yielder.is_null(), "the launch task cannot suspend");
    drop(me);
    // SAFETY: указатель задаёт корутина этой задачи при старте; Yielder живёт
    // на её стеке до её завершения, а приостанавливается только исполняемая
    // сейчас задача — то есть её корутина активна.
    unsafe { (*yielder).suspend(()) };
}

/// Запустить холодный Future: тело исполняется до первой приостановки.
pub fn start(future: &Future, starter: Option<Rc<Task>>) {
    if future.state.get() != State::Cold {
        return;
    }
    future.state.set(State::Running);
    *future.starter.borrow_mut() = starter;
    let task = Rc::new(Task {
        coroutine: RefCell::new(None),
        yielder: Cell::new(std::ptr::null()),
        future: Some(future.clone()),
        awaiting: RefCell::new(None),
        cancel_pending: Cell::new(false),
        floor: Cell::new(0),
        native: Cell::new(0),
    });
    *future.task.borrow_mut() = Some(task.clone());
    let body = future.body.borrow_mut().take().expect("cold future has a body");
    let stack = with(|s| s.stacks.borrow_mut().pop())
        .map_or_else(|| DefaultStack::new(STACK), Ok)
        .expect("coroutine stack");
    task.floor.set(stack.limit().get());
    let runner = Rc::downgrade(&task);
    let coroutine = Coroutine::with_stack(stack, move |yielder: &Yielder<(), ()>, ()| {
        let task = runner.upgrade().expect("a running task is alive");
        task.yielder.set(yielder as *const _);
        let future = task.future.clone().expect("task runs a future");
        drop(task);
        let outcome = body();
        complete(&future, outcome);
    });
    *task.coroutine.borrow_mut() = Some(coroutine);
    with(|s| s.live.borrow_mut().insert(Rc::as_ptr(&task), task.clone()));
    transfer(&task);
}

/// Исход Future; ожидающие продолжаются сразу, в порядке ожидания.
pub fn complete(future: &Future, outcome: Outcome) {
    // Исход — позднее ребро: через него Future может замкнуть цикл.
    let traced = match &outcome {
        Ok(value) => crate::gc::traced(value),
        Err(_) => true,
    };
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
    if traced {
        crate::gc::track_future(future);
    }
    future.task.borrow_mut().take();
    future.starter.borrow_mut().take();
    loop {
        let next = future.waiters.borrow_mut().pop_front();
        match next {
            None => break,
            Some(Waiter::Task(task)) => transfer(&task),
            Some(Waiter::Listener(listener)) => listener(),
        }
    }
}

/// Исход обещания, если его ещё не задала отмена.
pub fn settle(promise: &Future, outcome: Outcome) {
    if promise.running() {
        complete(promise, outcome);
    }
}

/// `await`: запуск холодного Future, затем ожидание исхода.
pub fn await_future(future: &Future) -> Outcome {
    future.observed.set(true);
    let me = current();
    start(future, Some(me.clone()));
    while future.state.get() == State::Running {
        future
            .waiters
            .borrow_mut()
            .push_back(Waiter::Task(me.clone()));
        *me.awaiting.borrow_mut() = Some(future.clone());
        suspend();
        me.awaiting.borrow_mut().take();
        if me.cancel_pending.get() {
            me.cancel_pending.set(false);
            future
                .waiters
                .borrow_mut()
                .retain(|waiter| !matches!(waiter, Waiter::Task(task) if Rc::ptr_eq(task, &me)));
            return Err(cancelled_failure());
        }
    }
    future.outcome()
}

/// `fut.spawn`: запустить сейчас (до первой приостановки).
pub fn spawn(future: &Future) {
    register(future);
    start(future, None);
}

/// `fut.cancel`: идемпотентная кооперативная отмена.
pub fn cancel(future: &Future) {
    if future.state.get() == State::Cold {
        future.body.borrow_mut().take();
        *future.failure.borrow_mut() = Some(cancelled_failure());
        future.state.set(State::Failed);
        return;
    }
    if future.state.get() != State::Running || future.cancel_requested.get() {
        return;
    }
    future.cancel_requested.set(true);
    let task = future.task.borrow().clone();
    let Some(task) = task else {
        // Обещание рантайма: отмена завершает его отказом `Cancelled`.
        complete(future, Err(cancelled_failure()));
        return;
    };
    task.cancel_pending.set(true);
    let awaiting = task.awaiting.borrow().clone();
    let Some(inner) = awaiting else {
        return;
    };
    if Rc::ptr_eq(&task, &current()) {
        return;
    }
    let started_here = inner
        .starter
        .borrow()
        .as_ref()
        .is_some_and(|starter| Rc::ptr_eq(starter, &task));
    if started_here && inner.state.get() == State::Running {
        // Отмена идёт вглубь цепочки: внутренний завершится отменой и
        // продолжит эту задачу как ожидающую.
        cancel(&inner);
        if future.state.get() != State::Running || task.awaiting.borrow().is_none() {
            return;
        }
    }
    inner
        .waiters
        .borrow_mut()
        .retain(|waiter| !matches!(waiter, Waiter::Task(other) if Rc::ptr_eq(other, &task)));
    transfer(&task);
}

/// Ошибка программы цикла событий (`Rt.trap`).
fn trap(message: &str) -> Rc<Failure> {
    Failure::new(Kind::Program(message.into()))
}

/// Обрабатывать внешние события, пока `until` не завершён.
fn drive(until: &Future) -> Result<(), Rc<Failure>> {
    while until.state.get() == State::Running {
        if with(|s| s.external.get()) == 0 {
            return Err(trap("async main cannot complete: no task can make progress"));
        }
        let (id, delivery) = with(|s| s.receiver.recv())
            .map_err(|_| trap("async runtime was interrupted"))?;
        let done = with(|s| s.pending.borrow_mut().remove(&id));
        if let Some(done) = done {
            done(delivery);
        }
    }
    Ok(())
}

fn shutdown() -> Result<Option<Rc<Failure>>, Rc<Failure>> {
    let registered = with(|s| s.registered.borrow().clone());
    for future in &registered {
        if future.state.get() < State::Done {
            cancel(future);
        }
    }
    let registered = with(|s| s.registered.borrow().clone());
    for future in &registered {
        while future.state.get() == State::Running && with(|s| s.external.get()) > 0 {
            drive(future)?;
        }
    }
    let registered = with(|s| s.registered.borrow().clone());
    Ok(registered
        .iter()
        .find(|future| {
            !future.observed.get() && future.state.get() == State::Failed && !future.cancelled()
        })
        .and_then(|future| future.failure()))
}

/// `async main`: запуск корня и цикл внешних событий до его исхода, затем
/// завершение: незавершённые задачи `spawn` отменяются, ненаблюдённый отказ
/// задачи становится отказом программы (§18, D-034).
pub fn run_main(root: &Future) -> Outcome {
    root.observed.set(true);
    start(root, None);
    drive(root)?;
    let unobserved = shutdown()?;
    with(|s| s.registered.borrow_mut().clear());
    if root.state.get() == State::Failed {
        return Err(root.failure().expect("failed root has a failure"));
    }
    if let Some(failure) = unobserved {
        return Err(failure);
    }
    Ok(root.value().unwrap_or(Value::Undef))
}

/// Тест (§14): async-тело — как корень, затем завершение задач `spawn`
/// теста; синхронное — только завершение.
pub fn run_test(body: Option<&Future>) -> Result<(), Rc<Failure>> {
    with(|s| s.registered.borrow_mut().clear());
    if let Some(body) = body {
        return run_main(body).map(|_| ());
    }
    let unobserved = shutdown()?;
    with(|s| s.registered.borrow_mut().clear());
    match unobserved {
        Some(failure) => Err(failure),
        None => Ok(()),
    }
}

/// Внешняя операция: пока она присоединена, цикл `async main` её ждёт;
/// отсоединённая (её Future отменён) не держит программу, но её исход всё
/// равно доставляется.
#[derive(Clone, Debug)]
pub struct Op(Rc<Cell<bool>>);

impl Op {
    /// Снова ждать операцию (её исход нужен новому ожиданию).
    pub fn attach(&self) {
        if !self.0.replace(true) {
            with(|s| s.external.set(s.external.get() + 1));
        }
    }

    pub fn detach(&self) {
        if self.0.replace(false) {
            with(|s| s.external.set(s.external.get() - 1));
        }
    }
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
        s.external.set(s.external.get() + 1);
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
    });
    op
}

/// Ждать исход внешней операции в текущей задаче.
pub fn await_offload<R: Send + 'static>(
    work: impl FnOnce() -> R + Send + 'static,
    convert: impl FnOnce(R) -> Outcome + 'static,
) -> Outcome {
    let promise = promise();
    let target = promise.clone();
    let op = offload(work, move |result| settle(&target, convert(result)));
    promise.listen(move || op.detach());
    await_future(&promise)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(value: i64) -> Value {
        Value::Int(crate::IntValue::from_i64(value))
    }

    #[test]
    fn completion_resumes_waiters_in_order_and_caches_the_outcome() {
        reset();
        let log = Rc::new(RefCell::new(Vec::new()));
        let gate = promise();
        let make = |name: &'static str| {
            let (log, gate) = (log.clone(), gate.clone());
            cold(
                Box::new(move || {
                    log.borrow_mut().push(format!("{name} waits"));
                    let value = await_future(&gate)?;
                    log.borrow_mut().push(format!("{name} resumed"));
                    Ok(value)
                }),
                Vec::new(),
            )
        };
        let (first, second) = (make("a"), make("b"));
        spawn(&first);
        spawn(&second);
        log.borrow_mut().push("main".into());
        complete(&gate, Ok(int(7)));
        assert_eq!(
            *log.borrow(),
            ["a waits", "b waits", "main", "a resumed", "b resumed"]
        );
        assert!(matches!(first.outcome(), Ok(Value::Int(v)) if v == crate::IntValue::from_i64(7)));
        reset();
    }

    #[test]
    fn cancellation_reaches_the_innermost_awaited_future() {
        reset();
        let gate = promise();
        let inner_gate = gate.clone();
        let inner = cold(Box::new(move || await_future(&inner_gate)), Vec::new());
        let awaited = inner.clone();
        let outer = cold(Box::new(move || await_future(&awaited)), Vec::new());
        spawn(&outer);
        cancel(&outer);
        assert!(outer.cancelled());
        assert!(inner.cancelled());
        reset();
    }

    #[test]
    fn async_main_without_progress_is_a_program_error() {
        reset();
        let gate = promise();
        let root = cold(Box::new(move || await_future(&gate)), Vec::new());
        let failure = run_main(&root).unwrap_err();
        assert_eq!(
            failure.message(),
            "async main cannot complete: no task can make progress"
        );
        reset();
    }

    #[test]
    fn offloaded_work_is_delivered_by_the_event_loop() {
        reset();
        let root = cold(
            Box::new(|| await_offload(|| 40 + 2, |value| Ok(int(value)))),
            Vec::new(),
        );
        assert!(matches!(run_main(&root), Ok(Value::Int(v)) if v == crate::IntValue::from_i64(42)));
        reset();
    }
}
