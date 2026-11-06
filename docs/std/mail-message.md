# Mail content

This slice represents message content independently of an SMTP envelope.
`MailMessage(headers,body[,MimeLimits])` implements ByteSource and retains
immutable MessageHeaders plus a borrowed MimeBody. It validates MIME metadata
at construction, without reading body bytes. headers(), body(), limits() expose
those coordinates; bytes() renders through MimePart with the same limits.
Unknown fields, order, repeated non-MIME fields and Bcc are preserved. Nothing
infers recipients, invents fields or performs I/O. Address-field validation and
required origination fields belong to a future addressed-message contract.

`MailMessage(part is MimePart)` retains an existing part and its limits.
`part()` returns that explicit domain value. ParsedMail preserves the parser's
structured rendering capability; DeliveredMail creates a validated header
replacement on that part, retaining its body capability and limits. No body
methods outside MimeBody become available through body().

`ParsedMail(@byte[,MimeLimits]).message()` delegates recursive parsing to
ParsedMime. InvalidMime becomes InvalidMail with the original offset/reason;
limit and unsupported-charset errors retain their MIME types. Parsed messages
are snapshots. The parser requires CRLF framing and a blank header separator;
it is not a tolerant mailbox or SMTP parser.

`DeliveredMail(message)` is an explicit ByteSource view. bytes() removes every
root Bcc and Resent-Bcc field, case-insensitively, while preserving other field
order, body and rendering limits. It does not change the source message or
inspect nested encapsulated messages. This preparation neither sends mail nor
creates an envelope. Private recipient data elsewhere is not automatically
redacted. The field-removal option follows the delivery model in
[RFC 5322](https://www.rfc-editor.org/rfc/rfc5322.html).

`MessageId(string)` eagerly validates a bounded modern identifier profile:
angle brackets, dot-atom left side, and dot-atom or no-fold literal right side.
It retains case and spelling. The right side is an identifier, not a DNS lookup
or HostName. The maximum is 998 ASCII bytes, with no whitespace, comments,
quoted obsolete syntax or non-ASCII input. InvalidMail offsets are zero-based
input scalar positions (also byte positions for valid ASCII prefixes).
Generation remains blocked on the HostName contract; parsing makes no claim
about an identifier's uniqueness.

`MailDate(ZonedDateTime[,offset_known bool])` observes the zone offset once at
construction and stores a formatted snapshot. Offsets must be whole minutes
within one day, dates in 1900..9999, and times whole seconds. It never silently
rounds fractions. It prints English weekday/month names, seconds and a numeric
offset. An explicit false offset_known requires zero displacement and prints
-0000. The default is true and prints +0000 for zero displacement.

`ParsedMailDate(string[,maximum_bytes int])` bounds input (default 4096), parses
on observation and exposes value() as a fixed-zone ZonedDateTime, plus
offset_known(). The latter retains -0000/unknown military-zone provenance;
pass both coordinates to MailDate to preserve it. This ASCII profile accepts
optional matching weekday, 1–2 digit day, case-insensitive English month,
2–4 digit year using the obsolete year pivot, HH:MM[:SS], and numeric zones.
It accepts UT/GMT and the eight US standard/daylight abbreviations; single
military letters except J mean unknown zero displacement. Other alphabetic
zones are rejected rather than guessed. Comments may nest to 32 levels,
quoted pairs are accepted inside comments, and CRLF must fold into whitespace.
Comments/whitespace separate date tokens; splitting clock digits or punctuation
with comments is outside this profile. Leap seconds cannot be represented by
TimeOfDay and are explicitly rejected. Lexical errors retain scalar offsets;
invalid civil values or token structure report offset zero. A negative bound
throws InvalidArgument; oversized text throws InvalidMail before scanning.

`InvalidMail(offset,reason)` exposes offset(), reason(), text(). Borrowed bodies
are never closed; MIME renderers own their scratch buffers. Repeated rendering
observes a custom body again, while parsed/binary/text bodies are stable.
