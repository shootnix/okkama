# UDP bind and ownership

`UdpBinding(local is IpEndpoint, options is UdpOptions)` retains a bind request
without acquiring a socket. Each `async socket() is UdpSocket` call creates a
fresh lazy operation; awaiting the same Future twice returns its cached socket.
No DNS, service lookup or IDNA is involved. Port zero requests OS assignment.

The private runtime/network.udp_bound operation creates a nonblocking UDP socket,
applies reuse/broadcast/IPv6-only settings before bind, then publishes its handle.
Every configuration/bind failure closes the unpublished descriptor. IPv4 with
dual_stack=true and IPv6 with broadcast=true fail explicitly as unsupported.
IPv6 zones accept existing interface names or decimal indexes in uint32 range;
names are resolved by the host without DNS. IPv4 zones are rejected.

UdpSocket owns this handle and closes it on destruction. Its initial public
surface is local_endpoint(), closed(), close(): actual OS-selected endpoint,
shared closure observation and idempotent close. Endpoint observation is captured
while acquiring the socket and remains usable after close. All aliases share the
same native closure state. Close errors are translated through NetworkError;
binding errors are attributed to NetworkOperation.binding.

This document covers acquisition and ownership. Subsequent
[receive](udp-receive.md) and [send](udp-send.md) contracts cover packet I/O,
readiness cancellation and datagram limits. Connected UDP remains open.

TDD: native loopback bind, port zero, actual address and configured OS options,
family/zone failures, busy address, nonblocking/CLOEXEC flags, idempotent close;
stdlib lazy acquisition, cached/fresh futures, endpoint lifetime and destroy
cleanup. Run focused native/std tests first and full regression for the provider
change. No new host dependencies or parser/interpreter architecture changes.

The scope-release test exposed a runtime defect: starting a lazy Future from an
async continuation used the ambient environment instead of the awaiting lexical
scope. Returned owners were retained until the outer environment exited. The
focused lifetime test returns a child from a nested scope and requires its async
parent to be destroyed before the next outer statement, while the child remains
usable. Await must start the lazy body in its actual lexical environment.
