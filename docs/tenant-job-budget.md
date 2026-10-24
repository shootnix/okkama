# Tenant work budget and operational leases

The all-eight candidate uses one SQL budget row per tenant (alpha1, beta2 in the
local fixture). Admission counts unexpired RUNNING payment jobs and unexpired
PENDING/UNKNOWN refunds using storage.lease_clock(), the shared DB-UTC expression.
Billing/business time continues to control due dates/audit, never lease expiry.
Schema creation stays in SqliteBilling.initialize's existing transaction.

Every admission locks the tenant row and keeps it through work-generation CAS and
commit. Heartbeat renewals and refund completion/UNKNOWN extension acquire that
SAME row in the SAME transaction before their work row. Lock order is budget→work.
A count runs in a subsequent READ COMMITTED statement after the lock is acquired,
so it sees an earlier renewal's committed expiry; an old uncommitted renewal
cannot later resurrect capacity behind that admission snapshot. SQLite's existing
immediate transaction serializes writes. Completion that only releases a charge
lease does not need a new extension lock.

This protocol bounds valid SQL leases, not stale physical HTTP calls or unresolved
financial obligations. Heartbeat defaults120s/40s, generation/key fencing and
structured cancellation stay authoritative. It assumes shared DB time does not
move backward across an expiry/admission boundary, normal statement/commit latency
within the configured lease margin, fixed tenant limits during active work and
the declared transaction isolation. It is not a distributed monotonic clock proof.
SQL lock contention can yield a typed known rollback/timeout; there is no automatic
application retry of foreign effects or uncertain commits.

Delivery cancellation is an authorized operator action: it revokes the generation,
marks the original operation conservatively UNKNOWN and releases admission, while
retaining the original key and debt. A stale worker cannot complete it. The CLI
uses the same DB/bridge environment and FixtureAuthentication as refund-reconcile:
```
R15_OPERATOR_TOKEN=r15-alpha-admin perl \
  improvements-roadmap/workspace/billing/experiments/run_app.pl \
  improvements-roadmap/workspace/billing/app/release_lease.ok \
  DATABASE TENANT JOB ORIGINAL_KEY EXPECTED_GENERATION
```
CANCELLED exits0; STALE exits2. Wrong role/tenant/token is rejected before SQL.
Fixture tokens are the named local profile's credential mapping. Refund operator
reconciliation shares admission; inspection's eligible flag describes operation
state/expiry/CAS eligibility, not a reservation of current tenant capacity.
Denied admission changes no refund state/audit and makes no provider lookup.

Correlation emits opaque trace_id/generation/attempt. Computed cap reads perform
best-effort logging each time; caps do not imply purity or log delivery. No raw
operation key/customer/bearer is included. Source effects/module policy prototypes
remain separate from the installed compiler behavior.
