# Billing schema phases and startup

The maintained candidate is improvements-roadmap/workspace/billing/app. Fresh
SQLite/PostgreSQL schemas use structured customer address fields and USD/EUR scale2;
issued invoice text snapshots stay immutable. Initializers are transactional and
idempotent. The current main does not invoke address-expand at startup: it reuses
R20 CUMULATIVE-final-v2's verified policy. It never reconstructs a retired trigger.

| Physical customer schema | Application / allowed operation |
|---|---|
| Legacy billing_address_text only | Retained old version; explicit PG expand required before transitional/new reads |
| Expanded, nullable structured fields + synchronization trigger | Old and E06 transitional readers/writers coexist; bounded validated backfill |
| Expanded and fully backfilled | New structured writer/reader works; explicit validate sets not-null and validates constraint |
| Switched: validated constraint +3 not-null fields, legacy still present | Transitional/new versions; contract may retire old column/trigger |
| Contracted: structured fields, no legacy column/trigger | Current final version; old writer/reader retired; repeated startup/new writes supported |
| Fresh current schema | Same usable structured shape as contracted, without rolling history |

Phase authority is actual column/result metadata and PostgreSQL validated/not-null
constraints, not migration filename/directory or schema_versions1/2/3. Those version
numbers belong to other baseline upgrades. Expand requires the legacy column;
trying it after contract fails55000 before restoring retired machinery. Contract
requires the switched physical schema, then idempotently removes legacy objects.
Backfill after contract returns0. Invalid legacy addresses require explicit repair;
no guessed city/country or silent normalization is performed.

The rolling coexistence profile is PostgreSQL18.6. SQLite fresh/repeated startup
and historical schema1→3 upgrades are verified; no automatic destructive address
upgrade or zero-downtime SQLite rolling protocol is claimed. Choose the declared
old/transitional/final application for its phase. After contract, restoring the
old application alone is insufficient: use an explicit forward repair or an
independently validated backup/restore procedure. Destructive downgrade was not
executed. The disposable negative control recreated the obsolete trigger and made
a new write503; current contract explicitly removed it, and the same command then
returned201 with its failed attempt rolled back.

Current source-pinned evidence: improvements-roadmap/results/IMP-30/RESULT.md.

## E03 invoice expansion when composing all eight features

A legacy CONTROL/B1 database also has an invoice kind CHECK accepting only
PERIOD/UPGRADE and a broad per-period UNIQUE constraint. Before starting the new
SEATS writer, inspect that catalog and execute the existing
app/schema/seat-upgrade-postgres.sql through the current transactional migration
CLI. It expands the CHECK and replaces broad uniqueness with the existing
PERIOD/UPGRADE-only index. Apply this declared legacy transition before the new
initializer, which may otherwise create the partial index without upgrading an
already existing table. CREATE TABLE IF NOT EXISTS is not a constraint migration.

The script is a one-time phase transition, not an unconditional startup hook.
After a committed transition/restart, inspect catalog state rather than replaying
its unguarded DROP/CREATE steps. The supported all-eight sequence is E03 expansion,
address expand/validated backfill, then current writer and later address contract.
The IMP32 first full run retained the missing-E03 failure; its focused control
observes actual23514/complete rollback versus1320 with the existing migration.
No CAS or financial constraint was weakened to convert that failure into success.
