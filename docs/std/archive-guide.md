# Archive value contracts

Concrete format: [`archive/tar` — PAX/USTAR values and streams](archive-tar.md).
TAR streaming reading is implemented; ZIP and filesystem extraction gates remain
open. Value parsing/writing never touches the filesystem itself.

## `archive/path`

`ArchivePath(origin is string)` denotes an immutable relative member name inside
an archive. It is independent of `fs/Path`, the current directory, platform and
filesystem. Construction validates the whole name before an object is available:

- Reject empty names and a leading `/` (including slash-form UNC names).
- Reject every backslash and NUL, without converting separators or truncating.
- Reject an ASCII drive prefix `[A-Za-z]:`, including drive-relative `C:foo`.
- Split on literal `/`; reject empty, `.` and `..` segments at any depth.

No trimming, case folding, Unicode normalization, percent decoding, separator
conversion or dot-segment cleanup is performed. Valid Unicode scalars, spaces,
literal percent escapes and ordinary punctuation are preserved exactly. Other
than a leading drive prefix, colon is an ordinary archive-name character; this
value does not assert that every name is writable on every host filesystem.
A trailing `/` is an empty segment and is rejected. Format adapters must model
directory kind separately and explicitly handle their wire-format directory
suffix before constructing an `ArchivePath`.

`string() is string` returns the exact validated name. `segments() is
Sequencing<string>` exposes its immutable slash-separated components, and
`name() is string` returns the last one. `parent() is ArchivePath` returns the
nonempty prefix without the last component. A one-component path has no parent
and raises `MissingArchiveParent(path)`; there is no synthetic root value.

`UnsafeArchivePath(origin is string, message is string)` implements `Error`:
`path() is string` retains the offending input, and `text() is string` is
`"unsafe archive path: " + message`. Validation uses stable reasons: `empty path`,
`absolute path`, `backslash`, `NUL character`, `Windows drive prefix`,
`empty segment`, `dot segment`, `parent segment`. Whole-name checks run first
in the order above, followed by the three segment checks from left to right.
`MissingArchiveParent(path is ArchivePath)` retains the exact path through
`path() is ArchivePath`; `text()` is `"archive path has no parent"`.

This is lexical validation only. Symlinks, hard links, filesystem aliases,
platform filename restrictions, destination containment and races belong to
the later extraction capability. Constructing or observing this value performs
no I/O and owns no resources. Repeated observations and returned segments/parents
remain valid independently of the original object's scope.

```okkama
let member = ArchivePath("assets/иконка.png");
member.name();            // "иконка.png"
member.parent().string(); // "assets"
```

## `archive/metadata`

`ArchiveMetadata` is an immutable snapshot at the archive-format boundary. Its
observations describe a member's stored attributes, not the current filesystem
or account database. Construction binds the supplied immutable values and never
reads a file, resolves a user, applies permissions or observes the process zone.

`ArchiveOwnership(user is string, group is string, user_identity is BigInt,
group_identity is BigInt)` retains exact names and nonnegative arbitrary-size
numeric identities. Negative identities raise `InvalidArchiveOwnership` during
construction (`text(): "archive ownership identities must be nonnegative"`).
Zero is a real identity. Empty names are valid wire metadata; neither empty
names nor a magic numeric value stand for a missing ownership object. The four
observations `user()`, `group()`, `user_identity()` and `group_identity()` return
these archive coordinates unchanged. There is no host name/ID lookup.

`ArchiveMetadata` accepts `modified is LocalDateTime, comment is string`, with
optional `PermissionMode` and then `ArchiveOwnership` via the three overloads:

```okkama
ArchiveMetadata(modified, comment);
ArchiveMetadata(mode, modified, comment);
ArchiveMetadata(mode, modified, ownership, comment);
```

`modification()` and `comment()` preserve exact calendar/nanosecond fields and
comment text. `mode()` raises `MissingArchiveMode` when absent; `ownership()`
raises `MissingArchiveOwnership` when absent. Error texts are `"archive mode is
absent"` and `"archive ownership is absent"`. Mode zero and UID/GID zero remain
present values. Presence is immutable; no public setters or null/sentinel values
are introduced. Ownership without a mode is outside these initial constructors;
a future format needing that shape must introduce an unambiguous contract rather
than fabricate a mode. Format-specific representable ranges and
text encodings remain the responsibility of the format adapter.

ZIP calendar fields bind directly as `LocalDateTime`, with no inferred offset
or daylight-saving interpretation. TAR epoch values use
`ArchiveModificationFromEpoch(seconds is rat).value() is LocalDateTime`: exact
seconds since 1970-01-01T00:00:00 represented as UTC calendar fields (the
[TAR mtime epoch](https://www.gnu.org/software/tar/manual/html_node/Standard.html)). Construction
uses `Duration`, rejecting sub-nanosecond values with `InvalidDuration`;
observation composes epoch-calendar arithmetic and propagates `InvalidDate` for
results outside years 1..9999. Negative and fractional seconds at
integral-nanosecond resolution are supported. No process timezone or clock is
consulted. Calendar/epoch conversion is explicit; the metadata never interprets
an arbitrary local date as a UTC instant itself.

The stored ZIP timestamp distinction follows PKWARE's
[format specification, §4.4.6](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT).
The metadata layer does not yet parse TAR or ZIP records.

## `archive/entry`

An `ArchiveEntry` is an immutable member value, not a file handle or a deferred
filesystem read. It exposes `path() is ArchivePath`, `kind() is ArchiveEntryKind`,
`metadata() is ArchiveMetadata`, and `accept(to is ArchiveEntryVisitor) is void`.
The kind enum contains `file`, `directory`, `symbolic_link`, `hard_link`, `fifo`,
`character_device` and `block_device`.

Concrete values bind a validated path and metadata. `ArchiveFile(path, metadata,
data is @byte)` additionally retains exact immutable bytes, including empty and
high-bit values. Only this concrete entry exposes `data() is @byte`. A directory
or FIFO has no data observation; neither does a link or device. Constructors
perform no filesystem access, permission changes, device creation or extraction.
File data is already in memory; aggregate memory limits belong to archive readers.

`ArchiveSymbolicLink(path, metadata, target is ArchivePath)` and
`ArchiveHardLink(path, metadata, target is ArchivePath)` remain distinct concrete
types/kinds. Their `target()` preserves the supplied path. A symbolic target is
relative to the link's parent; a hard-link target is relative to the archive root.
This initial subset accepts only the strict `ArchivePath` grammar, so absolute
and parent-traversing symbolic targets cannot be represented. Values neither
resolve targets nor check existence, cycles or extraction containment.

`ArchiveDevice(path, kind, metadata, major is int, minor is int)` requires kind
`character_device` or `block_device` and nonnegative numbers. Invalid kind is
checked first; `InvalidArchiveDevice(message)` exposes `text()` as
`"invalid archive device: " + message`, with validation reasons `expected character
or block kind` and `negative device number`. `major()`/`minor()` retain exact
integers, without narrowing to any OS device-number encoding.

`accept()` invokes exactly one nominal visitor action: `file(ArchiveFile)`,
`directory(ArchiveDirectory)`, `symbolic_link(ArchiveSymbolicLink)`,
`hard_link(ArchiveHardLink)`, `fifo(ArchiveFifo)` or `device(ArchiveDevice)`, all
returning void. The concrete value passes itself unchanged. Visitors control
output/effects, may retain the value, and their errors propagate unchanged. Each
call repeats the event; no traversal or data consumption occurs. This protocol
lets heterogeneous consumers access variant-specific data without reflection or
unsupported downcasts. Kind observations alone do not narrow a role value.

`ArchiveEntry.fake()` represents a file named `entry` with epoch metadata and
empty content; it emits a matching concrete `ArchiveFile` to the visitor. It is
a test double, not archive parsing or a filesystem resource.

## `archive/limits`

`ArchiveLimits(entries is int, entry_size is int, total_size is int,
path_length is int)` is an immutable admission policy. All four bounds must be
positive; the first invalid coordinate (in constructor order) raises
`InvalidArchiveLimit(limit, maximum)`. Entry and total sizes count uncompressed
payload bytes. Path length counts Unicode code points in the supplied spelling,
including separators; it does not count UTF-8 bytes or normalize names.
The bounds are independent: an entry-size maximum may exceed the total maximum.

The four same-named observations retain the configured bounds. `path(origin is
string) is ArchivePath` checks length **before** constructing/validating the
archive path, then propagates `UnsafeArchivePath` unchanged. It is also usable
for link targets. `budget() is ArchiveBudget` starts an independent allowance.

`ArchiveBudget(limits is ArchiveLimits)` tracks remaining entry slots and total
payload bytes, with at most one active entry. All transitions return successors;
they perform no I/O and allocate no payload. Callers must retain each successor
and check it before appending/allocating entry data; reusing an earlier budget
deliberately creates an independent branch, not shared mutable enforcement.

- `entered(origin is string, declared_size is int) is ^ArchiveBudget` requires
  no active entry and a nonnegative declared size. It checks entry count, path
  length/grammar, declared entry size and remaining total, in that order, before
  spending one entry slot and retaining the validated current path. It reserves
  no payload bytes. Empty files and non-file entries still cost one slot.
- `expanded(amount is int) is ^ArchiveBudget` requires an active entry and a
  nonnegative actual chunk size. Before a chunk is retained, it checks remaining
  entry bytes then remaining total bytes, and spends both. Zero is allowed.
  A small/zero header declaration cannot bypass this check. The header is only
  an early rejection hint; exact declared/actual equality is a format invariant
  checked by a parser, not by this generic budget.
- `completed() is ^ArchiveBudget` requires an active entry and ends it. It
  never restores entry slots or total bytes. The next entry gets a fresh
  per-entry allowance. Draining/discarding output must not reset the budget.

`entries_remaining()` and `total_remaining()` are always available. `path()` and
`entry_remaining()` require an active entry, or raise `error/InvalidState`.
Entering while active or expanding/completing without an entry also raises
`InvalidState`. Negative sizes raise `error/InvalidArgument` after the phase
check. Errors leave the original immutable budget intact; comparisons happen
before subtraction, avoiding counter overflow even at INT64_MAX.

`ArchiveLimitExceeded(limit is string, maximum is int)` and
`InvalidArchiveLimit(limit is string, maximum is int)` retain `limit() is string`
and `maximum() is int`. The policy uses labels `entries`, `entry_size`,
`total_size`, `path_length`. Stable texts are respectively `"archive limit
exceeded: " + limit + " (maximum " + maximum.string() + ")"` and `"archive limit
" + limit + " must be positive, got " + maximum.string()`. Exceeded errors keep
the original configured maximum, not the currently remaining allowance.

```okkama
let initial = ArchiveLimits(10, 1024, 4096, 255).budget();
let entry = initial.entered("assets/icon.bin", 2);
let consumed = entry.expanded(2); // check before retaining the actual payload
let ready = consumed.completed();
ready.total_remaining();         // 4094
```

These are policy/accounting primitives, not a TAR/ZIP parser or an expansion
codec. Future readers must call them at each admission/allocation boundary,
count actual expanded bytes across all members, and separately bound raw input,
header/metadata decoding and temporary codec memory. The module neither trusts
header sizes as actual consumption nor retroactively bounds existing byte arrays.

## `archive/stream`

`ReadingArchive` is a changing, consuming resource with `entry() is ArchiveEntry`,
`finished() is bool`, `close() is void` and `closed() is bool`. Each entry call
returns one independent immutable member. Normal completion raises
`EndOfArchive()` (`text(): "end of archive"`); subsequent calls repeat EOF without
revisiting the input. Completion is known only after the terminator/EOF has been
observed. `finished()` is a passive observation, never a prefetch or blocking
read. These resources are not iterable cursors.

`WritingArchive` exposes `write(entry is ArchiveEntry) is void`, `finish() is
void`, `finished() is bool`, `close() is void` and `closed() is bool`. Writes are
explicit consuming commands. Successful finish writes the format terminator
and performs the format's documented flushing before setting finished. Finishing
an empty archive is valid. Writes and repeated finish after completion raise
`error/InvalidState` before any callback/output work.

Both roles borrow their byte-stream collaborators: neither success nor failure
implicitly closes them. Explicit close is idempotent and releases only private
adapter state; close never means finish. After close, only close/closed are valid.
Processing errors other than a reader's normal `EndOfArchive` propagate unchanged
and make the adapter unusable except for close/closed. In particular, failed
finish is not success, and partial writes are not retried or rolled back.

`ArchiveReading(next_entry is () is ArchiveEntry)` implements `ReadingArchive`
and `io/Closing`. It binds a synchronous, one-entry operation without calling it.
Each active `entry()` invokes that operation exactly once. `EndOfArchive` marks
normal completion; other `Error` objects mark failure. The adapter retains no
returned entry and never prefetches the next one. Returned entries can outlive
the adapter. It owns and destroys only its private lifecycle buffer.

`ArchiveWriting(write_entry is (ArchiveEntry) is void, finish_stream is () is void)`
implements `WritingArchive` and `io/Closing`. Construction binds both operations
without invoking them. A write passes the exact supplied entry once; a finish
invokes its operation once and becomes finished only after success. The adapter
does not additionally flush or close a sink: those are explicit format/caller
capabilities. It owns and destroys only its lifecycle buffer. Both adapters
retain operation closures according to the language's ordinary ownership rules;
callers continue to own borrowed sources/sinks. Callback effects remain explicit
and must honor the same borrowing contract.

The `ReadingArchive` role fake yields one empty file named `entry`, then normal
EOF; no prefetch occurs. The writer fake accepts entries and finishes without
producing a wire format. Their mutable test storage is per fake instance.

These shared adapters enforce lifecycle, single-entry invocation and error
behavior. **A callback must enforce ArchiveLimits before allocating payload**;
post-validating a returned entry would be too late. Actual bounded materialization,
truncated format detection and wire-level borrowed I/O must also be tested in
TarReading/ZipReading and their writers. TarReading now verifies these properties
with independent wire fixtures and a fragmented borrowed source; ZipReading is
still pending. No fake or callback adapter is itself a TAR/ZIP implementation.
