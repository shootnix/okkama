# Query, fragment and form contracts

UrlQuery/UrlFragment retain their original component contract below. The full
[URL/authority/reference API](url-value.md) is now verified using the actual
[HostName/IDNA capability](host-policy.md); no hostname substitute is introduced.

UrlQuery()/UrlFragment() represent absence. Their string-taking constructors
represent presence, even for an empty string. string() omits the introducing
question mark/hash. Construction validates component syntax and stores canonical
ASCII: uppercase escapes, escaped unreserved ASCII decoded, reserved escapes
preserved. Raw Unicode uses the RFC 3987 ucschar ranges (plus iprivate only in
query) and is escaped as UTF-8. No normalization, sorting or recursive decoding.
Raw whitespace, controls, hash, brackets and backslash are invalid here; callers
encode data explicitly with PercentEncoded. Escaped arbitrary octets remain
valid even if not UTF-8. InvalidUrl reports the original component scalar offset
and reason. Equality includes presence and canonical text.

Syntax references: [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986.html#section-3.4),
[RFC 3987](https://www.rfc-editor.org/rfc/rfc3987.html#section-2.2).

QueryParameter(name) is unassigned; QueryParameter(name,value) is assigned.
name/value are exact Unicode strings. value() on an unassigned parameter is
empty. Empty names, duplicate names and empty values are allowed.
QueryParameters snapshots a Sequencing<QueryParameter> once (size once, items
once in order), retains only entries and returns an immutable sequence.
membership/first/values compare exact names; first throws MissingQueryParameter
when absent, values returns an empty sequence. No all() alias is added.
string() preserves order/assignment and uses PercentEncoded(data), joined by &.

ParsedQueryParameters(UrlQuery).value() explicitly interprets that query as a
parameter list: split only at literal &, then the first literal =, then decode
each side once as strict UTF-8. Plus stays literal; semicolon is data. Absent or
empty query yields no entries. In nonempty queries, empty segments become empty
unassigned parameters. A singleton empty unassigned parameter prints empty text
and cannot be distinguished from no entries on reparsing; the value API retains
the distinction. InvalidQueryEncoding reports the byte offset in the canonical
ASCII query supplied to the parser, including the original percent position
for a decoded UTF-8 error. Malformed escapes are validated before UTF-8 per field.

FormUrlEncoded(QueryParameters) emits UTF-8 application/x-www-form-urlencoded:
ASCII alphanumeric and *-._ stay literal, space becomes +, plus becomes %2B;
every entry prints name=value (unassigned values serialize as empty assignments).
ParsedForm(@byte).parameters() splits literal & and first = before replacing +
with space and decoding escapes. Empty segments are skipped; a bare name becomes
an assigned empty value. Duplicates/order are retained. Unlike forgiving browser
decoding, this API rejects malformed escapes and malformed UTF-8, with
InvalidFormEncoding.offset() pointing into the original byte input, not decoded
text. NUL and escaped delimiters are ordinary field data.
The serialization set follows the
[URL Standard](https://url.spec.whatwg.org/#application/x-www-form-urlencoded).

Defaults: at most 10000 parameters; parsers at most 1048576 input bytes.
QueryParameters(entries,maximum_parameters), ParsedQueryParameters(query,
maximum_parameters,maximum_bytes), and ParsedForm(bytes,maximum_parameters,
maximum_bytes) permit explicit nonnegative limits. InvalidArgument covers invalid
limits/negative source size; QueryLimitExceeded covers input/cardinality excess.
Limits precede source item observation, splitting and decoding respectively.
String contents are retained coordinates, not constrained by the entry count.
Parsing and printing use memory proportional to finite input/output; no parser
does repeated suffix scans or per-byte growing-array copies.
Parsers retain inputs and validate when observed; component values validate at
construction. Failures from source size/item propagate unchanged. No I/O,
native URL parser, ambient policy or new dependency.
