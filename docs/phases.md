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
| IR passes (`lowering/mod.rs`) | verified OIR | verified OIR with the same meaning: `tail-calls`, `hoist`, `regions`, `jumps`, `undef` | `ir::verify` after them; the gate's IR conformance run with the passes off; `okkama-cli/tests/ir_passes.rs` ([#29]) |
| Sparrow codegen (`codegen/sparrow`) | OIR (verified again on entry) | SPB whose register allocation keeps every read value in its register | independent allocation check in debug builds and the gate ([#30]); `okkama_sparrow::Program::verify` (types, [#16]) on encode |
| JVM codegen (`codegen/jvm`) | OIR (verified again on entry) | class files the JVM verifier accepts | JVM verification at load; conformance on `jvm` |
| SPB loading (`okkama-sparrow`, `sparrow-core`) | any bytes | a verified, type-checked program within decode and analysis budgets ([#24]), intrinsic contracts hold | `decode`, `sparrow check`, fuzz target `spb` |

Public consumers of OIR — `codegen::generate` (both backends),
`okkama_runtime::execute*` and `okkama_runtime::run_tests` — run `ir::verify`
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
