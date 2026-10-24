# Direct HTTP client

`HttpClient(Resolving,TlsClientPolicy,ClientPolicy,Timing)` implements Exchanging and Closing. `async response(Request) is ClientResponse` is lazy. Each call creates a distinct exchange; repeated await of one Future observes that exchange. Constructor agreement is checked by Http1ClientProtocols before effects.

Requests must use AbsoluteTarget with HTTP(S) URL, no userinfo/fragment, HTTP/1.1. HttpDirectRequest creates origin-form, substitutes `/` for an empty path, preserves query presence and duplicate fields, and adds Host from canonical URL authority. A supplied Host must be single and exactly equal to that authority. Other target forms and overrides reject before DNS; this deliberately strict first contract does not claim all equivalent Host spellings are accepted. Request preparation never observes body length, replayability or opening.

HttpEndpointSelection checks all numeric DNS candidates with the explicit endpoint policy. Http1Connecting races only these destinations and uses the URL host for TLS identity. `acquisition()` additionally returns Http1Acquisition(session,details); existing `session()` remains compatible. ExchangeDetails reports actual endpoints and selected protocol; secure exchanges retain actual TLS observations, plain `tls()` throws PlainHttpHasNoTls. ClientResponse retains the original absolute request, final response, details and bounded ordered ExchangeHop history (initially empty).

Connection deadline spans DNS, TCP racing and TLS; response-header deadline spans the HTTP1 exchange through final headers, including request upload/100-Continue coordination. Body-idle deadline is fresh for each explicit read. Zero bypasses Within entirely; positive expiry cancels/drains the current operation and throws HttpTimeout with phase connection/headers/body_idle. There is no deadline while the caller is deliberately not reading a response body. Header/body limits are applied by the existing incremental framers. Queued admission is cancellable and is outside connection timing.

The client returns on final headers. A nonempty body opening and its reader retain the concurrency permit until verified EOF, early close, failure or cancellation. Unopened body destruction aborts the exchange. Verified empty responses release admission immediately and provide an independent EmptyBody. Every live transport is owned by a supervised async task, rather than by borrowed Atomic storage. Its lexical lifetime ends on eviction or client close. This lets a clean idle session survive removal of every previous response alias.

A client-local pool key is scheme/canonical host/effective port. TLS policy and absence of a proxy are fixed for that client; sessions cannot cross those boundaries. The connection budget bounds active plus idle connections across all keys, which is stronger than a per-key budget. After admission, a compatible idle session is reused only after endpoint policy is checked again against the new URL and its actual remote endpoint. Fresh acquisition still checks each candidate once. A miss evicts the oldest idle session before opening another. Closed sessions are removed deterministically. There is no idle TTL, automatic drain or stale retry in this first profile. Clean EOF is required before reuse; ambiguous/truncated/abandoned bodies close transport.

The H1-only pending-operation registry admits at most 256 exchanges, including
callers waiting for a connection permit. Further admission throws
InvalidClientRequest("pending H1 exchange budget") before allocating an operation
or consuming its serial. Completing/removing an operation frees a slot. This is
separate from the connection budget; H2/mixed admission already has a 256 cap.
The bounded registry still copies/filters its immutable array on updates.

`close()` first cancels all queued/pending operations, then releases permits and closes active/idle transports. Cancelling all queue entries first prevents a released permit from starting another request during close. Pending awaiters drain cleanup. ClientClosed reports use after close. Client destruction closes explicitly owned infrastructure.

No redirects, cookies, authentication, proxy, cache, content decoding or User-Agent are implicit. An explicit H2-only policy uses the [verified H2 engine and shared physical pool](http2-client.md). A mixed H1.1/H2 policy uses [actual verified ALPN](http-alpn.md) with H1 fallback only when enabled; protocol choice precedes application bytes. [Optional H1 stale retry](http-retry.md) is verified; remaining decorators stay open; explicit redirects/cookies/scoped Basic/Bearer already have their own contracts.

Focused tests cover route preparation without body effects, metadata, real HTTP reuse across caller scopes, all deadline stages with gated clocks, disabled clocks, admission cancellation, actual HTTPS DNS/IP identity/SNI and CLI output. Full regression is intentionally not run because no interpreter/provider code changed.
