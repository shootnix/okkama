# Numeric TCP connections and stream ownership

`TcpConnecting(IpEndpoint[, TcpOptions]).connection()` lazily acquires a
nonblocking connection. Destination port zero is rejected before acquisition.
Repeated awaits of the same Future return the same owner. Acquisition failure
or cancellation closes every unpublished socket. Numeric addresses, including
IPv6 scope identifiers, never trigger a second DNS lookup.

`TcpListening(IpEndpoint[, TcpListeningOptions[, TcpOptions]]).acceptance()`
applies reuse and IPv6-only/explicit dual-stack options before bind, then listens.
Failures distinguish binding from listening. The last options configure accepted
connections. `local_endpoint()` reports the actual OS-selected endpoint, including
an ephemeral port. Closing the listener wakes pending acceptance and leaves
already accepted connections independent.

`TcpConnection` implements `Closing`; `input()`, `output()` and `endpoints()`
expose concrete `StreamReading`, `StreamWriting` and immutable directional
`ConnectionEndpoints<IpEndpoint>`. Closing the connection closes both directions
idempotently. Endpoint snapshots remain available after closure.

Reads require a positive limit and may return a short prefix. Empty bytes mean
orderly EOF. Input aliases share one pending-read gate; overlapping reads fail
with `InvalidState`. Cancelling a read removes its readiness wait, releases the
gate and preserves later bytes. Closing the input half shuts down only input.

Writes sample the ByteSource once and complete all bytes or fail. Started writes
are serialized in arrival order across output aliases. Closing output rejects
new writes but drains already started entries before output shutdown. Cancelling
a queued write releases its reservation and any unclaimed granted lock guard.
Cancellation or failure of a native write can leave a transmitted prefix and
therefore closes the whole connection. There is no synchronous Writing adapter
or flush operation. Keepalive and TCP_NODELAY requests are applied after connect
and on accepted sockets; an enabled host flag need not be represented as integer 1.

The current connector takes numeric endpoints. Host resolution, Happy Eyeballs,
TLS and Internet HTTP exchange remain subsequent stages. Timeout policies compose
through the existing Future/Within contract rather than socket timeout fields.

[Executable loopback example](../../examples/std-v4/tcp-stream.ok).
Cancellation investigation.
Evidence.
