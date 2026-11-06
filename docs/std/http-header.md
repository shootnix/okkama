# HTTP header values and bounded snapshots

`HeaderName(string)` validates the HTTP token grammar, preserves `string()`
spelling and provides ASCII `lowercase()` and `Comparing<HeaderName>.order()`.
Case-insensitive HTTP matching uses these observations, not structural `==`.
`Header(name,value)` validates a logical unfolded field value at construction:
HTAB, SP, visible ASCII and obs-text octets are accepted; C0 except HTAB, DEL,
CR/LF/NUL and scalars above U+00FF are rejected as InvalidHeader. Strings represent
wire octets through ISO-8859-1, not an implicit UTF-8 or locale conversion.
Only leading/trailing SP and HTAB are removed; inner whitespace is preserved.
Validation and trimming use exact regex checks in windows of at most 65536
characters. This does not impose the regex provider's 1 MiB input limit on one
Header. Non-octet Unicode is reported before controls, including across windows;
aggregate HeaderLimits retain their existing separate budget.
Mail/MIME unfolding is deliberately not used. Folding is invalid HTTP input.

`Headers(entries[,limits])` snapshots a Sequencing<Header> once and preserves
order and duplicate fields. `HeaderLimits(entries,bytes)` defaults to 1024 and
1048576, with nonnegative coordinates. Bytes count logical HTTP/1 field lines
(name + ': ' + value + CRLF); the terminating empty line is outside this budget.
This is also a conservative logical-size bound for HTTP/2, not an HPACK wire bound.
`entries()`, case-insensitive `membership`, `first`, `values` and persistent
`addition` preserve the limits. Missing first throws MissingHeader(name); values
returns an empty sequence. No automatic comma-join, including for Set-Cookie.
`limits()` exposes the immutable budget for composing new representations.

`HeadersWithout(origin,names is Sequencing<HeaderName>).result()` removes all
matching entries, preserving others' order. Names are snapshotted and bounded
by the origin's entry limit. A sequence replaces the old Set candidate because
structural equality of case-preserving names is not HTTP name identity.
`ReplacedHeader(origin,entry).result()` removes all matching fields then appends
one new entry. Absence therefore means append; no unrelated field is reordered.
These are pure immutable representations; original values never change.

`Http2Headers(origin,request is bool).result()` validates a logical HTTP/2 field
list and returns lowercase names. Connection, Proxy-Connection, Keep-Alive,
Transfer-Encoding and Upgrade are forbidden. TE is allowed only in a request
with value `trailers` (case-insensitive). Pseudo-fields are outside HeaderName and
must be handled by the eventual message/framing layer. This is not HPACK or an
HTTP/2 encoder/decoder, negotiation or a claim of complete message validity.

Typed errors: InvalidHeader(name,reason), MissingHeader(name),
HeaderLimitExceeded(kind); invalid limits use InvalidArgument. Constructors and
observations perform no hidden I/O or resource acquisition; there is no close,
async/cancellation effect or native state. Repeated observations are stable.
Snapshot memory is bounded by explicit entries/bytes; matching is linear, and
removal costs at most entries times selected names.

Basis: [RFC 9110 fields](https://www.rfc-editor.org/rfc/rfc9110.html#section-5),
[RFC 9113 fields](https://www.rfc-editor.org/rfc/rfc9113.html#section-8.2).

HeaderName stores canonical ASCII spelling at construction while string() keeps
its original spelling. Repeated matching observes the cached value; object equality
and case-insensitive ordering remain unchanged. Measured motivation
and 58-file /523-assertion HTTP compatibility gate PASS; native unchanged.
