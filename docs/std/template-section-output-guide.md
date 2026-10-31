# Streamed section and partial output

## Contract before implementation

`SectionTemplate.write(data is Json, destination is WritingText) is void`
streams the same text as string(data), using the same constructor limits and
explicit library. string(data) delegates to this path with a local UTF-8 buffer,
so both forms share traversal and resource accounting.

Each call compiles/validates the root before lookup or output. Reached partials
compile before their own data access/output; earlier parent contributions can
already be visible. Contributions (one literal or interpolation) are preflighted
against the remaining aggregate UTF-8 budget before their first write, then split
at scalar boundaries into nonempty chunks of at most 1024 scalars/4096 UTF-8 bytes.
No added newline, flush, close, or ownership transfer of the borrowed destination.

Successful prefixes remain visible on later source, sink, syntax, missing-partial
or limit failures. Sink exceptions propagate unchanged and stop later item reads,
lookups and writes. If a custom JSON source catches a callback failure, the existing
completion check rejects the source rather than reporting success. There is no
rollback guarantee. Repeated calls append anew with fresh budgets.

Streaming keeps no full-result buffer. Its output meter owns exactly eight bytes,
encoding a nonnegative signed-int-range count via existing std binary adapters.
Reservations compare against remaining budget before addition (no overflow), and
replace the private eight-byte record before calling the sink. Section iteration
still uses its bounded byte ledger; parser stacks/member snapshots and one scalar
interpolation remain bounded as before. All owned temporary buffers close on every
path; external data, library and destination remain borrowed. No new native APIs.

- [x] RED streaming/parity/failure/chunk tests.
- [x] Shared stream/string implementation and embedded meter boundary tests.
- [x] GREEN focused tests, runnable example and roadmap/API updates.

[Example](../../examples/std-v4/template-section-output.ok),
RED/GREEN evidence.
