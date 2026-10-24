# Asynchronous IMAP response reading

[`mail/imap_reading`](../../std/mail/imap_reading.ok) reads one response from a
borrowed `net/stream.AsyncReading`, using the bounded ASCII
[response framing profile](imap-responses.md). Literal CRLF bytes do not end the
response. Untagged and continuation responses return immediately; the operation
does not collect responses until a command's tagged completion.

```okkama
let request=ImapResponseReading(input);
let first=await request.result();
let second=await ImapResponseReading(input,first.remainder()).result();
```

`ImapResponseReading(origin)` uses an empty prefix, default `ImapResponseLimits`
and a 4096-byte maximum read. `(origin,prefix)` accepts previously retained bytes.
`(origin,prefix,limits,chunk_bytes)` sets explicit bounds. Construction rejects a
nonpositive chunk size with `InvalidArgument` and a prefix larger than
`limits.wire_bytes()` with `ImapResponseLimitExceeded("chunk_bytes")`.
Construction and `result()` perform no source reads. Prefix syntax is checked
when the future starts, including when that prefix already contains a full frame.

Every `result()` call returns the same `Future<ImapResponseRead>`. Concurrent and
subsequent awaits share its immutable result, failure or cancellation. The future
starts once; it never retries. Cancellation before startup consumes nothing.

`ImapResponseRead.response()` returns the completed `ImapResponse`.
`consumed()` counts all wire bytes of that frame, including prefix bytes, literal
announcements, payloads and CRLF. `remainder()` contains the exact supplied bytes
after that frame. A complete prefix requires no source read. Pass the remainder
explicitly to the next operation on the same source; it is not hidden stream state.

Each read requests the smaller of `chunk_bytes` and the remaining response wire
budget. A short result is accepted. Exhausting the budget while incomplete throws
`ImapResponseLimitExceeded("wire_bytes")` before another source read. Syntax-line,
literal-size and literal-count limits remain enforced by the decoder; oversized
announced literals fail before another payload read. An empty source result means
permanent EOF and throws `InvalidImapResponse` at the truncation offset, including
inside a literal or before the final syntax CRLF. An oversized result throws
`io/decorators.InvalidReading`, even if it contains a complete response. Source
and parser failures otherwise retain their types and values.

The underlying framer's profile restrictions apply: ASCII syntax, known response
names, ordinary literals without NUL, literal8 with arbitrary bytes, and no
multiline response codes. This adapter performs no capability negotiation or
semantic validation beyond that profile.

Only one operation may consume a source at a time. Separate operations are not
serialized. Use `Within<ImapResponseRead>(request.result(),duration,clock)` or
`Before<ImapResponseRead>` for a deadline on the complete read. Cancellation
propagates into the awaited source future and waits for its cleanup, including
mid-literal. There is no implicit timeout. The adapter never closes its borrowed
source. After a started failure or cancellation, the owning session must discard
the connection: partially consumed bytes are not returned, protocol alignment is
not recovered, and replaying a command is not safe recovery.

Response bounds apply to one frame. Command-wide byte/count budgets, command
serialization, tagged-completion matching, TLS, authentication and sessions remain
separate work. This module opens no connection and sends no command.

[Executable example](../../examples/std-v4/imap-reading.ok) ·
Tests ·
Evidence.
