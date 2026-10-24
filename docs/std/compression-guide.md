# Streaming compression contracts

## `compression/state`

`Compressing` and `Decompressing` are changing resources, not immutable byte
values. Both expose `write(origin is @byte) is void`, `output() is Reading`,
`finish() is void`, `finished() is bool`, `close() is void`, `closed() is bool`.
`Decompressing` additionally exposes `remainder() is @byte`.

The caller owns a codec and closes it; concrete resource owners must also supply
`destroy` cleanup. `close()` is idempotent and discards unclaimed buffered output.
Closing never implicitly finishes. After close, only `closed()` and `close()`
are valid. Empty writes are permitted while active. Writing or finishing an
already finished codec raises `error/InvalidState`. Finishing successfully makes
all remaining output available and `finished()` true; format-specific incomplete
input raises that format's stable error. Codecs may finish during a write if a
complete stream has been recognized. Codec/data failures make the codec unusable
except for closing; argument validation errors do not consume input.

### Output batches and EOF

`Reading` promises permanent EOF. A codec that has no output *yet* cannot expose
a live buffer as an ordinary `Reading`. Instead, each `output()` transfers the
currently available bytes to an independent finite batch, leaving the codec's
queue empty. Subsequent output calls transfer subsequent batches. A batch's EOF
is permanent; it says nothing about completion of the codec.

`CompressionOutput(origin is @byte)` implements `Reading` and `Closing`. It owns
a private byte buffer and releases it through `close()` or `destroy`. Reads
require a positive limit (`InvalidReadLimit`), return at most that many bytes,
consume bytes in order and return stable empty EOF. Closing is idempotent;
reading a closed batch raises `InvalidState`. A batch may escape and outlive the
codec that produced it; closing either does not close the other. Its internal
buffer is never exposed. Batch construction snapshots immutable input bytes.

An adapter must drain the batch before requesting more input or retain it until
it has been consumed. It must not treat an empty batch as overall EOF while the
codec is active. Output is allowed after successful finish to drain the final
batch; subsequent output batches are empty. `output()` does not finish a codec.

### Decompression limits and errors

`remainder()` is valid only after successful stream completion and returns exact
unconsumed trailing input already supplied to that codec. Before completion or
after close/failure it raises `InvalidState`. Stream framing and concatenated
member behavior are specified by concrete formats.

Every decompressor requires a positive cumulative output limit. Reject zero or
negative limits with `InvalidCompressionLimit(maximum)` before resource creation.
Exceeding the limit raises `CompressionLimitExceeded(maximum)` before excess
bytes become observable. Reading/draining output does not replenish the budget.
Both errors expose `maximum() is int` and stable `text()`; host exception messages
must not become the stable public error contract.

`DecompressionBudget(maximum)` is an immutable output allowance shared by codec
implementations. It validates the positive maximum; `remaining() is int` reports
the available bytes and `spent(amount is int) is ^DecompressionBudget` returns
a reduced allowance. Zero is permitted, negative amounts raise `InvalidArgument`,
and excess raises `CompressionLimitExceeded` without changing the old allowance.
Checking against remaining capacity before subtraction avoids integer overflow.
Codecs must retain each returned budget; draining output never replaces it.

### Reference fakes and verification boundary

The role fakes are byte echoes, not compression algorithms. `Compressing.fake()`
buffers all bytes until finish, exercising delayed output. `Decompressing.fake()`
emits supplied bytes immediately with a cumulative limit of eight bytes; finish
completes the fake and its remainder is empty. It becomes failed on budget excess.
Both implement the ownership/state/error contract above using test-only storage.
No fake implements gzip/zlib framing or proves the mathematics of a codec.

This step delivers the roles, errors and finite output owner. Concrete codec
implementation, truncated/malformed streams and roundtrips remain unchecked in
their later roadmap sections. No native compression provider currently exists;
the interpreter/provider edit restriction remains in force.

```okkama
codec.write([byte(1), byte(2)]);
// An output batch here may legitimately be empty.
codec.finish();
let final_output = codec.output();
let prefix = final_output.bytes(1);
// Continue draining final_output to EOF before dropping it.
codec.close();
```

## `compression/read`

`CompressedReading(origin is Reading, compressor is Compressing)` and
`DecompressedReading(origin is Reading, decompressor is Decompressing)` implement
`Reading` and `Closing`. A third `chunk_size is int` argument overrides the
8192-byte input/batch-read chunk size. Non-positive chunk sizes raise
`InvalidArgument` before reading or touching the codec.

Construction only binds dependencies and allocates empty owned buffers. A call
to `bytes(limit)` validates its positive limit, returns already buffered output
first, and otherwise repeatedly drains output batches and feeds source chunks.
Short source reads are ordinary data. An empty batch while the codec is active
causes another input read, not premature EOF. Source EOF triggers `finish()`;
successful finish must report completion, or the adapter raises `InvalidState`.
The final output batch is drained before overall EOF becomes observable.

If the codec is already finished (or finishes during a write), the adapter drains
its output and stops without another source read or finish. Trailing bytes already
fed to a decompressor remain accessible through that caller-owned decompressor's
`remainder()`. The adapter neither concatenates members nor interprets framing.

Each output batch is fully drained into an owned cache before returning at most
`limit` bytes; excess is retained for subsequent reads. Additional source data is
not consumed while this cache has bytes. Memory is proportional to the largest
pending batch, plus the current input chunk; a decompressor's cumulative budget
must independently bound its output. This adapter is not an expansion-limit
implementation. Sources/batches that exceed their requested read limit are
rejected with `InvalidState` rather than silently violating bounded reads.

Source and codec `Error` objects propagate unchanged. After a processing failure,
further reads raise `InvalidState`; caller-owned source/codec remain open. Invalid
read-limit errors do not mark the adapter failed. Successful EOF is remembered,
so later empty reads do not revisit the source or codec. `close()`/`destroy` close
only the adapter's cache and status buffer; explicit close is idempotent, and
subsequent reads raise `InvalidState`.

Both wrappers compose an internal `CompressionReading` over a `CompressionFlow`.
`CompressingFlow` and `DecompressingFlow` bind the source and nominal codec; the
flow exposes input, write, output, finish and finished, with no close capability.
This shares the pumping logic while preserving the two codec roles. These are
consuming streams, not iterable cursors.

## Concrete codecs deferred

User decision, 2026-09-25: defer gzip, zlib, raw DEFLATE, bzip2, XZ, LZMA-Alone
and zstd until reviewed host codec libraries are available. Do not implement
these algorithms in pure Okkama in this pass. Existing state/read contracts stay
implemented; no native compression provider currently exists. The interpreter
edit restriction remains in force. Archive path/metadata work is independent;
compressed archive entries and content-coding remain gated on concrete codecs.
