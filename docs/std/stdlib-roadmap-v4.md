# Okkama Standard Library — v4 object architecture

Status: architectural contract and implementation checklist, 2026-09-11.
This supersedes v3 as the current design. V3 is retained as historical scope,
not as an approved API or a promise that all its proposed modules exist.

Read [design principles](stdlib-design-principles.md) and the
[complete public API audit](stdlib-api-audit-v4.md), [current API](stdlib-api-v4.md),
and [detailed completion TODO](stdlib-completion-todo-v4.md) together with this roadmap.
The audit accounts for every declaration and public signature in the initial
`std/` tree, including infrastructure that is importable because Okkama has no
module-private declarations. A separate substrate table records native modules.

## Delivery update — 2026-10-23

`crypto/*` and `digest/sha2|sha3|shake|blake2|read` are implemented as std
objects over the reviewed host provider `okkama-native` (RustCrypto, C ABI),
as the 2026-09-25 decision required; see the [crypto guide](crypto-guide.md).
`db/postgres` is a PostgreSQL protocol driver written in Okkama over
`net/*` and `crypto/*`; see the [PostgreSQL guide](db-postgres.md).

## Delivery update — 2026-09-25

Remaining digest algorithms/adapters and all `crypto/*` are deferred by user
choice. Resume them through reviewed host crypto implementations with std object
contracts; do not continue implementing cryptographic primitives in pure Okkama.
`compression/state` and `compression/read` are verified. Concrete compression
formats are also deferred by user choice pending host codec libraries. Remaining
archive work is now deferred as well, including ZIP wire APIs and extraction;
`parallel/map` now provides bounded ordered process batches with awaited cleanup.
Its runtime prerequisites were fixed separately. `net/failure` is verified;
the next active section is `net/ip`.
Archive path, metadata, entry, limit-accounting, stream lifecycle, TAR value/stream
APIs and ZIP writing choices remain verified. Concrete bounded wire-entry
materialization is verified for TAR. The missing no-follow regular-file capability
still blocks `archive/file`; deferred checklists remain open.
Checksum modules remain active.
Existing implemented digest modules are retained; deferred tasks are not complete.
See the [queue decision](stdlib-completion-todo-v4.md#криптография-отложена--решение-2026-09-25)
for scope and dependency handling. The user authorized process-worker runtime
fixes with full regression; that does not resume the deferred archive/crypto/codecs.
A host-backed boundary can use an established library's higher-level interfaces;
[OpenSSL's libcrypto guide](https://docs.openssl.org/3.5/man7/ossl-guide-libcrypto-introduction/)
describes such interfaces. Provider choice and security properties require a
separate review when this scope resumes.

## Architecture and delivery order

The unit of design is a represented entity, observation, immutable composition
or explicit capability. Python breadth and Go/Java/Rust API familiarity are not
architectural acceptance criteria. No universal data/value/manager interfaces.

1. **Representation and immutable composition:** collections, paths, civil time,
   numeric values, encodings, regex patterns and explicit structured documents.
2. **Observation and effect capabilities:** streams, clocks, randomness,
   filesystem/process resources and synchronization. Effects are stated at the
   capability boundary; observations distinguish snapshots from live resources.
3. **Composition adapters:** byte/text adapters, parsing/rendering, file views,
   explicit error translations. They preserve the boundary's nominal contract.
4. **Private substrate:** existing `runtime/*` opaque states and portable wire
   values. No high-level state getter or arbitrary object reflection.
5. **Deferred protocols:** networking, TLS, archives, databases and the remaining
   v3 families require independently tested vertical slices, not placeholder
   DTO/service APIs or copied host APIs. Their scope ledger is below.

## Required redesigns

These decisions were written before implementation. All twelve redesigns below
are implemented and have regression tests; no compatibility aliases. The separate
completion TODO covers further work, without treating v3 signatures as approved.

- [x] **File capability composition (`fs/file`).** Replace the mixed `File`
  read/write facade with an owner exposing `binary() is FileBytes`,
  `text() is FileText`, `closed() is bool`, `close() is void`.
  `FileBytes does Reading, Writing`: `bytes(limit)`, `write(ByteSource)`,
  `flush()`. `FileText does ReadingText, WritingText`: `line()`, `text()`,
  `write(...string)`, `writeln(...string)`, `flush()`. View constructors accept
  the owner, not a public native handle. Views retain its exact state; reads
  share position, close is shared, unused close does not open the path. Mode
  and encoding errors stay typed; views do not silently switch the data mode.
  `FilePosition` and file locks remain explicit collaborators. Test borrowed
  views, escaped views, mode mismatch, short writes, EOF and repeated close.
- [x] **Directory trees (`fs/directory`).** Remove `RemovedTree` and its effectful
  projection. `DirectoryTree(origin is DirectoryLocating).remove() is void`
  denotes the tree being deleted. Construction does nothing; removal preserves
  existing symlink-leaf semantics and portable errors. No result masquerading
  as a deleted path. Test construction without deletion and command effects.
- [x] **Temporary preservation (`fs/temp`).** Remove `PreservedTemporaryFile`.
  `TemporaryFilePreservation(origin is TemporaryFile, destination is Path)`
  has `preserve() is void` and `destination() is FileEntry`. The latter is a
  pure locator query, never preservation. Ownership transfers only on the
  explicit command; failure leaves normal temporary cleanup active. No double
  close/unlink after preservation. Test normal, failed and abandoned plans.
- [x] **Environment (`os/environment`).** Rename persistent `add`/`diff` to
  `addition`/`difference`; retain the original snapshot under chaining. Keep
  explicit missing-variable errors and typed normalization. Maps here denote
  the complete OS environment snapshot, not arbitrary object's private fields.
- [x] **Storage observations (`os/storage`, `numbers/bigint`).** Store capacity
  and available bytes, derive `used()` by exact arbitrary-precision difference.
  Reject negatives or available > capacity in the constructor. Provide
  `fits(amount is BigInt) is bool`, rejecting negative requests. Remove the
  redundant `used` constructor argument. Complete `BigDifference(left,right)`
  with a portable string-based native arithmetic primitive and exact tests.
- [x] **Fitted behavior (`statistics`).** Replace `LinearRegression` with
  `FittedLine`: retain meaningful `slope()`/`intercept()` and add
  `prediction(at is float)` and `residual(at is float, observed is float)`.
  A line represents a mathematical relation, not a transport record. Preserve
  finite arithmetic, paired-sample validation and immutable input snapshots.
- [x] **File identity (`fs/stat`).** Replace public device/inode getters with
  `same(other is FileIdentity) is bool`. Identity coordinates stay private;
  consumers compare identities instead of reproducing host identity rules.
  Preserve explicit missing creation time and permission observations.
- [x] **Regex encapsulation (`regex/pattern`, `regex/search`).** Remove
  `Regex.state()`. Have the pattern create its behavioral query observation without
  private peer access. Public queries use `RegexQuery` cases, not integer
  engine modes. Keep bounded matching, lazy compilation and failure memoization.
  No adapter obtains or exposes a handle through a high-level method.
- [x] **PEM representation (`encoding/pem`).** Replace ambiguous `data()` with
  `bytes()` and implement `ByteSource`; a parsed PEM block can feed `Writing`
  directly. The label is protocol metadata; decoded bytes remain validated.
- [x] **Text formatting (`strings/format`).** Remove broken `PercentFormatted`
  and `UnsupportedFormatValue`, including the missing `runtime/format` import.
  `Formatted(pattern is string, values is @string).string()` substitutes exact
  `{}` slots, keeps all other text literal, and throws `InvalidFormat` on count
  mismatch. No printf type dispatch, arbitrary `any`, reflection or locale.
  Explicit `.string()` conversion belongs to the caller. Test Unicode, empty
  slots/arguments, brace literals, mismatch and repeated observation.
- [x] **UTF-8 output (`io/text`).** Complete `Utf8Output(origin is Writing)` as a
  `WritingText` adapter: strict encoding, exact ordering, one newline for
  `writeln`, no ownership/implicit close of origin. Test through ByteBuffer.
- [x] **JSON behavior (`json/value`, `json/parse`, `json/write`).** Replace
  `HandlingJsonValue.handle(...)` with explicit `JsonWriting` protocol actions
  `null_value`, `boolean`, `number`, `text`, `elements`, `members`, all void.
  `JsonValue.print(to is JsonWriting)` controls protocol emission. Remove scalar
  wrapper `value()` getters. Retain JSON-kind observations as wire-grammar
  categories, not Okkama type reflection, and object member selection as a
  domain operation. Complete `CompactJson(origin is JsonValue).string()` with
  correct escaping and exact Decimal output, using the explicit protocol.
  Test custom JsonValue implementations, nested values, duplicate keys,
  Unicode/control escapes, parse/render roundtrips and invalid input. Do not
  introduce arbitrary-object serialization. Pretty printing stays deferred.

### Resource ownership correction found by lifetime tests

Native handles are not language ObjectValues and are not part of the interpreter's
scope destroy graph; the registry finalizes them at interpreter teardown. Therefore
scope RAII must be represented by std owners with `destroy { self.close(); }`.
File, TemporaryFile, TemporaryDirectory and FileLock require this explicit cleanup.
Byte/text/position/lock views and temporary preservation/path collaborators retain
the owner object as well as any private native state, so escapes transfer the owner
through the existing object graph. A plain returned Path does not retain a resource.
Tests must cover abandoned plans/directories, escaped owners/views, borrowed scope
exit, repeated close and acquisition failure. No global runtime lifetime rewrite.

## Kept contracts and explicit compromises

- Iterating/Cursor and persistent collection roles are behavioral contracts;
  `available`/`item`/`successor` denote immutable traversal positions. `Pair`,
  `Indexed`, `Frequency` are typed positions/associations needed by these
  contracts, not untyped application DTOs. Their selectors are intrinsic to
  the represented relation. Do not add a generic reflection/record protocol.
- `Path`, `Duration`, date/time, Decimal and UUID observations are meaningful
  coordinates or canonical representations, backed by validation/comparison.
  A zero-argument constant/empty collection is an explicit domain atom, not a
  stateless algorithm service. Computed projections remain valid when pure.
- Error details express a typed failure and stable diagnostics. A JSON null
  literal, EOF and empty collections have domain meaning; they are not absent
  Okkama values. Internal search indices may use sentinels; existing diagnostic
  source-position sentinels are explicit migration debt in the completion TODO,
  not a general absence model.
- Stream reads/random draws, async waits and guard acquisition are explicit
  capability effects that must return observations. They are documented
  adaptations of strict builder/manipulator separation. Async void does not
  exist; some existing async commands return their receiver as a documented temporary compromise. New contracts should use meaningful completion results; this is not an EO requirement.
- Native state allocation, immutable materialization and current temporary
  resource acquisition are documented constructor compromises. Do not hide
  unrelated effects or move them into constructors merely to pass a style check.
- Native-only adapters and persistent cursor/search state have implementation
  coordinates. Okkama lacks module-private types, primary-constructor
  delegation and library-defined generic methods; do not fake these features.
  The audit marks this infrastructure and its intended boundary explicitly.
- The process command is an immutable invocation specification consumed by a
  single process adapter; completed process output is a coherent immutable
  observation of exit/stdout/stderr. These domain-specific boundaries may
  expose explicit byte/text representations. They are not permission for
  general-purpose data objects or service locators.

## Module contracts

Each module entry below identifies the actual represented objects and public
behavior in the audit, its native boundary, effect/ownership policy, tests and
non-goals. Revised signatures above override the initial inventory. Future
modules have no implementation checkbox until their slice is actually tested.


### `async/barrier`

**Represents:** coordination capabilities and future compositions.

**Roles/objects:** `Barrier`, `BarrierState`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

**Native dependencies:** No direct runtime import; VM intrinsic dispatch is required and covered by the module tests.

**Testing and non-goals:** Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors. Tests: `244-std-v3-async-barrier.t`.

### `async/cancellation`

**Represents:** coordination capabilities and future compositions.

**Roles/objects:** `Cancelling`, `Cancellation`, `CancellationToken`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

**Native dependencies:** No direct runtime import; VM intrinsic dispatch is required and covered by the module tests.

**Testing and non-goals:** Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors. Tests: `163-std-v3-async-cancellation.t`.

### `async/channel`

**Represents:** coordination capabilities and future compositions.

**Roles/objects:** `Channel`, `Sending`, `Receiving`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

**Native dependencies:** No direct runtime import; VM intrinsic dispatch is required and covered by the module tests.

**Testing and non-goals:** Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors. Tests: `04-async-composition.t`, `08-successor-field-lifetimes.t`, `09-async-lexical-blocks.t`, `162-async-block-adapter.t`, `234-std-v3-async-channel.t`, `235-std-v3-async-scope-group.t`, `236-std-v3-async-future-scope.t`, `237-std-v3-async-all-futures.t`, `238-std-v3-async-future-selection.t`, `239-std-v3-async-future-completions.t`, `240-std-v3-async-timeout.t`, `241-std-v3-async-lock.t`.

### `async/collect`

**Represents:** coordination capabilities and future compositions.

**Roles/objects:** `AllFutures`, `Raced`, `FirstSuccessful`, `FutureCompletions`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

**Native dependencies:** No direct runtime import; composed imports: `collections/sequence`; VM intrinsic dispatch is required and covered by the module tests.

**Testing and non-goals:** Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors. Tests: `237-std-v3-async-all-futures.t`, `238-std-v3-async-future-selection.t`, `239-std-v3-async-future-completions.t`.

### `async/event`

**Represents:** coordination capabilities and future compositions.

**Roles/objects:** `Event`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

**Native dependencies:** No direct runtime import; VM intrinsic dispatch is required and covered by the module tests.

**Testing and non-goals:** Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors. Tests: `13-void-stdlib.t`, `243-std-v3-async-event.t`.

### `async/lock`

**Represents:** coordination capabilities and future compositions.

**Roles/objects:** `Lock`, `LockGuard`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

**Native dependencies:** No direct runtime import; composed imports: `io/stream`; VM intrinsic dispatch is required and covered by the module tests.

**Testing and non-goals:** Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors. Tests: `241-std-v3-async-lock.t`.

### `async/scope`

**Represents:** coordination capabilities and future compositions.

**Roles/objects:** `Supervising`, `FutureGroup`, `FutureScope`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

**Native dependencies:** No direct runtime import; VM intrinsic dispatch is required and covered by the module tests.

**Testing and non-goals:** Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors. Tests: `235-std-v3-async-scope-group.t`, `236-std-v3-async-future-scope.t`.

### `async/semaphore`

**Represents:** coordination capabilities and future compositions.

**Roles/objects:** `Semaphore`, `Permit`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

**Native dependencies:** No direct runtime import; composed imports: `io/stream`; VM intrinsic dispatch is required and covered by the module tests.

**Testing and non-goals:** Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors. Tests: `242-std-v3-async-semaphore.t`.

### `async/timeout`

**Represents:** coordination capabilities and future compositions.

**Roles/objects:** `Within`, `Before`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

**Native dependencies:** No direct runtime import; composed imports: `error`, `time/duration`, `time/clock`.

**Testing and non-goals:** Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors. Tests: `240-std-v3-async-timeout.t`.

### `calendar`

**Represents:** validated calendar representations.

**Roles/objects:** `YearMonth`, `MonthDates`, `MonthDateCursor`, `CalendarWeek`, `CalendarWeekCursor`, `CalendarMonth`, `CalendarMonthCursor`, `CalendarMonthLayout`, `IsoWeek`, `LeapYear`, `WeekdayIndex`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** No direct runtime import; composed imports: `collections/iteration`, `collections/sequence`, `compare`, `error`, `size`, `time/date`.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `210-std-v3-calendar.t`.

### `collections/combinatorics`

**Represents:** immutable sequences, associations and traversal.

**Roles/objects:** `Combinations`, `CombinationCursor`, `CombinationsWithReplacement`, `ReplacementCombinationCursor`, `Permutations`, `PermutationCursor`, `CartesianProduct`, `CartesianCursor`, `CombinationPositions`, `CombinationPivot`, `ReplacementPositions`, `ReplacementPivot`, `PermutationPositions`, `PrefixMembership`, `PermutationCandidate`, `PermutationPivot`, `PrefixBefore`, `PermutationAvailableIndex`, `CartesianAvailability`, `CartesianPivot`, `CartesianPositions`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

**Native dependencies:** No direct runtime import; composed imports: `collections/iteration`, `collections/sequence`, `error`.

**Testing and non-goals:** Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade. Tests: `175-std-v3-collections-combinatorics.t`.

### `collections/combining`

**Represents:** immutable sequences, associations and traversal.

**Roles/objects:** `Pair`, `Zipped`, `ZippedCursor`, `StrictlyZipped`, `StrictlyZippedCursor`, `Chained`, `ChainedCursorSearch`, `ChainedCursorCandidates`, `ChainedCursorCandidate`, `ChainedCursorCandidateValue`, `ExhaustedChainedCursor`, `ChainedItemCursor`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

**Native dependencies:** No direct runtime import; composed imports: `collections/iteration`.

**Testing and non-goals:** Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade. Tests: `165-std-v3-collections-combining.t`, `194-std-v3-math-aggregate.t`, `195-std-v3-statistics.t`.

### `collections/deque`

**Represents:** immutable sequences, associations and traversal.

**Roles/objects:** `Deque`, `DequeFront`, `DequeBack`, `DequeItems`, `ConcatenatedDequeCursor`, `ArrayItems`, `ArrayCursor`, `ForwardList`, `ReversedList`, `DequeParts`, `BalancedDeque`, `NormalizedDeque`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

**Native dependencies:** No direct runtime import; composed imports: `collections/iteration`, `collections/list`, `collections/views`, `error`, `size`.

**Testing and non-goals:** Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade. Tests: `170-std-v3-collections-deque.t`.

### `collections/enumeration`

**Represents:** immutable sequences, associations and traversal.

**Roles/objects:** `Indexed`, `Enumerated`, `EnumerationCursor`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

**Native dependencies:** No direct runtime import; composed imports: `collections/iteration`.

**Testing and non-goals:** Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade. Tests: `164-std-v3-collections-enumeration.t`.

### `collections/frequencies`

**Represents:** immutable sequences, associations and traversal.

**Roles/objects:** `Frequency`, `Frequencies`, `FrequencyBuilding`, `FrequencyRevision`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

**Native dependencies:** No direct runtime import; composed imports: `collections/iteration`, `collections/list`, `error`, `size`.

**Testing and non-goals:** Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade. Tests: `173-std-v3-collections-frequencies.t`.

### `collections/heap`

**Represents:** immutable sequences, associations and traversal.

**Roles/objects:** `Heap`, `HeapBuilding`, `HeapRepresenting`, `EmptyHeapTree`, `HeapTreeNode`, `HeapChildren`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

**Native dependencies:** No direct runtime import; composed imports: `collections/iteration`, `collections/sequence`, `compare`, `error`, `size`.

**Testing and non-goals:** Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade. Tests: `172-std-v3-collections-heap.t`.

### `collections/iteration`

**Represents:** immutable sequences, associations and traversal.

**Roles/objects:** `Cursor`, `Iterating`, `CursorItems`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

**Native dependencies:** No direct runtime import.

**Testing and non-goals:** Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade. Tests: `162-async-block-adapter.t`, `164-std-v3-collections-enumeration.t`, `165-std-v3-collections-combining.t`, `166-std-v3-collections-repetition.t`, `167-std-v3-collections-views.t`, `168-std-v3-collections-list.t`, `170-std-v3-collections-deque.t`, `173-std-v3-collections-frequencies.t`, `174-std-v3-collections-ordering.t`, `175-std-v3-collections-combinatorics.t`, `187-std-v3-encoding-pem.t`, `193-std-v3-io-decorators.t`, `194-std-v3-math-aggregate.t`, `195-std-v3-statistics.t`, `198-std-v3-random-distribution.t`, `216-std-v3-fs-glob.t`, `219-std-v3-os-environment.t`, `221-std-v3-os-account.t`, `29-builtin-value-behavior.t`, `84-custom-object-iteration.t`, `88-custom-iterable-functional-operations.t`.

### `collections/list`

**Represents:** immutable sequences, associations and traversal.

**Roles/objects:** `List`, `EmptyList`, `LinkedList`, `EmptyListCursor`, `LinkedListCursor`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

**Native dependencies:** No direct runtime import; composed imports: `collections/iteration`, `size`.

**Testing and non-goals:** Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade. Tests: `168-std-v3-collections-list.t`, `169-std-v3-collections-set.t`, `170-std-v3-collections-deque.t`, `172-std-v3-collections-heap.t`, `173-std-v3-collections-frequencies.t`, `174-std-v3-collections-ordering.t`, `194-std-v3-math-aggregate.t`, `195-std-v3-statistics.t`.

### `collections/ordering`

**Represents:** immutable sequences, associations and traversal.

**Roles/objects:** `Sorted`, `SortedBy`, `Ranked`, `PositionOf`, `LowerBound`, `UpperBound`, `LowerSearching`, `UpperSearching`, `OrderedCursor`, `RankedCursor`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

**Native dependencies:** `runtime/ordering`.

**Testing and non-goals:** Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade. Tests: `174-std-v3-collections-ordering.t`.

### `collections/repetition`

**Represents:** immutable sequences, associations and traversal.

**Roles/objects:** `Repeating`, `RepeatingCursor`, `Repeated`, `RepeatedCursor`, `Cycled`, `CycledCursor`, `EmptyCycleCursor`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

**Native dependencies:** No direct runtime import; composed imports: `collections/iteration`, `collections/sequence`, `error`, `size`.

**Testing and non-goals:** Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade. Tests: `166-std-v3-collections-repetition.t`, `237-std-v3-async-all-futures.t`.

### `collections/sequence`

**Represents:** immutable sequences, associations and traversal.

**Roles/objects:** `Sequencing`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

**Native dependencies:** No direct runtime import; composed imports: `collections/iteration`, `size`.

**Testing and non-goals:** Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade. Tests: `166-std-v3-collections-repetition.t`, `167-std-v3-collections-views.t`, `172-std-v3-collections-heap.t`, `174-std-v3-collections-ordering.t`, `175-std-v3-collections-combinatorics.t`, `193-std-v3-io-decorators.t`, `194-std-v3-math-aggregate.t`, `195-std-v3-statistics.t`, `198-std-v3-random-distribution.t`, `219-std-v3-os-environment.t`, `221-std-v3-os-account.t`, `237-std-v3-async-all-futures.t`, `248-std-v3-json-value.t`, `249-std-v3-json-parse.t`, `29-builtin-value-behavior.t`.

### `collections/set`

**Represents:** immutable sequences, associations and traversal.

**Roles/objects:** `Set`, `OrderedSet`, `Union`, `Intersection`, `Difference`, `SymmetricDifference`, `SetWith`, `SetWithout`, `SetWithCursor`, `ConcatenatedSetCursor`, `SetMembership`, `SetSize`, `AbsentFrom`, `DifferentFrom`, `DistinctList`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

**Native dependencies:** No direct runtime import; composed imports: `collections/iteration`, `collections/list`, `collections/views`, `size`.

**Testing and non-goals:** Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade. Tests: `169-std-v3-collections-set.t`.

### `collections/views`

**Represents:** immutable sequences, associations and traversal.

**Roles/objects:** `CursorStep`, `CursorSteps`, `CursorStepCursor`, `Mapped`, `MappedCursorSearch`, `MappedCursor`, `EmptyMappedCursor`, `Filtered`, `FilteredCursorSearch`, `FilteredCursor`, `EmptyFilteredCursor`, `Taken`, `TakenCursor`, `Dropped`, `DroppedCursorSearch`, `TakenWhile`, `TakenWhileCursorSearch`, `TakenWhileCursor`, `EmptyTakenWhileCursor`, `DroppedWhile`, `DroppedWhileCursorSearch`, `Chunked`, `ChunkedCursor`, `Flattened`, `Pairwise`, `PairwiseCursor`, `Accumulated`, `AccumulatedCursor`, `ExhaustedAccumulatedCursor`, `Reduced`, `Reversed`, `ReversedCursor`, `Sliced`, `SlicedCursor`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

**Native dependencies:** No direct runtime import; composed imports: `collections/iteration`, `collections/enumeration`, `collections/sequence`, `collections/combining`, `error`, `size`.

**Testing and non-goals:** Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade. Tests: `167-std-v3-collections-views.t`.

### `compare`

**Represents:** small nominal compare contracts.

**Roles/objects:** `Comparing`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** No I/O or owned resource state. Preserve interpreter-provided nominal behavior.

**Native dependencies:** No direct runtime import.

**Testing and non-goals:** Existing conformance, error and ordering tests; no generic reflection/base-object API. Tests: `171-generic-bounds-and-builtin-comparing.t`, `172-std-v3-collections-heap.t`, `174-std-v3-collections-ordering.t`, `199-std-v3-uuid-value.t`, `201-std-v3-time-duration.t`, `202-std-v3-time-clock.t`, `203-std-v3-time-date.t`, `204-std-v3-time-day.t`, `205-std-v3-time-local-datetime.t`, `206-std-v3-time-zone.t`, `207-std-v3-time-iso.t`, `210-std-v3-calendar.t`, `211-std-v3-fs-path-core.t`, `247-std-v3-math-decimal.t`.

### `encoding/base32`

**Represents:** validated wire/text representations.

**Roles/objects:** `Base32Encoded`, `Base32Decoded`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

**Native dependencies:** `runtime/base32`.

**Testing and non-goals:** Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion. Tests: `183-std-v3-encoding-base32.t`.

### `encoding/base64`

**Represents:** validated wire/text representations.

**Roles/objects:** `Base64Encoded`, `Base64Decoded`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

**Native dependencies:** `runtime/base64`.

**Testing and non-goals:** Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion. Tests: `184-std-v3-encoding-base64.t`.

### `encoding/base85`

**Represents:** validated wire/text representations.

**Roles/objects:** `Base85Encoded`, `Base85Decoded`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

**Native dependencies:** `runtime/base85`.

**Testing and non-goals:** Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion. Tests: `185-std-v3-encoding-base85.t`.

### `encoding/binary`

**Represents:** validated wire/text representations.

**Roles/objects:** `IntegerFromBytes`, `IntegerBytes`, `FloatFromBytes`, `FloatBytes`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

**Native dependencies:** `runtime/binary`.

**Testing and non-goals:** Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion. Tests: `188-std-v3-encoding-binary.t`.

### `encoding/hex`

**Represents:** validated wire/text representations.

**Roles/objects:** `HexEncoded`, `HexDecoded`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

**Native dependencies:** `runtime/hex`.

**Testing and non-goals:** Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion. Tests: `182-std-v3-encoding-hex.t`.

### `encoding/pem`

**Represents:** validated wire/text representations.

**Roles/objects:** `PemEncoded`, `PemBlock`, `PemBlocks`, `PemBlockCursor`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

**Native dependencies:** `runtime/pem`.

**Testing and non-goals:** Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion. Tests: `16-stdlib-v4.t`, `187-std-v3-encoding-pem.t`.

### `encoding/quoted_printable`

**Represents:** validated wire/text representations.

**Roles/objects:** `QuotedPrintableEncoded`, `QuotedPrintableDecoded`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

**Native dependencies:** `runtime/quoted_printable`.

**Testing and non-goals:** Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion. Tests: `186-std-v3-encoding-quoted-printable.t`.

### `encoding/single_byte`

**Represents:** validated wire/text representations.

**Roles/objects:** `AsciiEncoded`, `AsciiDecoded`, `Latin1Encoded`, `Latin1Decoded`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

**Native dependencies:** `runtime/single_byte`.

**Testing and non-goals:** Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion. Tests: `181-std-v3-encoding-single-byte.t`.

### `encoding/utf`

**Represents:** validated wire/text representations.

**Roles/objects:** `Utf8Encoded`, `Utf8Decoded`, `Utf16Encoded`, `Utf16Decoded`, `Utf32Encoded`, `Utf32Decoded`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

**Native dependencies:** `runtime/utf`.

**Testing and non-goals:** Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion. Tests: `06-pipeline-drain.t`, `16-stdlib-v4.t`, `180-std-v3-encoding-utf.t`, `188-std-v3-encoding-binary.t`, `191-std-v3-io-stdin.t`, `192-std-v3-io-buffer.t`, `220-std-v3-os-platform.t`, `224-std-v3-os-process.t`, `225-std-v3-os-command-capture.t`, `226-std-v3-os-pipeline-capture.t`, `249-std-v3-json-parse.t`.

### `error`

**Represents:** small nominal error contracts.

**Roles/objects:** .

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** No I/O or owned resource state. Preserve interpreter-provided nominal behavior.

**Native dependencies:** No direct runtime import.

**Testing and non-goals:** Existing conformance, error and ordering tests; no generic reflection/base-object API. Tests: `166-std-v3-collections-repetition.t`, `167-std-v3-collections-views.t`, `170-std-v3-collections-deque.t`, `173-std-v3-collections-frequencies.t`, `174-std-v3-collections-ordering.t`, `175-std-v3-collections-combinatorics.t`, `176-std-v3-text-layout.t`, `188-std-v3-encoding-binary.t`, `190-std-v3-io-stdout.t`, `193-std-v3-io-decorators.t`, `194-std-v3-math-aggregate.t`, `195-std-v3-statistics.t`, `198-std-v3-random-distribution.t`, `200-std-v3-uuid-generation.t`, `210-std-v3-calendar.t`, `219-std-v3-os-environment.t`, `221-std-v3-os-account.t`, `240-std-v3-async-timeout.t`.

### `fs/directory`

**Represents:** filesystem locators, observations and owned resources.

**Roles/objects:** `DirectoryLocating`, `Directory`, `DirectoryEntry`, `DirectoryEntries`, `DirectoryEntryCursor`, `Descendants`, `DescendantCursor`, `DescendantCursorFrom`, `ExhaustedDescendantCursor`, `DirectoryTree`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

**Native dependencies:** `runtime/fs`.

**Testing and non-goals:** Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries. Tests: `16-stdlib-v4.t`, `214-std-v3-fs-directory.t`, `215-std-v3-fs-temp.t`, `216-std-v3-fs-glob.t`, `218-std-v3-fs-watch.t`, `222-std-v3-os-directory.t`, `223-std-v3-os-command.t`.

### `fs/file`

**Represents:** filesystem locators, observations and owned resources.

**Roles/objects:** `FileLocking`, `File`, `FileBytes`, `FileText`, `FilePosition`, `FileEntry`, `FileContent`, `ConfiguredFileState`, `SupportedFileEncoding`, `FileModeCode`, `SeekOriginCode`, `FileError`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

**Native dependencies:** `runtime/fs`.

**Testing and non-goals:** Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries. Tests: `16-stdlib-v4.t`, `212-std-v3-fs-file.t`, `213-std-v3-fs-lock.t`, `215-std-v3-fs-temp.t`, `216-std-v3-fs-glob.t`, `217-std-v3-fs-link.t`, `218-std-v3-fs-watch.t`.

### `fs/glob`

**Represents:** filesystem locators, observations and owned resources.

**Roles/objects:** `MatchingPaths`, `MatchingPathCursor`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

**Native dependencies:** `runtime/fs`.

**Testing and non-goals:** Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries. Tests: `216-std-v3-fs-glob.t`.

### `fs/link`

**Represents:** filesystem locators, observations and owned resources.

**Roles/objects:** `SymbolicLink`, `HardLink`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

**Native dependencies:** `runtime/fs`.

**Testing and non-goals:** Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries. Tests: `217-std-v3-fs-link.t`.

### `fs/lock`

**Represents:** filesystem locators, observations and owned resources.

**Roles/objects:** `FileLock`, `FileLockModeCode`, `FileLockWaitingCode`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

**Native dependencies:** `runtime/fs`.

**Testing and non-goals:** Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries. Tests: `16-stdlib-v4.t`, `213-std-v3-fs-lock.t`.

### `fs/path`

**Represents:** filesystem locators, observations and owned resources.

**Roles/objects:** `Path`, `NativePathStyle`, `PathStyleCode`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

**Native dependencies:** `runtime/fs`, `runtime/system`.

**Testing and non-goals:** Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries. Tests: `06-pipeline-drain.t`, `16-stdlib-v4.t`, `211-std-v3-fs-path-core.t`, `212-std-v3-fs-file.t`, `213-std-v3-fs-lock.t`, `214-std-v3-fs-directory.t`, `215-std-v3-fs-temp.t`, `216-std-v3-fs-glob.t`, `217-std-v3-fs-link.t`, `218-std-v3-fs-watch.t`, `223-std-v3-os-command.t`, `224-std-v3-os-process.t`, `225-std-v3-os-command-capture.t`, `226-std-v3-os-pipeline-capture.t`, `227-std-v3-os-storage.t`.

### `fs/stat`

**Represents:** filesystem locators, observations and owned resources.

**Roles/objects:** `FileStat`, `FileTimes`, `FileIdentity`, `Permissions`, `PermissionMode`, `FileStatFromMap`, `EpochInstant`, `PermissionBits`, `FileKindFromCode`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

**Native dependencies:** No direct runtime import; composed imports: `numbers/bigint`, `time/clock`, `time/duration`.

**Testing and non-goals:** Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries. Tests: `16-stdlib-v4.t`, `212-std-v3-fs-file.t`, `214-std-v3-fs-directory.t`.

### `fs/temp`

**Represents:** filesystem locators, observations and owned resources.

**Roles/objects:** `TemporaryFile`, `TemporaryFilePath`, `TemporaryFilePreservation`, `TemporaryDirectory`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

**Native dependencies:** `runtime/fs`.

**Testing and non-goals:** Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries. Tests: `16-stdlib-v4.t`, `215-std-v3-fs-temp.t`.

### `fs/watch`

**Represents:** filesystem locators, observations and owned resources.

**Roles/objects:** `FileEvent`, `FileWatch`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

**Native dependencies:** `runtime/fs`.

**Testing and non-goals:** Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries. Tests: `218-std-v3-fs-watch.t`.

### `html/entities` — escaping and text-context decoding VERIFIED

**Represents:** immutable HTML character-escaped text, `EscapedHtml(origin)` or
`EscapedHtml(origin, quotes)`, observed through `string()`. Default quotes=false.

**Effects and ownership:** State-only constructors. Escaping uses pure Okkama;
UnescapedHtml uses the existing runtime/html text decoder with literal markup
protected. Each nonempty decoding query creates a private registry-owned event
handle retained until teardown. No external I/O, contextual validation, trusted
markup or sanitization claim; the guide records cumulative allocation behavior.

- [x] EscapedHtml with exact ampersand/angle-bracket and optional quote escaping.
- [x] Contract comments, consumer RED/GREEN, embedded tests and executable example.
- [x] UnescapedHtml: all pinned named references, numeric and text-context recovery,
  one-pass decoding and preservation of literal markup; RED/GREEN and inline tests.

[Contract](html-entities-guide.md), escaping evidence,
decoding evidence.

### `html/events`

**Represents:** HTML token observations and explicit event protocols.

**Roles/objects:** `HtmlAttribute`, `StartingHtmlTag`, `EndingHtmlTag`, `ContainingHtmlText`, `ContainingHtmlComment`, `DeclaringHtml`, `ProcessingHtmlInstruction`, `HandlingHtmlEvent`, `HtmlEvent`, `HtmlStartTag`, `HtmlEndTag`, `HtmlText`, `HtmlComment`, `HtmlDeclaration`, `HtmlProcessingInstruction`, `HtmlEvents`, `HtmlEventCursor`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Immutable event sequence over private parser state. Grammar kind is a wire category, not object-class reflection.

**Native dependencies:** `runtime/html`.

**Testing and non-goals:** Malformed text, event ordering and typed event sinks; no browser DOM, arbitrary field inspection or sanitization promise. Tests: `246-std-v3-html-events.t`.

### `io/buffer`

**Represents:** explicit stream capabilities and decorators.

**Roles/objects:** `ByteBuffer`, `BufferReading`, `BufferWriting`, `BufferClosing`, `BufferContent`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Reads/writes are capability effects. Borrowed adapters never close origins; owned buffers/states clean up deterministically.

**Native dependencies:** `runtime/buffer`.

**Testing and non-goals:** ByteBuffer fakes and real stdio, short writes, bounded reads, EOF, aliases; no format/type inference or hidden ownership transfer. Tests: `13-void-stdlib.t`, `16-stdlib-v4.t`, `192-std-v3-io-buffer.t`, `193-std-v3-io-decorators.t`, `246-std-v3-html-events.t`, `248-std-v3-json-value.t`, `249-std-v3-json-parse.t`.

### `io/decorators`

**Represents:** explicit stream capabilities and decorators.

**Roles/objects:** `BufferedReading`, `LimitedReading`, `ChainedReading`, `ByteSnapshot`, `TeeWriting`, `Discarding`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Reads/writes are capability effects. Borrowed adapters never close origins; owned buffers/states clean up deterministically.

**Native dependencies:** `runtime/buffer`, `runtime/io_decorators`.

**Testing and non-goals:** ByteBuffer fakes and real stdio, short writes, bounded reads, EOF, aliases; no format/type inference or hidden ownership transfer. Tests: `193-std-v3-io-decorators.t`, `246-std-v3-html-events.t`, `248-std-v3-json-value.t`, `249-std-v3-json-parse.t`.

### `io/stdin`

**Represents:** explicit stream capabilities and decorators.

**Roles/objects:** `Stdin`, `Utf8Input`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Reads/writes are capability effects. Borrowed adapters never close origins; owned buffers/states clean up deterministically.

**Native dependencies:** `runtime/stdio`, `runtime/input_state`.

**Testing and non-goals:** ByteBuffer fakes and real stdio, short writes, bounded reads, EOF, aliases; no format/type inference or hidden ownership transfer. Tests: `191-std-v3-io-stdin.t`, `192-std-v3-io-buffer.t`, `212-std-v3-fs-file.t`.

### `io/stdout`

**Represents:** explicit stream capabilities and decorators.

**Roles/objects:** `Stdout`, `Stderr`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Reads/writes are capability effects. Borrowed adapters never close origins; owned buffers/states clean up deterministically.

**Native dependencies:** `runtime/stdio`.

**Testing and non-goals:** ByteBuffer fakes and real stdio, short writes, bounded reads, EOF, aliases; no format/type inference or hidden ownership transfer. Tests: `190-std-v3-io-stdout.t`.

### `io/stream`

**Represents:** explicit stream capabilities and decorators.

**Roles/objects:** `ByteSource`, `Writing`, `Reading`, `WritingText`, `ReadingText`, `Closing`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Reads/writes are capability effects. Borrowed adapters never close origins; owned buffers/states clean up deterministically.

**Native dependencies:** No direct runtime import; composed imports: `error`.

**Testing and non-goals:** ByteBuffer fakes and real stdio, short writes, bounded reads, EOF, aliases; no format/type inference or hidden ownership transfer. Tests: `13-void-stdlib.t`, `16-stdlib-v4.t`, `162-async-block-adapter.t`, `189-std-v3-io-stream.t`, `190-std-v3-io-stdout.t`, `191-std-v3-io-stdin.t`, `192-std-v3-io-buffer.t`, `193-std-v3-io-decorators.t`, `212-std-v3-fs-file.t`, `215-std-v3-fs-temp.t`, `224-std-v3-os-process.t`, `246-std-v3-html-events.t`, `248-std-v3-json-value.t`, `249-std-v3-json-parse.t`.

### `json/parse`

**Represents:** explicit JSON grammar values, parsing and emission.

**Roles/objects:** `ParsedJson`, `JsonNode`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Immutable nodes and bounded lazy parse state; print emits only through an explicit JsonWriting capability. Rendering owns its local buffer.

**Native dependencies:** `runtime/json`.

**Testing and non-goals:** Nested literals, exact decimals, duplicates, invalid/deep input and custom nominal implementations; no object reflection or null-like language value. Tests: `16-stdlib-v4.t`, `249-std-v3-json-parse.t`.

### `json/value`

**Represents:** explicit JSON grammar values, parsing and emission.

**Roles/objects:** `JsonWriting`, `JsonValue`, `JsonMember`, `JsonNull`, `JsonBoolean`, `JsonNumber`, `JsonText`, `JsonArray`, `JsonObject`, `JsonMemberIndex`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Immutable nodes and bounded lazy parse state; print emits only through an explicit JsonWriting capability. Rendering owns its local buffer.

**Native dependencies:** No direct runtime import; composed imports: `collections/sequence`, `math/decimal`.

**Testing and non-goals:** Nested literals, exact decimals, duplicates, invalid/deep input and custom nominal implementations; no object reflection or null-like language value. Tests: `16-stdlib-v4.t`, `248-std-v3-json-value.t`, `249-std-v3-json-parse.t`.

### `math/aggregate`

**Represents:** validated math aggregate representations.

**Roles/objects:** `Minimum`, `Maximum`, `Sum`, `AccurateSum`, `AccurateTotal`, `Product`, `SumOfProducts`, `Hypotenuse`, `Distance`, `ScaledSquares`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** `runtime/math_aggregate`.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `194-std-v3-math-aggregate.t`.

### `math/constants`

**Represents:** validated math constants representations.

**Roles/objects:** `Pi`, `Tau`, `Euler`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** No direct runtime import.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `196-std-v3-math-constants.t`.

### `math/decimal`

**Represents:** validated math decimal representations.

**Roles/objects:** `Decimal`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** `runtime/decimal`.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `16-stdlib-v4.t`, `247-std-v3-math-decimal.t`, `248-std-v3-json-value.t`, `249-std-v3-json-parse.t`.

### `math/rounding`

**Represents:** validated math rounding representations.

**Roles/objects:** .

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** No direct runtime import.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: module semantic audit and dependent capability tests.

### `net/failure`

**Represents:** portable networking vocabulary.

**Roles/objects:** .

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Immutable error observations and an explicit NativeFailure
adapter, without I/O or socket ownership. [Mapping contract](network-failure.md).

**Native dependencies:** No direct runtime import.

**Testing and non-goals:** 31 focused external + 2 inline groups and an executable
example verify categories, scope, exact diagnostics and timeout/cancellation
channels. IP/TCP/TLS transports remain separate work.

### `net/ip`

**Represents:** portable networking vocabulary.

**Roles/objects:** .

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Only the currently implemented enums/errors are approved; socket state lives in runtime/network.

**Native dependencies:** No direct runtime import.

**Testing and non-goals:** Existing native boundary tests; full IP/TCP/TLS behavior is deferred, not implemented by these declarations. Tests: module semantic audit and dependent capability tests.

### `numbers/bigint`

**Represents:** exact integers and immutable arithmetic expressions over private operands.

**Roles/objects:** BigInt does Comparing<BigInt>; BigSum, BigDifference, BigProduct,
BigQuotient, BigRemainder, BigPower, BigGcd, BigLcm. Every expression projects BigInt
through value(). Binary expressions take named left/right operands.

**Behavior:** see the [complete contract](bigint-guide.md) and [API](stdlib-api-v4.md).
Division truncates toward zero; remainder reconstructs the dividend and has its sign.
Gcd/lcm are non-negative. Power admits exponent >= 0 and defines 0^0=1. Default
power result budget is 1048576 bits, with an explicit positive maximum_bits overload.
Configuration errors are checked in constructor validation; zero division and oversized results fail
on value(). Public failures are BigDivisionByZero, InvalidBigExponent,
InvalidBigPowerLimit and BigPowerLimitExceeded.

**Effects and ownership:** pure computation, no I/O, handles, mutable user state or
operand mutation. The power budget bounds result bits, not backend RSS/time.

**Native dependencies:** runtime/bigint; canonical strings at the ABI. Existing
NativeFailure codes invalid_input/resource_exhausted suffice; no ABI-code expansion.

**Testing and non-goals:** 251-std-v4-bigint-arithmetic.t contains independent signed
integer matrices, 80–100-digit fixtures, reconstruction/alias invariants and power
budget boundaries. StorageUsage, Decimal and time contracts remain regression consumers.
No operator overloading, general arithmetic service object or implicit int narrowing.


### `os/account`

**Represents:** process/host observations and execution capabilities.

**Roles/objects:** `User`, `Group`, `GroupUsers`, `GroupUserCursor`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

**Native dependencies:** `runtime/account`.

**Testing and non-goals:** Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation. Tests: `221-std-v3-os-account.t`.

### `os/command`

**Represents:** process/host observations and execution capabilities.

**Roles/objects:** `Commanding`, `Command`, `InheritedCommand`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

**Native dependencies:** No direct runtime import; composed imports: `fs/directory`, `fs/path`, `os/directory`, `os/environment`.

**Testing and non-goals:** Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation. Tests: `06-pipeline-drain.t`, `223-std-v3-os-command.t`, `224-std-v3-os-process.t`, `225-std-v3-os-command-capture.t`, `226-std-v3-os-pipeline-capture.t`.

### `os/command_capture`

**Represents:** process/host observations and execution capabilities.

**Roles/objects:** `CommandCapture`, `CompletedCommand`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

**Native dependencies:** `runtime/process`.

**Testing and non-goals:** Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation. Tests: `225-std-v3-os-command-capture.t`.

### `os/directory`

**Represents:** process/host observations and execution capabilities.

**Roles/objects:** `WorkingDirectory`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

**Native dependencies:** `runtime/fs`.

**Testing and non-goals:** Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation. Tests: `222-std-v3-os-directory.t`.

### `os/environment`

**Represents:** process/host observations and execution capabilities.

**Roles/objects:** `Environment`, `HomeDirectory`, `SearchPath`, `SearchPathCursor`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

**Native dependencies:** `runtime/environment`.

**Testing and non-goals:** Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation. Tests: `16-stdlib-v4.t`, `219-std-v3-os-environment.t`, `223-std-v3-os-command.t`.

### `os/pipeline_capture`

**Represents:** process/host observations and execution capabilities.

**Roles/objects:** `PipelineCapture`, `CompletedPipeline`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

**Native dependencies:** `runtime/process`.

**Testing and non-goals:** Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation. Tests: `06-pipeline-drain.t`, `226-std-v3-os-pipeline-capture.t`.

### `os/platform`

**Represents:** process/host observations and execution capabilities.

**Roles/objects:** `Platform`, `Host`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

**Native dependencies:** `runtime/system`.

**Testing and non-goals:** Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation. Tests: `220-std-v3-os-platform.t`.

### `os/process`

**Represents:** process/host observations and execution capabilities.

**Roles/objects:** `Exit`, `Process`, `ProcessInput`, `ProcessOutput`, `ProcessErrors`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

**Native dependencies:** `runtime/process`.

**Testing and non-goals:** Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation. Tests: `224-std-v3-os-process.t`, `225-std-v3-os-command-capture.t`, `226-std-v3-os-pipeline-capture.t`.

### `os/storage`

**Represents:** process/host observations and execution capabilities.

**Roles/objects:** `Storage`, `StorageUsage`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

**Native dependencies:** `runtime/system`.

**Testing and non-goals:** Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation. Tests: `16-stdlib-v4.t`, `227-std-v3-os-storage.t`.

### `random/distribution`

**Represents:** explicit entropy and distribution capabilities.

**Roles/objects:** `Distribution`, `UniformIntegers`, `UniformFloats`, `NormalDistribution`, `StandardNormal`, `ExponentialDistribution`, `GammaDistribution`, `GammaUnit`, `GammaCandidate`, `BetaDistribution`, `BetaSample`, `LogNormalDistribution`, `VonMisesDistribution`, `VonMisesSample`, `VonMisesCandidate`, `ParetoDistribution`, `WeibullDistribution`, `Choices`, `WeightedChoices`, `WeightedSelection`, `Samples`, `IndexSamples`, `ListRemoval`, `PermutationSamples`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Sampling advances only the encapsulated generator/entropy capability. Seeds and sequences are explicit.

**Native dependencies:** `runtime/random_distribution`.

**Testing and non-goals:** Seed reproducibility, boundaries, malformed distributions and fake entropy; no hidden global random source in value objects. Tests: `198-std-v3-random-distribution.t`, `200-std-v3-uuid-generation.t`.

### `random/source`

**Represents:** explicit entropy and distribution capabilities.

**Roles/objects:** `Randomizing`, `PcgRandom`, `SecureRandom`, `SecureInteger`, `SecureBytes`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Sampling advances only the encapsulated generator/entropy capability. Seeds and sequences are explicit.

**Native dependencies:** `runtime/random`, `runtime/system`.

**Testing and non-goals:** Seed reproducibility, boundaries, malformed distributions and fake entropy; no hidden global random source in value objects. Tests: `197-std-v3-random-source.t`, `198-std-v3-random-distribution.t`, `200-std-v3-uuid-generation.t`.

### `regex/escape`

**Represents:** bounded pattern/match/replacement representations.

**Roles/objects:** `EscapedRegex`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Pure lazy compilation and matching; engine state remains private and failures memoized.

**Native dependencies:** `runtime/regex`.

**Testing and non-goals:** Dialect boundaries, captures, ranges, limits and composed replacement; no native handle API, host regex dialect or executable patterns. Tests: `233-std-v3-regex-escape.t`.

### `regex/match`

**Represents:** bounded pattern/match/replacement representations.

**Roles/objects:** `RegexMatching`, `RegexMatch`, `RegexCapture`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Pure lazy compilation and matching; engine state remains private and failures memoized.

**Native dependencies:** `runtime/regex`.

**Testing and non-goals:** Dialect boundaries, captures, ranges, limits and composed replacement; no native handle API, host regex dialect or executable patterns. Tests: `230-std-v3-regex-match-search.t`, `231-std-v3-regex-replace.t`.

### `regex/pattern`

**Represents:** bounded pattern/match/replacement representations.

**Roles/objects:** `Regex`, `RegexMatchObservation`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Pure lazy compilation and matching; engine state remains private and failures memoized.

**Native dependencies:** `runtime/regex`, `runtime/regex`.

**Testing and non-goals:** Dialect boundaries, captures, ranges, limits and composed replacement; no native handle API, host regex dialect or executable patterns. Tests: `16-stdlib-v4.t`, `228-std-v3-regex-pattern.t`, `230-std-v3-regex-match-search.t`, `231-std-v3-regex-replace.t`, `232-std-v3-regex-split.t`, `233-std-v3-regex-escape.t`.

### `regex/range`

**Represents:** bounded pattern/match/replacement representations.

**Roles/objects:** `RegexRange`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Pure lazy compilation and matching; engine state remains private and failures memoized.

**Native dependencies:** No direct runtime import.

**Testing and non-goals:** Dialect boundaries, captures, ranges, limits and composed replacement; no native handle API, host regex dialect or executable patterns. Tests: `229-std-v3-regex-range.t`, `230-std-v3-regex-match-search.t`.

### `regex/replace`

**Represents:** bounded pattern/match/replacement representations.

**Roles/objects:** `ReplacingRegexMatch`, `LiteralRegexReplacement`, `ExpandedRegexReplacement`, `ReplacementToken`, `ReplacementTokens`, `ReplacementTokenCursor`, `RegexSubstitution`, `RegexSubstitutionProgress`, `RegexSubstitutionResult`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Pure lazy compilation and matching; engine state remains private and failures memoized.

**Native dependencies:** `runtime/regex`.

**Testing and non-goals:** Dialect boundaries, captures, ranges, limits and composed replacement; no native handle API, host regex dialect or executable patterns. Tests: `231-std-v3-regex-replace.t`.

### `regex/search`

**Represents:** bounded pattern/match/replacement representations.

**Roles/objects:** `FirstRegexMatch`, `PrefixRegexMatch`, `EntireRegexMatch`, `RegexMatches`, `RegexMatchCursor`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Pure lazy compilation and matching; engine state remains private and failures memoized.

**Native dependencies:** No direct runtime import; composed imports: `collections/iteration`, `regex/pattern`, `regex/range`, `regex/match`.

**Testing and non-goals:** Dialect boundaries, captures, ranges, limits and composed replacement; no native handle API, host regex dialect or executable patterns. Tests: `230-std-v3-regex-match-search.t`, `233-std-v3-regex-escape.t`.

### `regex/split`

**Represents:** bounded pattern/match/replacement representations.

**Roles/objects:** `RegexParts`, `RegexPartsCursor`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Pure lazy compilation and matching; engine state remains private and failures memoized.

**Native dependencies:** No direct runtime import; composed imports: `collections/iteration`, `regex/match`, `regex/pattern`, `regex/search`.

**Testing and non-goals:** Dialect boundaries, captures, ranges, limits and composed replacement; no native handle API, host regex dialect or executable patterns. Tests: `232-std-v3-regex-split.t`.

### `size`

**Represents:** small nominal size contracts.

**Roles/objects:** `Sizing`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** No I/O or owned resource state. Preserve interpreter-provided nominal behavior.

**Native dependencies:** No direct runtime import.

**Testing and non-goals:** Existing conformance, error and ordering tests; no generic reflection/base-object API. Tests: `166-std-v3-collections-repetition.t`, `167-std-v3-collections-views.t`, `175-std-v3-collections-combinatorics.t`, `192-std-v3-io-buffer.t`, `194-std-v3-math-aggregate.t`, `195-std-v3-statistics.t`, `198-std-v3-random-distribution.t`, `29-builtin-value-behavior.t`.

### `statistics`

**Represents:** validated statistics representations.

**Roles/objects:** `FloatSample`, `Mean`, `GeometricMean`, `HarmonicMean`, `Median`, `MedianLow`, `MedianHigh`, `Quantiles`, `Mode`, `Modes`, `ModesCursor`, `SampleVariance`, `PopulationVariance`, `SampleDeviation`, `PopulationDeviation`, `Covariance`, `Correlation`, `FittedLine`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** `runtime/math_aggregate`, `runtime/statistics`.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `16-stdlib-v4.t`, `195-std-v3-statistics.t`.

### `strings/format`

**Represents:** validated strings format representations.

**Roles/objects:** `Formatted`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** No direct runtime import.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `16-stdlib-v4.t`.

### `strings/parse`

**Represents:** validated strings parse representations.

**Roles/objects:** `ParsedInt`, `ParsedFloat`, `ParsedRat`, `ParsedBool`, `ParsedByte`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** No direct runtime import.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `177-std-v3-strings-parse.t`.

### `text/layout`

**Represents:** validated text layout representations.

**Roles/objects:** `Indented`, `Dedented`, `Wrapped`, `Shortened`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** `runtime/layout`.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `176-std-v3-text-layout.t`.

### `time/clock`

**Represents:** validated time clock representations.

**Roles/objects:** `Instant`, `MonotonicInstant`, `Timing`, `SystemClock`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** `runtime/time`.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `202-std-v3-time-clock.t`, `206-std-v3-time-zone.t`, `208-std-v3-time-rfc3339.t`, `218-std-v3-fs-watch.t`, `240-std-v3-async-timeout.t`.

### `time/date`

**Represents:** validated time date representations.

**Roles/objects:** `Date`, `ShiftedDate`, `DaysBetween`, `DateOrdinal`, `OrdinalDate`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** No direct runtime import; composed imports: `compare`.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `203-std-v3-time-date.t`, `205-std-v3-time-local-datetime.t`, `206-std-v3-time-zone.t`, `207-std-v3-time-iso.t`, `209-std-v3-time-format.t`, `210-std-v3-calendar.t`.

### `time/day`

**Represents:** validated time day representations.

**Roles/objects:** `TimeOfDay`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** No direct runtime import; composed imports: `compare`.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `204-std-v3-time-day.t`, `205-std-v3-time-local-datetime.t`, `206-std-v3-time-zone.t`, `207-std-v3-time-iso.t`, `209-std-v3-time-format.t`.

### `time/duration`

**Represents:** validated time duration representations.

**Roles/objects:** `Duration`, `ScaledDuration`, `DurationString`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** No direct runtime import; composed imports: `compare`, `numbers/bigint`.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `201-std-v3-time-duration.t`, `202-std-v3-time-clock.t`, `205-std-v3-time-local-datetime.t`, `206-std-v3-time-zone.t`, `208-std-v3-time-rfc3339.t`, `218-std-v3-fs-watch.t`, `240-std-v3-async-timeout.t`.

### `time/format`

**Represents:** validated time format representations.

**Roles/objects:** `FormattedDate`, `DateFromFormat`, `FormattedLocalDateTime`, `LocalDateTimeFromFormat`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** `runtime/time_format`.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `209-std-v3-time-format.t`.

### `time/iso`

**Represents:** validated time iso representations.

**Roles/objects:** `IsoDate`, `DateFromIso`, `IsoLocalDateTime`, `LocalDateTimeFromIso`, `IsoFraction`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** No direct runtime import; composed imports: `time/date`, `time/day`, `time/local_datetime`, `strings/parse`.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `207-std-v3-time-iso.t`.

### `time/local_datetime`

**Represents:** validated time local_datetime representations.

**Roles/objects:** `LocalDateTime`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** No direct runtime import; composed imports: `compare`, `time/duration`, `time/date`, `time/day`.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `205-std-v3-time-local-datetime.t`, `206-std-v3-time-zone.t`, `207-std-v3-time-iso.t`, `209-std-v3-time-format.t`.

### `time/rfc3339`

**Represents:** validated time rfc3339 representations.

**Roles/objects:** `Rfc3339`, `InstantFromRfc3339`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** No direct runtime import; composed imports: `time/duration`, `time/clock`, `time/date`, `time/day`, `time/local_datetime`, `time/iso`, `strings/parse`.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `208-std-v3-time-rfc3339.t`.

### `time/zone`

**Represents:** validated time zone representations.

**Roles/objects:** `TimeZone`, `Zone`, `FixedZone`, `ZonedDateTime`, `ZoneObservation`, `InstantFromLocal`, `TimeZoneDatabaseVersion`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** `runtime/time_zone`.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `206-std-v3-time-zone.t`.

### `unicode/character`

**Represents:** validated unicode character representations.

**Roles/objects:** `UnicodeCharacter`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** `runtime/unicode_character`.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `178-std-v3-unicode-character.t`.

### `unicode/normalization`

**Represents:** validated unicode normalization representations.

**Roles/objects:** `Normalized`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

**Native dependencies:** `runtime/unicode_normalization`.

**Testing and non-goals:** Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O. Tests: `179-std-v3-unicode-normalization.t`.

### `uuid/generation`

**Represents:** validated identifiers and explicit generation.

**Roles/objects:** `Uuid3`, `Uuid5`, `Uuid8`, `Uuid4s`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value encodings/comparison are pure; random UUID generation requires its entropy capability.

**Native dependencies:** `runtime/uuid`.

**Testing and non-goals:** Canonical forms, version/variant and injected randomness; no random constructor disguised as a stable value query. Tests: `200-std-v3-uuid-generation.t`.

### `uuid/value`

**Represents:** validated identifiers and explicit generation.

**Roles/objects:** `Uuid`, `UuidFromBytes`.

**Behavior:** public signatures are listed in the [current API](stdlib-api-v4.md); design decisions are recorded in the matching audit section and the required redesigns above.

**Effects and ownership:** Value encodings/comparison are pure; random UUID generation requires its entropy capability.

**Native dependencies:** `runtime/uuid`.

**Testing and non-goals:** Canonical forms, version/variant and injected randomness; no random constructor disguised as a stable value query. Tests: `199-std-v3-uuid-value.t`, `200-std-v3-uuid-generation.t`.

### `io/text`

Represents an explicit UTF-8 text output over borrowed Writing. `Utf8Output(origin is Writing)`
implements WritingText (`write(...string)`, `writeln(...string)`, both void).
No implicit flush/close; Utf8Encoded + ByteSnapshot compose the byte boundary.
Test with ByteBuffer and failing/short-writing capabilities; no arbitrary formatting.

### `json/write`

`CompactJson(origin is JsonValue).string()` represents a compact JSON document.
`JsonTextOutput(destination is WritingText)` implements the six JsonWriting commands.
Ownership is local for CompactJson's buffer, borrowed for JsonTextOutput's destination.
Native dependencies are indirect through buffer/UTF-8; no reflection or node-state
exposure. Exact decimal representation, all scalar controls, Unicode, insertion order,
empty/nested documents and nominal implementations are contract tests.
PrettyJson now adds structural LF/indentation through the same nominal dispatch and
scalar printer. Default depth is 128 and maximum UTF-8 output is 1048576 bytes;
an explicit constructor supplies both limits. Configuration and exceeded budgets
have distinct typed errors. See [printing contract](json-printing-guide.md).
CompactJson remains unbounded. The new printer does not sandbox user print() code.

### `csv` - bounded printing and parsing with configurable grammar

CsvString represents a finite snapshot of text rows. CsvRecord and CsvField represent
one terminated record and one escaped field. Default overloads use comma/double quote/CRLF;
no header inference, implicit byte conversion, native dependency or resource I/O.
Empty records with zero fields fail on string(); a single empty field is valid.
Quoted CR and LF are preserved independently, not just a matching CRLF pair.
`CsvRows(origin is string) does Iterating<@string>` validates the entire source
on each cursor() observation and returns an independent immutable cursor.
InvalidCsv reports a zero-based Unicode-scalar offset; malformed quotes, bare
line endings and unfinished quoted fields are rejected. A final CRLF is optional;
a leading BOM is data. Construction only retains text. No header inference.
[Contract and tests](csv-guide.md). Reads now have default and explicit input/field/row/column budgets, checked
before retaining excess parsed data; scalar offsets avoid growing field copies.
Print snapshots now enforce field/row/column budgets during traversal and
encoded output budgets before materializing each escaped record.
CsvDialect adds configurable single-scalar delimiter/quote and strict LF/CRLF endings,
with behavioral encoding/decoding and bounded record printing. CsvRows/CsvString
accept the dialect with default or explicit budgets; existing overloads keep their format.
CsvInput in `csv/input` adds explicit bounded UTF-8 document reading from a
borrowed Reading. It returns fully validated arrays and never hides I/O in a
cursor. Incremental row-reading and ownership adapters remain in the TODO.

### `toml/value` — immutable values and temporal coordinates

TomlValue.accept dispatches to TomlVisitor without JSON conversion or casts.
Scalar variants retain exact runtime string/int/float/bool values; arrays and
ordered tables snapshot bounded nominal sequences. Tables reject duplicate
literal names and provide typed missing-member lookup. In-module tests accompany
the implementation. Temporal variants retain year zero, nanoseconds, leap-second
notation and known/unknown offsets. Explicit TomlInfinity and TomlNaN values
preserve TOML special categories while builtin float remains finite; TomlFloat
preserves binary64 signed zero and subnormals. The bounded TOML 1.0 parser is implemented in toml/parse;
see [the contract and pending gates](toml-values-guide.md).

## V3 scope reconciliation and deliberate deferrals

All 320 v3 module headings are accounted for below. Deferred is not implemented
and is not API approval. In particular DTO-shaped HTTP/database/telemetry
records, reflection-based serialization, global managers, and result-returning
copy/extract/mode-change commands must not be copied from v3. A future slice
must specify a represented entity and explicit protocol/command first.

Networking/TLS has only a minimal native socket substrate, not complete std/net
capabilities; crypto, archives, databases and HTTP need provider/ownership
work and are not rushed into this refactor. Existing legacy `stdlib/` modules
are not silently counted as new-library implementations.

| V3 scope | V4 disposition |
| --- | --- |
| `core/error` | Reviewed existing module; audit + v4 corrections are authoritative |
| `builtin/conversion` | Language-owned; no stdlib/compiler rewrite authorized by this roadmap |
| `builtin/array` | Language-owned; no stdlib/compiler rewrite authorized by this roadmap |
| `builtin/map` | Language-owned; no stdlib/compiler rewrite authorized by this roadmap |
| `core/compare` | Reviewed existing module; audit + v4 corrections are authoritative |
| `core/size` | Reviewed existing module; audit + v4 corrections are authoritative |
| `collections/iteration` | Reviewed existing module; audit + v4 corrections are authoritative |
| `collections/sequence` | Reviewed existing module; audit + v4 corrections are authoritative |
| `builtin/range` | Language-owned; no stdlib/compiler rewrite authorized by this roadmap |
| `collections/enumeration` | Reviewed existing module; audit + v4 corrections are authoritative |
| `collections/combining` | Reviewed existing module; audit + v4 corrections are authoritative |
| `collections/repetition` | Reviewed existing module; audit + v4 corrections are authoritative |
| `collections/views` | Reviewed existing module; audit + v4 corrections are authoritative |
| `collections/list` | Reviewed existing module; audit + v4 corrections are authoritative |
| `collections/set` | Reviewed existing module; audit + v4 corrections are authoritative |
| `collections/deque` | Reviewed existing module; audit + v4 corrections are authoritative |
| `collections/heap` | Reviewed existing module; audit + v4 corrections are authoritative |
| `collections/frequencies` | Reviewed existing module; audit + v4 corrections are authoritative |
| `collections/ordering` | Reviewed existing module; audit + v4 corrections are authoritative |
| `collections/combinatorics` | Reviewed existing module; audit + v4 corrections are authoritative |
| `builtin/string` | Language-owned; no stdlib/compiler rewrite authorized by this roadmap |
| `strings/join` | Joined: bounded snapshot of nominal string iteration, exact literal separators, repeatable projection; [contract](strings-join-guide.md) |
| `text/layout` | Reviewed existing module; audit + v4 corrections are authoritative |
| `strings/format` | Reviewed existing module; audit + v4 corrections are authoritative |
| `strings/parse` | Reviewed existing module; audit + v4 corrections are authoritative |
| `diff` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `unicode/character` | Reviewed existing module; audit + v4 corrections are authoritative |
| `unicode/normalization` | Reviewed existing module; audit + v4 corrections are authoritative |
| `encoding/utf` | Reviewed existing module; audit + v4 corrections are authoritative |
| `encoding/single_byte` | Reviewed existing module; audit + v4 corrections are authoritative |
| `encoding/hex` | Reviewed existing module; audit + v4 corrections are authoritative |
| `encoding/base32` | Reviewed existing module; audit + v4 corrections are authoritative |
| `encoding/base64` | Reviewed existing module; audit + v4 corrections are authoritative |
| `encoding/base85` | Reviewed existing module; audit + v4 corrections are authoritative |
| `encoding/quoted_printable` | Reviewed existing module; audit + v4 corrections are authoritative |
| `encoding/pem` | Reviewed existing module; audit + v4 corrections are authoritative |
| `encoding/binary` | Reviewed existing module; audit + v4 corrections are authoritative |
| `io/stream` | Reviewed existing module; audit + v4 corrections are authoritative |
| `io/stdout` | Reviewed existing module; audit + v4 corrections are authoritative |
| `io/stdin` | Reviewed existing module; audit + v4 corrections are authoritative |
| `io/text` | Approved v4 implementation slice above; other proposed extras deferred |
| `io/buffer` | Reviewed existing module; audit + v4 corrections are authoritative |
| `io/decorators` | Reviewed existing module; audit + v4 corrections are authoritative |
| `builtin/integer-math` | Language-owned; no stdlib/compiler rewrite authorized by this roadmap |
| `builtin/float-math` | Language-owned; no stdlib/compiler rewrite authorized by this roadmap |
| `builtin/rat-math` | Language-owned; no stdlib/compiler rewrite authorized by this roadmap |
| `math/rounding` | Reviewed existing module; audit + v4 corrections are authoritative |
| `math/aggregate` | Reviewed existing module; audit + v4 corrections are authoritative |
| `math/decimal` | Reviewed existing module; audit + v4 corrections are authoritative |
| `numbers/bigint` | Reviewed existing module; audit + v4 corrections are authoritative |
| `numbers/bigrational` | Canonical exact ratios, comparison and four arithmetic representations over existing BigInt; [contract](bigrational-guide.md) |
| `math/complex` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `statistics` | Reviewed existing module; audit + v4 corrections are authoritative |
| `math/constants` | Reviewed existing module; audit + v4 corrections are authoritative |
| `random/source` | Reviewed existing module; audit + v4 corrections are authoritative |
| `random/distribution` | Reviewed existing module; audit + v4 corrections are authoritative |
| `uuid/value` | Reviewed existing module; audit + v4 corrections are authoritative |
| `uuid/generation` | Reviewed existing module; audit + v4 corrections are authoritative |
| `time/duration` | Reviewed existing module; audit + v4 corrections are authoritative |
| `time/clock` | Reviewed existing module; audit + v4 corrections are authoritative |
| `time/date` | Reviewed existing module; audit + v4 corrections are authoritative |
| `time/day` | Reviewed existing module; audit + v4 corrections are authoritative |
| `time/local-datetime` | Reviewed existing module; audit + v4 corrections are authoritative |
| `time/zone` | Reviewed existing module; audit + v4 corrections are authoritative |
| `time/iso` | Reviewed existing module; audit + v4 corrections are authoritative |
| `time/rfc3339` | Reviewed existing module; audit + v4 corrections are authoritative |
| `time/format` | Reviewed existing module; audit + v4 corrections are authoritative |
| `calendar` | Reviewed existing module; audit + v4 corrections are authoritative |
| `fs/path` | Reviewed existing module; audit + v4 corrections are authoritative |
| `fs/file` | Reviewed existing module; audit + v4 corrections are authoritative |
| `fs/lock` | Reviewed existing module; audit + v4 corrections are authoritative |
| `fs/stat` | Reviewed existing module; audit + v4 corrections are authoritative |
| `fs/copy` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `fs/directory` | Reviewed existing module; audit + v4 corrections are authoritative |
| `fs/glob` | Reviewed existing module; audit + v4 corrections are authoritative |
| `fs/link` | Reviewed existing module; audit + v4 corrections are authoritative |
| `fs/temp` | Reviewed existing module; audit + v4 corrections are authoritative |
| `fs/mmap` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `fs/watch` | Reviewed existing module; audit + v4 corrections are authoritative |
| `os/environment` | Reviewed existing module; audit + v4 corrections are authoritative |
| `os/platform` | Reviewed existing module; audit + v4 corrections are authoritative |
| `os/account` | Reviewed existing module; audit + v4 corrections are authoritative |
| `os/directory` | Reviewed existing module; audit + v4 corrections are authoritative |
| `os/command` | Reviewed existing module; audit + v4 corrections are authoritative |
| `os/process` | Reviewed existing module; audit + v4 corrections are authoritative |
| `os/command-capture` | Reviewed existing module; audit + v4 corrections are authoritative |
| `os/pipeline-capture` | Reviewed existing module; audit + v4 corrections are authoritative |
| `os/storage` | Reviewed existing module; audit + v4 corrections are authoritative |
| `json/value` | Reviewed existing module; audit + v4 corrections are authoritative |
| `json/parse` | Reviewed existing module; audit + v4 corrections are authoritative |
| `json/write` | Approved v4 implementation slice above; other proposed extras deferred |
| `json/read` | [Verified typed views](json-reading.md): JsonView with RFC 6901 pointers, exact integers and typed failures; JSON writer spells numbers positionally (660 and consumers PASS) |
| `csv` | Bounded configurable printing/parsing and explicit document input; incremental row reading remains deferred |
| `toml/value` | Value model, temporal coordinates and bounded TOML 1.0 parser implemented; no document writer |
| `toml/parse` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `config/ini` | Bounded model, lazy parser and canonical renderer implemented; [dialect/limits](ini-guide.md) |
| `xml/name` | Bounded immutable XML 1.0 serialization and namespace-aware values verified; parsing remains separate. [Contract](xml-values.md) |
| `xml/node` | Bounded immutable XML 1.0 serialization and namespace-aware values verified; parsing remains separate. [Contract](xml-values.md) |
| `xml/parse` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `xml/write` | Bounded immutable XML 1.0 serialization and namespace-aware values verified; parsing remains separate. [Contract](xml-values.md) |
| `html/entities` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `plist/value` | Implemented pure Okkama nominal values and print protocol; bounded array/dictionary snapshots, ordered entries, typed errors. [Contract/evidence](plist-value-guide.md). Parsing and writing implemented below |
| `plist/parse` | Pure Okkama strict UTF-8 XML/bplist00 parsing, bounded expansion, byte diagnostics, binary spans/references and cycle rejection. [Contract/evidence](plist-parse-guide.md) |
| `plist/write` | Pure Okkama bounded XML/bplist00 encoding through nominal printing; stable member order, UTC dates, typed unsupported-value errors. [Contract/evidence](plist-write-guide.md) |
| `regex/pattern` | Reviewed existing module; audit + v4 corrections are authoritative |
| `regex/range` | Reviewed existing module; audit + v4 corrections are authoritative |
| `regex/match` | Reviewed existing module; audit + v4 corrections are authoritative |
| `regex/search` | Reviewed existing module; error-contract audit, 9 focused groups PASS; v4 corrections are authoritative |
| `regex/replace` | Reviewed existing module; audit + v4 corrections are authoritative |
| `regex/split` | Reviewed existing module; audit + v4 corrections are authoritative |
| `regex/escape` | Reviewed existing module; audit + v4 corrections are authoritative |
| `html/events` | Reviewed existing module; audit + v4 corrections are authoritative |
| `checksum/state` | [Immutable successor contract](checksum-guide.md), nominal consumer and fake; 3 consumer + 3 inline groups PASS |
| `checksum/adler32` | [Pure Okkama Adler-32](checksum-guide.md#checksumadler32-contract); zlib fixtures, 8 consumer + 3 inline groups PASS |
| `checksum/crc` | [Pure Okkama CRC-32/ISO-HDLC and explicit CRC-16](checksum-guide.md#checksumcrc-contract); 11 consumer + 5 inline groups PASS |
| `digest/state` | [Immutable raw-byte successor contract](digest-guide.md); 3 consumer + 2 inline groups PASS; algorithm behavior verified separately |
| `digest/blocks` | [Internal immutable block/padding infrastructure](digest-guide.md#digestblocks-internal-contract); 6 consumer + 3 inline groups PASS |
| `digest/legacy` | [Pure Okkama MD5/SHA-1](digest-guide.md#digestlegacy-contract), legacy interoperability only; 26 algorithm + 3 inline groups PASS |
| `digest/sha2` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `digest/sha3` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `digest/shake` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `digest/blake2` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `digest/read` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `crypto/constant-time` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `crypto/hmac` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `compression/state` | [Streaming contracts, finite output owner and cumulative budget](compression-guide.md); 13 consumer + 5 inline groups PASS; wire codecs remain pending |
| `compression/read` | [Bounded consuming adapters over borrowed codecs and sources](compression-guide.md#compressionread); 13 consumer + 3 inline groups PASS; wire codecs remain pending |
| `compression/gzip` | Deferred by user decision 2026-09-25 pending host codec libraries; [queue policy](stdlib-completion-todo-v4.md#алгоритмы-сжатия-отложены--решение-2026-09-25); not implemented |
| `compression/zlib` | Deferred by user decision 2026-09-25 pending host codec libraries; [queue policy](stdlib-completion-todo-v4.md#алгоритмы-сжатия-отложены--решение-2026-09-25); not implemented |
| `compression/deflate` | Deferred by user decision 2026-09-25 pending host codec libraries; [queue policy](stdlib-completion-todo-v4.md#алгоритмы-сжатия-отложены--решение-2026-09-25); not implemented |
| `compression/bzip2` | Deferred by user decision 2026-09-25 pending host codec libraries; [queue policy](stdlib-completion-todo-v4.md#алгоритмы-сжатия-отложены--решение-2026-09-25); not implemented |
| `compression/xz` | Deferred by user decision 2026-09-25 pending host codec libraries; [queue policy](stdlib-completion-todo-v4.md#алгоритмы-сжатия-отложены--решение-2026-09-25); not implemented |
| `compression/lzma` | Deferred by user decision 2026-09-25 pending host codec libraries; [queue policy](stdlib-completion-todo-v4.md#алгоритмы-сжатия-отложены--решение-2026-09-25); not implemented |
| `compression/zstd` | Deferred by user decision 2026-09-25 pending host codec libraries; [queue policy](stdlib-completion-todo-v4.md#алгоритмы-сжатия-отложены--решение-2026-09-25); not implemented |
| `archive/path` | [Exact immutable relative member names](archive-guide.md#archivepath); 10 consumer + 3 inline groups PASS; extraction containment remains separate |
| `archive/metadata` | [Immutable format metadata and explicit epoch conversion](archive-guide.md#archivemetadata); 11 consumer + 3 inline groups PASS, three process TZ settings; wire parsers remain pending |
| `archive/entry` | [Immutable member variants with nominal visitor dispatch](archive-guide.md#archiveentry); 10 consumer + 3 inline groups PASS; no extraction effects |
| `archive/limits` | [Positive policy and immutable cumulative actual-byte accounting](archive-guide.md#archivelimits); 14 consumer + 4 inline groups PASS; real format integration remains pending |
| `archive/file` | DESIGN, blocked by missing no-follow open and opened-descriptor metadata; capability audit; checklist remains open |
| `archive/stream` | [Resource contracts and shared lifecycle adapters](archive-guide.md#archivestream); 13 consumer + 3 inline groups PASS; bounded format materialization verified for TAR, pending for ZIP |
| `archive/tar` | [PAX/USTAR values and streams](archive-tar.md): TarArchive/TarWriting/ParsedTar/TarReading, 12 writing + 37 value-reading + 25 streaming + 8 inline groups; independent PAX/GNU fixtures, checksums, malformed inputs, admission before I/O/allocation and borrowed lifecycle |
| `archive/zip` | [ZipMethod/ZipCompression/ZipArchiveEntry](archive-zip.md): validated immutable writing choices and effect-free member binding; 16 external + 2 inline groups PASS. Remaining wire APIs are DEFERRED by user choice; compressed methods also depend on deferred host codecs |
| `archive/zip` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `archive/extract` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `crypto/secret` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `crypto/kdf` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `crypto/password` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `crypto/nonce` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `crypto/aead` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `crypto/aes-gcm` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `crypto/chacha20-poly1305` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `crypto/key` | PublicKey SPKI role for [certificate parsing](x509-certificate.md); VerifyingKey/SigningKey and [key formats](crypto-guide.md) implemented over the host provider — 2026-10-23 |
| `crypto/ed25519` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `crypto/x25519` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `crypto/ecdsa` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `crypto/rsa` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `crypto/key-format` | [Implemented over the host provider](crypto-guide.md) (okkama-native, RustCrypto); RFC/NIST vectors + OpenSSL interop, std inline PASS on JVM (macOS, Linux) — 2026-10-23 |
| `crypto/x509/name` | [Grouped metadata snapshots](x509-name.md) and [original DER integration](x509-certificate.md) implemented; PKIX name matching is separate |
| `crypto/x509/certificate` | [DER/RDN/validity/SPKI](x509-certificate.md), [SAN/supplied chain](certificate-observers.md) verified; [BC/KU/EKU/unknown extension metadata](certificate-extensions.md) focused/inline/example PASS; full gate 627 files / 11,753 assertions PASS |
| `crypto/x509/trust` | [Explicit roots](certificate-observers.md)/[system acquisition](system-trust-store.md) verified; [explicit TLS-purpose path verification](certificate-verification.md) final 97 focused assertions plus inline/example PASS, full 631/11826 PASS; [owned TLS client](tls-client-transport.md) focused/inline/HTTPS example PASS, full 647/11950 PASS |
| `builtin/async` | Language-owned; no stdlib/compiler rewrite authorized by this roadmap |
| `async/cancellation` | Reviewed existing module; audit + v4 corrections are authoritative |
| `async/channel` | Reviewed existing module; audit + v4 corrections are authoritative |
| `async/scope` | Reviewed existing module; audit + v4 corrections are authoritative |
| `async/collect` | Reviewed existing module; audit + v4 corrections are authoritative |
| `async/timeout` | Reviewed existing module; audit + v4 corrections are authoritative |
| `async/lock` | Reviewed existing module; audit + v4 corrections are authoritative |
| `async/semaphore` | Reviewed existing module; audit + v4 corrections are authoritative |
| `async/event` | Reviewed existing module; audit + v4 corrections are authoritative |
| `async/barrier` | Reviewed existing module; audit + v4 corrections are authoritative |
| `parallel/map` | [Bounded ordered process mapping](parallel-map.md), 15 external + 1 inline groups PASS and actual CLI example; typed bound workers, checked transfer, failure/cancellation cleanup. Evidence |
| `runtime/tls_transport` | Private managed-TCP transfer/TlsState ABI for [owned verified client](tls-client-transport.md); no public engine handles |
| `runtime/network` | Reviewed existing module; audit + v4 corrections are authoritative |
| `net/failure` | [Portable error observations and explicit native adapter](network-failure.md); 31 external + 2 inline groups PASS, including separate timeout/cancellation channels and inapplicable transport rethrow |
| `net/ip` | [IpAddress numeric value](ip-address.md) and [pinned IANA properties](ip-properties.md): 36 value groups + 76 property assertions and 5 inline groups PASS. [IpNetwork](ip-network.md) adds exact CIDR endpoints/membership/BigInt cardinality, 18 groups PASS. Lazy iteration and interface remain DESIGN |
| `net/endpoint` | [Verified numeric and IDNA endpoints](host-policy.md): canonical HostName, portable bounds and no DNS/socket effects |
| `net/public_suffix` | [Verified pinned PSL](host-policy.md): ICANN/PRIVATE exact/wildcard/exception policy, indexed domain boundaries, no live update |
| `net/dns` | Fixed typed configuration and async system worker resolution verified; [fixed](fixed-resolution.md), [system](system-resolution.md); recursive DNS deferred |
| `net/interface` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `net/stream` | Async roles, endpoint pairs and owned IP stream halves verified; [ownership](tcp-stream.md) |
| `net/tcp_racing` | [Bounded supplied-address racing](tcp-racing.md): relative stagger/early failure, stable families, pending/ready loser cleanup; 6 files/36 assertions plus inline/CLI PASS |
| `net/tcp` | Numeric connector/listener/acceptance verified; hostname racing remains open; [contract](tcp-stream.md) |
| `net/udp` | Datagram/options/binding, [bounded receive](udp-receive.md), [atomic send](udp-send.md) and [connected UDP](udp-connection.md) implemented; packet boundaries, fixed-peer filtering, typed errors, cancellation/close and IPv4/IPv6/dual-stack loopback tests |
| `net/multicast` | [Options](multicast-options.md) and [any-source memberships](multicast-membership.md) implemented: IPv4/IPv6, typed group validation, pinned socket policy, atomic acquisition/rollback and deterministic leave/parent closure. Source-specific multicast is excluded |
| `net/unix` | [Address](unix-address.md) and [stream](unix-streams.md) contracts implemented: nonblocking pair/connect/listen/accept, named/unnamed endpoints, retained async halves, cancellation and guarded cleanup. Parent directory must be protected during listener acquisition/close |
| `net/tls` | [Policy](tls-client-policy.md) and [owned verified client](tls-client-transport.md): focused compatibility 21/187, inline and real HTTPS CLI PASS, full 647/11950 PASS; server/client identity waits for PrivateKey |
| `runtime/message` | Deferred substrate; require a consuming tested object capability, not speculative primitives |
| `url/percent` | [Contract](url-percent.md) implemented in ordinary std: explicit component choices, uppercase UTF-8 escapes, one-pass binary decoding and exact malformed-source offsets |
| `url/value` | [Verified bounded URL values](url-value.md): typed authority, IDNA/IP/IPvFuture host projections, IRI canonical ASCII and RFC 3986 reference resolution; profiled dot bulk path |
| `url/query` | [Implemented](url-parameters.md): bounded snapshot and parser, duplicates/order/assignment, one-pass UTF-8 decoding and encoded input offsets |
| `url/form` | [Implemented](url-parameters.md): UTF-8 form codec, explicit plus semantics, original-byte errors, input/cardinality bounds |
| `url/file` | BLOCKED on full Url/authority API; no filesystem conversion placeholder |
| `message/header` | [Implemented](message-headers.md): ASCII field values, case-insensitive lookup, bounded immutable headers, CRLF rendering with safe whitespace folding and hard line limit |
| `mime/media-type` | [Implemented as `mime/media_type`](mime-media-type.md): bounded media types and RFC 2231 parameters, strict ASCII field-body parser |
| `url/data` | BLOCKED on full Url/authority API; MediaType dependency is available |
| `mime/disposition` | [Implemented](mime-disposition.md): shared MIME parameter grammar, extension kinds and decoded filename data |
| `mime/transfer` | [Implemented](mime-transfer.md): deferred byte-source adapters, 7bit/8bit/binary profiles, bounded MIME Base64 and quoted-printable |
| `mime/word` | [Implemented](mime-word.md): UTF-8/US-ASCII B output and B/Q input, scalar-safe splitting, unstructured/forbidden contexts; structured consumers remain open |
| `mime/part` | [Implemented](mime-part.md): decoded bodies, bounded MIME trees, strict recursive multipart parsing, explicit charsets and collision-checked rendering; encapsulated messages stay opaque |
| `mime/types` | [Implemented](mime-types.md): lexical extension maps, longest compound suffix, caller overrides, bounded optional system-file snapshots |
| `mail/address` | BLOCKED on HostName/IDNA; raw string domains are not substituted |
| `mail/auth` | BLOCKED on deferred SecretBytes ownership/redaction and verified TLS |
| `mail/message` | [Implemented content profile](mail-message.md): MIME-backed content, explicit delivery view, modern IDs and bounded mail dates; envelope/generation BLOCKED on Mailbox/HostName |
| `mail/mbox_store` | [Local mboxrd sessions](mbox-session.md) implemented: permanent exclusive lock, bounded snapshots, session-local IDs and atomic successor mutations |
| `mail/mbox` | [Bounded mboxrd codec](mbox.md) implemented: explicit envelope metadata, reversible quoting, strict framing and limits; local sessions use mail/mbox_store |
| `mail/mailbox` | [Flags](mail-flags.md), [stored snapshots](stored-mail.md) and [Maildir read/add/remove/flags](maildir.md) implemented with cooperative locking; local mbox sessions are in mail/mbox_store |
| `mail/smtp_reply` | [Bounded ASCII reply values and incremental framing](smtp-wire.md) implemented |
| `mail/smtp_reading` | [Lazy one-shot asynchronous reply reading](smtp-reading.md), explicit remainder and Within/Before composition; stream/session ownership stays with caller |
| `mail/smtp` | [EHLO capabilities](smtp-wire.md) implemented; connection/session/authentication/delivery remain DESIGN |
| `mail/smtp_data` | [Bounded DATA preparation](smtp-wire.md) implemented: Bcc stripping, SIZE/8BITMIME, dot transparency; no network effects |
| `mail/imap_name` | [Bounded modified UTF-7 and quoted IMAP4rev1 mailbox names](imap-names.md) implemented; explicit canonical profile, no normalization or network effects |
| `mail/imap` | [Bounded CAPABILITY snapshots and exact untagged line parsing](imap-capabilities.md) implemented; secure connection/UTF-8 remain DESIGN; authenticated rev1 commands/session/search are implemented in [protocol modules](imap-protocol.md) |
| `mail/imap_response` | [Bounded ASCII response framing](imap-responses.md) implemented: known tagged/untagged/continuation forms, literal/literal8, exact remainder; complete extension semantics remain DESIGN; bounded command collection is implemented in [protocol modules](imap-protocol.md) |
| `mail/imap_reading` | [One-shot asynchronous response reading](imap-reading.md) implemented: borrowed AsyncReading, cached Future, bounded requests, explicit remainder and cancellation cleanup; authenticated session/command collection is implemented in [protocol modules](imap-protocol.md) |
| `mail/imap_*` | [Authenticated rev1 protocol layer](imap-protocol.md): typed UID actions, selection state, mutations and IDLE; secure connection and UTF-8 remain blocked/open |
| `mail/pop3` | [Reply framing/reading](pop3-responses.md) and [owned transaction session](pop3-session.md): typed CAPA/STAT/LIST/UIDL, RETR/TOP, DELE/RSET/QUIT; connector and verified authentication remain BLOCKED |
| `runtime/http` | [Contract gate](http-runtime-gate.md): reuse async streams; TCP/TLS/URL foundations BLOCKED, native acceleration deferred pending measured consumer bottleneck |
| `http/method` | [Verified pure method values](http-method.md): exact case-sensitive HTTP tokens, conservative standard semantics, typed validation |
| `http/status` | [Verified pure statuses](http-status.md): exact 100..599, numeric class, optional display phrases and typed rejection |
| `http/version` | Verified identities/exact wire tokens; [HTTP1 TLS selection](http1-tls-session.md) accepts exact/absent ALPN and rejects others before I/O; HTTP2/prior-knowledge open |
| `http/connection` | [Checked-route HTTP1 opening](http1-connecting.md): real HTTP/HTTPS/DNS-IP identity/SNI/cleanup, 453/454 15 assertions + inline/CLI PASS; bounded TcpRacing resolves checked-route PERF-18 |
| `http/endpoint` | [Checked HTTP endpoint selection](http-endpoints.md): 12 focused assertions, inline/example PASS; numeric bypass, exact port, immutable allowlists and cancellation |
| `http/tls_session` | [Verified HTTP1 TLS owner](http1-tls-session.md): 450/452 12 assertions, inline and actual CLI PASS; no runtime change |
| `http/header` | [Verified bounded values](http-header.md): canonical name matching, ordered duplicates, persistent snapshots and logical HTTP/2 validation; HPACK/wire remain open |
| `http/body` | [Verified byte/owned-stream bodies](http-body.md): replay, one-shot ownership, bounded reads, cancellation and trailers; FileBody/wire lifecycle BLOCKED |
| `http/content-coding` | [Verified canonical names](http-content-coding.md); streaming body transformations BLOCKED by deferred Stage 16 codecs |
| `http/message` | [Verified targets and Request/Response metadata](http-message.md), with role snapshots and unobserved BodyOpening retention; exchange/TLS/connection metadata follows |
| `http/date` | [Verified HTTP dates](http-date.md): canonical whole-second GMT, three grammars and explicit RFC850 reference window |
| `http/conditional` | [Verified conditional values](http-conditional.md): validators, bounded recipient lists and exact/coalesced byte-range plans; protocol preconditions/multipart remain blocked |
| `http/negotiation` | [Verified quality and four explicit preference profiles](http-negotiation.md): bounded grammar, exact weights, media/language specificity and supplied-value selection; response effects remain protocol work |
| `http/media_type` | [Verified HTTP media values](http-media-type.md): strict octet parameters, canonical names/order and bounded grammar; verified media-range consumer in http/negotiation |
| `http/cookie` | [Verified values/date/policy/memory and persistence jars](http-cookie.md): explicit context, deterministic eviction, exact versioned private snapshots, atomic host rename and session exclusion; PERF-13 tracks remaining VM cost |
| `http/protocol` | [Verified strict HTTP1 head metadata](http1-framing.md); and [borrowed async head reading](http1-framing.md#asynchronous-borrowed-head-reading); [body boundary decisions](http1-body-framing.md); chunk/trailer metadata verified; [owned body consumption](http1-body-reading.md) verified; canonical head writing verified; [bounded body writing](http1-body-writing.md) verified; [complete-message writing](http1-message-writing.md) and [owned stream sessions](http1-session.md) verified; [continue coordination](http1-continue.md) verified; typed Internet exchange details and HTTP2 framing/HPACK/lifecycle follow |
| `http/client` | [Direct H1](http-client.md) and [explicit shared H2](http2-client.md) verified with [mixed ALPN](http-alpn.md); [optional H1 stale retry](http-retry.md) verified; content decoding remains open |
| `http/redirect` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `http/auth` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `http/cookie-client` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `http/proxy` | [Explicit proxy](http-proxy.md) verified: HTTP forwarding, CONNECT HTTPS/H2, credentials, access and environment rules; 37/339 plus inline PASS |
| `http/cache` | [Private completed GET200 cache](http-cache.md) verified: bounded storage, Vary, freshness, validation, current no-cache collapse and invalidation; 11/59 plus prior HTTP compatibility |
| `http/websocket` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `http/server` | Owned plaintext H1.1/H2 listeners, deadlines/drain and typed H1 WebSocket verified; TLS/mixed selection/RFC8441 remain open |
| `http/routing` | Immutable literal/named/wildcard routing, intersection checks, HEAD/405, RouteRejecting for Routes-generated 400/405 and successor contexts verified; PERF-31 construction optimization open |
| `web/routing` | [Verified web layer](web.md): controller bound-action endpoints, nested Scope prefixes and ExceptionScope chains compiled into one std Routes (661–663 PASS, actual TCP) |
| `web/request` | [Verified](web.md): route parameters, form-decoded query, fields, bounded text/json/form bodies with typed 400/413/415 |
| `web/reply` | [Verified](web.md): Text, Html, Json, Created, NoContent, Redirect as HandlingResult with owned framing |
| `web/exception` | [Verified](web.md): Exception is an Error and an RFC 9457 problem reply |
| `web/app` | [Verified](web.md): WebApplication over plaintext HttpServer; listen/server |
| `http/static` | [Verified rooted static files](http-static.md): ordered preconditions, bounded ranges, explicit listing/precompressed policy and one-shot bodies; actual H1/inline/example PASS |
| `http/form` | [Verified streaming multipart/URL/spool](http-form.md), including explicit cancellation ownership; focused/inline/example PASS |
| `http/sse` | [Verified bounded event streams](http-sse.md): framing/UTF8/state/ownership/actual H1, PERF-32 OPEN |
| `http/robots` | [Verified immutable grouped rules and owned fetch](http-robots.md), PERF-33 OPEN |
| `http/xmlrpc` | [Verified bounded capability codecs and HTTP adapters](http-xmlrpc.md), actual H1/cancellation, PERF-34 OPEN |
| `runtime/database` | [Implemented owned SQLite stack](db-sqlite.md); 823files/13395checks full PASS, current100focused checks PASS |
| `db/value` | [Verified portable values/statements/rows/errors](db-values.md), typed printing and bounded immutable observations; provider/async work follows |
| `db/statement` | [Verified portable values/statements/rows/errors](db-values.md), typed printing and bounded immutable observations; provider/async work follows |
| `db/row` | [Verified portable values/statements/rows/errors](db-values.md), typed printing and bounded immutable observations; provider/async work follows |
| `db/error` | [Verified portable values/statements/rows/errors](db-values.md), typed printing and bounded immutable observations; provider/async work follows |
| `db/connection` | [Implemented owned SQLite stack](db-sqlite.md); 823files/13395checks full PASS, current100focused checks PASS |
| `db/transaction` | [Implemented owned SQLite stack](db-sqlite.md); 823files/13395checks full PASS, current100focused checks PASS |
| `db/prepared` | [Implemented owned SQLite stack](db-sqlite.md); 823files/13395checks full PASS, current100focused checks PASS |
| `db/acquisition` | [Explicit preparation facets](db-pool.md#explicit-preparation-facets) preserve role confinement for prepared pooling/transactions; focused PASS |
| `db/pool` | [Implemented FIFO pool](db-pool.md): reset/expiry/cancellation/shutdown, focused PASS;823files/13395checks full PASS |
| `db/sqlite` | [Implemented owned SQLite stack](db-sqlite.md); 823files/13395checks full PASS, current100focused checks PASS |
| `db/postgres` | [Protocol 3.0 driver written in Okkama](db-postgres.md) (`db/postgres/wire`, `db/postgres/auth`): SCRAM/MD5/cleartext, TLS, streaming, transactions, prepared; verified against PostgreSQL 18.6 — 2026-10-23 |
| `db/sqlite/blob` | [Implemented owned SQLite stack](db-sqlite.md); 823files/13395checks full PASS, current100focused checks PASS |
| `db/sqlite/backup` | [Implemented owned SQLite stack](db-sqlite.md); 823files/13395checks full PASS, current100focused checks PASS |
| `db/sqlite/script` | [Implemented owned SQLite stack](db-sqlite.md); 823files/13395checks full PASS, current100focused checks PASS |
| `runtime/application` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `cli/arguments` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `config/key` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `config/source` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `config/value` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `config/environment` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `config/document` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `cli/spec` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `cli/parse` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `cli/help` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `terminal/capability` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `terminal/width` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `terminal/style` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `terminal/session` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `terminal/mode` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `terminal/screen` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `terminal/prompt` | [Verified application/CLI contract](application-cli.md): focused17/103 and native full840/13550 PASS. POSIX real integration / Windows16 fake checks; live Windows gate separate |
| `runtime/logging` | [Verified logging stack](logging.md): current15/87 PASS; frozen full scope recovered after one fixture-only failure,855files/13670 unique checks; all152 runtime/runner inputs unchanged |
| `logging/level` | [Verified logging stack](logging.md): current15/87 PASS; frozen full scope recovered after one fixture-only failure,855files/13670 unique checks; all152 runtime/runner inputs unchanged |
| `logging/name` | [Verified logging stack](logging.md): current15/87 PASS; frozen full scope recovered after one fixture-only failure,855files/13670 unique checks; all152 runtime/runner inputs unchanged |
| `logging/field` | [Verified logging stack](logging.md): current15/87 PASS; frozen full scope recovered after one fixture-only failure,855files/13670 unique checks; all152 runtime/runner inputs unchanged |
| `logging/record` | [Verified logging stack](logging.md): current15/87 PASS; frozen full scope recovered after one fixture-only failure,855files/13670 unique checks; all152 runtime/runner inputs unchanged |
| `logging/write` | [Verified logging stack](logging.md): current15/87 PASS; frozen full scope recovered after one fixture-only failure,855files/13670 unique checks; all152 runtime/runner inputs unchanged |
| `logging/named` | [Verified logging stack](logging.md): current15/87 PASS; frozen full scope recovered after one fixture-only failure,855files/13670 unique checks; all152 runtime/runner inputs unchanged |
| `logging/enrichment` | [Verified logging stack](logging.md): current15/87 PASS; frozen full scope recovered after one fixture-only failure,855files/13670 unique checks; all152 runtime/runner inputs unchanged |
| `logging/filter` | [Verified logging stack](logging.md): current15/87 PASS; frozen full scope recovered after one fixture-only failure,855files/13670 unique checks; all152 runtime/runner inputs unchanged |
| `logging/redact` | [Verified logging stack](logging.md): current15/87 PASS; frozen full scope recovered after one fixture-only failure,855files/13670 unique checks; all152 runtime/runner inputs unchanged |
| `logging/line` | [Verified logging stack](logging.md): current15/87 PASS; frozen full scope recovered after one fixture-only failure,855files/13670 unique checks; all152 runtime/runner inputs unchanged |
| `logging/stream` | [Verified logging stack](logging.md): current15/87 PASS; frozen full scope recovered after one fixture-only failure,855files/13670 unique checks; all152 runtime/runner inputs unchanged |
| `logging/file` | [Verified logging stack](logging.md): current15/87 PASS; frozen full scope recovered after one fixture-only failure,855files/13670 unique checks; all152 runtime/runner inputs unchanged |
| `logging/rotation` | [Verified logging stack](logging.md): current15/87 PASS; frozen full scope recovered after one fixture-only failure,855files/13670 unique checks; all152 runtime/runner inputs unchanged |
| `logging/queue` | [Verified logging stack](logging.md): current15/87 PASS; frozen full scope recovered after one fixture-only failure,855files/13670 unique checks; all152 runtime/runner inputs unchanged |
| `logging/syslog` | [Verified logging stack](logging.md): current15/87 PASS; frozen full scope recovered after one fixture-only failure,855files/13670 unique checks; all152 runtime/runner inputs unchanged |
| `runtime/locale` | Native immutable Unicode snapshot, Darwin live and Windows decoder/fake contract PASS; 873-file full gate PASS; live Windows separate. [Contract](locale-i18n.md) |
| `template/path` | Bounded exact path snapshots, lazy dotted parsing and nominal lexicographic ordering; [contract](template-guide.md) |
| `template/model` | Bounded explicit string bindings; library/section extensions listed below; [contract](template-guide.md) |
| `template/compile` | Plain parsing entirely in Okkama; [migration](template-parser-migration.md) |
| `template/tree` | Bounded section/inverted/partial instruction tree; [contract](template-tree-guide.md) |
| `template/render` | Bounded plain streaming and typed missing interpolation implemented; [contract](template-guide.md) |
| `template/sections` | Bounded JSON section/partial string and stream rendering; [contract](template-sections-guide.md) |
| `template/html` | Strict contextual JSON interpolation in HTML text/quoted ordinary attributes, string and stream output; composition pending; [contract](template-html-guide.md) |
| `template/syntax exclusions` | Retain exclusion; no added language syntax |
| `locale/tag` | Complete bounded RFC 5646 syntax, Unicode/transformed/private extensions, IANA 2026-09-17 preferences and canonical order verified. [Contract](locale-tag-guide.md) |
| `locale/data` | Pinned CLDR 46.1.0 / Unicode 16.0.0 parents, likely-subtags and plural data verified; number/currency/date/list data and separately pinned ICU collation verified. [Contract/evidence](locale-i18n.md) |
| `locale/system` | Explicit Environment/POSIX policy, injectable preference capability, canonical-case order and invariant fallback PASS. [Contract](locale-i18n.md) |
| `locale/match` | RFC 4647 bounded filter/lookup and explicit locale/likely expansion verified; full BCP 47 candidates remain open. [Contract/evidence](locale-i18n.md) |
| `locale/number` | Exact pinned CLDR rounding/notation/digits and strict parsing verified. [Contract](locale-number.md) |
| `locale/currency` | ISO identity/minor units and pinned CLDR currency representations verified. [Contract](locale-currency.md) |
| `locale/date_time` | Explicit Gregorian styles, wall projections and compatible historical zone names verified. [Contract](locale-date-time.md) |
| `locale/list` | CLDR 46.1.0 conjunction/disjunction/unit × long/short/narrow; 726 locale/root datasets and all 292 profiles checked independently. [Contract](locale-list.md) |
| `locale/collation` | Explicit immutable policy, ICU 76.1/UCA 16/CLDR 46.0, checked key domains and native ownership verified; full gate 889 files / 17,515 checks PASS. [Contract](locale-collation.md) |
| `i18n/plural` | Exact visible-precision PluralNumber, compiled cardinal/ordinal rules and 1,084 upstream sample checks verified. [Contract/evidence](locale-i18n.md) |
| `i18n/catalog` | Bounded immutable singular/plural catalogs, exact context identity, binary key lookup and explicit fallback verified. [Contract/evidence](locale-i18n.md) |
| `i18n/gettext` | Bounded MO 0.0 both endians, declared codecs, unsigned64 plural AST, ordered active PO roundtrip and Translating adapter verified against GNU fixtures. [Contract](gettext.md) |
| `runtime/testing` | Contextual skip/positions/causes, bounded capture, deadlines, split fake capabilities and process-isolated seeded runner implemented. Full completion gate:911 files/19,132 checks PASS. [Contract](testing.md) |
| `testing/check` | Scalar/byte/generic/unified diagnostics implemented with explicit rendering and bounded output. [Contract](testing.md) |
| `testing/error` | Typed matchers and sync/async assertions retain structured original causes in every report format. [Contract](testing.md) |
| `testing/time` | Guarded ManualClock with unique native domain identity, ordered wakes and joined cancellation cleanup PASS. [Contract](testing.md) |
| `testing/generation` | Scalar/domain/filter/sequence generation and bounded lazy reductions PASS. [Contract](testing.md) |
| `testing/property` | Seeded PCG cases, bounded shrinking and real async suspension PASS. [Contract](testing.md) |
| `testing/snapshot` | Explicit text/JSON/raw values, bounded checks and separate atomic descriptor-relative updates implemented; POSIX backend fails closed on unsupported hosts. [Contract](testing.md) |
| `testing/double` | Guarded fresh native state owners, borrowed nominal payloads and immutable snapshots PASS. [Contract](testing.md) |
| `testing/result` | Bounded nominal report values and explicit Base64 runner bridge PASS. [Contract](testing.md) |
| `testing/report` | Human/TAP13/JUnit XML/Base64 JSON reports include reasons and original causes; process bridge PASS. [Contract](testing.md) |
| `diagnostics/source` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `diagnostics/model` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `diagnostics/format` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `observability/attributes` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `tracing/model` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `tracing/span` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `tracing/sampling` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `tracing/propagation` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `tracing/export` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `tracing/instrument` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `metrics/model` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `metrics/state` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `metrics/export` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `metrics/instrument` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `profiling/cpu` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `profiling/allocation` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |
| `coverage` | Deferred scope; v3 class/action signatures are withdrawn pending an object contract, native boundary and failure/lifetime tests |

### `template/library` — VERIFIED

NamedTemplate binds an exact nonempty name to TextTemplate. TemplateLibrary
snapshots a bounded sequence (default 1024), rejects duplicates, preserves order,
and supports names/membership/value without compiling or rendering. Empty is
TemplateLibrary(); MissingTemplate retains the exact requested name.
No file lookup, native additions or partial expansion in this slice.

- [x] Ordered immutable library, empty value and missing-name errors.
- [x] Contract, consumer RED/GREEN, inline tests, comments and example.

[Contract](template-library-guide.md), evidence.

### `template/json` — explicit scalar adapter VERIFIED

JsonTemplateStrings implements TemplateStrings over the nominal JsonValue.print
protocol. Exact member paths yield scalar text; missing/non-object intermediate
paths and non-scalar leaves have separate typed errors. Traversal is bounded by
64 segments, per-object cardinality and per-scalar UTF-8 output. Source failures
propagate, temporary buffers close on all paths, and values are never reflected.

- [x] Unquoted strings, canonical Decimal/bool/null and NonScalarTemplateValue.
- [x] RED/GREEN consumer tests, comments, embedded diagnostics and example.
- [x] Current-item `.` and named member context via explicit JsonTemplateStrings;
  tests.
- [x] Section iteration and conditional string rendering in SectionTemplate;
  [contract](template-sections-guide.md).
- [x] Explicit partial expansion, direct/indirect recursion bounds and shared
  work/output budgets; [contract](template-partials-guide.md).
- [x] Structured streaming: bounded chunks, borrowed sink, shared budgets and
  string parity; [contract](template-section-output-guide.md).

[Contract](template-json-guide.md), evidence.

## Release verification

- [x] Red/green evidence for each required redesign.
- [x] All stdlib tests and complete Okkama suite pass.
- [x] Representative examples execute against std only.
- [x] Old changed APIs occur only in explicit migration/history/rejection examples.
- [x] All std imports resolve without the legacy-library fallback.
- [x] Audit, Book references and module docs reflect the final implementation.

Final evidence: review and test results.
Std suite: 90 files / 1381 tests, PASS. Full suite: 370 files / 3807 tests, PASS.
This closes the required v4 refactor, not the full breadth of the completion TODO.

Completion follow-up: the seven remaining BigInt arithmetic expressions are
VERIFIED. Related suites: 7 files / 512 tests, PASS; full suite: 371 files /
3834 tests, PASS. See [BigInt contract](bigint-guide.md) and
test-first evidence.

### INI implementation

IniDocument and IniSection implement IniContent.print(IniWriting); IniEntry
represents an ordered named string binding. Bounded snapshots observe their
sources once, reject duplicates and preserve exact Unicode names. Missing/
duplicate coordinates and invalid budgets have typed errors. No ambient DEFAULT,
implicit interpolation, map-shaped DTO or reflection-based printing. Lazy parser and
canonical bounded text follow the [recorded dialect](ini-guide.md). All names and
values are quoted on emission; custom-emitter failures are sticky.

## Synchronized state capability — 2026-10-03

`async/atomic::Atomic<T>` represents a shared single-value infrastructure cell.
Implemented synchronous `value`, void `replace`, boolean `compare_and_replace`,
and `closed`/idempotent `close`; comparison uses existing language equality. Operations
have no suspension point and are atomic within one interpreter scheduler.
Tests cover aliases, failed publication, lifecycle, inference and concurrent tasks.
Invoice evolution 09 uses Atomic-backed Memory with the existing CAS contract,
Go reference and File adapter; historical snapshots remain unchanged.

## Incremental CSV input — 2026-10-03

Implemented `csv/stream::CsvStream` with explicit consuming `row()` and typed EndOfCsv,
bounded read-ahead, incremental UTF-8/CSV validation and stable parser failures.
The materializing CsvInput remains compatible. Split boundaries, resource
budgets and lossless invoice interoperability are covered by tests; evolution
10 adopts the API after its focused checks. See [contract](csv-stream-guide.md).

## HTTP client policy — 2026-10-12

[Explicit policy](http-client-policy.md) implemented and verified (459: seven assertions, inline: one). Deadline and pool consumption remain the next step; no runtime change/full gate.

## Direct HTTP1 client — 2026-10-12

[Client, owned pool and deadlines](http-client.md) implemented; route/metadata/real reuse across caller scopes, gated cancellation, HTTPS CLI verified. No interpreter changes/full gate. H2, stale retry, decoding and optional client decorators remain open.

## Explicit redirects — 2026-10-12

[Redirecting](http-redirect.md) verified: 465/466/467 fifteen/three/two assertions; client 462/463 compatibility eight. Reused peers are checked against each new URL before emission. No interpreter/full gate. Optional cookie/auth/proxy/cache and H2 remain open.

## Cookie exchange consumer — 2026-10-12

[CookieExchanging](http-cookie-client.md) verified with explicit contexts/conflict rules, multiple Set-Cookie storage, failure cleanup and actual redirect-wire reselection. 468/469/470: eight/seven/two assertions; inline/example PASS. PERF-20 early budget abort fixed; no interpreter/full gate.

## Secret and scoped auth - provider gate PASS

[Opaque secrets](secret-bytes.md) and [ASCII Basic/Bearer](http-auth.md) have focused/actual HTTPS evidence. Full provider regression: 672 files / 12185 assertions PASS. Digest and remaining crypto APIs stay open; H2 work is isolated from the frozen root.

## HTTP2 wire foundation - 2026-10-12

[Local settings/internal frames/reader](http2-wire-foundation.md) verified: 474/475/476 five/seven/six assertions, inline settings. Pure std-only after the secret/auth full gate; no repeat full run. H2 preface/received settings/HPACK/flow/streams remain open.

### HTTP2 preface/settings slice verified

[Client preface and peer SETTINGS](http2-preface.md) verified with lazy exact wire emission, sequential peer observations, ACK, cancellation cleanup and one real loopback TCP example. Focused 477/478/479: six/ten/three assertions. Std-only; no repeat full regression. Public H2 exchanges, live streams/HPACK/flow and outgoing settings ACK tracking remain open.

### HPACK bounded wire primitives

[Integer/string/Huffman primitives](hpack-primitives.md) verified with all RFC octet codes and provider bounds: 480/runtime53 eight/thirteen assertions, inline/example PASS. Measured slow per-bit std decoding moved to a narrow native bulk provider (PERF-22). Full regression PASS: 680 files / 12,266 assertions / 1084 seconds; table/block/header semantics remain the next slice.

### HPACK tables and compression contexts

[Bounded tables and blocks](hpack-blocks.md) verified: complete static/dynamic decoding, exact context across RFC request/response sequences, minimum table shrink and conservative never-indexed encoder. 481/482 four/ten assertions; combined compatibility thirty-five PASS, inline/example PASS. Std-only after native full gate; no repeat full run. Live H2 transport remains open.

### HTTP2 HEADERS and typed field metadata

[Block and field mapping](http2-headers.md) verified: exact continuation sequence, cancellation, preflighted fragmentation, pseudo-field validation and normalized Host/authority agreement on input/output without body observations. 483/484/485 nine/nine/three assertions; combined 480..485 six files / forty-three PASS, inline/real TCP example PASS. Std-only; no repeat full gate. Live flow/streams/ALPN remain open.

### HTTP2 flow accounting

[Signed window/DATA accounting](http2-flow.md) verified: padded payload charge, negative SETTINGS credit, scoped overflow, dual-window send preflight and retained connection charge on stream error. 486 eight cases; compatibility twenty-one PASS, inline/example PASS. Std-only, no full rerun. Async credit/scheduler/deadlock prevention remain open.

### HTTP2 control values

[Reset/PING/GOAWAY metadata](http2-controls.md) verified: exact unknown codes, no ACK echo loop, monotone admission stop and explicitly opaque diagnostics. 487 five cases; compatibility twelve PASS, inline/example PASS. Std-only, no full rerun. Live stream/connection effects and retry remain open.

### HTTP wire byte serialization

[Bounded internal bulk joins](http-wire-bytes.md) replace per-byte maps through the existing buffer provider; exact 16 KiB DATA/HPACK payloads verified. PERF-23 records about 2.59x fixture improvement with residual VM traversal open. Std-only; no full rerun.

### HTTP2 stream lifecycle

[Half-close/reset/ID state](http2-stream-life.md) verified: independent stream states, final accepted payload distinction, minimal late-frame processing and odd uint31 exhaustion. 489 five cases; compatibility ten PASS, inline/example PASS. Std-only, no full rerun. Actual multiplexing/sibling physical reset remains open.

### HTTP2 consuming connection reader

[Internal input loop](http2-input.md) routes complete fields/DATA/control frames, retains HPACK before stream validation, tracks initial local ACK and queues replies without blocking on output. 490 eight plus 483 nine assertions PASS; nested continuation cancellation settles. Std-only, no full rerun. Owning writer/flow/public H2 client remain open.

### HTTP2 connection writer

[Bounded output and tickets](http2-output.md) serialize whole header blocks and isolate producer cancellation from physical writes. 491 eight cases; 483/490/491 compatibility 25 PASS after common preflight refactor. Std-only, no full rerun. Fair DATA/flow scheduling and physical session integration remain open.

HTTP2 duplex integration: `examples/std-v4/http2-duplex.ok`, 492 three actual TCP assertions PASS. Reader/writer run concurrently, responses reverse stream order, padding charges and PING ACK verified. Std-only, no full rerun; live body/flow/client work follows.

### HTTP2 outbound DATA

[Sender scheduler](http2-sending.md) separates reservation/wire debit, waits outside the writer, queues one frame per producer, handles SETTINGS/credit notifications and retires closed slots. 493 eleven cases; flow/writer/actual TCP compatibility 30 PASS after cleanup refactor. Std-only, no full rerun. Incoming body flow and public session remain open.

### HTTP2 incoming body buffer

[Body storage/reading](http2-body.md) bounds storage, credits padding/consumption separately, preserves final DATA and validates length/trailers. 494 eleven PASS; shared trailer refactor compatibility 76 PASS. No runtime changes/full rerun. Aggregate receiver/session ownership follow. Fresh probe startup measured separately in PERF-24.

### HTTP2 aggregate incoming flow

[Receive ledger](http2-receiving.md) preserves connection debit on stream failure, credits discard once, separates request registration/response attachment and retires body storage at application EOF. 494/495/496: 22 PASS, including real TCP WINDOW_UPDATE after app reads. Std-only, no full rerun; full owned session/client/ALPN remain open.


### Owned HTTP2 session

[Owned session](http2-session.md): `Http2Session(input,output,owner,local,peer,limits)` performs typed exchanges on one previously selected H2 transport. Supervised stream holders preserve native bodies across reverse response arrival, per-stream cancellation/reset, GOAWAY and physical close. Completed buffered bodies remain readable until application EOF/close/drop. Final compatibility: nine files / 79 assertions PASS, including actual multiplexed TCP with PING and exact consumption credit. Interpreter cancellation races have independent red/green evidence; the required frozen-source full regression passed: 699 files / 12455 assertions, 1125 seconds, unchanged 1324-file source inventory. H2 HTTPS/opening/client policy and the remaining HTTP roadmap are still open.


### HTTP2 verified HTTPS and shared client pool

[Opening](http2-connecting.md) verifies URL-derived certificate identity/SNI and actual h2 ALPN before preface. An explicit H2 policy in the normal [HttpClient](http2-client.md) enables shared physical owners, multiplexed leases, stream-local abandonment/body deadlines, checked-peer reuse and GOAWAY replacement. Final root H2 ten files /83 assertions PASS; unchanged H1 client/decorator compatibility eleven files /89 PASS. Interpreter ownership correction full gate: 700/12465 PASS; std-only client changes use focused gates. Mixed ALPN, replay-safe retry and remaining HTTP modules stay open. The recorded PERF-25 warnings were later fixed by the completion trampoline.


### Mixed HTTP1/H2 ALPN and shared conversation admission

[Negotiation contract](http-alpn.md): HttpClientProtocols validates exact enabled/advertised agreement; HttpSelectedConnecting adopts one checked verified transport according to actual ALPN. HttpClient supports a mixed H1.1/H2 policy; absent ALPN selects enabled H1, ordinary cleartext uses H1, H2-only remains explicit prior knowledge. The common physical pool serializes actual H1 body leases and multiplexes H2. Unsent queued requests are re-admitted after conversation retirement; emitted requests are not replayed. Whole admission tasks are supervised so client close cancels queues before releasing references. Final H1/H2/decorator/mixed compatibility: 28 files /240 assertions PASS. Std-only; the last interpreter full gate stays 700/12465 PASS. Replay-safe stale retry and remaining HTTP modules follow.


### Replay-safe stale HTTP1 retry

[Optional Retrying](http-retry.md) performs at most one replacement exchange for
an idempotent request with a replayable body after zero-response-byte failure on
a reused H1 conversation. Partial/1xx bytes, fresh errors, POST, one-shot bodies,
timeout/cancellation and broad TLS failures do not qualify. Direct session errors
remain compatible. Real TCP, mixed H1 and verified close_notify TLS are covered;
affected H1/H2/decorator compatibility: 33 files /295 assertions PASS. Std-only,
no repeated full regression. H2 unprocessed replay and other HTTP modules remain
open.


### Scoped Digest SHA-256

[Digest profile](http-digest.md) supports SHA-256/SHA-256-sess, qop=auth, userhash,
fresh secure cnonce, atomic bounded nonce counts, direct HTTPS scope, one bounded
stale refresh and optional header/trailer rspauth verification. Generic received
challenge parsing handles combined schemes, token68, quoted commas and bounded
empty list members. Final client/decorator/secret/Digest compatibility: 18 files
/144 assertions PASS; inline and independent actual HTTPS CLI PASS. No interpreter
or provider code changed, so no full regression repeated. SHA-512/256, NFC
credentials, auth-int and preemptive nonce caching remain open.


### Digest SHA-512/256 and UTF-8/NFC

[Modern Digest profile](http-digest.md) adds SHA-512-256/-sess, explicit UTF-8
credential NFC normalization, userhash/username* and exact UTF-8 realm wire
handling. Existing ASCII profiles remain covered. Correct algorithm vectors
agree with independent OpenSSL; RFC7616's truncated-SHA-512 example has a
reported erratum and is documented. Final focused 3/35 PASS; required native
full gate 727 files /12773 assertions PASS, unchanged 2079-file inventory.
Auth-int and preemptive nonce caching remain open. Proxy work is separate.

### Explicit HTTP proxy routing

[Proxy contract](http-proxy.md): explicit ProxyPolicy/Proxied and EnvironmentProxy;
absolute HTTP forwarding, HTTPS CONNECT with verified origin TLS/H1/H2, separate
hop credentials, actual proxy endpoint access/reuse checks and bounded no_proxy.
Deadline/cancellation drain real CONNECT transport. Final distinct focused coverage
37 files /339 assertions PASS, inline PASS. Interpreter unchanged; no full rerun.
TLS to proxy itself/SOCKS remain separate; PERF-26 is investigated and open.

### HTTP cache metadata foundation

[Bounded cache metadata](http-cache-metadata.md) implements Cache-Control and
private-cache freshness/age over explicit wall receipt and monotonic elapsed
coordinates. Five files /90 assertions and inline PASS. Std-only; no full rerun.
Body storage, Vary, revalidation and concurrent collapse remain open.

### Completed private HTTP cache storage

[Memory and streaming consumer](http-cache.md) implement bounded completed GET200
storage, Vary/auth/cookie partitioning, newest-Date selection, independent bodies
and unsafe invalidation with a late-GET barrier. Six files /40 assertions PASS,
including actual TCP. Std-only; no full rerun. Revalidation/collapse remain open.

### HTTP cache conditional validation

[Private cache](http-cache.md) now validates ETag/Last-Modified, merges identified
304 metadata over complete bytes, constrains request freshness and handles
only-if-cached without network effects. Original no-store and hop-field rules
remain effective through sanitization. Seven distinct files /43 assertions PASS;
std-only/no full. Concurrent collapse and stale permissions remain open.

### Cache admission and immutable header names

[Admission](http-cache.md) collapses ordinary concurrent validation and retains
full-body publication ownership; explicit waiter cancellation retires its key.
Fresh hits bypass hashing and immutable HeaderName canonicalization is memoized.
58 targeted HTTP files /523 assertions PASS; 2112 source hashes unchanged. No
interpreter/native change/full rerun. PERF-27/28 report remaining performance work.
Explicit stale permission and concurrent no-cache sharing are the next slice.

### Private cache final policy slice

[Private GET200 profile](http-cache.md) adds constrained max-stale and shares a
current no-cache validation only with already waiting bookings. Publication is
revoked across invalidation. Eleven files /59 assertions PASS, actual TCP and
secondary cleanup included; no interpreter/native change/full rerun.

### WebSocket value foundation

[Typed values](websocket.md) implement text/binary projection errors, explicit
message/frame/queue limits and UTF-8-sized wire close values with pinned IANA codes.
Four assertions and inline PASS; std-only/no full rerun. Framing/transport follow.

### WebSocket framing foundation

Exact masked/unmasked frames, minimal lengths, early prefix rejection and borrowed
short-read cancellation are implemented: 3 files /14 assertions PASS.
Evidence. Handshake and owned
connection follow; HTTP scope remains open. PERF-29 tracks measured mask cost.

### WebSocket handshake and message protocol

Bounded RFC6455 client/server handshake, strict browser SameOrigin policy and
fragment reassembly are implemented: 6 files /35 assertions PASS, inline PASS.
Report. Upgrade ownership and actual
connection follow; ordinary Exchanging does not expose hijacking. HTTP stays open.

### Owned WebSocket HTTP1 Upgrade

Checked direct ws/wss opening, URL-verified TLS/SNI/ALPN, bounded queues, message
controls/fragments and supervised close/cancellation are implemented. Explicit
server Upgrade checks Origin before 101 and preserves frame prefix.
Final compatibility: 13 files /70
assertions PASS, including actual TCP/TLS CLI. Failure.pm cause warning fix has
a separate required full runtime gate. Complete server, compression negotiation
and RFC8441 remain open; HTTP is not complete.

### HTTP server value foundation

[Context, handler variants and policy](http-server.md) establish endpoint/TLS
observations, restricted cancellation, parameter successors, explicit typed
WebSocket application transfer and zero-disabled deadlines/positive limits.
Two focused files /four assertions PASS; listener effects, lifecycle, drain and
routing follow. Std-only, no full regression repeated for this slice.

### Async missing-origin cause fix

Warning-capture RED → focused4/58 PASS → required full754files/12994assertions
PASS, frozen2140files unchanged. Optional source-token comparison preserves
cause identity without undef numeric warnings. Report.
Subsequent std-only changes retain separate focused gates; PERF-25 stays open.

### Owned plaintext HTTP1.1 server

[Server profile](http-server.md) owns accepted tasks, pipelined response/body
framing, independent zero-disabled deadlines,100Continue, immediate close/drain
and typed WebSocket application transfer against explicit trusted Origin.
Focused compatibility:7files/26
assertions PASS, actual TCP CLI/WS included. Explicit H2-only listener/session now has separate 7/39 session and 3/14 listener compatibility gates. Std-only; no full rerun. H2 idle/wire deadlines and protocol GOAWAY have a separate 11/47 focused gate. TLS/mixed selection/RFC8441 and general H1 peer-disconnect handling remain open. PERF-30 fixed.


http/form: [bounded streaming multipart, URL form and explicit spool](http-form.md) verified by focused, inline and executable example gates. No interpreter/provider or syntax change; no full regression repeated.


http/sse: [owned bounded event streams](http-sse.md), focused4files/12checks and inline/example PASS. Std-only; PERF-32 records remaining malformed UTF8 dispatch cost.


http/robots: [bounded immutable rules and explicit owned fetch](http-robots.md), focused/inline/example PASS; PERF-33 records dispatch cost. No runtime or syntax change.


http/xmlrpc: [bounded capability codecs and HTTP adapters](http-xmlrpc.md), five focused files/16checks including real H1, inline/example PASS. No runtime or syntax change; PERF-34 records residual XML scanning costs.

### Finite-float decimal prerequisite — 2026-10-13

numbers/float::ShortestFloat supplies exact interval-verified finite binary64
conversion and signed-zero identity through existing native primitives.
[Contract](shortest-float.md). [Locale/number](locale-number.md) now consumes it with exact rounding, all numeric
notations, pinned CLDR data and strict bounded parsing.


### Localized numbers — 2026-10-13

[Contract](locale-number.md): immutable numeric policies, decimal/BigInt/int/float
printing, percent/per-mille, explicit nonfinite symbols and strict parsing.
726 CLDR rows / 204 profiles / 77 numeric systems, generation-time pattern compilation.
Rounded carry promotes scientific/engineering/compact buckets, including the first
compact unit. Focused contracts and independent CLDR source oracle are green;
no interpreter/native or syntax change. Currency is the next locale module.


### Explicit localized currencies — 2026-10-13

[Contract](locale-currency.md): 178 current ISO codes, defined/undefined minor units,
explicit precision override, symbol/narrow/code/name, CLDR plurals, accounting and
all numeric notations. Currency placement belongs to one actual compact frame.
Parent-delta preservation covers all 726 source locales; boundary Letter data is
pinned Unicode 16.0.0. Focused contracts and independent ISO oracle are green.
No interpreter/native or syntax change. Localized date/time is the next module.


### Localized Gregorian date/time — 2026-10-13

[Contract](locale-date-time.md), module locale/date_time: standard Gregorian
styles, month/weekday/era/day-period data, numeric extensions, wall values without
zones and explicit-zone instant representations. Historical localized names
validate the observed offset/daylight state, with localized GMT fallback.
Source oracle covers every distinct date profile; constructor-bound wall text
removes repeated metadata preparation. No runtime or language syntax change.
Collation is now verified with its separately pinned ICU snapshot.


## Locale/i18n/testing completion continuation — 2026-10-13

The active profiles are implemented end to end, including the process runner,
snapshots, monotonic identity and [bounded diffs](diff.md). Existing legacy CLI and
fake.storage remain compatible. ICU, GNU gettext revision/codec, Gregorian-only
and POSIX snapshot limitations are explicit contracts in their guides.
Completion evidence distinguishes the
frozen full gate from later std-only verification. Remaining measured performance
work is tracked separately, not represented as missing functionality.
