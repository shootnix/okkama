# Ownership corpus — issue #560

This directory is the source corpus for the ownership/OIR oracle (#560).
The corpus records what the Book requires; runtime disagreements have tickets,
not runtime-derived replacement expectations. The branch rule follows #308,
which supersedes the outdated branch-tail wording in OIR-1 section 7.

The entire corpus lives outside `t/`: `tools/t/run.py` discovers every `.ok`
under `t/` for the six-configuration release gate, including JVM. Intentional
failures, compile-negative fixtures and pending semantics must not enter it.

## Inventory and validation

- Main directory: **97 programs, 441 TAP assertions** (37 handwritten programs
  with 201 assertions, plus 60 generated programs with 240 assertions).
- 95 main programs expect exit code 0 with all TAP assertions passing.
- Two main programs intentionally exit 1 after two passing cleanup assertions
  each: `panic-unwind.ok` and `secondary-failures.ok`.
- No pending assertions remain: #581 restores the eight checks from #579–#581
  after focused in-process verification on both engines.
- `rejected/`: six compile-negative fixtures, not executable oracle inputs.
- All 441 assertions are in the main corpus. Original assertions are retained
  exactly once; the four historical corrections below and `channel-transfer`
  check 1 (the #581 ownership decision) change expected values. #578 adds
  four checks for running
  an escaped Future, cached reuse, and cleanup at its owner’s exit.
- Claude's original 0.5.8 IR/Sparrow results: 849 passing and 25 failing
  check executions (437 checks on each engine). Those saved results were
  inspected without running programs. Existing passing expectations remain
  unchanged except for the ownership decision recorded under #581 below.
- Regeneration command: `python3 tests/ownership-corpus/gen.py`. All 60
  generated files reproduce byte for byte after the import-path migration;
  their only change from the old location is `../support/tap` → `tap`.
- At relocation, all 100 executable files passed the checker; all six
  negative fixtures were rejected for their intended rule. TAP plans match
  literal assertion counts.
- No programs, cargo, Docker, release gate or benchmarks were run in the
  original corpus relocation. Whole-corpus runtime verification belongs to
  oracle runner #562; the #578 in-process checks are recorded below.

Programs import `tap` from `t/support` through `OKKAMA_IMPORT_DIRS`; oracle
runner #562 will set it. From the repository root, check a positive file with:

```sh
OKKAMA_STDLIB_DIR=$PWD/std OKKAMA_IMPORT_DIRS=$PWD/t/support ~/.local/bin/okkama check tests/ownership-corpus/branches.ok
```

Each runnable file has its own resource objects and log. Each assertion first
prints the actual log as a TAP diagnostic, then compares it with a literal
expected string. Uppercase letters normally identify resources; lowercase
letters and `|` are explicit defer/handler/use markers. Async destroy uses
`A<` and `A>` to distinguish entry and completion.

## Execution classes for the Docker oracle

Use the 95 ordinary successful main programs as standard TAP inputs. Run
both `EXPECT-RUNTIME-FAIL` programs separately: require all cleanup assertions
to pass and exit code **1** on both engines. Match stderr using these Python
regular expressions (stack traces and source locations may differ):

| File | Exit | Required stderr pattern |
| --- | ---: | --- |
| `panic-unwind.ok` | 1 | `(?m)^Panic: ownership corpus deliberate panic$` |
| `secondary-failures.ok` | 1 | `(?m)^Body: Body\.failed\nSecondary cleanup failure:\n[ \t]+Cleanup: Cleanup\.second\nSecondary cleanup failure:\n[ \t]+Cleanup: Cleanup\.first$` |

The panic cleanup logs are `dBA`, then `dBAP`; the secondary-failure logs
are `BA`, then `BAC`. The primary body failure and both secondary failures
must appear in the stated order. Both saved engines satisfied these contracts.
Exclude `pending/` and `rejected/` from ordinary runtime discovery; the latter
are checked for intentional language refusals instead.

## Corrected expectations and pending tickets

Numbers below refer to checks in the original 97-program corpus.

| Original check | Old expectation | New expectation | Rule |
| --- | --- | --- | --- |
| `branches` 3 | `""` | `"B"` | #308: discarded if value dies with its branch |
| `branches` 7 | `""` | `"D"` | #308: discarded match value dies with its branch |
| `temporary-receivers` 3 | `"AC"` | `"A"` | OIR-1 §7: aggregate receiver contents live until block exit |
| `temporary-receivers` 5 | `"ED"` | `""` | Same rule; D and E are still live at this checkpoint |

All four new expectations match both saved engine observations. Final cleanup
expectations (`B`, `D`, `ACB`, `ED`) are unchanged.

#581 resolves cached-outcome ownership: a Future owns its outcome and `await`
borrows it. The Book wording change is proposed separately; the Book is not
edited by this implementation.

| Restored checks | Expectations | Verification |
| --- | --- | --- |
| `async-results` 6 (#579) | `"LR"` | Temporary Future cleanup, including `region.track` after await |
| `all-futures` 1–3 (#580) | `""`, `""`, `"BA"` | Duplicate inputs, repeated collection, creation-order LIFO |
| `future-completions` 1–4 (#581) | `""`, `""`, `""`, `"BA"` | Repeated Future identities and settlement requests |

The restored assertions keep their original expectations. `channel-transfer`
check 1 changes from `""` to `"R"`: the receive helper returns a borrowed await
result, whose local Future dies at helper exit. The caller must not dereference
that expired borrow. Checks 2–3 still expect `"R"` and describe no duplicate
cleanup. Wording only changes in `async-results` 4–5, `async-destroy` 4,
`raced` 2 and 4, and restored `all-futures` 3 identify the Future as owner and
state reverse creation order explicitly.

## Handwritten programs

The table gives the Book sections and the distinction beyond the simple
scope/defer/alias examples already in L07. Checks include intermediate
empty-log observations, so an early destroy cannot hide behind a correct final
order.

| File | Checks | Book rules and coverage |
| --- | ---: | --- |
| [scopes.ok](scopes.ok) | 6 | 12.2, 20.1–20.3: lexical ownership, nested scopes, shadowing and nonescaping block tails |
| [branches.ok](branches.ok) | 8 | 11.6, 12.3–12.4, 20.2: selected branch values retained and discarded |
| [loops.ok](loops.ok) | 6 | 12.5–12.7, 20.2–20.3: iteration cleanup on next, last and valued last |
| [rebinds.ok](rebinds.ok) | 5 | 12.1a, 18.5, 19, 20.2: rebind retains old identities; closure snapshot |
| [temporary-receivers.ok](temporary-receivers.ok) | 6 | 20.2: fresh receiver adopted by action, borrowed receiver retained |
| [temporary-arguments.ok](temporary-arguments.ok) | 5 | 20.2: fresh and borrowed arguments, duplicate identities, anonymous actions |
| [call-chains.ok](call-chains.ok) | 5 | 18.1, 20.2: fresh values through multiple calls and early returns |
| [call-results.ok](call-results.ok) | 6 | 18.1, 20.2: kept/dropped results, returned fields and extracted children |
| [fields.ok](fields.ok) | 6 | 16.2, 20.2: nested fields, shared fields and parent/child destruction |
| [arrays.ok](arrays.ok) | 7 | 8.1, 8.2a, 20.2: nested arrays, repeated references, equal-but-distinct identities and concatenation |
| [maps.ok](maps.ok) | 5 | 8.2–8.3, 20.2: map creation order, merge replacement, repeated values |
| [collection-copies.ok](collection-copies.ok) | 5 | 8.2a–8.3, 13, 20.2: slices, subtraction and derived arrays retain identity |
| [successor-fields.ok](successor-fields.ok) | 4 | 19, 20.2: copied fields borrow, replaced fields transfer, original preserved |
| [successor-staging.ok](successor-staging.ok) | 4 | 19, 20.2–20.3: nested staged fields survive caught failure; failed staging drains |
| [unwind.ok](unwind.ok) | 5 | 14.3, 20.3: nested throw cleanup, handler-owned resources, rethrow |
| [try-results.ok](try-results.ok) | 6 | 14.3, 14.5, 20.2: resources returned from try and catch, try optional |
| [defer-order.ok](defer-order.ok) | 5 | 15, 20.3: lazy defer inputs, resources acquired by defer, return cleanup |
| [destroy-failures.ok](destroy-failures.ok) | 4 | 15, 20.1–20.3: throwing destroy, primary failure and remaining cleanup |
| [partial-construction.ok](partial-construction.ok) | 4 | 17.5, 20.2: completed resources tracked before failed argument or aggregate |
| [confirm-and-errors.ok](confirm-and-errors.ok) | 5 | 14.1–14.4, 20.3: confirm throws, scalar Error payloads, automatic causes |
| [closures.ok](closures.ok) | 6 | 18.5, 20.2: escaped captures, shared captures, closure field storage |
| [bound-actions.ok](bound-actions.ok) | 5 | 18.5, 20.2: bound receiver lifetime, escaping method reference |
| [caps.ok](caps.ok) | 4 | 16.3, 19, 20.2: caps retain hidden resources and original snapshot |
| [roles-generics.ok](roles-generics.ok) | 5 | 22, 25, 20.2: role returns, generic wrappers, any transport |
| [async-results.ok](async-results.ok) | 6 | 20.2, 31.5: Future-owned results across suspension and repeated awaits |
| [cold-futures.ok](cold-futures.ok) | 10 | 20.2, 31.5: lazy futures retain receiver and arguments before startup |
| [spawn-cancellation.ok](spawn-cancellation.ok) | 5 | 15, 20.3, 31.5: cooperative spawn, cancellation waits for child cleanup |
| [future-group.ok](future-group.ok) | 5 | 20.2–20.3, 31.5; async/scope: supervised resources and duplicate registration |
| [all-futures.ok](all-futures.ok) | 6 | 20.2, 31.5; async/collect: duplicate results, empty collector and failure/cancellation cleanup |
| [future-completions.ok](future-completions.ok) | 4 | #581; 20.2, 31.5; async/collect: duplicate identities borrow Future-owned outcomes |
| [raced.ok](raced.ok) | 4 | 20.2–20.3, 31.5; async/collect: loser cleanup precedes winner publication |
| [channels.ok](channels.ok) | 8 | 20.2, 31.5.2; async/channel: message pins, unread messages, close and alias ownership |
| [atomic-resources.ok](atomic-resources.ok) | 6 | 20.2; Book Shared infrastructure snapshots with Atomic: stored capabilities borrow |
| [async-destroy.ok](async-destroy.ok) | 10 | 20.1.1, 20.3, 31.5: awaited LIFO destruction and shielded cancellation |
| [channel-transfer.ok](channel-transfer.ok) | 6 | 20.2, 31.5–31.5.2: returned await borrow expires; cancel blocked send |
| [panic-unwind.ok](panic-unwind.ok) | 2 | 14.2, 15, 20.1–20.3: panic unwind; cleanup allocation |
| [secondary-failures.ok](secondary-failures.ok) | 2 | 14.2–14.4, 20.1–20.3: uncaught primary and secondary failures |

## Generator

[`gen.py`](gen.py) uses Python 3 and only `itertools`, `pathlib`, and
`random`. It does not invoke Okkama or any subprocess. It writes only the
60 `gen-NNN.ok` files alongside itself, with UTF-8 text and no timestamps.
Running it from any working directory produces the same files.

The design is an exhaustive **3 × 2 × 2 × 5** product:

- Scope: plain block, selected if branch, single for iteration.
- Temporary: fresh receiver or fresh callee argument.
- Exit: normal completion or typed throw.
- Aggregate: array, reverse-key-order map, nested array, object fields, or
  repeated references.

`Random(560)` shuffles the numbering of these combinations; it never chooses
which combinations to omit. All cases construct outer resource O, scoped
resource H, aggregate resources A then B, and temporary T. A reached defer
appends d. Thus the four assertions expect: empty log; T while the aggregate
is live; TdBAHs on normal continuation or TdBAHc in a catch; then the same
string with O appended. The field wrapper has no destroy; repeated aggregate
references never manufacture a second resource identity. Expected strings
come from this construction sequence, not an implementation of the runtime
ownership algorithm.

Each file is small and self-contained; neither generating nor reading it
requires running another corpus program.

| Generated file | Scope | Temporary | Exit | Aggregate |
| --- | --- | --- | --- | --- |
| [gen-000.ok](gen-000.ok) | block | argument | throw | nested |
| [gen-001.ok](gen-001.ok) | iteration | argument | throw | fields |
| [gen-002.ok](gen-002.ok) | block | argument | normal | nested |
| [gen-003.ok](gen-003.ok) | iteration | receiver | normal | map-reverse-keys |
| [gen-004.ok](gen-004.ok) | iteration | argument | normal | array |
| [gen-005.ok](gen-005.ok) | iteration | argument | normal | fields |
| [gen-006.ok](gen-006.ok) | iteration | receiver | throw | shared |
| [gen-007.ok](gen-007.ok) | iteration | argument | normal | map-reverse-keys |
| [gen-008.ok](gen-008.ok) | branch | argument | throw | array |
| [gen-009.ok](gen-009.ok) | branch | argument | normal | shared |
| [gen-010.ok](gen-010.ok) | branch | receiver | normal | fields |
| [gen-011.ok](gen-011.ok) | branch | receiver | throw | array |
| [gen-012.ok](gen-012.ok) | iteration | receiver | normal | fields |
| [gen-013.ok](gen-013.ok) | block | receiver | normal | map-reverse-keys |
| [gen-014.ok](gen-014.ok) | branch | argument | normal | fields |
| [gen-015.ok](gen-015.ok) | block | receiver | normal | fields |
| [gen-016.ok](gen-016.ok) | branch | argument | throw | shared |
| [gen-017.ok](gen-017.ok) | block | argument | normal | fields |
| [gen-018.ok](gen-018.ok) | iteration | receiver | normal | shared |
| [gen-019.ok](gen-019.ok) | block | argument | normal | array |
| [gen-020.ok](gen-020.ok) | iteration | receiver | normal | nested |
| [gen-021.ok](gen-021.ok) | branch | receiver | throw | map-reverse-keys |
| [gen-022.ok](gen-022.ok) | branch | argument | throw | fields |
| [gen-023.ok](gen-023.ok) | branch | receiver | normal | array |
| [gen-024.ok](gen-024.ok) | branch | receiver | normal | map-reverse-keys |
| [gen-025.ok](gen-025.ok) | block | argument | normal | shared |
| [gen-026.ok](gen-026.ok) | block | receiver | throw | array |
| [gen-027.ok](gen-027.ok) | iteration | argument | throw | nested |
| [gen-028.ok](gen-028.ok) | branch | argument | normal | nested |
| [gen-029.ok](gen-029.ok) | iteration | argument | normal | shared |
| [gen-030.ok](gen-030.ok) | block | receiver | throw | nested |
| [gen-031.ok](gen-031.ok) | branch | receiver | throw | fields |
| [gen-032.ok](gen-032.ok) | block | argument | throw | fields |
| [gen-033.ok](gen-033.ok) | block | argument | normal | map-reverse-keys |
| [gen-034.ok](gen-034.ok) | block | argument | throw | map-reverse-keys |
| [gen-035.ok](gen-035.ok) | branch | argument | throw | map-reverse-keys |
| [gen-036.ok](gen-036.ok) | iteration | receiver | throw | map-reverse-keys |
| [gen-037.ok](gen-037.ok) | block | argument | throw | shared |
| [gen-038.ok](gen-038.ok) | branch | argument | normal | map-reverse-keys |
| [gen-039.ok](gen-039.ok) | block | receiver | throw | fields |
| [gen-040.ok](gen-040.ok) | iteration | argument | throw | map-reverse-keys |
| [gen-041.ok](gen-041.ok) | block | receiver | throw | shared |
| [gen-042.ok](gen-042.ok) | iteration | argument | throw | array |
| [gen-043.ok](gen-043.ok) | iteration | argument | normal | nested |
| [gen-044.ok](gen-044.ok) | block | receiver | throw | map-reverse-keys |
| [gen-045.ok](gen-045.ok) | branch | receiver | throw | nested |
| [gen-046.ok](gen-046.ok) | block | receiver | normal | shared |
| [gen-047.ok](gen-047.ok) | iteration | argument | throw | shared |
| [gen-048.ok](gen-048.ok) | block | argument | throw | array |
| [gen-049.ok](gen-049.ok) | branch | receiver | normal | nested |
| [gen-050.ok](gen-050.ok) | branch | receiver | normal | shared |
| [gen-051.ok](gen-051.ok) | branch | argument | throw | nested |
| [gen-052.ok](gen-052.ok) | iteration | receiver | throw | array |
| [gen-053.ok](gen-053.ok) | block | receiver | normal | array |
| [gen-054.ok](gen-054.ok) | branch | receiver | throw | shared |
| [gen-055.ok](gen-055.ok) | iteration | receiver | throw | nested |
| [gen-056.ok](gen-056.ok) | branch | argument | normal | array |
| [gen-057.ok](gen-057.ok) | block | receiver | normal | nested |
| [gen-058.ok](gen-058.ok) | iteration | receiver | normal | array |
| [gen-059.ok](gen-059.ok) | iteration | receiver | throw | fields |

## Expected checker refusals

These files start with `EXPECT-CHECK-FAIL`, deliberately distinct from the
`CHECK-FAILS` marker reserved for a supposedly valid construct that the
checker rejects. Each is a standalone negative compilation unit so one
diagnostic cannot mask another.

| File | Book rule | Observed diagnostic |
| --- | --- | --- |
| [rejected/once-resource.ok](rejected/once-resource.ok) | 18.6: direct resource | `once let cannot keep a resource: its value would outlive the scope that releases it` |
| [rejected/once-generic.ok](rejected/once-generic.ok) | 18.6, 25: resource in a generic array | Same resource-lifetime diagnostic |
| [rejected/once-role.ok](rejected/once-role.ok) | 18.6, 22: role with resource-bearing implementer | Resource-lifetime diagnostic, with `Cache keeps Label, which reaches one through a function value or a role` |
| [rejected/once-closure.ok](rejected/once-closure.ok) | 18.5–18.6: action type with a resource capture | Resource-lifetime diagnostic, with `Cache keeps () is string, which reaches one through a function value or a role` |
| [rejected/map-resource-key.ok](rejected/map-resource-key.ok) | 8.2: map key must be resource-free | `Map key type Res cannot be a key: Res has a destroy block` |
| [rejected/error-resource-payload.ok](rejected/error-resource-payload.ok) | 14.2: enum Error payload restriction | `Payload "item" of error enum "Invalid" has type Res; error payloads are limited to string, int, byte, float, rat, bool and enums with such payloads` |

## Semantic assumptions and interesting boundaries

These expectations follow the Book and the rulings above. The saved runtime
results corroborate the four historical corrections; #581's focused regressions
verify the new ownership decision.

1. **Error payload contradiction.** Sections 14.3, 20.2 and 20.3 describe
   ownership transfer of resource-bearing Error payloads. Section 14.2
   expressly forbids object/role/collection payloads, and objects cannot
   implement Error. The checker enforces 14.2. Accordingly this corpus checks
   scalar/nested-enum payloads and rethrow at runtime, and records the
   resource-payload refusal separately. There is no legal resource-bearing
   Error fixture in the current language. This is a documentation/language
   boundary, not a missing runnable example hidden behind a skip.

2. **Resource map keys are impossible by design.** Section 8.2 excludes a
   destroy block and resource-bearing fields in keys. The positive map case
   uses resource-free object keys and resource values, in creation order
   different from key sorting. The negative fixture records the prohibited
   resource key rather than inventing a runtime meaning.

3. **Discarded branch tails.** Since #308, branch-local lets and discarded
   values die with the selected branch (see `t/lang/destroy.ok`). The old
   OIR-1 §7 branch-value paragraph is outdated. Kept branch results still
   transfer to their caller; checks 3 and 7 now expect B and D immediately.

4. **Rebind from a child scope.** `rebinds.ok` expects C assigned into an
   outer local to survive the child scope and later close as CBA. Section
   12.1a explicitly updates the declaring scope, while 20.2 determines
   resource ownership; the lifetime transfer of a fresh replacement follows
   from combining those rules, rather than a dedicated Book example. The old
   values must not die at either assignment.

5. **Builtin aggregate receivers.** OIR-1 §7 explicitly gives
   `let k = [Res(name: "q")].size(); // Res "q" lives to the end of the block`.
   Array contents stay live at the intermediate checkpoints; they close in
   reverse construction order when the block ends. The fresh object receiver
   rule remains covered separately.

6. **Parent versus field order.** `fields.ok`, `bound-actions.ok` and
   `caps.ok` expect the completed owner to be newer than resources built as
   its arguments, hence OC or OR. Returning a child releases O immediately
   while C escapes. This uses creation-order LIFO and the explicit
   child-extraction rule in 20.2; it does not assume field traversal order.

7. **Channels and Atomic have different contracts.** Channel fresh messages
   are retained through delivery. The receiving Future owns its fresh outcome;
   returning only an awaited alias does not extend that Future's lifetime.
   `channel-transfer` now expects R at helper exit; a receive Future owned by
   the receiving scope releases R at that scope's end. Unread U appears when
   all channel capabilities finish. A cancelled rendezvous send expects ML: message input cleanup
   before the sender's local. In contrast, the Book's “Shared infrastructure
   snapshots with Atomic” section explicitly says stored resources remain
   borrowed. `atomic-resources.ok` therefore expects an inner stored resource
   to die at its original scope boundary and never dereferences that expired
   stored value. Atomic close is not treated as a resource owner.

8. **Cleanup failure diagnostics.** The Book explicitly gives the body
   failure priority and requires every destroy to run. The special
   `secondary-failures.ok` fixture also expects secondary diagnostic order
   to match actual LIFO cleanup order, B then A. The TAP assertions verify
   the cleanup sequence; Claude must inspect stderr to validate the primary
   and secondary diagnostic carrier behavior.

## Original corpus relocation completion record

- [x] Move the entire corpus, generator, README and rejected fixtures out of `t/`.
- [x] Use `tap` through `OKKAMA_IMPORT_DIRS` and document runner #562's setup.
- [x] Correct four expectations using #308 and OIR-1 §7.
- [x] Isolate exactly 10 ticket-dependent assertions in four pending files,
  retaining every previously passing assertion and its expectation.
- [x] Record expected exit codes and stderr patterns for intentional failures.
- [x] Regenerate 60 programs and verify byte-identical output after migration.
- [x] Check 100 positive files and six intended checker refusals.
- [x] Verify 427 main + 10 pending assertions against TAP plans and the
  original saved results; document old/new expectations and ticket mapping.
- [x] Remove `.corpus-run/` after inspecting its results. No commit made.

## #578 cold Future dependencies

- [x] Treat Future and aggregates containing Future as non-plain during lowering
  and region elision, preserving track/adopt and keep across both function scopes.
- [x] Walk Sparrow Future retained arguments and cached results. Keep arguments
  separately from reusable registers, trace them for GC, and exclude mutable
  Future graphs from ownership summaries.
- [x] Verify eight in-process regressions on both IR and Sparrow: never started,
  cancelled before startup, repeated await, borrowed dependencies, object fields,
  escape after completion, duplicate identity, and cached result escape.
- [x] Verify IR graph transfer/deduplication/order with a focused unit test and
  lowering/region keeps with a focused integration test.
- [x] Restore both original pending checks to `cold-futures.ok`, add four await
  checks, and lower the actual corpus file with stdlib in-process. Its TAP plan
  is 10; main/pending inventories are 433/8 assertions with matching plans.

Whole-corpus execution in Docker remains assigned to Claude / oracle runner
#562. At that milestone, #579–#581 were outside the #578 fix.

## #581 Future-owned outcomes

- [x] Give Futures ownership identities and private outcome regions on both
  engines; await, track, adopt, keep and transfer of an outcome remain borrows.
- [x] Preserve the #578 retained-input graph and escaping Future behavior,
  including cold and completed collector Futures with fresh receivers.
- [x] Drain dying Future outcomes into creation-order cleanup, including
  duplicate inputs, nested Futures, async destroy and failing destroy.
- [x] Keep the IR collector's cached Future owned by the collector, with GC edges.
- [x] Restore #579–#581 checks and update the channel-return expectation and
  ownership wording listed above; all 441 TAP assertions have matching plans.
- [x] Verify 15 focused in-process cases on both engines and eight #578 cases.
- [x] Verify three IR ownership unit cases, the #578 transfer unit case, a
  cycle-collection regression, and three lowering tests (including all changed
  corpus sources with stdlib).
- [x] Pass `cargo check` and `cargo clippy --all-targets -- -D warnings` for
  `okkama-compiler`, `okkama-runtime-core`, `okkama-runtime` and `sparrow-core`,
  always from `okkama-rs/` with `--offline -j 4`; pass `git diff --check`.

The full corpus, Docker and benchmarks are not run under this task's machine
restrictions. The Book remains unchanged; the final report proposes exact wording.
