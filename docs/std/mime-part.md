# MIME parts

`MimeBody` has `bytes() is @byte` and `validate(type is MediaType) is void`.
It represents decoded body bytes; validation checks compatibility with declared
content metadata before observing those bytes. `BinaryBody(@byte)` is an
immutable snapshot usable for nonmultipart media types. `TextBody(string,charset)`
eagerly validates/encodes text and exposes string(), charset(), bytes(); it
requires a text media type with the same charset (absent means US-ASCII).
Supported labels are UTF-8, US-ASCII, ISO-8859-1, UTF-16BE/LE and UTF-32BE/LE.
Byte order is explicit, no BOM insertion/guessing, replacement or newline
conversion. UnsupportedMimeCharset retains the unsupported label. Codec
representability failures preserve their existing typed errors.

`MimePart(headers,body)` borrows a body capability and retains immutable headers.
Construction validates metadata/compatibility without calling body.bytes().
Each bytes() observation reads the body once, applies the declared transfer
encoding, and renders headers + CRLF + encoded body. Neither source nor body is
closed; scratch buffers are owned and closed on every path. An optional third
MimeLimits controls serialization. Repeated calls may observe effects of a
custom body; BinaryBody/TextBody/parsed parts are deterministic.
The root part's limits govern the entire rendering operation, including children;
descendant construction/standalone-render limits are not additional budgets.
The four-argument constructor `(headers,body,limits,default_type)` retains an
explicit enclosing-context default without synthesizing a header (used for
multipart/digest). `default_type()` exposes that immutable parsing coordinate.

An explicit `MultipartBody.part(headers[,limits[,default_type]])` factory retains
its concrete rendering capability in the resulting MimePart. Known explicit
multipart descendants reuse checked subtree accounting; leaf/opaque children
still undergo bounded representation validation with their enclosing default
and charset. `MimePart.multipart(body is MultipartBody)` returns an immutable
successor with a validated replacement body and that capability. The existing
MimeBody role and all old MimePart constructors keep their narrow contract.

`MimePart.headers(headers,bounds)` creates a validated immutable part with new
headers, retaining its body capability, default type and structural evidence.
`limits()` exposes the part's bounds. Parsing preserves the known structure;
parsed multipart rendering does not parse each nested multipart again.
Opaque custom bodies and legacy role-only constructors still use the existing
representation path. Standalone MultipartBody.bytes() retains its own profile;
explicit part rendering uses enclosing limits across known descendants.

`MimeContent(part)` exposes type(), disposition(), encoding(). Unknown headers
and their ordering remain untouched. Content-Type, Content-Disposition and
Content-Transfer-Encoding cannot occur more than once. Absent type defaults to
text/plain;charset=US-ASCII, absent encoding to seven_bit; absent disposition
throws MissingContentDisposition. Unknown encoding tokens fail. Structured MIME
field bodies use the existing strict grammar, independently of encoded-words.

`MultipartBody(subtype,boundary,parts)` eagerly snapshots its sequence once;
parts must be nonempty. The optional fourth argument is MimeLimits. A six-argument
constructor also receives preamble/epilogue byte snapshots before limits.
subtype(), boundary(), cursor() expose in-memory traversal, not blocking I/O.
bytes() renders each child once and checks boundary collisions before returning
the complete result. validate() requires matching multipart subtype and boundary.
Only identity transfer profiles are allowed for enclosing multipart entities.

Delimiter syntax follows [RFC 2046](https://www.rfc-editor.org/rfc/rfc2046.html):
boundary text is bounded ASCII, at most 70 characters, with no trailing space.
Delimiters are recognized at line starts. A delimiter's preceding CRLF is not
child data. Strict parsing requires a closing delimiter, accepts transport
padding, and rejects malformed boundary-prefix lines. Preamble and epilogue are
retained separately. Serialization uses CRLF and checks collision prefixes,
including nested boundaries. `multipart/digest` children default to message/rfc822;
encapsulated message payloads remain opaque bytes in this slice.

`MimeLimits()` defaults: header_bytes=65536 per entity (including separator),
line_bytes=998, parts=1000 including root, depth=16 with root at zero,
body_bytes=16777216. Its five-argument constructor replaces nonnegative bounds;
header line limits cannot exceed 998. body_bytes independently caps total input
wire bytes, each rendered entity, and cumulative decoded leaf bytes. Parsing
checks bounds before descent/append. Arrays are materialized, with memory bounded
by input and nesting limits; this is not a streaming parser. `MimeLimitFailure(kind)`
distinguishes resource limits. `InvalidMime(offset,reason)` uses original wire
byte positions for syntax/transfer errors, entity starts for metadata and body
starts for decoded charset failures. Negative source sizes throw InvalidArgument.
Header parsing removes leading SP/HTAB after the colon, retains trailing value
whitespace, unfolds continuations, and requires an explicit empty separator line.
Delimiter scanning visits bytes without splitting the full payload into lines;
header continuation fragments assemble once within the per-entity header budget.

`ParsedMime(@byte[,MimeLimits]).result()` parses once per observation and returns
an immutable `MimeParsedNode`. Its part(), nodes(), decoded() retain the parsed
part and aggregate accounting; children() is an ordered @MimeParsedNode of direct
multipart children, empty for a leaf. Store the explicit result when performing
several tree observations: children() never serializes or reparses. Parsing
checks limits and decoded-byte accounting as before.

`ParsedMime(@byte[,MimeLimits]).part()` delegates to result().part(), returning immutable
headers, decoded leaves and multipart children. No I/O, filesystem extraction,
HTML interpretation, charset guessing or automatic encoded-word substitution.
`GeneratedBoundary(random is Randomizing[,parts is Sequencing<MimePart>])`
observes randomness eagerly; the second overload rejects candidates occurring
in serialized children, with 32 attempts and a 16 MiB cumulative snapshot budget.
Its string() is stable. Use SecureRandom
for production randomness; multipart rendering always rechecks actual bytes.
