# Bounded CSV input from a resource

`csv/input.CsvInput` represents a borrowed `io/stream.Reading` interpreted as one
UTF-8 CSV document. Construction performs no reads. `read() is @@string` is an
explicit synchronous consuming operation: it reads until EOF, strictly decodes
UTF-8 and validates the entire CSV document before returning immutable rows.
It does not implement Cursor/Iterating, normalize line endings, strip BOM, guess
headers, or close the source. Parsing uses `csv.CsvDialect` and the same original
Unicode scalar offsets as CsvRows. InvalidUtf8.offset() counts bytes instead.

Constructors (positional and named):

- `CsvInput(origin is Reading)` uses the default CSV grammar and budgets.
- `CsvInput(origin is Reading, dialect is CsvDialect)` changes grammar only.
- `CsvInput(origin is Reading, maximum_bytes is int, maximum_input is int,
  maximum_field is int, maximum_rows is int, maximum_columns is int,
  dialect is CsvDialect)` sets all budgets.

Defaults are 4194304 bytes, 1048576 input scalars, 65536 decoded field scalars,
10000 rows, 1024 columns. Negative limits throw InvalidCsvLimits before observing
origin. CsvByteLimitExceeded reports a byte budget failure; CsvLimitExceeded
retains the existing scalar/field/row/column vocabulary. Byte validation precedes
UTF-8 validation, which precedes CSV validation. Bounds apply per read operation.

The reader requests at most 4096 bytes at a time and at most one byte beyond its
remaining byte budget to distinguish exact-boundary EOF from excess input. It
retains at most maximum_bytes input bytes; a nonconforming source returning more
than requested throws InvalidReading. Short reads do not mean EOF. UTF-8 scalars,
CRLF pairs and doubled quotes may cross any read boundary. Scalar/field limits
are checked after bounded byte collection; this is not an incremental parser.
Memory includes the byte snapshot, decoded text and bounded parsed rows.

Each read consumes the remaining document at the source's CURRENT position. With
a conforming stable-EOF source, later reads return an empty array. This is not a
cached/replayable representation: retain returned rows to observe them again.
On source/decode/parse/limit error, no rows are returned. Already consumed bytes
cannot be rolled back; a later read starts at the remaining source position.
Source errors retain their types. The reader neither retries nor closes origin.

The Reading capability is retained by CsvInput. Dropping CsvInput releases that
reference but never explicitly closes a borrowed source. Closing the source
externally is observed on the next read according to the source's contract.
There is no CsvInput.close() or destructor: the private byte buffer is local to
each read. Concurrent/reentrant consumers of the same source require caller
coordination. No asynchronous or cancellation contract is claimed here.

For incremental row-at-a-time parsing and stable failure state use
[CsvStream](csv-stream-guide.md). Owned-source variants remain separate design
work. CsvInput keeps its compatible bounded, materializing document contract.

[Executable example](../../examples/std-v4/csv-input.ok).
RED/GREEN evidence: **8 files / 628 tests, PASS**.
Inline tests live in the module and run through `okkama -t` or the shared
`266-std-v4-inline-tests.t` gate. No interpreter/native modifications.
