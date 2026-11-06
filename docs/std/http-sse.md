# Server-sent events

`ServerEvent(data)` is an immutable data event. The four-argument constructor
adds event type, an explicitly supplied identifier (including empty reset), and
retry Duration. Retry requires integral milliseconds, clamps negative values to
zero and caps at 2147483647 ms. Without retry, `retry()` throws MissingEventRetry.
CR/LF/NUL are rejected in output metadata; data accepts arbitrary Unicode.
`ServerEvent(comment,true)` explicitly supplies a keepalive comment. The application
owns its timer/cadence. `ServerEventWire` writes strict UTF-8, a data field for
every normalized CRLF/CR/LF line and a terminating blank line.

`EventBody(receiving)` adopts a bounded `Receiving<ServerEvent>` endpoint. It is
one-shot, non-replayable and has UnknownBodyLength. Each BodyReading owns one
channel receive and one event wire buffer; close cancels the receive and closes
the endpoint. ChannelClosed after buffered events is clean body EOF. Trailer
metadata is available only at EOF. There is no accumulating receive-task history.

`ServerEvents(body)` adopts an HTTP-delimited BodyReading. `await event()` ignores
comments/unknown fields and dispatches only complete blank-line-terminated data
events; unfinished EOF data is discarded. CR dispatch does not wait for another
byte; a following LF is swallowed. One leading BOM is stripped. UTF-8 uses
replacement maximal-subpart decoding, preserving split multibyte scalars.
Type defaults to message, id/retry persist, NUL id values are ignored, and invalid
retry values leave prior retry unchanged. `last_identifier()` updates at blank
block dispatch, even in a block without data, following WHATWG. Incomplete EOF id
is not committed. Concurrent event reads throw EventStreamBusy. close cancels
owned work; settled joins it, even when the source's own close cannot wake a read.

Limits: 8 KiB wire line, 1 MiB data event, 1 KiB event/id/comment metadata and
65536 output data lines. These explicit resource/metadata restrictions are
narrower than a browser's unrestricted input profile and throw typed errors.
Input retains at most a bounded line plus 4 KiB chunk and current event data;
whole stream lifetime and number of comment blocks are not materialized.

`EventStreamHeaders().headers()` explicitly chooses text/event-stream UTF-8,
Cache-Control:no-cache and X-Accel-Buffering:no. Applications compose this into
Response; ordinary H1/H2 streaming/framing performs transmission. Proxy buffering
depends on the intermediary's configuration. Reconnection, redirect, credentials,
Last-Event-ID requests and keepalive scheduling remain explicit client/app policy.

[Runnable channel example](../../examples/std-v4/http-sse.ok),
tests/performance investigation, and
[WHATWG wire semantics](https://html.spec.whatwg.org/multipage/server-sent-events.html#parsing-an-event-stream).
