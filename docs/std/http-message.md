# Immutable HTTP message metadata

`http/message`: `Request(method is Method,target is RequestTarget,headers is
Headers[,body is BodyOpening[,version is HttpVersion]])` observes method(),
target(), headers(), body(), version(). `Response(status is Status,headers is
Headers[,body is BodyOpening[,version is HttpVersion]])` observes status(),
headers(), body(), version(). Defaults are EmptyBody and HTTP/1.1 identity.
Constructing either message never observes length/replayability or opens/closes
the body role. Body openings retain their existing stateful consumption/lifetime
contract; metadata and header values remain immutable. There is no automatic
transport, body replay, clock, GET/DELETE body prohibition or cleanup capability
added to the message itself.

Request snapshots the target through its declared role and validates method/form
rules. Method source is capped at 8192 token octets. InvalidHttpMessage(reason)
observes reason()/text(); target failures retain their typed coordinate errors.
Headers preserve duplicates/order and existing bounds; no implicit joins occur.
Status retains numeric extension identity. Host/framing/body suppression and
request destination policy belong to the actual wire/connection consumers.
HTTP/2 metadata identity alone does not certify an HTTP/2 connection or codec.
Neither object exposes a raw request/status line as a public value.

`http/version` now supplies consumed message identities
HttpVersion=(http_1_0,http_1_1,http_2). Http1VersionText(origin).string() emits only
HTTP/1.0 or HTTP/1.1; ParsedHttp1Version(origin is string).value() accepts those
exact tokens. UnsupportedHttpVersion() does Error with a payload-free text().
HTTP/2 cannot acquire an HTTP/1 textual version through this conversion. This is
the wire metadata prerequisite, not ALPN negotiation or a transport fallback.
TLS/ALPN and prior-knowledge selection remain separate roadmap work.

Evidence,
[example](../../examples/std-v4/http-messages.ok).
