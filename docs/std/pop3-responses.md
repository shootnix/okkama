# POP3 reply framing contract

Status: VERIFIED within this framing/reading contract; full-suite baseline failures remain. This is the next independent protocol slice after IMAP. It does
not complete the blocked IMAP connector or authorize implementing deferred crypto.

`mail/pop3_reply` provides an immutable incremental `PopReplyDecoder` with an
explicit single-line or multiline expected response mode. It parses exactly one
reply; the caller, which knows the command, selects the mode. A negative reply
always completes after its status line, even in multiline mode. Status tokens
are exactly uppercase +OK or -ERR; status text uses printable ASCII and optional
horizontal tabs. Strict CRLF is required throughout.

Multiline data is opaque bytes other than CR/LF framing; no Unicode or message
parsing is implicit. A single-dot line terminates and is omitted. Any longer line
beginning with a dot loses exactly its first dot; this follows the receiver
algorithm even for a nonconforming sender's singly-prefixed nonterminal line.
All other bytes and each data CRLF are preserved. The status and terminal line
are excluded from returned body bytes. The empty body is valid.

Finite configurable limits independently bound status-line wire bytes (default
512 including CRLF), data-line wire bytes (1 MiB including CRLF), decoded body
bytes (16 MiB), aggregate consumed wire bytes (32 MiB), and data lines (100000).
Limits are nonnegative, checked before retaining excess input. Dot-stuffing and
terminal bytes count toward wire bounds but not decoded body bounds. Fed chunks
have an explicit input bound; remainder after completion stays byte-exact and
bounded by that input. Independent successor branches must not share mutable
buffers. Queries on incomplete input and EOF raise typed errors with wire offsets.

`ParsedPopReply(bytes,mode[,limits])` requires exactly one complete reply.
`mail/pop3_reading` provides `PopReplyReading(input,mode)`,
`(input,mode,prefix)` and `(input,mode,prefix,limits,chunk_bytes)`.
`result()` returns the same lazy `Future<PopReplyRead>` on every call, including
failure/cancellation. The completed snapshot provides `reply`, `consumed` and
`remainder`. Default chunks are 4096 bytes, reduced to the remaining wire budget.
Oversized source results are `InvalidReading`; empty input invokes EOF validation.
Cancellation propagates to the pending source read and awaits cleanup. The caller
owns the stream, must serialize consumers, and must discard protocol alignment
after any started failure/cancellation. No implicit retry, close or timeout.
Use `Within` for deadlines. Complete prefixes require no source calls.

The parser and adapter are constructors of observation operations; neither sends
a POP3 command. `PopReply` is an immutable snapshot with `successful`, `text`,
`body`, `lines` and `multiline`; a negative reply has no multiline body. Status
text has no implied CAPA, STAT, UIDL or authentication semantics.
`InvalidPopReply` reports offset/reason, `PopReplyLimitExceeded` reports bound
kind, and invalid configuration/incomplete queries use InvalidArgument/InvalidState.
Internal progress, status-line and snapshot constructors are parser infrastructure.

[Example](../../examples/std-v4/pop3-replies.ok) ·
Evidence.

Verified tests: every split point, one-byte feeding, coalesced replies, empty
body, -ERR in multiline mode, dot lines, high bytes/NUL in message data, strict
status grammar, malformed CRLF, truncation, exact/one-over limits, independent
successor branches, cached async success/error/cancellation and oversized reader
results. A deterministic example passes. The shared full regression ran 546 files /
9544 assertions with the same seven failing-file signatures as the previous
slice; all new tests pass. This is not a green-suite claim.

Wire basis: [RFC 1939 §3](https://www.rfc-editor.org/rfc/rfc1939.html#section-3).
CAPA semantics, command sessions, UIDL, TOP and deletion/QUIT are now covered by
[the owned transaction session](pop3-session.md). TLS and authentication remain
blocked by their shared foundations.
