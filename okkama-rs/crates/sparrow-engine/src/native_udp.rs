//! Native UDP datagrams: one send/receive per request, Tokio readiness and
//! cancellation, exact private packet envelopes, bounded receive buffers.
use crate::{
    Error,
    native::{Outcome, ProviderError},
    native_io::Output,
    native_network::{
        Busy, NetworkHandle, Stream, Transport, address, failure, host, interruptible,
    },
    value::*,
};
use okkama_runtime_abi::{Native, NativeFailure};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::{
    cell::{Cell, RefCell},
    net::{SocketAddr, SocketAddrV6},
    rc::Rc,
};
use tokio::net::UdpSocket;
#[derive(Debug)]
pub(crate) struct Udp {
    socket: RefCell<Option<Rc<UdpSocket>>>,
    changed: tokio::sync::watch::Sender<()>,
    pub(crate) members: RefCell<
        std::collections::HashMap<std::net::IpAddr, Rc<crate::native_multicast::Membership>>,
    >,
    pub(crate) policy: Cell<Option<(u32, u32, bool)>>,
    pub(crate) family_v6: bool,
    dual: bool,
    pub(crate) peer: Cell<Option<SocketAddr>>,
    receiving: Rc<Cell<bool>>,
    pub(crate) sending: Rc<Cell<bool>>,
    pub(crate) connecting: Rc<Cell<bool>>,
}
impl Udp {
    fn new(socket: UdpSocket, family_v6: bool, dual: bool) -> Self {
        Self {
            socket: RefCell::new(Some(Rc::new(socket))),
            changed: tokio::sync::watch::channel(()).0,
            members: RefCell::new(std::collections::HashMap::new()),
            policy: Cell::new(None),
            family_v6,
            dual,
            peer: Cell::new(None),
            receiving: Rc::new(Cell::new(false)),
            sending: Rc::new(Cell::new(false)),
            connecting: Rc::new(Cell::new(false)),
        }
    }
    pub(crate) fn close(&self) {
        for member in self.members.borrow_mut().drain().map(|(_, v)| v) {
            member.closed.set(true);
        }
        self.socket.borrow_mut().take();
        self.changed.send_replace(());
    }
    pub(crate) fn closed(&self) -> bool {
        self.socket.borrow().is_none()
    }
    pub(crate) fn socket(&self, n: Native) -> Result<Rc<UdpSocket>, NativeFailure> {
        self.socket
            .borrow()
            .clone()
            .ok_or_else(|| failure(n, "already_closed", "Socket is closed"))
    }
    fn destination(&self, n: Native, target: SocketAddr) -> Result<SocketAddr, NativeFailure> {
        if target.is_ipv6() != self.family_v6 {
            if !(self.family_v6 && self.dual) {
                return Err(failure(
                    n,
                    "unsupported",
                    "UDP socket and destination families do not match",
                ));
            }
            let SocketAddr::V4(v) = target else {
                unreachable!()
            };
            return Ok(SocketAddr::V6(SocketAddrV6::new(
                v.ip().to_ipv6_mapped(),
                v.port(),
                0,
                0,
            )));
        }
        if self.family_v6 && is_v4(target) && !self.dual {
            return Err(failure(
                n,
                "unsupported",
                "IPv6-only socket cannot send to IPv4",
            ));
        }
        Ok(target)
    }
}
impl Drop for Udp {
    fn drop(&mut self) {
        for member in self.members.get_mut().values() {
            member.closed.set(true);
        }
    }
}
fn is_v4(target: SocketAddr) -> bool {
    match target {
        SocketAddr::V4(_) => true,
        SocketAddr::V6(v) => v.ip().to_ipv4_mapped().is_some(),
    }
}
struct Connecting {
    state: Rc<Udp>,
    complete: bool,
}
impl Drop for Connecting {
    fn drop(&mut self) {
        if !self.complete {
            self.state.close();
        }
    }
}
impl Output for i64 {
    fn publish(self: Box<Self>, _: &mut Heap) -> Result<Slot, Error> {
        Ok(Slot::small(*self))
    }
}
pub(crate) fn handles(n: Native) -> bool {
    use Native::*;
    matches!(
        n,
        NetworkSocket
            | NetworkUdpBound
            | NetworkLocalEndpoint
            | NetworkPeerEndpoint
            | NetworkUdpConnect
            | NetworkUdpReceive
            | NetworkUdpConnectedReceive
            | NetworkUdpSend
            | NetworkUdpConnectedSend
            | NetworkPacketSource
            | NetworkPacketBytes
            | NetworkPacketTruncated
    )
}
fn packet(source: SocketAddr, payload: &[u8], limit: usize) -> Vec<u8> {
    let (family, scope, raw) = match source {
        SocketAddr::V4(v) => (1, 0, v.ip().octets().to_vec()),
        SocketAddr::V6(v) => (2, v.scope_id(), v.ip().octets().to_vec()),
    };
    let mut address = vec![79, 75, 1, family];
    address.extend(source.port().to_be_bytes());
    address.extend(raw);
    let mut bytes = vec![79, 75, 3, u8::from(payload.len() > limit)];
    bytes.extend(scope.to_be_bytes());
    bytes.extend([79, 75, 2]);
    bytes.extend((address.len() as u16).to_be_bytes());
    bytes.extend(address);
    bytes.extend(&payload[..payload.len().min(limit)]);
    bytes
}
fn unpack(n: Native, raw: &[u8]) -> Result<(bool, SocketAddr, u32, &[u8]), NativeFailure> {
    let invalid = |message| failure(n, "invalid_data", message);
    if raw.len() < 8 || raw[..3] != [79, 75, 3] || raw[3] > 1 {
        return Err(invalid("Invalid received packet envelope"));
    }
    if raw.len() < 13 || raw[8..11] != [79, 75, 2] {
        return Err(invalid("Invalid datagram envelope"));
    }
    let length = u16::from_be_bytes([raw[11], raw[12]]) as usize;
    if length == 0 || raw.len() < 13 + length {
        return Err(invalid("Invalid datagram envelope"));
    }
    let mut source = address(n, &raw[13..13 + length], "")?;
    let scope = u32::from_be_bytes(raw[4..8].try_into().expect("four bytes"));
    if let SocketAddr::V6(v) = &mut source {
        v.set_scope_id(scope);
    }
    Ok((raw[3] != 0, source, scope, &raw[13 + length..]))
}
impl Heap {
    pub(crate) fn udp(&self, value: Slot) -> Result<Rc<Udp>, Error> {
        match self.network_handle(value)? {
            NetworkHandle::Udp(state) => Ok(state.clone()),
            _ => Err(Error("expected UDP socket".into())),
        }
    }
    pub(crate) fn native_udp(&mut self, n: Native, a: &[Slot]) -> Result<Outcome, Error> {
        match self.udp_execute(n, a) {
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
    fn udp_execute(&mut self, n: Native, a: &[Slot]) -> Result<Slot, ProviderError> {
        use Native::*;
        let native = ProviderError::Native;
        let invalid = |code, message| native(failure(n, code, message));
        if matches!(
            n,
            NetworkPacketBytes | NetworkPacketSource | NetworkPacketTruncated
        ) {
            let bytes = self.native_bytes(a[0])?;
            let (truncated, source, scope, payload) = unpack(n, &bytes).map_err(native)?;
            return Ok(match n {
                NetworkPacketBytes => self.native_byte_array(payload.to_vec())?,
                NetworkPacketSource => {
                    let endpoint = self.network_endpoint(source)?;
                    if scope == 0 {
                        endpoint
                    } else {
                        let mut values = self.elements(endpoint)?.to_vec();
                        values[2] = self.allocate_text(scope.to_string())?;
                        self.allocate_array(values)?
                    }
                }
                _ => Slot::boolean(truncated),
            });
        }
        if matches!(n, NetworkSocket | NetworkUdpBound) {
            let (v6, kind, endpoint, reuse, broadcast, dual) = if n == NetworkSocket {
                let v6 = match self.integer(a[0])?.to_i64() {
                    Some(1) => false,
                    Some(2) => true,
                    _ => return Err(invalid("unsupported", "IP address family is not supported")),
                };
                let kind = self
                    .integer(a[1])?
                    .to_i64()
                    .filter(|v| (0..=2).contains(v))
                    .ok_or_else(|| invalid("invalid_input", "Unknown portable socket type"))?;
                let protocol = self.integer(a[2])?.to_i64().unwrap_or(-1);
                if !(protocol == 0 || kind == 1 && protocol == 1 || kind == 2 && protocol == 2) {
                    return Err(invalid(
                        "invalid_input",
                        "Socket protocol does not match its type",
                    ));
                }
                (v6, kind, None, false, false, false)
            } else {
                let endpoint =
                    address(n, &self.native_bytes(a[0])?, self.text(a[1])?).map_err(native)?;
                let (reuse, broadcast, dual) = (a[2].bits != 0, a[3].bits != 0, a[4].bits != 0);
                if endpoint.is_ipv4() && dual {
                    return Err(invalid(
                        "unsupported",
                        "Dual stack requires an IPv6 binding",
                    ));
                }
                if endpoint.is_ipv6() && broadcast {
                    return Err(invalid(
                        "unsupported",
                        "IPv6 has no broadcast socket option",
                    ));
                }
                (
                    endpoint.is_ipv6(),
                    2,
                    Some(endpoint),
                    reuse,
                    broadcast,
                    dual,
                )
            };
            if tokio::runtime::Handle::try_current().is_err() {
                return Err(Error("socket acquisition requires a Tokio runtime".into()).into());
            }
            let socket = Socket::new(
                if v6 { Domain::IPV6 } else { Domain::IPV4 },
                if kind == 2 { Type::DGRAM } else { Type::STREAM },
                Some(if kind == 2 {
                    Protocol::UDP
                } else {
                    Protocol::TCP
                }),
            )
            .map_err(|e| native(host(n, "Cannot create socket", e)))?;
            socket
                .set_nonblocking(true)
                .map_err(|e| native(host(n, "Cannot configure socket", e)))?;
            if v6 {
                let _ = socket.set_only_v6(false);
            }
            if let Some(endpoint) = endpoint {
                socket
                    .set_reuse_address(reuse)
                    .and_then(|()| {
                        if v6 {
                            Ok(())
                        } else {
                            socket.set_broadcast(broadcast)
                        }
                    })
                    .map_err(|e| native(host(n, "Cannot apply UDP binding option", e)))?;
                socket
                    .bind(&SockAddr::from(endpoint))
                    .map_err(|e| native(host(n, "Cannot bind UDP socket", e)))?;
            }
            if kind != 2 {
                let stream = tokio::net::TcpStream::from_std(socket.into())
                    .map_err(|e| native(host(n, "Cannot register stream socket", e)))?;
                return Ok(self.network_new(NetworkHandle::Stream(Rc::new(Stream::new(stream))))?);
            }
            let socket = UdpSocket::from_std(socket.into())
                .map_err(|e| native(host(n, "Cannot register UDP socket", e)))?;
            return Ok(self.network_new(NetworkHandle::Udp(Rc::new(Udp::new(socket, v6, dual))))?);
        }
        if n == NetworkLocalEndpoint {
            if let NetworkHandle::Stream(state) = self.network_handle(a[0])? {
                let socket = state.socket(n).map_err(native)?;
                let Transport::Tcp(socket) = &*socket else {
                    return Err(invalid("invalid_input", "Operation requires an IP socket"));
                };
                return Ok(self.network_endpoint(
                    socket
                        .local_addr()
                        .map_err(|e| native(host(n, "Cannot inspect local endpoint", e)))?,
                )?);
            }
        }
        if let NetworkHandle::Stream(state) = self.network_handle(a[0])? {
            return Err(if state.closed() {
                invalid("already_closed", "Native handle is already closed")
            } else {
                invalid("invalid_input", "Operation requires a datagram socket")
            });
        }
        let state = self.udp(a[0])?;
        let socket = state.socket(n).map_err(native)?;
        match n {
            NetworkLocalEndpoint => Ok(self.network_endpoint(
                socket
                    .local_addr()
                    .map_err(|e| native(host(n, "Cannot inspect local endpoint", e)))?,
            )?),
            NetworkPeerEndpoint => Ok(self.network_endpoint(
                state
                    .peer
                    .get()
                    .ok_or_else(|| invalid("not_connected", "UDP socket is not connected"))?,
            )?),
            NetworkUdpConnect => {
                if state.policy.get().is_some()
                    || state.peer.get().is_some()
                    || state.sending.get()
                    || state.receiving.get()
                    || state.connecting.get()
                {
                    return Err(invalid(
                        "invalid_input",
                        "Connect requires an idle unconnected UDP socket",
                    ));
                }
                let target =
                    address(n, &self.native_bytes(a[1])?, self.text(a[2])?).map_err(native)?;
                if target.port() == 0 {
                    return Err(invalid(
                        "invalid_input",
                        "UDP destination port must be positive",
                    ));
                }
                let target = state.destination(n, target).map_err(native)?;
                let busy = Busy::acquire(&state.connecting, n, "UDP connect already pending")
                    .map_err(native)?;
                let guard = Connecting {
                    state: state.clone(),
                    complete: false,
                };
                let changed = state.changed.subscribe();
                Ok(self.native_io(async move {
                    let _busy = busy;
                    let mut guard = guard;
                    let result = interruptible(changed, n, async move {
                        socket
                            .connect(target)
                            .await
                            .map_err(|e| host(n, "Cannot connect UDP socket", e))?;
                        state.peer.set(Some(socket.peer_addr().unwrap_or(target)));
                        Ok(true)
                    })
                    .await;
                    guard.complete = result.is_ok();
                    result
                })?)
            }
            NetworkUdpReceive | NetworkUdpConnectedReceive => {
                let limit = self.integer(a[1])?;
                if limit.to_i64().is_some_and(|v| v <= 0)
                    || matches!(&limit,Integer::Big(v) if v.sign()!=num_bigint::Sign::Plus)
                {
                    return Err(invalid(
                        "invalid_input",
                        "UDP receive limit must be positive",
                    ));
                }
                if n == NetworkUdpReceive && state.peer.get().is_some() {
                    return Err(invalid(
                        "invalid_input",
                        "Unconnected receive requires an unconnected UDP socket",
                    ));
                }
                if n == NetworkUdpConnectedReceive && state.peer.get().is_none() {
                    return Err(invalid("not_connected", "UDP socket is not connected"));
                }
                if state.connecting.get() {
                    return Err(invalid("resource_exhausted", "UDP connection is pending"));
                }
                let limit = limit.to_i64().map_or(65536, |v| (v as usize).min(65536));
                let busy = Busy::acquire(
                    &state.receiving,
                    n,
                    "UDP socket already has a pending receive",
                )
                .map_err(native)?;
                let changed = state.changed.subscribe();
                Ok(self.native_io(async move {
                    let _state = state;
                    let _busy = busy;
                    interruptible(changed, n, async move {
                        let mut buffer = vec![0; 65536];
                        let (size, source) = socket
                            .recv_from(&mut buffer)
                            .await
                            .map_err(|e| host(n, "Cannot receive UDP datagram", e))?;
                        Ok(packet(source, &buffer[..size], limit))
                    })
                    .await
                })?)
            }
            NetworkUdpSend | NetworkUdpConnectedSend => {
                let target = if n == NetworkUdpSend {
                    if state.peer.get().is_some() {
                        return Err(invalid(
                            "invalid_input",
                            "Unconnected send requires an unconnected UDP socket",
                        ));
                    }
                    let target =
                        address(n, &self.native_bytes(a[1])?, self.text(a[2])?).map_err(native)?;
                    if target.port() == 0 {
                        return Err(invalid(
                            "invalid_input",
                            "UDP destination port must be positive",
                        ));
                    }
                    Some(state.destination(n, target).map_err(native)?)
                } else {
                    None
                };
                let peer = target
                    .or(state.peer.get())
                    .ok_or_else(|| invalid("not_connected", "UDP socket is not connected"))?;
                let payload = self.native_bytes(a[if n == NetworkUdpSend { 3 } else { 1 }])?;
                if payload.len() > if is_v4(peer) { 65507 } else { 65527 } {
                    return Err(invalid(
                        "message_too_large",
                        "UDP payload exceeds IP version limit",
                    ));
                }
                if state.connecting.get() {
                    return Err(invalid("resource_exhausted", "UDP connection is pending"));
                }
                let busy =
                    Busy::acquire(&state.sending, n, "UDP socket already has a pending send")
                        .map_err(native)?;
                let changed = state.changed.subscribe();
                Ok(self.native_io(async move {
                    let _state = state;
                    let _busy = busy;
                    interruptible(changed, n, async move {
                        let sent = if let Some(target) = target {
                            socket.send_to(&payload, target).await
                        } else {
                            socket.send(&payload).await
                        }
                        .map_err(|e| {
                            if e.raw_os_error() == Some(libc::EMSGSIZE) {
                                failure(
                                    n,
                                    "message_too_large",
                                    "UDP payload exceeds host or path limit",
                                )
                            } else {
                                host(n, "Cannot send UDP datagram", e)
                            }
                        })?;
                        if sent != payload.len() {
                            return Err(failure(
                                n,
                                "unexpected",
                                "Host reported an incomplete UDP datagram send",
                            ));
                        }
                        Ok(sent as i64)
                    })
                    .await
                })?)
            }
            _ => Err(Error("unsupported UDP native".into()).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::future::State;
    fn call(h: &mut Heap, n: Native, a: &[Slot]) -> Slot {
        let result = if handles(n) {
            h.native_udp(n, a)
        } else {
            h.native_network(n, a)
        };
        match result.unwrap() {
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
            .expect("UDP did not wake");
            h.native_io_settle(f).unwrap();
        }
        let State::Ready(result) = h.future(f).unwrap().state else {
            panic!("not ready")
        };
        result
    }
    fn encoded(address: SocketAddr) -> Vec<u8> {
        let mut bytes = vec![79, 75, 1, if address.is_ipv4() { 1 } else { 2 }];
        bytes.extend(address.port().to_be_bytes());
        match address.ip() {
            std::net::IpAddr::V4(v) => bytes.extend(v.octets()),
            std::net::IpAddr::V6(v) => bytes.extend(v.octets()),
        };
        bytes
    }
    async fn bound(h: &mut Heap) -> (Slot, SocketAddr) {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();
        (
            h.network_new(NetworkHandle::Udp(Rc::new(Udp::new(socket, false, false))))
                .unwrap(),
            address,
        )
    }
    #[test]
    fn envelopes_preserve_binary_prefix_truncation_scope_and_reject_truncated_headers() {
        let source = "[fe80::1%7]:42".parse().unwrap();
        let bytes = packet(source, &[0, 255, 42], 2);
        let (truncated, endpoint, scope, payload) =
            unpack(Native::NetworkPacketBytes, &bytes).unwrap();
        assert!(truncated);
        assert_eq!(endpoint, source);
        assert_eq!(scope, 7);
        assert_eq!(payload, [0, 255]);
        for end in 0..35 {
            assert!(unpack(Native::NetworkPacketBytes, &bytes[..end]).is_err());
        }
        let empty = packet(source, &[], 1);
        assert!(
            unpack(Native::NetworkPacketBytes, &empty)
                .unwrap()
                .3
                .is_empty()
        );
    }
    #[test]
    fn datagrams_remain_atomic_and_cancelled_receive_does_not_consume_a_packet() {
        crate::AsyncHost::new()
            .unwrap()
            .block_on(async {
                let mut h = Heap::new(&[], &[]);
                let (sender, source) = bound(&mut h).await;
                let (receiver, destination) = bound(&mut h).await;
                let cancelled = call(
                    &mut h,
                    Native::NetworkUdpReceive,
                    &[receiver, Slot::small(1)],
                );
                h.future_control(okkama_sparrow::FutOp::Cancel, cancelled)
                    .unwrap();
                let target = h.native_byte_array(encoded(destination)).unwrap();
                let zone = h.allocate_text(String::new()).unwrap();
                let payload = h.native_byte_array(vec![0, 255, 42]).unwrap();
                let send = call(
                    &mut h,
                    Native::NetworkUdpSend,
                    &[sender, target, zone, payload],
                );
                assert_eq!(settle(&mut h, send).await.unwrap().bits, 3);
                let receive = call(
                    &mut h,
                    Native::NetworkUdpReceive,
                    &[receiver, Slot::small(2)],
                );
                let result = settle(&mut h, receive).await.unwrap();
                let raw = h.native_bytes(result).unwrap();
                let (truncated, observed, _, bytes) =
                    unpack(Native::NetworkPacketBytes, &raw).unwrap();
                assert!(truncated);
                assert_eq!(observed, source);
                assert_eq!(bytes, [0, 255]);
                let receive = call(
                    &mut h,
                    Native::NetworkUdpReceive,
                    &[receiver, Slot::small(1)],
                );
                call(&mut h, Native::NetworkClose, &[receiver]);
                assert!(settle(&mut h, receive).await.is_err());
            })
            .unwrap();
    }
    #[test]
    fn connected_empty_datagram_gc_and_invalid_destination_preserve_contracts() {
        crate::AsyncHost::new()
            .unwrap()
            .block_on(async {
                let mut h = Heap::new(&[], &[]);
                let (sender, _) = bound(&mut h).await;
                let (receiver, destination) = bound(&mut h).await;
                let target = h.native_byte_array(encoded(destination)).unwrap();
                let zone = h.allocate_text(String::new()).unwrap();
                let connect = call(&mut h, Native::NetworkUdpConnect, &[sender, target, zone]);
                settle(&mut h, connect).await.unwrap();
                let empty = h.native_byte_array(vec![]).unwrap();
                let send = call(&mut h, Native::NetworkUdpConnectedSend, &[sender, empty]);
                let receive = call(
                    &mut h,
                    Native::NetworkUdpReceive,
                    &[receiver, Slot::small(1)],
                );
                h.collect([send, receive].into_iter()).unwrap();
                assert_eq!(settle(&mut h, send).await.unwrap().bits, 0);
                let result = settle(&mut h, receive).await.unwrap();
                let raw = h.native_bytes(result).unwrap();
                assert!(
                    unpack(Native::NetworkPacketBytes, &raw)
                        .unwrap()
                        .3
                        .is_empty()
                );
            })
            .unwrap();
    }
}
