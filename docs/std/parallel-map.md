# ParallelMapped contract

`ParallelMapped<T,R>(origins is Sequencing<T>, action is (T) is R, workers is int)`
represents a finite ordered process mapping. The two-argument constructor obtains
its worker count from `runtime/system.system_cpu_count`. `workers()` observes that
positive bound. Invalid counts throw `InvalidWorkerCount`, which preserves the
supplied count in `workers()` and explains it in `text()`.

`async results() is Sequencing<R>` reads inputs and performs work only when its
Future starts. Each call represents a fresh run; repeatedly awaiting one returned
Future uses the language's normal cached completion. Empty inputs start no worker.
Results preserve input order. The first implementation uses bounded batches, not
a work-conserving pool: it dispatches at most `workers` inputs, awaits the batch,
then advances. A failed batch prevents all later input assignment.

Workers are synchronous typed bound object actions accepted by existing `spawn`.
Their receiver must be reconstructible by named field constructor arguments;
receiver fields, arguments and results must satisfy the existing spawn codec.
Anonymous closures, async actions, arbitrary objects and shared native handles
are not transferable workers. Process errors retain the existing spawn diagnostic
boundary; arbitrary typed child errors are not reconstructed in the parent.
The ordinary single-file CLI supplies process isolation. Embedded/Local execution
retains its existing executor behavior and cannot establish CPU parallelism.

Every dispatched child is registered in a FutureScope before further dispatch.
AllFutures observes a batch in completion order for failure and publishes its
values in input order. Input access/transfer failure, worker failure or parent
cancellation cancels outstanding workers and awaits their cleanup before the map
settles. Cancellation remains Cancelled. No new interpreter/provider API is added.

Validation: RED import/API tests, injected host executor tests for bounded dispatch,
failure and cleanup, actual process CLI tests for ordered results and transferable
boundaries; inline module tests and runnable example. Interpreter changes, if
required by a reproduced blocker, need a separate runtime task and full regression.
