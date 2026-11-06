# Percent encoding

PercentEncoded(origin, component).string() encodes a Unicode string as UTF-8
octets, escaping every octet outside the selected ASCII set with uppercase
%HH. A literal percent is always escaped, including in an existing triplet.
This is encoding of decoded component text, not URL parsing or normalization.

UrlComponent selects these preserved ASCII sets (U = letters, digits, -._~;
S = !$&'()*+,;=):

| Component | Preserved |
| --- | --- |
| data | U only; suitable for an individual query name/value |
| userinfo | U + S + : |
| path_segment | U + S + :@ |
| path | U + S + :@/ |
| query, fragment | U + S + :@/? |

`query` encodes a whole query, preserving its separators; use `data` for a
single parameter name/value. No host, IDNA, scheme or form-encoding mode is
implied. Whole-path encoding preserves slash and dot segments; resolving a
relative reference and disambiguating its first segment belong to URL values.
The sets follow [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986.html#appendix-A).

PercentDecoded(origin).bytes() replaces each %HH exactly once. Both ASCII hex
cases are accepted. Bare Unicode characters contribute their UTF-8 bytes;
literal plus, spaces, slashes and controls are preserved. This decoder does
not validate URL grammar. string() strictly decodes the resulting bytes with
Utf8Decoded, preserving InvalidUtf8 and its decoded-byte offset. Binary bytes
remain available even when string() fails. No replacement or normalization.

Malformed or incomplete escapes throw InvalidPercentEncoding at the Unicode
scalar offset of the introducing percent, including after non-ASCII text.
The first malformed escape wins. Construction only retains inputs; validation
and conversion occur on bytes()/string(). Empty values return empty results;
repeated observations are deterministic and do not share mutable output.

The codec traverses the supplied finite input with output storage proportional
to input size. Encoding produces at most three output characters per UTF-8
octet; decoding never exceeds the input's UTF-8 byte count. There is no hidden
I/O or native URL parser; existing UTF, hex and buffer capabilities are reused.
