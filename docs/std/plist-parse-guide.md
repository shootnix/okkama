# Property-list parsing

## Contract before implementation

ParsedPlist(origin is @byte) retains immutable bytes and parses on each projected
value() is PlistValue. An overload accepts maximum_input, maximum_nodes and
maximum_depth (defaults 1048576 bytes, 4096 visited nodes and 64 levels).
Negative budgets are InvalidPlistLimits at construction. InvalidPlist keeps a
zero-based original byte offset() and text(); malformed, unsupported and budget
failures are typed. Input bounds precede decoding. Root depth is one; dictionary
keys and repeated binary references count as visited nodes. Snapshots survive the
parser's scope. No I/O, resources, interpreter edits or new native providers.

Detection recognizes bplist00 or UTF-8 XML (optional UTF-8 BOM). Other binary
versions and encodings fail explicitly. XML requires one plist wrapper containing
one value, version absent or 1.0. Supported values: true/false, signed decimal
integer, finite real, string, base64 data, UTC date, array and ordered dictionary.
Empty strings/data/containers are valid; dict keys must alternate with values.
Duplicate keys fail. XML text supports five predefined entities, numeric character
references, CDATA, comments and XML line-ending normalization. Scalars cannot
contain child elements. XML 1.0 character restrictions apply; references preserve
referenced CR. Dates require full RFC3339 UTC seconds, with optional nanoseconds.
Real/integer text is limited to 4096 characters; real decimal exponent magnitude
is limited to 4096 and values outside finite binary64 are rejected.

This is a strict plist XML dialect, not a general XML processor: element attributes
other than plist version, arbitrary processing instructions, custom DTDs/internal
subsets and entity declarations are rejected. A UTF-8 XML declaration and the
standard Apple public/system DOCTYPE are accepted only in the prolog and never
fetched. Whitespace/comments may surround the root. No external entity expansion.

Binary parsing validates trailer widths/count/root/table span and every object
offset before reading objects. Each reached payload must stay in its object span;
references must be in range. Supports booleans, 1/2/4-byte unsigned and 8/16-byte
signed integers, finite IEEE binary32/64, Apple-epoch dates, data, ASCII/UTF-16BE
strings, UIDs, arrays and ordered string-key dictionaries, including extended
lengths. Dates that cannot fit Instant's exact nanosecond resolution fail rather
than round. Null/fill/set and other unsupported markers fail when reached.
Cycles are detected along the active path; shared acyclic references are allowed
and expanded under the visited-node budget. Unreachable object payloads are not
decoded; their offsets are still validated. Limits bound expansion, not only the
number of unique wire objects. No identity-preserving graph model is promised.
The node budget also caps the offset-table entry count before allocation.

Sources: [Apple DTD](https://www.apple.com/DTDs/PropertyList-1.0.dtd) and
[CoreFoundation binary format](https://github.com/apple/swift-corelibs-foundation/blob/main/Sources/CoreFoundation/CFBinaryPList.c).

- [x] RED XML/binary consumer fixtures and hostile boundary cases.
- [x] Pure Okkama parser, comments and embedded tests.
- [x] GREEN focused tests, runnable example and API/roadmap updates.

Evidence: 72 parser groups, 85 with the
value-model suite; three embedded groups and the runnable example PASS. No full
regression or interpreter/provider changes.
