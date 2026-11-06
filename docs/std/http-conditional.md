# HTTP conditional values

`http/conditional` begins with immutable validators, not a request handler.
EntityTag(value is string, weak is bool) validates the opaque octet string:
`!`, ASCII 0x23..0x7E and Latin-1 0x80..0xFF. Empty values, commas and literal
backslashes are valid; spaces, quotes, DEL, controls and non-Latin-1 are not.
`value()` observes the opaque validator identity, `weak()` its validation strength,
and `string()` emits the exact quoted protocol representation. No unescaping.
`strong_matches(other)` requires both validators strong and equal opaque values;
`weak_matches(other)` compares opaque values regardless of strength. Ordinary
language equality retains both coordinates and is not HTTP weak comparison.

`EntityTags()` denotes the wildcard; `EntityTags(values is Sequencing<EntityTag>)`
denotes an ordered immutable snapshot, including the empty list. The sequence
overload also accepts explicit ConditionalLimits. `values()` returns Sequencing<EntityTag>;
`wildcard()` identifies the wildcard. Rendering preserves duplicates/order with
canonical comma-space separation. Strong/weak matching succeeds for any matching
member, or wildcard. The caller must already have an existing representation:
wildcard does not discover resource existence. An empty list matches nothing.

`ParsedEntityTags(origin[,limits]).value()` parses one combined field value.
Only SP/HTAB are OWS; `W/` is case-sensitive and adjacent to the quote. Commas
inside tags are opaque data. Empty comma-list members are accepted and omitted;
wildcard cannot be mixed with a list, including empty members. Canonical
rendering does not generate empty members. Parsing is deferred and stable.

ConditionalLimits defaults to 8192 wire octets and 128 list slots; both budgets
are nonnegative. Empty wire/list fits zero budgets. Every comma-delimited slot,
including an ignored empty slot, uses the entry budget. A wildcard uses one slot.
The wire budget is checked before encoding/scanning; a one-scalar-per-octet
Latin-1 representation is required. Snapshot construction checks declared source
size before item observation and checks canonical wire length. No implicit
trimming, network I/O, host parser, downcast or arbitrary serialization.

InvalidEntityTag(offset,reason) identifies a bad scalar/octet position;
ConditionalLimitExceeded(kind) identifies `bytes` or `entries` budget exhaustion.
Negative budgets/source size use the existing InvalidArgument contract.
These are explicit failure observations, not reflection over private fields.

## Byte-range contract

ByteRange(start is BigInt,end is BigInt) denotes a nonempty inclusive interval:
0 <= start <= end. start/end/size are exact BigInt observations; string() emits
canonical `start-end`. ByteRanges(origin[,ConditionalLimits]) denotes a validated
bytes range plan, independent of a representation. Construction accepts bounded,
open and suffix decimal forms; canonical storage normalizes decimal zeroes/unit
case, retains order and duplicates, and renders with comma-space separators.
Only SP/HTAB are OWS; whitespace inside numerals or around `-`/`=` is invalid.
Recipient empty list slots are ignored but charged; at least one range is needed.
Wire and canonical-output budgets and entry budgets are validated before serving.

ranges(size is BigInt) requires a known nonnegative representation length. It
clips endpoints, discards forms containing no concrete bytes, sorts ascending
and coalesces overlapping or adjacent intervals. Results are immutable
Sequencing<ByteRange>; no arithmetic is narrowed to signed-64 offsets.
satisfiable(size) preserves the RFC rule for a positive suffix even when size is
zero; ranges(0) contains no intervals because there are no bytes. Consumers must
not infer HTTP satisfiability solely from the interval count. Serving an empty
representation may ignore Range under RFC 9110 14.2. Unknown length is not `-1`.
InvalidRange(reason) covers grammar and contradictory/negative coordinates.
Work depends on bounded wire/member count and numeric widths, not representation
length; no body bytes, content decoding, network or file access occur.
Basis: [RFC 9110 byte ranges](https://www.rfc-editor.org/rfc/rfc9110.html#section-14.1.2).

Full precondition evaluation is a subsequent task. Date precedence,
unknown-length representations and multipart serving belong to actual protocol
consumers; this validator slice does not claim those behaviors implemented.
Basis: [RFC 9110 entity tags](https://www.rfc-editor.org/rfc/rfc9110.html#section-8.8.3),
[list recipients](https://www.rfc-editor.org/rfc/rfc9110.html#section-5.6.1.2),
[If-Match](https://www.rfc-editor.org/rfc/rfc9110.html#section-13.1.1).
