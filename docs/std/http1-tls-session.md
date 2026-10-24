# HTTP1 over selected TLS protocol

Http1TlsSession(connection is TlsConnection[, limits is Http1Limits]) adopts an
already verified TLS connection after inspecting its actual ALPN snapshot.
Exact http/1.1 is accepted. Absent ALPN (empty string) enables HTTP1.1 fallback.
h2, h2c, case variants and unknown selected protocols throw UnsupportedHttpVersion
before session construction or any application I/O. Rejection leaves the TLS
owner with the caller; it can choose a suitable protocol consumer or close it.
An already closed connection throws Http1SessionClosed.

The successful object implements Closing and delegates async exchange(Request)
is Http1Exchange, closed() and close() to one existing Http1Session. Its body
leases, cancellation, reuse and shutdown rules remain the session contract.
No new framer, HTTP2 implementation, h2c probe, handshake, DNS lookup or downgrade
retry is introduced. Multiple wrappers around the same TLS owner are not a
multiplexing API: supply one exclusive owner to one conversation.

Http1TlsProtocol(selected is string).version() is the same exact pure selection
rule consumed by the constructor. It returns HttpVersion.http_1_1 or the typed
unsupported-version error. Input is a selected protocol, not an advertisement.
Server ALPN selection is definitive for a connection: [RFC7301 section 3.2](https://www.rfc-editor.org/rfc/rfc7301.html#section-3.2).
HTTP2 over TLS requires h2 negotiation and its own protocol consumer:
[RFC9113 section 3.2](https://www.rfc-editor.org/rfc/rfc9113.html#section-3.2).
Absent-ALPN HTTP1 fallback is this explicit consumer policy, not a negotiated h2
fallback or a TLS-version downgrade.

[Runnable CLI example](../../examples/std-v4/https-http1-session.ok),
[underlying leases](http1-session.md), [verified TLS](tls-client-transport.md).
