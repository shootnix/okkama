# Property-list encoding

## Contract before implementation

EncodedPlist(origin is PlistValue, format is PlistFormat) retains dependencies;
bytes() is @byte encodes afresh. PlistFormat is (xml, binary). A five-argument
overload adds maximum_bytes, maximum_nodes, maximum_depth (defaults 1048576,
4096, 64). Negative limits fail at construction with InvalidPlistWriteLimits.
PlistWriteLimitExceeded.kind() identifies bytes/nodes/depth. Root depth is one;
dictionary keys count as nodes at their containing value's child depth.

Traversal uses only PlistValue.print(PlistWriting), never kind(), reflection or
casts. Every value must issue exactly one event; zero/multiple events throw
InvalidPlistPrintProtocol. Custom values work nominally. Callback/source errors
propagate unchanged; repeated calls have fresh buffers/budgets. Recursive custom
values stop at node/depth limits. Partial output stays private on failure.

Container sources are borrowed: size once, reject negative/excess count before
items, then item(i) once in ascending order, without cursor/first/last/close.
Negative size is InvalidPlistWriteSource. Dictionary insertion order and exact
names are preserved; duplicate names throw DuplicatePlistMember. No sorting or
deduplication. Shared children are observed per occurrence, not memoized.

XML is UTF-8 with `<?xml version="1.0" encoding="UTF-8"?>` plus LF, one compact
`<plist version="1.0">` wrapper and closing LF. No DOCTYPE or indentation. Empty
containers/strings/data use paired tags; booleans use empty tags. Integers retain
arbitrary signed precision; finite real values use the language's numeric spelling.
Text/key characters obey XML 1.0: escape &, <, > and literal CR (as &#13;) so parsing
preserves line endings. Base64 uses the standard padded alphabet without whitespace.
UIDs and invalid XML characters are UnsupportedPlistValue with reason()/text().
Dates print RFC3339 at offset zero, preserving integral nanoseconds.

Binary output is deterministic bplist00 with postorder object records, no identity
deduplication, fixed eight-byte references and offsets, then a 32-byte trailer.
Lengths use inline counts below 15 and unsigned integer length records otherwise.
Integers use unsigned 1/2/4-byte or signed 8/16-byte records, choosing the smallest
supported width; values outside signed 128-bit fail. UIDs use 1..16 unsigned bytes.
Reals use binary64; strings use ASCII when possible, otherwise UTF-16BE code-unit
counts. Data is exact bytes; dictionaries emit key refs then value refs.

Date support in both formats is UTC years 0001..9999. Binary dates store binary64
seconds from 2001-01-01. The writer checks conversion back through the existing
float.rat()/Instant contract and rejects changed nanoseconds instead of rounding.
All selected-format restrictions use UnsupportedPlistValue. Model values remain
unrestricted by these serialization limits.

The byte budget includes envelope, object records, offsets and trailer. Preflight
scalar/container sizes before encoding allocation; temporary buffers remain
bounded by byte/node/depth budgets. Writer-owned buffers close on success/failure;
the returned immutable byte snapshot outlives them. No external I/O, flush/close
of caller resources, new providers or interpreter changes.

Sources: [Apple XML DTD](https://www.apple.com/DTDs/PropertyList-1.0.dtd),
[CoreFoundation binary format](https://github.com/apple/swift-corelibs-foundation/blob/main/Sources/CoreFoundation/CFBinaryPList.c).

- [x] RED fixtures, format restrictions, protocol/lifetime/budget tests.
- [x] Pure Okkama implementation, comments and embedded tests.
- [x] GREEN focused suites, independent wire checks, example and roadmap/API updates.

Evidence: 33 writer groups, 46 with the
value model, four embedded groups and independent example decoding PASS.
