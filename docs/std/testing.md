# Testing values and explicit policies

These modules build on the contextual `test` capability and existing structured
async operations. They do not install global clocks or generate mocks by
reflection. [Source](../../std/testing), focused evidence.

## Context and checks

`testing/context::TestContext().require()` throws `MissingTestContext` unless a
case is currently running, including its imported actions and async cleanup.
`runtime/testing::test_context_active()` returns an immutable
boolean; test-state providers receive no interpreter, lexical environment, assertion state
or first-class test capability. Production mode and test mode without a running
case both return false. NativeCallContext's optional flag defaults to false and
accepts only 0/1.

`testing/check` defines `Checking.check() is void` and
`AsyncChecking.async check() is void`. Checkers require context even when they
pass; failures are recorded with contextual `test.fail()` and ordinary test
execution continues.

- `CloseTo(actual,expected,relative,absolute)` rejects negative tolerances and
  uses the symmetric max(abs, relative * max(abs(actual),abs(expected))) criterion.
  Equivalent scaled comparisons avoid overflowing on opposite extreme finite
  floats. Builtin float currently admits only finite values; no artificial
  NaN/infinity values are introduced by this library.
- `TextEquals(actual,expected[,context])` reports the first Unicode scalar
  mismatch and escaped surrounding text; `BytesEqual(actual,expected[,context])`
  reports the first byte and bounded surrounding octets. Context defaults to 8,
  must be 1..64, and diagnostics never dump the entire input.
- `SequenceEquals<T>(actual,expected[,context])` compares ascending items, stops
  at the first mismatch, and reports its index or length difference. It reads
  each size once and rejects negative / over-1,048,576 sizes before indexing.
  Arbitrary T is not reflected or implicitly serialized. The fourth constructor
  argument is an explicit `(T) is string` rendering action for bounded surrounding
  values. TextEquals also adds [unified diffs](diff.md); diagnostics cap at 8,192
  scalars and escape controls. Long shared Unicode prefixes use binary search.
- `MatchingRegex(origin,pattern)` uses an ordinary RegexMatches search, not an
  implicit full-string match. Invalid patterns/source failures propagate.
- `Eventually(condition,clock,timeout,interval)` and `AsyncEventually` evaluate
  immediately, then await through the explicit Timing capability. Timeout is
  nonnegative; interval is positive; waits are clipped to the remaining monotonic
  deadline. Zero timeout still evaluates once. Conditions are serial and exact
  bool; typed failures/cancellation are propagated, never retried as false.

`testing/error` defines `ErrorMatching.match(Error)/description()`,
`AnyErrorMatching()`, `ErrorTextMatching(expected)`, `ErrorRegexMatching(pattern)`,
`Throwing<R>(action,matching)` and `AsyncThrowing<R>`. A normal return or a wrong
error records a failure including the original public error text. Panics remain
outside typed-error matching. Domain-specific matching is an ordinary role
implementation. Failed matching retains the original typed error as a structured cause.
Assertion positions identify the external checker call, including after async
suspension; ordinary uncaught errors and panics remain failed cases.

## Manual time and doubles

`ManualClock(start is Instant)` implements Timing and is usable only inside a
running case. Monotonic reading begins at zero. `advance(duration)` moves wall and
monotonic coordinates together; `set_wall(instant)` changes wall only. Negative
advances/waits fail without publication; a zero wait completes immediately.
No host time or sleeping. Up to 1,024 pending waits / 2,000,000,000 registrations.

State publication precedes wake delivery. Due waits complete by deadline then
registration order; cancellations remove registrations through defer. Reentrant
advance during wake delivery throws `InvalidManualClock` to preserve ordering.
`pending()` observes registrations; `closed()` observes closure. Idempotent
`close()`/destroy wakes pending waits with `ManualClockClosed`; aliases see the
same owner. Each clock receives a fresh runtime-minted monotonic domain; values
from different ManualClock owners or system/manual clocks throw IncompatibleClock
on order/minus. plus() retains the domain. SystemClock instances share the active
registry domain. Explicit one-argument MonotonicInstant values share a separate
"explicit" domain. Identity allocation reads no host clock. Async defers retain
private declaration rights and complete before an owned awaiting caller unwinds.

`RecordedCalls<T>()` records immutable `Recorded<T>` values with zero-based BigInt
indices. `record(value)`, `size()`, `records()`, `close()`; snapshots preserve call
order and old snapshots never grow. A persistent list makes record publication
constant-time; records() materializes a forward snapshot. Capacity 65,536.
`Stub<T>(Sequencing<T>)` snapshots at most 65,536 items once; `next()` consumes,
`size()` is configured size, `remaining()` decreases, and exhaustion throws
`StubExhausted`. Close/destroy is idempotent; post-close actions reject stale owners. Native TestState owns bounded cursor/capacity
state and rejects operations without a running case, including retained handles.
Generic T remains in nominal std values and never crosses a portable native ABI.
Both owners require a running test context; create fresh owners instead of reset.
Role fakes compose them explicitly. Nested resource T remains borrowed, with its
original owner's lifetime; neither owner extends resource scope.

## Reproducible generation and properties

`Generation<T>.sample(random,size)` returns `Generated<T>` with value(), string()
and `reductions() is Sequencing<Generated<T>>`. This accepted contract revises the
old raw-T reduction candidate: every counterexample retains its own reproduction
text without reflection. Size is 0..65,536. Core reduction lists are finite,
deterministic and duplicate-free; reductions stay inside their generator bounds.

`BooleanGeneration()`, `IntegerGeneration(minimum,maximum)` (half-open),
`FiniteFloatGeneration(minimum,maximum)` (finite closed bounds after rounding),
`ElementGeneration<T>(Sequencing<GeneratedElement<T>>)` and
`FilteredGeneration<T>(origin,predicate,attempts)` are implemented. Integer size
controls distance from the nearest admissible zero; exact rat arithmetic computes
windows and midpoint shrinking at int64 limits. Float generation consumes the
Randomizing 53-bit float stream and safely interpolates toward its canonical
admissible target as size grows. Caller-supplied domain reproduction text is
bounded to 4,096 scalars; element count 1..4,096; rejection attempts 1..10,000.
Filters apply to reductions too; exhausted rejection throws GenerationExhausted.
`SequenceGeneration<T>(origin,minimum,maximum)` implements
Generation<Sequencing<T>> with inclusive length bounds 0..4,096. Size controls the
upper length minimum + min(size,maximum-minimum); zero size fixes the minimum.
One length draw precedes serial element draws, all receiving the same size. Equal
seeds and size schedules reproduce both length and order.

Sequence reductions first offer the minimum-length prefix and an intermediate
prefix, then one first element reduction per position in order. They preserve
minimum length and element-generator bounds; identical reproduction text is
excluded. A reduction list holds O(n) plans and builds only requested candidates,
each in O(n), rather than eagerly allocating O(n²) elements. At most 4,096 plans;
if length plans consume slots, the final positions beyond that bound are omitted.
This is a bounded strategy, not exhaustive search. Each element reproduction is
at most 4,096 scalars; the unambiguous `[length:text,...]` sequence representation
is at most 65,536 scalars. Arbitrary Generated implementations must supply faithful,
deterministic representations and valid reductions. Broader shrink strategies
remain roadmap work.

`Property<T>(generation,seed,cases,maximum_size,shrink_limit,condition)` and
`AsyncProperty<T>` create a private PcgRandom for each check. Cases 1..10,000;
size increases deterministically from zero to maximum (one case uses zero);
shrink limit 0..128 bounds condition evaluations while reducing only the first
false case. A report includes seed, zero-based case index, size and smallest
observed counterexample, not a claim of a globally minimal example. Typed condition
errors are reported distinctly without shrinking; cancellation propagates;
panics remain programming failures. Async evaluation is serial and has real
suspension/shrinking coverage. `PropertyReproduction` retains those coordinates.

## Structured results and output

`TestPosition(source,line,column)` uses `source()`, not the withdrawn v3
`module()` candidate: `module` is a language keyword. Positive 1-based line/column.
`TestFailure(message[,position])` has message()/located()/position(); absent
position throws MissingTestPosition. `TestOutput(stdout,stderr,truncated)` retains
raw immutable bytes, never assumes UTF8, and bounds combined bytes to 16 MiB.

`TestResult(name,status,duration,failures,output)` preserves each case; statuses are
passed/failed/skipped/timed_out/crashed. Passed/skipped cannot carry failures;
negative durations and invalid sizes fail. `TestReport(start,duration,seed,results)`
snapshots discovery order, counts timeout/crash as failures, and supplies
`exit_status()` (0 or 1), TestCounts and all coordinates. At most 4,096 results,
16 MiB aggregate output and 1 Mi scalar aggregate names/failure messages.
These are explicit result values. The process runner projects its wire through
RecordedTestReport; legacy result hashes remain an optional embedding projection.

`HumanTestReport(report,width)` (20..512 Unicode scalars per content line) emits
plain text, bounded escaped names/messages, seed/counts and a bounded hex output
preview. `TapTestReport(report)` emits TAP13, complete hex-encoded captured bytes,
status/duration diagnostics and a skip plan for an empty report. Names cannot inject
TAP directives; an independent TAP::Parser verifies this. `JsonTestReport(report)`
uses Stage 13 objects with stable member order and complete Base64 stdout/stderr;
start/duration nanoseconds are strings for exact transport, seed is an exact JSON
number. Positions use a `module` JSON key while the Okkama action remains source().
No ANSI styling. The completed runner bridge is described below; reports include
structured original causes and skip reasons.

`JUnitTestReport(report,suite)` uses [immutable XML objects](xml-values.md), never
handwritten report markup. Suite length 1..4,096 scalars. The testsuite root counts
failed cases as failures and timeout/crash as errors; skips have skipped children.
Case order is unchanged, duration is exact fixed-nine-place decimal seconds,
positions remain in diagnostics and seed/start_unix_ns are suite properties.
stdout/stderr are Base64 text in system-out/system-err; suite property
captured-output-encoding=base64 identifies it. Each case carries a
captured-output-truncated property. Names and messages use explicit escaped
control diagnostics so forbidden XML characters cannot corrupt the report.
An empty report remains valid XML with zero counts. XML output is bounded to
32 Mi scalars; excessive output (including repeated long classnames) fails with
InvalidXml. This is a documented JUnit dialect with case properties, not a claim
of compatibility with every vendor's schema. Independent ElementTree assertions
verify structure, counts, durations, positions and arbitrary byte roundtrips.

[TAP13 primary specification](https://testanything.org/tap-version-13-specification.html).


## Snapshots

`SnapshotFile(root is DirectoryLocating,name is string)` represents one file
beneath an explicit directory. Names are one Unicode component, at most 255 UTF-8
bytes: empty, dot, parent traversal, separators and NUL are rejected.
`TextSnapshot(text)` normalizes CRLF and CR to LF and strictly encodes UTF-8.
`RawSnapshot(bytes)` preserves every octet. `JsonSnapshot(Json)` uses ordered
compact JSON (`Json.string()`) with a final LF. These are explicit ByteSource values; they
perform no filesystem access during construction.

`Snapshot(source,file,limit).check()` requires context, observes actual bytes once,
and reads exact expected bytes. Missing expected data records a failure and never
creates a file. Byte mismatches use BytesEqual's first index and bounded window.
Limits are 1..1,048,576 bytes. Oversized values and unsafe coordinates are typed
failures, not automatic truncation of expected data.

Only `SnapshotUpdate(source,file,limit).update()` writes. No runner option enables
implicit updates. `runtime/snapshot` acquires a no-follow directory descriptor,
opens the leaf relative to it without following symlinks, and rejects directories
or special files. An update writes and fsyncs a private sibling temporary, closes
it, then atomically renames it relative to the retained descriptor. Failed writes
or replacement remove the temporary and preserve expected bytes. Replacement of
the ambient root path cannot redirect an acquired descriptor. A concurrent leaf
symlink replacement can be replaced as a directory entry, never followed outside
the root. This guarantees atomic visibility, not directory-fsync crash durability.
POSIX IO::FD 0.3.11 and FFI::Platypus 2.12 supply the boundary; unsupported hosts
fail closed. The existing read-only FileRoot API gains no write action.

## Runner and contextual capabilities

Existing `okkama -t file.ok` retains the legacy textual runner. Explicit options
select the process runner and the real std report bridge:

```sh
OKKAMA_STDLIB_DIR=/path/to/std okkama -t suite.ok --workers 4 --seed 42 --report junit
okkama -t suite.ok --filter invoice --timeout 5 --capture-limit 65536 --report json
```

Reports accept `human`, `tap`, `json`, `junit`. Workers are 1..32, optional seed is
signed int32, timeout is 0..86,400 seconds (zero disables it), capture is
1..1,048,576 bytes combined per case. Filtering is a literal name substring.
Explicit seed uses a documented uint32 LCG/Fisher-Yates schedule, separate from
property PCG streams. Results always retain declaration/discovery order. Each
case runs in a new executable process and registry. Zero matches is a valid
empty report; a source with no declared tests remains a configuration error.

`test.skip(reason)` is the fifth contextual action. It unwinds cleanups; previous
assertion or cleanup failures still make the case failed. The reason is retained
in structural and std reports. Skipped cases have successful exit status; failed,
timed_out and crashed cases do not. Ordinary typed errors, assertion failures and
panics are failed; unexpected worker termination is crashed.

Async deadlines request cooperative cancellation and join owned child cleanup.
A synchronous deadline unwinds through the same lexical defers. Cleanup runs once
with declaration rights. A worker exceeding its timeout plus one-second cleanup
grace (including cold worker startup) is killed and reported timed_out with that limitation;
POSIX workers own separate process groups so inherited-output descendants are
also stopped, even if the worker parent has already exited; an unresponsive
process cannot be promised completion of arbitrary user cleanup.

Process workers capture actual stdout/stderr pipe bytes, preserving each stream
order across native and terminal writers. Each stream prefix is bounded while
draining; published combined output gives stdout priority and clips stderr to the
remaining budget, independent of read-event ordering. Temporary memory is bounded
by twice the configured combined limit; truncation is explicit. Aggregate report capture is capped at 16 MiB; case
names and failure text have independent bounds. No filesystem spool grows with
output. `Interpreter.test_results()` keeps its legacy three-key projection;
`test_results(1)` adds status, start/duration nanoseconds, bytes, truncation,
positions, causes and skip reason. Direct embedding with test_capture_limit captures Native Stdio calls through the
default registry or explicitly wired TestCapture; the process runner captures all
worker writers through pipes. Worker source/cwd/stdlib coordinates use strict
Unicode UTF-8 transport.

`testing/runner::RecordedTestReport(text).value()` projects bounded explicit JSON
wire into nominal TestReport values. Capture crosses this bridge as Base64,
not individual numeric JSON nodes. TestFailure supports one position and one
recursive cause (maximum depth 31); all std report formats retain cause text and
JSON preserves its structure. TestResult optionally carries skip reason.

Inside role fakes, `fake.reading` exposes get/has/size and `fake.writing` exposes
set/remove/clear. set/clear return void; remove returns a bool, including false for
absence. A wrong capability action is rejected before evaluating its arguments.
Neither view can escape as a first-class value. The legacy fake.storage surface
is retained for compatibility and deprecated in active examples.

`DistinctReductions<T>(source,parentText)` explicitly removes self/repeated
reproduction keys while retaining first discovery order. It caches keys once,
sorts them, then restores source order: no quadratic membership scan. Bounds are
4,096 candidates, 65,536 scalars per key and 1 Mi scalar aggregate. Filters use it;
core generators already provide unique finite deterministic candidates. Custom
Generated implementations must provide stable reproduction identity and finite
reductions; properties retain lazy sequence plans and their explicit shrink bound.

## Interpreter performance guards

The legacy CSV/TOML parser fixtures retain exact values, recursion-warning
checks and measured interpreter call/depth budgets. Wall-clock budgets belong
to a separate sequential job:

```sh
cd okkama-pl
perl script/check-parser-performance.pl
```

Use the current Perlbrew Perl. The job runs one unmeasured warmup and five fresh
VM observations per fixture, compares their median against the original 10/25
second budgets, and prints every sample. CLOCK_MONOTONIC measures end-to-end
parse/import/execution time without call-count instrumentation. Run on an idle,
fixed machine without a concurrent regression/profiler for a comparative baseline.
The ordinary parallel regression keeps structural guards on every invocation.

## Configuration maximum-quota stress

The ordinary regression keeps structural overlay growth and AVL invariants.
The real 4096-key boundary has an additional explicit stress command:

```sh
cd okkama-pl
prove -Ilib -It/lib xt/audit/setting-overlay-limits.t
```

It admits 4096 distinct keys and a replacement, rejects the next distinct key,
and verifies that the following source entry is never observed. The test uses
the actual quota, without reducing a production limit for testing. Expect several
minutes of interpreted work; this ceiling is not a latency promise. The measured
CPU backlog and ways to investigate it are recorded in
the overlay audit.

## Robots record-limit ordering stress

```sh
cd okkama-pl
prove -Ilib -It/lib xt/audit/robots-record-limit-order.t
```

The explicit test uses all4096 retained records. Invalid delay text is ignored
at capacity; the following valid record must fail with the records limit rather
than being swallowed by parsing error recovery. It performs two full policy
constructions and may take minutes. See the CPU investigation.

## Maximum route partition depth

```sh
cd okkama-pl
prove -Ilib -It/lib xt/audit/route-partition-deep-family.t
```

This explicit stress constructs129 disjoint routes of128 segments, requiring128
successive partitions. It checks zero full overlaps and no host recursion warning.
The production route/segment budgets stay unchanged; the workload may take
minutes. See the measured CPU backlog.
