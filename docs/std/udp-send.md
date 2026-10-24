# UDP send

`async send(target is IpEndpoint, source is ByteSource) is int` on `UdpSocket`
submits one ordinary UDP datagram. Its result is the payload byte count accepted
by the local OS, not a delivery acknowledgement. Empty payload is a real packet
and returns zero. No socket ownership is transferred by the result. This replaces
the withdrawn v3 receiver-return signature; no `Future<void>` or sentinel is used.

The call creates a lazy Future. On first await, reject destination port zero with
`InvalidArgument`, then a closed socket with `NetworkClosed`, before evaluating
`source.bytes()`. Read that source once and retain its exact snapshot across any
readiness retries. Source errors propagate unchanged. Repeated awaits return the
cached count and never resend. Separate send calls represent separate packets.

Preflight payload limits are 65507 bytes for IPv4 (including IPv4-mapped IPv6)
and 65527 for IPv6. These are protocol ceilings, not promises that a host or path
accepts packets that large. The host may reject smaller packets with EMSGSIZE;
both kinds of size failure become `DatagramTooLarge(actual_size)`. The provider
uses a new portable `message_too_large` code for this operation only, keeping
the legacy generic errno mapping unchanged. Native errors do not leak host errno.
The ceilings follow [RFC 8085, section 1](https://www.rfc-editor.org/rfc/rfc8085.txt).
Jumbograms, splitting into multiple UDP datagrams and application-level
retransmission are excluded. IP fragmentation remains the host's responsibility.

A successful attempt must report the entire payload length. An unexpected short
host result becomes `NetworkFailure(writing, other, ...)` without retrying a suffix.
Terminal native failures use `NetworkError(writing, ...)`; native closure maps
to `NetworkClosed`. Size checks happen before invoking the host send operation;
smaller host/path limits are enforced atomically by the kernel at that operation.
No portable pre-query of the path's effective payload limit is claimed.

Only one send may wait per socket. Overlap fails with
`NetworkFailure(writing, resource_exhausted, ...)`; receive can wait independently.
Would-block/interrupted attempts wait for writable readiness and retry the same
packet. Cancellation removes that registration and releases the send slot; it
cannot retract a packet already accepted by the OS. Close wakes both pending
directions with `NetworkClosed`. Deadlines compose through `Within`.

Matching address families are accepted. An explicitly dual-stack IPv6 binding
also accepts IPv4 destinations, translated privately to IPv4-mapped addresses.
An IPv4 binding cannot send to IPv6; an IPv6-only binding cannot send to IPv4.
These combinations yield `NetworkFailure(writing, unsupported, ...)`. Explicit
IPv6 scope is passed to the numeric native address conversion; there is no DNS.

The native layer retains at most one ordinary packet per pending send and one
readiness registration. ByteSource itself is an unbounded whole-value protocol:
an arbitrary source may allocate a large array before size validation can reject
it. This API does not claim to bound source evaluation or language/ABI copies.

Tests use an independent host receiver, real IPv4/IPv6 loopback, and a scripted
native send primitive for deterministic would-block, interrupted, short-send and
EMSGSIZE paths. No external network or new host dependency is needed.

## Example

[udp-send.ok](../../examples/std-v4/udp-send.ok) runs a complete loopback exchange:

```sh
OKKAMA_STDLIB_DIR=std perl okkama-pl/bin/okkama examples/std-v4/udp-send.ok
```

Expected output is `accepted: 3`, `cached: 3`, `received: 3`, `binary intact`.
