# `archive/tar`: value and streaming contracts

The module provides value encoding/decoding and streaming reading/writing.
No filesystem extraction or compressed TAR variant is involved.

`TarArchive(entries is Iterating<ArchiveEntry> [, format is TarFormat])` binds
the traversal and format without consuming it. `bytes() is @byte` traverses in
declared order and returns a complete independent archive. Each call repeats the
traversal; it does not cache the result. The value API buffers the complete output.

`TarWriting(origin is Writing [, format is TarFormat])` implements
`WritingArchive` and `Closing`. Construction does not write. `write(entry)` emits
one member and padding. `finish()` emits two zero blocks and flushes the borrowed
sink exactly once. `finished()` is passive. Write/finish after finish, processing
failure or close raises `InvalidState`; the original processing error propagates
unchanged. `close()` is idempotent, never finishes/flushes/closes the sink, and
releases private lifecycle/path tracking state. Destroy calls close. Partial
output is neither retried nor rolled back. Validation precedes each member's
first output byte; earlier members are not rolled back on a later invalid entry.

`TarFormat = (pax, ustar)`; PAX is the default. Both use 512-byte USTAR headers,
unsigned header checksums (the checksum field counted as eight spaces), octal
numeric fields and zero padding. PAX adds local `x` records only when needed;
GNU/base-256 output is not produced. Empty archives are exactly 1024 zero bytes.

Writer policy:

- Preserve all seven archive entry kinds and file bytes. Non-files have size 0.
  Directory names get a wire-only trailing slash. Link targets retain their
  `ArchivePath` interpretation. Device numbers must fit seven octal digits;
  no nonstandard device-number extension is invented.
- Missing mode defaults to 0755 for directories and 0644 otherwise. Missing
  ownership becomes numeric 0/0 with empty names. Explicit zeroes remain zeroes.
  No host account, permission, timezone or clock lookup occurs.
- Epoch time uses the metadata calendar as UTC, retaining nanoseconds. USTAR
  requires an integral nonnegative value fitting eleven octal digits. PAX
  retains negative/fractional/out-of-range timestamps in decimal `mtime`.
- Strict USTAR uses ASCII names: name <=100 bytes; optional prefix <=155 bytes
  split at an existing slash; link <=100; user/group <=32. No truncation. UID/GID
  must fit seven octal digits. Comments, Unicode and fields outside these bounds
  raise `InvalidTar`. Directory slash counts toward the limit.
- PAX uses UTF-8 `path`, `linkpath`, `uname`, `gname`, decimal `uid`, `gid`,
  `size`, `mtime`, and the conventional `comment` keyword when needed. Record
  lengths count UTF-8 bytes including their own decimal length and newline.
  Placeholder USTAR fields are valid and overridden by the local record. Newline
  and `=` in values are retained using length framing; embedded NUL in ownership
  names or comment is rejected. Long paths and arbitrarily large UID/GID retain
  their exact value. Optional extension order and fallback names are deterministic.
- Exact duplicate `ArchivePath` strings raise `DuplicateArchivePath(path)` before
  that member is written, even across different kinds. Tracking stores names,
  not payloads; memory grows with member count/name lengths. Paths are already
  validated archive values. Writers do not take reader allocation limits.

`InvalidTar(message is string)` implements `Error` with `text()` equal to
`"invalid tar: " + message`. `DuplicateArchivePath(path is ArchivePath)` implements
`Error`, preserves `path()`, and reports `"duplicate archive path: " + path.string()`.

Format references: [GNU USTAR layout](https://www.gnu.org/software/tar/manual/html_node/Standard.html),
[Go TAR writer](https://go.dev/src/archive/tar/writer.go) and
[PAX number/record framing](https://go.dev/src/archive/tar/strconv.go).
An independent host TAR reader is used only by tests, never by the std codec.

## Value reading contract

`ParsedTar(origin is @byte, limits is ArchiveLimits)` binds immutable bytes.
`entries() is Sequencing<ArchiveEntry>` reparses and returns an independent,
indexed snapshot in declaration order. Construction performs no parsing or I/O;
there is no filesystem extraction. Empty archives require two zero blocks, not
an empty byte array. Extra complete zero blocks are accepted; trailing nonzero
data, incomplete trailing blocks, a missing second EOF block and orphan local
extensions are invalid. Entry padding must be present and zero.

Accept POSIX USTAR and GNU headers, checking the unsigned checksum of EVERY
header before interpreting fields, including PAX/GNU extension headers. V7 and
sparse formats are outside this initial reader. Header strings decode strict
UTF-8. Numeric fields accept trimmed octal and GNU base-256; mode is 0..4095,
ownership is nonnegative BigInt, device coordinates must fit nonnegative int.

Local PAX (`x`) and GNU long-name/long-link (`L`/`K`) affect the next member;
global PAX (`g`) persists. Records use byte-count framing, strict UTF-8, nonempty
keys, one separating `=`, final newline and last-record precedence. Local records
override globals; an empty value suppresses that global value for this member
and restores the ordinary header field. Extension headers are not returned as
entries. Unknown ordinary keys are ignored; sparse directives and non-UTF-8
`hdrcharset` are rejected. GNU strings require a final NUL without embedded NUL.

PAX path/link/ownership/size/mtime/comment override ordinary fields exactly.
Decimal times retain integral nanoseconds; nonzero sub-nanosecond precision,
invalid decimals and dates outside the metadata calendar range raise InvalidTar.
Regular files, directories, links, FIFOs and devices become their existing typed
archive values. Non-files must declare size zero. Exactly one trailing directory
slash is removed before ArchivePath validation; other unsafe paths/link targets
raise UnsafeArchivePath. Duplicate effective member paths raise
DuplicateArchivePath even when one path comes from an extension.

ArchiveLimits counts real members, expanded file payload and Unicode path/link
length. Admission and spending happen BEFORE payload slicing. Declared sizes are
compared as BigInt against policy bounds before conversion to int, preventing
overflow and oversized allocation. Header truncation and data/padding spans are
checked before copying. The caller already owns origin; these bounds do not limit
the original input allocation or its trailing-zero scan.

Metadata has separate cumulative limits: 1 MiB extension payload and 4096 extension
headers by default. Optional constructor `(origin, limits, maximum_metadata,
maximum_extensions)` accepts positive int bounds. Both are checked before slicing
extension payload; failures are ArchiveLimitExceeded with `tar_metadata` or
`tar_extensions` and the configured maximum. This also bounds global/local record
storage and chains of empty metadata records. Real entries do not spend these
independent metadata budgets. Parsed snapshots retain at most the admitted payload.

## Streaming reading contract

`TarReading(origin is Reading, limits is ArchiveLimits)` implements ReadingArchive
and Closing. The four-argument overload adds positive `maximum_metadata` and
`maximum_extensions`, with the same defaults and meaning as ParsedTar (1 MiB and
4096). Constructors bind dependencies/private state without reading the source.

`entry()` consumes exactly one real member, including preceding extensions and
that member's padding. It returns an independent immutable ArchiveEntry and does
not read the following header. Payload admission (entry size, cumulative size,
count, effective path/link and duplicate path) precedes payload I/O/allocation.
Extension count/size admission precedes extension body I/O. Headers use fixed
512-byte buffers; payload/metadata read requests are at most 8192 bytes and never
exceed the remaining admitted span. Short positive reads are normal; zero bytes
inside an incomplete span are InvalidTar. A source returning more than requested
raises InvalidTar before those bytes are retained by the reader.

The format rules and typed values are shared with ParsedTar: checksums on all
headers, USTAR/PAX/GNU, local/global precedence, strict UTF-8, exact numeric/time
boundaries, safe relative paths/links, no sparse/V7 formats and zero padding.
Global PAX records persist across entry calls; local PAX/GNU records do not.

Normal completion requires two zero blocks, optional complete zero blocks and
the byte source's EOF, matching ParsedTar's whole-document rule. Framed callers
must provide a Reading bounded to the archive. This final entry() call scans the
trailer in 512-byte blocks, then raises EndOfArchive. `finished()` is passive and
becomes true only after this observed completion. Repeated entry() at normal EOF
raises EndOfArchive without more source calls.

Source Errors propagate unchanged and poison the reader, including an
EndOfArchive thrown by the byte source itself (which is not byte-stream EOF).
Format/limit/path/duplicate failures also poison it. After failure or close,
entry()/finished() raise InvalidState; close()/closed() remain available.
close() is idempotent and destroy calls close. Neither reads, drains or closes
the borrowed source. Returned entries survive reader close and destruction.

Only the current admitted payload is materialized; previously returned entries
are not cached. Private state contains cumulative counters, exact seen names and
bounded global PAX bytes. Global PAX is replayed when a new entry call starts;
this is bounded metadata work, not payload retention. Limits bound admitted data,
not caller-retained entries, allocations performed by a misbehaving source, or
all interpreter bookkeeping. No cursor API or filesystem extraction is added.
