# Owned HTTP1 body reading

`http/http1_body` consumes a previously chosen Http1BodyPlan over AsyncReading.
Http1BodyReading(origin,owner is Closing,plan,prefix is @byte,head is Headers
[,limits is Http1Limits]) implements BodyReading. Prefix is the exact remainder
of a head operation, not a second source read. Constructor validates plan/bounds
without consuming data. Tunnel/upgrade plans require a dedicated protocol consumer
and are rejected here. A successful constructor adopts the explicit owner.

The owner is closed at clean EOF and on started failure, cancellation or early
close. It may be an actual connection-closing adapter or an eventual pool lease;
this reader does not implement a pool or promise to reuse a TCP connection. The
UnixConnection example uses an explicit nominal Closing adapter. Role membership
is not fabricated merely because a concrete object has similarly named actions.

Fixed framing consumes exactly the declared length, accepts short reads and
rejects premature EOF. Empty framing performs no read. Chunked framing strips
size/extensions/CRLF, publishes only payload, validates final trailers and keeps
prefetched next-message bytes. Close-delimited framing ends only at source EOF.
All payload is bounded cumulatively by body_bytes. Metadata (size lines, chunk
terminators and trailers together) is independently bounded by head_bytes;
individual metadata lines use line_bytes, trailer count uses fields. These
explicit profile bounds also limit many tiny chunks. Prefix cannot exceed
head_bytes + 65536; data reads request at most 65536, metadata at most 4096.
The implementation uses native ByteBuffer and array slices for bulk payload.

bytes(limit) requires a positive limit, returns at most that many octets and uses
empty bytes for clean completion. Invalid caller limits consume nothing and do
not abort the session. Oversized source results are InvalidReading; incomplete
framing is InvalidHttpMessage; ceilings are Http1LimitExceeded. Reads serialize
through StreamBodyReading. Every operation owns helper/source Futures through
structured groups, draining cancellation before publishing failure. An unstarted
cancelled Future consumes nothing; explicit reader close is still required.

trailers() is unavailable until a read returns clean EOF. Error/abort never
publishes trailers. remainder() has the same clean-EOF gate and returns the exact
already-read tail, preserving pipeline boundaries without soliciting another
response. Ordinary BodyReading recipients cannot access this extra concrete
action; a session retains its concrete framer separately. After EOF, repeated
reads return empty and trailers remain observable until explicit close. closed()
reflects reader abort/close; clean EOF closes the owner while leaving these
completion observations available. close() is idempotent; destroy aborts.

Http1IncomingBody(reader) implements BodyOpening using the reader's original plan:
length() is exact/UnknownBodyLength, replayable() false and reading() succeeds
once. A separate caller-supplied plan cannot contradict that reader. Unopened
destruction closes its reader. No content decompression, timeout, implicit
discard/drain policy, connection pool or Internet client is supplied here.

Evidence,
[example](../../examples/std-v4/http1-body.ok),
capability investigation.
