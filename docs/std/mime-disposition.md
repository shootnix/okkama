# MIME content disposition

ContentDisposition(text) parses an unfolded ASCII disposition field body using
the same token/quoted-string/RFC 2231 grammar as mime/media_type. Extension kinds
remain valid and are lowercased. ContentDisposition(kind,MimeParameters) composes
the same immutable value. kind(), parameters() and string() expose canonical
values; rendering is a logical field body, not a complete header or HTTP policy.

The parser defaults to 256 wire parameters and 1048576 input bytes. The overload
ContentDisposition(text,maximum_parameters,maximum_bytes) replaces those bounds;
it has different arity from the composition constructor. MimeLimitExceeded and
InvalidArgument preserve the shared limit contract. Lexical/assembly failures
become InvalidContentDisposition with the same offset and reason.
MissingContentDisposition is the explicit domain diagnostic for an absent field;
it is available to consuming message APIs, not inferred from an empty string.
An empty supplied field is invalid syntax.

Filenames are plain decoded strings. Relative paths, separators, drive-looking
text and dot segments are retained as received names; no Path conversion, join,
normalization or filesystem access occurs. Applications must choose their own
storage name. Extended filename values and fallback precedence follow the
[shared MIME contract](mime-media-type.md), including strict charset handling.
No IANA lookup, attachment download or automatic filename sanitization.
