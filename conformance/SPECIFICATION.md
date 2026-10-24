# Okkama Conformance Suite — Specification (suite format `ocs-case/1`, protocol `ocs-adapter/1`)

This document is normative for the suite itself: how cases are written, how an
implementation is driven, how observations are judged and what a conformance
claim means. It does not define the Okkama language; the language contract is
`docs/okkama-spec-11072026.md` together with the authorities recorded in
`contracts/` and `contracts/DISCREPANCY-LEDGER.md`.

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119.

## 1. Terms

* **Case** — a directory under `cases/<level>/` containing `case.json` and the
  Okkama sources it lists. A case is one executable claim about observable
  behaviour.
* **Contract** — an identified, normative behaviour from the language or
  standard-library sources (`contracts/*.json`). Each contract has four
  coverage **aspects**: `normal`, `boundary`, `invalid`, `interaction`.
* **Observation** — what the runner sees: exit status, exact stdout/stderr
  bytes, declared artifacts, and the adapter's report of stage and logical
  failure category.
* **Implementation** — any engine (interpreter, compiler, VM, AOT, JIT) driven
  through an adapter.
* **Adapter** — an executable that translates the suite's launch/result
  protocol to one implementation. It reports observations, never verdicts.

## 2. What "100 % conformant" means (release gate)

An implementation **passes the full release gate of a suite release** only
when all of the following hold. `ocs gate` evaluates them mechanically.

1. Every case with `"status": "mandatory"` selected by the release gate
   profile (`profiles/full-release.json`, the union of its member profiles)
   has status **PASS**.
2. Every member profile was exercised in its declared environment (a run of
   `full-release` exercises all members).
3. No required case is UNSUPPORTED, NOT_RUN, BLOCKED, TIMEOUT, CRASH,
   HARNESS_ERROR or FAIL. Expected-failure or exclusion lists may be
   reported for development, but they never turn a non-PASS into a pass.
4. All results come from the same suite tree hash as the gate evaluation:
   expectations cannot have been changed for that implementation.
5. Coverage obligations of the release are complete: no aspect gap, no
   mandatory contract without cases, no orphan or duplicate mapping, no
   contract inventory error.
6. No contract is `unresolved`: an open normative question blocks the release
   even if no mandatory case depends on it.

A claim MUST name: suite version and tree hash; language contract document and
date; standard-library root and tree hash; the gate profile and its members;
platform/environment; implementation name, revision and configuration
(`ocs gate --out` writes exactly these fields).

A **milestone** (`bootstrap`, `frontend`) or a single **release-member**
profile can pass on its own; its report says "not a release claim" and MUST
NOT be summarised as language conformance. "100 % conformant to this suite
release" is a release criterion, not a proof that every possible program is
correct.

## 3. Versioning

* `VERSION` / `release.json` — suite version (semantic versioning), language
  contract document and snapshot date, default std root.
* Every run records the **suite tree hash** (SHA-256 over every file except
  `reports/`, `coverage/`, adapter build caches such as `.target/` and `.DS_Store` files), the std tree hash, the implementation
  revision reported by the adapter, the profile file hash and the timeout scale.
* Case IDs are stable: a case may be fixed, but its ID is never reused for a
  different claim. Removing a mandatory case requires a ledger or contract
  change that explains why.

## 4. Levels, profiles and environment

Levels (L00–L13) order cases by complexity and diagnostic value; **profiles**
select cases by environment. They are independent dimensions.

| Level | Content |
|---|---|
| L00 | bootstrap: entry point, exit status, source shape, basic stdout/stderr — no JSON, files, harness or embedded tests |
| L01 | lexical structure and grammar acceptance/rejection |
| L02 | values and operators |
| L03 | control flow and calls |
| L04 | objects, roles, caps, successors (and the harness self-tests) |
| L05 | types, generics, collections |
| L06 | modules and native boundaries |
| L07 | errors, cleanup, lifetimes, embedded test runner |
| L08 | async and concurrency |
| L09 | pure standard library |
| L10 | host resources (files, environment, argv, stdin, processes, clocks, randomness) |
| L11 | protocols and persistence (loopback TCP, SQLite) |
| L12 | cross-feature adversarial regressions |
| L13 | bounded application scenarios |

Environment tags (`requires.environment`) declare host requirements:
`stdout stderr stdin args env-vars filesystem process-spawn worker-process
real-clock real-random loopback-network tls sqlite unicode-data locale-data
timezone-data`. A selected case whose tag is unavailable is **BLOCKED**, never
skipped. `ocs validate` fails if a mandatory case is selected by no release
member profile, so a mis-tagged case cannot silently leave the gate.

Profiles (`profiles/*.json`) are fixed and reviewable: `bootstrap` and
`frontend` (milestones), `portable-core`, `portable-stdlib`,
`platform-resources`, `controlled-services` (release members), `full-release`
(the gate), and `informative` (unresolved/informative cases only, never
counted).

## 5. Case format

`schema/case.schema.json` documents the format; `runner/ocs/model.py` is the
validator the runner executes. Required keys: `schema`, `id`, `title`,
`purpose`, `level`, `contracts` (contract id + aspects), `mode`, `sources`,
`entry`, `expect`, `spec_refs`, `budget.timeout_s`, `provenance.origin`,
`provenance.expected_basis`, `status`. Optional: `support`, `args`, `stdin`,
`env`, `requires`, `harness`, `notes`, `defects`.

* `sources` lists the module files passed to the implementation (several
  files of one module form a multi-file module); `support` lists imported or
  data files. Every `.ok` file below `cases/` MUST belong to exactly one case
  as a source or support file; discovery fails otherwise.
* `status`: `mandatory` (counts toward the gate), `informative`, or
  `unresolved` (candidate expectation for an open ledger entry; `notes` MUST
  cite the entry).
* `provenance.expected_basis` MUST say how the expectation was justified
  (specification text, exact calculation, published vector, controlled
  fixture). Golden output is never regenerated from an implementation.

## 6. Modes, stages and logical categories

Modes: `parse` (syntax acceptance of the listed sources), `check` (parse +
static analysis incl. imports; never runs program code), `run` (production
execution of the entry point), `test` (the language's embedded test runner,
`-t`).

Stages at which a rejection or failure is reported: `parse`, `check`,
`runtime`. A rejection happens before execution (`parse`/`check`); a runtime
failure after execution started.

Logical categories: `syntax`, `semantic`, `resolution`, `runtime`
(uncatchable programming error), `panic`, `uncaught-error` (a typed Error
escaped the entry point; its concrete type name is compared when the case
states `error_type`). An implementation-internal crash is **never** a logical
category: it is reported as `internal-error` and judged CRASH.

Stage expectations come from the language contract. When the contract does not
fix the detection stage, the case lists every permitted stage/category and
MUST justify it (`justification`). Value-dependent failures that the
specification assigns to runtime are expected at runtime, with the effects that
precede them asserted (e.g. `before` printed first).

## 7. Comparison rules

Comparisons are exact by default.

* **stdout/stderr** are compared as raw bytes. Text expectations are UTF-8
  encoded before comparison; `bytes_b64` and `file` give exact bytes. There is
  no trimming, case folding, line-ending normalisation or Unicode
  normalisation.
* **stderr** of a successful case MUST be exactly the expected bytes (default
  empty). For reject/failure/error outcomes stderr is ignored by default,
  because diagnostic prose, stack traces and paths are not normative; stdout is
  still compared exactly (default empty), which observes effects performed
  before a failure and proves that rejected programs did not run.
* **test mode** stdout MAY be `"ignore"` with a justification, because the
  embedded runner's report format is not normative; the exit status is.
* **Numbers** are compared as program output text produced by the program
  itself; there are no numeric tolerances. Large integers and decimals are
  printed exactly or compared by value inside the program (e.g. coefficient
  and exponent), never through a rounding transport.
* **Collections**: programs print elements in an order that the contract
  defines (array order, sorted map keys). The runner never sorts output.
* **Nondeterminism**: when the contract permits several schedules,
  `stdout.partial_order` lists the exact multiset of lines and the required
  `before` pairs, with a justification. No other normalisation exists.
* **Artifacts**: declared files are read from the case working directory after
  the run and compared exactly; a missing artifact fails.

## 8. Statuses

| Status | Meaning |
|---|---|
| PASS | the observation matches the expectation exactly |
| FAIL | the implementation produced a valid but non-conforming observation |
| UNSUPPORTED | the adapter declares the mode unavailable for this implementation |
| NOT_RUN | selected by the profile but not executed (filters, interruption) |
| BLOCKED | a required environment tag or fixture is unavailable |
| TIMEOUT | no observation within `budget.timeout_s × timeout_scale` |
| CRASH | the adapter process died by signal, or the implementation reported an internal error |
| HARNESS_ERROR | malformed/missing adapter response, protocol violation (e.g. a verdict-like key, mismatched exit status), or a runner exception |

A timeout or crash MUST NOT satisfy an expected language error. An expected
rejection passes only when the specified rejection (stage and category)
actually occurs.

## 9. Adapters and the trust boundary

The runner launches `command --request R --response P` with the case's
working directory and stdin, captures stdout/stderr itself and reads the
response file. The request contains source paths, mode, argv, working
directory, std root and timeout; it deliberately contains **no case id and no
expectation**. The response contains observations only. See
`adapters/PROTOCOL.md`.

Adapters MAY translate launch and result conventions (argv encoding, exit
status, classification of the implementation's own failure carriers into
logical categories). They MUST NOT rewrite or wrap sources, repair invalid
entry points, implement missing behaviour, consult expected results, or report
PASS. A finite self-test cannot prove an adapter honest; the runner therefore
keeps raw evidence (command, request, response, stdout, stderr, process exit,
source hashes) for every case in `results.jsonl`, and adapter source is part of
the hashed suite tree.

Changing implementations means choosing another adapter configuration
(`adapters/*/adapter.json`) — cases and expectations are never edited.

## 10. Isolation and environment

* Each case runs in a fresh process in a fresh temporary directory
  (`<tmp>/work`); case sources are copied there (symlinks preserved) and the
  originals' hashes are verified unchanged afterwards. Harness modules are
  copied to `work/conformance/` only for cases with `"harness": true`.
* Working directory: `work`. Imports resolve relative to the importing file and
  then the std root (spec 5.2).
* Environment: only `LC_ALL=C`, `LANG=C`, `TZ=UTC`, `HOME=<tmp>/home`,
  `TMPDIR=<tmp>/tmp`, the case's `env`, plus variables an adapter
  configuration explicitly lists in `pass_env` to launch the implementation.
* Standard library: `release.json` default (`../std`), overridable with
  `OCS_STDLIB_ROOT` or `--stdlib-root`; passed to adapters as `stdlib_root`.
* stdin: the case's `stdin` file, otherwise `/dev/null`.
* Network: only loopback fixtures owned by the run; ordinary runs never need
  internet access.
* Timeouts: `budget.timeout_s × --timeout-scale` (recorded); the process group
  is killed on timeout and after every run.
* Cleanup: temporary directories are deleted except for non-passing cases
  (`--keep failed`, default), whose evidence is retained.

## 11. Fixtures

`fixtures/<name>/fixture.json` declares either `files` (copied into the working
directory, never overwriting case files) or `service` (a helper started before
the case; it prints `READY key=value ...`; values are substituted into `args`
and `env` as `{fixture.<name>.<key>}`; it is terminated afterwards and its log
retained). Services bind loopback addresses on OS-assigned ports and own all
resources they create. A fixture that fails to start BLOCKS the case.

## 12. Coverage obligations

`contracts/*.json` (schema `ocs-contracts/1`) inventory contracts with status
`mandatory`, `unresolved`, `deferred` or `informative` and a requirement for
each aspect (`required` or `n/a: <justification>`). Standard-library contracts
are generated per module from the audited inventory (`tools/inventory_std.py`);
importing a module never counts as coverage. `ocs coverage` writes
`coverage/matrix.json` (contract → cases → observed results) and
`coverage/COVERAGE.md`, and reports missing aspects, uncovered mandatory
contracts, orphan and duplicate mappings, unresolved contracts and deferred
items. Large file counts are never presented as completeness.

## 13. Differential testing and oracle discipline

`ocs diff` classifies each case as both-conform, one-conforms,
both-violate, contract-unresolved or execution-unavailable. Agreement between
engines is not evidence of correctness: every row is judged against the shared
expectation. A discovered implementation defect keeps its normative case, gets
a record in `defects/`, and is referenced from the case's `defects` list; the
expectation is never weakened.

## 14. Verifying the suite itself

* `runner-tests/test_runner.py` drives the real runner against a deliberately
  misbehaving adapter (wrong output, extra whitespace, CRLF, unexpected stderr,
  wrong exit, invalid program executed, wrong category, wrong stage, wrong
  uncaught type, timeout, crash, internal error, malformed/missing response,
  verdict-like key, protocol mismatch, lying exit status, unsupported mode,
  missing fixture, unavailable environment, omitted execution, missing async
  completion, artifact corruption/absence, partial-order violations, duplicate
  ids, undiscovered sources, unlisted support files, invalid expected data,
  orphan contract mappings, unselected mandatory cases, gate logic). Each
  failure mode has a positive control.
* `runner-tests/mutations.json` + `ocs mutate` apply source and expectation
  mutations to real cases; every mutant MUST be reported as non-PASS.
* The harness module's own failure paths are cases (`L04.harness.*`).

## 15. Performance and stress

Semantic cases use generous budgets and the timeout scale; no mandatory case
asserts speed. Benchmarks or implementation stress limits, if added, belong in
a profile of kind `benchmark` and never in a release member. Mandatory limits
come only from explicit language/library resource contracts (e.g. documented
std limits), never from a host stack size or a temporary bridge.
