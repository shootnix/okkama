# New Sparrow execution core

This is the new implementation, separate from `sparrow-vm` and the copied
`sparrow-runtime`. It uses the independent SPB/ABI schema, Cranelift and Rust numeric libraries.
There is no delegation to the old interpreter, heap, scheduler or JIT.

The current implementation supports synchronous scalar functions, exact integers,
finite float arithmetic, exact rational arithmetic, bool/byte/char, numeric
conversions, Unicode text operations, arrays/builders, array fallback, branches,
parallel edge moves, direct calls and returns. Objects
support allocation, field reads, late initialization and successor. The entry must still
be parameterless and return int. All SPB instruction forms now have explicit lowering; unsupported types and
native providers are rejected before execution. This remains incomplete, not a production replacement or a claim of
language compatibility.

```sh
cargo test -p sparrow-engine
cargo run -p sparrow-engine --bin sparrow-next -- program.spb
cargo run -p sparrow-engine --bin sparrow-next -- --cold program.spb
python3 tools/check-sparrow-boundary.py
```

Both paths consume a verified immutable execution plan. Machine code is compiled
lazily in regions of at most 64 plan instructions, cached on the shared function
plan. Cross-region edges spill destination-live values and preserve exact PC,
parallel moves and instruction fuel. Large functions do not become one giant SSA
graph. The test runner shares these plans across cases while isolating heaps.

`--test program.spb` runs the compiled test profile (build it with `okkama build -t`).
Assertions accumulate within a case; runtime failures end the current case and
later cases continue. Native coverage currently includes clocks, UTF and wire
encodings, BigInt, binary numeric formats, numeric IP addresses, RustCrypto,
BufferState, SecretState, TestState, Decimal, IDNA/PSL, regex, flat-arena JSON,
DB cells/schemas/captures, forward/reverse DNS, TCP/Unix streams, UDP and multicast.
Environment and cwd are VM-local. Files, directories, locks, temporary resources,
glob, Tokio file watch and descriptor-relative roots/snapshots have native adapters.
DB drivers, TLS, process APIs
and the remaining ABI are still required by the roadmap.

Native I/O owns a Rust future with no VM slots or heap borrow across suspension.
Tokio polls it; completion publishes the typed result into the heap synchronously.
Cancellation drops the Rust future and unpublished resources. DNS uses Tokio's
resolver and a safe `dns-lookup` reverse lookup on the bounded blocking pool;
an already-running OS resolver call cannot be interrupted, but its discarded
result never accesses the VM. External I/O participates in stall detection.

Both paths consume a verified immutable execution plan. The cold path belongs to
this crate; JIT compilation failures are returned to the caller rather than
silently switching engines. JIT scalar values use native i64/f64 SSA values, without
old `Value` slots. The private machine ABI is C `(RunState*) -> u32`, with an owned
slot buffer, fuel, result and continuation PC. Status distinguishes return,
call, numeric slow-path request, cooperative yield and invalid internal state. Executable memory is owned for the entire call and explicitly
released on drop, including compilation failure paths. No code pointer escapes.

`Executable::start()` creates an independent session. `resume(fuel)` performs at
most that many instructions, and zero fuel does not advance the program. Both
engines reuse session storage across resumes; completion is memoized. A backward
dataflow analysis includes parallel edge moves and loops, and the JIT stores only
live values at suspension boundaries. `live_registers()` and `live_integer()` provide read-only inspection. Heap
collection traces active roots, saved callers and object fields; failure payloads and chains are traced; native
and suspended task roots are traced; native handle arenas are traced and swept independently of old runtime storage.

Direct calls return from machine code to a flat session dispatcher. A direct call
immediately followed by returning its result is treated as a tail call: it replaces
the current function without adding a continuation. This includes mutually recursive
functions. Functions with handlers disable tail/recurrence/object-elision rewrites;
cleanup regions also disable these rewrites.

Ordinary calls append a return label and only caller values live on normal or exceptional continuation
to shared buffers. Argument capture uses reusable scratch storage before overwriting
parameters. There is one active slot window sized for the largest reachable function,
with no per-call allocation or full-window clearing. Continuation buffers grow
amortized when needed; this is not constant-space execution for general recursion.
No function call recursively enters Rust or recursively calls a native JIT entry.
This is an initial dispatch implementation, not yet the final optimized call path.

Precise roots cover suspended frames, native requests and errors. Cooperative
budget exhaustion is separate from an Okkama Future; Tokio drives actual async
waiting through the host adapter. A complete pure factorial-shaped recurrence is recognized and rewritten to an
exact accumulator loop, independent of its name. Different bases, steps, register
aliases, effects and other shapes are not guessed equivalent. General recurrence
elimination and more general tail-position analysis remain future work. Per-instruction budget checks are a correctness bootstrap, not the
final safepoint placement for the optimized JIT.

All phases and remaining obligations are tracked in
[the roadmap](../../docs/sparrow-new-runtime-roadmap.md). In particular:

- Eliminate recursive work with accumulators where equivalence is proven.
- Lower tail calls to flat control flow, including mutually recursive functions.
- Store remaining continuations as labels plus live values in reusable storage,
  without Rust recursion or a heap allocation for each call.
- Keep async semantics on a Rust Future/Tokio backend with explicit suspension.
- Full compatibility precedes performance comparisons. Automatic CPU parallelism
  is a separate v2 objective after the current roadmap.

Slots are 16-byte payload/tag pairs. Small integers occupy the payload directly;
big integers use generational handles into a numeric heap. Small-int native
add/subtract/multiply/comparison uses SSA; overflow and big operands return to
exact `num-bigint` arithmetic. Large operands are borrowed, not deeply copied to
read them. Results normalize to small integers when possible. Collection reuses
dead entries and rejects stale handles; constants are pinned and exported results
own their data. This is not the completed object collector of phase 3.

Finite floats remain f64 in native SSA. Small normalized rationals pack an i32
numerator and positive u32 denominator into one payload; larger ratios use
`num-rational`. Float-to-rational conversion follows Okkama's canonical decimal
text contract, and large rational-to-float conversion preserves DECIMAL128
rounding. Typed SPB validation checks operand types and call arity before JIT
compilation and before recurrence rewriting.

Objects currently use one boxed slot slice containing a descriptor ID header and
fields, addressed by a generational arena handle. A stop-the-world, nonmoving
mark/sweep pass traces outgoing handles with a reusable worklist. Copies and
argument transfers never traverse object graphs. Native code returns and spills
live values before heap service; collection runs after an instruction commits.
Pinned numeric constants are leaves. No collector runs concurrently, so this
initial collector needs no write barrier. Stale handles are rejected. Deep graph
release does not recurse through Rust ownership. Declared destroy bodies execute through explicit cleanup continuations. This is not yet full phase 3:
native/async resource cleanup, all root classes and general allocation elimination remain
outstanding. Local nonescaping objects used only through field reads are replaced
with captured typed registers. Snapshots preserve the values at construction;
object arguments, aliases, late binding and other escapes prevent this rewrite.
`elided_object_count()` reports the static count; `collection_count()` reports
completed collections in a session. An unbound constructor field is storage,
not a user-visible null; evaluating it fails in both cold and native paths.

Strings, arrays and builders use the same traced handle contract. Array builder
finish transfers its Vec to an immutable array and leaves the aliased builder
empty and usable; append borrows the array slots directly. String indices count
Unicode code points; UTF-8 conversion returns bytes. Array fallback captures the
found element, executes parallel edge moves using old values, then binds the
result. Liveness models this edge-specific definition. Ordered maps/builders, scalar enums, compiler-lowered enum payloads, exact
ranges, void calls and closures are supported. Roles, caps, any boxes and iterative structural equality are supported.
Typed handlers, throw/rethrow and cause/secondary links execute in both paths.
Synchronous resource regions, destroy, defer lowering and cleanup failure aggregation
are supported. Source-level diagnostics, broad native integration and the full async acceptance gate remain open.

Closure captures share one allocation with their function ID header. Calls copy
captures and explicit arguments into reusable session scratch before entering the
callee; captures are traced even when only a suspended caller holds the closure.
The loader checks capture and function signatures before generating code. A
terminal uncaught error is memoized; restarting a failed session is not allowed.
Trap categories are retained in the plan but the current host error API still
exposes text: this is not yet the full language failure/handler protocol.

Role references and caps retain the owner handle without an additional wrapper.
Witness/cap signatures are validated before native compilation. Structural
comparison uses reusable iterative work buffers and remembers visited pairs;
float structural equality preserves signed zero, and range equality retains
original bounds and inclusivity. See [the ABI contract](../../docs/new-runtime/abi.md)
for current roots and the outstanding exceptional/native/async boundaries.

Resource-bearing descriptors are found by iterative reverse type reachability.
Plain object graphs have no ownership records, even in programs with resources.
Closing cursors retain roots across yield/GC and execute destroy in reverse
registration order. Deeply nested cleanup uses flat continuations rather than
Rust recursion. The compatibility probes include real compiler output for
try/catch, defer failure precedence and returned resource escape.

Tokio 1.54.1 now supplies AsyncHost (current-thread + LocalSet) and cooperative
Session.complete_async / Executable.run_async. Code ownership is shared separately
from mutable Task and Heap. Cold async functions, ready awaits and cached failures
are supported, including 10000 nested awaits without a Rust call stack.
Native timer Pending, typed native failures, local spawn, concurrent/cached await,
and cancellation are implemented. Spawn startup waits for the first language
suspension or completion, not a fuel yield. Root shutdown cancels outstanding
spawned tasks, runs their cleanup and reports unobserved failures. Tokio owns task
scheduling and notifications; the VM does not implement a parallel executor.
Driver transitions also consume Tokio's cooperative budget, so repeated spawning
of ready Futures cannot indefinitely reset the VM fuel budget without yielding.
Host yields preserve eager-start ordering and are not language suspension points.
Native output-publication errors become cached language failures. Parent
cancellation waits for pending native/worker cleanup before language destroy and
retains completion failures as secondary failures.
These paths have focused compiler fixtures and stress tests, not yet the full
async conformance gate. FutureCompletions and Within/Before now run with FIFO
requests, structured cancellation and injected or native SystemClock providers.
Completion stream preparation occurs synchronously at method call; reservation
and input execution stay in the cold body. Direct and bound synchronous process
workers use Tokio process/pipes and framed scalar/container transfers; cancellation
reaps the child. The codec is iterative, and a response length trailer prevents
marker text inside returned strings from corrupting framing. Full native lifecycle
integration, nested worker audits and broad native providers remain open.

Worker commands insert `--worker` before the SPB path. Arguments after the path
belong to the language program and must not select CLI modes. The launcher also
sets the reserved `SPARROW_INTERNAL_WORKER` environment marker: a marked process
rejects a missing/misplaced worker switch before loading or executing main.
The process-worker integration test is ignored by default; run it only under
external process-tree limits. Pure argument-construction and parsing regressions
do not execute VM programs or create child processes.

Library executables have no implicit worker command: embedders must call
`with_worker_command` before process spawn is available. `run_tests` and
`worker_response` retain their signatures and likewise have no implicit launcher;
`run_tests_with_worker_command` and `worker_response_with_command` opt in explicitly.
The CLI supplies its executable, mode and absolute SPB path to all three entry
paths (main, tests, worker). Worker working directories are VM-session state;
request processing does not change the embedding process's current directory.
The reserved worker marker is excluded from the language's host-environment
snapshot so ordinary subprocesses do not inherit a worker-only entry restriction.

Worker stdout and stderr share a 64 MiB captured-output budget, including the
serialized response. `SPARROW_WORKER_OUTPUT_LIMIT_BYTES` can override this host
limit with a positive byte count. Overflow returns a process failure; it never
silently truncates a result. Pipe I/O belongs to the request future. On Unix,
cancellation/drop stops the worker process group; ordinary descendants may not
keep pipes open after worker exit. Descendants that deliberately leave the group
are outside that guarantee.

System DNS forward/reverse lookups share four blocking-job permits across hosts.
Queued lookups are aborted on cancellation; a started system call keeps its
permit until it actually returns. Cancellation cannot stop a running libc
resolver call. Numeric addresses retain the ready path without a blocking job.

The owned Tokio host diagnoses stalled VM waits from `on_thread_park`, after
Tokio has drained runnable work. A pending timer prevents a deadlock diagnosis.
The failure unwinds language cleanup and removes native queue membership first.
Embedders using their own current-thread runtime install
`AsyncHost::check_stalled_tasks` with `Builder::on_thread_park`; without this hook,
Tokio can wait indefinitely for an impossible signal. The hook observes idleness;
it does not schedule VM tasks or implement a separate executor.
Error.text and failure graph snapshots execute independently of the old runtime;
source maps and reconstructed tail-call traces are not yet provided.


The intrinsic loader now validates Event and Lock/LockGuard signatures/layouts.
Event uses a Tokio watch latch. Lock uses Tokio Semaphore FIFO acquisitions;
requests register before host polling and cancellation removes them immediately.
Guard.close releases its permit once; collecting an unclosed guard does not unlock
it. Native side tables are swept with their object handles. LocalSet and startup
acknowledgements live in Session, so temporarily abandoning a host completion poll
preserves child frames, native queue positions and GC roots for resumption.

Cancellation/Cancelling use shared Tokio watch signals. Atomic stores a borrowed
GC edge and validates closed/CAS behavior. Channel supports bounded FIFO buffers,
zero-capacity rendezvous, cancellation and close/drain semantics; Tokio delivers
notifications. The channel policy queues do not schedule VM tasks. GC traverses
buffered/queued payloads and completed requests, and sweeps endpoint side tables.
Focused OCS probes currently pass 167 core and 23 async cases in both modes;
this is not the full conformance gate. Native UTF-8 encoding and stdout/stderr
writes now execute directly through Rust; byte arrays still use ordinary slots.

FutureGroup and AllFutures/Raced/FirstSuccessful use ordinary VM continuations;
Tokio notifications drive pending selections. Collector calls cache Future identity
per receiver; GC follows that cache without retaining unreachable receiver cycles.
Cleanup failures after explicit group cancellation remain visible, unlike the
frozen origin which loses them (see roadmap's known compatibility discrepancy).

Semaphore supports full i64 capacities with a Tokio FIFO admission gate and exact
logical accounting. Barrier supports cyclic generations, cancellation/broken
states, abort/reset and immutable snapshots. FutureScope caches its body and waits
for child cleanup on success, body failure and cancellation. Channel payload GC
retention is implemented; cross-scope resource ownership remains an open gate.
