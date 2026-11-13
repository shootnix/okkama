# Pointer-free roots (#535): implementation and validation handoff

Scope: GH-01 only, tracker #534. The owner assigns Docker gates, benchmarking,
review and merge to Claude. No commits, Docker, full suites, binaries or
benchmarks were run on the host by the implementer. Performance acceptance
remains pending; this document does not close #535 or the tracker.

## Baseline and source

- Worktree: `feature/535-pointer-free-roots`, starting at
  `4a781717cff9f658fa43a06b615821301e4dcde0` (Darwin arm64).
- #522 is present: `9c482994` is an ancestor. Rebuild this baseline and the
  reviewed candidate; record the actual candidate revision/source hash after
  review. The starting revision is not the final implementation revision.
- Reviewed `docs/LLM-GUIDE.md`, #534, #433 and the external source
  `/Volumes/SamsungSSD/stuff/okkama-docs/general-hardening.md`, SHA-256
  `2f18263103fc26281316acb826210486b2788f59d6674082640d1c1e9d0f1139`.
- The source's 31.0/19.9 ms arithmetic and 15.4/18.7 ms roles timings are
  historical, pre-#522 measurements. They are neither candidate results nor
  a guaranteed gain or upper bound. The roles explanation is a hypothesis.

## Proof and integration

`sparrow-core/src/pointer_free.rs` analyzes verified SPB, once per function
when `Loaded` builds function information, before layouts or JIT declarations.
It is a whole-register proof, not a dominating-definition or liveness proof:

1. A small integer must parse as `i64` and pass `word::small`, which tests
   `SMALL_MIN..=SMALL_MAX`. `Heap::big` uses that same conversion. Its low bit
   is one, so `Repr::Int.pointer` returns `None`. Larger/unsupported literals
   remain unknown; no arbitrary-precision parse is needed for the proof.
2. Raw bool/byte/char/float constants are independently pointer-free by kind.
   `undef` writes zero for Int/Ref (`ops.rs`), which neither representation
   traces, and an unbound raw word for raw kinds. Initial storage is unbound
   too, but an otherwise undefined register does not acquire a proof.
3. All ordinary destinations, all `Access::Target` parallel moves and every
   unwind handler are visited, including unreachable code. Any unsupported
   definition vetoes the proof. Call/await/lookup/catch results are unknown.
   Parameter registers are unknown even if overwritten by a constant;
   closure captures are the same parameter prefix. Native/intrinsic bodies
   conservatively receive no proofs.
4. Moves form dependency edges. A worklist proves a destination only when
   **all** source dependencies have been proven and no definition is unknown.
   Duplicated edges are counted individually. Cycles cannot bootstrap facts;
   even cycles with a constant definition stay traced in this initial version.
   The census is linear in registers and SPB operands; worklist propagation
   is O(registers + move operands). Auxiliary storage is O(registers + move
   operands), bounded by a conservative 64 MiB preflight estimate; beyond
   that it returns no proofs. The persistent result is one boolean per
   register. Scanning integer text is additionally linear in its length.
5. Async frames start unbound, except for unknown parameters. JIT suspension
   saves each register into its own slot and resume reloads that same slot.
   This preserves the invariant inductively. Await and settle-unwind results
   are already unknown definitions. Drivers are excluded. Frame fixed words
   (result, failure, waiter, awaiting) and driver state retain their layouts.

The cached proof is used by `Emitter::body` for both frontend stack maps and
explicit root-word allocation, and by `Loaded::layouts` for async register
slots. Kinds/representations used for execution are unchanged. Frame size,
save/reload operations, closures, ordinary liveness and written-register
reporting, allocation safepoints, and OIR passes are unchanged. BigInt and text
literals remain rooted even though JIT constants have separate lifetime roots.

`SPARROW_CORE_POINTER_FREE=0` restores the previous tracing policy, including
async layouts. Unset (or any value other than `0`) enables it. Read once per
loaded program: do not change a live runtime's proof/layout contract.

## Focused evidence

Tests were added before the analysis implementation. With an all-false stub,
all six initial proof tests failed at their expected assertions; after the
implementation they passed. Further fixtures cover transitive moves, unbound
references, map/layout reduction, public loading and failure cleanup.

- Proof fixtures: small limits and adjacent values, large/text constants,
  multiple definitions and later pointers, overwritten parameters/captures,
  known/unknown joins, parallel swaps, loops, unseeded and seeded cycles,
  undefined storage, handler moves/failure words and lookup edge values.
- Differential in-process execution: off/on; sync call/closure/edge/unwind
  paths; ready and genuinely suspended Event waits; async explicit roots and
  forced frontend maps. Dynamically allocated BigInts replace small constants
  and remain live across calls and suspension. Expected return is 42; failures
  retain their messages. The reused #580 fixture checks result identities and
  exact `BA` cleanup order. A separate sync/async unwind fixture checks `A`
  destruction on failure. Assertions compare to these oracles, not just to
  the other configuration. Unit tests do not mutate process environment.
- Structural check: a traced BigInt-capable register stays `Repr::Int`, a
  small constant becomes `Repr::Raw` only in the tracing layout, captured
  parameters remain traced, and compiled stack-map words decrease off/on.

Allowed host commands, all from `okkama-rs/`:

```sh
cargo check --offline -j 4 -p sparrow-core -p okkama-sparrow -p okkama-compiler
cargo test --offline -j 4 -p sparrow-core --lib pointer_free
SPARROW_CORE_GC_STRESS=1 cargo test --offline -j 4 -p sparrow-core --lib pointer_free
cargo test --offline -j 4 -p okkama-sparrow --lib liveness::tests
cargo clippy --offline -j 4 -p sparrow-core -p okkama-sparrow --all-targets -- -D warnings
```

The public-loader file is run with `SPARROW_CORE_POINTER_FREE=0` and `=1`,
each with GC stress unset and `SPARROW_CORE_GC_STRESS=1`:

```sh
cargo test --offline -j 4 -p sparrow-core --test pointer_free pointer_free
```

Final local results (2026-11-14, Darwin arm64):

| Check | Result |
|---|---|
| Three-package cargo check | PASS |
| `--lib pointer_free`, ordinary / GC stress | 12 / 12 PASS |
| Public-loader off/on x ordinary/GC stress | 2 PASS in each of four configurations |
| `okkama-sparrow --lib liveness::tests` | 9 PASS |
| Two-package all-target clippy, warnings denied | PASS |
| `git diff --check` | PASS |
| Docker gate / benchmark timing | NOT_RUN, assigned to Claude |

The diagnostic command below also passed (one structural test). Its dump is
temporary at `/private/tmp/okkama-535-root-maps.log`, reproducible with:

```sh
SPARROW_CORE_DUMP=main cargo test --offline -j 4 -p sparrow-core --lib stack_maps_and_suspended_layouts -- --nocapture
```

| Tiny sync fixture, analysis off/on | Off | On |
|---|---:|---:|
| Traced registers | 6 | 3 |
| Sum of stack-map words | 37 | 10 |
| JIT code bytes | 2456 | 2412 |
| Machine frame bytes | 64 | 48 |
| Uninstrumented execution timing | NOT_RUN | NOT_RUN |

These are structural counts for the test fixture on the host target, **not**
ladder/micro performance evidence. The async test also asserts fewer actual
stack-map words in each root mode. The auxiliary-memory fallback is tested to
return all-false proofs. No target-specific `Lcg.run` evidence is claimed.

## Claude: required Docker validation and measurements

1. Use the current guide's resource-limited Docker gate, isolated from release
   outputs. Run the complete current gate matrix, including conformance,
   Sparrow GC stress and all `t/` configurations. Repeat focused tests with
   `SPARROW_CORE_PLAIN=*` and async `SPARROW_CORE_FRONTEND_MAPS=*` as applicable.
   Review SPB typing, all writes to frame register slots, and every implicit
   definition before merging. Cover AArch64 and x86-64 supported targets.
2. Rebaseline ladder **01–11** and current micro schema-2 programs. Compare
   the rebuilt starting revision against the candidate at defaults, then use
   the same candidate for the 2x2 matrix below. Keep hoist on by default.

   | Analysis | Hoist | Configuration |
   |---|---|---|
   | off | on | `SPARROW_CORE_POINTER_FREE=0`; default OIR passes |
   | on | on | `SPARROW_CORE_POINTER_FREE=1`; default OIR passes |
   | off | off | `=0`; `OKKAMA_IR_PASSES=regions,tail-calls,fold,undef,jumps` |
   | on | off | `=1`; same explicit OIR pass list |

   Hoist selection must affect **SPB construction**, not only execution of a
   previously built SPB. Verify the runner forwards environment settings into
   the container; record them explicitly in immutable artifacts.
3. Follow #433's bounded contract and external `benchmarks/micro/README.md`.
   At this revision `tools/profile/ab.py` still uses unbounded
   `subprocess.run(..., capture_output=True)` without a timeout; do not run it
   as the safety boundary. Use current schema-2 tooling or an independently
   bounded wrapper (wall/process/output/memory/CPU/PID/FD limits, no network).
   Keep independent expected-output oracles, binary/source/SPB/std/image
   hashes, CPU/OS, flags/toolchain, N, warmup, seed/order, raw repeated samples,
   and variability. Preserve historical and invalid results unchanged.
4. Separate cold/startup from warmed batches; exercise small programs and
   W/2W/4W sensitivity. Run no gate/build concurrently with timings. Separate
   uninstrumented timings from stats/profiling; do not subtract JIT time.
   INCONCLUSIVE/INVALID/too-short/noisy samples cannot establish a gain.
5. Record root counts, relevant loads/stores, code/frame bytes, compile time,
   **load-time proof cost**, peak memory and GC/allocation counters as well as
   time. New `--stats` keys per compiled function are `roots SYMBOL, traced
   registers`, `omitted registers`, and `stack-map words`. The latter is the
   **sum across safepoints**, not unique frame slots. `SPARROW_CORE_DUMP` and
   `SPARROW_CORE_PROFILE_COMPILE` print these as `; roots` lines. Profiling
   additionally prints `; pointer-free analysis` times at load time; ordinary
   `compile SYMBOL, microseconds` excludes that work. Keep diagnostics out of
   timed samples and account for upfront work on functions never compiled.
6. Inspect `Lcg.run`'s specific `% 2147483648` and `% 1000` sites in CLIF and
   machine code on each supported target. Establish that divisor constants
   survive across collecting calls and observe target-specific lowering.
   A whole-function `sdiv` count alone is not an acceptance criterion. Inspect
   roles too before attributing its historical result to root spills.
7. Report measured benefit and small-input regressions. If results are within
   noise, say so. Do not use the historical 36% to justify acceptance.

## Acceptance tracker

- [x] Conservative proof and common JIT/frame integration implemented.
- [x] Unit/IR fixtures and differential sync/async failure/cleanup tests added.
- [x] Final allowed host verification recorded above.
- [ ] Refreshed ladder 01–11 and schema-2 micro A/B, including hoist matrix.
- [ ] Target-specific constant-divisor CLIF/machine-code evidence.
- [ ] Full Docker gate, GC-stress conformance and `t/` coverage.
- [ ] Measured benefit/regressions reviewed; Claude review and merge.
