# HTTP server contracts

The server profile begins with explicit objects and focused checks.
`http/request_context.HttpRequestContext` is infrastructure-created from supplied
ExchangeDetails and a restricted Cancelling capability; handlers receive endpoint/
TLS observations and immutable route parameters. parameters(map) is a successor,
not mutation, preserving transport facts and cancellation identity. Parameter
count/text budgets are 128/8192 characters with nonempty names up to256.

`http/handler` separates HandlingResult variants. ResponseHandling(response)
returns only a Response projection. WebSocketHandlingResult(upgrade,application)
binds approved protocol policy separately from WebSocketHandling.handle(connection,
request,context). Wrong projections throw WrongHandlingKind. Handling.handling
is async(Request,HttpRequestContext)->HandlingResult. No raw socket is granted.

`http/server_policy.ServerTimeouts(headers,body_idle,keep_alive,handling)` accepts
nonnegative Duration values: zero explicitly disables each respective deadline.
Defaults30/30/5/30 seconds. ServerPolicy(protocols,limits,timeouts,connections,ranges)
snapshots nonempty H1.1/H2 identities with positive bounded connection/range budgets
1..1024/1..128. DefaultsH1.1/HttpLimits/64connections/16ranges. Policy does not
imply an available negotiated transport; the concrete server selects its profile.

Evidence: two files /four
assertions PASS. Constructor/configuration objects open no listener/transport.
Concrete server/request body lifecycle, deadlines, drain and routing follow.

## Owned plaintext HTTP1.1 profile

`http/server.HttpServer(TcpAcceptance,Handling,ServerPolicy,Timing)` adopts the
listener. A fifth Url argument supplies a trusted plaintext WebSocket destination;
Host never establishes that trust. A single H1.1 policy selects origin-form HTTP1 requests; a single H2 policy selects explicit cleartext prior knowledge. Mixed selection is rejected until a negotiated transport is supplied. await serve(); close() stops/cancels; await settled() drains.
`ServerDrain(server,Duration,clock).drain()` stops accepting and idle peers, lets
active responses finish and cancels/drains at its deadline (TimedOut remains
typed). Pending/active/retired task registries are bounded and joined.

Request framing is validated before handler admission. Pipelined responses stay
in order; unread bodies are drained only for reuse. Early nonreusable replies
close without waiting for unsent bodies. Expect100 sends an interim only upon
actual source consumption; early final response closes; unsupported expectation
417. Defaults add no Date/Server. Request errors400, limits413/431, headers/body
timeout408, handler deadline503 and handler error500 precede response emission;
mid-response errors close. Keep-alive, whole-header, body-idle and handler
deadlines are independent, zero-disabled as documented.

Typed WebSocket upgrade checks configured Origin before101 and transfers to a
separate supervised application lifetime, exempt from ordinary handler timeout.
Server close/drain deadline still cancels the application.
Final profile evidence:7files/26
assertions PASS, including [actual CLI](../../examples/std-v4/http-server.ok).
Problem/cause/solutions. Std-only; no full
rerun. TLS server and general H1 peer-disconnect monitoring during unrelated
handler work remain separate open profiles.

## Explicit plaintext H2 listener

A policy containing only `HttpVersion.http_2` selects [Http2ServerSession](http2-server.md) for every accepted peer. Each worker supervises its session and a drain observer. `ServerDrain` stops acceptance, sends GOAWAY, finishes admitted responses and cancels/drains all work at its deadline. No H1 failure response is written on an H2 transport. The application API is shared, but H2 request targets are reconstructed absolute URLs rather than H1 origin targets. H1 remains the default.

Listener compatibility evidence: 3 files /14 assertions PASS, actual graceful H2 drain and deadline cancellation plus H1 compatibility. [H2 wire example](../../examples/std-v4/http2-server.ok) independently composes one accepted H2 session. Server TLS/ALPN, h2c Upgrade, mixed-protocol listeners and negotiated RFC8441 are open. H2 wire/idle deadlines and protocol-failure GOAWAY now have a 47-check focused gate.
