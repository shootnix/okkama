# MIME media type and parameters

The executable module is `mime/media_type` (module identifiers do not permit the
hyphen used in the historical `mime/media-type` queue heading).

MimeParameter(name,value) retains a decoded Unicode value and lowercase ASCII
token name. Logical names cannot contain '*', which is reserved for wire
continuations. Values reject ASCII controls except HTAB, and DEL; CR/LF are not
header folding here. MimeParameters snapshots ordered entries once, preserving
duplicates and unknown names. membership/first/values lowercase lookup names;
first throws MissingMimeParameter, values returns an empty sequence.

MediaType(type,subtype,parameters) validates and lowercases tokens, rejecting
wildcard '*' types/subtypes (those belong to media ranges). MediaType(text)
eagerly parses the same value from an unfolded ASCII field body: optional SP/HTAB,
type/subtype and semicolon-separated parameters, with strict token/quoted-string
values and quoted-pair unescaping. No Content-Type prefix, CRLF, comments,
obsolete syntax or raw non-ASCII wire text is accepted by this first parser.
Use MessageHeader.value() for unfolding and composed parameters for Unicode.

RFC 2231 extended parameters support UTF-8 and US-ASCII. An empty charset label
allows ASCII bytes only, without inventing a charset for non-ASCII bytes.
Language labels are syntax-checked ASCII letters/digits/hyphen and not retained
as a presentation policy. Numbered sections may arrive out of order; require
unique contiguous indices starting at zero without leading zeros. Encoded and
unencoded sections can mix. Decode percent bytes per encoded section, concatenate
bytes, then decode UTF-8 once (a scalar may span sections). Malformed escapes,
gaps, duplicate indices, duplicate singleton extensions and conflicting singleton
versus continuation representations are rejected. A valid extended/continued
value replaces plain fallbacks at the name's first wire occurrence. Plain-only
duplicates retain their order. Invalid extended values never silently fall back.

Serialization emits lowercase type/subtype/name, ordered parameters, safe token
values or escaped quoted strings; non-ASCII values use UTF-8 extended parameters.
No parameter sorting, registry lookup, charset conversion or MIME encoded-word
substitution. Serialization is a logical field body; header line limits remain
the responsibility of RenderedMessageHeaders. Long continuation generation is
not automatic in this slice.

Defaults: 256 source/wire parameters and 1048576 input bytes. Explicit overloads
MimeParameters(entries,maximum_parameters) and MediaType(text,MediaTypeLimits(
maximum_parameters,maximum_bytes)) replace nonnegative bounds. InvalidArgument covers invalid limits
and negative sequence sizes; MimeLimitExceeded covers excess. Parsing checks byte
limits before grammar work and parameter count before consuming the next entry.
Grouping is O(n²) within the supplied parameter bound; scanning is linear in text.
InvalidMediaType.offset() is a source scalar offset for lexical errors and the
start of the offending parameter for assembly/charset failures. Composed-value
errors refer to their supplied token/value. No I/O or native parser is added.

Sources: [RFC 2045](https://www.rfc-editor.org/rfc/rfc2045.html#section-5.1),
[RFC 2231](https://www.rfc-editor.org/rfc/rfc2231.html#section-3).
