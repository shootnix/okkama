# UDP payload size failure

`DatagramTooLarge(size is int)` does `Error` and preserves the attempted payload
size in bytes through `size() is int`. The size is non-negative; negative counts
throw `InvalidArgument`. Zero and ordinary sizes remain valid diagnostic values:
the failing operation determines the effective limit, not this error object.

`text()` is stable: `UDP datagram too large: N bytes`. The value contains no
payload, peer, host errno or socket capability. It is safe to preserve after the
socket closes and can be handled through either its concrete type or `Error`.

`UdpSocket.send()` now uses this value for IP-version payload ceilings and native
message-too-large failures. See the [send contract](udp-send.md) for the distinction
between protocol preflight and a smaller host/path limit. The diagnostic object
itself still performs no I/O and contains no built-in size policy.

TDD: missing declaration first; then exact size/text, equality, concrete and role
catches, and negative-count rejection. Add an inline test to net/udp; no runtime
changes and no additional full regression are required for this value.
