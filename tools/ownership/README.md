# Ownership oracle (#562)

Use this oracle at every step of the 0.6.0 ownership rebuild. Python 3.8+
standard library only; the binaries come from an installed toolchain with
`bin/okkama`, `bin/sparrow`, `lib/okkama/std` and the native library.

```sh
python3 tools/ownership/oracle.py MODE --toolchain /tc \
  [--root /work] [--corpus ownership --corpus std] [--pending] \
  [--bench /bench] [--program-arg 10] \
  [--a-env NAME=VALUE] [--b-env NAME=VALUE] \
  [--jobs 4] [--timeout 60] [--build-timeout 120] \
  [--out /out/artifacts] [--json /out/report.json]
```

`MODE` is one of:

- `engines`: build SPB, run Sparrow from source and SPB, compare stdout bytes, exit code
  and destroy-trace bytes. Test traces are compared as ordered blocks starting
  with `# test <name>`; both engines restart creation serials for each test.
  Empty tests retain their separator. No trace sorting or serial normalization.
- `ab`: build independently with environments A and B; execute A, A again,
  then B on Sparrow. Classify each corpus as `same`, `DIFF`,
  `nondeterministic`, `build-fail` (plus explicit infrastructure failures).
  Both determinism and A/B differences are retained, even if A is unstable.
- `stats`: the same A/A/B protocol with `SPARROW_CORE_STATS=1`; sum
  `helper region_*`, `helper created`, `region.enter, count` and all numeric
  `OKKAMA_OWNERSHIP_REPORT.totals` fields (including reasons and static/dynamic
  region operations), per corpus and globally for A/B. A2 is excluded from
  totals. Only matched pairs without instrumentation/infrastructure errors
  or nondeterminism contribute. Missing reports are explicit errors. Within a
  report, omitted zero-call helpers count as zero (Sparrow omits them); the
  unconditional heap counter proves the report exists. Shared imported functions count once per
  compiled program, as in the census totals; there is no symbol deduplication.
- `tap`: run Sparrow from source and SPB, compare their outputs/traces, and count TAP
  `ok`/`not ok` per corpus and engine. Check plan/count/order, list failing
  assertions with preceding `#` diagnostics and following diagnostics/YAML.
  SKIP/TODO counts remain visible; those assertions do not fail the run.
  Non-TAP programs are executed but excluded from TAP totals. Ownership and
  t/ programs require TAP; nonzero exits fail except the two intentional
  ownership runtime failures below.

Discovery defaults to `ownership`, `conformance`, `std`, `t`, plus `bench`
when supplied. Repeat `--corpus` to restrict it. Ownership main `*.ok` files
run normally; `--pending` adds a separate pending corpus. `rejected/**/*.ok`
are always compile-only when ownership is selected: every selected build
variant must exit 1 (not timeout, signal, launch/I/O/usage failure).
`panic-unwind.ok` and `secondary-failures.ok` must exit 1 and match the stderr
patterns documented in the corpus README; TAP mode also checks their cleanup
assertions. Other matching nonzero exits in comparison modes remain `same`:
comparison is not an assertion that every source is a successful program.

Conformance selects `cases/**/main.ok` and reads adjacent `case.json` (the repository's current `ocs-case/1` schema). The oracle copies the case tree to an isolated work directory, omitting `case.json` and `expected` as the runner does; declared `support` files are included through that copy. It applies `requires.fixtures` of kind `files` using fixture `path` and `target`, and passes `args`, `stdin`, `env`, `budget.timeout_s`, and the copied directory as cwd. Runtime timeout is capped by `--timeout`. Cases requesting `harness`, service fixtures, or fixture placeholders needing service-provided values are skipped with a reason in `summary.skipped` and per-program `skip_reason`; they are not comparison mismatches.
Std selects modules with a test
declaration using the gate's detection convention and builds/runs with `-t`.
The t/ suite selects all existing `**/*.ok` except `support/`, as ordinary
programs; it does not generate additional sources. Benchmarks select
`**/main.ok` under the supplied directory, or the supplied single file;
`--program-arg` applies only to benchmarks. This is a correctness/counter
oracle, not the alternating steady-state timing benchmark from `ab555.sh`.

Every subprocess gets closed stdin, a wall timeout, and a separate process
group killed at completion/timeout. Default concurrency is four programs.
Build and runtime cwd is the source directory for both engines and every A/A2/B
variant; each command records its cwd in the JSON report. `OKKAMA_IMPORT_DIRS` defaults to
`<root>/t/support`; std comes from the installation. Environment overrides
apply to both build and run, independently for A/B, on top of the inherited
environment. For a clean baseline explicitly set
`--a-env OKKAMA_OWNERSHIP_PROTOTYPE= --b-env OKKAMA_OWNERSHIP_PROTOTYPE=1`.
The oracle owns all four instrumentation variables and removes inherited
values. Other inherited settings are not scrubbed; run in a clean container
for reproducibility. Reports record explicit overrides, not the entire
potentially secret-bearing host environment.

Fresh artifact directories hold SPB, census, stdout, stderr and trace files.
The JSON contains per-program commands, codes, timeouts, artifact paths,
short diffs (40 lines), differing-program lists, TAP details and totals.
Comparisons use complete byte streams; displayed diffs decode as UTF-8 with
replacement. A report is always saved in the artifact directory; `--json`
writes another copy. Container paths in reports map `/out` to the host result
directory. Exit 0 means no detected comparison/TAP/rejection failure; positive
build failures are reported coverage gaps and do not fail the suite. Review
those counts even after exit 0. Exit 1 means a detected failure; exit 2 is
invalid configuration. Host execution is refused unless `--allow-host`.

## Docker and verification

Use the gate's existing `okkama-dev` image and populated
`okkama-cargo-registry` volume (see `tools/gate/docker.sh`). The wrapper does
not build the image or fetch dependencies. It copies the current checkout,
builds a fresh offline release toolchain using the former `micro_toolchain.sh`
logic, installs it in `/tc`, and runs the oracle in the same disposable
container. Only the registry is cached, preventing stale worktree builds.
No Docker socket is mounted. Network is disabled; defaults are 3 CPUs, 6 GiB,
512 PIDs, two Cargo jobs, and a 1800-second toolchain build timeout.
`ORACLE_CPUS`, `ORACLE_MEMORY`, `ORACLE_CARGO_JOBS`, `ORACLE_BUILD_TIMEOUT`
override these defaults. Each wrapper invocation builds a fresh toolchain.

Claude should run these checks (runtime checks were intentionally not run
while implementing #562):

```sh
python3 -m py_compile tools/ownership/oracle.py
python3 tools/ownership/oracle.py --help
bash -n tools/ownership/toolchain.sh tools/ownership/oracle-docker.sh

tools/ownership/oracle-docker.sh engines --corpus ownership
tools/ownership/oracle-docker.sh tap --corpus ownership --pending
# Full corpora, including build-failure coverage and known TAP failures:
tools/ownership/oracle-docker.sh engines
tools/ownership/oracle-docker.sh tap
tools/ownership/oracle-docker.sh ab \
  --a-env OKKAMA_OWNERSHIP_PROTOTYPE= --b-env OKKAMA_OWNERSHIP_PROTOTYPE=1
tools/ownership/oracle-docker.sh stats \
  --a-env OKKAMA_OWNERSHIP_PROTOTYPE= --b-env OKKAMA_OWNERSHIP_PROTOTYPE=1
# Optional external benchmark corpus:
tools/ownership/oracle-docker.sh stats --corpus bench --bench /HOST/PATH/TO/benchmarks \
  --program-arg 10 --b-env OKKAMA_OWNERSHIP_PROTOTYPE=1
```

Results default to `tools/ownership/results/{MODE}.json` plus unique
`artifacts/MODE-*` directories; `--results HOST_DIR` changes the host mount.
`--bench HOST_PATH` is mounted read-only. Additional oracle options pass
through; paths supplied to `--out`/`--json` are container paths (use `/out/…`).

Inspect all six compile-negative fixtures, intentional exit-1 diagnostics,
known pending failures, and trace diffs. Verify A=A yields no deterministic
A/B differences; force a tiny `--timeout` and confirm timeout is never `same`;
force a tiny `--build-timeout` and confirm processing continues with
`build-fail`. Cross-check helper/census totals against saved raw files and
confirm A2 is not counted. Test with `--jobs 1` and `--jobs 4` to confirm
per-program artifact isolation. Host refusal can be checked by invoking
without `--allow-host` outside Docker; do not actually execute corpus programs
on the host. No performance or runtime acceptance claim is made by the static
implementation checks.
