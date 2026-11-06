# HTTP1 body boundary decisions

`http/http1_framing` implements an immutable framing decision before any body is
opened or read. [RFC 9112 sections 6 and 9](https://www.rfc-editor.org/rfc/rfc9112.html#section-6.3)
define precedence and persistence. This strict profile rejects ambiguous framing
instead of forwarding it. Actual stream decoding/chunking/leases remain separate.

Http1RequestFraming(head is Http1RequestHead[,limits is Http1Limits]).plan();
Http1ResponseFraming(head is Http1ResponseHead,method is Method[,limits]).plan().
The response method is explicit: status alone cannot determine HEAD or CONNECT.
Http1BodyPlan exposes kind() is Http1BodyKind, length() is BigInt and
reuse_after_eof() is bool. Kinds: empty, fixed, chunked, close_delimited, tunnel,
upgrade. length() is zero for empty, exact for fixed, otherwise UnknownBodyLength.
Eligibility is not a lease: actual clean completion, session state, peer request
version and absence of unread unsolicited responses are still required for reuse.
HTTP1.0 keep-alive eligibility applies to direct endpoints; a proxy cannot retain
an HTTP1.0 client connection merely because this flag is true.

Content-Length accepts unsigned ASCII digits, optional edge SP/HTAB and up to 16
comma/duplicate members. Every trimmed lexeme must be byte-for-byte identical:
`02,02` is two octets, `2,02` is rejected. Numeric lexemes are limited to 1024
digits before BigInt parsing; no host int/float conversion can silently wrap.
For payloads, exact declared length is compared to the explicit body ceiling
before reading. Header field/count/line bounds also apply to directly supplied
head values (using minimal field wire size; full start-line budget belongs to the
head parser/encoder).

Transfer-Encoding and Connection use strict nonempty token lists, at most 16
members each. Names are ASCII case-insensitive. HTTP1.0 Transfer-Encoding is
invalid. Chunked must occur once and last; the first decoder profile supports
chunked alone. Earlier gzip/etc produce UnsupportedTransferCoding, without
silently returning compressed transfer bytes as decoded content. Transfer-coding
parameters are outside this initial token profile. Both requests and ordinary
responses reject simultaneous Transfer-Encoding and Content-Length. Connection
close overrides keep-alive; HTTP1.1 defaults to eligible, HTTP1.0 requires an
explicit keep-alive token.

A request without length/coding is empty; explicit length zero remains fixed.
An ordinary response without either is close-delimited and ineligible for reuse.
HEAD and 304 are empty: validated length/coding describes the hypothetical
representation and does not consume the body ceiling or require an unused
decoder. 1xx and 204 are empty and reject forbidden framing fields in this
strict profile. 101 produces upgrade, requiring a dedicated consumer. Successful
CONNECT produces tunnel before Content-Length/Transfer-Encoding interpretation,
ignoring both fields as required; it never grants ordinary HTTP connection reuse.
CONNECT 204 has no HTTP body, but switches to tunnel mode like every CONNECT
2xx under RFC 9110 section 9.3.6; it cannot be reused as an ordinary HTTP session.

Evidence,
[example](../../examples/std-v4/http1-framing.ok).

## Chunk lines and trailer sections

`http/http1_chunk` validates metadata independently of the eventual source
consumer. ParsedHttp1ChunkLine(origin is string[,limits]).size() is BigInt.
The source excludes its terminal CRLF and uses Latin1 octets. Size requires
1..16 hexadecimal digits, including leading zeroes; exact BigInt decoding and
body ceiling comparison prevent overflow. The whole line respects line_bytes.
RFC chunk extensions accept historical BWS around semicolon/assignment, token
names, optional token/quoted values, safe quoted-pairs and obs-text in quotes.
Unknown valid extensions are ignored; dangling assignment/escape, folding,
controls, Unicode outside Latin1 and trailing junk fail as InvalidHttpMessage.

ParsedHttp1Trailers(origin is string,head is Headers[,limits]).value() is Headers.
Empty source section is one CRLF; fields require CRLFCRLF and no following body.
Wire bytes, individual lines and field count use explicit Http1Limits. Duplicates
remain ordered, names compare case-insensitively, and values retain ordinary
Header validation. Connection-nominated fields and known framing, routing,
authentication, request modifier, response control and content-format fields are
forbidden. Cookies are forbidden too. Custom metadata/integrity fields may be
retained separately; this receiving codec does not authorize their generation or
merge them into headers. Senders still need the field definition's trailer
permission under [RFC 9110 section 6.5](https://www.rfc-editor.org/rfc/rfc9110.html#section-6.5.1).
This is chunk/trailer metadata, not a claim of completed stream decoding.

Chunk evidence,
[example](../../examples/std-v4/http1-chunks.ok).
