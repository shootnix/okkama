# Sparrow-next execution and suspension boundary

Status: current synchronous and async boundary checkpoints, reconciled with source
on 2026-11-03. Focused implementations do not close full phase acceptance.
Follow [the execution order](execution-order.md) and [evidence matrix](foundation-evidence.md).

## Existing machine ABI

The private generated entry is C `(RunState*) -> u32`. `RunState` owns no data;
it borrows the session slot buffer for one synchronous invocation. Slots are
16-byte payload/tag pairs. Offsets are computed with Rust `offset_of!`, not copied
into a second hand-maintained layout. Generated code is owned by `CodeMemory`;
no function pointer escapes that owner's lifetime. A session cannot move or grow
its slot buffer while generated code is running. No Rust unwind crosses this
boundary.

Heap calls (2026-11-05): `RunState` also borrows the task's slot count and heap,
and (2026-11-06) the immutable program: a static `Call` of a native function is
also a heap call, so natives run without the dispatcher round trip. A typed native
failure is left in `Heap::deferred_raise` and unwound by the service path.
For heap-only instructions (`jit::heap_call`: objects, arrays, maps, text, ranges,
equality, rationals, conversions and the general cases of natively specialised
scalar operations) generated code stores the live values of the PC to their
slots and calls `jit::heap_service(RunState*, const Op*)` through an indirect C
call. It runs `execution::heap_op`, the same function as the service path, and
returns 0 (committed), 1 (committed, collection due) or 2 (failed). After a
commit the code reloads the successor's live values from slots, so no machine
register is kept live across the call. On 1 it exits with status 12 at the next
PC and the dispatcher collects. On 2 the error is left in `Heap::deferred_failure`
and the code exits with status 4 at the same PC; the service path raises the
deferred failure instead of executing the instruction again. Collection never
runs inside a heap call. An internal panic inside `heap_service` cannot unwind
through generated code and aborts the process. The instruction pointer is
embedded in the code; the immutable plan owns the compiled region.

Status 0 returns a slot (ignored for a statically void call), 1 yields with PC and
remaining fuel, 2 reports invalid internal state, 3 requests call dispatch, 4
requests heap/scalar service, 5 reports unbound evaluation, and 6 requests a
conditional lookup/catch transfer; 7 requests throw/rethrow dispatch; 8 requests resource cleanup, 9 reports an unreachable internal terminator,
10 requests await dispatch, 11 transfers to another compiled region while
remaining in the dispatcher (it is not a host yield), and 12 requests collection
before resuming at the stored PC. These are internal constants, not a public plugin
ABI. Literal trap categories remain in the immutable plan. The present public
host error is text, and uncaught terminal failures are memoized by Session.
`Session::failure_report()` additionally exposes a structured snapshot of the
terminal language failure; the textual host Error is not its complete representation.

Floats occupy F64 SSA variables; other supported payloads use I64 variables.
Tags cross copy/edge/call/suspension boundaries with payloads. Arithmetic uses
native instructions where supported and returns to the service dispatcher for
exact large values. SPB operand types, call arity, captures, role witnesses and
cap signatures are checked before JIT generation.

JIT compilation is lazy per region (at most 64 plan instructions). Native target
selection, definition and finalization errors propagate as `new engine JIT:`
errors through `Compiled::new`; the dispatcher does not select Cold on failure.
Cold uses the same verified plan without requesting a compiled region. Current
focused results are from this host only, not cross-platform acceptance.
An engine/JIT compilation error is memoized and terminal for that Session.
Subsequent resumes return the same error without retrying compilation or
switching engines. Active registers are no longer live; unreferenced values can
be collected. A fresh Session sharing Code may retry a region that failed to
compile. This differs from a catchable language failure and its traced payload.

## Calls and roots

There is one active register window, sized for the largest loaded function.
Direct, closure, role and cap calls capture arguments into reusable scratch before
reusing that window. Closure captures precede explicit arguments; role/cap calls
prepend the original owner. Calls do not recurse through Rust or native entries.
Continuations store return PC and only live caller values in shared buffers.

Current safepoints spill active live values before service. GC traces those
values, saved callers, pinned constants and outgoing object/container/closure/
any references. Handles contain arena index and generation; references never
own descendants recursively. Collection is nonmoving and stop-the-world, after
an instruction has committed its output. No concurrent mutation means no write
barrier is needed by this collector. Parked task roots, Future captures/results,
failure payloads and native-handle metadata participate in tracing. Every new
provider must register its retained VM values before permitting suspension/GC;
Rust-only native I/O requests do not retain VM Slots.

Normal lookup edges capture the found value, perform parallel edge moves from
old registers, then bind the found result. The missing edge does not define it.
Backward liveness models this difference.

## Exceptional control flow

Implemented and tested in cold/JIT:

- A handler edge belongs to the failing instruction's pre-commit state. Values
  live only in a handler must be included in JIT spills and GC roots.
- A call must preserve both normal-return live values and exceptional live values.
  The previous destination value can be live in a handler even when that register
  is the call's normal result. Excluding the destination unconditionally is wrong.
- Unwinding uses the explicit continuation buffer. For a failed callee, lookup
  resumes at the caller's call PC, not at the ordinary continuation PC.
- Handler moves read old registers before the failure register is bound.
  Catch selection binds a thrown payload after its edge moves, like lookup.
- Tail elimination is prohibited across pending handler/cleanup obligations.
  PC-changing rewrites must remap handlers or remain disabled in their presence.
- Language thrown values, panic, trap, causes and secondary failures stay distinct.
  Engine validation/internal faults do not become catchable user objects.

Failures are traced arena records, including thrown payloads, cause chains and
secondary failures. Cyclic chains do not recurse through Rust ownership.

Synchronous resource cleanup uses a stack of closing cursors. Each destroy body
is entered through the same explicit continuation buffer. Region handles and the
primary failure are roots while cleanup is suspended; saved caller registers are
restored before another destructor or the error handler runs. Cleanup continues
after a destructor throws. Normal close reports the first cleanup failure;
unwind retains the original failure and appends cleanup failures as secondary.

Region instructions implement resource ownership, not memory reclamation. A
closed-world type graph identifies descriptors that can reach declared destroy
bodies, including late fields and recursive types. Proven plain objects have no
ownership entries and their graphs are skipped. Unknown/opaque types remain
conservative. Resource-bearing graphs are traversed only by explicit ownership
instructions; ordinary copies and calls never walk them. Metadata is separate
from object storage and reclaimed with unreachable handles. This is a correct
initial resource path, not the final allocation/throughput result.

Still required for phase 5: complete native close/cancellation coverage, source
mapping, traces for elided calls, and full diagnostic compatibility. Async
cancellation already uses these cleanup continuations; focused tests do not
establish the complete cross-provider lifecycle contract.

## Required native and Tokio boundary

The shared `okkama-runtime-abi` crate contains provider signature metadata, not a
Rust value representation. It remains the signature authority. Phase 7 must map
that metadata directly to new engine values and independent native libraries;
there must be no conversion through old runtime Value/Object/Region.

Synchronous adapters may borrow text/bytes only for the duration of the call.
Pending operations own their Rust inputs or hold registered VM handles as roots;
no pointer into active registers, scratch arguments or an unrooted arena may
survive an await. Results become rooted VM values before any subsequent safepoint.
Native resources have explicit close/cancellation states independent of memory GC.

Tokio owns scheduling/reactor/timers; the engine supplies resumable state through
Rust Future/Poll/Waker. Local VM values remain non-Send unless a particular boundary
creates owned cross-thread data. Blocking providers must use a bounded blocking path,
with provider-specific interruption where required; the system DNS path has
bounded admission, while the complete blocking-provider audit remains open. Abort alone cannot implement
Okkama cleanup, cancellation or SQLite interruption. Cold start, repeat await,
multiple waiters and cached result/error are language contracts of the adapter.

Native and async dispatch use the same machine statuses plus heap Future states
(`Cold`, `Running`, `Io`, `Worker`, synchronization waits and cached `Ready`).
Typed native failures and resource handles are implemented for existing adapters;
full registry support and complete ABI/lifecycle acceptance remain open.

## Tokio and cold Future implementation checkpoint

Tokio 1.54.1 owns current-thread scheduling, LocalSet, reactor and timers.
AsyncHost drives Rust futures; Session.complete_async yields through Tokio after
4096 VM instructions. This is tested for fairness, but does not bound the time
of an individual long heap/native operation. Host drop is not VM cancellation.
References: [Runtime builder](https://docs.rs/tokio/1.54.1/tokio/runtime/struct.Builder.html),
[LocalSet](https://docs.rs/tokio/1.54.1/tokio/task/struct.LocalSet.html).

Immutable Code has shared ownership; sessions/tasks retain generated code.
Task holds execution state separately from Heap. One resume borrows Code and
Heap; ordinary instruction/call dispatch does not acquire RefCell/Rc guards.
Cold async calls allocate a Future arena record with retained argument slots.
Await enters its body through an explicit continuation and caches success or
failure. Both active await continuations and future captures/results are roots.
Repeated await does not execute the body again. Pending I/O, local spawn, multiple
waiters and cancellation have focused implementations described below; a running
external Future is not silently treated as ready or serviced through the legacy scheduler.

FailureReport snapshots have index edges, so cycles/shared causes do not impose
recursive ownership. Error.text is evaluated only for uncaught failures and can
yield/collect through the dispatcher. Description roots retain the whole failure
graph. Report traces describe surviving activations, without reconstructing
elided tail calls; source/rewritten-PC mapping is an outstanding obligation.


### Parked tasks and host notifications (2026-11-03)

A session borrows its shared heap once per execution slice. Dispatcher operations
and generated code do not borrow a RefCell per instruction. Suspended root sets
participate in GC; the active set is cleared before resume and republished at
return, including failures. Internal task results are typed Slots and remain GC
roots after completion. The public entrypoint still returns Integer.

A pending Future lazily creates a Tokio watch channel. Subscription precedes
host suspension; completion wakes all subscribers, and cancelling a native timer
wakes its driver without waiting for the original deadline. Ready futures do not
allocate notification channels. Local language spawn now uses Tokio tasks with a startup acknowledgement at the
first language suspension or completion. Root shutdown cancels pending registered
tasks and waits for VM cleanup before reporting an unobserved non-cancellation
failure. This is covered by focused fixtures, not full async acceptance. Real
process workers require isolated revalidation after the launcher fix; general
deadlock detection and complete native-handle shutdown coverage remain open.


### Intrinsic objects and cancellation (2026-11-03)

Validated intrinsic entrypoints lower to short plans using the same flat call and
await ABI. Event uses Tokio watch; Lock uses an owned Tokio Semaphore acquisition.
A permit request is registered before the parent resumes after spawn, remains in
its FIFO position across host polls, and is removed synchronously on cancellation.
Pending requests trace their Okkama receiver. Object-keyed native side tables are
swept after the object arena. A guard releases only on close; its GC drop forgets
the permit instead of silently unlocking the application lock.

Session owns its LocalSet and pending startup acknowledgement. Dropping a Rust
completion future is not language cancellation and does not discard spawned tasks;
polling the same session again resumes them. Dropping the Session itself still is
host teardown, not a substitute for running the language shutdown protocol.


### Native synchronization and structured tasks (2026-11-03)

Channel and Barrier requests use a compact enum over provider-owned Rc records,
Tokio watch notifications, cancellation hooks and explicit GC roots. No wrapper
allocation is added for that enum. Semaphore uses a binary FIFO Tokio gate with
an i64 logical capacity; pending acquisitions reserve capacity for diagnostics.
Grant close returns capacity once; GC drop does not release it.

Groups and collectors lower to ordinary flat VM plans with handlers. A cached
async call keeps a receiver-to-Future edge traced by GC. Collection selections
subscribe to completion notifications; initial ready inputs use input order,
subsequent arrivals use completion serials. Selection currently scans remaining
inputs per event. FutureScope runs its action and child-group cleanup using the
same continuation ABI. These features have focused checks, not full acceptance.


Completion streams have a synchronous factory prefix and a cold body entry PC in
FunctionPlan. The factory reads origins, preserving synchronous errors/effects;
Future captures still contain only parameters. Inline await and spawned sessions
both start at the cold entry, without adding a PC field to every Future object.
Timeout/deadline use the common completion collector with clock methods executed
as language calls. Ready origins bypass clocks; cancellation drains both inputs.
System clock readings and the decimal bigint operations required by Duration
cross directly into Rust, without legacy native trampolines.

Owned AsyncHost observes Tokio's idle callback to diagnose impossible waits.
Per-driver metadata records runnable/internal/external waits and only a Waker;
Tokio owns all scheduling. Native pending requests are removed before diagnostic
unwinding so cleanup cannot grant a resource to an abandoned request. Embedded
current-thread Tokio runtimes install AsyncHost::check_stalled_tasks themselves.
New native I/O providers must classify their external wait source in this hook;
the host currently distinguishes timer deadlines, process workers and native I/O
as external waits (`async_host.rs::drive_task`). Internal synchronization waits
must not be misclassified as external work to hide a stalled VM.


## Native I/O ownership

A native request owns a Rust future and Rust-only output data; it must not retain
untraced VM Slots. Tokio polls the request with no Heap borrow. A completed output
is published synchronously into the generational arenas, then the language Future
caches the Slot or typed NativeFailure. Suspended tasks retain their usual roots.
Publication failure itself is cached as a program failure, so consuming a Rust
output cannot leave the language Future permanently pending. Cancellation before
publication drops the unpublished output; cancellation after Ready preserves the
cached outcome. Parent cancellation drains a cancelled Io/Worker request before
language cleanup; a drain failure is retained as secondary.
Cancellation disposes the pending Rust future; language defer/destroy continues
through ordinary VM unwinding. TCP reads release their reservation on cancellation;
incomplete writes close the shared stream. Close wakes outstanding readers,
writers and acceptors. Input shutdown wakes readers separately from writers.
DNS blocking resolver calls already started by the host cannot be interrupted;
their abandoned result never references or mutates the VM.
DNS admission is process-wide, limited to four jobs. A started blocking job owns
its permit until it actually exits, even after its async waiter is cancelled.
Ready spawn transitions consume Tokio cooperative budget in addition to VM fuel;
neither budget bounds the duration of a single blocking native operation.

Native type validation accepts a short declaration name only inside its owning
root module; imported native handles require their fully qualified name.
