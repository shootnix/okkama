# HTTP1 session opening from checked routes

Http1Connecting(route is HttpEndpointSelection, policy is TlsClientPolicy) is a
lazy request with default Http1Limits/SystemClock. Its full constructor adds
explicit limits and Timing. Construction rejects every advertised ALPN name
other than exact http/1.1; empty advertisement allows absent-ALPN fallback. This
HTTP1 consumer never advertises support for an unimplemented h2 conversation.

async session() is Http1Session resolves/filters through the route once and uses
only its exact numeric endpoints. It uses bounded TcpRacing with stable family interleaving, relative 250 ms
stagger and early-failure acceleration. FirstSuccessful preserves typed failure
ordering when every attempt fails. There is no second DNS lookup. Other failure types retain
their meaning and are not treated as ordinary fallback. HTTP creates an owned
cleartext session. HTTPS upgrades the connected TCP, verifies the URL's own
nominal DNS/IP identity with the supplied roots and clock, checks actual ALPN
using Http1TlsProtocol, then transfers the owner to the same Http1Session API.

Every startup stage retains the raw future before eager group start. Cancelled
unclaimed successful results are explicitly disposed; unpublished TCP/TLS owners
are closed. Published sessions own their transport and body leases normally.
An unstarted session cancellation creates no route/socket work. A repeated open
requires a fresh request object. The caller supplies HTTP requests to the session;
this opener does not generate headers, rewrite targets, forward credentials,
follow redirects or implement a shared HttpClient pool.

[TcpRacing](tcp-racing.md) now removes sequential blocking behind a silent first
candidate and closes every losing attempt, resolving the checked-route part of
PERF-18. Compose Within for an
explicit overall deadline; streamed DNS/strict RFC8305 profile remain separate.
No timeout budget is silently installed or relaxed here.

[Actual URL-derived HTTPS CLI](../../examples/std-v4/http1-connecting.ok) demonstrates
checked loopback candidates, DNS/IP reference verification, DNS-only SNI,
query-preserving origin target and discarded URI fragment. URL/port and a DER
trust anchor are explicit environment fixture inputs; no public Internet service
is contacted. TLS mismatches reject before HTTP bytes.
