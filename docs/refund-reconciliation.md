# Inspect and reconcile one exhausted refund

The billing workspace includes `app/refund-reconcile.ok`. Run it through the
existing `experiments/run_app.pl`, using the same database/bridge and provider
URLs as its worker. `R15_OPERATOR_TOKEN` is an existing tenant-admin fixture
credential; `R20_PROVIDER_B` supplies vendor B's configured URL for reconciliation.
These are local lab credentials and trusted endpoint configuration.

The two command forms are:

```text
DATABASE PROVIDER_A TENANT REFUND inspect
DATABASE PROVIDER_A TENANT REFUND EXPECTED_GENERATION EXPECTED_STATE reconcile
```

`help` prints these forms without opening a database or requiring credentials.
`inspect` discovers the current state/generation and eligibility, and shows the
original refund and capture keys. It changes nothing and never calls a provider.
It requires tenant admin authority, just like reconciliation. An invalid token,
viewer/support actor, foreign tenant or missing refund cannot authorize a lookup.

Reconcile requires the reviewed generation and state (PENDING or UNKNOWN), at
least10 exhausted generations, and an expired lease according to the DB clock.
It atomically advances the generation, marks uncertainty and records a CLAIMED
audit row before any vendor I/O. The original capture binds the vendor; the
original refund key, amount and capture reference remain unchanged.

The command performs one status lookup. It never sends a charge/refund retry or
uses charge failover. An absent/indeterminate reply leaves the money reserved.
A definitive success moves reserved money to refunded; a definitive decline
releases that reservation. Existing exact RefundBalance transitions and SQL CAS
perform the settlement. The result audit joins the same transaction as the
balance update. Both audits correlate the original key and new generation using
the existing durable tuple encoding.

The existing structured lease supervisor covers the lookup; before settlement
it stops heartbeat, renews the still-valid claim and uses the completion fence.
A process dying at any stage cannot authorize an old generation to settle later.
An uncertain commit remains an error requiring inspection. Repeating the exact
old reconcile command conflicts without a new lookup or balance update. Inspect
again before explicitly requesting another lookup with the resulting generation.

Native tests exercise actual SQLite SQL, authorization, expected-state/generation
checks, UNKNOWN retention, repeated settlement and rollback on audit failure.
The named provider/HTTP integration and exact source pins are recorded under
`improvements-roadmap/results/IMP-19/`; provider ledgers and captured request paths
separately establish lookup-only behavior. Full fresh SQLite billing startup is
owned by IMP30, distinct from the minimal native operations fixture.
