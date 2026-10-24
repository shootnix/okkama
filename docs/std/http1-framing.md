# HTTP/1 framing

## Bounded head metadata

`http/http1_head` implements strict request/status lines and header fields from
[RFC 9112](https://www.rfc-editor.org/rfc/rfc9112.html). `Http1Limits()` defaults to
65536 head octets, 8192 line octets, 128 fields and 1073741824 body octets. Explicit
`(head_bytes,line_bytes,fields,body_bytes)` requires head 4..1048576, line 1..head,
fields 0..1024 and body 0..2^53-1. The body ceiling is consumed by subsequent body
framing, not by the head parser. Http1LimitExceeded(kind) observes kind()/text().

`ParsedHttp1RequestHead(origin is string[,limits]).value() is Http1RequestHead`;
`ParsedHttp1ResponseHead(origin is string[,limits]).value() is Http1ResponseHead`.
Sources are Latin-1 octet strings containing exactly one complete head ending
CRLFCRLF, with no body suffix. Bare CR/LF, obsolete folding, invalid name/value
octets, extra text and unsupported textual versions are typed InvalidHttpMessage
failures, without source/field payloads. Wire/line/count budgets are checked before
coordinate construction. Logical Headers allow one extra serialization space per
field; this cannot bypass the separately enforced actual wire byte budget.

Request lines require exactly three nonempty space-separated tokens. Method
spelling and four typed target forms retain their existing rules. HTTP/1.1 requires
exactly one syntactically valid ASCII Host authority without userinfo/empty host;
HTTP/1.0 allows omission but rejects duplicates/invalid values. Absolute-form is
accepted on origin servers as RFC 9112 requires. Supplied Host remains distinct
from absolute target authority; routing consumers must use the latter for that
form instead of blindly trusting Host.

Status lines require exact HTTP1 version, three digits in supported 100..599 and
the required space before an optional reason phrase. Reason permits HTAB, SP,
visible/obs-text octets and is ignored presentation, not status identity. No
source phrase or raw request/status line is exposed by the resulting metadata.
Headers retain order and duplicates, including Set-Cookie; no pseudo-fields or
comma joins are introduced.

Http1RequestHead observes method()/target()/headers()/version()/host(); missing
HTTP/1.0 Host throws MissingHttp1Host(). request(body is BodyOpening) explicitly
assembles Request. Http1ResponseHead observes status()/headers()/version();
response(body) assembles Response. Head values never claim an empty body simply
because body bytes were not parsed, and never open/query a supplied body role.
Direct constructors require HTTP1 identity and validated request coordinates.
Content-Length/Transfer-Encoding, chunking/trailers and source consumption are
separate framing slices; no HTTP Internet client/server is claimed here.

Head evidence,
[example](../../examples/std-v4/http1-head.ok).

## Asynchronous borrowed head reading

`http/http1_reading` supplies Http1RequestHeadReading and
Http1ResponseHeadReading. Constructors take `(origin is AsyncReading)`,
`(origin,prefix is @byte)` or `(origin,prefix,limits is Http1Limits,chunk_bytes is int)`.
`result()` returns one memoized Future<Http1HeadRead<Head>>. Construction performs
no input; successful, failed and cancelled results are cached. Chunk sizes are
1..65536 (default 4096). Prefix size cannot exceed head_bytes + chunk_bytes.

The result exposes head(), remainder() and consumed(). Consumed counts exactly
one wire head, including supplied prefix bytes; remainder preserves every binary
octet already obtained after CRLFCRLF. A complete prefix requires no source read.
Delimiter scanning examines only each new chunk plus the preceding three octets;
accumulated metadata is materialized once. An incomplete head at the wire ceiling,
premature permanent EOF and oversized source results fail with typed diagnostics.
Original source failures are preserved.

The operation borrows the source and never closes it. Every started child read is
owned by FutureScope: cancellation drains child cleanup before publishing failure.
The standard reader creates and drains a separate scope for each physical read,
so completed byte Futures do not accumulate for the whole head. This also keeps
CONNECT's one-octet reads from retaining their history. The internal explicit
Http1HeadInput.read(group) path preserves the supplied whole-head supervision.
Unstarted cancellation consumes nothing. A started failed/cancelled read may have
consumed input, so the caller must discard that protocol session; there is no
implicit retry, timeout, body reader or connection reuse promise.

Reader evidence,
cancellation investigation,
[stream example](../../examples/std-v4/http1-reading.ok).

## Canonical borrowed head writing

`http/http1_writing`: Http1RequestHeadWriting(origin is AsyncWriting,head is
Http1RequestHead[,limits]); Http1ResponseHeadWriting(origin,head is
Http1ResponseHead,method is Method[,limits]). result() is a memoized
Future<AsyncWriting>. Construction validates the framing decision and canonical
head wire/line/field budgets before any write. Headers preserve order, duplicates
and name spelling, use `: ` and CRLF. Response version/status are exact, with the
standard phrase or an empty extension phrase; the mandatory separator remains.
Original peer reason text is never copied into outgoing metadata.

The started Future performs one registered write and returns its resulting
AsyncWriting capability. Sink failures and cancellation are cached; structured
cancellation drains sink cleanup. The sink is borrowed and never closed. A
started write may emit a prefix before failing, so the session must be discarded.
Unstarted cancellation performs no write. This writes heads only: the caller
must consume the matching body framing and any tunnel/upgrade protocol separately.
Body/source opening, HTTP exchanges and Internet transport are not supplied here.

Writing evidence,
[Unix round-trip example](../../examples/std-v4/http1-writing.ok).
