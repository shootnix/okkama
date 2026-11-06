# Bounded IMAP server-response framing

[`mail/imap_response`](../../std/mail/imap_response.ok) frames one server
response from incremental byte chunks. It preserves syntax lines, literal
payloads and trailing bytes without connecting, sending commands or interpreting
FETCH values, response codes, sequence numbers or UIDs.

## Decoder and snapshots

`ImapResponseDecoder([limits])` advances through
`decoder=decoder.fed(chunk)`. complete()/consumed() observe progress. response()
requires completion; finish() reports a truncated frame as InvalidImapResponse
if incomplete. Empty feeds do not mean EOF. A completed decoder rejects all
further feeds, including empty ones, with InvalidState.

Decoding stops at the first complete response. remainder() requires completion
and returns the exact unused suffix, which may contain the next response.
Old decoder versions remain independent; a failed feed leaves the earlier
version unchanged. Literal payload fragments are accepted in bulk rather than
being rescanned as protocol characters.

`ParsedImapResponse(origin[,limits]).response()` requires exactly one complete
response, with no trailing bytes. It performs parsing when response() is called.

`ImapResponse` does ByteSource and exposes:

- kind(): ImapResponseKind.tagged, untagged or continuation.
- name(): uppercase response name/status, or empty for continuation.
- tag(): original tag for a tagged response; otherwise MissingImapTag.
- parts(): immutable ordered ImapResponsePart values.
- bytes(): the exact original response bytes, without the decoder remainder.

Part kind is ImapResponsePartKind.text, literal or literal8. Text part bytes
include their CRLF and any literal marker. Literal parts contain only the payload,
including empty payloads. Concatenating the parts reproduces bytes().
ImapResponse is constructed from completed ImapResponseProgress; progress and
the other parser collaborators are implementation helpers.

## Supported framing profile

Tagged responses require a valid tag and OK/NO/BAD followed by a space and
possibly empty response text. Untagged status names are OK/NO/BAD/BYE/PREAUTH.
Continuation requires `+ `, with possibly empty text. Braces and quotes in
human-readable response text do not introduce literals or quoted data.

Structured untagged names supported here are CAPABILITY, FLAGS, LIST, LSUB,
SEARCH, STATUS, ENABLED, NAMESPACE, ID, ESEARCH, VANISHED and METADATA. Numeric
prefixes support EXISTS, RECENT, EXPUNGE and FETCH. Numeric values and each
response's payload grammar are left for semantic parsers. This is deliberately
a closed list: unknown response names fail instead of guessing whether their
extension grammar uses literals or free text.

Outside literals, this profile accepts printable ASCII and HT, with strict CRLF
line endings. NUL, other controls, DEL and high bytes fail. It does not implement
negotiated UTF-8 response text or UTF-8 quoted strings. In structured data, quoted
strings cannot cross a line boundary, and escapes may quote only quote/backslash.
An unquoted literal marker must start after space or opening parenthesis, or at
the beginning of a continuation syntax line.

The literal and response forms follow
[RFC 3501 §4.3](https://www.rfc-editor.org/rfc/rfc3501.html#section-4.3) and
[§9](https://www.rfc-editor.org/rfc/rfc3501.html#section-9): `{n}` followed by CRLF
introduces exactly n payload bytes, then syntax resumes until a final CRLF.
CRLF inside the payload has no framing meaning. Ordinary literals reject NUL;
`~{n}` creates a literal8 and accepts all octets. Zero-length literals still
require the following syntax line's final CRLF. Multiple literals are supported.

Non-synchronizing `{n+}` and `~{n+}` markers are rejected on server input. The
server/client distinction and literal8 are specified by
[RFC 9051 §4.3](https://www.rfc-editor.org/rfc/rfc9051.html#section-4.3) and
[§9](https://www.rfc-editor.org/rfc/rfc9051.html#section-9). Recognizing literal8
does not validate its permitted FETCH position or negotiate BINARY support.

If status/continuation text starts with a bracketed response code, the code must
close on the same physical line, outside quoted strings, and be followed by a
space. Literal-bearing/multiline response codes are outside this profile and
fail explicitly. Code contents are not otherwise interpreted. This prevents a
partial multiline code from silently being returned as a complete response.

## Bounds, failures and ownership

`ImapResponseLimits()` defaults to line_bytes=8192, literal_bytes=16777216,
wire_bytes=33554432 and literals=100. Its four-argument constructor takes those
nonnegative integers; negative bounds throw InvalidArgument.

line_bytes includes the syntax line's CRLF and excludes literal payload bytes.
literal_bytes bounds each literal; literals bounds their count, including empty
ones. wire_bytes bounds the whole response including markers, payloads and CRLF.
Each fed chunk is separately bounded by wire_bytes; remainder does not contribute
to consumed(). Marker lengths are checked without integer overflow. A marker
must fit the literal limit and remaining response budget, reserving at least the
final CRLF, before payload is accumulated. The API has no aggregate budget for
all responses belonging to a command; that belongs to a future command reader.

`ImapResponseLimitExceeded(kind)` exposes kind()/text(): line_bytes,
literal_bytes, wire_bytes, literals or chunk_bytes. `InvalidImapResponse(offset,
reason)` exposes offset()/reason()/text(); offsets are relative to the response
start, and truncation reports bytes consumed. Header errors identify the prefix
field/start rather than an exact malformed character. `MissingImapTag` exposes
text().

All stored bytes and state are immutable snapshots. Scratch buffers close on
success and failure. No native runtime or dependency was added. No asynchronous
reader, command completion policy, timeout, authentication or session is implied.

[Example](../../examples/std-v4/imap-responses.ok) ·
Tests ·
Evidence.
