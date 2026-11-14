# Compiler phase contracts

What each phase of `okkama-compiler` takes, what it guarantees to the next,
and what checks that guarantee (hardening H-19). A phase may assume the
guarantees of the previous one only where a verifier enforces them; every
public consumer of a phase's output checks it again.

| Phase | Takes | Guarantees | Checked by |
|---|---|---|---|
| Lexer (`syntax/lexer`) | any text | terminates; every token lies on UTF-8 character boundaries | lexer tests, fuzz target `lex` |
| Parser (`syntax`) | tokens | an AST within depth 128 and 2²⁰ nodes, or diagnostics; trees drop without recursion; formatted source parses again | parser limits, fuzz target `parse` |
| Analysis (`analysis`) | AST of the root files and their imports (graph bounded, [#20]) | THIR: every name resolved once (conflicts name both sources, [#33]), every expression typed, roles and caps checked; deterministic in the order of files and hash maps | analysis tests, conformance (check stage) |
| Lowering (`lowering`) | THIR | OIR-1: typed SSA, blocks with explicit unwind targets, cleanup regions; specializations bounded ([#32]); synthetic instructions carry the span of the source operation they implement (short-circuit, `throw`, closures, region cleanup) | `ir::verify` on the result (an invalid module is an internal compiler error), `okkama-cli/tests/provenance.rs` |
| IR passes (`lowering/mod.rs`) | verified OIR | verified OIR with the same meaning: `regions`, `tail-calls`, `fold`, `hoist`, `undef`, `jumps` | `ir::verify` after them; the gate's sparrow-passes-off conformance run; `okkama-cli/tests/ir_passes.rs` ([#29]) |
| Sparrow codegen (`codegen/sparrow`) | OIR (verified again on entry) | SPB whose register allocation keeps every read value in its register | independent allocation check in debug builds and the gate ([#30]); `okkama_sparrow::Program::verify` (types, [#16]) on encode |
| SPB loading (`okkama-sparrow`, `sparrow-core`) | any bytes | a verified, type-checked program within decode and analysis budgets ([#24]), intrinsic contracts hold | `decode`, `sparrow check`, fuzz target `spb` |

OIR-2 has four independent verifier boundaries in `okkama-oir`. This is the
contract for the new pipeline; the OIR-1 production pipeline above continues
until its replacement is integrated. Each producer sets the exact output
profile and verifies its result. Each public consumer verifies again on entry.
A rejected producer result is a compiler transformation error. Verification
neither repairs the module nor invokes ownership inference.

| Boundary | Producer obligations |
|---|---|
| `high` | THIR lowering emits typed SSA, explicit failure paths and lexical scope/result metadata satisfying H1–H9. |
| `own` | Ownership elaboration emits checked summaries, full deps, packets, provenance, acceptance and cleanup. |
| `coro` | Coroutine lowering preserves ownership across save/resume/cancel and supplies frame and proof evidence. |
| `low` | Cleanup lowering emits checked phase/serial plans, typed exit/failure dispatch and SSA root maps. |

The entry checks, in that order, are `verify_high`, `verify_own`, `verify_coro`
and `verify_low`; each requires an exact `Module.profile` match. All four also
check declarations, types, SSA, edge contracts, authority and the obligations
represented at that profile. `verify_function` is a development aid and does
not certify a whole module. External implementations require a trusted ABI
catalogue supplied by the consumer; input declarations cannot register trust.

The `high` producer must preserve single-entry lexical scope subtrees, defer
prefixes at merges, explicit want/discard and preparation for valued exits,
entry-visible ordinary unwind arguments, and original rebind/stage destinations.
It emits no linear ownership states. The `own` producer accepts fresh inputs
at call entry, preserves bound input ownership and borrow lifetimes, conserves
every linear result on every outcome, and makes cleanup and cancellation
obligations explicit. Empty selections do not certify a managed value as plain.

The `coro` producer must transfer every live linear slot exactly once, preserve
full dependencies and cancellation cleanup, and renew `UniqueCacheProof`
evidence after alias/frame changes. The synchronous slice may skip this pass
only with proved synchronous entries, descriptors and owned cleanup. The `low`
producer must settle service duties, preserve reserve and active failure,
finish the whole destroy suffix, and supply exactly one typed root map at each
SSA boundary. Direct cleanup needs independently checked order/suffix evidence.

`StructuralCheck::ALL` is the explicit inventory of checks that remain limited
by producer metadata. `VerifyStats::structural_checks` reports those encountered:

- `B95FrameLayoutAndContinuations`: represented slot types and save/resume/cancel
  transfers are checked; a complete declared frame layout and continuation graph
  still require coroutine producer metadata (#570).
- `B105SuspensionRootMaps`: Low SSA root bindings and their completeness are
  checked. Complete suspension maps still require producer metadata, and physical
  machine stack maps require backend validation after register allocation (#571).
- `B96CollectorPositions`: holder, transition, cancellation and root contracts
  are checked. Positional multiplicity, duplicate input positions and collector
  scheduling require typed producer metadata and behavioral validation.

A successful certificate does not close these three gaps. Owned carrier graphs
remain excluded from 0.6.0 (B7.5); canonical M7 witnesses preserve that rejection.
Protocol checks do not prove sidecar destroy traces or scheduler outcomes.

Verifier coverage is executable: `tests/fixtures/verify/matrix.tsv` names a
canonical positive and negative for every `RuleId::ALL` entry. The spec manifest
retains the #563 codec illustrations and separately names complete canonical
verifier witnesses, expected negative RuleIds and a verification level for every
example, M1–M9 and A.1.1–A.1.24. `runtime-behavior`, `frontend-reject` and
`historical` entries do not claim runtime, frontend or OIR-1 verification from
the OIR-2 certificate. G1 needs joint #563/#564/#565 evidence; G2–G5 are not
established by this verifier corpus.

Test-mode role fakes lower to ordinary `Role$fake` objects with typed fields.
Mutable test state uses `Cell` (synchronous locals) or `Atomic` (async code and
object fields). Its test-specific intrinsics are assertions and runner controls;
there are no hidden fake-storage operations. Receiver captures follow ordinary
field and action references (#583).

Public consumers of OIR — `codegen::generate` (both backends) — run `ir::verify`
first and return an error for a corrupted module.

Inspecting the pipeline: `okkama ir FILE` prints the OIR after all passes,
`okkama ir --passes none FILE` before them, `okkama ir --after PASS FILE`
after the passes up to `PASS`; `OKKAMA_IR_PASSES` selects passes for every
command; `OKKAMA_DUMP_INVALID_IR=FILE` writes a module lowering rejected.

[#16]: https://github.com/shootnix/okkama/issues/16
[#20]: https://github.com/shootnix/okkama/issues/20
[#24]: https://github.com/shootnix/okkama/issues/24
[#29]: https://github.com/shootnix/okkama/issues/29
[#30]: https://github.com/shootnix/okkama/issues/30
[#32]: https://github.com/shootnix/okkama/issues/32
[#33]: https://github.com/shootnix/okkama/issues/33
