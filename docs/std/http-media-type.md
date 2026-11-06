# HTTP media type values

HttpMediaType(origin is string[,limits is HttpMediaLimits]) denotes a concrete
Content-Type value. The coordinate constructor binds type/subtype strings and
HttpMediaParameters. Tokens and parameter names use HTTP tchar and canonical
ASCII lowercase. Exact `*` type/subtype is rejected for a concrete value; wildcard
ranges are subsequent consumers of the scanning infrastructure.

HttpMediaParameter(name,value) validates an HTTP token name and Latin-1 value.
Values reject controls except HTAB and DEL; quotes/backslashes are escaped on
output. Charset values must be tokens and are ASCII case-normalized; other values
remain exact. Token values serialize bare, others as HTTP quoted-string.
Extended names containing `*` are retained literally: no MIME RFC 2231 assembly,
charset decoding or percent decoding occurs in this protocol value model.

HttpMediaParameters(values is Sequencing<HttpMediaParameter>[,maximum is int])
snapshots once, sorts names and rejects duplicates. entries()/membership()/first()
observe the immutable parameter association; lookup uses binary search over
canonical names (5/8 name observations for 16/128 parameters). This measures
lookup steps, not overall interpreter cost; existing snapshot/ownership overhead
remains in the performance backlog.
Missing lookup is
MissingHttpMediaParameter. Canonical ordering and quote-independent storage make
structural identity independent of parameter order and wire quoting. A type
observes type()/subtype()/parameters() and emits canonical string(). It does not
advertise a renderer, perform I/O or discover a registry.

HttpMediaLimits defaults to 8192 input octets and 128 raw parameter slots.
Explicit budgets are nonnegative. Input bytes are checked before decoding and
scanning; source parameter size is checked before item observations. Empty
semicolon parameters are accepted and charged to the raw slot budget. Only
SP/HTAB are whitespace. No whitespace around `/` or `=` is admitted. Quoted
values decode HTTP quoted-pairs, including obs-text, and reject unterminated
quotes/escapes or unquoted invalid characters. Limits bound syntax work, not a
portable execution deadline. Canonical output can add quotes/escapes.

InvalidHttpMediaType(offset,reason) and HttpMediaLimitExceeded(kind) are typed
errors. HttpMediaScanning/HttpMediaFields/HttpMediaWireParameter/position/items
are importable grammar infrastructure; wire quoting/source positions are kept
there for the eventual Accept qvalue interpretation, not in public parameter
identity. A future Accept consumer recognizes q independently of parameter order.
No existing MIME API or interpreter behavior changes.
Basis: [RFC 9110 media types](https://www.rfc-editor.org/rfc/rfc9110.html#section-8.3.1),
[parameters](https://www.rfc-editor.org/rfc/rfc9110.html#section-5.6.6),
[quoted-string](https://www.rfc-editor.org/rfc/rfc9110.html#section-5.6.4).
