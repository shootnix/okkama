//! Memberships use socket2's safe platform wrappers. The UDP owner retains
//! memberships; each membership refers back weakly, so native GC has no cycle.
use crate::{
    Error,
    native::{Outcome, ProviderError},
    native_network::{NetworkHandle, address, failure, host},
    native_udp::Udp,
    value::*,
};
use okkama_runtime_abi::Native;
use socket2::{InterfaceIndexOrAddress, SockRef};
use std::{
    cell::Cell,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    rc::{Rc, Weak},
};
#[derive(Debug)]
pub(crate) struct Membership {
    owner: Weak<Udp>,
    group: IpAddr,
    interface: u32,
    pub(crate) closed: Cell<bool>,
}
pub(crate) fn handles(n: Native) -> bool {
    matches!(
        n,
        Native::NetworkMulticastJoin
            | Native::NetworkMulticastClose
            | Native::NetworkMulticastClosed
    )
}
impl Heap {
    pub(crate) fn native_multicast(&mut self, n: Native, a: &[Slot]) -> Result<Outcome, Error> {
        match self.multicast_execute(n, a) {
            Ok(v) => Ok(Outcome::Value(v)),
            Err(ProviderError::Engine(e)) => Err(e),
            Err(ProviderError::Native(e)) => {
                let payload = self.native_failure_new(e)?;
                self.failure_new(crate::failure::FailureKind::Native(payload))
                    .map(Outcome::Failure)
            }
        }
    }
    fn multicast_execute(&mut self, n: Native, a: &[Slot]) -> Result<Slot, ProviderError> {
        use Native::*;
        let native = ProviderError::Native;
        let invalid = |code, message| native(failure(n, code, message));
        if n != NetworkMulticastJoin {
            let NetworkHandle::Membership(member) = self.network_handle(a[0])? else {
                return Err(Error("expected multicast membership".into()).into());
            };
            if n == NetworkMulticastClosed {
                return Ok(Slot::boolean(member.closed.get()));
            }
            if member.closed.replace(true) {
                return Ok(Slot::boolean(true));
            }
            if let Some(state) = member.owner.upgrade() {
                state.members.borrow_mut().remove(&member.group);
                if let Ok(socket) = state.socket(n) {
                    let socket = SockRef::from(&*socket);
                    match member.group {
                        IpAddr::V4(ip) => {
                            let _ = socket.leave_multicast_v4_n(
                                &ip,
                                &InterfaceIndexOrAddress::Index(member.interface),
                            );
                        }
                        IpAddr::V6(ip) => {
                            let _ = socket.leave_multicast_v6(&ip, member.interface);
                        }
                    }
                }
            }
            return Ok(Slot::boolean(true));
        }
        let state = self.udp(a[0])?;
        let socket = state.socket(n).map_err(native)?;
        if state.peer.get().is_some() || state.sending.get() || state.connecting.get() {
            return Err(invalid(
                "invalid_input",
                "Multicast requires an unconnected UDP socket without a pending send",
            ));
        }
        let group = address(n, &self.native_bytes(a[1])?, "").map_err(native)?;
        let name = self.text(a[2])?;
        let index = self
            .integer(a[3])?
            .to_i64()
            .filter(|n| (1..=u32::MAX as i64).contains(n));
        let hops = self
            .integer(a[4])?
            .to_i64()
            .filter(|n| (0..=255).contains(n))
            .ok_or_else(|| invalid("invalid_input", "Multicast hop limit must be in 0..255"))?
            as u32;
        let loopback = a[5].bits != 0;
        if group.port() != 0 || !group.ip().is_multicast() {
            return Err(invalid("invalid_input", "Address is not a multicast group"));
        }
        if group.is_ipv6() != state.family_v6 {
            return Err(invalid(
                "unsupported",
                "Multicast group must match socket IP family",
            ));
        }
        let valid_name = !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'~' | b'-'))
            && !name.bytes().all(|b| b.is_ascii_digit());
        let index = index
            .filter(|_| valid_name)
            .ok_or_else(|| invalid("invalid_input", "Invalid multicast interface identity"))?
            as u32;
        if state
            .policy
            .get()
            .is_some_and(|p| p != (index, hops, loopback))
        {
            return Err(invalid(
                "invalid_input",
                "Multicast interface and options conflict with the socket policy",
            ));
        }
        if state.members.borrow().contains_key(&group.ip()) {
            return Err(invalid(
                "already_exists",
                "Multicast group is already joined on this socket",
            ));
        }
        let mut numeric = vec![79, 75, 1, 2, 0, 0];
        numeric.extend(Ipv6Addr::LOCALHOST.octets());
        let identity = address(n, &numeric, name).map_err(|_| {
            invalid(
                "invalid_input",
                "Multicast interface name/index no longer identifies the same interface",
            )
        })?;
        if !matches!(identity,SocketAddr::V6(v) if v.scope_id()==index) {
            return Err(invalid(
                "invalid_input",
                "Multicast interface name/index no longer identifies the same interface",
            ));
        }
        let socket = SockRef::from(&*socket);
        if state.policy.get().is_none() {
            let result = match group.ip() {
                IpAddr::V4(_) => socket
                    .set_multicast_ttl_v4(hops)
                    .and_then(|()| socket.set_multicast_loop_v4(loopback)),
                IpAddr::V6(_) => socket
                    .set_multicast_if_v6(index)
                    .and_then(|()| socket.set_multicast_hops_v6(hops))
                    .and_then(|()| socket.set_multicast_loop_v6(loopback)),
            };
            result.map_err(|e| native(host(n, "Cannot configure multicast socket", e)))?;
        }
        match group.ip() {
            IpAddr::V4(ip) => {
                socket.join_multicast_v4_n(&ip, &InterfaceIndexOrAddress::Index(index))
            }
            IpAddr::V6(ip) => socket.join_multicast_v6(&ip, index),
        }
        .map_err(|e| native(host(n, "Cannot join multicast group", e)))?;
        state.policy.set(Some((index, hops, loopback)));
        let member = Rc::new(Membership {
            owner: Rc::downgrade(&state),
            group: group.ip(),
            interface: index,
            closed: Cell::new(false),
        });
        state
            .members
            .borrow_mut()
            .insert(group.ip(), member.clone());
        Ok(self.network_new(NetworkHandle::Membership(member))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn value(outcome: Outcome) -> Slot {
        match outcome {
            Outcome::Value(v) => v,
            Outcome::Failure(_) => panic!("unexpected native failure"),
        }
    }
    #[test]
    fn loopback_membership_rejects_duplicates_and_owner_close_closes_aliases() {
        crate::AsyncHost::new()
            .unwrap()
            .block_on(async {
                let mut h = Heap::new(&[], &[]);
                let local = h
                    .native_byte_array(vec![79, 75, 1, 1, 0, 0, 127, 0, 0, 1])
                    .unwrap();
                let zone = h.allocate_text(String::new()).unwrap();
                let socket = value(
                    h.native_udp(
                        Native::NetworkUdpBound,
                        &[
                            local,
                            zone,
                            Slot::boolean(false),
                            Slot::boolean(false),
                            Slot::boolean(false),
                        ],
                    )
                    .unwrap(),
                );
                let interface = if cfg!(target_os = "linux") {
                    "lo"
                } else {
                    "lo0"
                };
                let mut raw = vec![79, 75, 1, 2, 0, 0];
                raw.extend(Ipv6Addr::LOCALHOST.octets());
                let SocketAddr::V6(scope) =
                    address(Native::NetworkMulticastJoin, &raw, interface).unwrap()
                else {
                    panic!("IPv6 scope")
                };
                assert!(scope.scope_id() > 0);
                let name = h.allocate_text(interface.into()).unwrap();
                let group = h
                    .native_byte_array(vec![79, 75, 1, 1, 0, 0, 224, 0, 0, 251])
                    .unwrap();
                let args = [
                    socket,
                    group,
                    name,
                    Slot::small(scope.scope_id() as i64),
                    Slot::small(1),
                    Slot::boolean(true),
                ];
                let member = value(
                    h.native_multicast(Native::NetworkMulticastJoin, &args)
                        .unwrap(),
                );
                assert!(matches!(
                    h.native_multicast(Native::NetworkMulticastJoin, &args)
                        .unwrap(),
                    Outcome::Failure(_)
                ));
                h.native_multicast(Native::NetworkMulticastClose, &[member])
                    .unwrap();
                h.native_multicast(Native::NetworkMulticastClose, &[member])
                    .unwrap();
                assert_eq!(
                    value(
                        h.native_multicast(Native::NetworkMulticastClosed, &[member])
                            .unwrap()
                    )
                    .bits,
                    1
                );
                let second = value(
                    h.native_multicast(Native::NetworkMulticastJoin, &args)
                        .unwrap(),
                );
                h.native_network(Native::NetworkClose, &[socket]).unwrap();
                assert_eq!(
                    value(
                        h.native_multicast(Native::NetworkMulticastClosed, &[second])
                            .unwrap()
                    )
                    .bits,
                    1
                );
            })
            .unwrap();
    }
}
