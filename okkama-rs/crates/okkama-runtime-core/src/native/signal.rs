//! `runtime/signal`: ожидание сигналов процесса (TERM, INT, HUP), #80.
//! Обработчик пишет номер сигнала в self-pipe (единственное, что можно делать
//! в обработчике); поток-диспетчер читает трубу и считает приходы каждого
//! сигнала, ожидающие ждут роста своих счётчиков. Обработчик стоит, пока
//! сигнал кто-то ждёт: когда ожидание кончилось (сигнал пришёл или ожидание
//! отменено), сигналу возвращается прежнее действие, и следующий завершает
//! процесс как обычно. Sparrow берёт этот native через мост (`native_origin`).
#![allow(unsafe_code)]

use std::rc::Rc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock, PoisonError};

use super::Error;
use super::support::{items, text};
use crate::failure::{Failure, Kind};
use crate::sched;
use crate::{Value, abi::Native};

/// Сигналы, которые можно ждать, и их номера.
const SIGNALS: [(&str, libc::c_int); 3] = [("TERM", libc::SIGTERM), ("INT", libc::SIGINT), ("HUP", libc::SIGHUP)];

/// Пишущий конец трубы (`-1`, пока диспетчер не запущен).
static WRITE_END: AtomicI32 = AtomicI32::new(-1);

struct Registry {
    /// Сколько раз пришёл каждый сигнал (по месту в `SIGNALS`).
    arrived: [u64; 3],
    /// Прежние действия сигналов, которым поставлен обработчик.
    prior: [Option<libc::sigaction>; 3],
    /// Идущие ожидания: номер и места ожидаемых сигналов.
    waiters: Vec<(u64, Vec<usize>)>,
    next: u64,
    started: bool,
}

struct Hub {
    registry: Mutex<Registry>,
    changed: Condvar,
}

fn hub() -> &'static Hub {
    static HUB: OnceLock<Hub> = OnceLock::new();
    HUB.get_or_init(|| Hub {
        registry: Mutex::new(Registry {
            arrived: [0; 3],
            prior: [None; 3],
            waiters: Vec::new(),
            next: 0,
            started: false,
        }),
        changed: Condvar::new(),
    })
}

fn registry() -> MutexGuard<'static, Registry> {
    hub().registry.lock().unwrap_or_else(PoisonError::into_inner)
}

fn fail(native: Native, code: &str, message: &str) -> Value {
    Value::Future(sched::resolved(Err(Failure::new(Kind::Native(Rc::new(crate::abi::NativeFailure {
        domain: "core.signal".into(),
        code: code.into(),
        operation: native.symbol().into(),
        message: message.into(),
    }))))))
}

extern "C" fn on_signal(number: libc::c_int) {
    let fd = WRITE_END.load(Ordering::SeqCst);
    if fd >= 0 {
        let byte = number as u8;
        // SAFETY: write(2) безопасен в обработчике сигнала; буфер — один байт,
        // запись неблокирующая (полная труба теряет лишний байт, не процесс).
        unsafe {
            let saved = *errno();
            libc::write(fd, &byte as *const u8 as *const libc::c_void, 1);
            *errno() = saved;
        }
    }
}

/// Место errno текущего потока: обработчик не должен его менять.
#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn errno() -> *mut libc::c_int {
    // SAFETY: errno текущего потока.
    unsafe { libc::__errno_location() }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
unsafe fn errno() -> *mut libc::c_int {
    // SAFETY: errno текущего потока.
    unsafe { libc::__error() }
}

/// Труба и поток-диспетчер (один раз на процесс).
fn start(registry: &mut Registry) -> bool {
    if registry.started {
        return true;
    }
    let mut ends = [0 as libc::c_int; 2];
    // SAFETY: pipe(2) заполняет два дескриптора.
    if unsafe { libc::pipe(ends.as_mut_ptr()) } != 0 {
        return false;
    }
    // SAFETY: свои дескрипторы: не наследуются потомками; запись не блокирует
    // обработчик сигнала.
    unsafe {
        libc::fcntl(ends[0], libc::F_SETFD, libc::FD_CLOEXEC);
        libc::fcntl(ends[1], libc::F_SETFD, libc::FD_CLOEXEC);
        let flags = libc::fcntl(ends[1], libc::F_GETFL);
        libc::fcntl(ends[1], libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    let read_end = ends[0];
    let spawned = std::thread::Builder::new()
        .name("okkama-signal".into())
        .spawn(move || dispatch(read_end));
    if spawned.is_err() {
        // SAFETY: свои дескрипторы, ещё никому не отданы.
        unsafe {
            libc::close(ends[0]);
            libc::close(ends[1]);
        }
        return false;
    }
    WRITE_END.store(ends[1], Ordering::SeqCst);
    registry.started = true;
    true
}

/// Поток-диспетчер: приход сигнала увеличивает его счётчик и будит ожидающих.
fn dispatch(read_end: libc::c_int) {
    loop {
        let mut byte = 0u8;
        // SAFETY: свой конец трубы и буфер в один байт.
        let count = unsafe { libc::read(read_end, &mut byte as *mut u8 as *mut libc::c_void, 1) };
        if count == 1 {
            if let Some(place) = SIGNALS.iter().position(|(_, number)| *number == libc::c_int::from(byte)) {
                registry().arrived[place] += 1;
                hub().changed.notify_all();
            }
        } else if count == 0 || std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return;
        }
    }
}

/// Записать ожидание сигналов `wanted`, поставив обработчики тем, у кого их
/// ещё нет. Возвращает номер ожидания и счётчики приходов на этот момент.
fn enter(wanted: &[usize]) -> Option<(u64, [u64; 3])> {
    let mut registry = registry();
    if !start(&mut registry) {
        return None;
    }
    for &place in wanted {
        if registry.prior[place].is_some() {
            continue;
        }
        // SAFETY: обработчик — extern "C" функция без состояния; прежнее
        // действие сохраняется, чтобы вернуть его.
        let prior = unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = on_signal as *const () as usize;
            action.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&mut action.sa_mask);
            let mut prior: libc::sigaction = std::mem::zeroed();
            (libc::sigaction(SIGNALS[place].1, &action, &mut prior) == 0).then_some(prior)
        };
        let Some(prior) = prior else {
            release(&mut registry);
            return None;
        };
        registry.prior[place] = Some(prior);
    }
    let id = registry.next;
    registry.next += 1;
    registry.waiters.push((id, wanted.to_vec()));
    Some((id, registry.arrived))
}

/// Снять ожидание `id` (повторный вызов ничего не делает).
fn leave(registry: &mut Registry, id: u64) {
    registry.waiters.retain(|(waiter, _)| *waiter != id);
    release(registry);
}

/// Вернуть прежнее действие сигналам, которых больше никто не ждёт.
fn release(registry: &mut Registry) {
    for (place, (_, number)) in SIGNALS.iter().enumerate() {
        if registry.waiters.iter().any(|(_, wanted)| wanted.contains(&place)) {
            continue;
        }
        if let Some(prior) = registry.prior[place].take() {
            // SAFETY: действие, сохранённое sigaction(2) при установке.
            unsafe {
                libc::sigaction(*number, &prior, std::ptr::null_mut());
            }
        }
    }
}

/// Рабочий поток: ждать прихода одного из сигналов `wanted` после `seen`;
/// `None` — ожидание снято (отменено).
fn wait(id: u64, wanted: &[usize], seen: [u64; 3]) -> Option<usize> {
    let mut registry = registry();
    loop {
        if !registry.waiters.iter().any(|(waiter, _)| *waiter == id) {
            return None;
        }
        if let Some(&place) = wanted.iter().find(|&&place| registry.arrived[place] > seen[place]) {
            leave(&mut registry, id);
            return Some(place);
        }
        registry = hub().changed.wait(registry).unwrap_or_else(PoisonError::into_inner);
    }
}

pub fn handles(native: Native) -> bool {
    native.module() == "runtime/signal"
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    if native != Native::SignalSignalWait {
        return Err(Error::InvalidState("not a signal native".into()));
    }
    let mut wanted = Vec::new();
    for item in items(&args[0])?.iter() {
        let name = item.as_string()?;
        let Some(place) = SIGNALS.iter().position(|(known, _)| *known == name) else {
            return Ok(fail(native, "invalid_input", "Unknown signal name"));
        };
        if !wanted.contains(&place) {
            wanted.push(place);
        }
    }
    if wanted.is_empty() {
        return Ok(fail(native, "invalid_input", "No signal to wait for"));
    }
    let Some((id, seen)) = enter(&wanted) else {
        return Ok(fail(native, "unsupported", "Signals cannot be handled here"));
    };
    let promise = sched::promise();
    let target = promise.clone();
    let op = sched::offload(
        move || wait(id, &wanted, seen),
        move |received: Option<usize>| {
            if let Some(place) = received {
                sched::settle(&target, Ok(text(SIGNALS[place].0)));
            }
        },
    );
    // Исход или отмена: ожидание снимается сразу, рабочий поток выходит.
    promise.listen(move || {
        op.detach();
        leave(&mut registry(), id);
        hub().changed.notify_all();
    });
    Ok(Value::Future(promise))
}
