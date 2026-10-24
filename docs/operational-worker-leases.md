# Operational worker leases

The maintained billing candidate uses two time authorities. `business_clock`
continues to drive invoice periods, due dates and business retry schedules.
Payment/refund SQL leases use the database's UTC epoch seconds: PostgreSQL
`floor(extract(epoch from clock_timestamp()))::bigint`, or SQLite
`cast(strftime('%s','now') as integer)`. The expression is evaluated inside the
mutating SQL statement. A reading taken before expensive invoice reconstruction
cannot authorize a later completion. PostgreSQL processes share the server
clock; processes sharing a SQLite file must share the database host clock.

`SystemClock.wait` schedules the local heartbeat interval. Its monotonic/civil
value is never stored as a shared lease timestamp. `WorkLease` is application
policy for existing tenant/job/generation tables. `LeasedWork<T>` composes the
existing `Raced<T>` selector with preparation/provider work and a heartbeat that
fails on lease loss. The selector cancels and drains the other future on failure,
success or caller cancellation. Provider/telemetry adapters preserve `Cancelled`.
Ordinary provider transport failures still produce UNKNOWN.

Default lease duration is120 seconds, with renewal every40 seconds (the whole-second interval is floor(duration/3)). The bounded
real-process test uses12/4 seconds. Renewal requires the same tenant, id and
generation, a running/pending/unknown state as appropriate, and a lease that is
still valid according to the DB. An expired worker cannot revive its own lease.

Before financial finalization, the worker stops and drains the heartbeat,
renews its still-valid claim once, then executes the existing transaction.
Completion independently checks generation/status/DB time. This avoids racing a
heartbeat against the same transaction releasing its lease on successful commit.
Finalization has at most120 seconds from the last renewal; a slower finalization
fails the gate. An uncertain commit remains uncertain and is never automatically
retried as a fresh economic operation.

Expiry and process death retain the original payment/refund operation key.
If death happened before durable vendor routing, lookup returns UNKNOWN. Routing
is committed before any vendor effect, so ordinary recovery can bind the original
key through the existing routing path; it never invents a replacement key.
Refund money stays reserved until a definitive success or decline is committed.
A valid lease is SQL authority, not proof that all older physical I/O has stopped.
The E07 budget counts valid leases; external idempotency and durable provider
routing remain necessary. Session abandonment is a separate resource protocol
(IMP22). Existing billing retries after a definitive decline remain business
policy; the heartbeat does not implement economic retries.

Adoption requires draining/fencing old workers before changing clock protocols;
mixed virtual-clock and DB-clock workers are unsupported. Host/server UTC clock
jumps, indefinite CPU starvation and SIGSTOP affect liveness. The next generation
and completion gate preserve fencing after expiration. Native SQLite full billing
schema startup is tracked separately in IMP30; the focused native lease fixture
is not that startup test. Evidence and exact source identities are under
`improvements-roadmap/results/IMP-18/`.
