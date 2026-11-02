# SQL/DB: Okkama versus Python sqlite3

Run from the repository root, with a release compiler containing `for await`:

```sh
python3 benchmarks/db/run.py --runs 5 --rows 5000 --lookups 500 --out /tmp/db-results.json
```

Use a specific Python executable to launch the runner to benchmark that interpreter.
The runner and reference workload use the same interpreter. No external Python
packages, network, Docker or persistent database files are required.

Each implementation creates its own SQLite in-memory database from identical SQL,
with identical text, integers and NULL distribution. Rows are ordered by primary
key. The checksum uses id, name length and nickname length, substituting `guest`
for NULL only; empty strings stay empty. Expected sums are calculated separately
by the runner. Each phase is warmed once inside each process, then measured with
a monotonic clock. Process startup/compilation are outside phase timings; overall
process durations are recorded separately. All processes run sequentially and
engine order rotates between samples. Medians/min/max retain the raw samples.

Cases:

- `projection_only`: repeatedly read one already fetched row; no query in the timer.
- `scan_raw`: fetch every row and count its fields, without typed column reads.
- `scan_stream`: fetch and consume every row through the normal API.
- `scan_all`: fetch all rows, then consume them in order; Okkama's limit equals row count.
- `lookup_prepared`: repeated point lookup, reused preparation/cursor, exactly-one check.
- `lookup_statement`: repeated point lookup with a new Statement/cursor, exactly-one check.
- `insert_transaction`: delete the destination rows, insert individually in one
  explicit transaction using reused preparation/cursor, commit, and verify the sum.

Comparison boundaries:

- Okkama uses its asynchronous SQLite worker, immutable Row snapshots, typed reads,
  parameter validation, and the public Query API. Python uses synchronous sqlite3
  with sqlite3.Row and idiomatic accumulation. These are API/workload timings,
  not an isolated language/JIT or SQLite-engine comparison.
- Python sqlite3 caches SQL statements by default. Reusing a Python cursor is not
  an explicit server-side prepared statement API. `lookup_statement` retains the
  normal Python cache; its label does not claim uncached SQL compilation.
- The fixed phase order and one warmup do not establish JVM steady state. Compare
  dispersion; do not subtract independently timed cases as an exact cost breakdown.
- In-memory SQLite excludes disk/fsync and network/server latency. PostgreSQL is
  not measured here. RSS, CPU profiles and allocation counts are not measured.
- The exact interpreter, compiler hash, DB stdlib hashes and benchmark source
  hashes are recorded in each result. The initial exploratory `baseline.json`
  predates the two diagnostic phases; use `diagnostic.json` as the main comparison.

See [REPORT.md](REPORT.md) for the measured results and next steps.

To compare a saved stdlib snapshot with the current stdlib and fresh Python
measurements (five pairs, four warmups per phase):

```sh
python3 benchmarks/db/compare-throughput.py --before-stdlib /path/to/saved/std --out /tmp/db-throughput.json
```

The workload accepts `DB_BENCH_WARMUPS` (default 1); the paired runner sets it
explicitly for both languages. Four warmups reduce JIT warmup effects but do not
prove every helper reached steady state. The paired runner records full stdlib
hashes and rotates before/after order; do not run builds/tests alongside it.


Native DB migration can be compared with the previous stdlib using one current
release compiler for both versions:

```sh
python3 benchmarks/db/compare-readers.py --before-ref a24ec302192496311a3ea68bc5f5b6938226762b --files read row value sqlite postgres --out /tmp/db-native-paired.json
```

This restores only those DB modules in the before snapshot. The compiler and
native providers are shared by both versions, so this isolates the stdlib/native
handoff rather than comparing two historical compiler binaries. All other stdlib
files remain identical; JSON results record both source snapshots.


For a runtime-only change, save the previous release executable before rebuilding
and compare binaries with an identical stdlib snapshot:

```sh
python3 benchmarks/db/compare-readers.py --before-ref HEAD --files --before-compiler /tmp/okkama-before-async-delivery --engines ir,sparrow --out /tmp/db-async-paired.json
```

`--files` with no names restores nothing. `--before-ref` is unused in this mode.
The isolated scheduler benchmark excludes SQLite and stdlib object construction:

```sh
cd okkama-rs
cargo run --release -p okkama-runtime-core --example async_delivery -- 500
```

It compares completed futures, cold futures, a new offload thread, a persistent
worker with a new relay thread, and a persistent worker delivering directly to
the event loop. Each case warms once; five samples rotate case order.


Layer isolation with identical `SELECT 7 AS n` and 500 repetitions:

```sh
python3 benchmarks/db/run-layers.py --runs 5 --out /tmp/db-async-layers.json
```

This uses four warmups per phase: Sparrow's default hot threshold is 1 000, so
one warmup of 500 calls can leave compilation inside the timed sample. The
historical main DB runs retain their original one-warmup methodology; they
should not be interpreted as guaranteed steady-state JIT measurements. The layer
benchmark also checks exactly-one cardinality at every level, but the native
level bypasses the driver's public object/lifecycle abstraction, so differences
are not a CPU profile attributing all cost to a single function.

To isolate SQLite worker delivery, build a separate diagnostic executable:

```sh
cargo build --release -p okkama-cli -p okkama-runtime-core --features okkama-runtime-core/sqlite-inline-bench --manifest-path okkama-rs/Cargo.toml --target-dir /tmp/okkama-inline-build
python3 benchmarks/db/compare-inline.py --compiler /tmp/okkama-inline-build/release/okkama --out /tmp/inline-vs-worker.json
python3 benchmarks/db/python-cache-probe.py
```

The feature is off by default. In that build only,
`OKKAMA_SQLITE_INLINE_BENCH=1` runs the same SQLite engine on the program thread
and returns completed native Futures. The runner probes both modes before
benchmarking, uses the same executable and stdlib, and checks all checksums.
Okkama async wrappers still execute. This is a diagnostic of worker delivery,
not a completely synchronous public API. Blocking SQL prevents concurrent event
loop progress and cancellation during the call; do not use this mode to assess
production cancellation or concurrent query behavior. JVM is unchanged.

`DB_BENCH_SQLITE_CACHE` optionally sets Python's cached_statements argument;
without it, Python's interpreter default remains unchanged. **A value of 0 is
not sufficient to disable caching on CPython 3.9.6**: its implementation clamps
cache capacity to five entries. The separate cache probe counts preparation via
SQLITE_SELECT authorizer callbacks, then measures with callbacks removed.
Prebuilt unique SQL comments force cache misses without changing query results;
this control includes cache-miss bookkeeping, not just SQLite preparation time.
