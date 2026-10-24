# Asynchronous SMTP reply reading

[`mail/smtp_reading`](../../std/mail/smtp_reading.ok) adapts the bounded
[SMTP reply decoder](smtp-wire.md) to `net/stream.AsyncReading`. It reads one
reply from a borrowed stream and returns its reply, consumed wire length and
exact remainder. It neither opens a connection nor sends commands.

```okkama
let request=SmtpReplyReading(input);
let first=await request.result();
let second=await SmtpReplyReading(input,first.remainder()).result();
```

`SmtpReplyReading(origin)` uses an empty prefix, default `SmtpReplyLimits` and
a 4096-byte maximum read size. The two-argument constructor accepts a prefix.
The four-argument constructor takes `(origin,prefix,limits,chunk_bytes)`.
Construction rejects nonpositive chunk sizes with `InvalidArgument` and prefixes
larger than `limits.wire_bytes()` with `SmtpLimitExceeded("chunk_bytes")`.
Construction and `result()` perform no stream reads.

`result()` returns the same `Future<SmtpReplyRead>` on every call. Awaiting it
starts the operation once; simultaneous waiters and later awaits share the
result or failure. Cancellation is also permanent for that operation. Cancelling
before startup consumes nothing, even if the prefix contains a complete reply.

`SmtpReplyRead.reply()` returns the immutable reply; `consumed()` includes prefix
bytes belonging to that reply and excludes the remainder. `remainder()` retains
all bytes after the first final CRLF. A complete prefix requires no source read.
Pass the remainder explicitly to the next operation with the same stream.
Only one operation may consume a stream at a time: this adapter does not provide
a shared session queue or serialize separately constructed operations.

Each request is capped at the smaller of `chunk_bytes` and the remaining reply
wire budget. Exhausting that budget while incomplete throws
`SmtpLimitExceeded("wire_bytes")` before another read. Existing line/count limits
remain enforced by the decoder. Short reads are accepted; an empty result is
permanent EOF and produces `InvalidSmtpReply` with the truncation offset.
An oversized source result throws `io/decorators.InvalidReading`. Source and
decoder failures otherwise retain their types and values. There are no retries.

Use `Within<SmtpReplyRead>(request.result(),duration,clock)` or
`Before<SmtpReplyRead>` to bound the complete operation. Cancellation propagates
through the awaited source future and waits for its cleanup. There is no implicit
timeout. The adapter never closes the borrowed stream, on success or failure.
A started failure or cancellation may consume part of a reply without returning
it: the owning session must discard that connection, rather than start another
reply read or replay a command. This layer does not recover protocol alignment.

No native runtime changes, DNS, TLS, SMTP authentication, envelope negotiation,
command serialization or delivery state machine are included.

[Executable example](../../examples/std-v4/smtp-reading.ok) ·
Tests ·
Evidence.
