//! TCP ownership is native Rust state; Tokio supplies readiness, connect and
//! accept. Closing a shared state wakes pending operations before dropping it.
use crate::{
    Error,
    handle::Handle,
    native::{Outcome, ProviderError},
    native_io::Output,
    value::*,
};
use okkama_runtime_abi::{Native, NativeFailure};
use socket2::{Domain, Protocol, SockAddr, SockRef, Socket, Type};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6},
    rc::Rc,
    task::Poll,
};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
#[derive(Debug)]
pub(crate) enum Transport {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(tokio::net::UnixStream),
}
impl From<TcpStream> for Transport {
    fn from(s: TcpStream) -> Self {
        Self::Tcp(s)
    }
}
#[cfg(unix)]
impl From<tokio::net::UnixStream> for Transport {
    fn from(s: tokio::net::UnixStream) -> Self {
        Self::Unix(s)
    }
}
impl Transport {
    fn label(&self) -> &'static str {
        match self {
            Self::Tcp(_) => "TCP socket",
            #[cfg(unix)]
            Self::Unix(_) => "Unix socket",
        }
    }

    async fn readable(&self) -> std::io::Result<()> {
        match self {
            Self::Tcp(s) => s.readable().await,
            #[cfg(unix)]
            Self::Unix(s) => s.readable().await,
        }
    }
    async fn writable(&self) -> std::io::Result<()> {
        match self {
            Self::Tcp(s) => s.writable().await,
            #[cfg(unix)]
            Self::Unix(s) => s.writable().await,
        }
    }
    fn try_read(&self, bytes: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(s) => s.try_read(bytes),
            #[cfg(unix)]
            Self::Unix(s) => s.try_read(bytes),
        }
    }
    fn try_write(&self, bytes: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(s) => s.try_write(bytes),
            #[cfg(unix)]
            Self::Unix(s) => s.try_write(bytes),
        }
    }
    fn shutdown(&self, how: std::net::Shutdown) -> std::io::Result<()> {
        match self {
            Self::Tcp(s) => SockRef::from(s).shutdown(how),
            #[cfg(unix)]
            Self::Unix(s) => SockRef::from(s).shutdown(how),
        }
    }
}
#[derive(Debug)]
pub(crate) struct Stream {
    socket: RefCell<Option<Rc<Transport>>>,
    changed: tokio::sync::watch::Sender<()>,
    read_changed: tokio::sync::watch::Sender<()>,
    input_closed: Cell<bool>,
    output_closed: Cell<bool>,
    reading: Rc<Cell<bool>>,
    writing: Rc<Cell<bool>>,
}
impl Stream {
    pub(crate) fn new(socket: impl Into<Transport>) -> Self {
        Self {
            socket: RefCell::new(Some(Rc::new(socket.into()))),
            changed: tokio::sync::watch::channel(()).0,
            read_changed: tokio::sync::watch::channel(()).0,
            input_closed: Cell::new(false),
            output_closed: Cell::new(false),
            reading: Rc::new(Cell::new(false)),
            writing: Rc::new(Cell::new(false)),
        }
    }
    pub(crate) fn closed(&self) -> bool {
        self.socket.borrow().is_none()
    }
    pub(crate) fn close(&self) {
        self.socket.borrow_mut().take();
        self.changed.send_replace(());
        self.read_changed.send_replace(());
    }
    pub(crate) fn socket(&self, n: Native) -> Result<Rc<Transport>, NativeFailure> {
        self.socket
            .borrow()
            .clone()
            .ok_or_else(|| failure(n, "already_closed", "Socket is closed"))
    }
}
#[derive(Debug)]
pub(crate) struct Listener {
    socket: RefCell<Option<Rc<TcpListener>>>,
    changed: tokio::sync::watch::Sender<()>,
    listening: Cell<bool>,
    accepting: Rc<Cell<bool>>,
}
impl Listener {
    fn socket(&self, n: Native) -> Result<Rc<TcpListener>, NativeFailure> {
        self.socket
            .borrow()
            .clone()
            .ok_or_else(|| failure(n, "already_closed", "Socket is closed"))
    }
    fn close(&self) {
        self.socket.borrow_mut().take();
        self.changed.send_replace(());
    }
}
#[derive(Debug)]
pub(crate) enum NetworkHandle {
    Stream(Rc<Stream>),
    Udp(Rc<crate::native_udp::Udp>),
    Membership(Rc<crate::native_multicast::Membership>),
    Listener(Rc<Listener>),
    #[cfg(unix)]
    Unix(crate::native_unix::UnixHandle),
}
impl Output for NetworkHandle {
    fn publish(self: Box<Self>, heap: &mut Heap) -> Result<Slot, Error> {
        heap.network_new(*self)
    }
}
impl Output for Vec<u8> {
    fn publish(self: Box<Self>, heap: &mut Heap) -> Result<Slot, Error> {
        heap.native_byte_array(*self)
    }
}
impl Output for bool {
    fn publish(self: Box<Self>, _: &mut Heap) -> Result<Slot, Error> {
        Ok(Slot::boolean(*self))
    }
}
pub(crate) struct Busy(Rc<Cell<bool>>);
impl Busy {
    pub(crate) fn acquire(
        flag: &Rc<Cell<bool>>,
        n: Native,
        message: &str,
    ) -> Result<Self, NativeFailure> {
        if flag.replace(true) {
            return Err(failure(n, "resource_exhausted", message));
        }
        Ok(Self(flag.clone()))
    }
}
impl Drop for Busy {
    fn drop(&mut self) {
        self.0.set(false);
    }
}
struct WriteGuard {
    state: Rc<Stream>,
    complete: bool,
}
impl Drop for WriteGuard {
    fn drop(&mut self) {
        if !self.complete {
            self.state.close();
        }
    }
}
pub(crate) fn handles(n: Native) -> bool {
    use Native::*;
    matches!(
        n,
        NetworkTcpConnect
            | NetworkTcpBound
            | NetworkTcpStartListening
            | NetworkTcpListenerEndpoint
            | NetworkTcpEndpoint
            | NetworkTcpAccept
            | NetworkTcpListenerClose
            | NetworkTcpListenerClosed
            | NetworkStreamRead
            | NetworkStreamWrite
            | NetworkStreamShutdown
            | NetworkStreamHalfClosed
            | NetworkClose
            | NetworkClosed
    )
}
pub(crate) fn failure(n: Native, code: &str, message: &str) -> NativeFailure {
    NativeFailure {
        domain: "core.net".into(),
        code: code.into(),
        operation: n.symbol().into(),
        message: message.into(),
    }
}
pub(crate) fn host(n: Native, message: &str, error: std::io::Error) -> NativeFailure {
    use std::io::ErrorKind::*;
    let raw = match error.raw_os_error() {
        Some(libc::EMFILE) | Some(libc::ENFILE) => Some("resource_exhausted"),
        Some(libc::EHOSTUNREACH) | Some(libc::ENETUNREACH) => Some("network_unreachable"),
        _ => None,
    };
    let code = raw.unwrap_or(match error.kind() {
        AddrInUse => "address_in_use",
        AddrNotAvailable => "address_not_available",
        ConnectionRefused => "connection_refused",
        NetworkUnreachable | HostUnreachable => "network_unreachable",
        TimedOut => "timed_out",
        ConnectionReset | ConnectionAborted => "connection_reset",
        BrokenPipe => "broken_pipe",
        NotConnected => "not_connected",
        PermissionDenied => "permission_denied",
        OutOfMemory => "resource_exhausted",
        _ => "unexpected",
    });
    failure(n, code, message)
}
pub(crate) fn address(n: Native, bytes: &[u8], zone: &str) -> Result<SocketAddr, NativeFailure> {
    if bytes.len() < 6
        || bytes[..3] != [79, 75, 1]
        || !matches!((bytes[3], bytes.len()), (1, 10) | (2, 22))
    {
        return Err(failure(
            n,
            "invalid_data",
            "Invalid network address envelope",
        ));
    }
    let port = u16::from_be_bytes([bytes[4], bytes[5]]);
    if bytes[3] == 1 {
        if !zone.is_empty() {
            return Err(failure(n, "invalid_input", "Invalid IPv6 binding zone"));
        }
        return Ok(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(bytes[6], bytes[7], bytes[8], bytes[9])),
            port,
        ));
    }
    let ip = Ipv6Addr::from(<[u8; 16]>::try_from(&bytes[6..]).expect("validated bytes"));
    if zone.is_empty() {
        return Ok(SocketAddr::V6(SocketAddrV6::new(ip, port, 0, 0)));
    }
    if !zone
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'~' | b'-'))
    {
        return Err(failure(n, "invalid_input", "Invalid IPv6 binding zone"));
    }
    if zone.bytes().all(|b| b.is_ascii_digit()) {
        let digits = zone.trim_start_matches('0');
        let scope = if digits.is_empty() {
            0
        } else {
            digits
                .parse::<u32>()
                .map_err(|_| failure(n, "invalid_input", "IPv6 scope index exceeds uint32"))?
        };
        return Ok(SocketAddr::V6(SocketAddrV6::new(ip, port, 0, scope)));
    }
    // Safe numeric-only getaddrinfo resolves interface scope, never a hostname.
    let name = format!("{ip}%{zone}");
    let hints = dns_lookup::AddrInfoHints {
        flags: libc::AI_NUMERICHOST | libc::AI_NUMERICSERV,
        address: libc::AF_INET6,
        socktype: libc::SOCK_STREAM,
        protocol: libc::IPPROTO_TCP,
    };
    dns_lookup::getaddrinfo(Some(&name), Some(&port.to_string()), Some(hints))
        .ok()
        .and_then(|mut records| records.find_map(|r| r.ok().map(|r| r.sockaddr)))
        .ok_or_else(|| {
            failure(
                n,
                "invalid_input",
                "Unknown or unsupported IPv6 interface zone",
            )
        })
}
pub(crate) async fn interruptible<F, T>(
    mut changed: tokio::sync::watch::Receiver<()>,
    n: Native,
    body: F,
) -> Result<T, NativeFailure>
where
    F: Future<Output = Result<T, NativeFailure>>,
{
    let mut body = std::pin::pin!(body);
    let mut closed = std::pin::pin!(changed.changed());
    std::future::poll_fn(|cx| {
        if closed.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Err(failure(n, "already_closed", "Socket is closed")));
        }
        body.as_mut().poll(cx)
    })
    .await
}
fn options(stream: &TcpStream, keep: bool, delay: bool, n: Native) -> Result<(), NativeFailure> {
    SockRef::from(stream)
        .set_keepalive(keep)
        .and_then(|()| stream.set_nodelay(delay))
        .map_err(|e| host(n, "Cannot apply TCP connection option", e))
}
impl Heap {
    pub(crate) fn network_new(&mut self, state: NetworkHandle) -> Result<Slot, Error> {
        Ok(Slot {
            tag: HANDLE,
            bits: self.handles.allocate(Handle::Network(state))?,
        })
    }
    pub(crate) fn network_handle(&self, slot: Slot) -> Result<&NetworkHandle, Error> {
        if slot.tag != HANDLE {
            return Err(Error("expected network handle".into()));
        }
        match self.handles.get(slot.bits)? {
            Handle::Network(state) => Ok(state),
            _ => Err(Error("expected network handle".into())),
        }
    }
    pub(crate) fn network_stream(&self, slot: Slot) -> Result<Rc<Stream>, Error> {
        match self.network_handle(slot)? {
            NetworkHandle::Stream(state) => Ok(state.clone()),
            _ => Err(Error("expected socket handle".into())),
        }
    }
    fn network_listener(&self, slot: Slot) -> Result<Rc<Listener>, Error> {
        match self.network_handle(slot)? {
            NetworkHandle::Listener(state) => Ok(state.clone()),
            _ => Err(Error("expected listener handle".into())),
        }
    }
    pub(crate) fn network_endpoint(&mut self, endpoint: SocketAddr) -> Result<Slot, Error> {
        let (ip, scope) = match endpoint {
            SocketAddr::V4(v) => (crate::native_net::format(&v.ip().octets()), String::new()),
            SocketAddr::V6(v) => (
                crate::native_net::format(&v.ip().octets()),
                if v.scope_id() == 0 {
                    String::new()
                } else {
                    v.scope_id().to_string()
                },
            ),
        };
        let values = [ip, endpoint.port().to_string(), scope]
            .into_iter()
            .map(|s| self.allocate_text(s))
            .collect::<Result<Vec<_>, _>>()?;
        self.allocate_array(values)
    }
    pub(crate) fn native_network(&mut self, n: Native, a: &[Slot]) -> Result<Outcome, Error> {
        if matches!(n, Native::NetworkClose | Native::NetworkClosed) {
            if let NetworkHandle::Udp(state) = self.network_handle(a[0])? {
                if n == Native::NetworkClose {
                    state.close();
                    return Ok(Outcome::Value(Slot::boolean(true)));
                }
                return Ok(Outcome::Value(Slot::boolean(state.closed())));
            }
        }
        match self.network_execute(n, a) {
            Ok(v) => Ok(Outcome::Value(v)),
            Err(ProviderError::Engine(e)) => Err(e),
            Err(ProviderError::Native(e)) => {
                let payload = self.native_failure_new(e)?;
                let failure = self.failure_new(crate::failure::FailureKind::Native(payload))?;
                if n.signature().is_async {
                    self.future_with_state(crate::future::State::Ready(Err(failure)))
                        .map(Outcome::Value)
                } else {
                    Ok(Outcome::Failure(failure))
                }
            }
        }
    }
    fn network_execute(&mut self, n: Native, a: &[Slot]) -> Result<Slot, ProviderError> {
        use Native::*;
        let invalid = |code, message| ProviderError::Native(failure(n, code, message));
        let native = ProviderError::Native;
        match n {
            NetworkTcpConnect | NetworkTcpBound => {
                let endpoint =
                    address(n, &self.native_bytes(a[0])?, self.text(a[1])?).map_err(native)?;
                let (first, second) = (a[2].bits != 0, a[3].bits != 0);
                if n == NetworkTcpConnect {
                    if endpoint.port() == 0 {
                        return Err(invalid(
                            "invalid_input",
                            "TCP destination port must be positive",
                        ));
                    }
                    return Ok(self.native_io(async move {
                        let socket = if endpoint.is_ipv4() {
                            TcpSocket::new_v4()
                        } else {
                            TcpSocket::new_v6()
                        }
                        .map_err(|e| host(n, "Cannot create TCP socket", e))?;
                        let stream = socket
                            .connect(endpoint)
                            .await
                            .map_err(|e| host(n, "TCP connection failed", e))?;
                        options(&stream, first, second, n)?;
                        Ok(NetworkHandle::Stream(Rc::new(Stream::new(stream))))
                    })?);
                }
                if endpoint.is_ipv4() && second {
                    return Err(invalid("unsupported", "Dual stack requires IPv6 listener"));
                }
                let socket = Socket::new(
                    Domain::for_address(endpoint),
                    Type::STREAM,
                    Some(Protocol::TCP),
                )
                .map_err(|e| native(host(n, "Cannot create TCP socket", e)))?;
                if endpoint.is_ipv6() {
                    let _ = socket.set_only_v6(false);
                }
                socket
                    .set_reuse_address(first)
                    .map_err(|e| native(host(n, "Cannot apply TCP listener address reuse", e)))?;
                socket
                    .set_nonblocking(true)
                    .and_then(|()| socket.bind(&SockAddr::from(endpoint)))
                    .and_then(|()| socket.listen(128))
                    .map_err(|e| native(host(n, "Cannot bind TCP listener", e)))?;
                if tokio::runtime::Handle::try_current().is_err() {
                    return Err(Error("TCP listener requires a Tokio runtime".into()).into());
                }
                let socket = TcpListener::from_std(socket.into())
                    .map_err(|e| native(host(n, "Cannot register TCP listener", e)))?;
                Ok(self.network_new(NetworkHandle::Listener(Rc::new(Listener {
                    socket: RefCell::new(Some(Rc::new(socket))),
                    changed: tokio::sync::watch::channel(()).0,
                    listening: Cell::new(false),
                    accepting: Rc::new(Cell::new(false)),
                })))?)
            }
            NetworkTcpListenerClose
            | NetworkTcpListenerClosed
            | NetworkTcpListenerEndpoint
            | NetworkTcpStartListening
            | NetworkTcpAccept => {
                let state = self.network_listener(a[0])?;
                if n == NetworkTcpListenerClose {
                    state.close();
                    return Ok(Slot::boolean(true));
                }
                if n == NetworkTcpListenerClosed {
                    return Ok(Slot::boolean(state.socket.borrow().is_none()));
                }
                let socket = state.socket(n).map_err(native)?;
                match n {
                    NetworkTcpListenerEndpoint => Ok(self.network_endpoint(
                        socket
                            .local_addr()
                            .map_err(|e| native(host(n, "Cannot inspect TCP endpoint", e)))?,
                    )?),
                    NetworkTcpStartListening => {
                        if self
                            .integer(a[1])?
                            .to_i64()
                            .filter(|v| (0..=i32::MAX as i64).contains(v))
                            .is_none()
                        {
                            return Err(invalid(
                                "invalid_input",
                                "TCP backlog must be nonnegative and fit a host int",
                            ));
                        }
                        if state.listening.replace(true) {
                            return Err(invalid(
                                "invalid_input",
                                "TCP listener is already listening",
                            ));
                        }
                        Ok(Slot::boolean(true))
                    }
                    _ => {
                        if !state.listening.get() {
                            return Err(invalid("invalid_input", "TCP socket is not listening"));
                        }
                        let busy = Busy::acquire(
                            &state.accepting,
                            n,
                            "TCP listener already has a pending accepting",
                        )
                        .map_err(native)?;
                        let changed = state.changed.subscribe();
                        let (keep, delay) = (a[1].bits != 0, a[2].bits != 0);
                        Ok(self.native_io(async move {
                            let _busy = busy;
                            let _state = state;
                            interruptible(changed, n, async move {
                                let (stream, _) = socket
                                    .accept()
                                    .await
                                    .map_err(|e| host(n, "Cannot accept TCP connection", e))?;
                                options(&stream, keep, delay, n)?;
                                Ok(NetworkHandle::Stream(Rc::new(Stream::new(stream))))
                            })
                            .await
                        })?)
                    }
                }
            }
            _ => {
                if let NetworkHandle::Udp(state) = self.network_handle(a[0])? {
                    return Err(if state.closed() {
                        invalid("already_closed", "Native handle is already closed")
                    } else {
                        invalid(
                            "invalid_input",
                            "Operation requires a managed stream connection",
                        )
                    });
                }
                let state = self.network_stream(a[0])?;
                if n == NetworkClose {
                    state.close();
                    return Ok(Slot::boolean(true));
                }
                if n == NetworkClosed {
                    return Ok(Slot::boolean(state.closed()));
                }
                let output = a.get(1).is_some_and(|v| v.bits != 0);
                if n == NetworkStreamHalfClosed {
                    return Ok(Slot::boolean(
                        state.closed()
                            || if output {
                                state.output_closed.get()
                            } else {
                                state.input_closed.get()
                            },
                    ));
                }
                if n == NetworkStreamShutdown && state.closed() {
                    return Ok(Slot::boolean(true));
                }
                let socket = state.socket(n).map_err(native)?;
                match n {
                    NetworkTcpEndpoint => {
                        let Transport::Tcp(socket) = &*socket else {
                            return Err(invalid(
                                "invalid_input",
                                "Operation requires a TCP stream",
                            ));
                        };
                        Ok(self.network_endpoint(
                            if output {
                                socket.peer_addr()
                            } else {
                                socket.local_addr()
                            }
                            .map_err(|e| native(host(n, "Cannot inspect TCP endpoint", e)))?,
                        )?)
                    }
                    NetworkStreamShutdown => {
                        let closed = if output {
                            &state.output_closed
                        } else {
                            &state.input_closed
                        };
                        if closed.get() {
                            return Ok(Slot::boolean(true));
                        }
                        if output && state.writing.get() {
                            return Err(invalid(
                                "resource_exhausted",
                                "Output shutdown requires a drained writer",
                            ));
                        }
                        socket
                            .shutdown(if output {
                                std::net::Shutdown::Write
                            } else {
                                std::net::Shutdown::Read
                            })
                            .map_err(|e| native(host(n, "Cannot shut down stream half", e)))?;
                        closed.set(true);
                        // Input shutdown must interrupt an outstanding reader only.
                        if !output {
                            state.read_changed.send_replace(());
                        }
                        Ok(Slot::boolean(true))
                    }
                    NetworkStreamRead => {
                        let limit = self.integer(a[1])?;
                        if limit.to_i64().is_some_and(|n| n <= 0)
                            || matches!(&limit,Integer::Big(v) if v.sign()!=num_bigint::Sign::Plus)
                        {
                            return Err(invalid(
                                "invalid_input",
                                "Stream read limit must be positive",
                            ));
                        }
                        if state.input_closed.get() {
                            return Err(invalid("already_closed", "Stream input is closed"));
                        }
                        let limit = limit.to_i64().map_or(65536, |v| (v as usize).min(65536));
                        let busy = Busy::acquire(
                            &state.reading,
                            n,
                            &format!("{} already has a pending reading", socket.label()),
                        )
                        .map_err(native)?;
                        let changed = state.read_changed.subscribe();
                        Ok(self.native_io(async move {
                            let _busy = busy;
                            let _state = state;
                            interruptible(changed, n, async move {
                                let mut bytes = vec![0; limit];
                                loop {
                                    socket
                                        .readable()
                                        .await
                                        .map_err(|e| host(n, "Cannot read managed stream", e))?;
                                    match socket.try_read(&mut bytes) {
                                        Ok(size) => {
                                            bytes.truncate(size);
                                            return Ok(bytes);
                                        }
                                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                            continue;
                                        }
                                        Err(e) => {
                                            return Err(host(n, "Cannot read managed stream", e));
                                        }
                                    }
                                }
                            })
                            .await
                        })?)
                    }
                    NetworkStreamWrite => {
                        if state.output_closed.get() {
                            return Err(invalid("already_closed", "Stream output is closed"));
                        }
                        let bytes = self.native_bytes(a[1])?;
                        if state.writing.get() {
                            return Err(invalid(
                                "resource_exhausted",
                                &format!("{} already has a pending writing", socket.label()),
                            ));
                        }
                        if bytes.is_empty() {
                            return Ok(self.future_with_state(crate::future::State::Ready(Ok(
                                Slot::boolean(true),
                            )))?);
                        }
                        let guard = WriteGuard {
                            state: state.clone(),
                            complete: false,
                        };
                        let busy = Busy::acquire(
                            &state.writing,
                            n,
                            &format!("{} already has a pending writing", socket.label()),
                        )
                        .map_err(native)?;
                        let changed = state.changed.subscribe();
                        Ok(self.native_io(async move {
                            let _busy = busy;
                            let mut guard = guard;
                            let result = interruptible(changed, n, async move {
                                let mut at = 0;
                                while at < bytes.len() {
                                    socket
                                        .writable()
                                        .await
                                        .map_err(|e| host(n, "Cannot write managed stream", e))?;
                                    match socket.try_write(&bytes[at..]) {
                                        Ok(0) => {
                                            return Err(failure(
                                                n,
                                                "broken_pipe",
                                                "Cannot write managed stream",
                                            ));
                                        }
                                        Ok(size) => at += size,
                                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                            continue;
                                        }
                                        Err(e) => {
                                            return Err(host(n, "Cannot write managed stream", e));
                                        }
                                    }
                                }
                                Ok(true)
                            })
                            .await;
                            guard.complete = result.is_ok();
                            result
                        })?)
                    }
                    _ => Err(Error("unsupported network provider".into()).into()),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::future::State;
    fn call(h: &mut Heap, n: Native, a: &[Slot]) -> Slot {
        match h.native_network(n, a).unwrap() {
            Outcome::Value(v) => v,
            Outcome::Failure(_) => panic!("failed {}", n.symbol()),
        }
    }
    async fn settle(h: &mut Heap, f: Slot) -> Result<Slot, Slot> {
        if let Some(request) = h.future(f).unwrap().io.clone() {
            tokio::time::timeout(
                std::time::Duration::from_secs(3),
                std::future::poll_fn(|cx| request.poll(cx)),
            )
            .await
            .expect("I/O did not wake");
            h.native_io_settle(f).unwrap();
        }
        let State::Ready(result) = h.future(f).unwrap().state else {
            panic!("not ready")
        };
        result
    }
    async fn pair(h: &mut Heap) -> (Slot, Slot) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let connect = TcpStream::connect(address);
        let client = connect.await.unwrap();
        let (server, _) = listener.accept().await.unwrap();
        (
            h.network_new(NetworkHandle::Stream(Rc::new(Stream::new(client))))
                .unwrap(),
            h.network_new(NetworkHandle::Stream(Rc::new(Stream::new(server))))
                .unwrap(),
        )
    }
    #[test]
    fn tcp_read_cancellation_releases_reservation_and_keeps_stream_usable() {
        crate::AsyncHost::new()
            .unwrap()
            .block_on(async {
                let mut h = Heap::new(&[], &[]);
                let (client, server) = pair(&mut h).await;
                let first = call(
                    &mut h,
                    Native::NetworkStreamRead,
                    &[server, Slot::small(32)],
                );
                let busy = call(
                    &mut h,
                    Native::NetworkStreamRead,
                    &[server, Slot::small(32)],
                );
                assert!(settle(&mut h, busy).await.is_err());
                h.future_control(okkama_sparrow::FutOp::Cancel, first)
                    .unwrap();
                assert!(!h.network_stream(server).unwrap().closed());
                let read = call(
                    &mut h,
                    Native::NetworkStreamRead,
                    &[server, Slot::small(32)],
                );
                let bytes = h.native_byte_array(vec![0, 128, 255]).unwrap();
                let write = call(&mut h, Native::NetworkStreamWrite, &[client, bytes]);
                h.collect([client, server, read, write].into_iter())
                    .unwrap();
                assert_eq!(settle(&mut h, write).await.unwrap().bits, 1);
                let result = settle(&mut h, read).await.unwrap();
                assert_eq!(h.native_bytes(result).unwrap(), [0, 128, 255]);
                call(
                    &mut h,
                    Native::NetworkStreamShutdown,
                    &[client, Slot::boolean(true)],
                );
                let eof = call(
                    &mut h,
                    Native::NetworkStreamRead,
                    &[server, Slot::small(32)],
                );
                let result = settle(&mut h, eof).await.unwrap();
                assert!(h.native_bytes(result).unwrap().is_empty());
            })
            .unwrap();
    }
    #[test]
    fn close_wakes_read_and_cancelled_write_closes_shared_stream() {
        crate::AsyncHost::new()
            .unwrap()
            .block_on(async {
                let mut h = Heap::new(&[], &[]);
                let (client, server) = pair(&mut h).await;
                let read = call(&mut h, Native::NetworkStreamRead, &[server, Slot::small(1)]);
                call(&mut h, Native::NetworkClose, &[server]);
                let error = settle(&mut h, read).await.unwrap_err();
                let crate::failure::FailureKind::Native(payload) = h.failure(error).unwrap().kind
                else {
                    panic!("wrong failure")
                };
                assert_eq!(h.native_failure(payload).unwrap().code, "already_closed");
                let bytes = h.native_byte_array(vec![1, 2, 3]).unwrap();
                let write = call(&mut h, Native::NetworkStreamWrite, &[client, bytes]);
                h.future_control(okkama_sparrow::FutOp::Cancel, write)
                    .unwrap();
                assert!(h.network_stream(client).unwrap().closed());
                call(&mut h, Native::NetworkClose, &[client]);
            })
            .unwrap();
    }
    #[test]
    fn input_shutdown_does_not_cancel_pending_output_and_requests_retain_owner() {
        crate::AsyncHost::new()
            .unwrap()
            .block_on(async {
                let mut h = Heap::new(&[], &[]);
                let (client, server) = pair(&mut h).await;
                let bytes = h.native_byte_array(vec![42]).unwrap();
                let write = call(&mut h, Native::NetworkStreamWrite, &[client, bytes]);
                call(
                    &mut h,
                    Native::NetworkStreamShutdown,
                    &[client, Slot::boolean(false)],
                );
                let read = call(&mut h, Native::NetworkStreamRead, &[server, Slot::small(1)]);
                // The native futures retain Rust socket state after both VM handles die.
                h.collect([read, write].into_iter()).unwrap();
                assert!(h.network_stream(client).is_err());
                assert!(settle(&mut h, write).await.is_ok());
                let result = settle(&mut h, read).await.unwrap();
                assert_eq!(h.native_bytes(result).unwrap(), [42]);
            })
            .unwrap();
    }
    #[test]
    fn numeric_scope_validation_does_not_resolve_hostnames() {
        let mut bytes = vec![79, 75, 1, 2, 0, 42];
        bytes.extend(Ipv6Addr::LOCALHOST.octets());
        let n = Native::NetworkTcpBound;
        assert_eq!(
            address(n, &bytes, "0007").unwrap(),
            SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 42, 0, 7))
        );
        assert!(address(n, &bytes, "4294967296").is_err());
        assert!(address(n, &bytes, "bad zone").is_err());
        assert!(address(n, &[], "").is_err());
    }
}
