# MIME transfer bodies

`mime/transfer` represents a deferred observation of a borrowed `ByteSource`
under an explicit `TransferEncoding`: `seven_bit`, `eight_bit`, `binary`,
`base64`, or `quoted_printable`. `TransferEncodedBody(origin, encoding)` and
`TransferDecodedBody(origin, encoding)` implement `ByteSource`. Construction
does not read; each `bytes()` calls the origin exactly once and transforms
that snapshot. Repeated calls observe the source again. Neither adapter closes
the source; source errors propagate unchanged. Owned scratch buffers close on
success or failure. Adapters add no independent I/O; source observation may have
its own effects. There is no charset conversion or newline normalization.

Both constructors also accept `maximum_bytes` as a third argument (default
16 MiB). Negative limits throw `InvalidArgument` before source observation.
Input and output independently must fit the limit, otherwise
`TransferLimitExceeded(kind)` reports `input` or `output`. The source itself
materializes before its input size can be checked. Scratch/output allocation
is bounded by a constant multiple of accepted input; this is a materializing
adapter, not a streaming codec. Empty input is valid even at a zero limit.

Profiles follow [RFC 2045 sections 2.7–2.9 and 6.7–6.8](https://www.rfc-editor.org/rfc/rfc2045.html):
7bit/8bit preserve bytes, forbid NUL and bare CR/LF, and limit each line to
998 octets excluding CRLF; 7bit additionally forbids bytes above 127. A final
unterminated line is accepted. Binary preserves arbitrary bytes.

Base64 encoding reuses `encoding/base64` with the standard alphabet and
padding, inserts CRLF between lines of at most 76 characters and adds no final
CRLF. Decoding uses a deliberately strict transport profile: SP/HTAB and CRLF
may separate encoded characters, each physical line is at most 76 bytes,
other nonalphabet bytes and bare CR/LF fail. Unlike the permissive RFC decoder,
arbitrary junk is not discarded. Padding and unused bits remain strict codec
checks. Quoted-printable delegates to `encoding/quoted_printable`, including
its strict physical-line, escape and trailing-whitespace rules.

Malformed payloads throw `InvalidTransferEncoding(offset, reason)`; `offset()`
is an original input byte offset, including skipped Base64 whitespace. An
incomplete final quantum reports input size. There is no transfer-token parser
here: `mime/part` rejects unknown tokens when constructing or parsing a MIME entity.
