# CSV: fixed-format printing and parsing

First implementation slice: `CsvString(rows is Iterating<@string>)` represents a
snapshot of text records with comma delimiter, double-quote escaping and CRLF record
endings. It produces a Unicode string, not bytes; encoding belongs to the caller.
No header detection, spreadsheet formatting, BOM insertion or formula rewriting.

Every record has at least one field. A zero-field record fails with EmptyCsvRecord
when printed. No records print as an empty string; one empty field prints as CRLF.
Every record, including the last, has a terminator. Spaces and Unicode are preserved.
Fields containing comma, double quote, CR or LF are quoted; internal quotes double.
Other fields remain unquoted. This follows the quoting/record structure described
in [RFC 4180](https://www.rfc-editor.org/rfc/rfc4180); accepting arbitrary Unicode
field content is the explicitly selected application dialect, not a strict ASCII
ABNF validation claim. Control scalars are preserved as data.

`CsvRecord(fields is @string).string()` represents one record including CRLF.
`CsvField(origin is string).string()` represents one escaped field.
The implementation has no native dependency, reflection or resource ownership.
CsvString snapshots its finite input; printing is repeatable independently of later
cursor observations. The current implementation is in-memory with read and print budgets below;
resource-based row input and streaming output remain separate tasks.

Tests before implementation: empty document versus empty field; trailing empty
fields; commas/quotes/CR/LF; Unicode and spaces; zero-field rejection; repeatability;
nominal custom Iterating input. Fixed-format parsing and round-trip fixtures are now implemented below.
Configurable dialects are implemented below. [Resource document input](csv-input-guide.md)
is implemented separately; incremental row reading remains open.

## Remaining steps

1. Completed: strict in-memory parser and fresh immutable cursors, with
   explicit EOF, CRLF, Unicode offsets and BOM rules below.
2. Completed round-trip fixtures against this printer: quoted line endings, quotes, blank
   records, final terminator and significant spaces. Do not copy the legacy scanner's
   unquoted-field behavior: it currently lacks quote validation.
3. Completed: configurable grammar with distinct scalar delimiter/quote and
   LF/CRLF endings; see the contract below.
   Header interpretation remains separate from CSV syntax.
4. Read and print budgets are implemented below. Next: resource-based row
   reading, with explicit limits/errors and no blocking I/O hidden in Cursor.

[Example](../../examples/std-v4/csv-basic.ok). From okkama-pl with Perl 5.42:

```sh
prove -l t/interpreter/253-std-v4-csv-print.t t/interpreter/250-std-v4-examples.t t/audit/14-library-syntax.t t/audit/16-stdlib-v4.t
```

Verified printing slice: 4 files / 494 tests, PASS (including 9 new test groups).
This is the historical printing-only result; these are focused and semantic checks,
not a new full-project regression result.

## Parsing slice — VERIFIED

`CsvRows(origin is string) does Iterating<@string>` stores text without parsing.
Each `cursor()` validates the entire document before returning a fresh immutable
cursor over decoded records. No partial result escapes malformed input.
The constructor performs no I/O; cursors own no external resource.

The default overload uses comma, double quote and CRLF. The final record may omit CRLF. Empty
input contains no records; CRLF denotes one empty field. A final CRLF does not
create an extra record. Commas preserve final empty fields; widths may differ.
Quotes are legal only at field start. Quoted fields double internal quotes and
preserve every other scalar, including CR/LF. After the closing quote only a
comma, CRLF or EOF is valid. Lone CR/LF outside quotes is invalid.
Leading U+FEFF is ordinary field data, never silently stripped. No header,
whitespace, formula or encoding normalization occurs.

`InvalidCsv(offset is int)` reports the zero-based Unicode-scalar `offset()`;
unterminated quotes and an incomplete CRLF report EOF (`origin.length()`).
Malformed quotes or unexpected characters report the offending scalar.
`text()` is a stable human-readable description. Parsing is in-memory; the read
budgets below supersede the initial unbounded parsing contract.
EOF cursor item/successor use the existing `IterationEnded` contract.

Implementation uses immutable prefix states and persistent lists; no native
CSV parser or mutable buffer is introduced. Completed fields/records share list
tails. The bounded read slice below replaces growing-string concatenation with
source offsets and one slice/unescape operation per completed field.

[Parsing example](../../examples/std-v4/csv-parse.ok) and
RED/GREEN evidence.

Full regression for this slice: **383 files / 4103 checks, PASS**.

## Read budgets — VERIFIED

`CsvRows(origin)` uses scalar budgets: input 1048576, decoded field 65536,
records 10000, and fields per record 1024. The explicit overload is
`CsvRows(origin, maximum_input, maximum_field, maximum_rows, maximum_columns)`;
all arguments may be named. Budgets are nonnegative integers; zero is valid.
Negative configuration raises `InvalidCsvLimits` during construction. Source
validation still occurs only on cursor(). Input length is checked before syntax;
then syntax/decoded-field/record-column budgets are checked in scan order.
`CsvLimitExceeded(limit is CsvLimit)` is catchable, with `kind() is CsvLimit`.
Read kinds are input, field, rows, columns; printing additionally uses output. Limits count Unicode scalars, not encoded bytes. Escaped doubled
quotes count once in decoded fields; record terminators do not count as fields.
An empty source fits all-zero budgets. A record with one empty field needs one
row and one column, but no field-content allowance. No partial cursor escapes.

The implementation retains source offsets while scanning and slices/unescapes
each field once, replacing repeated concatenation of growing field strings. Completed
fields/rows continue to use persistent lists; no native parser or I/O is added.
Existing source strings already allocated by callers are outside these budgets.

[Read-budget example](../../examples/std-v4/csv-read-limits.ok).

Read-budget regression: **384 files / 4127 tests, PASS**.

## Print budgets — VERIFIED

CsvString retains snapshot semantics. Its default budgets are output 1048576
Unicode scalars, decoded field 65536, rows 10000, columns per row 1024.
Explicit overload: CsvString(rows, maximum_output, maximum_field, maximum_rows,
maximum_columns). Nonnegative budgets are checked before observing the source;
negative configuration raises InvalidCsvLimits. Constructor traversal stops on
the first excess row/column/field, raising CsvLimitExceeded with the same enum
vocabulary as parsing. A custom cursor may have item() observed at most
maximum_rows + 1 times before row rejection; traversal does not continue after it.
No source ownership is transferred; subsequent string() calls use the snapshot.

Output budget is checked on string(), before building an escaped record. It counts
quotes, doubled quotes, delimiters and all final CRLF terminators. Empty input
prints empty at budget zero. An empty field costs two scalars; a quoted quote
costs six including CRLF. An empty zero-field record still raises EmptyCsvRecord
on observation. No partial output is returned and no external writer is involved.
Standalone CsvField/CsvRecord remain explicit unbounded representations;
CsvField.size() counts the scalar length of its encoded representation.

[Print-budget example](../../examples/std-v4/csv-print-limits.ok).

Print-budget regression: **385 files / 4152 tests, PASS**.

## Configurable dialect contract

`CsvDialect(delimiter is string, quote is string, line_ending is string)` is an
immutable grammar, not an options record. Delimiter and quote must be distinct
single Unicode scalars other than CR/LF. The ending is exactly LF or CRLF.
Invalid configuration throws `InvalidCsvDialect` during construction. Doubling
is the sole escape convention; backslash has no special meaning.

`CsvRows(origin, dialect)` and `CsvString(rows, dialect)` use existing default
budgets. Their six-argument overloads append `dialect` to the existing five
arguments. Old overloads retain comma, double quote and CRLF. Parsing is strict:
LF dialect rejects unquoted CR; CRLF dialect rejects lone LF/CR. Quoted fields
preserve either newline. Offsets and all budgets count original Unicode scalars;
field budgets count decoded scalars. Final record terminators remain optional
on input and mandatory on output. Each printed empty field costs the configured
ending length. Construction snapshots printing input once; reading stays lazy.
No resources are opened or owned. Repeat observations are independent.

The grammar exposes behavior: `symbol(value)` classifies a scalar as CsvSymbol;
`encoded(value)` minimally quotes a field; `decoded(value)` undoubles an already
validated quoted field's interior; `size(value)` counts its encoded length;
`record(fields, allowance)` checks and prints a complete bounded record.
`decoded` is a parser collaborator, not an independent CSV validation API.


[Executable dialect example](../../examples/std-v4/csv-dialects.ok).
Focused checks: **8 files / 624 tests, PASS**, including 33 dialect groups.
RED/GREEN and regression evidence.

Dialect full regression: **388 files / 4246 tests, PASS**.

For explicit bounded reading from a borrowed byte source, see [CsvInput](csv-input-guide.md).

For resource replay without complete text/row materialization, use the explicit
[CsvStream record reader](csv-stream-guide.md). CsvRows retains its in-memory contract.
