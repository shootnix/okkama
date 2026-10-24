# TOML 1.0 document parsing

Contract written before implementation; implementation and validation are complete. `ParsedToml` represents one immutable TOML text
with explicit resource budgets. Construction binds the source and validates
budgets; `value() is TomlTable` parses on observation and returns the root table.
Repeated observations have identical values/diagnostics. No filesystem, ambient
locale/time zone, JSON conversion, generic reflection or document writing.

Constructors:

- `ParsedToml(origin is string)` uses 1048576 input Unicode scalars, depth 64,
  and 10000 total value nodes (including the root and implicit tables).
- `ParsedToml(origin is string, maximum_input is int, maximum_depth is int,
  maximum_nodes is int)` configures those bounds. Input may be zero; depth is
  1..128; nodes must be positive. Invalid configuration throws InvalidTomlBudget
  at construction, before text parsing. Hard depth ceiling prevents host-stack
  exhaustion even if a caller requests an excessive budget.

The root is depth 1; every array element or table member adds one level. Implicit
and explicit tables and array-of-table elements count identically. Every table
has at most 1024 members, every array at most 10000 items, matching existing
TomlTable/TomlArray default capacities. Limits are enforced during parsing,
before allocating an excess node. String lengths are bounded by the input budget;
no byte decoding occurs because origin is already a language string.

`InvalidToml(offset, line, column, message) does Error` exposes `offset()`,
`line()`, `column()`, `text()`. Offsets count zero-based original Unicode scalars;
line/column are one-based; CRLF is one newline. Syntax, semantic conflicts,
out-of-domain numbers and exceeded document budgets produce InvalidToml when
value() is observed. No partially parsed root is returned. Errors are stable
across repeat observations. Diagnostic wording is useful but not a versioned ABI;
coordinates and nominal error category are the stable contract.

Rules derive from [TOML 1.0](https://toml.io/en/v1.0.0), deliberately not 1.1.
Tests must cover all four string forms, escapes and Unicode scalars, quoted and
dotted keys, mixed/nested arrays, inline tables, table definitions and arrays of
tables, exact integers of every supported radix, finite binary64, explicit
special floats, booleans and all four temporal categories. Multiline strings
normalize CRLF to LF. Leading delimiter newline is trimmed; line-ending
backslash folds whitespace in multiline basic strings. No normalization of keys.

Decimal integers have optional sign, other radices do not. Underscores occur
only between digits. Decimal leading zeros are forbidden except zero itself.
Signed 64-bit bounds are checked exactly before native conversion. Finite float
conversion preserves signed zero; overflow is rejected, underflow follows binary64
rounding. inf/+inf/-inf and nan/+nan/-nan construct dedicated TomlValue objects.
Datetime year zero and second 60 follow the existing coordinate model; fractional
seconds are truncated to nine digits, never rounded. Unknown zero offset -00:00
remains distinguishable from known zero. Dates/times are validated without host
calendar normalization or instant conversion.

All duplicate decoded names are errors. Scalar/table conflicts, table redefinition,
extending sealed inline tables, extending static arrays as arrays of tables and
conflicting dotted keys are errors. Implicit ancestors created by a table header
may be declared later; tables defined by dotted keys may not be redeclared.
Nested headers under arrays of tables attach to the most recent array element.
Insertion order is retained, including implicit table positions.

The native parser owns only immutable text/tree memory. Its opaque handles are
private substrate, not public domain API. Returned Okkama values retain their own
coordinates and remain usable after ParsedToml leaves scope; no close/destroy
operation is exposed and no borrowed resource is closed. Parse failures release
partial trees; registry teardown releases opaque memo state.

Implementation sequence: (1) contract and independent accepted/rejected fixtures,
(2) embedded tests in both new std modules, (3) strict bounded provider and nominal
projection, (4) focused grammar/lifetime tests, (5) full regression for native
changes, example, API inventory, evidence and commit. Keep unresolved behavior
unchecked in the completion TODO; do not infer full conformance from smoke tests.

[Executable example](../../examples/std-v4/toml-parse.ok).
Evidence: 127 consumer scenarios,
679 independent TOML 1.0 fixtures, native lifetime/error checks and resource bounds.
There are six embedded domain and two substrate test groups.
Final full regression: **400 files / 5259 tests, PASS**.
