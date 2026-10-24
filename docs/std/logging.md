# Logging: explicit records and destinations

Stage25 composes existing clocks, process and I/O capabilities. There is no
native logging provider, global logger or ambient task context. Applications
pass the `Logging` role; sinks consume only `LogWriting`. Resource owners remain
explicit. [Executable example](../../examples/std-v4/logging-stack.ok).

## Records and policy

`LogLevel` is debug/info/warning/error/critical in increasing severity.
`LogLevelName(level).string()` is uppercase; `LogLevelFromName(text).value()`
accepts case-insensitive canonical ASCII names, without whitespace aliases.
`LogLevelOrder(left,right).value()` returns `Order`.

`LogName(Sequencing<string>)` preserves exact segments; dots are diagnostic only.
`ChildLogName(name,segment).value()` appends one segment. Bounds reuse SettingKey:
1..64 segments, each nonempty, at most4096 scalars. Ordering is lexicographic.

`LogFields(Sequencing<JsonMember>)` rejects duplicate/empty names, preserves order
and freezes custom JsonValue sources through Stage13. It accepts at most256
members, names at most4096 scalars, depth32 and rendered snapshot1MiB.
`membership(name)`, `value(name)` (MissingLogField), `members()` and `values()`
observe the immutable snapshot; values returns JsonObject. `EmptyLogFields().value()`
and `CombinedLogFields(base,added).value()` are projections. Overrides keep their
original position; new names append. Empty immutable layers reuse the snapshot.
Bytes, domain values and dates require explicit JsonValue representations.

`LogContext(name,fields,process)` requires a nonnegative process ID; zero means
unavailable. `LogRecord(time,level,message,context[,cause])` observes time, level,
message, context, has_cause and cause. Absent cause throws MissingLogCause. The
cause remains an Error; formatters call only public text(). Mutable custom Error
text is observed at formatting, not frozen at record construction.

`NamedLogging(name,destination,clock,fields[,process_reader])` implements Logging:
log(level,message), log(level,message,fields), log(level,message,fields,cause).
Each action merges fields, captures one clock instant and process ID, constructs
one record and calls the destination once. The default CurrentLogProcess uses
an existing provider symbol; no provider implementation changed. LogProcessReading
allows deterministic injection. `child(segment)` preserves destination/clock.
`FieldLogging(origin,fields)` merges explicit context before nearest call fields;
`ChildLogging(named,segment).value()` delegates child construction.

`CollectedLogs([capacity])` defaults4096 records and returns immutable snapshots;
capacity is nonnegative and exceeding it throws LogCollectionLimit.
`DiscardingLogs()`, `BroadcastLogs(destinations)` (at most128),
`FallbackLogs(primary,fallback)` and `BestEffortLogs(origin)` implement LogWriting.
Broadcast attempts all in declaration order; LogDeliveryFailure retains ordered
causes. BestEffort explicitly counts failures with BigInt. Fallback runs only
on failure and can duplicate a partially delivered record; ordinary errors propagate.

`LevelThresholds(fallback,Sequencing<LevelOverride>)` selects the longest exact
segment prefix (at most1024 unique overrides) and admits the threshold inclusively.
`AllowedLogs(destination,allowing)` filters before formatting/I/O;
`EnabledLog(allowing,name,level).value()` permits avoiding expensive record creation.

`RedactedLogs(destination,Sequencing<string>,replacement)` replaces exact,
case-sensitive top-level fields, keeping their positions. It freezes replacement
JsonValue and bounds selected names to256. `RedactedCauseLogs(destination,text)`
replaces only an existing cause. Place redaction outside queue/broadcast/sinks.
This policy does not discover secrets in messages or nested objects.

## Formatting and output

`TextLogLine(record,TimeZone).string()` produces RFC3339 with explicit offset;
`JsonLogLine(record).string()` uses UTC and ordered time/level/name/process/message/
fields/optional-cause members. User fields are nested and absent cause is omitted.
Stage13 handles JSON quoting. Controls C0/C1/DEL and U+2028/U+2029 are escaped, so
one record is one physical line without a trailing newline. Depth34 and1MiB bound
rendering. JSON numeric spelling follows the current Stage13 number writer; short integers
are positional, large exponents stay bounded and exact. Earlier profile/example
captures used the previous scientific spelling and retain the same numeric value.

`TextStreamLogs(Writing,zone)` and `JsonStreamLogs(Writing)` append UTF8 plus one
LF through exactly one all-or-error write. No implicit flush or retry occurs.
`TextTerminalLogs(TerminalOutput,zone,Sequencing<LogStyle>)` and
`JsonTerminalLogs(TerminalOutput,Sequencing<LogStyle>)` style the complete line.
`LogStyle(level,TextStyle)` entries replace the unimplemented enum-key map
candidate: imported nominal enum keys are not supported by the current map type
encoding. At most five unique levels are accepted; duplicates throw InvalidLogStyles.
Missing styles use default colors; TerminalOutput/ColorPolicy govern emission.

## Files and rotation

`FileLogOptions(PermissionMode,flush_each,sync_each)` is explicit policy.
`LogFile(Path,options)` implements Writing/Closing with write, flush, reopen,
closed and close. Construction/unused close never open a file. First write opens
append/create with close-on-exec. Creation respects umask; an existing file is
never chmodded. One native write reports IncompleteWrite with its exact prefix;
no retry occurs. Native failures translate to existing filesystem errors.
flush_each flushes the underlying unbuffered descriptor; sync_each requests fsync.
Explicit flush does not open an unused file. Reopen opens a replacement before
releasing the old handle; failure to open keeps the old descriptor usable.

`Rotating.needed(BigInt size,Instant opened,Instant now)` and
`suffix(Instant now,int index)` define explicit rotation policy.
`SizeRotating(positive BigInt)` compares the existing size before the next append;
one record can exceed the quota and is never split.
`IntervalRotating(positive Duration,TimeZone)` compares wall-clock time since open;
a backward clock delays rotation. Its suffix uses filename-safe RFC3339.
`RotatingLogFile(Path,options,rotation,backups,clock)` offers the same stream/owner
operations. Backups are0..4096; zero removes the completed active file.

The sink reserves `<path>.archive.<positive-generation>.<suffix>` regular files.
It recovers increasing BigInt generations and retention by scanning the parent
on rotation; collection uses a persistent list and trimming one stable BigInt sort.
It refuses non-regular active/archived paths (including symlinks), and removes oldest
generations. The suffix index is1 plus the current archive count, not a lifetime
counter; the generation ensures uniqueness even with equal/backward timestamps.
Suffixes are1..128 scalars and must be one portable path component (no slash,
backslash or colon). Each rename is atomic where supported; rename/prune/reopen
is not a crash-atomic transaction. A failed write/rotation poisons future writes;
close remains available. No independent processes may rotate the same path.
Compose archive compression only after that file is inactive.

All plain/rotating actions are non-suspending in one cooperative interpreter.
This serializes a sink, not several processes or native threads. Host filesystem,
flush and durable-sync operations can block; queues do not make them asynchronous.
Performance investigation.

## Bounded supervised queues

`QueuedLogs(LogWriting,capacity,LogOverflow,Supervising<int>)` adapts a synchronous
sink. For asynchronous delivery use
`QueuedLogs(LogDelivering,LogQueueOptions,Supervising<int>)`, where LogDelivering
has `async deliver(LogRecord) is int`. Constructor arities differ deliberately;
there is no type-based same-arity overload. LogDraining has async drain() is void.

LogQueueOptions accepts capacity1..65536 and reject/drop_newest/drop_oldest.
Capacity bounds waiting records; one extra record may be in flight. Immutable
record references live in a private persistent Deque; admission/overflow updates
have no await. Full reject throws LogQueueFull and counts that loss; drop_newest
counts/refuses the incoming record; drop_oldest discards the oldest waiting record.
Retained records preserve FIFO. Dropped/failed/cancelled records are observable
through BigInt dropped_records(), not silently treated as successful delivery.

The first accepted record starts exactly one worker through the supplied scope.
Event wakes it on state changes. There is no unmanaged spawn, capacity wait,
implicit default scope or process-global sink. Supervising.start is eager: the
first write, or an Event wake, can execute downstream formatting/synchronous I/O
before returning. This API is not a bounded-latency promise. A synchronous file
sink still blocks the cooperative scheduler during host write/fsync.

`await drain()` captures the accepted sequence at execution and waits for earlier
in-flight/queued records, excluding later calls. Overflow-discarded earlier records
are settled according to the explicit policy and loss count. The first destination
error is latched; drain and later write throw it, and the scope observes the worker
failure. A raw builtin NativeFailure is latched as LogDeliveryFailure with the
original public Error cause; supervision retains the original native failure.
Ordinary domain Error types are preserved unchanged. Close rejects input, counts/removes undrained records, wakes an idle worker
and cancels only its current delivery Future. It does not cancel the caller's scope
or unrelated children. Call drain first for graceful shutdown; keep the queue and
borrowed destination alive until its supervising scope has settled.

External scope cancellation closes the worker and publishes LogQueueCancelled to
later queue observations; supervision keeps the original builtin cancellation.
Builtin Cancelled cannot be stored and later thrown as an ordinary user Error.
Unexpected delivery cleanup errors remain real worker failures. Use explicit close
or defer close before waiting for a scope: an open idle worker intentionally waits
for more records. Destroy delegates idempotent close.

## UDP syslog

`SyslogFacility` follows the24 RFC facility values, kernel0 through local7=23.
`SyslogOptions(facility,host,application,maximum_payload)` accepts1..65507 bytes;
header names are printable non-space ASCII, at most255/48 characters, empty as NIL.
`SyslogMessage(record,options)` is a complete ByteSource. Its RFC5424 version1
header maps debug/info/warning/error/critical to7/6/4/3/2, uses UTC and truncates
fractional seconds to six digits without rounding. Process0, missing header names,
message ID and structured data use NIL. MSG is the JSON record with a UTF8 BOM,
with no trailing newline. Oversize packets throw DatagramTooLarge, never truncate.
These choices follow [RFC5424](https://www.rfc-editor.org/rfc/rfc5424.html).

`SyslogLogs(UdpConnection,SyslogOptions,LogQueueOptions,Supervising<int>)`
reuses queue/write/drain/drop/close behavior and borrows the connected UDP owner.
The seven-argument form `(connection,facility,host,application,capacity,overflow,scope)`
uses2048 bytes. Configure the budget for the receiver and network:2048 is a receiver
recommendation, while the minimum required support is480 for IPv4 and1180 for IPv6.
Queue drain confirms local send completion, not remote receipt/order; UDP has no
acknowledgment, per [RFC5426](https://www.rfc-editor.org/rfc/rfc5426.html).
TCP/TLS/journald transport is not part of this sink.

## Delivery status

Current focused15files/87checks PASS; inline levels and current JSON example PASS;
VSCode grammar184checks PASS. The frozen full run checked853files/13654 successful
assertions with one fixture load failure. Absolute FindBin loading fixed that
fixture; frozen recovery6files/31checks PASS. Reconciliation verifies all853 full
files (13664 checks), plus two late tests:855 files/13670 unique successful checks.
This is a recovered full result, not a single uninterrupted full PASS.

All5332 original frozen inputs remain unchanged; all152 runtime/runner files match
current code. Post-freeze changes are a std-only native failure latch, two focused
checks and absolute test loading. Parallel committed JSON/web work is preserved;
current logging was rechecked against its updated JSON number writer. Details and
hashes are in the delivery audit and
evidence. No new grammar syntax was added.
