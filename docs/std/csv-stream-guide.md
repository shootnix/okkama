# Incremental CSV records

`csv/stream::CsvStream` borrows an `io/stream::Reading`. Construction performs
no reads. `row() is @string` explicitly consumes one record; `EndOfCsv` marks
stable EOF. An empty record is `[""]`, never EOF. The reader is synchronous;
callers serialize access. It does not implement a cursor that hides I/O.

```okkama
let file = File(mode: FileMode.reading, path: path, binary: true);
let input = CsvStream(file.binary());
defer input.close();
loop {
    let fields = try { input.row(); } catch end is EndOfCsv { last; };
    consume(fields);
}
```

The default grammar matches `CsvRows`: comma, double quote with doubling, CRLF
record separators, optional final terminator. Inside quoted fields every CR/LF
is preserved; outside them lone CR/LF is invalid. No trimming, BOM stripping,
header detection, blank-line skipping or Unicode normalization occurs. Every
Unicode scalar is legal, including controls and noncharacters; malformed UTF-8,
surrogates, overlong sequences and codepoints above U+10FFFF are rejected.
Custom CsvDialect remains available on CsvRows/CsvInput; this first streaming
API intentionally supports the default grammar only.

Constructors:

- `CsvStream(origin is Reading)` uses default budgets.
- `CsvStream(origin is Reading, maximum_bytes is int, maximum_input is int,
  maximum_field is int, maximum_rows is int, maximum_columns is int)` sets them.

Defaults: 4,194,304 encoded bytes, 1,048,576 original Unicode scalars, 65,536
decoded field scalars, 10,000 records, 1,024 fields per record. Limits must be
nonnegative; zero is meaningful. They apply cumulatively to the entire stream,
not separately per row. The input budget includes quotes/separators/terminators.
Row memory is bounded by field/column budgets and the remaining input budget.

The state machine decodes and parses incrementally, stopping at the next record.
It retains only the current row plus at most 4,099 raw bytes (a 4,096-byte block
and up to three bytes of a split scalar). Earlier rows are detached and never
retained by the scanner. Source requests are positive, at most 4,096 bytes and
at most remaining byte budget plus one EOF probe. A source returning more than
requested throws InvalidReading. Short reads are accepted. Record separators,
doubled quotes and Unicode scalars may cross any boundary. A read can buffer
part of later rows but never parses/materializes the entire document ahead.

Failures:

- InvalidCsv.offset() counts original Unicode scalars; InvalidUtf8.offset()
  counts original bytes. Offsets include previously returned records.
- CsvByteLimitExceeded and CsvLimitExceeded retain the existing CSV vocabulary.
- Syntax, UTF-8 and limit failures are terminal and repeat with the same kind
  and offset, without reading the source again. Detection follows consumption
  order, unlike CsvInput's whole-document decoding-before-parsing ordering.
- Source Errors propagate unchanged, then the reader is closed; retry raises
  ClosedCsvStream. Programming failures are not reclassified as corrupt input.
- Earlier rows may already have escaped when later input fails. A repository
  must keep replay private and validate EOF before returning a snapshot.

`closed`/`close` implement Closing. Close is idempotent and releases all scanner
buffers; it never closes the borrowed source. Returned records remain valid.
The source owner keeps its normal scope/destroy/close responsibility. There is
no implicit source ownership transfer; explicit close/defer releases scanner
state early, and native registry teardown is the final memory-cleanup fallback.

CsvInput.read() and CsvRows remain compatible materializing APIs. Choose them
for all-or-nothing document results, and CsvStream for explicit record replay.
The invoice storage contract
adds header, numeric, domain and publication rules above this generic CSV grammar.
