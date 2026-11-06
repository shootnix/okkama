# Property-list values

## Contract before implementation

`plist/value` represents immutable values, independently from JSON or a serialized
format. `PlistValue` exposes `kind() is PlistKind` and `print(to is PlistWriting)`.
The nine kinds are boolean_value, integer_value, real_value, string_value,
data_value, date_value, array_value, dictionary_value and uid_value.

Scalar constructors and `value()` projections preserve their exact nominal type:
PlistBoolean(bool), PlistInteger(BigInt), PlistReal(float), PlistString(string),
PlistData(@byte), PlistDate(Instant), PlistUid(BigInt). UIDs must be nonnegative,
otherwise InvalidPlistUid; integers remain arbitrary precision and signed.
Okkama float is finite; no narrowing, timezone conversion, UID resolution, XML
character restrictions or serialization-specific integer limits are imposed here.
Strings and immutable bytes are retained, not copied or normalized. Their sizes
are caller-owned; container cardinality limits do not bound complete tree bytes.

`PlistWriting` receives one typed event per print call: boolean(bool),
integer(BigInt), real(float), text(string), data(@byte), date(Instant),
elements(Sequencing<PlistValue>), members(Sequencing<PlistMember>) or uid(BigInt).
The recipient chooses whether to recurse; no automatic recursive traversal,
reflection, formatting, I/O, flushing or closing. Callback errors propagate
unchanged and repeated print calls repeat the same event. Construction does not
observe children by kind or print. Custom nominal PlistValue implementations work
without casts. Writers must impose their own traversal/output budgets.

`PlistMember(name is string, value is PlistValue)` exposes name()/value().
`PlistDictionary(members is Sequencing<PlistMember> [, maximum_items is int])`
replaces the v3 map candidate: nominal entries preserve insertion order, support
dynamic construction in Okkama, and allow explicit duplicate rejection. Keys are
exact strings, including empty strings; no case folding, trimming or normalization.
members() returns the ordered immutable sequence; membership(name) is bool;
value(name) is PlistValue or throws MissingPlistMember(name). Duplicate names throw
DuplicatePlistMember(name); both errors retain name() and text().

`PlistArray(values is Sequencing<PlistValue> [, maximum_items is int])` exposes
values() as an immutable Sequencing<PlistValue>. Both containers default to 1024
items. Negative limits fail with InvalidPlistContainerLimit before source access;
negative source size fails with InvalidPlistContainerSize; excessive size fails
with PlistContainerLimitExceeded before reading items. Zero permits empty input.
Read size once, then item(0..size-1) once in ascending order; do not request
cursor/first/last or close the source. Source failures propagate and stop reads.
Queries use the snapshot, which retains children and survives constructor scopes.
Duplicate detection is quadratic within the explicit cardinality budget.

This task adds only the value model. XML/binary parsing and writing remain the
following unchecked roadmap sections; accepting a model value is no assertion
that every serialization can represent it. No interpreter/provider changes.

Background: [Apple property-list types](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/PropertyLists/AboutPropertyLists/AboutPropertyLists.html).

- [x] RED consumer tests.
- [x] Implementation with comments and embedded tests.
- [x] GREEN focused suite and example, API/roadmap update.

Evidence: 13 consumer groups and three
embedded groups PASS; [runnable example](../../examples/std-v4/plist-value.ok).
