# GH-02 / #536: closure bridge candidate and handoff

## Status and provenance

Phase A is an **unmeasured candidate**, not an adopted optimization. Phase B
is a design proposal, not an implemented or reviewed ABI. #536 stays open.
The owner assigns Docker measurement, full gate, review and merge to Claude.
No host benchmarks, CLI programs, Docker, full suites, formatting or commits were run.

- Source baseline: `923c558011aa1e37aea145e66197328fb01143ec`, branch
  `feature/536-closure-bridge`; initially clean worktree.
- #522's reused-cell accounting fix `9c482994` is an ancestor of this baseline.
  The older report's binary is not a valid comparison baseline.
- Tracker [#534](https://github.com/shootnix/okkama/issues/534),
  [#433](https://github.com/shootnix/okkama/issues/433), current
  `docs/LLM-GUIDE.md` and external `benchmarks/micro/README.md` were consulted.
- External `general-hardening.md` SHA-256 matches the tracker:
  `2f18263103fc26281316acb826210486b2788f59d6674082640d1c1e9d0f1139`.
- The historical 78%, 20–30% and 2–3x figures remain hypotheses. Inclusive
  bridge/copy samples must not be added as independent savings.

## Phase A implementation and safety argument

`Runtime::invocations` replaces the address-only table with a boxed, stable
array of `#[repr(C)] Invocation { code: u64, params: u32, is_async: bool }`.
Its size is 16 bytes on the supported 64-bit targets, versus 8 bytes/function
previously. It supplies one coherent metadata lookup, not one machine load.
The checked u32 conversion refuses an unrepresentable parameter count.
Loaded function signatures are immutable for a runtime's lifetime; generated
worker declarations are added before this table is built.

JIT static, role, cap and async-body dispatch uses `size_of::<Invocation>()`
and `offset_of!(Invocation, code)`. Rust calls and compilation use the same
record. No second address table needs synchronization. Code is published only
after finalization, stack-map registration and code-range registration, and
remains stable for this runtime/JIT module's lifetime. There is no eviction,
recompilation or concurrent publication in this change. The existing async
body entry remains `(ctx, frame)`, never the ordinary parameter signature.

`rt_call_closure` checks the function index, `params <= 8` and
`count <= params` **before** subtraction, argument conversion, capture reads,
slice bounds or invocation. Thus `captured = params - count` is in 0..=8,
and `captured + count = params`. SPB typing independently proves that the
closure captures that prefix and that call.fn supplies exactly the remaining
parameters with matching types. Over-eight-word SPB signatures still load,
but invoking them fails before copying. Invalid direct-helper counts fail
before touching even a null input pointer; valid helper inputs still require
the initialized, non-null spill storage supplied by generated code.

`rt_closure` also checks the function and capture count against the function
layout before constructing a slice or writing the heap object. The helper
declares `Fails`, with a status check after fn.closure. Valid SPB cannot
trigger these defensive failure paths. The capture count is
**not a function-level constant**: one function can be closed over different
prefixes and called through different fn types. The regression fixture
exercises this along with a normal static call to the same target.

The candidate copies arguments with a loop bounded by eight iterations;
the reference branch retains `copy_from_slice` and source-metadata reads.
Both retain `[UNBOUND; 8]`. Every consumed word in `words[..params]` is either
an initialized capture or an initialized argument. The suffix remains
initialized and is never passed to invoke/future_new. No MaybeUninit, no
uninitialized array value, and no uninitialized slice are introduced.
Compilation, language failure handling and cold async construction remain
on their existing paths. Removing initialization awaits measured evidence.

**No values are dropped from root maps.** `Kind::traced`, liveness,
`report_roots`, closure-call effects, explicit async roots and frame walking
are unchanged. Closure creation now declares its defensive failure effect.
In particular `Int` still includes heap BigInts, `Ref` still traces
closures/objects, and pointer-free `Raw` classification is unchanged. The
metadata's code pointer is an executable address owned by JIT, not a GC heap
value; counts/flags are raw metadata. None replaces or hides language roots.
During the Rust bridge, the caller's mapped closure keeps its captures alive
by heap layout and its argument registers remain call inputs. The callee
uses ordinary parameter roots; Rust frames are skipped by the existing
frame walk. Compilation currently allocates constants into permanent roots
without running a collector; any future collecting compilation must preserve
the caller's closure/argument roots. Future construction copies all parameters
into the existing traced frame/retained-argument layout before execution.

The forced reference switch is read once when a runtime is created:
`SPARROW_CORE_CLOSURE_BRIDGE=reference`. It preserves validation and uses the
new table's ordinary address; it is a same-binary bridge/copy control, **not**
an independent control for the table layout/cache cost. Compare separate
baseline/candidate binaries as well. All Phase A synchronous calls still
reach `helper call_closure`; there is no expected helper-count reduction yet.

Diagnostics add `invocation metadata, bytes`, `stack map sites` and
`stack map root words, static sum`. The last is a sum of static root locations
across call sites, not a dynamic live-root count. These are report-time
calculations, not new hot-path counters.

## Focused verification

Tests were written before the bridge change. The initial seven integration
tests passed on the baseline; the new count-validation unit test initially
failed to compile until the checked invocation record was implemented.

- `closure_bridge.rs`: 0/1/2/8 total words with every capture split, first
  and repeated calls; verified nine-word rejection with 0/1/9 captures;
  invalid SPB closure/call counts; dynamically allocated captured and passed
  BigInts/strings consumed after allocations; native trampoline and role
  entry reuse; a function with multiple capture prefixes and a static entry;
  bounded recursive closure reentry; cold async construction without running
  the body; Event suspension/resumption and cancellation; exact failure trace
  identity `(callee, pc)` then `(caller, call.fn pc)`.
- Runtime unit tests: count boundaries including `u64::MAX`; invalid helper
  arguments/captures rejected before null input access; stable metadata
  allocation, ordinary/async publication and repeated compile cache hits;
  first-call compilation refusal leaves code unpublished and retryable.
- `jit.rs`/`liveness.rs` root selection has no semantic edits.

Final verification: **PASS**, macOS/AArch64, debug test profile. All commands
ran from `okkama-rs/`, with `--offline -j 4`; no unfiltered suites were used:

```sh
cargo test --offline -j 4 -p sparrow-core --lib closure_bridge
cargo test --offline -j 4 -p sparrow-core --test closure_bridge closure_bridge
SPARROW_CORE_GC_STRESS=1 cargo test --offline -j 4 -p sparrow-core --test closure_bridge closure_bridge
SPARROW_CORE_CLOSURE_BRIDGE=reference cargo test --offline -j 4 -p sparrow-core --test closure_bridge closure_bridge
SPARROW_CORE_CLOSURE_BRIDGE=reference SPARROW_CORE_GC_STRESS=1 cargo test --offline -j 4 -p sparrow-core --test closure_bridge closure_bridge
cargo check --offline -j 4 -p sparrow-core -p okkama-sparrow -p okkama-compiler
cargo clippy --offline -j 4 -p sparrow-core -p okkama-sparrow --all-targets -- -D warnings
```

Four unit tests and all ten integration tests passed (the latter in each of
four modes); check and clippy passed without warnings. `git diff --check`
passed. These results do not establish Linux/x86-64 or release codegen parity,
a speedup, full-gate success, or Phase B correctness.

## Measurement and acceptance handoff to Claude

| Evidence | Baseline | Candidate | Verdict |
|---|---|---|---|
| 09-closures cold / warmed time and variability | NOT_RUN | NOT_RUN | pending Docker |
| 06-collections / 10-app controls | NOT_RUN | NOT_RUN | pending Docker |
| schema-2 micro A/B and independent checksum oracles | NOT_RUN | NOT_RUN | pending Docker |
| bridge/copy self and inclusive samples | historical only | NOT_RUN | no savings claimed |
| compile time / machine code / frame bytes / roots | NOT_RUN | NOT_RUN | separate stats runs |
| invocation table storage | 8 bytes/function | 16 bytes/function | static layout only |
| full gate / ownership / conformance / t/ | NOT_RUN | NOT_RUN | pending Docker |

1. Freeze baseline and candidate source/stdlib/toolchains and the current
   schema-2 micro suite. Record source/diff, binary, std and input hashes,
   compiler version, image ID, CPU/OS/architecture, build flags/profile,
   host load/power conditions, N, warmup, seed, all repeated samples and spread.
   Use isolated build/result locations separate from release work.
2. Use the current project Docker guide and schema-2 runner's process-tree
   deadlines, bounded output, CPU, RAM/swap, PID/FD limits, no network and
   cleanup. The checked-in legacy `tools/profile/ab.py` still uses unbounded
   `subprocess.run(..., capture_output=True)` and is **not** a safe standalone
   runner. Reuse #433's bounded implementation/current micro tooling; for
   ladder inputs apply equivalent bounds and independent expected outputs.
3. Measure baseline, candidate and candidate+reference. Keep cold launches
   separate from in-process warm batches, test warmup sensitivity, and retain
   small-input controls. Use independent output oracles, not parity alone.
   Do not run benchmarks alongside builds, gates or other campaigns. Reject
   invalid, noisy and too-short samples; preserve immutable result artifacts.
4. Reprofile `09-closures`; attribute self/inclusive bridge, invoke and copy
   work separately. Inspect **optimized Rust** rt_call_closure assembly on
   supported targets for memcpy/memmove, stores and bounds branches. The
   bounded Rust loop is not evidence of eliminated library copies. This
   inspection is NOT_RUN here; debug tests do not establish release codegen.
5. In separate stats/profile runs compare compilation, code/frame bytes,
   metadata bytes, stack-map sites/static roots, allocations, collections and
   RSS. Check GC effects with #522 present. Run the normal/reference paths,
   ownership/closure conformance, t/, GC stress and full gate before adoption.

## Phase B proposal: contract required before a prototype

The naive single alternate entry per function is insufficient: the same
function can have different capture prefixes. A prototype must either use
separate closure entries keyed by `(function, capture_count)` (equivalently
verified explicit arity), or validate a genuinely uniform ABI separately.
Keep `Invocation::code` exclusively ordinary `(ctx, all_params...)` or the
existing async `(ctx, frame)` body address. Proposed closure-entry addresses
belong in a separate stable side table; GH-08 (#542) must never bake one into
an ordinary/static/role call. Do not call either through `jit::invoke` with
an incompatible signature.

For a synchronous supported closure entry the signature is exactly
`(ctx, closure, explicit_arguments...) -> Word`, and its variant loads the
verified capture prefix into ordinary parameter variables before the shared
body. Original total capture+argument limit remains eight; adding the closure
word does not silently reduce the supported explicit arity. Async closures,
missing entries/first compilation, natives/intrinsics and unsupported cases
retain the helper until individually proven/tested. Check status after the
helper or indirect call and preserve its original call.fn trace site.

Required entry/call effects: may allocate, collect, reenter and fail, as the
current call_closure helper declares. The caller must map the closure **and
all pointer-capable arguments** at both the cold compilation helper and the
direct call, including last-use inputs. The closure traces captures until
callee capture loads/parameter roots are established. Every `Int` can still
be a BigInt pointer. No raw Rust buffer can substitute for those roots.
Any compiling path that begins collecting needs an explicit protected-root
scope, with all exit/failure paths restoring it. Suspension/cancellation
continues to use traced Future frames and retained arguments through the
helper; no direct async-body dispatch through this entry is allowed.

Each alternate entry must have the normal prologue, stack/headroom and budget
checks, complete stack maps with its actual frame layout and return addresses,
and function/pc identity shared with the ordinary body. A wrapper frame must
not add a language-visible trace frame or bypass a stack check. Recursive
and mutually recursive first calls must fall back without recursively
compiling an unpublished variant; reentrant calls see only finalized code.
Publish a variant after all maps/ranges/relocations are ready, clear readiness
on compilation failure, and retain code for the runtime/JIT module lifetime.
No concurrent code mutation or eviction is introduced by this proposal.

Before enabling a prototype, review the above contract and record Phase A
measurements. Then add differential ordinary-versus-closure-entry tests for
every arity/capture split, native/role boundaries, dynamic BigInts/references,
recursion, collection during cold compilation, failures/traces, suspended and
cancelled async fallbacks. Existing A fixtures are reusable but do **not**
claim differential B coverage. Update `okkama-cli/tests/helpers.rs` with
separate synchronous warm and cold/async fixtures: hot synchronous helper
calls should disappear, intentional async/cold fallback calls must remain.
Do not equate all helper calls with first calls. Gate and measure B separately;
record a rejected experiment explicitly if it fails the evidence/ABI contract.

## Execution tracker

- [x] Read source/guide/tracker; verify #522 ancestry and report hash.
- [x] Write Phase A behavior/edge tests first and record baseline outcome.
- [x] Implement shared metadata, checked counts, bounded copy and reference mode.
- [x] Preserve initialized storage, all pointer roots and existing entry ABIs.
- [x] Final focused verification and check/clippy recorded.
- [ ] Optimized target-code inspection, bounded Phase A A/B and Docker gate.
- [ ] Phase A adoption decision based on measurements and full validation.
- [x] Document a Phase B ABI/effect proposal and multiple-prefix constraint.
- [ ] Phase B contract reviewed after A evidence; prototype/differential tests.
- [ ] Phase B helper-reaching tests, separate A/B/gate and adopt/reject decision.

Remaining items are tracked by #536/#534. Phase B execution is blocked by its
explicit measured-A prerequisite; no new deferred-work issue duplicates them.
