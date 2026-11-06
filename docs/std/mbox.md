# Bounded mboxrd content

The pure `mail/mbox` codec uses an explicit mboxrd profile. It does not guess
between mboxo, mboxrd, mboxcl and mboxcl2, whose escaping rules differ; see
[Dovecot's format description](https://doc.dovecot.org/2.3/admin_manual/mailbox_formats/mbox/).
Every `From ` line, including lines already prefixed with any number of `>`,
receives one extra `>` during encoding; decoding removes exactly one. Each
record has an LF-terminated envelope separator, LF message lines and one extra
empty line. Message wire data uses CRLF on the MIME side. Inputs must have a
terminal CRLF; absent terminators are rejected rather than silently added to
body content. Bare CR/LF and NUL in MIME wire data are rejected by this profile.
All other byte values are preserved through newline/escaping conversion.

`MboxSeparator(string)` eagerly validates 8..998 printable ASCII characters,
`From ` prefix, a nonempty sender token and nonempty remaining metadata. It
retains the exact spelling with string(); no newline, SMTP envelope inference,
address parsing or date guessing is performed. Separator metadata is deliberately
opaque and is supplied by the caller when composing a record.

`MboxRecord(separator is MboxSeparator,message is MailMessage)` retains the two
coordinates without observing body bytes. separator()/message() expose them.
`MboxArchive(records is Sequencing<MboxRecord>[,MboxLimits])` eagerly snapshots
record references once, bounding the declared count before item observations;
bytes() renders each message once with the supplied MIME limits. A custom body
may have effects on each rendering; parsed records are immutable snapshots.
Output bounds include separators, quoting, line endings and empty delimiters.
Empty input produces empty output. Borrowed bodies are not closed.

`ParsedMbox(@byte[,MboxLimits]).records()` parses on observation and returns an
eager Sequencing<MboxRecord>. It requires LF-only framing, a first separator,
terminal LF and the extra empty delimiter for every record. It preserves record
order and envelope metadata. MIME parsing remains strict and bounded; Content-
Length/Status/X-UID fields remain ordinary headers, never framing instructions.
No global split of the input or unbounded line-reader is used. Scratch buffers
are owned and closed; no filesystem or network effects occur.
Returned messages are semantic MIME snapshots: re-rendering can canonicalize
header folding or transfer-encoding spelling. Exact original archive byte
spelling is not retained as a separate projection.

`MboxLimits()` defaults to messages=1000, wire_bytes=67108864 and MimeLimits().
The three-argument constructor replaces these nonnegative bounds. Getters expose
them. wire_bytes bounds complete archive input/output; MimeLimits applies per
message including decoded MIME bodies. `MboxLimitExceeded(kind)` distinguishes
messages/wire_bytes; negative bounds/source sizes throw InvalidArgument.
`InvalidMbox(offset,reason)` reports original archive offsets for framing errors,
or a message-relative wire offset while rendering. InvalidMail/MimeLimitFailure
retain their types and decoded-message-relative coordinates during MIME parsing.

Filesystem Mbox/MboxSession are implemented separately in
[mail/mbox_store](mbox-session.md), with a permanent sidecar lock across atomic
replacement and IDs stable within a successor lineage. The pure codec itself
has no locking or filesystem effects.
