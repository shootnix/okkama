//! Кооперативный планировщик (OIR-1 §13, §18 спецификации) — перенос
//! `okkama.rt.Scheduler` JVM. Исполняется ровно одна задача: `transfer` отдаёт
//! управление и возвращается, когда его вернут. Запуск Future исполняет тело
//! до первой приостановки сразу; `spawn` запускает задачу немедленно.
//! Завершение не продолжает ожидающих на месте: они встают в очередь готовых
//! задач по порядку, и её разбирает цикл запуска, когда текущая задача
//! приостановится или закончится (#340) — код задачи между двумя точками
//! приостановки не перемежается чужим. Внешние события (таймеры, ввод-вывод)
//! приходят в очередь цикла `async main`.
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
use std::collections::{BTreeMap, HashMap, VecDeque};
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
    /// Закрытие области ждёт его без отмены: `async destroy` (#154).
    shielded: Cell<bool>,
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
    /// Задача в очереди готовых: второе пробуждение её не дублирует.
    queued: Cell<bool>,
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
            queued: Cell::new(false),
        })
    }
}

/// Исход внешней операции хоста: вычислен в рабочем потоке, доставляется в
/// цикле событий потока программы.
type Delivery = Box<dyn std::any::Any + Send>;

/// Продолжение внешней операции: получает её исход.
type Continuation = Box<dyn FnOnce(Delivery)>;

struct Scheduler {
    current: RefCell<Rc<Task>>,
    /// Future, запущенные `spawn`: их исход проверяет завершение программы.
    registered: RefCell<BTreeMap<u64, Future>>,
    registered_ids: RefCell<HashMap<*const FutureCell, u64>>,
    next_registration: Cell<u64>,
    /// Внешние операции, исхода которых ждёт программа.
    external: Cell<usize>,
    /// Незавершённые задачи: их корутины освобождаются при завершении запуска.
    live: RefCell<HashMap<*const Task, Rc<Task>>>,
    /// Разбуженные задачи по порядку пробуждения: их продолжает цикл запуска.
    ready: RefCell<VecDeque<Rc<Task>>>,
    /// Свободные стеки корутин.
    stacks: RefCell<Vec<DefaultStack>>,
    sender: std::sync::mpsc::Sender<(u64, Delivery)>,
    receiver: std::sync::mpsc::Receiver<(u64, Delivery)>,
    /// Продолжения внешних операций по номеру.
    pending: RefCell<HashMap<u64, Continuation>>,
    next_op: Cell<u64>,
}

impl Scheduler {
    fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self {
            current: RefCell::new(Task::root()),
            registered: RefCell::new(BTreeMap::new()),
            registered_ids: RefCell::new(HashMap::new()),
            next_registration: Cell::new(0),
            external: Cell::new(0),
            live: RefCell::new(HashMap::new()),
            ready: RefCell::new(VecDeque::new()),
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
        s.registered_ids.borrow_mut().clear();
        s.external.set(0);
        s.pending.borrow_mut().clear();
        while s.receiver.try_recv().is_ok() {}
        for task in s.ready.borrow_mut().drain(..) {
            task.queued.set(false);
        }
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
            shielded: Cell::new(false),
        })
    }

    pub fn state(&self) -> State {
        self.state.get()
    }

    pub fn observe(&self) {
        self.observed.set(true);
        self.release_registration();
    }

    // Keep active tasks and unobserved failures, never completed successful results.
    fn needs_registration(&self) -> bool {
        !self.done() || (self.state.get() == State::Failed && !self.observed.get() && !self.cancelled())
    }

    fn release_registration(&self) {
        if self.needs_registration() {
            return;
        }
        let removed = with(|s| {
            let id = s.registered_ids.borrow_mut().remove(&(self as *const FutureCell));
            id.and_then(|id| s.registered.borrow_mut().remove(&id))
        });
        // Drop outside registry borrows: payload destruction may release other futures.
        drop(removed);
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
    if !future.needs_registration() {
        return;
    }
    with(|s| {
        let mut ids = s.registered_ids.borrow_mut();
        if let std::collections::hash_map::Entry::Vacant(entry) = ids.entry(Rc::as_ptr(future)) {
            let id = s.next_registration.get();
            s.next_registration.set(id.checked_add(1).expect("future registration counter"));
            entry.insert(id);
            s.registered.borrow_mut().insert(id, future.clone());
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

/// Разбудить задачу: она встаёт в очередь готовых и продолжится, когда цикл
/// запуска до неё дойдёт, а не внутри кода, который её разбудил (#340).
fn make_ready(task: &Rc<Task>) {
    if task.queued.replace(true) {
        return;
    }
    with(|s| s.ready.borrow_mut().push_back(task.clone()));
}

/// Продолжить первую готовую задачу; `false` — очередь пуста. Зовёт только
/// цикл запуска: так задача не продолжается внутри другой.
fn run_ready() -> bool {
    debug_assert!(current().yielder.get().is_null(), "only the launch loop dispatches");
    let Some(task) = with(|s| s.ready.borrow_mut().pop_front()) else {
        return false;
    };
    task.queued.set(false);
    // Задачу могли освободить (`reset`) после пробуждения.
    if task.coroutine.borrow().is_some() {
        transfer(&task);
    }
    true
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
        queued: Cell::new(false),
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

/// Исход Future; ожидающие задачи встают в очередь готовых в порядке
/// ожидания, слушатели вызываются сразу.
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
    future.release_registration();
    loop {
        let next = future.waiters.borrow_mut().pop_front();
        match next {
            None => break,
            Some(Waiter::Task(task)) => make_ready(&task),
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
    future.observe();
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

/// Ожидание `async destroy` при закрытии области (#154): отмена задачи его не
/// прерывает и остаётся отложенной до её следующего `await`.
pub fn await_shielded(future: &Future) -> Outcome {
    future.shielded.set(true);
    future.observe();
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
        future.release_registration();
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
    if Rc::ptr_eq(&task, &current()) || inner.shielded.get() {
        return;
    }
    let started_here = inner
        .starter
        .borrow()
        .as_ref()
        .is_some_and(|starter| Rc::ptr_eq(starter, &task));
    let shared = inner
        .waiters
        .borrow()
        .iter()
        .any(|waiter| matches!(waiter, Waiter::Task(other) if !Rc::ptr_eq(other, &task)));
    if inner.state.get() == State::Running && (started_here || !shared) {
        // Structured await (#147): the awaited future — one this task
        // started, a spawned task or a native request no other task awaits —
        // is cancelled with its awaiter, which keeps waiting and raises
        // Cancelled once it has settled.
        cancel(&inner);
        return;
    }
    inner
        .waiters
        .borrow_mut()
        .retain(|waiter| !matches!(waiter, Waiter::Task(other) if Rc::ptr_eq(other, &task)));
    // Отменённая задача продолжится и бросит Cancelled в свою очередь, не
    // внутри `cancel` (#340).
    make_ready(&task);
}

/// Ошибка программы цикла событий (`Rt.trap`).
fn trap(message: &str) -> Rc<Failure> {
    Failure::new(Kind::Program(message.into()))
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
    while let Ok((id, delivery)) = with(|s| s.receiver.try_recv()) {
        let done = with(|s| s.pending.borrow_mut().remove(&id));
        if let Some(done) = done {
            done(delivery);
        }
    }
}

/// Продолжать готовые задачи и обрабатывать внешние события, пока `until`
/// не завершён. Готовая задача — тоже ход вперёд, без внешних событий.
fn drive(until: &Future) -> Result<(), Rc<Failure>> {
    while until.state.get() == State::Running {
        if run_ready() {
            continue;
        }
        if with(|s| s.external.get()) == 0 {
            return Err(trap("async main cannot complete: no task can make progress"));
        }
        deliver()?;
    }
    Ok(())
}

/// Дождаться одной внешней доставки и выполнить её продолжение.
fn deliver() -> Result<(), Rc<Failure>> {
    let (id, delivery) = with(|s| s.receiver.recv())
        .map_err(|_| trap("async runtime was interrupted"))?;
    let done = with(|s| s.pending.borrow_mut().remove(&id));
    if let Some(done) = done {
        done(delivery);
    }
    Ok(())
}

fn shutdown() -> Result<Option<Rc<Failure>>, Rc<Failure>> {
    let registered: Vec<_> = with(|s| s.registered.borrow().values().cloned().collect());
    for future in &registered {
        if future.state.get() < State::Done {
            cancel(future);
        }
    }
    let registered: Vec<_> = with(|s| s.registered.borrow().values().cloned().collect());
    for future in &registered {
        while future.state.get() == State::Running {
            if run_ready() {
                continue;
            }
            if with(|s| s.external.get()) == 0 {
                break;
            }
            deliver()?;
        }
    }
    let registered: Vec<_> = with(|s| s.registered.borrow().values().cloned().collect());
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
    root.observe();
    start(root, None);
    drive(root)?;
    let unobserved = shutdown()?;
    with(|s| {
        s.registered.borrow_mut().clear();
        s.registered_ids.borrow_mut().clear();
    });
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
    with(|s| {
        s.registered.borrow_mut().clear();
        s.registered_ids.borrow_mut().clear();
    });
    if let Some(body) = body {
        return run_main(body).map(|_| ());
    }
    let unobserved = shutdown()?;
    with(|s| {
        s.registered.borrow_mut().clear();
        s.registered_ids.borrow_mut().clear();
    });
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
        s.external.set(s.external.get() + 1);
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
        wake();
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

    /// The launch loop's turn: every task that is ready runs.
    fn dispatch() {
        while run_ready() {}
    }

    /// Completion makes the waiters ready in order; they run on the launch
    /// loop's turn, not inside `complete` (#340).
    #[test]
    fn completion_queues_waiters_in_order_and_caches_the_outcome() {
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
        log.borrow_mut().push("main after complete".into());
        assert_eq!(*log.borrow(), ["a waits", "b waits", "main", "main after complete"]);
        dispatch();
        assert_eq!(
            *log.borrow(),
            ["a waits", "b waits", "main", "main after complete", "a resumed", "b resumed"]
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
        assert_eq!(outer.state(), State::Running, "cancel does not run the task");
        dispatch();
        assert!(outer.cancelled());
        assert!(inner.cancelled());
        reset();
    }

    /// Structured await (#147): a cancelled awaiter resumes only after the
    /// awaited future has settled, even when its cleanup suspends.
    #[test]
    fn cancellation_waits_for_the_awaited_future_to_settle() {
        reset();
        let log = Rc::new(RefCell::new(Vec::new()));
        let (gate, cleanup) = (promise(), promise());
        let (inner_gate, inner_cleanup, inner_log) = (gate.clone(), cleanup.clone(), log.clone());
        let inner = cold(
            Box::new(move || {
                let outcome = await_future(&inner_gate);
                inner_log.borrow_mut().push("inner cancelled");
                await_future(&inner_cleanup)?;
                inner_log.borrow_mut().push("inner settled");
                outcome
            }),
            Vec::new(),
        );
        let (awaited, outer_log) = (inner.clone(), log.clone());
        let outer = cold(
            Box::new(move || {
                let outcome = await_future(&awaited);
                outer_log.borrow_mut().push("outer resumed");
                outcome
            }),
            Vec::new(),
        );
        spawn(&outer);
        cancel(&outer);
        assert!(log.borrow().is_empty(), "cleanup does not run inside cancel");
        dispatch();
        assert_eq!(*log.borrow(), ["inner cancelled"]);
        assert_eq!(outer.state(), State::Running);
        complete(&cleanup, Ok(int(0)));
        dispatch();
        assert_eq!(*log.borrow(), ["inner cancelled", "inner settled", "outer resumed"]);
        assert!(inner.cancelled());
        assert!(outer.cancelled());
        reset();
    }

    /// A spawned future that only the cancelled task awaits is cancelled
    /// with it (#147); one that another task also awaits is left running.
    #[test]
    fn cancellation_reaches_a_spawned_future_unless_another_task_awaits_it() {
        reset();
        let gate = promise();
        let child_gate = gate.clone();
        let child = cold(Box::new(move || await_future(&child_gate)), Vec::new());
        spawn(&child);
        let awaited = child.clone();
        let outer = cold(Box::new(move || await_future(&awaited)), Vec::new());
        spawn(&outer);
        cancel(&outer);
        dispatch();
        assert!(outer.cancelled());
        assert!(child.cancelled());
        reset();

        let gate = promise();
        let child_gate = gate.clone();
        let child = cold(Box::new(move || await_future(&child_gate)), Vec::new());
        spawn(&child);
        let (first, second) = (child.clone(), child.clone());
        let outer = cold(Box::new(move || await_future(&first)), Vec::new());
        let watcher = cold(Box::new(move || await_future(&second)), Vec::new());
        spawn(&outer);
        spawn(&watcher);
        cancel(&outer);
        dispatch();
        assert!(outer.cancelled());
        assert_eq!(child.state(), State::Running);
        complete(&gate, Ok(int(7)));
        dispatch();
        assert!(matches!(watcher.outcome(), Ok(Value::Int(v)) if v.to_i64() == Some(7)));
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
    #[test]
    fn existing_worker_delivers_on_program_thread() {
        reset();
        let program_thread = std::thread::current().id();
        let root = cold(
            Box::new(move || {
                let p = promise();
                let target = p.clone();
                let (_op, completion) = external(move |v: Option<i64>| {
                    assert_eq!(std::thread::current().id(), program_thread);
                    settle(&target, Ok(int(v.unwrap())));
                });
                std::thread::spawn(move || completion.deliver(42));
                await_future(&p)
            }),
            Vec::new(),
        );
        assert!(matches!(run_main(&root),Ok(Value::Int(v)) if v.to_i64()==Some(42)));
        reset();
    }

    #[test]
    fn dropped_worker_token_delivers_failure_signal() {
        reset();
        let root = cold(
            Box::new(|| {
                let p = promise();
                let target = p.clone();
                let (_op, completion) = external(move |v: Option<i64>| {
                    assert!(v.is_none());
                    settle(&target, Ok(int(7)));
                });
                drop(completion);
                await_future(&p)
            }),
            Vec::new(),
        );
        assert!(matches!(run_main(&root),Ok(Value::Int(v)) if v.to_i64()==Some(7)));
        reset();
    }

    #[test]
    fn detached_delivery_still_cleans_up_and_cannot_replace_cancellation() {
        reset();
        let root = cold(
            Box::new(|| {
                let p = promise();
                let target = p.clone();
                let finished = promise();
                let done = finished.clone();
                let (op, completion) = external(move |v: Option<i64>| {
                    assert_eq!(v, Some(42));
                    settle(&target, Ok(int(42)));
                    assert!(target.cancelled());
                    settle(&done, Ok(int(1)));
                });
                cancel(&p);
                op.detach();
                op.attach();
                op.detach();
                // A second attached delivery keeps the loop alive while it drains
                // the detached late result, just as connection.settled does.
                let (_guard, tick) = external(|_: Option<()>| {});
                completion.deliver(42);
                tick.deliver(());
                await_future(&finished)
            }),
            Vec::new(),
        );
        assert!(matches!(run_main(&root),Ok(Value::Int(v)) if v.to_i64()==Some(1)));
        reset();
    }
    #[test]
    fn registration_preserves_identity_order_and_resets_between_runs() {
        reset();
        let a=promise();let b=promise();
        register(&a);register(&b);register(&a);
        with(|s| { let all:Vec<_>=s.registered.borrow().values().cloned().collect();assert_eq!(all.len(),2);assert!(Rc::ptr_eq(&all[0],&a));assert!(Rc::ptr_eq(&all[1],&b)); });
        reset();
        register(&b);register(&a);
        with(|s| { let all:Vec<_>=s.registered.borrow().values().cloned().collect();assert_eq!(all.len(),2);assert!(Rc::ptr_eq(&all[0],&b)); });
        reset();register(&a);
        with(|s| assert_eq!(s.registered.borrow().len(),1));
        reset();
    }

    #[test]
    fn completed_registrations_release_futures_without_clearing_cached_outcomes() {
        reset();
        for _ in 0..20_000 {
            let future = promise();
            let weak = Rc::downgrade(&future);
            register(&future);
            complete(&future, Ok(int(7)));
            with(|s| {
                assert!(s.registered.borrow().is_empty());
                assert!(s.registered_ids.borrow().is_empty());
            });
            assert!(matches!(await_future(&future), Ok(Value::Int(v)) if v.to_i64() == Some(7)));
            register(&future); // Re-registering a completed success must not retain it.
            drop(future);
            assert!(weak.upgrade().is_none());
        }
        reset();
    }

    #[test]
    fn unobserved_failures_keep_registration_order_and_release_after_observation() {
        reset();
        let first = promise();
        let second = promise();
        register(&first);
        register(&second);
        let a = trap("first registered");
        let b = trap("second registered");
        complete(&second, Err(b.clone()));
        complete(&first, Err(a.clone()));
        assert!(Rc::ptr_eq(&shutdown().unwrap().unwrap(), &a));
        first.observe();
        assert!(Rc::ptr_eq(&shutdown().unwrap().unwrap(), &b));
        assert!(Rc::ptr_eq(&await_future(&second).unwrap_err(), &b));
        with(|s| assert!(s.registered.borrow().is_empty()));
        assert!(shutdown().unwrap().is_none());
        reset();
    }

    #[test]
    fn observed_active_tasks_stay_registered_until_completion_and_cancel_releases() {
        reset();
        let active = promise();
        active.observe();
        register(&active);
        with(|s| assert_eq!(s.registered.borrow().len(), 1));
        assert!(shutdown().unwrap().is_none());
        assert!(active.cancelled());
        with(|s| assert!(s.registered.borrow().is_empty()));
        let cold = cold(Box::new(|| Ok(int(1))), Vec::new());
        register(&cold);
        cancel(&cold);
        register(&cold);
        with(|s| assert!(s.registered.borrow().is_empty()));
        reset();
    }

    /// Tasks that wake each other through promises finish without any
    /// external operation: a ready task is progress (#340).
    #[test]
    fn ready_tasks_finish_async_main_without_external_operations() {
        reset();
        let (ping, pong) = (promise(), promise());
        let (child_ping, child_pong) = (ping.clone(), pong.clone());
        let child = cold(
            Box::new(move || {
                let value = await_future(&child_ping)?;
                complete(&child_pong, Ok(value));
                Ok(int(0))
            }),
            Vec::new(),
        );
        let root = cold(
            Box::new(move || {
                spawn(&child);
                complete(&ping, Ok(int(5)));
                await_future(&pong)
            }),
            Vec::new(),
        );
        assert!(matches!(run_main(&root), Ok(Value::Int(v)) if v.to_i64() == Some(5)));
        reset();
    }

    /// A task woken twice before it runs (completion, then cancellation)
    /// runs once; a reset drops the queue.
    #[test]
    fn a_task_woken_twice_runs_once_and_reset_drops_the_queue() {
        reset();
        let runs = Rc::new(Cell::new(0));
        let gate = promise();
        let (task_gate, task_runs) = (gate.clone(), runs.clone());
        let task = cold(
            Box::new(move || {
                let outcome = await_future(&task_gate);
                task_runs.set(task_runs.get() + 1);
                outcome
            }),
            Vec::new(),
        );
        spawn(&task);
        complete(&gate, Ok(int(1)));
        cancel(&task);
        with(|s| assert_eq!(s.ready.borrow().len(), 1));
        dispatch();
        assert_eq!(runs.get(), 1);
        assert!(task.cancelled());
        reset();

        let gate = promise();
        let task_gate = gate.clone();
        let task = cold(Box::new(move || await_future(&task_gate)), Vec::new());
        spawn(&task);
        complete(&gate, Ok(int(1)));
        reset();
        with(|s| assert!(s.ready.borrow().is_empty()));
        dispatch();
        assert_eq!(task.state(), State::Running);
        reset();
    }

    /// When async main returns, a task it woke has not run; shutdown cancels
    /// it and its cleanup runs, as on Sparrow.
    #[test]
    fn a_task_woken_by_a_finished_root_is_cancelled_by_shutdown() {
        reset();
        let log = Rc::new(RefCell::new(Vec::new()));
        let gate = promise();
        let (child_gate, child_log) = (gate.clone(), log.clone());
        let child = cold(
            Box::new(move || {
                let outcome = await_future(&child_gate);
                child_log.borrow_mut().push(if outcome.is_err() { "child cancelled" } else { "child woke" });
                outcome
            }),
            Vec::new(),
        );
        let root_log = log.clone();
        let root = cold(
            Box::new(move || {
                spawn(&child);
                complete(&gate, Ok(int(1)));
                root_log.borrow_mut().push("root returns");
                Ok(int(0))
            }),
            Vec::new(),
        );
        assert!(run_main(&root).is_ok());
        assert_eq!(*log.borrow(), ["root returns", "child cancelled"]);
        reset();
    }

    #[test]
    fn completion_listener_can_register_and_complete_another_future() {
        reset();
        let first = promise();
        register(&first);
        first.listen(|| {
            let second = promise();
            register(&second);
            complete(&second, Ok(int(2)));
        });
        complete(&first, Ok(int(1)));
        with(|s| assert!(s.registered.borrow().is_empty()));
        reset();
    }

}
