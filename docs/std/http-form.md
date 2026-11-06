# HTTP forms

`http/form` parses an owned, already HTTP-delimited `BodyReading`. Construction
is lazy; successful multipart construction adopts the source. Use
`ParsedMultipartForm(source,boundary,FormLimits()).parts()` and close the returned
`MultipartParts` in a defer. One active operation and one current part are
allowed. Read the part to EOF before advancing; `MultipartPartUnread` preserves
the current part. `MultipartFormEnded` follows the closing boundary and complete
HTTP EOF. Early close aborts the form; `await parts.settled()` joins pending work.

`FormPart` exposes its name, file discriminator, untrusted display filename,
HTTP `Headers`, and streaming `BodyReading`. HTTP octet headers replace the old
v3 candidate `MessageHeaders`: that mail contract is ASCII-only and cannot
represent UTF-8 form-data metadata. Field/file names are strictly decoded UTF-8
without percent decoding or path interpretation. Extended disposition parameters
and Content-Transfer-Encoding are rejected. Duplicate field names retain order.

Defaults: 1000 fields, 100 files, 1 MiB per field, 64 MiB per file and 128 MiB
total wire bytes. The aggregate includes headers, preamble and epilogue. Explicit
nonnegative BigInt byte budgets and zero quotas are accepted; counts are capped
at 65535, byte coordinates at 9007199254740991. Boundary length is 1..70 ASCII
bytes, headers 16 KiB/64 fields, preamble 8 KiB and delimiter padding 256 bytes.
The parser retains a fixed chunk/window, never an entire file. False delimiter
prefixes remain payload. Quota failure closes the source before publication.

`await FormText(part).text()` explicitly materializes at most 1 MiB of one field.
UTF-8, US-ASCII and ISO-8859-1 are supported; undeclared charset defaults to
US-ASCII. The explicit `(part,charset,maximum)` overload can select an undeclared
default, but cannot override a declaration. There is no charset guessing or
implicit `_charset_` application. File parts cannot be text projections.

`await ParsedUrlEncodedForm(source,limits).parameters()` reuses `url/form` strict
UTF-8/percent decoding and ordered immutable parameters. Its one lazy Future
owns the source; encoded input is explicitly capped at 1 MiB and aggregate wire,
field count, decoded value and name budgets apply. Repeated calls share a result.

`await FormFileSpooling(part,directory,prefix).file()` is the only automatic
file-storage adapter: acquisition is lazy, copying uses 8 KiB chunks, the directory
and prefix are explicit, and client filename never selects a path. Cancellation
or failure deletes the temporary file. The returned `TemporaryFile` owns deletion
on close and explicit `TemporaryFilePreservation`; its cursor is at the written
end. Synchronous bounded disk writes follow the existing Stage 11 contract.

The [runnable example](../../examples/std-v4/http-form.ok) demonstrates UTF-8 text.
[RFC7578](https://www.rfc-editor.org/rfc/rfc7578.html) is the wire reference.
No interpreter/provider or language syntax changed.


URL/spool result wrappers explicitly register their memoized operation in a FutureGroup. Cancelling the outer result joins child cleanup before settling; red/green evidence verifies both source closure and temporary-file deletion.
