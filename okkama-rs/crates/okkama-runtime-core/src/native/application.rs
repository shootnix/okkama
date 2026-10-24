//! `runtime/application` (POSIX-терминал) и `runtime/account` — перенос
//! `TerminalNatives` и `AccountNatives` JVM: дескрипторы stdin/stdout/stderr
//! или `/dev/tty`, режимы termios (стек аренд), геометрия `TIOCGWINSZ` и
//! `SIGWINCH`; базы пользователей и групп хоста.
#![allow(unsafe_code)]

use super::support::{array, byte_array, bytes, handle, int, int_map, int_value, items, map, new_handle, string, string_array, text};
use super::Error;
use crate::failure::{Failure, Kind};
use crate::sched;
use crate::{Value, abi::Native};
use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::{CStr, CString, c_char};
use std::rc::Rc;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

const DOMAIN: &str = "core.application";

fn symbol(name: &str, arity: usize) -> String {
    format!("runtime/application@1::{name}/{arity}")
}

fn fail(domain: &str, code: &str, operation: &str, message: &str) -> Rc<Failure> {
    Failure::new(Kind::Native(Rc::new(crate::abi::NativeFailure {
        domain: domain.into(),
        code: code.into(),
        operation: operation.into(),
        message: message.into(),
    })))
}

fn terminal_failure(code: &str, operation: &str, message: &str) -> Rc<Failure> {
    fail(DOMAIN, code, operation, message)
}

fn as_error(failure: Rc<Failure>) -> Error {
    match &failure.kind {
        Kind::Native(native) => Error::Failure((**native).clone()),
        _ => Error::Trap(failure.message().to_string()),
    }
}

fn resolved(outcome: sched::Outcome) -> Value {
    Value::Future(sched::resolved(outcome))
}

/// Переносимый код errno (`Posix.code` JVM).
fn errno_code(errno: i32) -> &'static str {
    match errno {
        2 => "not_found",
        1 | 13 | 30 => "permission_denied",
        4 => "interrupted",
        9 | 20 | 21 | 22 => "invalid_input",
        32 => "broken_pipe",
        12 | 23 | 24 | 28 => "resource_exhausted",
        17 => "already_exists",
        _ if libc::EAGAIN == errno || libc::EWOULDBLOCK == errno => "would_block",
        _ if cfg!(target_os = "macos") && (errno == 36 || errno == 37) => "would_block",
        _ => "unexpected",
    }
}

fn last_errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

fn errno_failure(errno: i32, operation: &str, message: &str) -> Rc<Failure> {
    terminal_failure(errno_code(errno), operation, message)
}

// ---- сессии ----

static OWNERS: Mutex<Option<HashSet<String>>> = Mutex::new(None);

struct Mode {
    id: u32,
    before: Vec<u8>,
    released: bool,
}

struct Session {
    input: i32,
    output: i32,
    controlling: bool,
    input_interactive: bool,
    output_interactive: bool,
    original: Option<Vec<u8>>,
    closed: bool,
    modes: Vec<Mode>,
    serial: u32,
    cleanup: Vec<u8>,
    claims: Vec<String>,
    resizes: Vec<Rc<RefCell<Resizes>>>,
    /// Однопоточный исполнитель чтения JVM.
    reader: Arc<Mutex<()>>,
    reading: Option<sched::Future>,
    pending: Option<Vec<u8>>,
}

type SessionRef = Rc<RefCell<Session>>;

struct Lease {
    session: SessionRef,
    id: u32,
    dropped: RefCell<bool>,
}

struct Resizes {
    session: Option<SessionRef>,
    closed: bool,
    notified: bool,
    seen: u64,
    pending: Option<sched::Future>,
}

fn dup(fd: i32, operation: &str) -> Result<i32, Rc<Failure>> {
    // SAFETY: dup(2) действительного дескриптора.
    let copy = unsafe { libc::dup(fd) };
    if copy < 0 {
        return Err(errno_failure(last_errno(), operation, "Cannot duplicate terminal descriptor"));
    }
    // Как FFM JVM: дескриптор сессии не наследуется потомками.
    unsafe { libc::fcntl(copy, libc::F_SETFD, libc::FD_CLOEXEC) };
    Ok(copy)
}

fn tty(fd: i32) -> bool {
    // SAFETY: isatty(3).
    unsafe { libc::isatty(fd) == 1 }
}

fn termios_size() -> usize {
    std::mem::size_of::<libc::termios>()
}

fn snapshot(session: &Session, operation: &str) -> Result<Vec<u8>, Rc<Failure>> {
    let mut term = vec![0u8; termios_size()];
    // SAFETY: буфер размера struct termios.
    if unsafe { libc::tcgetattr(session.input, term.as_mut_ptr() as *mut libc::termios) } < 0 {
        return Err(errno_failure(last_errno(), operation, "Cannot observe terminal mode"));
    }
    Ok(term)
}

fn apply(session: &Session, snapshot: &[u8], operation: &str) -> Result<(), Rc<Failure>> {
    // SAFETY: снимок struct termios.
    if unsafe { libc::tcsetattr(session.input, libc::TCSANOW, snapshot.as_ptr() as *const libc::termios) } < 0 {
        return Err(errno_failure(last_errno(), operation, "Cannot apply terminal mode"));
    }
    Ok(())
}

fn close_fds(session: &Session) {
    for fd in [session.input, session.output] {
        if fd >= 0 {
            // SAFETY: собственный дескриптор сессии.
            unsafe { libc::close(fd) };
        }
    }
}

fn opened(mut session: Session, keys: &[String], operation: &str) -> Result<SessionRef, Rc<Failure>> {
    {
        let mut owners = OWNERS.lock().expect("owners lock");
        let owners = owners.get_or_insert_with(HashSet::new);
        if keys.iter().any(|key| owners.contains(key)) {
            close_fds(&session);
            return Err(terminal_failure(
                "resource_exhausted",
                operation,
                "Terminal descriptor already belongs to an open session",
            ));
        }
        for key in keys {
            owners.insert(key.clone());
            session.claims.push(key.clone());
        }
    }
    session.input_interactive = tty(session.input);
    session.output_interactive = tty(session.output);
    if session.input_interactive {
        session.original = Some(snapshot(&session, operation)?);
    }
    Ok(Rc::new(RefCell::new(session)))
}

fn new_session(input: i32, output: i32, controlling: bool) -> Session {
    Session {
        input,
        output,
        controlling,
        input_interactive: false,
        output_interactive: false,
        original: None,
        closed: false,
        modes: Vec::new(),
        serial: 0,
        cleanup: Vec::new(),
        claims: Vec::new(),
        resizes: Vec::new(),
        reader: Arc::new(Mutex::new(())),
        reading: None,
        pending: None,
    }
}

fn session(value: &Value, operation: &str) -> Result<SessionRef, Rc<Failure>> {
    let state = handle::<SessionRef>(value)
        .map_err(|_| terminal_failure("invalid_input", operation, "Invalid terminal session"))?
        .clone();
    if state.borrow().closed {
        return Err(terminal_failure("already_closed", operation, "Terminal session is closed"));
    }
    Ok(state)
}

/// [столбцы, строки] вывода; не терминал — нули.
fn size(session: &Session, operation: &str) -> Result<(i64, i64), Rc<Failure>> {
    if !session.output_interactive {
        return Ok((0, 0));
    }
    let mut winsize = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: TIOCGWINSZ пишет struct winsize.
    if unsafe { libc::ioctl(session.output, libc::TIOCGWINSZ, &mut winsize) } < 0 {
        return Err(errno_failure(last_errno(), operation, "Cannot observe terminal geometry"));
    }
    Ok((i64::from(winsize.ws_col), i64::from(winsize.ws_row)))
}

fn write_fd(fd: i32, data: &[u8], operation: &str) -> Result<usize, Rc<Failure>> {
    loop {
        // SAFETY: буфер `data`.
        let count = unsafe { libc::write(fd, data.as_ptr() as *const libc::c_void, data.len()) };
        if count >= 0 {
            return Ok(count as usize);
        }
        let code = last_errno();
        if code != libc::EINTR {
            return Err(errno_failure(code, operation, "Cannot write terminal output"));
        }
    }
}

fn close(state: &SessionRef) -> Result<(), Rc<Failure>> {
    if state.borrow().closed {
        return Ok(());
    }
    state.borrow_mut().closed = true;
    let resizes = state.borrow().resizes.clone();
    for source in resizes {
        close_resizes(&source);
    }
    let operation = symbol("terminal_close", 1);
    let reading = state.borrow().reading.clone();
    if let Some(reading) = reading {
        sched::settle(&reading, Err(terminal_failure("already_closed", &operation, "Terminal session is closed")));
    }
    let mut first = None;
    let mut session = state.borrow_mut();
    if !session.cleanup.is_empty() && session.output >= 0 {
        match write_fd(session.output, &session.cleanup, &operation) {
            Ok(count) if count != session.cleanup.len() => {
                first = Some(terminal_failure("unexpected", &operation, "Terminal screen restoration was incomplete"));
            }
            Ok(_) => {}
            Err(failure) => first = Some(failure),
        }
    }
    if let Some(original) = session.original.clone() {
        if let Err(failure) = apply(&session, &original, &operation) {
            first = first.or(Some(failure));
        }
    }
    session.modes.clear();
    close_fds(&session);
    if let Ok(mut owners) = OWNERS.lock() {
        if let Some(owners) = owners.as_mut() {
            for claim in &session.claims {
                owners.remove(claim);
            }
        }
    }
    session.claims.clear();
    match first {
        Some(failure) => Err(failure),
        None => Ok(()),
    }
}

fn terminal_read(value: &Value, limit: &crate::IntValue) -> Value {
    let operation = symbol("terminal_read", 2);
    let state = match session(value, &operation) {
        Ok(state) => state,
        Err(failure) => return resolved(Err(failure)),
    };
    let Some(limit) = limit.to_i64().filter(|limit| (1..=65536).contains(limit)) else {
        return resolved(Err(terminal_failure("invalid_input", &operation, "Terminal read limit must be 1..65536")));
    };
    let limit = limit as usize;
    if state.borrow().reading.is_some() {
        return resolved(Err(terminal_failure("resource_exhausted", &operation, "Terminal already has a pending read")));
    }
    if let Some(mut pending) = state.borrow_mut().pending.take() {
        let count = limit.min(pending.len());
        let rest = pending.split_off(count);
        if !rest.is_empty() {
            state.borrow_mut().pending = Some(rest);
        }
        return resolved(Ok(byte_array(pending)));
    }
    let promise = sched::promise();
    state.borrow_mut().reading = Some(promise.clone());
    let (fd, gate) = {
        let session = state.borrow();
        (session.input, session.reader.clone())
    };
    let target = promise.clone();
    let delivered = state.clone();
    let op = sched::offload(
        move || {
            let _gate = gate.lock().expect("terminal reader lock");
            let mut buffer = vec![0u8; limit];
            loop {
                // SAFETY: буфер длины `limit`.
                let count = unsafe { libc::read(fd, buffer.as_mut_ptr() as *mut libc::c_void, limit) };
                if count >= 0 {
                    buffer.truncate(count as usize);
                    return Ok(buffer);
                }
                let code = last_errno();
                if code != libc::EINTR {
                    return Err(code);
                }
            }
        },
        move |outcome: Result<Vec<u8>, i32>| {
            if target.state() != sched::State::Running {
                if let Ok(data) = outcome {
                    if !data.is_empty() {
                        delivered.borrow_mut().pending = Some(data);
                    }
                }
                return;
            }
            let result = match outcome {
                Ok(data) => Ok(byte_array(data)),
                Err(_) if delivered.borrow().closed => {
                    Err(terminal_failure("already_closed", &operation, "Terminal session is closed"))
                }
                Err(code) => Err(errno_failure(code, &operation, "Cannot read terminal input")),
            };
            sched::settle(&target, result);
        },
    );
    let weak = Rc::downgrade(&state);
    promise.listen(move || {
        op.detach();
        if let Some(state) = weak.upgrade() {
            state.borrow_mut().reading = None;
        }
    });
    Value::Future(promise)
}

// ---- режимы ----

#[cfg(target_os = "macos")]
mod flags {
    pub const FLAG: usize = 8;
    pub const CC: usize = 32;
    pub const VMIN: usize = 16;
    pub const VTIME: usize = 17;
    pub const ECHONL: u64 = 0x10;
    pub const ICANON: u64 = 0x100;
    pub const ISIG: u64 = 0x80;
    pub const IEXTEN: u64 = 0x400;
    pub const IXON: u64 = 0x200;
    pub const CSIZE: u64 = 0x300;
    pub const CS8: u64 = 0x300;
    pub const PARENB: u64 = 0x1000;
}

#[cfg(not(target_os = "macos"))]
mod flags {
    pub const FLAG: usize = 4;
    pub const CC: usize = 17;
    pub const VMIN: usize = 6;
    pub const VTIME: usize = 5;
    pub const ECHONL: u64 = 0x40;
    pub const ICANON: u64 = 0x2;
    pub const ISIG: u64 = 0x1;
    pub const IEXTEN: u64 = 0x8000;
    pub const IXON: u64 = 0x400;
    pub const CSIZE: u64 = 0x30;
    pub const CS8: u64 = 0x30;
    pub const PARENB: u64 = 0x100;
}

const ECHO: u64 = 0x8;
const IGNBRK: u64 = 0x1;
const BRKINT: u64 = 0x2;
const PARMRK: u64 = 0x8;
const INPCK: u64 = 0x10;
const ISTRIP: u64 = 0x20;
const INLCR: u64 = 0x40;
const IGNCR: u64 = 0x80;
const ICRNL: u64 = 0x100;
const OPOST: u64 = 0x1;

fn flag(term: &[u8], index: usize) -> u64 {
    let at = index * flags::FLAG;
    if flags::FLAG == 8 {
        u64::from_ne_bytes(term[at..at + 8].try_into().expect("eight"))
    } else {
        u64::from(u32::from_ne_bytes(term[at..at + 4].try_into().expect("four")))
    }
}

fn set_flag(term: &mut [u8], index: usize, value: u64) {
    let at = index * flags::FLAG;
    if flags::FLAG == 8 {
        term[at..at + 8].copy_from_slice(&value.to_ne_bytes());
    } else {
        term[at..at + 4].copy_from_slice(&(value as u32).to_ne_bytes());
    }
}

fn mode_open(value: &Value, mode: &crate::IntValue) -> Result<Value, Error> {
    use flags::*;
    let operation = symbol("terminal_mode_open", 2);
    let state = session(value, &operation).map_err(as_error)?;
    let attempt = || -> Result<Value, Rc<Failure>> {
        let mut session = state.borrow_mut();
        if !session.input_interactive {
            return Err(terminal_failure("unsupported", &operation, "Terminal modes require interactive input"));
        }
        let Some(mode) = mode.to_i64().filter(|mode| (0..=3).contains(mode)) else {
            return Err(terminal_failure("invalid_input", &operation, "Unknown terminal mode"));
        };
        let before = snapshot(&session, &operation)?;
        let mut term = if mode == 0 || mode == 3 { session.original.clone().unwrap_or_else(|| before.clone()) } else { before.clone() };
        let (mut iflag, mut oflag, mut cflag, mut lflag) = (flag(&term, 0), flag(&term, 1), flag(&term, 2), flag(&term, 3));
        if mode == 0 || mode == 3 {
            lflag |= ICANON | ISIG;
            iflag = (iflag & !(IGNCR | INLCR)) | ICRNL;
            oflag |= OPOST;
            if mode == 0 {
                lflag |= ECHO;
            } else {
                lflag &= !(ECHO | ECHONL);
            }
        } else {
            lflag &= !(ICANON | ECHO);
            if mode == 1 {
                lflag |= ISIG;
            }
            term[CC + VMIN] = 1;
            term[CC + VTIME] = 0;
            if mode == 2 {
                lflag &= !(ISIG | IEXTEN);
                iflag &= !(IGNBRK | BRKINT | PARMRK | INLCR | IGNCR | ICRNL | INPCK | ISTRIP | IXON);
                oflag &= !OPOST;
                cflag = (cflag & !(CSIZE | PARENB)) | CS8;
            }
        }
        set_flag(&mut term, 0, iflag);
        set_flag(&mut term, 1, oflag);
        set_flag(&mut term, 2, cflag);
        set_flag(&mut term, 3, lflag);
        apply(&session, &term, &operation)?;
        session.serial += 1;
        let id = session.serial;
        session.modes.push(Mode { id, before, released: false });
        Ok(new_handle(Lease {
            session: state.clone(),
            id,
            dropped: RefCell::new(false),
        }))
    };
    attempt().map_err(as_error)
}

fn mode_closed(session: &Session, id: u32) -> bool {
    !session.modes.iter().any(|mode| mode.id == id && !mode.released)
}

fn mode_close(state: &SessionRef, id: u32, dropping: bool, operation: &str) -> Result<(), Rc<Failure>> {
    let mut session = state.borrow_mut();
    if session.closed || mode_closed(&session, id) {
        return Ok(());
    }
    if !dropping && session.modes.last().is_some_and(|mode| mode.id != id) {
        return Err(terminal_failure("invalid_input", operation, "Terminal mode leases must close in reverse order"));
    }
    for mode in session.modes.iter_mut().filter(|mode| mode.id == id) {
        mode.released = true;
    }
    while session.modes.last().is_some_and(|mode| mode.released) {
        let before = session.modes.last().expect("mode").before.clone();
        apply(&session, &before, operation)?;
        session.modes.pop();
    }
    Ok(())
}

// ---- SIGWINCH ----

static PIPE_WRITE: AtomicI32 = AtomicI32::new(-1);
static GENERATION: AtomicU64 = AtomicU64::new(0);

struct Signals {
    generation: Mutex<u64>,
    changed: Condvar,
}

fn signals() -> &'static Signals {
    static SIGNALS: std::sync::OnceLock<Signals> = std::sync::OnceLock::new();
    SIGNALS.get_or_init(|| Signals {
        generation: Mutex::new(0),
        changed: Condvar::new(),
    })
}

static PRIOR: std::sync::OnceLock<libc::sigaction> = std::sync::OnceLock::new();

extern "C" fn on_winch(signal: libc::c_int) {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    let fd = PIPE_WRITE.load(Ordering::SeqCst);
    if fd >= 0 {
        let byte = 1u8;
        // SAFETY: write(2) безопасен в обработчике сигнала.
        unsafe { libc::write(fd, &byte as *const u8 as *const libc::c_void, 1) };
    }
    if let Some(prior) = PRIOR.get() {
        let handler = prior.sa_sigaction;
        if handler != libc::SIG_DFL && handler != libc::SIG_IGN && handler != on_winch as *const () as usize {
            // SAFETY: прежний обработчик сигнала в форме sa_handler.
            let chained: extern "C" fn(libc::c_int) = unsafe { std::mem::transmute(handler) };
            chained(signal);
        }
    }
}

fn install() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let mut fds = [0i32; 2];
        // SAFETY: pipe(2), sigaction(2).
        unsafe {
            if libc::pipe(fds.as_mut_ptr()) != 0 {
                return;
            }
            libc::fcntl(fds[0], libc::F_SETFD, libc::FD_CLOEXEC);
            libc::fcntl(fds[1], libc::F_SETFD, libc::FD_CLOEXEC);
            PIPE_WRITE.store(fds[1], Ordering::SeqCst);
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = on_winch as *const () as usize;
            action.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&mut action.sa_mask);
            let mut prior: libc::sigaction = std::mem::zeroed();
            libc::sigaction(libc::SIGWINCH, &action, &mut prior);
            let _ = PRIOR.set(prior);
        }
        let read = fds[0];
        std::thread::Builder::new()
            .name("okkama-winch".into())
            .spawn(move || {
                let mut byte = 0u8;
                loop {
                    // SAFETY: собственный конец канала.
                    let count = unsafe { libc::read(read, &mut byte as *mut u8 as *mut libc::c_void, 1) };
                    if count == 0 || (count < 0 && last_errno() != libc::EINTR) {
                        return;
                    }
                    let signals = signals();
                    if let Ok(mut generation) = signals.generation.lock() {
                        *generation = GENERATION.load(Ordering::SeqCst);
                    }
                    signals.changed.notify_all();
                }
            })
            .expect("winch thread");
    });
}

fn current_generation() -> u64 {
    signals().generation.lock().map(|generation| *generation).unwrap_or(0)
}

fn deliver(source: &Rc<RefCell<Resizes>>) {
    let operation = symbol("terminal_resize_next", 1);
    let (promise, session) = {
        let mut borrowed = source.borrow_mut();
        borrowed.notified = false;
        borrowed.seen = current_generation();
        (borrowed.pending.clone(), borrowed.session.clone())
    };
    let Some(promise) = promise else {
        return;
    };
    let outcome = match session {
        Some(session) => size(&session.borrow(), &operation).map(|(columns, rows)| array(vec![int_value(columns), int_value(rows)])),
        None => Ok(array(vec![int_value(0), int_value(0)])),
    };
    sched::settle(&promise, outcome);
}

fn close_resizes(source: &Rc<RefCell<Resizes>>) {
    let (pending, session) = {
        let mut borrowed = source.borrow_mut();
        if borrowed.closed {
            return;
        }
        borrowed.closed = true;
        (borrowed.pending.clone(), borrowed.session.clone())
    };
    if let Some(session) = session {
        if let Ok(mut session) = session.try_borrow_mut() {
            session.resizes.retain(|known| !Rc::ptr_eq(known, source));
        }
    }
    if let Some(pending) = pending {
        sched::settle(
            &pending,
            Err(terminal_failure("already_closed", &symbol("terminal_resize_next", 1), "Terminal resize source is closed")),
        );
    }
}

fn resize_next(value: &Value) -> Result<Value, Error> {
    let operation = symbol("terminal_resize_next", 1);
    let source = handle::<Rc<RefCell<Resizes>>>(value)?.clone();
    if source.borrow().closed {
        return Ok(resolved(Err(terminal_failure("already_closed", &operation, "Terminal resize source is closed"))));
    }
    if source.borrow().pending.is_some() {
        return Ok(resolved(Err(terminal_failure(
            "resource_exhausted",
            &operation,
            "Resize source already has a pending observation",
        ))));
    }
    let promise = sched::promise();
    source.borrow_mut().pending = Some(promise.clone());
    let seen = source.borrow().seen;
    if source.borrow().notified || current_generation() > seen {
        let weak = Rc::downgrade(&source);
        promise.listen(move || {
            if let Some(source) = weak.upgrade() {
                source.borrow_mut().pending = None;
            }
        });
        deliver(&source);
        return Ok(Value::Future(promise));
    }
    // Ожидание сигнала держит программу, как присоединённый Op JVM.
    let waiting = source.clone();
    let op = sched::offload(
        move || {
            let signals = signals();
            let mut generation = signals.generation.lock().expect("winch lock");
            while *generation <= seen {
                generation = signals.changed.wait(generation).expect("winch lock");
            }
        },
        move |()| {
            let pending = waiting.borrow().pending.clone();
            if pending.is_some_and(|pending| pending.state() == sched::State::Running) && !waiting.borrow().closed {
                deliver(&waiting);
            } else {
                waiting.borrow_mut().notified = true;
            }
        },
    );
    let weak = Rc::downgrade(&source);
    promise.listen(move || {
        op.detach();
        if let Some(source) = weak.upgrade() {
            source.borrow_mut().pending = None;
        }
    });
    Ok(Value::Future(promise))
}

// ---- учётные записи ----

const ACCOUNT: &str = "core.account";

fn c_text(pointer: *const c_char) -> Option<String> {
    // SAFETY: строка с завершающим нулём или NULL.
    (!pointer.is_null()).then(|| unsafe { CStr::from_ptr(pointer) }.to_string_lossy().into_owned())
}

fn user(entry: *const libc::passwd, operation: &str) -> Result<Value, Error> {
    if entry.is_null() {
        return Err(as_error(fail(ACCOUNT, "not_found", operation, "User was not found")));
    }
    // SAFETY: запись getpw* действительна до следующего вызова.
    let entry = unsafe { &*entry };
    let Some(name) = c_text(entry.pw_name).filter(|name| !name.is_empty()) else {
        return Err(as_error(fail(ACCOUNT, "invalid_data", operation, "Account provider returned an invalid user")));
    };
    Ok(map([
        ("home".to_string(), text(c_text(entry.pw_dir).unwrap_or_default())),
        ("identity".to_string(), text(entry.pw_uid.to_string())),
        ("name".to_string(), text(name)),
    ]))
}

fn account_name(value: &str, kind: &str, operation: &str) -> Result<CString, Error> {
    if value.is_empty() {
        return Err(as_error(fail(ACCOUNT, "invalid_input", operation, &format!("{kind} name must not be empty"))));
    }
    CString::new(value).map_err(|_| as_error(fail(ACCOUNT, "invalid_input", operation, &format!("{kind} name contains a zero byte"))))
}

fn group(name: &CString, operation: &str) -> Result<*const libc::group, Error> {
    // SAFETY: getgrnam(3).
    let entry = unsafe { libc::getgrnam(name.as_ptr()) };
    if entry.is_null() {
        return Err(as_error(fail(ACCOUNT, "not_found", operation, "Group was not found")));
    }
    Ok(entry)
}

fn account(native: Native, name: &str, args: &[Value]) -> Result<Value, Error> {
    let operation = native.symbol();
    match name {
        // SAFETY: getpwuid(3) текущего пользователя.
        "account_current_user" => user(unsafe { libc::getpwuid(libc::geteuid()) }, operation),
        "account_user_named" => {
            let name = account_name(string(&args[0])?, "User", operation)?;
            // SAFETY: getpwnam(3).
            user(unsafe { libc::getpwnam(name.as_ptr()) }, operation)
        }
        "account_group_named" => {
            let name = account_name(string(&args[0])?, "Group", operation)?;
            let entry = group(&name, operation)?;
            // SAFETY: запись getgrnam действительна до следующего вызова.
            let entry = unsafe { &*entry };
            let Some(found) = c_text(entry.gr_name).filter(|found| !found.is_empty()) else {
                return Err(as_error(fail(ACCOUNT, "invalid_data", operation, "Account provider returned an invalid group")));
            };
            Ok(map([
                ("identity".to_string(), text(entry.gr_gid.to_string())),
                ("name".to_string(), text(found)),
            ]))
        }
        "account_group_users" => {
            let name = account_name(string(&args[0])?, "Group", operation)?;
            let entry = group(&name, operation)?;
            // SAFETY: запись getgrnam, список членов завершён NULL.
            let (gid, mut names) = unsafe {
                let entry = &*entry;
                let mut names = std::collections::BTreeSet::new();
                let mut member = entry.gr_mem;
                while !member.is_null() && !(*member).is_null() {
                    if let Some(login) = c_text(*member).filter(|login| !login.is_empty()) {
                        names.insert(login);
                    }
                    member = member.add(1);
                }
                (entry.gr_gid, names)
            };
            // SAFETY: перебор базы пользователей.
            unsafe {
                libc::setpwent();
                loop {
                    let user = libc::getpwent();
                    if user.is_null() {
                        break;
                    }
                    if let Some(login) = c_text((*user).pw_name) {
                        if (*user).pw_gid == gid {
                            names.insert(login);
                        }
                    }
                }
                libc::endpwent();
            }
            Ok(string_array(std::mem::take(&mut names)))
        }
        _ => Err(Error::Unsupported(format!("native provider is not implemented: {}", native.symbol()))),
    }
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let name = native.symbol().split("::").nth(1).and_then(|rest| rest.split('/').next()).unwrap_or("");
    if native.symbol().starts_with("runtime/account@") {
        return account(native, name, args);
    }
    let operation = native.symbol();
    match name {
        "terminal_standard" => {
            let attempt = || -> Result<Value, Rc<Failure>> {
                let kind = int(&args[0]).ok().and_then(|kind| kind.to_i64()).filter(|kind| *kind == 0 || *kind == 1);
                let Some(kind) = kind else {
                    return Err(terminal_failure("invalid_input", operation, "Terminal output must be stdout (0) or stderr (1)"));
                };
                use std::io::Write;
                let _ = std::io::stdout().flush();
                let _ = std::io::stderr().flush();
                let input = dup(0, operation)?;
                let output = match dup(if kind == 0 { 1 } else { 2 }, operation) {
                    Ok(output) => output,
                    Err(failure) => {
                        unsafe { libc::close(input) };
                        return Err(failure);
                    }
                };
                let session = new_session(input, output, false);
                Ok(new_handle(opened(session, &["fd:0".to_string(), format!("fd:{}", kind + 1)], operation)?))
            };
            Ok(resolved(attempt()))
        }
        "terminal_controlling" => {
            let attempt = || -> Result<Value, Rc<Failure>> {
                let path = CString::new("/dev/tty").expect("no NUL");
                // SAFETY: open(2).
                let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
                if fd < 0 {
                    return Err(terminal_failure("not_found", operation, "No controlling terminal is available"));
                }
                let output = match dup(fd, operation) {
                    Ok(output) => output,
                    Err(failure) => {
                        unsafe { libc::close(fd) };
                        return Err(failure);
                    }
                };
                let session = new_session(fd, output, true);
                Ok(new_handle(opened(session, &["tty".to_string()], operation)?))
            };
            Ok(resolved(attempt()))
        }
        "terminal_metadata" => {
            let state = session(&args[0], operation).map_err(as_error)?;
            let session = state.borrow();
            let (columns, rows) = size(&session, operation).map_err(as_error)?;
            Ok(int_map([
                ("colors", i64::from(session.output_interactive)),
                ("columns", columns),
                ("controlling", i64::from(session.controlling)),
                ("input", i64::from(session.input_interactive)),
                ("output", i64::from(session.output_interactive)),
                ("rows", rows),
            ]))
        }
        "terminal_write" => {
            let state = session(&args[0], operation).map_err(as_error)?;
            let data = bytes(&args[1])?;
            if data.len() > 65536 {
                return Err(as_error(terminal_failure("invalid_input", operation, "Terminal write exceeds 64 KiB")));
            }
            if data.is_empty() {
                return Ok(int_value(0));
            }
            let output = state.borrow().output;
            Ok(int_value(write_fd(output, &data, operation).map_err(as_error)? as i64))
        }
        "terminal_read" => Ok(terminal_read(&args[0], int(&args[1])?)),
        "terminal_cleanup" => {
            let state = session(&args[0], operation).map_err(as_error)?;
            if items(&args[1])?.len() > 256 {
                return Err(as_error(terminal_failure("invalid_input", operation, "Terminal cleanup exceeds 256 bytes")));
            }
            state.borrow_mut().cleanup = bytes(&args[1])?;
            Ok(Value::Bool(true))
        }
        "terminal_closed" => Ok(Value::Bool(handle::<SessionRef>(&args[0])?.borrow().closed)),
        "terminal_close" => {
            close(handle::<SessionRef>(&args[0])?).map_err(as_error)?;
            Ok(Value::Bool(true))
        }
        "terminal_mode_open" => mode_open(&args[0], int(&args[1])?),
        "terminal_mode_closed" => {
            let lease = handle::<Lease>(&args[0])?;
            Ok(Value::Bool(*lease.dropped.borrow() || mode_closed(&lease.session.borrow(), lease.id)))
        }
        "terminal_mode_close" | "terminal_mode_drop" => {
            let lease = handle::<Lease>(&args[0])?;
            if *lease.dropped.borrow() {
                return Ok(Value::Bool(true));
            }
            let dropping = name == "terminal_mode_drop";
            if dropping {
                *lease.dropped.borrow_mut() = true;
            }
            mode_close(&lease.session, lease.id, dropping, operation).map_err(as_error)?;
            *lease.dropped.borrow_mut() = true;
            Ok(Value::Bool(true))
        }
        "terminal_resize_open" => {
            let attempt = || -> Result<Value, Rc<Failure>> {
                let state = session(&args[0], operation)?;
                if !state.borrow().output_interactive {
                    return Err(terminal_failure("unsupported", operation, "Resize notifications require interactive output"));
                }
                install();
                let source = Rc::new(RefCell::new(Resizes {
                    session: Some(state.clone()),
                    closed: false,
                    notified: true,
                    seen: current_generation(),
                    pending: None,
                }));
                state.borrow_mut().resizes.push(source.clone());
                Ok(new_handle(source))
            };
            Ok(resolved(attempt()))
        }
        "terminal_resize_next" => resize_next(&args[0]),
        "terminal_resize_closed" => Ok(Value::Bool(handle::<Rc<RefCell<Resizes>>>(&args[0])?.borrow().closed)),
        "terminal_resize_close" => {
            close_resizes(handle::<Rc<RefCell<Resizes>>>(&args[0])?);
            Ok(Value::Bool(true))
        }
        _ => Err(Error::Unsupported(format!("native provider is not implemented: {}", native.symbol()))),
    }
}

