//! `runtime/network`: TCP-слушатели и потоки, UDP, multicast, Unix-сокеты и
//! системный резолвер — перенос `NetNatives` и `UnixNatives` JVM. Сокеты
//! хоста неблокирующие: ожидающая операция — на рабочем потоке, который ждёт
//! готовности `poll(2)` и замечает закрытие; исход приходит событием цикла
//! (`sched::offload`). Результат, пришедший после отмены ожидания
//! (принятое соединение, прочитанные байты), не теряется: его получит
//! следующая операция. Тексты отказов — как у Perl-provider.
#![allow(unsafe_code)]

use super::net::{IPV4, IPV6, decode, encode, format, parse_ipv4, parse_ipv6};
use super::support::{array, byte_array, bytes, handle, int, new_handle, string, string_array, text};
use super::Error;
use crate::failure::{Failure, Kind};
use crate::sched;
use crate::{Value, abi::Native};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::os::fd::AsRawFd;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const DOMAIN: &str = "core.net";
const UNIX: u8 = 3;
const CHUNK: usize = 65536;

fn symbol(name: &str, arity: usize) -> String {
    format!("runtime/network@1::{name}/{arity}")
}

fn fail(code: &str, operation: &str, message: impl Into<String>) -> Rc<Failure> {
    Failure::new(Kind::Native(Rc::new(crate::abi::NativeFailure {
        domain: DOMAIN.into(),
        code: code.into(),
        operation: operation.into(),
        message: message.into(),
    })))
}

fn as_error(failure: Rc<Failure>) -> Error {
    match &failure.kind {
        Kind::Native(native) => Error::Failure((**native).clone()),
        _ => Error::Trap(failure.message().to_string()),
    }
}

fn resolved_failure(failure: Rc<Failure>) -> Value {
    Value::Future(sched::resolved(Err(failure)))
}

/// Код переносимого отказа по ошибке хоста (аналог errno у Perl).
fn host_code(error: &std::io::Error) -> &'static str {
    let text = error.to_string().to_lowercase();
    if text.contains("socket is closed") {
        return "already_closed";
    }
    match error.raw_os_error() {
        Some(libc::EADDRINUSE) => return "address_in_use",
        Some(libc::EADDRNOTAVAIL) => return "address_not_available",
        Some(libc::ECONNREFUSED) => return "connection_refused",
        Some(libc::ENETUNREACH) | Some(libc::EHOSTUNREACH) => return "network_unreachable",
        Some(libc::ETIMEDOUT) => return "timed_out",
        Some(libc::ECONNRESET) | Some(libc::ECONNABORTED) => return "connection_reset",
        Some(libc::EPIPE) => return "broken_pipe",
        Some(libc::ENOTCONN) => return "not_connected",
        Some(libc::EACCES) | Some(libc::EPERM) => return "permission_denied",
        Some(libc::EMFILE) | Some(libc::ENFILE) => return "resource_exhausted",
        _ => {}
    }
    if text.contains("address already in use") {
        "address_in_use"
    } else if text.contains("connection refused") {
        "connection_refused"
    } else if text.contains("unreachable") {
        "network_unreachable"
    } else if text.contains("timed out") {
        "timed_out"
    } else if text.contains("reset") || text.contains("aborted") {
        "connection_reset"
    } else if text.contains("broken pipe") {
        "broken_pipe"
    } else if text.contains("not connected") {
        "not_connected"
    } else if text.contains("permission denied") || text.contains("not permitted") {
        "permission_denied"
    } else if text.contains("too many open files") {
        "resource_exhausted"
    } else {
        "unexpected"
    }
}

fn host_failure(operation: &str, message: &str, error: &std::io::Error) -> Rc<Failure> {
    fail(host_code(error), operation, message)
}

fn closed_error() -> std::io::Error {
    std::io::Error::other("socket is closed")
}

/// Сокет, общий с рабочими потоками: закрытие видно им через флаг.
#[derive(Debug)]
pub(crate) struct Shared {
    pub(crate) socket: Socket,
    closed: AtomicBool,
}

impl Shared {
    fn new(socket: Socket) -> Arc<Self> {
        let _ = socket.set_nonblocking(true);
        Arc::new(Self {
            socket,
            closed: AtomicBool::new(false),
        })
    }

    /// Ждать готовности; закрытие сокета прерывает ожидание.
    fn wait(&self, events: libc::c_short) -> std::io::Result<()> {
        loop {
            if self.closed.load(Ordering::SeqCst) {
                return Err(closed_error());
            }
            let mut poll = libc::pollfd {
                fd: self.socket.as_raw_fd(),
                events,
                revents: 0,
            };
            // SAFETY: один pollfd с действительным дескриптором.
            let ready = unsafe { libc::poll(&mut poll, 1, 50) };
            if ready < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if ready > 0 {
                return Ok(());
            }
        }
    }

    pub(crate) fn retry<T>(&self, events: libc::c_short, mut operation: impl FnMut(&Socket) -> std::io::Result<T>) -> std::io::Result<T> {
        loop {
            self.wait(events)?;
            match operation(&self.socket) {
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                other => return other,
            }
        }
    }

    pub(crate) fn close(&self) {
        if !self.closed.swap(true, Ordering::SeqCst) {
            let _ = self.socket.shutdown(std::net::Shutdown::Both);
        }
    }
}

pub(crate) fn read_chunk(shared: &Shared) -> std::io::Result<Option<Vec<u8>>> {
    let mut buffer = vec![std::mem::MaybeUninit::<u8>::uninit(); CHUNK];
    let count = shared.retry(libc::POLLIN, |socket| socket.recv(&mut buffer))?;
    if count == 0 {
        return Ok(None);
    }
    // SAFETY: recv инициализировал первые `count` байтов.
    Ok(Some(buffer[..count].iter().map(|byte| unsafe { byte.assume_init() }).collect()))
}

pub(crate) fn write_all(shared: &Shared, data: &[u8]) -> std::io::Result<()> {
    let mut offset = 0;
    while offset < data.len() {
        let sent = shared.retry(libc::POLLOUT, |socket| socket.send(&data[offset..]))?;
        offset += sent;
    }
    Ok(())
}

/// Адрес сокета хоста; зона IPv6 — числовой индекс или имя интерфейса.
fn socket_address(family: u8, port: u16, address: &[u8], zone: &str, operation: &str) -> Result<SocketAddr, Rc<Failure>> {
    if family == IPV4 {
        if !zone.is_empty() {
            return Err(fail("invalid_input", operation, "Invalid IPv6 binding zone"));
        }
        let ip = Ipv4Addr::new(address[0], address[1], address[2], address[3]);
        return Ok(SocketAddr::V4(SocketAddrV4::new(ip, port)));
    }
    let raw: [u8; 16] = address.try_into().expect("sixteen bytes");
    let ip = Ipv6Addr::from(raw);
    if zone.is_empty() {
        return Ok(SocketAddr::V6(SocketAddrV6::new(ip, port, 0, 0)));
    }
    if !zone.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'~' | b'-')) {
        return Err(fail("invalid_input", operation, "Invalid IPv6 binding zone"));
    }
    if zone.bytes().all(|b| b.is_ascii_digit()) {
        let digits = zone.trim_start_matches('0');
        if digits.len() > 10 || (digits.len() == 10 && digits > "4294967295") {
            return Err(fail("invalid_input", operation, "IPv6 scope index exceeds uint32"));
        }
        let scope = if digits.is_empty() { 0 } else { digits.parse::<u64>().unwrap_or(0) as u32 };
        return Ok(SocketAddr::V6(SocketAddrV6::new(ip, port, 0, scope)));
    }
    let index = interface_index(zone);
    if index == 0 {
        return Err(fail("invalid_input", operation, "Unknown or unsupported IPv6 interface zone"));
    }
    Ok(SocketAddr::V6(SocketAddrV6::new(ip, port, 0, index)))
}

fn interface_index(name: &str) -> u32 {
    let Ok(text) = std::ffi::CString::new(name) else {
        return 0;
    };
    // SAFETY: строка с завершающим нулём живёт до конца вызова.
    unsafe { libc::if_nametoindex(text.as_ptr()) }
}

fn address_bytes(address: &SocketAddr) -> Vec<u8> {
    match address.ip() {
        IpAddr::V4(ip) => ip.octets().to_vec(),
        IpAddr::V6(ip) => ip.octets().to_vec(),
    }
}

/// [host, port, scope|""] как у Perl `inet_ntop`.
fn endpoint_strings(address: &SocketAddr, family: u8) -> Value {
    let raw = address_bytes(address);
    let mut scope = String::new();
    let host = if family == IPV6 && raw.len() == 4 {
        let mut mapped = [0u8; 16];
        mapped[10] = 0xff;
        mapped[11] = 0xff;
        mapped[12..].copy_from_slice(&raw);
        format(&mapped)
    } else {
        if let SocketAddr::V6(v6) = address {
            if v6.scope_id() != 0 {
                scope = v6.scope_id().to_string();
            }
        }
        format(&raw)
    };
    string_array([host, address.port().to_string(), scope])
}

fn sock_address(address: &SockAddr) -> Option<SocketAddr> {
    address.as_socket()
}

// ---- состояния ----

#[derive(Debug)]
struct Listener {
    shared: Arc<Shared>,
    family: u8,
    listening: bool,
    closed: bool,
    accepting: Option<sched::Future>,
    keep_alive: bool,
    no_delay: bool,
    worker: Option<sched::Op>,
    path: String,
    identity: (u64, u64),
    /// Принятые без ожидающего `accept`: соединение или ошибка.
    ready: VecDeque<std::io::Result<Socket>>,
}

type ListenerRef = Rc<RefCell<Listener>>;

#[derive(Debug)]
struct Stream {
    shared: Arc<Shared>,
    family: u8,
    unnamed: bool,
    input_closed: bool,
    output_closed: bool,
    eof: bool,
    reading: Option<sched::Future>,
    read_limit: usize,
    reader: Option<sched::Op>,
    pending: Option<Vec<u8>>,
    read_error: Option<std::io::Error>,
    writing: Option<sched::Future>,
}

#[derive(Debug)]
struct Membership {
    socket: Weak<RefCell<Sock>>,
    group: Vec<u8>,
    family: u8,
    interface: u32,
    closed: bool,
}

#[derive(Debug)]
struct Udp {
    shared: Arc<Shared>,
    family: u8,
    dual_stack: bool,
    receiving: Option<sched::Future>,
    sending: Option<sched::Future>,
    peer: Option<SocketAddr>,
    peer_maximum: usize,
    multicast_policy: Option<String>,
    memberships: Vec<(String, Rc<RefCell<Membership>>)>,
}

#[derive(Debug)]
enum Kind2 {
    Stream(Stream),
    Udp(Udp),
}

#[derive(Debug)]
struct Sock {
    closed: bool,
    kind: Kind2,
}

type SockRef = Rc<RefCell<Sock>>;

fn stream_label(stream: &Stream) -> &'static str {
    if stream.family == UNIX { "Unix socket" } else { "TCP socket" }
}

type Pending = Vec<(sched::Future, Rc<Failure>)>;

/// Закрыть поток; ожидающие операции завершаются вызывающим после
/// освобождения состояния (их слушатели снова его берут).
fn close_stream(sock: &mut Sock) -> Pending {
    let mut pending = Pending::new();
    sock.closed = true;
    if let Kind2::Stream(stream) = &mut sock.kind {
        stream.shared.close();
        if let Some(reading) = stream.reading.take() {
            pending.push((reading, fail("already_closed", &symbol("stream_read", 2), "Socket is closed")));
        }
        if let Some(writing) = stream.writing.take() {
            pending.push((writing, fail("already_closed", &symbol("stream_write", 2), "Socket is closed")));
        }
    }
    pending
}

fn shut(sock: &mut Sock) -> Pending {
    match &mut sock.kind {
        Kind2::Stream(_) => close_stream(sock),
        Kind2::Udp(udp) => {
            let mut pending = Pending::new();
            sock.closed = true;
            for (_, member) in udp.memberships.drain(..) {
                member.borrow_mut().closed = true;
            }
            udp.shared.close();
            if let Some(receiving) = udp.receiving.take() {
                pending.push((receiving, fail("already_closed", &symbol("udp_receive", 2), "Socket is closed")));
            }
            if let Some(sending) = udp.sending.take() {
                pending.push((sending, fail("already_closed", &symbol("udp_send", 4), "Socket is closed")));
            }
            pending
        }
    }
}

fn settle_all(pending: Pending) {
    for (future, failure) in pending {
        sched::settle(&future, Err(failure));
    }
}

/// `shut` над состоянием: завершения — после освобождения заимствования.
fn shut_now(state: &SockRef) {
    let pending = shut(&mut state.borrow_mut());
    settle_all(pending);
}

fn sock(value: &Value) -> Result<SockRef, Error> {
    Ok(handle::<SockRef>(value)?.clone())
}

fn listener(value: &Value, operation: &str) -> Result<ListenerRef, Rc<Failure>> {
    let state = handle::<ListenerRef>(value).map_err(|_| fail("invalid_input", operation, "Invalid listener"))?.clone();
    if state.borrow().closed {
        return Err(fail("already_closed", operation, "Native handle is already closed"));
    }
    Ok(state)
}

/// Управляемый поток (`managed_stream` Perl): TCP или Unix.
fn stream_of(value: &Value, operation: &str) -> Result<SockRef, Rc<Failure>> {
    let state = sock(value).map_err(|_| fail("invalid_input", operation, "Invalid socket"))?;
    {
        let borrowed = state.borrow();
        if borrowed.closed {
            return Err(fail("already_closed", operation, "Native handle is already closed"));
        }
        if !matches!(borrowed.kind, Kind2::Stream(_)) {
            return Err(fail("invalid_input", operation, "Operation requires a managed stream connection"));
        }
    }
    Ok(state)
}

fn udp_of(value: &Value, operation: &str) -> Result<SockRef, Rc<Failure>> {
    let state = sock(value).map_err(|_| fail("invalid_input", operation, "Invalid socket"))?;
    {
        let borrowed = state.borrow();
        if borrowed.closed {
            return Err(fail("already_closed", operation, "Native handle is already closed"));
        }
        if !matches!(borrowed.kind, Kind2::Udp(_)) {
            return Err(fail("invalid_input", operation, "Operation requires a datagram socket"));
        }
    }
    Ok(state)
}

fn with_stream<R>(state: &SockRef, f: impl FnOnce(&mut Stream) -> R) -> R {
    match &mut state.borrow_mut().kind {
        Kind2::Stream(stream) => f(stream),
        Kind2::Udp(_) => unreachable!("checked stream"),
    }
}

fn with_udp<R>(state: &SockRef, f: impl FnOnce(&mut Udp) -> R) -> R {
    match &mut state.borrow_mut().kind {
        Kind2::Udp(udp) => f(udp),
        Kind2::Stream(_) => unreachable!("checked datagram"),
    }
}

fn new_stream(socket: Socket, family: u8, unnamed: bool) -> SockRef {
    Rc::new(RefCell::new(Sock {
        closed: false,
        kind: Kind2::Stream(Stream {
            shared: Shared::new(socket),
            family,
            unnamed,
            input_closed: false,
            output_closed: false,
            eof: false,
            reading: None,
            read_limit: CHUNK,
            reader: None,
            pending: None,
            read_error: None,
            writing: None,
        }),
    }))
}

/// TLS-клиент отбирает TCP-поток (`TlsNatives.client` JVM): сетевой
/// дескриптор закрыт, сокет — нет; непрочитанные байты переходят к TLS.
pub(crate) fn take_tcp(value: &Value, operation: &str) -> Result<(Arc<Shared>, Vec<u8>), Rc<Failure>> {
    let state = stream_of(value, operation)?;
    let mut borrowed = state.borrow_mut();
    let Kind2::Stream(stream) = &mut borrowed.kind else {
        unreachable!("checked stream");
    };
    if stream.family != IPV4 && stream.family != IPV6 {
        return Err(fail("invalid_input", operation, "TLS requires a managed TCP stream"));
    }
    if stream.input_closed || stream.output_closed || stream.eof {
        return Err(fail("already_closed", operation, "TLS requires both open TCP halves"));
    }
    if stream.shared.socket.peer_addr().is_err() {
        return Err(fail("invalid_input", operation, "TLS requires a connected TCP stream"));
    }
    if stream.reading.is_some() || stream.writing.is_some() || stream.reader.is_some() {
        return Err(fail("resource_exhausted", operation, "TLS requires drained TCP operations"));
    }
    let early = stream.pending.take().unwrap_or_default();
    let shared = stream.shared.clone();
    borrowed.closed = true;
    Ok((shared, early))
}

// ---- accept ----

fn accept_symbol(state: &Listener) -> String {
    if state.family == UNIX { symbol("unix_accept", 1) } else { symbol("tcp_accept", 3) }
}

fn accept(state: &ListenerRef, keep_alive: bool, no_delay: bool) -> Value {
    let mut listener = state.borrow_mut();
    if listener.accepting.is_some() {
        let what = if listener.family == UNIX { "Unix socket" } else { "TCP listener" };
        return resolved_failure(fail(
            "resource_exhausted",
            &accept_symbol(&listener),
            format!("{what} already has a pending accepting"),
        ));
    }
    let promise = sched::promise();
    listener.accepting = Some(promise.clone());
    listener.keep_alive = keep_alive;
    listener.no_delay = no_delay;
    let weak = Rc::downgrade(state);
    promise.listen(move || {
        if let Some(state) = weak.upgrade()
            && let Ok(mut listener) = state.try_borrow_mut()
        {
            listener.accepting = None;
            if let Some(worker) = &listener.worker {
                worker.detach();
            }
        }
    });
    if !listener.ready.is_empty() {
        drop(listener);
        serve_accept(state);
    } else if let Some(worker) = &listener.worker {
        worker.attach();
    } else {
        drop(listener);
        start_accept(state);
    }
    Value::Future(promise)
}

fn start_accept(state: &ListenerRef) {
    let shared = state.borrow().shared.clone();
    let weak = Rc::downgrade(state);
    let op = sched::offload(
        move || shared.retry(libc::POLLIN, |socket| socket.accept().map(|(client, _)| client)),
        move |result: std::io::Result<Socket>| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            let accepting = {
                let mut listener = state.borrow_mut();
                listener.worker = None;
                listener.ready.push_back(result);
                if listener.accepting.is_none() && listener.closed {
                    listener.ready.clear();
                }
                listener.accepting.is_some()
            };
            if accepting {
                serve_accept(&state);
            }
        },
    );
    state.borrow_mut().worker = Some(op);
}

fn serve_accept(state: &ListenerRef) {
    let (promise, next, family, keep_alive, no_delay, closed, operation) = {
        let mut listener = state.borrow_mut();
        let Some(promise) = listener.accepting.clone() else {
            return;
        };
        let next = listener.ready.pop_front().expect("ready connection");
        (promise, next, listener.family, listener.keep_alive, listener.no_delay, listener.closed, accept_symbol(&listener))
    };
    match next {
        Err(error) => {
            let failure = if closed {
                fail("already_closed", &operation, "Socket is closed")
            } else {
                let message = if family == UNIX { "Cannot accept Unix connection" } else { "Cannot accept TCP connection" };
                host_failure(&operation, message, &error)
            };
            sched::settle(&promise, Err(failure));
        }
        Ok(client) => {
            if family != UNIX {
                let applied = client.set_keepalive(keep_alive).and_then(|()| client.set_nodelay(no_delay));
                if let Err(error) = applied {
                    sched::settle(&promise, Err(host_failure(&operation, "Cannot apply TCP connection option", &error)));
                    return;
                }
            }
            sched::settle(&promise, Ok(new_handle(new_stream(client, family, false))));
        }
    }
}

/// Закрывает listener; `false`, если уже был закрыт.
fn close_listener(state: &ListenerRef) -> bool {
    let accepting = {
        let mut listener = state.borrow_mut();
        if listener.closed {
            return false;
        }
        listener.closed = true;
        listener.shared.close();
        listener.ready.clear();
        listener.accepting.clone()
    };
    if let Some(accepting) = accepting {
        let operation = accept_symbol(&state.borrow());
        sched::settle(&accepting, Err(fail("already_closed", &operation, "Socket is closed")));
    }
    true
}

// ---- чтение и запись потока ----

fn stream_read(value: &Value, limit: &crate::IntValue) -> Value {
    let operation = symbol("stream_read", 2);
    if limit.signum() <= 0 {
        return resolved_failure(fail("invalid_input", &operation, "Stream read limit must be positive"));
    }
    let state = match stream_of(value, &operation) {
        Ok(state) => state,
        Err(failure) => return resolved_failure(failure),
    };
    let label = with_stream(&state, |stream| stream_label(stream));
    let busy = with_stream(&state, |stream| (stream.input_closed, stream.reading.is_some()));
    if busy.0 {
        return resolved_failure(fail("already_closed", &operation, "Stream input is closed"));
    }
    if busy.1 {
        return resolved_failure(fail("resource_exhausted", &operation, format!("{label} already has a pending reading")));
    }
    let promise = sched::promise();
    let ready = with_stream(&state, |stream| {
        stream.reading = Some(promise.clone());
        stream.read_limit = limit.to_i64().map_or(CHUNK, |limit| (limit as usize).min(CHUNK));
        stream.pending.is_some() || stream.read_error.is_some() || stream.eof
    });
    let weak = Rc::downgrade(&state);
    promise.listen(move || {
        if let Some(state) = weak.upgrade() {
            if let Ok(mut borrowed) = state.try_borrow_mut()
                && let Kind2::Stream(stream) = &mut borrowed.kind
            {
                stream.reading = None;
                if let Some(reader) = &stream.reader {
                    reader.detach();
                }
            }
        }
    });
    if ready {
        serve_read(&state);
    } else if let Some(reader) = with_stream(&state, |stream| stream.reader.clone()) {
        reader.attach();
    } else {
        start_read(&state);
    }
    Value::Future(promise)
}

fn start_read(state: &SockRef) {
    let shared = with_stream(state, |stream| stream.shared.clone());
    let weak = Rc::downgrade(state);
    let op = sched::offload(
        move || read_chunk(&shared),
        move |result: std::io::Result<Option<Vec<u8>>>| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            let reading = with_stream(&state, |stream| {
                stream.reader = None;
                match result {
                    Err(error) => stream.read_error = Some(error),
                    Ok(None) => stream.eof = true,
                    Ok(Some(data)) => stream.pending = Some(data),
                }
                stream.reading.is_some()
            });
            if reading {
                serve_read(&state);
            }
        },
    );
    with_stream(state, |stream| stream.reader = Some(op));
}

fn serve_read(state: &SockRef) {
    let operation = symbol("stream_read", 2);
    let closed = state.borrow().closed;
    let (promise, outcome, restart) = with_stream(state, |stream| {
        let promise = stream.reading.clone().expect("pending reading");
        if closed {
            return (promise, Some(Err(fail("already_closed", &operation, "Socket is closed"))), false);
        }
        if let Some(mut data) = stream.pending.take() {
            let count = data.len().min(stream.read_limit);
            let rest = data.split_off(count);
            if !rest.is_empty() {
                stream.pending = Some(rest);
            }
            return (promise, Some(Ok(byte_array(data))), false);
        }
        if let Some(error) = stream.read_error.take() {
            return (promise, Some(Err(host_failure(&operation, "Cannot read managed stream", &error))), false);
        }
        if stream.eof {
            return (promise, Some(Ok(byte_array([]))), false);
        }
        (promise, None, stream.reader.is_none())
    });
    match outcome {
        Some(outcome) => sched::settle(&promise, outcome),
        None if restart => start_read(state),
        None => {}
    }
}

fn stream_write(value: &Value, payload: Vec<u8>) -> Value {
    let operation = symbol("stream_write", 2);
    let state = match stream_of(value, &operation) {
        Ok(state) => state,
        Err(failure) => return resolved_failure(failure),
    };
    let label = with_stream(&state, |stream| stream_label(stream));
    let (output_closed, writing) = with_stream(&state, |stream| (stream.output_closed, stream.writing.is_some()));
    if output_closed {
        return resolved_failure(fail("already_closed", &operation, "Stream output is closed"));
    }
    if writing {
        return resolved_failure(fail("resource_exhausted", &operation, format!("{label} already has a pending writing")));
    }
    if payload.is_empty() {
        return Value::Future(sched::resolved(Ok(Value::Bool(true))));
    }
    let promise = sched::promise();
    let shared = with_stream(&state, |stream| {
        stream.writing = Some(promise.clone());
        stream.shared.clone()
    });
    let target = promise.clone();
    let weak = Rc::downgrade(&state);
    let op = sched::offload(
        move || write_all(&shared, &payload),
        move |result: std::io::Result<()>| {
            let closed = weak.upgrade().is_none_or(|state| state.borrow().closed);
            let outcome = match result {
                Ok(()) => Ok(Value::Bool(true)),
                Err(_) if closed => Err(fail("already_closed", &operation, "Socket is closed")),
                Err(error) => Err(host_failure(&operation, "Cannot write managed stream", &error)),
            };
            sched::settle(&target, outcome);
        },
    );
    let weak = Rc::downgrade(&state);
    let watched = promise.clone();
    promise.listen(move || {
        op.detach();
        if let Some(state) = weak.upgrade() {
            if let Ok(mut borrowed) = state.try_borrow_mut()
                && let Kind2::Stream(stream) = &mut borrowed.kind
            {
                stream.writing = None;
            }
            // Как у Perl: незавершённая запись (отказ или отмена) закрывает поток.
            if watched.state() != sched::State::Done && !state.borrow().closed {
                let pending = close_stream(&mut state.borrow_mut());
                settle_all(pending);
            }
        }
    });
    Value::Future(promise)
}

// ---- UDP ----

fn ipv4_mapped(family: u8, address: &[u8]) -> bool {
    family == IPV6 && address[..10].iter().all(|b| *b == 0) && address[10] == 0xff && address[11] == 0xff
}

fn destination(udp: &Udp, family: u8, port: u16, address: &[u8], zone: &str, operation: &str) -> Result<SocketAddr, Rc<Failure>> {
    let ipv4 = family == IPV4 || ipv4_mapped(family, address);
    let (family, address) = if udp.family != family {
        if !(udp.family == IPV6 && udp.dual_stack) {
            return Err(fail("unsupported", operation, "UDP socket and destination families do not match"));
        }
        if !zone.is_empty() {
            return Err(fail("invalid_input", operation, "IPv4 destination cannot have a zone"));
        }
        let mut mapped = vec![0u8; 16];
        mapped[10] = 0xff;
        mapped[11] = 0xff;
        mapped[12..].copy_from_slice(address);
        (IPV6, mapped)
    } else {
        (family, address.to_vec())
    };
    if udp.family == IPV6 && ipv4 && !udp.dual_stack {
        return Err(fail("unsupported", operation, "IPv6-only socket cannot send to IPv4"));
    }
    socket_address(family, port, &address, zone, operation)
}

/// Конверт принятого пакета v3: 'O' 'K' 3 усечён scope, затем конверт v2.
fn received_packet(source: &SocketAddr, payload: &[u8], limit: usize) -> Value {
    let raw = address_bytes(source);
    let family = if raw.len() == 4 { IPV4 } else { IPV6 };
    let address = encode(family, source.port(), &raw);
    let scope = match source {
        SocketAddr::V6(v6) => v6.scope_id(),
        SocketAddr::V4(_) => 0,
    };
    let truncated = payload.len() > limit;
    let mut out = vec![79, 75, 3, u8::from(truncated)];
    out.extend(scope.to_be_bytes());
    out.extend([79, 75, 2, (address.len() >> 8) as u8, address.len() as u8]);
    out.extend(&address);
    out.extend(&payload[..payload.len().min(limit)]);
    byte_array(out)
}

fn receive(state: &SockRef, limit: &crate::IntValue, operation: &str) -> Value {
    if with_udp(state, |udp| udp.receiving.is_some()) {
        return resolved_failure(fail("resource_exhausted", operation, "UDP socket already has a pending receive"));
    }
    let limit = limit.to_i64().map_or(65536, |limit| (limit as usize).min(65536));
    let promise = sched::promise();
    let shared = with_udp(state, |udp| {
        udp.receiving = Some(promise.clone());
        udp.shared.clone()
    });
    let target = promise.clone();
    let weak = Rc::downgrade(state);
    let name = operation.to_string();
    let op = sched::offload(
        move || {
            let mut buffer = vec![std::mem::MaybeUninit::<u8>::uninit(); 65536];
            let (count, source) = shared.retry(libc::POLLIN, |socket| socket.recv_from(&mut buffer))?;
            // SAFETY: recv_from инициализировал первые `count` байтов.
            let data: Vec<u8> = buffer[..count].iter().map(|byte| unsafe { byte.assume_init() }).collect();
            Ok::<_, std::io::Error>((data, sock_address(&source)))
        },
        move |result| {
            let closed = weak.upgrade().is_none_or(|state| state.borrow().closed);
            let outcome = match result {
                Ok((data, Some(source))) => Ok(received_packet(&source, &data, limit)),
                Ok((_, None)) => Err(fail("unexpected", &name, "Cannot receive UDP datagram")),
                Err(_) if closed => Err(fail("already_closed", &name, "Socket is closed")),
                Err(error) => Err(host_failure(&name, "Cannot receive UDP datagram", &error)),
            };
            sched::settle(&target, outcome);
        },
    );
    let weak = Rc::downgrade(state);
    promise.listen(move || {
        op.detach();
        if let Some(state) = weak.upgrade() {
            if let Ok(mut borrowed) = state.try_borrow_mut()
                && let Kind2::Udp(udp) = &mut borrowed.kind
            {
                udp.receiving = None;
            }
        }
    });
    Value::Future(promise)
}

fn send(state: &SockRef, payload: Vec<u8>, target_address: Option<SocketAddr>, operation: &str) -> Value {
    if with_udp(state, |udp| udp.sending.is_some()) {
        return resolved_failure(fail("resource_exhausted", operation, "UDP socket already has a pending send"));
    }
    let promise = sched::promise();
    let shared = with_udp(state, |udp| {
        udp.sending = Some(promise.clone());
        udp.shared.clone()
    });
    let size = payload.len();
    let target = promise.clone();
    let weak = Rc::downgrade(state);
    let name = operation.to_string();
    let op = sched::offload(
        move || {
            shared.retry(libc::POLLOUT, |socket| match &target_address {
                Some(address) => socket.send_to(&payload, &SockAddr::from(*address)),
                None => socket.send(&payload),
            })
        },
        move |result: std::io::Result<usize>| {
            let closed = weak.upgrade().is_none_or(|state| state.borrow().closed);
            let outcome = match result {
                Ok(count) if count == size => Ok(Value::Int(crate::IntValue::from_i64(count as i64))),
                Ok(_) => Err(fail("unexpected", &name, "Host reported an incomplete UDP datagram send")),
                Err(error) if error.to_string().to_lowercase().contains("message too long") => {
                    Err(fail("message_too_large", &name, "UDP payload exceeds host or path limit"))
                }
                Err(_) if closed => Err(fail("already_closed", &name, "Socket is closed")),
                Err(error) => Err(host_failure(&name, "Cannot send UDP datagram", &error)),
            };
            sched::settle(&target, outcome);
        },
    );
    let weak = Rc::downgrade(state);
    promise.listen(move || {
        op.detach();
        if let Some(state) = weak.upgrade() {
            if let Ok(mut borrowed) = state.try_borrow_mut()
                && let Kind2::Udp(udp) = &mut borrowed.kind
            {
                udp.sending = None;
            }
        }
    });
    Value::Future(promise)
}

/// Разбор конверта v3: усечён, scope, адрес-конверт, данные.
fn packet(native: Native, value: &Value) -> Result<(bool, u32, Vec<u8>, Vec<u8>), Error> {
    let raw = bytes(value)?;
    let operation = native.symbol();
    let invalid = |message: &str| as_error(fail("invalid_data", operation, message));
    if raw.len() < 8 || raw[..3] != [79, 75, 3] || raw[3] > 1 {
        return Err(invalid("Invalid received packet envelope"));
    }
    let scope = u32::from_be_bytes([raw[4], raw[5], raw[6], raw[7]]);
    let at = 8;
    if raw.len() - at < 5 || raw[at..at + 3] != [79, 75, 2] {
        return Err(invalid("Invalid datagram envelope"));
    }
    let length = usize::from(u16::from_be_bytes([raw[at + 3], raw[at + 4]]));
    if length == 0 || raw.len() - at < 5 + length {
        return Err(invalid("Invalid datagram envelope"));
    }
    let address = raw[at + 5..at + 5 + length].to_vec();
    decode(native, &byte_array(address.clone()))?;
    let payload = raw[at + 5 + length..].to_vec();
    Ok((raw[3] == 1, scope, address, payload))
}

// ---- Unix ----

fn path_limit() -> usize {
    if cfg!(target_os = "linux") { 107 } else { 103 }
}

fn checked_path(path: &[u8], operation: &str) -> Result<(), Rc<Failure>> {
    if path.is_empty() || path.contains(&0) {
        return Err(fail("invalid_input", operation, "Unix socket path must be nonempty and contain no NUL"));
    }
    if path.len() > path_limit() {
        return Err(fail("invalid_input", operation, format!("Unix socket path exceeds {} bytes", path_limit())));
    }
    Ok(())
}

/// Абсолютный путь от виртуального cwd с проверкой длины (`Path.normalize`).
fn unix_path(value: &Value, operation: &str) -> Result<std::path::PathBuf, Error> {
    let raw = bytes(value)?;
    checked_path(&raw, operation).map_err(as_error)?;
    let path = crate::host::path(&String::from_utf8_lossy(&raw));
    let mut normalized = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {}
            other => normalized.push(other),
        }
    }
    checked_path(normalized.to_string_lossy().as_bytes(), operation).map_err(as_error)?;
    Ok(normalized)
}

fn identity(path: &std::path::Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    let meta = std::fs::symlink_metadata(path).ok()?;
    let other = !(meta.is_file() || meta.is_dir() || meta.file_type().is_symlink());
    let _ = meta.file_type().is_socket();
    other.then(|| (meta.dev(), meta.ino()))
}

#[derive(Debug)]
struct Pair {
    sockets: [SockRef; 2],
    released: bool,
    closed: bool,
}

// ---- DNS ----

fn dns_addresses(native: Native, host: &str, port: &crate::IntValue) -> Value {
    let operation = native.symbol().to_string();
    let valid = !host.is_empty() && host.len() <= 254 && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    if !valid {
        return resolved_failure(fail("invalid_input", &operation, "Invalid system resolver hostname"));
    }
    let Some(port) = port.to_i64().filter(|port| (0..=65535).contains(port)) else {
        return resolved_failure(fail("invalid_input", &operation, "Invalid system resolver port"));
    };
    let host = host.to_string();
    let promise = sched::promise();
    let target = promise.clone();
    let op = sched::offload(
        move || {
            use std::net::ToSocketAddrs;
            (host.as_str(), port as u16).to_socket_addrs().map(Iterator::collect::<Vec<_>>)
        },
        move |result: std::io::Result<Vec<SocketAddr>>| {
            let outcome = match result {
                Err(_) => Err(fail("not_found", &operation, "Network name was not found")),
                Ok(records) if records.len() > 256 => {
                    Err(fail("resource_exhausted", &operation, "System resolution answer exceeds 256 records"))
                }
                Ok(mut records) => {
                    // InetAddress.getAllByName JVM: IPv4 раньше IPv6.
                    records.sort_by_key(|record| u8::from(record.is_ipv6()));
                    let mut seen = std::collections::HashSet::new();
                    let mut out = Vec::new();
                    for record in records {
                        let family = if record.is_ipv4() { IPV4 } else { IPV6 };
                        let raw = address_bytes(&record);
                        let key = (format(&raw), record.port(), if let SocketAddr::V6(v6) = record { v6.scope_id() } else { 0 });
                        if seen.insert(key) {
                            out.push(endpoint_strings(&record, family));
                        }
                    }
                    if out.is_empty() {
                        Err(fail("not_found", &operation, "System name resolution returned no usable address"))
                    } else {
                        Ok(array(out))
                    }
                }
            };
            sched::settle(&target, outcome);
        },
    );
    promise.listen(move || op.detach());
    Value::Future(promise)
}

/// Обратный поиск с проверкой прямым (`getCanonicalHostName` JVM).
fn reverse_lookup(address: IpAddr) -> Option<String> {
    let socket = SockAddr::from(SocketAddr::new(address, 0));
    let mut host = vec![0 as libc::c_char; 1025];
    // SAFETY: адрес и буфер действительны на время вызова.
    let status = unsafe {
        libc::getnameinfo(
            socket.as_ptr() as *const libc::sockaddr,
            socket.len(),
            host.as_mut_ptr(),
            host.len() as libc::socklen_t,
            std::ptr::null_mut(),
            0,
            libc::NI_NAMEREQD,
        )
    };
    if status != 0 {
        return None;
    }
    // SAFETY: getnameinfo записал строку с завершающим нулём.
    let name = unsafe { std::ffi::CStr::from_ptr(host.as_ptr()) }.to_string_lossy().to_string();
    use std::net::ToSocketAddrs;
    let confirmed = (name.as_str(), 0).to_socket_addrs().ok()?.any(|forward| forward.ip() == address);
    (confirmed && name != address.to_string()).then_some(name)
}

fn dns_name(native: Native, source: &str) -> Value {
    let operation = native.symbol().to_string();
    let valid = !source.is_empty() && source.len() <= 45 && source.bytes().all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'.');
    if !valid {
        return resolved_failure(fail("invalid_input", &operation, "Reverse lookup requires a bounded numeric IP address"));
    }
    let mut parsed = parse_ipv4(source).map(|a| a.to_vec());
    if parsed.is_none() && !source.contains('.') {
        parsed = parse_ipv6(source).map(|a| a.to_vec());
    } else if parsed.is_none() && source.contains(':') {
        let dotted = source.rsplit(':').next().unwrap_or(source);
        parsed = parse_ipv4(dotted).and_then(|_| parse_ipv6(source)).map(|a| a.to_vec());
    }
    let Some(parsed) = parsed else {
        return resolved_failure(fail("invalid_input", &operation, "Reverse lookup requires a numeric IP address"));
    };
    let address = if parsed.len() == 4 {
        IpAddr::V4(Ipv4Addr::new(parsed[0], parsed[1], parsed[2], parsed[3]))
    } else {
        IpAddr::V6(Ipv6Addr::from(<[u8; 16]>::try_from(parsed.as_slice()).expect("sixteen")))
    };
    let promise = sched::promise();
    let target = promise.clone();
    let op = sched::offload(
        move || reverse_lookup(address),
        move |name: Option<String>| {
            let outcome = match name {
                Some(name) => Ok(text(name)),
                None => Err(fail("not_found", &operation, "Network name was not found")),
            };
            sched::settle(&target, outcome);
        },
    );
    promise.listen(move || op.detach());
    Value::Future(promise)
}

fn domain_of(family: u8) -> Domain {
    if family == IPV4 { Domain::IPV4 } else { Domain::IPV6 }
}

fn tcp_bound(native: Native, args: &[Value]) -> Result<Value, Error> {
    let operation = native.symbol();
    let (family, port, address) = decode(native, &args[0])?;
    let zone = string(&args[1])?;
    let (reuse, dual_stack) = (args[2].as_bool()?, args[3].as_bool()?);
    if family == IPV4 && dual_stack {
        return Err(as_error(fail("unsupported", operation, "Dual stack requires IPv6 listener")));
    }
    let local = socket_address(family, port, &address, zone, operation).map_err(as_error)?;
    let socket = Socket::new(domain_of(family), Type::STREAM, Some(Protocol::TCP))
        .map_err(|error| as_error(host_failure(operation, "Cannot create TCP socket", &error)))?;
    if family == IPV6 {
        // ServerSocketChannel JVM — всегда dual stack.
        let _ = socket.set_only_v6(false);
    }
    socket
        .set_reuse_address(reuse)
        .map_err(|error| as_error(host_failure(operation, "Cannot apply TCP listener address reuse", &error)))?;
    // JVM связывает и начинает слушать одним вызовом; `tcp_start_listening` разрешает `accept`.
    socket
        .bind(&SockAddr::from(local))
        .and_then(|()| socket.listen(128))
        .map_err(|error| as_error(host_failure(operation, "Cannot bind TCP listener", &error)))?;
    Ok(new_handle(Rc::new(RefCell::new(Listener {
        shared: Shared::new(socket),
        family,
        listening: false,
        closed: false,
        accepting: None,
        keep_alive: false,
        no_delay: false,
        worker: None,
        path: String::new(),
        identity: (0, 0),
        ready: VecDeque::new(),
    }))))
}

fn tcp_connect(native: Native, args: &[Value]) -> Result<Value, Error> {
    let operation = native.symbol().to_string();
    let (family, port, address) = match decode(native, &args[0]) {
        Ok(decoded) => decoded,
        Err(Error::Failure(failure)) => {
            return Ok(resolved_failure(Failure::new(Kind::Native(Rc::new(failure)))));
        }
        Err(other) => return Err(other),
    };
    if port == 0 {
        return Ok(resolved_failure(fail("invalid_input", &operation, "TCP destination port must be positive")));
    }
    let remote = match socket_address(family, port, &address, string(&args[1])?, &operation) {
        Ok(remote) => remote,
        Err(failure) => return Ok(resolved_failure(failure)),
    };
    let (keep_alive, no_delay) = (args[2].as_bool()?, args[3].as_bool()?);
    let promise = sched::promise();
    let target = promise.clone();
    let op = sched::offload(
        move || -> Result<Socket, (bool, std::io::Error)> {
            let socket = Socket::new(Domain::for_address(remote), Type::STREAM, Some(Protocol::TCP)).map_err(|e| (false, e))?;
            socket.set_nonblocking(true).map_err(|e| (false, e))?;
            match socket.connect(&SockAddr::from(remote)) {
                Ok(()) => {}
                Err(error) if error.raw_os_error() == Some(libc::EINPROGRESS) || error.kind() == std::io::ErrorKind::WouldBlock => {
                    let shared = Shared {
                        socket,
                        closed: AtomicBool::new(false),
                    };
                    shared.wait(libc::POLLOUT).map_err(|e| (false, e))?;
                    if let Some(error) = shared.socket.take_error().map_err(|e| (false, e))? {
                        return Err((false, error));
                    }
                    let socket = shared.socket;
                    socket.set_keepalive(keep_alive).map_err(|e| (true, e))?;
                    socket.set_nodelay(no_delay).map_err(|e| (true, e))?;
                    return Ok(socket);
                }
                Err(error) => return Err((false, error)),
            }
            socket.set_keepalive(keep_alive).map_err(|e| (true, e))?;
            socket.set_nodelay(no_delay).map_err(|e| (true, e))?;
            Ok(socket)
        },
        move |result| {
            let outcome = match result {
                Err((options, error)) => {
                    let message = if options { "Cannot apply TCP connection option" } else { "TCP connection failed" };
                    Err(host_failure(&operation, message, &error))
                }
                Ok(socket) => {
                    if !target.running() {
                        return;
                    }
                    Ok(new_handle(new_stream(socket, family, false)))
                }
            };
            sched::settle(&target, outcome);
        },
    );
    promise.listen(move || op.detach());
    Ok(Value::Future(promise))
}

fn udp_bound(native: Native, args: &[Value]) -> Result<Value, Error> {
    let operation = native.symbol();
    let (family, port, address) = decode(native, &args[0])?;
    let zone = string(&args[1])?;
    let (reuse, broadcast, dual) = (args[2].as_bool()?, args[3].as_bool()?, args[4].as_bool()?);
    if family == IPV4 && dual {
        return Err(as_error(fail("unsupported", operation, "Dual stack requires an IPv6 binding")));
    }
    if family == IPV6 && broadcast {
        return Err(as_error(fail("unsupported", operation, "IPv6 has no broadcast socket option")));
    }
    let local = socket_address(family, port, &address, zone, operation).map_err(as_error)?;
    let socket = Socket::new(domain_of(family), Type::DGRAM, Some(Protocol::UDP))
        .map_err(|error| as_error(host_failure(operation, "Cannot create UDP socket", &error)))?;
    if family == IPV6 {
        let _ = socket.set_only_v6(false);
    }
    socket
        .set_reuse_address(reuse)
        .and_then(|()| if family == IPV4 { socket.set_broadcast(broadcast) } else { Ok(()) })
        .map_err(|error| as_error(host_failure(operation, "Cannot apply UDP binding option", &error)))?;
    socket
        .bind(&SockAddr::from(local))
        .map_err(|error| as_error(host_failure(operation, "Cannot bind UDP socket", &error)))?;
    Ok(new_handle(Rc::new(RefCell::new(Sock {
        closed: false,
        kind: Kind2::Udp(Udp {
            shared: Shared::new(socket),
            family,
            dual_stack: dual,
            receiving: None,
            sending: None,
            peer: None,
            peer_maximum: 0,
            multicast_policy: None,
            memberships: Vec::new(),
        }),
    }))))
}

fn multicast_join(native: Native, args: &[Value]) -> Result<Value, Error> {
    let operation = native.symbol();
    let state = udp_of(&args[0], operation).map_err(as_error)?;
    let (family, port, group) = decode(native, &args[1])?;
    let name = string(&args[2])?;
    let (index, hops, loopback) = (int(&args[3])?, int(&args[4])?, args[5].as_bool()?);
    let invalid = |message: &str| as_error(fail("invalid_input", operation, message));
    let (socket_family, busy) = with_udp(&state, |udp| (udp.family, udp.peer.is_some() || udp.sending.is_some()));
    if busy {
        return Err(invalid("Multicast requires an unconnected UDP socket without a pending send"));
    }
    let Some(hops) = hops.to_i64().filter(|hops| (0..=255).contains(hops)) else {
        return Err(invalid("Multicast hop limit must be in 0..255"));
    };
    let first = group[0];
    if !(port == 0 && if family == IPV4 { first & 0xf0 == 0xe0 } else { first == 0xff }) {
        return Err(invalid("Address is not a multicast group"));
    }
    if family != socket_family {
        return Err(as_error(fail("unsupported", operation, "Multicast group must match socket IP family")));
    }
    let policy = format!("{index}:{hops}:{}", u8::from(loopback));
    let key = format(&group);
    let (conflict, joined) = with_udp(&state, |udp| {
        (
            udp.multicast_policy.as_ref().is_some_and(|known| *known != policy),
            udp.memberships.iter().any(|(known, _)| *known == key),
        )
    });
    if conflict {
        return Err(invalid("Multicast interface and options conflict with the socket policy"));
    }
    if joined {
        return Err(as_error(fail("already_exists", operation, "Multicast group is already joined on this socket")));
    }
    let index = index.to_i64().filter(|index| (1..=4_294_967_295).contains(index));
    let name_valid = !name.is_empty()
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'~' | b'-'))
        && !name.bytes().all(|b| b.is_ascii_digit());
    let (Some(index), true) = (index, name_valid) else {
        return Err(invalid("Invalid multicast interface identity"));
    };
    if i64::from(interface_index(name)) != index {
        return Err(invalid("Multicast interface name/index no longer identifies the same interface"));
    }
    let shared = with_udp(&state, |udp| udp.shared.clone());
    let fresh = with_udp(&state, |udp| udp.multicast_policy.is_none());
    let configure = || -> std::io::Result<()> {
        if family == IPV4 {
            shared.socket.set_multicast_ttl_v4(hops as u32)?;
            shared.socket.set_multicast_loop_v4(loopback)?;
        } else {
            shared.socket.set_multicast_if_v6(index as u32)?;
            shared.socket.set_multicast_hops_v6(hops as u32)?;
            shared.socket.set_multicast_loop_v6(loopback)?;
        }
        Ok(())
    };
    if fresh {
        configure().map_err(|error| as_error(host_failure(operation, "Cannot configure multicast socket", &error)))?;
    }
    let joined = if family == IPV4 {
        let ip = Ipv4Addr::new(group[0], group[1], group[2], group[3]);
        shared
            .socket
            .join_multicast_v4_n(&ip, &socket2::InterfaceIndexOrAddress::Index(index as u32))
    } else {
        let ip = Ipv6Addr::from(<[u8; 16]>::try_from(group.as_slice()).expect("sixteen"));
        shared.socket.join_multicast_v6(&ip, index as u32)
    };
    joined.map_err(|error| as_error(host_failure(operation, "Cannot join multicast group", &error)))?;
    let member = Rc::new(RefCell::new(Membership {
        socket: Rc::downgrade(&state),
        group,
        family,
        interface: index as u32,
        closed: false,
    }));
    with_udp(&state, |udp| {
        udp.multicast_policy = Some(policy);
        udp.memberships.push((key, member.clone()));
    });
    Ok(new_handle(member))
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let name = native.name();
    let operation = native.symbol();
    match name {
        "tcp_bound" => tcp_bound(native, args),
        "tcp_start_listening" => {
            let state = listener(&args[0], operation).map_err(as_error)?;
            let backlog = int(&args[1])?;
            if backlog.is_negative() || backlog.to_i64().is_none_or(|b| b > i64::from(i32::MAX)) {
                return Err(as_error(fail("invalid_input", operation, "TCP backlog must be nonnegative and fit a host int")));
            }
            if state.borrow().listening {
                return Err(as_error(fail("invalid_input", operation, "TCP listener is already listening")));
            }
            state.borrow_mut().listening = true;
            Ok(Value::Bool(true))
        }
        "tcp_listener_endpoint" => {
            let state = listener(&args[0], operation).map_err(as_error)?;
            let state = state.borrow();
            let local = state
                .shared
                .socket
                .local_addr()
                .map_err(|error| as_error(host_failure(operation, "Cannot inspect TCP endpoint", &error)))?;
            let local = sock_address(&local).ok_or_else(|| as_error(fail("unexpected", operation, "Cannot inspect TCP endpoint")))?;
            Ok(endpoint_strings(&local, state.family))
        }
        "tcp_accept" => {
            let state = match listener(&args[0], operation) {
                Ok(state) => state,
                Err(failure) => return Ok(resolved_failure(failure)),
            };
            if !state.borrow().listening {
                return Ok(resolved_failure(fail("invalid_input", operation, "TCP socket is not listening")));
            }
            Ok(accept(&state, args[1].as_bool()?, args[2].as_bool()?))
        }
        "tcp_listener_close" => {
            close_listener(handle::<ListenerRef>(&args[0])?);
            Ok(Value::Bool(true))
        }
        "tcp_listener_closed" | "unix_listener_closed" => Ok(Value::Bool(handle::<ListenerRef>(&args[0])?.borrow().closed)),
        "tcp_connect" => tcp_connect(native, args),
        "tcp_endpoint" => {
            let state = stream_of(&args[0], operation).map_err(as_error)?;
            let family = with_stream(&state, |stream| stream.family);
            if family != IPV4 && family != IPV6 {
                return Err(as_error(fail("invalid_input", operation, "Operation requires a TCP stream")));
            }
            let peer = args[1].as_bool()?;
            let shared = with_stream(&state, |stream| stream.shared.clone());
            let address = if peer { shared.socket.peer_addr() } else { shared.socket.local_addr() };
            let address = address.map_err(|error| {
                if error.raw_os_error() == Some(libc::ENOTCONN) {
                    as_error(fail("not_connected", operation, "Cannot inspect TCP endpoint"))
                } else {
                    as_error(host_failure(operation, "Cannot inspect TCP endpoint", &error))
                }
            })?;
            let address = sock_address(&address).ok_or_else(|| as_error(fail("not_connected", operation, "Cannot inspect TCP endpoint")))?;
            Ok(endpoint_strings(&address, family))
        }
        "stream_read" => Ok(stream_read(&args[0], int(&args[1])?)),
        "stream_write" => Ok(stream_write(&args[0], bytes(&args[1])?)),
        "stream_shutdown" => {
            let state = sock(&args[0])?;
            if state.borrow().closed {
                return Ok(Value::Bool(true));
            }
            let state = stream_of(&args[0], operation).map_err(as_error)?;
            let output = args[1].as_bool()?;
            let (done, writing, shared) = with_stream(&state, |stream| {
                (
                    if output { stream.output_closed } else { stream.input_closed },
                    stream.writing.is_some(),
                    stream.shared.clone(),
                )
            });
            if done {
                return Ok(Value::Bool(true));
            }
            if output && writing {
                return Err(as_error(fail("resource_exhausted", operation, "Output shutdown requires a drained writer")));
            }
            let how = if output { std::net::Shutdown::Write } else { std::net::Shutdown::Read };
            shared
                .socket
                .shutdown(how)
                .map_err(|error| as_error(host_failure(operation, "Cannot shut down stream half", &error)))?;
            let reading = with_stream(&state, |stream| {
                if output {
                    stream.output_closed = true;
                    None
                } else {
                    stream.input_closed = true;
                    stream.reading.clone()
                }
            });
            if let Some(reading) = reading {
                sched::settle(&reading, Err(fail("already_closed", &symbol("stream_read", 2), "Stream input is closed")));
            }
            Ok(Value::Bool(true))
        }
        "stream_half_closed" => {
            let state = sock(&args[0])?;
            if state.borrow().closed {
                return Ok(Value::Bool(true));
            }
            let state = stream_of(&args[0], operation).map_err(as_error)?;
            let output = args[1].as_bool()?;
            Ok(Value::Bool(with_stream(&state, |stream| {
                if output { stream.output_closed } else { stream.input_closed }
            })))
        }
        "close" => {
            // Как таблица дескрипторов Perl: повторное закрытие — не ошибка.
            let state = sock(&args[0])?;
            if !state.borrow().closed {
                shut_now(&state);
            }
            Ok(Value::Bool(true))
        }
        "closed" => Ok(Value::Bool(sock(&args[0])?.borrow().closed)),
        "socket" => {
            let family = match int(&args[0])?.to_i64() {
                Some(1) => IPV4,
                Some(2) => IPV6,
                _ => return Err(as_error(fail("unsupported", operation, "IP address family is not supported"))),
            };
            let kind = int(&args[1])?.to_i64().filter(|k| (0..=2).contains(k));
            let Some(kind) = kind else {
                return Err(as_error(fail("invalid_input", operation, "Unknown portable socket type")));
            };
            let protocol = int(&args[2])?.to_i64().unwrap_or(-1);
            if !(protocol == 0 || (kind == 1 && protocol == 1) || (kind == 2 && protocol == 2)) {
                return Err(as_error(fail("invalid_input", operation, "Socket protocol does not match its type")));
            }
            let created = if kind == 2 {
                Socket::new(domain_of(family), Type::DGRAM, Some(Protocol::UDP))
            } else {
                Socket::new(domain_of(family), Type::STREAM, Some(Protocol::TCP))
            };
            let socket = created.map_err(|error| as_error(host_failure(operation, "Cannot create socket", &error)))?;
            if kind == 2 {
                return Ok(new_handle(Rc::new(RefCell::new(Sock {
                    closed: false,
                    kind: Kind2::Udp(Udp {
                        shared: Shared::new(socket),
                        family,
                        dual_stack: false,
                        receiving: None,
                        sending: None,
                        peer: None,
                        peer_maximum: 0,
                        multicast_policy: None,
                        memberships: Vec::new(),
                    }),
                }))));
            }
            Ok(new_handle(new_stream(socket, family, false)))
        }
        "udp_bound" => udp_bound(native, args),
        "local_endpoint" => {
            let state = sock(&args[0])?;
            let state = state.borrow();
            if state.closed {
                return Err(as_error(fail("already_closed", operation, "Native handle is already closed")));
            }
            let (family, shared) = match &state.kind {
                Kind2::Stream(stream) => (stream.family, stream.shared.clone()),
                Kind2::Udp(udp) => (udp.family, udp.shared.clone()),
            };
            if family != IPV4 && family != IPV6 {
                return Err(as_error(fail("invalid_input", operation, "Operation requires an IP socket")));
            }
            let local = shared.socket.local_addr().ok().and_then(|address| sock_address(&address));
            let local = local.unwrap_or_else(|| {
                if family == IPV4 {
                    SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))
                } else {
                    SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0))
                }
            });
            Ok(endpoint_strings(&local, family))
        }
        "udp_receive" | "udp_connected_receive" => {
            let limit = int(&args[1])?;
            if limit.signum() <= 0 {
                return Ok(resolved_failure(fail("invalid_input", operation, "UDP receive limit must be positive")));
            }
            let state = match udp_of(&args[0], operation) {
                Ok(state) => state,
                Err(failure) => return Ok(resolved_failure(failure)),
            };
            let connected = with_udp(&state, |udp| udp.peer.is_some());
            if name == "udp_receive" && connected {
                return Ok(resolved_failure(fail(
                    "invalid_input",
                    operation,
                    "Unconnected receive requires an unconnected UDP socket",
                )));
            }
            if name == "udp_connected_receive" && !connected {
                return Ok(resolved_failure(fail("not_connected", operation, "UDP socket is not connected")));
            }
            Ok(receive(&state, limit, operation))
        }
        "udp_send" => {
            let attempt = || -> Result<Value, Rc<Failure>> {
                let (family, port, address) = decode(native, &args[0 + 1]).map_err(|error| match error {
                    Error::Failure(failure) => Failure::new(Kind::Native(Rc::new(failure))),
                    other => Failure::new(Kind::Program(format!("{other:?}"))),
                })?;
                if port == 0 {
                    return Err(fail("invalid_input", operation, "UDP destination port must be positive"));
                }
                let state = udp_of(&args[0], operation)?;
                if with_udp(&state, |udp| udp.peer.is_some()) {
                    return Err(fail("invalid_input", operation, "Unconnected send requires an unconnected UDP socket"));
                }
                let payload = bytes(&args[3]).map_err(|_| fail("invalid_input", operation, "Invalid payload"))?;
                let ipv4 = family == IPV4 || ipv4_mapped(family, &address);
                if payload.len() > if ipv4 { 65507 } else { 65527 } {
                    return Err(fail("message_too_large", operation, "UDP payload exceeds IP version limit"));
                }
                let zone = string(&args[2]).map_err(|_| fail("invalid_input", operation, "Invalid zone"))?;
                let target = with_udp(&state, |udp| destination(udp, family, port, &address, zone, operation))?;
                Ok(send(&state, payload, Some(target), operation))
            };
            Ok(attempt().unwrap_or_else(resolved_failure))
        }
        "udp_connected_send" => {
            let attempt = || -> Result<Value, Rc<Failure>> {
                let state = udp_of(&args[0], operation)?;
                let (peer, maximum) = with_udp(&state, |udp| (udp.peer.is_some(), udp.peer_maximum));
                if !peer {
                    return Err(fail("not_connected", operation, "UDP socket is not connected"));
                }
                let payload = bytes(&args[1]).map_err(|_| fail("invalid_input", operation, "Invalid payload"))?;
                if payload.len() > maximum {
                    return Err(fail("message_too_large", operation, "UDP payload exceeds IP version limit"));
                }
                Ok(send(&state, payload, None, operation))
            };
            Ok(attempt().unwrap_or_else(resolved_failure))
        }
        "udp_connect" => {
            let attempt = || -> Result<Value, Rc<Failure>> {
                let state = udp_of(&args[0], operation)?;
                let idle = with_udp(&state, |udp| {
                    udp.peer.is_none() && udp.sending.is_none() && udp.receiving.is_none() && udp.multicast_policy.is_none()
                });
                if !idle {
                    return Err(fail("invalid_input", operation, "Connect requires an idle unconnected UDP socket"));
                }
                let (family, port, address) = decode(native, &args[1]).map_err(|error| match error {
                    Error::Failure(failure) => Failure::new(Kind::Native(Rc::new(failure))),
                    other => Failure::new(Kind::Program(format!("{other:?}"))),
                })?;
                if port == 0 {
                    return Err(fail("invalid_input", operation, "UDP destination port must be positive"));
                }
                let zone = string(&args[2]).map_err(|_| fail("invalid_input", operation, "Invalid zone"))?;
                let target = with_udp(&state, |udp| destination(udp, family, port, &address, zone, operation))?;
                let shared = with_udp(&state, |udp| udp.shared.clone());
                if let Err(error) = shared.socket.connect(&SockAddr::from(target)) {
                    shut_now(&state);
                    return Err(host_failure(operation, "Cannot connect UDP socket", &error));
                }
                let peer = shared.socket.peer_addr().ok().and_then(|peer| sock_address(&peer)).unwrap_or(target);
                let raw = address_bytes(&peer);
                let peer_family = if raw.len() == 4 { IPV4 } else { IPV6 };
                with_udp(&state, |udp| {
                    udp.peer = Some(peer);
                    udp.peer_maximum = if peer_family == IPV4 || ipv4_mapped(peer_family, &raw) { 65507 } else { 65527 };
                });
                Ok(Value::Future(sched::resolved(Ok(Value::Bool(true)))))
            };
            Ok(attempt().unwrap_or_else(resolved_failure))
        }
        "peer_endpoint" => {
            let state = udp_of(&args[0], operation).map_err(as_error)?;
            let peer = with_udp(&state, |udp| udp.peer);
            let Some(peer) = peer else {
                return Err(as_error(fail("not_connected", operation, "UDP socket is not connected")));
            };
            let family = if peer.is_ipv4() { IPV4 } else { IPV6 };
            Ok(endpoint_strings(&peer, family))
        }
        "packet_bytes" => Ok(byte_array(packet(native, &args[0])?.3)),
        "packet_truncated" => Ok(Value::Bool(packet(native, &args[0])?.0)),
        "packet_source" => {
            let (_, scope, address, _) = packet(native, &args[0])?;
            let (_, port, raw) = decode(native, &byte_array(address))?;
            Ok(string_array([
                format(&raw),
                port.to_string(),
                if scope != 0 { scope.to_string() } else { String::new() },
            ]))
        }
        "multicast_join" => multicast_join(native, args),
        "multicast_closed" => Ok(Value::Bool(handle::<Rc<RefCell<Membership>>>(&args[0])?.borrow().closed)),
        "multicast_close" => {
            let member = handle::<Rc<RefCell<Membership>>>(&args[0])?.clone();
            if member.borrow().closed {
                return Ok(Value::Bool(true));
            }
            member.borrow_mut().closed = true;
            let socket = member.borrow().socket.upgrade();
            if let Some(socket) = socket {
                if !socket.borrow().closed {
                    let key = format(&member.borrow().group);
                    let (group, family, interface) = {
                        let member = member.borrow();
                        (member.group.clone(), member.family, member.interface)
                    };
                    with_udp(&socket, |udp| {
                        udp.memberships.retain(|(known, _)| *known != key);
                        if family == IPV4 {
                            let ip = Ipv4Addr::new(group[0], group[1], group[2], group[3]);
                            let _ = udp
                                .shared
                                .socket
                                .leave_multicast_v4_n(&ip, &socket2::InterfaceIndexOrAddress::Index(interface));
                        } else {
                            let ip = Ipv6Addr::from(<[u8; 16]>::try_from(group.as_slice()).expect("sixteen"));
                            let _ = udp.shared.socket.leave_multicast_v6(&ip, interface);
                        }
                    });
                }
            }
            Ok(Value::Bool(true))
        }
        "dns_addresses" => Ok(dns_addresses(native, string(&args[0])?, int(&args[1])?)),
        "dns_name" => Ok(dns_name(native, string(&args[0])?)),
        // ---- Unix ----
        "unix_path_valid" => {
            checked_path(&bytes(&args[0])?, operation).map_err(as_error)?;
            Ok(Value::Bool(true))
        }
        "unix_pair" => {
            let (left, right) = Socket::pair(Domain::UNIX, Type::STREAM, None)
                .map_err(|error| as_error(host_failure(operation, "Cannot create Unix socket pair", &error)))?;
            Ok(new_handle(Rc::new(RefCell::new(Pair {
                sockets: [new_stream(left, UNIX, true), new_stream(right, UNIX, true)],
                released: false,
                closed: false,
            }))))
        }
        "unix_pair_socket" | "unix_pair_release" => {
            let pair = handle::<Rc<RefCell<Pair>>>(&args[0])?.clone();
            if pair.borrow().closed || pair.borrow().released {
                return Err(as_error(fail("already_closed", operation, "Native handle is already closed")));
            }
            if name == "unix_pair_release" {
                pair.borrow_mut().released = true;
                return Ok(Value::Bool(true));
            }
            let second = args[1].as_bool()?;
            Ok(new_handle(pair.borrow().sockets[usize::from(second)].clone()))
        }
        "unix_pair_close" => {
            let pair = handle::<Rc<RefCell<Pair>>>(&args[0])?.clone();
            if pair.borrow().closed || pair.borrow().released {
                return Ok(Value::Bool(true));
            }
            pair.borrow_mut().closed = true;
            for socket in pair.borrow().sockets.iter() {
                shut_now(socket);
            }
            Ok(Value::Bool(true))
        }
        "unix_endpoint" => {
            let state = stream_of(&args[0], operation).map_err(as_error)?;
            let (family, unnamed, shared) = with_stream(&state, |stream| (stream.family, stream.unnamed, stream.shared.clone()));
            if family != UNIX {
                return Err(as_error(fail("invalid_input", operation, "Operation requires a Unix stream connection")));
            }
            if unnamed {
                return Ok(byte_array([]));
            }
            let peer = args[1].as_bool()?;
            let address = if peer { shared.socket.peer_addr() } else { shared.socket.local_addr() };
            let address = address.map_err(|error| as_error(host_failure(operation, "Cannot inspect Unix endpoint", &error)))?;
            let path = address.as_pathname().map(|path| path.to_string_lossy().to_string()).unwrap_or_default();
            Ok(byte_array(path.into_bytes()))
        }
        "unix_connect" => {
            let path = match unix_path(&args[0], operation) {
                Ok(path) => path,
                Err(Error::Failure(failure)) => return Ok(resolved_failure(Failure::new(Kind::Native(Rc::new(failure))))),
                Err(other) => return Err(other),
            };
            let operation = operation.to_string();
            let promise = sched::promise();
            let target = promise.clone();
            let op = sched::offload(
                move || -> std::io::Result<Socket> {
                    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
                    socket.connect(&SockAddr::unix(&path)?)?;
                    Ok(socket)
                },
                move |result| {
                    let outcome = match result {
                        Err(error) => Err(host_failure(&operation, "Cannot connect Unix socket", &error)),
                        Ok(socket) => {
                            if !target.running() {
                                return;
                            }
                            Ok(new_handle(new_stream(socket, UNIX, false)))
                        }
                    };
                    sched::settle(&target, outcome);
                },
            );
            promise.listen(move || op.detach());
            Ok(Value::Future(promise))
        }
        "unix_listen" => {
            let backlog = int(&args[1])?;
            let Some(backlog) = backlog.to_i64().filter(|b| (0..=i64::from(i32::MAX)).contains(b)) else {
                return Err(as_error(fail("invalid_input", operation, "Unix backlog must be nonnegative and fit a host int")));
            };
            let path = unix_path(&args[0], operation)?;
            let socket = Socket::new(Domain::UNIX, Type::STREAM, None)
                .map_err(|error| as_error(host_failure(operation, "Cannot create Unix stream socket", &error)))?;
            SockAddr::unix(&path)
                .and_then(|address| socket.bind(&address))
                .and_then(|()| socket.listen(backlog as i32))
                .map_err(|error| as_error(host_failure(operation, "Cannot bind Unix listener", &error)))?;
            let Some(identity) = identity(&path) else {
                return Err(as_error(fail("unexpected", operation, "Cannot capture Unix listener entry identity")));
            };
            Ok(new_handle(Rc::new(RefCell::new(Listener {
                shared: Shared::new(socket),
                family: UNIX,
                listening: true,
                closed: false,
                accepting: None,
                keep_alive: false,
                no_delay: false,
                worker: None,
                path: path.to_string_lossy().to_string(),
                identity,
                ready: VecDeque::new(),
            }))))
        }
        "unix_listener_path" => {
            let state = listener(&args[0], operation).map_err(as_error)?;
            Ok(byte_array(state.borrow().path.clone().into_bytes()))
        }
        "unix_accept" => match listener(&args[0], operation) {
            Ok(state) => Ok(accept(&state, false, false)),
            Err(failure) => Ok(resolved_failure(failure)),
        },
        "unix_listener_close" => {
            let state = handle::<ListenerRef>(&args[0])?.clone();
            if !close_listener(&state) {
                return Ok(Value::Bool(true));
            }
            // Удаляем запись, только если это всё ещё наш сокет.
            let (path, known) = {
                let state = state.borrow();
                (crate::host::path(&state.path), state.identity)
            };
            if identity(&path) == Some(known) {
                if let Err(error) = std::fs::remove_file(&path) {
                    if error.kind() != std::io::ErrorKind::NotFound {
                        return Err(as_error(host_failure(operation, "Cannot remove Unix listener entry", &error)));
                    }
                }
            }
            Ok(Value::Bool(true))
        }
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}
