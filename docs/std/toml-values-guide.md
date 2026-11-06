# TOML value model

The model implements immutable scalar, temporal, array and table values for
the TOML 1.0 roadmap. It is not a TOML document parser or writer. Temporal values
use their own validated coordinates; they do not narrow the domain to existing
calendar types. See the temporal contract below.

The [TOML 1.0 specification](https://toml.io/en/v1.0.0) defines mixed arrays,
case-sensitive names, duplicate-key rejection and exact signed 64-bit integers.
The Okkama model additionally preserves table insertion order. Source spelling,
escaping and dotted-key resolution belong to toml/parse, not this model.

`TomlValue.accept(to is TomlVisitor) is void` performs exactly one typed callback:
text(string), integer(int), real(float), boolean(bool), elements(Sequencing of
TomlValue), or members(Sequencing of TomlMember). It does not walk nested values;
the visitor decides whether to descend. Visitor errors propagate unchanged.
Concrete scalar constructors take their exact typed value, without conversion.
No kind tags, downcasts, arbitrary-object reflection or JSON intermediates.
The temporal and special-float callbacks are specified below.

`TomlMember(name is string, value is TomlValue)` represents one named binding;
name() and value() identify its two coordinates. Names are literal Unicode strings:
empty names and dots are allowed, case and normalization are preserved.
`TomlArray(values is Sequencing<TomlValue> [, maximum_items is int])` snapshots
size() once and each item exactly once, preserving order and mixed types.
`values()` exposes the immutable snapshot as Sequencing. Default limit: 10000.
`TomlTable(members is Sequencing<TomlMember> [, maximum_members is int])` uses the
same snapshot rule (default 1024), then rejects duplicate exact names with
DuplicateTomlMember(name). members() preserves order, membership(name) returns
bool, value(name) returns the associated TomlValue or throws MissingTomlMember.
Neither container directly implements Sequencing or a map role.

Negative budgets throw InvalidTomlLimit before observing the source. A reported
negative source size throws InvalidTomlSize; size above budget throws
TomlLimitExceeded before item reads. Zero accepts an empty container. Errors
from size/item propagate unchanged. Sources are borrowed and never closed.
Snapshots are shallow: they retain nominal value objects without inspecting or
copying arbitrary implementations. Repeated observations do not revisit source.

Table duplicate validation currently uses O(n²) exact comparisons, lookup O(n),
with the explicit member bound. A hash index is a future performance change.
Limits bound container cardinality, not scalar length or total recursive graph
size. No I/O, resource ownership, normalization or implicit temporal conversion.
Integers retain runtime int semantics; float wrappers retain runtime float bits.
Binary64 value preservation is verified, including both zeros and subnormals.
Special float literal parsing is supplied by toml/parse. Textual fractional-second truncation is implemented by [toml/parse](toml-parse-guide.md).


[Executable example](../../examples/std-v4/toml-values.ok).
RED/GREEN evidence: **5 files / 560 tests, PASS**.
The module contains five test groups and a test-only nominal visitor fake;
external consumer tests additionally check exact callback counts through output,
custom values, source observation counts, bounds and propagated observer errors.

## Temporal value contract

The temporal model represents syntax-level temporal coordinates, not host
instants. `TomlLocalDate(year, month, day)` validates Gregorian dates in years
0000..9999, including year zero. `TomlLocalTime(hour, minute, second, nanosecond)`
accepts hours 0..23, minutes 0..59, seconds 0..60 and nanoseconds 0..999999999.
A second of 60 preserves leap-second notation; the model does not assert that an
actual leap event occurred. No clock, host zone or leap-second database is read.
This separates representability from any future instant-conversion validation.

`TomlLocalDateTime(date is TomlLocalDate, time is TomlLocalTime)` composes those
coordinates. `TomlOffset(minutes is int)` is a known displacement in -1439..1439.
The two-argument `TomlOffset(minutes, known is bool)` preserves unknown zero
(`-00:00`); unknown with a nonzero displacement is invalid.
`TomlOffsetDateTime(local is TomlLocalDateTime, offset is TomlOffset)` retains
both coordinates. No conversion to existing calendar types is implicit.

Temporal values expose their named coordinates, canonical string() and accept().
TomlVisitor gains local_date(TomlLocalDate), local_time(TomlLocalTime),
local_datetime(TomlLocalDateTime), offset_datetime(TomlOffsetDateTime), each void.
Known zero formats as Z; unknown zero as -00:00. Dates use four/two/two digits,
times retain seconds, a nonzero fraction uses at most nine digits without
trailing zeros, and date/time uses T. This is a scalar representation, not a
TOML document writer. Coordinates and repeated representations are immutable.
InvalidTomlDate, InvalidTomlTime and InvalidTomlOffset are constructor errors.

The future text parser must truncate excess fractional digits to nanoseconds,
never round or carry into the next second. The coordinate constructor requires
an already integral nanosecond value and rejects overflow. Source spelling such
as lowercase delimiters and trailing fractional zeros is not retained.
Rules follow [TOML 1.0](https://toml.io/en/v1.0.0#offset-date-time) and
[RFC3339](https://www.rfc-editor.org/rfc/rfc3339.html#section-5.6).


[Temporal example](../../examples/std-v4/toml-temporal.ok).
Temporal RED/GREEN and analyzer regression:
11 files / 645 focused tests; **393 files / 4371 full tests, PASS**.
## TOML special float representation

Okkama float remains finite. TOML infinity and NaN use separate immutable
TomlValue implementations, not invalid native Float payloads:

- TomlInfinity(negative is bool): string() is "inf" or "-inf";
  accept(to) calls to.infinity(negative) once.
- TomlNaN(): string() is "nan"; accept(to) calls to.not_a_number() once.
  TOML permits signed NaN spellings but does not require preserving their sign.
- TomlVisitor gets those two void callbacks. Finite TomlFloat continues to use
  real(float) exactly once; it preserves binary64 coordinates, including signed
  zero and subnormals. No conversion of the special objects to builtin float.

Tests first: literal output/dispatch, nominal use inside containers, repeatability,
callback failure, +0/-0 byte encodings, minimum subnormal and maximum finite
binary64 fixtures through FloatFromBytes → TomlFloat → FloatBytes. Parsing
inf/+inf/-inf/nan/+nan/-nan remains a parser responsibility; no string-to-float
conversion is added here. Malformed spellings and overflow get typed parse errors.

[Special-float example](../../examples/std-v4/toml-special-floats.ok).
Evidence: focused 6 files /
146 tests, examples 1 / 16; **full regression 395 files / 4401 tests, PASS**.

[Document parsing](toml-parse-guide.md) now implements those literal, temporal and table rules.
