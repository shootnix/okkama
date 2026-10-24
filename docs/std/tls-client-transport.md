# Owned verified TLS client transport

`TlsClientHandshake(origin is TcpConnection, server is CertificateIdentity,
policy is TlsClientPolicy[, clock is Timing])` captures a supplied TCP capability
and its endpoints. Construction performs no handshake or ownership transfer.
`async connection() is TlsConnection` is lazy; cancelling an unstarted Future
leaves TCP usable. DNS identity uses `DnsCertificateIdentity(HostName)`; a numeric
reference uses `IpCertificateIdentity(IpAddress)`. Policy roots are explicit.

At startup, the upgrade rejects pending/queued TCP reads and writes, disconnected
or half-closed sockets. Rejection before transfer preserves the original owner.
Successful transfer invalidates **all** old TCP aliases and keeps the same
nonblocking descriptor. TLS owns it exclusively; failure or cancellation after
transfer closes it. Reusing an already consumed origin cannot create another
connection. The private `TlsTcpOrigin` bridge observes shared admission state
without publishing native handles to applications.

The host engine negotiates only the enabled subset of TLS 1.2/1.3, at OpenSSL
security level 2, with renegotiation disabled. A DNS reference supplies ASCII SNI
without its final root dot; an IP reference supplies no SNI. ALPN advertisement
preserves the policy's exact ordered octets; selection is the server's negotiated
result, not a guarantee that it chose the first client preference. No ALPN result
is represented by the empty string.

An unpublished engine state cannot read or write application bytes. After the
handshake, its **actual** peer certificate chain is verified with the supplied
roots, identity, TLS server-auth purpose and the clock's instant sampled then.
This reuses [certificate verification](certificate-verification.md); there is no
default-root discovery/load (even inside the unpublished engine), inherited
client-certificate/CRL paths, disabled-verification policy, CN fallback, AIA/revocation
fetch, resumed session cache or early application data. The provider's internal
deferred verification mode is not an application policy: authorization is
mandatory before publication and every application-I/O entry point checks it.

`TlsConnection` implements `Closing` and owns both `input() is TlsReading` and
`output() is TlsWriting`. These halves implement `AsyncReading`/`AsyncWriting`
and `Closing`, and retain the connection. Closing either half closes the whole
TLS connection; TLS does not offer independent TCP half-shutdown semantics.
`details()` retains immutable `TlsDetails`: directional IP endpoints, the actual
peer-supplied `CertificateChain`, selected `TlsVersion`, cipher name and ALPN
protocol. The peer sequence is not the verifier's selected root path. Details
remain observable after closure; no engine pointer, alert number or key material
is published. Native-state constructors are infrastructure APIs.

Reads require positive limits and return at most 65,536 bytes per engine step.
Overlapping reads through aliases fail before a second read starts. Cancelling a
read detaches readiness, releases admission and preserves the connection. Empty
input means authenticated `close_notify`; raw TCP EOF is truncation and aborts
the owner. Writes sample their ByteSource once after FIFO admission and send all
bytes or fail. Partial progress preserves the offset and yields through readiness
even when the engine reports no WANT flag. Cancelling an active write aborts the
connection because a prefix may already have escaped. Cancelling a queued writer
also releases a guard granted just before cancellation, allowing later writers.

SSL reads can require writable readiness and SSL writes can require readable
readiness. One internal multiplexer shares each underlying registration among
subscribers; cancelling one subscriber does not cancel another operation.
Owned groups cancel and await pending work before publishing cleanup completion.

`close()` is synchronous, immediate, idempotent abort/disposal. `async shutdown()`
performs orderly TLS close-notify exchange, rejects active native reads/writes,
excludes new application I/O once started and disposes the descriptor on success,
failure or cancellation. Drain application work before calling it. A peer that
does not complete shutdown requires an external `Within` deadline/cancellation;
no hidden timeout is installed here.

Certificate-policy rejection gives `CertificateVerificationFailed(reason)`.
Other transport errors give `TlsFailure(phase is TlsPhase, reason is string)`;
phases are handshaking/verifying/reading/writing/closing and reasons are portable
provider codes. Diagnostics omit peer names, engine error stacks and raw alerts.
The engine must support required version/security/renegotiation controls; missing
support fails closed. Legacy `crypto/tls` remains separate and unchanged.

[Runnable HTTPS session](../../examples/std-v4/https-session.ok) consumes these
streams through the existing Http1Session. Its generated fixture is exercised by
interpreter test 449; the standalone example accepts explicit OKKAMA_TLS_PORT
and OKKAMA_TLS_ROOT (DER) environment inputs. The certificate needs DNS SAN
example.test. This supplied-endpoint consumer is not yet URL routing, DNS racing,
a shared HTTP client pool, HTTP2, server TLS or mutual TLS. Those remain queued.

Investigation,
verification evidence.
