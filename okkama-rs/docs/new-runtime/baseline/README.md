# Frozen Sparrow baseline

Safety note (2026-11-03): the commands below record historical reproduction,
not an approved local run sequence. Full suites, process workers and service
scenarios require externally enforced CPU, memory, process-count and time limits
for the entire isolated environment. Do not copy the historical `-j 4` run onto
the user's host. Follow [the current execution order](../execution-order.md).

Source revision: `1c289f8b3307858470e97489043cb4486eefa8ea`.
The root `sparrow-origin` is the preserved release binary. The additional debug
build below is compiled from an isolated source archive, not the changing worktree.
Toolchain, lockfile and executable hashes are in [build.json](build.json).
Cargo.lock pins the native Rust dependencies as well as the compiler backend.
Bit-identical binaries across machines are not asserted.

From the repository root, with a fresh destination:

```sh
mkdir -p /private/tmp/okkama-nom-origin-1c289f8b
git archive 1c289f8b okkama-rs std benchmarks/db/allocation-counter | tar -xf - -C /private/tmp/okkama-nom-origin-1c289f8b
CARGO_TARGET_DIR="$PWD/okkama-rs/target/nom-origin" cargo build --offline --locked --manifest-path /private/tmp/okkama-nom-origin-1c289f8b/okkama-rs/Cargo.toml -p okkama-cli -p sparrow-cli -p okkama-native
python3 conformance/ocs run --adapter conformance/adapters/rust/adapter.json --profile full-release --adapter-env TMPDIR=/private/tmp --adapter-env OCS_RUST_IMPLEMENTATION_ROOT=/private/tmp/okkama-nom-origin-1c289f8b/okkama-rs --adapter-env OCS_RUST_TARGET_DIR="$PWD/okkama-rs/target/nom-origin" --adapter-env OCS_RUST_ENGINE=sparrow --out "$PWD/okkama-rs/target/nom-baseline/conformance" -j 4
OKKAMA_ICU_PREFIX="$PWD/okkama-rs/target/nom-baseline/icu-76.1" sh tools/build-icu-collation.sh
OKKAMA_ICU_LIBDIR="$PWD/okkama-rs/target/nom-baseline/icu-76.1/lib" python3 okkama-rs/tools/new-runtime/stdlib-baseline.py --binary okkama-rs/target/nom-origin/debug/okkama --out okkama-rs/docs/new-runtime/baseline/stdlib.json
python3 okkama-rs/tools/new-runtime/compile-manifest.py --binary okkama-rs/target/nom-origin/debug/okkama --out okkama-rs/docs/new-runtime/baseline/source-checks.json
OKKAMA_ICU_LIBDIR="$PWD/okkama-rs/target/nom-baseline/icu-76.1/lib" python3 okkama-rs/tools/new-runtime/cookbook-baseline.py --binary okkama-rs/target/nom-origin/debug/okkama --out okkama-rs/docs/new-runtime/baseline/cookbook.json
```

An offline build requires the locked dependencies in the Cargo cache. Conformance
uses the current suite and current stdlib; both tree hashes are recorded in
[conformance-summary.json](conformance-summary.json). The archive has no `.git`,
so the adapter reports a null implementation revision; its source revision is
explicitly recorded above and in build metadata.

Result: **835/835 mandatory cases PASS**, including the controlled-services
profile with loopback networking, SQLite and process workers. Complete per-case
results, diagnostics and environment records: [conformance-results.jsonl.gz](conformance-results.jsonl.gz).
This is a behavior baseline, not a benchmark and not proof that every language
contract has a test. It does not establish PostgreSQL/MySQL live-service coverage
or completion of the examples/source manifest. Those obligations remain open.

The stdlib run passed **239/239 source files with tests**; individual test output
is preserved in [stdlib.json](stdlib.json). The initial run lacked pinned ICU;
the final run uses ICU 76.1 built by the repository script, whose source archive
SHA-256 is checked before compilation. A stale temporary build directory contained
dangling symlinks and was not usable. No system ICU was substituted.

Two PostgreSQL scenarios passed on the existing local `okkama-pg` container:
[postgres.json](postgres.json). They exercise TEMP tables, transactions, prepared
queries, row streaming, constraint errors and pooling; no persistent tables are
created. Credentials were passed in memory and are absent from the report.
There is currently no `std/db/mysql.ok` or MariaDB provider in the source inventory;
the roadmap's requested driver support remains an explicit implementation obligation.

All **1022 non-OCS source files** were checked by the frozen compiler:
1011 accepted, 10 expected semantic rejections from the gallery catalog, and one
historical v0.3.1 benchmark snapshot importing the removed `json/parse` module.
The latter remains recorded as incompatible historical evidence, not changed to
make a current-version test pass. The parser/printer `all-syntax.ok` fixture is
parsed rather than executed because it intentionally references undefined types.
[source-checks.json](source-checks.json) retains each mode and diagnostic.
Compilation success is not runtime success; runtime entries remain NOT_RUN there.

The cookbook harness passed **40 application/protocol checks**, including HTTP,
WebSocket, persistence and transaction behavior. It then stopped because its
`--report=json` test option is unsupported by the frozen Rust CLI; this is retained
as an incomplete harness run in [cookbook.json](cookbook.json). The actual Okkama
test module was separately executed successfully using `run --engine sparrow -t`:
[cookbook-tests.txt](cookbook-tests.txt). This does not turn the failed JSON-output
check or the unexecuted final coverage check into a pass.
