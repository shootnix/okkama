# Typed WebSocket values

`http/websocket` owns WebSocketMessageKind(text,binary), WebSocketMessage and its
immutable TextWebSocketMessage(string)/BinaryWebSocketMessage(@byte) variants.
kind()/text()/bytes() describe exactly one projection: the wrong projection
throws WebSocketMessageIsNotText or WebSocketMessageIsNotBinary. Text validates
Unicode through the existing UTF-8 encoder; no transport is opened.

WebSocketLimits() defaults to 16 MiB per message, 65536 data frame bytes and a
queue of 16. Constructor(message_bytes BigInt,frame_bytes int,queue int) requires
positive bounds at most 1 GiB/1 MiB/1024. Limits are choices, not allocation.

WebSocketClose(code int,reason string) is an emit-able wire close value. Reason
must fit 123 UTF-8 bytes beside the two-byte code. Reporting-only 1005/1006/1015,
reserved 1004 and unknown standards/library codes reject. The profile pins
1000..1003,1007..1014, registered 3000/3003/3008 and private 4000..4999 from the
[IANA registry checked 2026-10-12](https://www.iana.org/assignments/websocket).
Client-only 1010 direction is checked by the subsequent frame consumer.
WebSocketClosed(code,reason) reports closure and uses fixed diagnostic text.
WebSocketFailure(reason) is the typed protocol/budget error.

WebSocketConnection is the restricted future transport contract: async message(),
send(WebSocketMessage), ping(@byte), close(code,reason), closed(). Implementation,
framing, handshake, origin/subprotocol policy and connection lifetime follow in
separate modules; no raw frame API is public.
[RFC6455](https://www.rfc-editor.org/rfc/rfc6455.html),
evidence: four focused assertions
and inline PASS. Std-only; no full regression rerun.

Internal framing now validates minimal lengths, sign bit, direction masks, RSV,
opcodes and unfragmented controls before bounded payload consumption. The reader
consumes exactly one frame, memoizes its future and drains pending children on
cancellation without closing a borrowed transport. Invalid base prefixes reject
before extended reads. Three focused files /14 assertions PASS; actual upgrade
and connection ownership remain pending. Framing report.

Masking uses a checked native repeated-key XOR over at most 65536 bytes per
operation. The existing helper processes larger payloads in chunks, retaining the
four-byte key phase across boundaries. Input snapshots remain independent; this
adds no whole-payload 1 MiB cap. The chunk loop remains synchronous.

`http/websocket_handshake` provides WebSocketClientHandshake(url,protocols,headers,
nonce), WebSocketServerHandshake(Http1RequestHead) and WebSocketAccept(key).
Protocol fields are bounded/singleton as appropriate, subprotocol selection is
case-sensitive and extensions require future explicit negotiation. The client
value accepts deterministic nonce bytes for vectors; production connecting uses
SecureRandom. Server `.response(protocol)` must follow explicit Origin approval.
`http/websocket_origin` owns WebSocketAllowing.allowed(Request,destination Url) and
SameOriginWebSockets: trusted destination, canonical origin tuple, strict browser
Origin, independent of request Host. Internal message assembly supports fragments
and strict whole-message UTF-8 with cumulative budgets and control interleaving.
Protocol report and evidence:
6 files /35 assertions PASS, inline PASS. Owned connection is still pending.

## Owned HTTP1 Upgrade profile

WebSocketPolicy() uses PublicEndpoints, 30-second opening and 5-second close
deadlines plus default message/frame/queue limits. Explicit constructor takes
(limits,opening Duration,closing Duration,access). Deadlines must be positive.
WebSocketConnecting(url,protocols @string,headers,resolver,tls,policy,clock) binds
one lazy checked direct ws/wss attempt. TLS verifies the URL host and SNI before
GET; ALPN permits absent/http/1.1 only. Nonces/masks use SecureRandom. Constructor
preflight performs no DNS/socket/entropy acquisition. Ordinary Exchanging cannot
transfer upgraded ownership. Received headers preserve exact frame prefix.

The returned WebSocketConnection role supports message(), send(), ping(),
close(code,reason), closed(). Incoming messages and outgoing physical admission
are bounded. Close prevents new data, wakes/cancels pending work, waits for its
wire close/peer reply within a deadline, then drains tasks and closes the owner.
A received close echoes once; protocol failure preserves its typed reason.
The infrastructure WebSocketSession can also adopt explicit borrowed halves plus
an owned Closing transport. Its sync close() aborts and settled() drains cleanup;
application receivers get the restricted connection role.

WebSocketUpgrade(protocols,limits,WebSocketAllowing,deadline) is explicit server
protocol policy. WebSocketAccepting(input,output,owner,head-with-tail,destination,
upgrade,clock) checks Origin against the trusted destination before 101, selects
one offered protocol and preserves prefix. Application handler/server integration
is the subsequent layer. Owned transport report:
13 files /70 assertions PASS including actual TCP/TLS. Examples:
[local TCP](../../examples/std-v4/websocket-client.ok),
[verified TLS](../../examples/std-v4/websocket-tls-client.ok).

Per-message deflate and RFC8441 H2 extended CONNECT are pending explicit profiles.
PERF-29 tracks bulk masking: current interpreted XOR is slow for large frames.
