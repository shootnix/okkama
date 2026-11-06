# HTTP runtime gate

HTTP protocol objects consume existing `net/stream::AsyncReading` and
`AsyncWriting`, with their normal Future cancellation and cleanup rules.
`Within` supplies deadlines. A native HTTP ABI is not justified by a method,
status or header value; these are pure std objects and perform no I/O.

Numeric TCP connect/listen/accept and system/fixed DNS are implemented.
[Verified owned TLS client](tls-client-transport.md), explicit roots/path verification,
complete URL authority, IDNA hosts and pinned PSL are now implemented.
The supplied-endpoint HTTPS CLI example exercises Http1Session over verified TLS.
Content coding depends on deferred Stage 16 codecs. [Checked URL route opening](http1-connecting.md) now connects exact allowed
numeric candidates and verifies the URL identity. Bounded numeric connection racing is now implemented; streamed DNS/strict resolver
profile, shared pools, server TLS and HTTP2 remain open;
these prevent claiming a complete Internet client/server stack. Legacy `stdlib/http` is not evidence of
that v4 implementation.

Proceed with independent pure values and bounded protocol contracts in queue
order. A stream-only framer can be tested with deterministic streams and an
existing Unix pair; it must not imply Internet connecting or certificate
verification. HTTP/2 requires its own tested framing, HPACK and stream lifecycle.

Native acceleration remains **deferred pending a measured consumer bottleneck**.
It must preserve the same object contract and portable failure model, and reuse
the existing scheduler, socket ownership and TLS boundary. There is no second
event loop, speculative native handle, fake provider or placeholder HTTP ABI.

This gate adds no HTTP native ABI. The concrete supplied-stream HTTP1 and
verified TLS providers have their own integration evidence; it does not certify
the remaining shared client/pool, streamed DNS/strict resolver profile or HTTP2.

## Version negotiation

`http/version` supplies tested identities consumed by Request/Response and exact
HTTP1 token conversion/parsing. Actual TLS/ALPN metadata is consumed by [Http1TlsSession](http1-tls-session.md):
exact http/1.1 or absent ALPN, with no fallback from another selected protocol. Cleartext selection needs protocol consumers;
prior-knowledge HTTP2 requires explicit policy and the HTTP2 implementation. No
h2c upgrade or silent fallback is added; an identity does not certify a transport.

## Target coordinate prerequisite

[http/target](http-target.md) is verified independently of transport: four actual
coordinate forms, canonical role snapshots and method-aware parsing. 70 focused
groups with URL compatibility, inline and example PASS. The old full-URL gate is
resolved. Request/Response/wire metadata now consume these values; connection
and automatic protocol selection/routing still need consuming contracts and tests.
