# Complete HTTP1 message writing

`http/http1_message_writing`: Http1RequestWriting(origin is AsyncWriting,
request is Request[,limits is Http1Limits]); Http1ResponseWriting(origin,response
is Response,method is Method[,limits]). result() is one memoized
Future<AsyncWriting>. The constructor validates head metadata and exact wire
budgets without querying/opening the supplied BodyOpening or writing bytes.

At startup, known body length is checked **before head emission**. Fixed-length
contradictions, negative metadata and a known source above the body ceiling fail
without opening a reader or emitting any head byte. Unknown length is allowed
only with explicit fixed/chunked framing (or close-delimited response). A request
without length/coding must have a known empty source, preventing silent payload
suppression. HEAD/304/204 and other body-forbidden responses never query/open the
hypothetical source. Tunnel/upgrade responses require dedicated handling and are
rejected before emission; head-only writing remains available to that handler.

The operation then owns one head-writing Future and one bounded body-writing
Future through FutureScope. The body writer uses the sink capability returned by
the completed head write, checks actual fixed EOF and inherits payload/metadata,
trailer and source-closure rules. Started cancellation drains body/sink cleanup.
Unstarted cancellation performs no source/header effect. Errors/results are
cached; replayability is never queried and no replay is attempted.

The sink remains borrowed. A started failure can have emitted a head/payload
prefix; discard that session. Close-delimited output requires its connection
owner to terminate the stream after completion. Framing and Host must be supplied
explicitly: there is no automatic length/header injection, endpoint routing,
100-continue coordination, pool, TCP/TLS connection or Internet client here.

Evidence,
[Unix complete-message example](../../examples/std-v4/http1-message-writing.ok).
