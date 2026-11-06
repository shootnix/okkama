# SMTP reply, EHLO and DATA values

These modules implement protocol content without opening a connection, issuing
commands, authenticating or inferring an envelope. Server/connection/session and
delivery state machines still depend on HostName/DNS/TCP/TLS and mail addresses.
Advertising STARTTLS or AUTH does not establish verified TLS or authorize sending
credentials. Replies received after STARTTLS require a new capability snapshot.

## Replies (`mail/smtp_reply`)

`SmtpReply(code,lines[,SmtpReplyLimits])` eagerly snapshots Sequencing<string>,
observing its size once and each item once after checking the count bound.
code()/lines()/text()/bytes() are pure projections; text joins logical lines with
LF for display, while bytes emits SMTP CRLF framing. An empty final text is
canonically emitted as a bare code; nonfinal lines use `code-`. Empty sequences,
invalid codes, non-ASCII/control text and embedded newlines fail explicitly.

`ParsedSmtpReply(@byte[,limits]).reply()` requires exactly one complete frame.
`SmtpReplyDecoder([limits])` accepts chunks through `decoder=decoder.fed(chunk)`.
complete()/consumed() observe progress. reply()/remainder() require completion;
finish() reports an incomplete frame as a truncated reply. Empty chunks do not
signal EOF. fed stops at the first final line and preserves the exact remaining
bytes, including a coalesced next reply. A completed decoder rejects any further
feed with InvalidState; initialize a new decoder for its remainder. Successor
branches have independent progress; an error does not mutate the earlier version.

The ASCII profile follows [RFC 5321 reply syntax](https://www.rfc-editor.org/rfc/rfc5321.html#section-4.2):
three digits with first 2..5, second 0..5 and third 0..9; unknown valid codes are
retained. Multiline codes must agree. Bare final codes, empty continuation text,
HT and printable ASCII text are accepted. CRLF may cross chunks. Bare CR/LF,
NUL, other controls, high bytes, changed codes and trailing data in the one-frame
parser are rejected. SMTPUTF8 reply text is outside this ASCII profile.

`SmtpReplyLimits()` defaults to line_bytes=512 (including CRLF), lines=100 and
wire_bytes=65536; the three-argument constructor accepts nonnegative bounds.
Each decoder chunk is separately capped at wire_bytes, as is the accumulated
reply; remainder bytes do not count toward the reply's consumed length.
SmtpLimitExceeded.kind() is line_bytes/lines/wire_bytes/chunk_bytes.
InvalidSmtpReply(offset,reason) exposes offset()/reason()/text(): decoder offsets
are relative to the reply start, constructor text errors to their logical line.
The codec performs no asynchronous reads. The separate
[`mail/smtp_reading` adapter](smtp-reading.md) supplies lazy one-shot reads,
explicit remainders and cancellation/deadline composition over borrowed streams.

## EHLO (`mail/smtp`)

`SmtpCapabilities(reply)` requires code 250 and a nonempty greeting. The first
line is retained only in the original reply and never parsed as an extension;
hostname validation is separate. Remaining lines become eager SmtpExtension
values. extensions() retains their order, original spelling and unknown tokens.
`SmtpExtension(text)` validates at most 512 ASCII characters: a keyword beginning
with an alphanumeric character, followed by alphanumerics/hyphens, then optional
single-space-separated visible ASCII parameters. name() is uppercase;
parameters()/string() preserve values/spelling. Duplicate keywords are rejected
case-insensitively as a conservative profile rule, including unknown extensions.

features() is Set<SmtpFeature>: size, eight_bit_mime, smtp_utf8, pipelining,
start_tls, authentication, dsn, enhanced_status_codes, chunking and binary_mime.
Known extensions except SIZE/AUTH require no parameters in this profile.
Unknown extensions are retained without pretending to support their behavior.

SIZE accepts zero or one decimal parameter as an arbitrary-precision BigInt,
bounded by the reply line length. has_maximum_size() reports an explicit numeric
parameter; maximum_size() throws MissingSmtpSize if none was supplied. Missing
SIZE, bare SIZE and explicit SIZE 0 remain distinct; zero means no fixed server
maximum, per [RFC 1870](https://www.rfc-editor.org/rfc/rfc1870.html#section-3).

AUTH requires at least one mechanism. mechanisms() retains first occurrences
in advertisement order. Names follow the 1..20 uppercase ASCII letter/digit/
hyphen/underscore grammar from [RFC 4422](https://www.rfc-editor.org/rfc/rfc4422.html#section-3.1).
This is an observation of the [AUTH advertisement](https://www.rfc-editor.org/rfc/rfc4954.html#section-3),
not an authentication exchange. Malformed/duplicate extensions, parameters or
mechanisms throw InvalidSmtpCapabilities(reason), with reason()/text().

## DATA (`mail/smtp_data`)

`PreparedSmtpData(message,capabilities[,SmtpDataLimits])` eagerly calls the
borrowed body's bytes() once and returns a ByteSource snapshot. It builds DeliveredMail to
remove Bcc/Resent-Bcc while preserving the source message, then validates rendered
MIME and transport bounds. DATA CTE inspection walks the explicit parsed
MimeParsedNode.children() tree once; it does not serialize and parse each
multipart body again. The existing MimeBody bytes()/validate() role is unchanged.
Rendering may still parse nested bytes to check budgets at that role boundary.
Every leading dot is doubled; `bytes()` ends with the
DATA terminator. message_size() counts content octets before dot stuffing and
the terminator, including content CRLFs. It is suitable for the future MAIL SIZE
argument; the frame contains neither the DATA command nor an envelope.

Literal content must end with CRLF. Missing terminators are rejected; an absent
final CRLF is added only for top-level Base64, where that whitespace leaves the
decoded body unchanged. This normalization counts toward all byte/SIZE limits.
Quoted-printable and literal encodings otherwise retain their existing rendering
rules; no general automatic seven-bit recoding or body-newline change is made.

body_mode() is SmtpBodyMode.seven_bit/eight_bit_mime. High wire bytes or an 8bit
declaration in the parsed MIME/multipart tree require advertised 8BITMIME, or
MissingSmtpExtension("8BITMIME") is thrown. SMTPUTF8 alone does not substitute
for it. Binary CTE in that tree is rejected even when BINARYMIME/CHUNKING is
advertised: DATA does not implement BDAT. Encapsulated message bodies remain
opaque MIME payloads; the flattened DATA byte checks still apply. This follows
the transport distinction in [RFC 6152](https://www.rfc-editor.org/rfc/rfc6152.html#section-3).
NUL, bare newlines and excessive lines fail. MIME errors retain their existing
types. UTF-8 envelope/header negotiation is not implemented here.

`SmtpDataLimits()` defaults to message_bytes=67108864, wire_bytes=134217731 and
line_bytes=1000. All three constructor bounds are nonnegative. MIME rendering
uses the smaller of the message's MimeLimits.body_bytes and message_bytes, while
retaining its other MIME limits. A positive advertised SIZE is checked before
the prepared frame is returned; SIZE 0 supplies no additional maximum. Line
limits include CRLF but exclude the inserted transparency dot. Output limits
include every inserted dot and the three-byte terminator.

SmtpLimitExceeded.kind() is message_bytes/data_bytes/data_line_bytes/server_size
for this layer; stricter original MIME limits retain MimeLimitFailure.
InvalidSmtpData(offset,reason) identifies content-wire errors; a MIME-tree binary
CTE failure uses offset 0. MissingSmtpExtension exposes name()/text(). Scratch
buffers are owned and closed; borrowed message bodies are never closed.
