# Static HTTP files

`StaticFiles(root, media, policy)` implements `Handling` and borrows an explicitly
acquired [`FileRoot`](fs-root.md). The fourth argument optionally supplies
`Randomizing`; a fifth supplies `Timing`. Short constructors explicitly select
the standard OS secure-random source and wall clock. The complete constructor
supports reproducible boundary and date tests without changing the interpreter.
The handler never closes the borrowed root. Each returned file body owns an
independent descriptor, so closing the root does not revoke that response.

`StaticPolicy()` selects `index.html`, disables listing/precompressed siblings,
and bounds ranges at 16 and directory entries at 1024. The three-argument
constructor supplies index names, listing and precompressed flags. The five-
argument constructor additionally bounds ranges (0..128) and names (0..4096).
Index configuration is at most 32 single decoded coordinates. There is no CGI,
template execution, extension-based process invocation or ambient MIME lookup;
the supplied `MediaIdentifying` capability controls media policy.

GET/HEAD support origin and HTTP absolute targets. Raw path segments are split
before exactly one UTF-8 percent decode; dot coordinates, empty interior
segments, decoded separators/NUL/controls are rejected with 400. The native root
rejects every symlink component. Missing/denied/nonregular paths return 404.
Non-GET/HEAD requests return 405 with `Allow: GET, HEAD`. A directory without a
trailing slash redirects with 308 to the validated origin path plus its original
query; a double-slash authority cannot be reflected in Location. Index selection
uses the same descriptor boundary. Without an index, listing defaults to 403.

Metadata comes from the selected descriptor, before any content read. Responses
have Date, Last-Modified, Content-Type, Content-Length, Accept-Ranges and a weak
ETag derived from device/inode, size, whole-second modification/change times and
the selected coding. Last-Modified cannot exceed the sampled Date. The supplied
clock also determines obsolete RFC850 century interpretation. Malformed dates
are ignored; tag syntax errors are 400. Preconditions follow RFC9110 ordering:
If-Match, otherwise If-Unmodified-Since; If-None-Match, otherwise If-Modified-Since.
Conditional decisions precede ranges. HEAD/304/412 close without `pread`.

GET supports bounded byte ranges, clipped/coalesced by `http/conditional`.
Unsatisfiable ranges return 416 and `Content-Range: bytes */size`; one range
returns 206 with inclusive Content-Range. Multiple ranges stream part headers,
descriptor ranges, CRLF and a final boundary, with an exact aggregate length.
The default multipart boundary uses 192 fresh random bits; applications can
provide an explicit random capability. Unknown/malformed/over-budget ranges are
ignored; HEAD ignores Range. The metadata ETag/date cannot certify strong byte
identity under in-place mutation, so any If-Range receives the full 200 response.

Precompressed delivery is opt-in and serves existing `.br`/`.gz` bytes without
running a codec. Only regular siblings whose modification time equals the source
are eligible. The trusted publisher must generate correct companion contents
and publish matching timestamps atomically; timestamps do not prove that bytes
decompress to the source. Accept-Encoding quality/identity rules choose among
the actual eligible representations, with 406 for no acceptable choice. The
source filename determines Content-Type; the selected descriptor determines
Content-Length/ETag; Content-Encoding and Vary are explicit. Byte ranges address
the selected encoded representation.

An enabled directory listing bounds names and output to 1 MiB, separately
percent-encodes href coordinates and HTML-escapes display text/quoted attributes.
It is a bounded UTF-8 snapshot with `Cache-Control: no-store`.

File bodies are known-length, nonreplayable and one-shot. Reads are bounded at
64 KiB, maintain positional offsets and accept legal positive short reads;
premature EOF throws `StaticFileChanged` and closes the descriptor. Close is
idempotent, and failure/cancellation destruction closes the owner. No whole-file
snapshot is allocated. Filesystem calls remain synchronous (PERF-35); descriptor
safety does not freeze contents. Trusted atomic publication is the intended
deployment model, including the limits of whole-second weak validators.

The wire reference is [RFC9110 precondition order](https://www.rfc-editor.org/rfc/rfc9110.html#section-13.2.2)
and [range responses](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.3.7).
