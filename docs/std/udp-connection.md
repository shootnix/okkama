# Connected UDP

`UdpConnecting(local is IpEndpoint, remote is IpEndpoint, options is UdpOptions)`
is an immutable acquisition request. Construction does no I/O. Each
`async connection() is UdpConnection` call creates a fresh lazy Future, and each
started Future acquires its own socket. Repeated awaits share the same owner.
Port zero is allowed locally for OS assignment and rejected remotely with
`InvalidArgument` before acquisition. No DNS or service lookup occurs.

The old candidate taking an existing UdpSocket is withdrawn: connecting does not
mutate or consume another public socket. Bind options and family/zone rules are
the existing UDP binding/send rules, including explicit IPv6 dual stack.
The socket is owned before awaiting native connect. Failure or cancellation
closes every unpublished descriptor; an unstarted cancellation allocates none.
Binding failures use NetworkOperation.binding; connect failures use connecting.

`UdpConnection` exposes:

- `async send(source is ByteSource) is int`: one packet to the fixed peer, with
  the same lazy single sampling, accepted-byte count, cached Future, size errors
  and cancellation rules as [UDP send](udp-send.md).
- `async datagram(limit is int) is Datagram`: one packet only from the fixed peer,
  with the same positive limit, empty-packet and explicit truncation rules as
  [UDP receive](udp-receive.md). There is no bytes() alias that hides truncation,
  and the connection does not implement byte-stream roles.
- `endpoints() is ConnectionEndpoints<IpEndpoint>`: immutable actual OS-selected
  local and remote numeric endpoints captured after connect, usable after close.
  A wildcard local address becomes route-selected; IPv4 on a dual-stack socket
  may be observed as IPv4-mapped IPv6. Scope is a numeric OS observation.
- `closed() is bool`, `close() is void`, and deterministic destruction. Aliases
  share closure; destroying the immutable request does not close the connection.

The peer cannot be replaced and there is no disconnect or destination-taking
send overload. The private native boundary rejects unconnected operations on a
connected socket and vice versa, so an alternate destination cannot bypass the
fixed peer through an internal UdpSocket wrapper.

Host connect sets the default peer and receive filter; it is not a handshake or
proof of remote reachability. Asynchronous errors reported by the OS (including
ICMP-derived refusal/unreachable errors) keep the attempted reading/writing
operation and existing portable category. Delivery of such errors is not
guaranteed across platforms. These distinctions follow the host
[connect](https://man7.org/linux/man-pages/man2/connect.2.html) and
[UDP](https://man7.org/linux/man-pages/man7/udp.7.html) contracts.

Foreign packets already queued before connect are discarded without becoming
public results. Filtering yields through readiness between discarded packets;
it neither builds an unbounded queue nor spins without allowing cancellation.
The filter treats IPv4 and its mapped IPv6 form as the same peer: some hosts
return different forms from getpeername and recv on a dual-stack socket. Public
endpoint observations retain the actual host form.
One receive and one send may wait independently. Close wakes both; deadlines
compose through Within. Native connect waits for readiness when necessary and
checks SO_ERROR/peer state before publishing success.

Ownership implementation: the unpublished UdpSocket created inside connection()
anchors the existing scoped destructor while connect awaits. UdpConnection
retains that owner internally and captures its own post-connect endpoints. No
native handle accessor or public mutation method is added.
