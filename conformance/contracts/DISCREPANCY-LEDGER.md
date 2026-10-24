# Discrepancy ledger

Generated from `contracts/discrepancy-ledger.json` by `tools/render_ledger.py`; edit the JSON.

Material disagreements between Okkama sources, with the conformance decision for each.

Implementation evidence: okkama-pl (Perl) at git f1ad6d43. The interpreter is evidence, not an oracle:
an entry is resolved only by a cited authority, never because an implementation behaves a certain way.

Summary: 4 deferred, 43 resolved.

| ID | Subject | Resolution |
|---|---|---|
| [D-001](#d-001) | Projection syntax: `&action` versus `#projected` | resolved |
| [D-002](#d-002) | `confirm ... else throw` missing from the EBNF | resolved |
| [D-003](#d-003) | Productions the specification adds but the EBNF lacks | resolved |
| [D-004](#d-004) | Category of a bare failed `confirm` | resolved for conformance (category naming remains a documentation inconsistency) |
| [D-005](#d-005) | Disputed reserved words `optional`, `setters`, `mutable`, `var` | resolved |
| [D-006](#d-006) | Spurious secondary lexer diagnostic after an unknown escape | resolved (non-normative observation) |
| [D-007](#d-007) | Byte values as map keys | resolved |
| [D-008](#d-008) | Runtime support for `import ... as` | resolved |
| [D-009](#d-009) | Decimal literal tokens | deferred |
| [D-010](#d-010) | Documents cite standard-library paths that are not in the active std/ | resolved |
| [D-011](#d-011) | How the standard library is located | resolved |
| [D-012](#d-012) | Detection stage of missing `take` names | resolved |
| [D-013](#d-013) | Specification section 22 describes async as having no futures or scheduler | resolved |
| [D-014](#d-014) | "No cancellation, timeout, race or join" versus library support | resolved |
| [D-015](#d-015) | Diagnostic message formats | resolved |
| [D-016](#d-016) | Ordinary int arithmetic beyond signed 64-bit | resolved |
| [D-017](#d-017) | Detection stage of a missing entry point | resolved |
| [D-018](#d-018) | Anonymous action parameter annotations | resolved |
| [D-019](#d-019) | `probe` fixture shorthand | resolved |
| [D-020](#d-020) | Untyped `has` declarations | resolved (runtime typing of untyped fields not asserted) |
| [D-021](#d-021) | Residual mentions of the removed `allow` declaration | resolved |
| [D-022](#d-022) | Experimental effects and export checking | resolved (excluded) |
| [D-023](#d-023) | Explicit runtime import directories | resolved |
| [D-024](#d-024) | Match exhaustiveness and branch type compatibility | deferred |
| [D-025](#d-025) | Generic role bounds absent from the EBNF | resolved |
| [D-026](#d-026) | Proposed backend contracts | resolved (excluded) |
| [D-027](#d-027) | Unicode data versions | resolved |
| [D-028](#d-028) | Grammar `for` patterns with several bindings or an index | deferred |
| [D-029](#d-029) | Section 22 lists owner-only private fields as parser-only | resolved |
| [D-030](#d-030) | CR LF line endings | resolved |
| [D-031](#d-031) | Sign of `%` with negative operands | resolved |
| [D-032](#d-032) | Canonical text of floats | resolved |
| [D-033](#d-033) | Whitespace set of trim() and CR handling of chomp() | resolved |
| [D-034](#d-034) | Exit status after an unobserved spawned failure | resolved |
| [D-035](#d-035) | Field defaults and construction order | resolved |
| [D-036](#d-036) | Iteration protocol names | resolved |
| [D-037](#d-037) | Ordering relation of map keys | resolved (with a documentation request) |
| [D-038](#d-038) | Async native declarations | resolved |
| [D-039](#d-039) | Channel API names | resolved |
| [D-040](#d-040) | Channel capacity zero | resolved |
| [D-041](#d-041) | `Cancelled` as a catch type | resolved |
| [D-042](#d-042) | Regex API | resolved |
| [D-043](#d-043) | Builtin `order()` on scalars | resolved |
| [D-044](#d-044) | Formatting API | resolved |
| [D-045](#d-045) | Canonical spelling of Decimal values | deferred |
| [D-046](#d-046) | Implicit promotion between numeric categories | resolved |
| [D-047](#d-047) | Map literals with keys of different types | resolved |

## D-001

**Projection syntax: `&action` versus `#projected`**

- Sources: `docs/okkama-grammar-1.0.ebnf lines 58, 94 (`projection_action_decl = "&" action_decl`)`; `docs/okkama-spec-11072026.md section 11.3`; `docs/okkama-llm-context-1.0.md sections 2 and 14`
- Observed: okkama-pl (Perl) at git f1ad6d43: `&doubled()` is rejected ("Object projection declarations use #projected, not &"); `#projected` and `#projected(alias)` work.
- Intended contract: Single `&` projection syntax is removed; `#projected` / `#projected(alias)` mark projection actions.
- Authority: Specification 11.3 (updated 18.10.2026) states the removal explicitly; the EBNF was not updated.
- Conformance expectation: Mandatory: `&` projections are rejected, `#projected` prefixes work (GRAM.PROJECTED-DIRECTIVE, OBJ.PROJECTIONS).
- Resolution: **resolved**
- Contracts: `GRAM.PROJECTED-DIRECTIVE`, `OBJ.PROJECTIONS`
- Cases: `L01.gram.projected.directive`, `L01.gram.projected.removed-ampersand`

## D-002

**`confirm ... else throw` missing from the EBNF**

- Sources: `docs/okkama-grammar-1.0.ebnf line 137 (`confirm_else = "else" "panic" expr`)`; `docs/okkama-spec-11072026.md section 9.4`
- Observed: okkama-pl (Perl) at git f1ad6d43: both `else throw` and `else panic` parse and run.
- Intended contract: confirm accepts exactly `else throw expr` or `else panic expr`; the operand is lazy.
- Authority: Specification 9.4.
- Conformance expectation: Mandatory: both forms accepted, any other else operand rejected.
- Resolution: **resolved**
- Contracts: `GRAM.CONFIRM`, `ERR.CONFIRM`
- Cases: `L01.gram.confirm.forms`, `L01.gram.confirm.else-value`

## D-003

**Productions the specification adds but the EBNF lacks**

- Sources: `docs/okkama-grammar-1.0.ebnf`; `docs/okkama-spec-11072026.md sections 9.5 (throw / throw;), 11.3 (#projected), 14 (async test), 6.5 (generic bounds `<T: Role>`)`
- Observed: okkama-pl (Perl) at git f1ad6d43: all listed forms parse.
- Intended contract: The grammar is a parser construction target; the specification adds throw statements, bare rethrow, async tests, directives and bounds.
- Authority: Specification sections cited; the EBNF is incomplete rather than restrictive.
- Conformance expectation: Grammar contracts include these forms (GRAM.THROW-PANIC, GRAM.TEST-BLOCK, GRAM.PROJECTED-DIRECTIVE, GEN.BOUNDS).
- Resolution: **resolved**
- Contracts: `GRAM.THROW-PANIC`, `GEN.BOUNDS`
- Cases: `L01.gram.throw.forms`, `L01.gram.test.skipped-in-production`, `L05.generic.bound-satisfied`

## D-004

**Category of a bare failed `confirm`**

- Sources: `docs/okkama-spec-11072026.md section 9.4 ("uncatchable programming error")`; `docs/okkama-spec-11072026.md section 11.4 ("Without else, an unsuccessful condition panics")`; `docs/okkama-llm-context-1.0.md section 1 rule 9`
- Observed: okkama-pl (Perl) at git f1ad6d43: reported as `RuntimeError: confirm failed`, not catchable.
- Intended contract: A bare failed confirm is uncatchable; the documents disagree whether its category is panic or programming error.
- Authority: Both texts agree on uncatchability, which is what programs can observe.
- Conformance expectation: Expectations accept categories panic or runtime and require that no catch observes it.
- Resolution: **resolved for conformance (category naming remains a documentation inconsistency)**
- Contracts: `ERR.CONFIRM`
- Cases: `L07.confirm.bare-uncatchable`, `L07.destroy.failure-reported`

## D-005

**Disputed reserved words `optional`, `setters`, `mutable`, `var`**

- Sources: `docs/okkama-spec-11072026.md section 4.3 (lists them as not valid syntax; "code MUST NOT use them")`; `docs/okkama-grammar-1.0.md context restrictions (lists only isa, super, abstract, protected, null, none, nil, with)`
- Observed: okkama-pl (Perl) at git f1ad6d43: accepts all four as ordinary identifiers.
- Intended contract: `optional`, `setters`, `mutable` and `var` are ordinary identifiers.
- Authority: Language owner decision (19.10.2026): "no" (not reserved). Spec 4.3 must drop them from its list.
- Conformance expectation: Mandatory LEX.NONRESERVED-WORDS: the words may name locals, fields, parameters and actions.
- Resolution: **resolved**
- Contracts: `LEX.NONRESERVED-WORDS`
- Cases: `L01.lex.nonreserved-words`

## D-006

**Spurious secondary lexer diagnostic after an unknown escape**

- Sources: `okkama-pl/lib/Okkama/Lexer.pm`; `docs/okkama-spec-11072026.md section 4.2`
- Observed: okkama-pl (Perl) at git f1ad6d43: `"\q"` reports the unknown escape and then a spurious `Unterminated string` at a later line.
- Intended contract: An unknown escape is a lexer error.
- Authority: Specification 4.2.
- Conformance expectation: Diagnostic prose is not normative; the rejection itself is conforming. Recorded as an observation only.
- Resolution: **resolved (non-normative observation)**
- Contracts: `LEX.STRING-LITERALS`
- Cases: `L01.lex.string.unknown-escape`

## D-007

**Byte values as map keys**

- Sources: `docs/okkama-spec-11072026.md section 6.3 (byte listed as a map key; identity distinct from int)`; `docs/OkkamaBook.md section 5.3 ("Byte-valued map keys are currently unsupported")`; `docs/okkama-llm-context-1.0.md sections 8 and 11 (keys cannot be byte) versus section 4 (byte listed)`; `docs/okkama-spec-11072026.md section 13 (fake storage rejects byte keys)`
- Observed: okkama-pl (Perl) at git f1ad6d43: `[byte(1): "byte", 1: "int"]` works with distinct keys.
- Intended contract: byte values are not valid map keys.
- Authority: Language owner decision (19.10.2026): "no". Spec 6.3 must remove byte from the key list (the book and fake-storage rules already agree).
- Conformance expectation: Mandatory COLL.MAP-BYTE-KEYS: a byte key is rejected (statically or when evaluated). Perl accepts it: DEF-004.
- Resolution: **resolved**
- Contracts: `COLL.MAP-BYTE-KEYS`
- Cases: `L05.map.byte-keys-rejected`

## D-008

**Runtime support for `import ... as`**

- Sources: `docs/okkama-llm-context-1.0.md section 25 ("alias (as) imports parse but are not runtime-supported")`; `docs/okkama-spec-11072026.md section 5.2 (as replaces the namespace)`
- Observed: okkama-pl (Perl) at git f1ad6d43: aliases work, including `as` combined with `take`.
- Intended contract: `as` replaces the basename namespace; with take it contains only the selected names.
- Authority: Specification 5.2 (the LLM context status table is stale).
- Conformance expectation: Mandatory MOD.ALIAS cases.
- Resolution: **resolved**
- Contracts: `MOD.ALIAS`
- Cases: `L06.alias.namespace`, `L06.alias.with-take`

## D-009

**Decimal literal tokens**

- Sources: `docs/okkama-grammar-1.0.ebnf (`DECIMAL = implementation_defined_decimal_token`)`; `docs/okkama-spec-11072026.md section 4.2`; `docs/okkama-llm-context-1.0.md section 25`
- Observed: Not exercised.
- Intended contract: Decimal literals have no runtime semantics yet.
- Authority: Specification 4.2 and grammar.
- Conformance expectation: Contract LEX.DECIMAL-LITERALS is deferred; no mandatory expectation.
- Resolution: **deferred**
- Contracts: `LEX.DECIMAL-LITERALS`

## D-010

**Documents cite standard-library paths that are not in the active std/**

- Sources: `docs/okkama-spec-11072026.md sections 6.7 (`import "okkama" take Dump`), 6.9 (`import "regex"`), 14 (`test/more`)`; `docs/okkama-llm-context-1.0.md section 25 (socket/tcp, encoding/csv, test/more, http/headers, ...)`; `stdlib_DEPRECATED/`
- Observed: Those import paths do not exist under std/; std/ provides testing/*, runtime/debug, regex/* instead.
- Intended contract: The active standard library is std/.
- Authority: Task instruction: std/ is the active standard library; historical trees do not define behaviour.
- Conformance expectation: Conformance cases import only modules that exist in the audited std/ inventory (contracts/stdlib-inventory.json).
- Resolution: **resolved**

## D-011

**How the standard library is located**

- Sources: `docs/okkama-llm-context-1.0.md toolchain section ("stdlib resolves as ../stdlib relative to cwd")`; `okkama-pl/lib/Okkama.pm (OKKAMA_STDLIB_DIR, default /usr/local/lib/okkama/std)`
- Observed: The CLI uses OKKAMA_STDLIB_DIR or /usr/local/lib/okkama/std (a symlink to this repository's std/ on the audited host).
- Intended contract: The suite passes the selected library root explicitly in every adapter request.
- Authority: Implementation source.
- Conformance expectation: Adapters receive `stdlib_root`; the Perl adapter sets OKKAMA_STDLIB_DIR from it.
- Resolution: **resolved**

## D-012

**Detection stage of missing `take` names**

- Sources: `docs/okkama-spec-11072026.md section 5.2 ("Missing take names are runtime errors")`
- Observed: okkama-pl (Perl) at git f1ad6d43: the analyzer reports `Import ... has no declaration ...` before execution.
- Intended contract: Missing take names are module-resolution errors; no program code runs.
- Authority: Specification 5.2 plus the general rule that declaration-derived facts are checked before execution.
- Conformance expectation: Expectation accepts check or runtime stage with resolution/semantic category and empty stdout.
- Resolution: **resolved**
- Contracts: `MOD.TAKE`
- Cases: `L06.take.missing-name`, `L06.reexport.take-transitive-name`

## D-013

**Specification section 22 describes async as having no futures or scheduler**

- Sources: `docs/okkama-spec-11072026.md section 22 ("synchronous-contract async actions ... no future or scheduler runtime")`; `docs/okkama-spec-11072026.md section 18`
- Observed: okkama-pl (Perl) at git f1ad6d43: lazy futures, cooperative scheduler, spawn and channels are implemented.
- Intended contract: Section 18 (futures, await, spawn) governs.
- Authority: Specification 18 and docs/async-guide.md.
- Conformance expectation: L08 cases follow section 18.
- Resolution: **resolved**
- Contracts: `ASYNC.FUTURES`

## D-014

**"No cancellation, timeout, race or join" versus library support**

- Sources: `docs/okkama-spec-11072026.md section 18 (first version has no cancellation, timeout syntax, race, join)`; `std/async/cancellation.ok`; `std/async/timeout.ok`; `std/async/collect.ok`; `docs/async-guide.md`
- Observed: Cancellation, timeouts, races and joins exist as std library objects and Future.cancel().
- Intended contract: There is no dedicated syntax; the capabilities are library contracts.
- Authority: std/ modules and the async guide.
- Conformance expectation: Covered as STD.ASYNC.* contracts, not language syntax.
- Resolution: **resolved**
- Cases: `L08.cancellation.cooperative-token`, `L08.future.cancel-unstarted`

## D-015

**Diagnostic message formats**

- Sources: `docs/okkama-spec-11072026.md section 20`; `docs/okkama-llm-context-1.0.md section 24`
- Observed: Perl renders `Type: message` with stack frames; older documents show `Runtime error at line N: ...`.
- Intended contract: Diagnostic prose, stack traces and rendering are not normative.
- Authority: Task requirement and spec 20 (tooling projection).
- Conformance expectation: The suite compares only logical categories, stages and uncaught Error types; stderr is ignored for failing cases.
- Resolution: **resolved**

## D-016

**Ordinary int arithmetic beyond signed 64-bit**

- Sources: `docs/okkama-spec-11072026.md section 8.3`; `docs/bitwise-guide.md ("Это не новая общая спецификация переполнений")`; `docs/OkkamaBook.md section 5.4`
- Observed: okkama-pl (Perl) at git f1ad6d43: `9223372036854775807 + 1` evaluates to 9223372036854775808 (arbitrary precision).
- Intended contract: **Superseded by R01 (OIR-1 §23, 20.10.2026): `int` is an exact bigint; arithmetic never wraps; MIN % -1 is 0.** The earlier decision (19.10.2026, "same as in Go": signed 64-bit wrap) no longer applies.
- Authority: Language owner decision R01 (20.10.2026), replacing the decision of 19.10.2026.
- Conformance expectation: Mandatory VAL.INT-OVERFLOW with exact results. Perl represents only −2^63…2^64−1 and fails beyond it: DEF-005.
- Resolution: **resolved**
- Contracts: `VAL.INT-OVERFLOW`
- Cases: `L02.int.overflow-wraps`

## D-017

**Detection stage of a missing entry point**

- Sources: `docs/okkama-spec-11072026.md section 3`
- Observed: okkama-pl (Perl) at git f1ad6d43: `RuntimeError: Program requires main() declaration` before any program code.
- Intended contract: Executable runs require main; the stage is unspecified.
- Authority: Specification 3.
- Conformance expectation: Expectation accepts check or runtime stages.
- Resolution: **resolved**
- Contracts: `ENTRY.REQUIRED`
- Cases: `L00.entry.missing-main-fails`

## D-018

**Anonymous action parameter annotations**

- Sources: `docs/okkama-llm-context-1.0.md section 15 ("parameter annotations and variadic parameters are rejected")`; `docs/okkama-spec-11072026.md section 8.10.2 (parameter types are mandatory)`
- Observed: okkama-pl (Perl) at git f1ad6d43: parameter types are required; untyped parameters are rejected.
- Intended contract: Parameter types and an explicit result are mandatory.
- Authority: Specification 8.10.2.
- Conformance expectation: GRAM.ANON-ACTION cases.
- Resolution: **resolved**
- Contracts: `GRAM.ANON-ACTION`
- Cases: `L01.gram.anon.untyped-param`

## D-019

**`probe` fixture shorthand**

- Sources: `okkama-pl/t/README.md`; `okkama-pl/t/lib/Okkama/Test.pm`
- Observed: The Perl test helper rewrites a module-level `probe()` action into an object method and records its value with an interpreter subclass.
- Intended contract: Not language syntax; a test-fixture convenience.
- Authority: okkama-pl/t/README.md.
- Conformance expectation: The conformance suite never uses probe rewriting or internal value extraction; every case is a real program observed through public behaviour.
- Resolution: **resolved**

## D-020

**Untyped `has` declarations**

- Sources: `docs/okkama-grammar-1.0.ebnf (`has_decl = "has" attr_name_list has_type? has_default? ";"`)`; `docs/okkama-spec-11072026.md section 11.2`
- Observed: okkama-pl (Perl) at git f1ad6d43: `has count = 0;` parses.
- Intended contract: The grammar allows omitting the type; the specification does not define the field's runtime type.
- Authority: Grammar only.
- Conformance expectation: Only syntactic acceptance is mandatory (GRAM.HAS-DECL boundary); shorthand rejects untyped fields (spec 11.3).
- Resolution: **resolved (runtime typing of untyped fields not asserted)**
- Contracts: `GRAM.HAS-DECL`
- Cases: `L01.gram.has.untyped-with-default`, `L04.shorthand.untyped-field`

## D-021

**Residual mentions of the removed `allow` declaration**

- Sources: `docs/object-caps-0.0.1-ru.md ("Роли, allow и #projected сохраняют свои отдельные контракты")`; `stdlib_DEPRECATED/`; `research/ R13-R21 sources`; `docs/okkama-spec-11072026.md section 16`; `docs/allow-removal.md`
- Observed: okkama-pl (Perl) at git f1ad6d43: `allow` declarations are rejected by the parser.
- Intended contract: `allow` is removed with no compatibility mode (IMP-27).
- Authority: Specification 16 and the IMP-27 decision.
- Conformance expectation: Mandatory rejection cases; historical sources do not define behaviour.
- Resolution: **resolved**
- Contracts: `GRAM.REMOVED-SYNTAX`, `OBJ.NO-INHERITANCE`
- Cases: `L01.gram.removed.allow`, `L01.gram.removed.allow-target`

## D-022

**Experimental effects and export checking**

- Sources: `improvements-roadmap/results/IMP-25/RESULT.md`; `improvements-roadmap/results/IMP-26/RESULT.md`; `docs/qualified-export-prototype.md`; `docs/source-effect-prototype.md`
- Observed: Implemented as opt-in prototypes outside the main compiler.
- Intended contract: Not language contracts.
- Authority: IMP-25/IMP-26 results ("not included in the main compiler as mandatory policy").
- Conformance expectation: Excluded from the mandatory suite.
- Resolution: **resolved (excluded)**

## D-023

**Explicit runtime import directories**

- Sources: `docs/okkama-spec-11072026.md section 5.2 (lookup step 1: explicit runtime import directories)`; `okkama-pl/lib/Okkama.pm`
- Observed: The CLI exposes no option for import directories.
- Intended contract: Lookup is local-first: importer directory, then the standard library.
- Authority: Specification 5.2.
- Conformance expectation: Cases use only importer-relative and std/ lookup; harness modules are copied next to the program.
- Resolution: **resolved**
- Contracts: `MOD.RESOLUTION`

## D-024

**Match exhaustiveness and branch type compatibility**

- Sources: `docs/okkama-spec-11072026.md section 8.12`; `docs/okkama-llm-context-1.0.md section 7`
- Observed: Not checked.
- Intended contract: Future semantic-analysis work.
- Authority: Specification 8.12.
- Conformance expectation: Contract CTRL.MATCH-EXHAUSTIVENESS is deferred.
- Resolution: **deferred**
- Contracts: `CTRL.MATCH-EXHAUSTIVENESS`

## D-025

**Generic role bounds absent from the EBNF**

- Sources: `docs/okkama-spec-11072026.md section 6.5 (`object Heap<T: Comparing<T>>`)`; `docs/okkama-grammar-1.0.ebnf (`generic_params` has no bound)`
- Observed: okkama-pl (Perl) at git f1ad6d43: bounds parse and are checked after inference.
- Intended contract: One nominal role bound per object type parameter.
- Authority: Specification 6.5.
- Conformance expectation: GEN.BOUNDS cases.
- Resolution: **resolved**
- Contracts: `GEN.BOUNDS`
- Cases: `L05.generic.bound-satisfied`, `L05.generic.bound-violated`

## D-026

**Proposed backend contracts**

- Sources: `okkama-pl/bin/okkamac`; `okkama-pl/bin/okkama-ir`; `okkama-pl/compiler-draft/`; `docs/okkama-spec-11072026.md section 11.2.1 (OIR Profile 0 remains int-only)`
- Observed: An experimental IR/compiler path accepts only a narrow int profile.
- Intended contract: Backends must eventually satisfy the same language contracts; their temporary limits are not language rules.
- Authority: Task instruction (do not standardise temporary limits).
- Conformance expectation: Excluded from the mandatory suite; a compiled backend adapter would run the same cases.
- Resolution: **resolved (excluded)**

## D-027

**Unicode data versions**

- Sources: `std/regex/pattern.ok (Unicode 16.0.0)`; `std/unicode/normalization.ok (Unicode 16.0.0)`
- Observed: std pins Unicode 16.0.0; the suite's Python oracle uses an older unicodedata (13.0.0).
- Intended contract: Behaviour depends on the pinned 16.0.0 tables.
- Authority: std module headers.
- Conformance expectation: Unicode cases use code points whose properties are identical in both versions and require the `unicode-data` environment tag.
- Resolution: **resolved**
- Cases: `L09.unicode.normalization`, `L09.html.entities`

## D-028

**Grammar `for` patterns with several bindings or an index**

- Sources: `docs/okkama-grammar-1.0.ebnf (`pattern = BINDING_NAME ("," BINDING_NAME)* | BINDING_NAME "[" INT "]"`)`; `docs/okkama-spec-11072026.md section 9.9 ("Current runtime supports one binding")`
- Observed: Not exercised.
- Intended contract: Undocumented.
- Authority: None.
- Conformance expectation: No mandatory expectation; recorded as an undocumented grammar alternative.
- Resolution: **deferred**

## D-029

**Section 22 lists owner-only private fields as parser-only**

- Sources: `docs/okkama-spec-11072026.md section 22 ("Parser-supported but not fully runtime-specified: owner-only private fields")`; `docs/okkama-spec-11072026.md sections 11.2, 16`
- Observed: okkama-pl (Perl) at git f1ad6d43: field privacy is enforced at runtime and statically.
- Intended contract: Fields are private to their owner.
- Authority: Specification 11.2 and 16.
- Conformance expectation: Mandatory OBJ.FIELD-PRIVACY cases.
- Resolution: **resolved**
- Contracts: `OBJ.FIELD-PRIVACY`
- Cases: `L04.privacy.external-read`, `L04.privacy.peer-read`

## D-030

**CR LF line endings**

- Sources: `docs/okkama-spec-11072026.md section 4.2`
- Observed: okkama-pl (Perl) at git f1ad6d43: CRLF sources run.
- Intended contract: As in Go: CR is whitespace between tokens, so CRLF sources are accepted; CR characters inside fenced raw strings are discarded.
- Authority: Language owner decision (19.10.2026): "same as in other langs / Go". Spec 4.2 ("every character ... retained") must be amended for CR.
- Conformance expectation: Mandatory LEX.CRLF. Perl keeps CR in raw strings: DEF-006.
- Resolution: **resolved**
- Contracts: `LEX.CRLF`
- Cases: `L01.lex.crlf.line-endings`, `L01.lex.crlf.raw-string-discards-cr`

## D-031

**Sign of `%` with negative operands**

- Sources: `docs/okkama-spec-11072026.md section 8.3`
- Observed: okkama-pl (Perl) at git f1ad6d43: `-7 % 3` is 2 and `7 % -3` is -2 (floored, sign of the divisor).
- Intended contract: Truncated division as in Go: x % y has the sign of x.
- Authority: Language owner decision (19.10.2026): "same" (as Go).
- Conformance expectation: Mandatory VAL.INT-MODULO-SIGN. Perl uses floored modulo (-7 % 3 = 2): DEF-007.
- Resolution: **resolved**
- Contracts: `VAL.INT-MODULO-SIGN`
- Cases: `L02.int.modulo-sign`

## D-032

**Canonical text of floats**

- Sources: `docs/okkama-spec-11072026.md section 6.6 ("canonical numeric text")`
- Observed: Perl prints integral floats without `.0`; non-terminating values, exponents, signed zero and non-finite values are not exercised.
- Intended contract: Float text is Go's strconv.FormatFloat(x, 'g', -1, 64): shortest round-trip digits, exponent notation when the decimal exponent is < -4 or >= 6, IEEE negative zero as -0. Non-finite values are not representable in the finite float contract.
- Authority: Language owner decision (19.10.2026): proposal accepted ("ок").
- Conformance expectation: Mandatory VAL.FLOAT-TEXT cases; expected strings computed with go1.26.1 using runtime (not constant-folded) values.
- Resolution: **resolved**
- Contracts: `VAL.FLOAT-TEXT`
- Cases: `L02.float.text-shortest-round-trip`, `L02.float.text-exponent-and-zero`

## D-033

**Whitespace set of trim() and CR handling of chomp()**

- Sources: `docs/okkama-spec-11072026.md section 6.6`
- Observed: Basic ASCII space/LF behaviour observed.
- Intended contract: trim() removes leading and trailing Unicode White_Space as Go strings.TrimSpace does; chomp() removes exactly one trailing CRLF or LF.
- Authority: Language owner decision (19.10.2026): proposal accepted ("ок").
- Conformance expectation: Mandatory VAL.STRING-WHITESPACE case.
- Resolution: **resolved**
- Contracts: `VAL.STRING-WHITESPACE`
- Cases: `L02.string.trim-whitespace-set`

## D-034

**Exit status after an unobserved spawned failure**

- Sources: `docs/async-guide.md section 6.4 ("ненаблюдённая ошибка не теряется и сообщается пользователю")`
- Observed: okkama-pl (Perl) at git f1ad6d43: the error is rendered on stderr and the run fails (nonzero status).
- Intended contract: An unobserved spawned failure is reported and the run fails with a nonzero status.
- Authority: Language owner decision (19.10.2026): proposal accepted ("ок").
- Conformance expectation: Mandatory ASYNC.UNOBSERVED-FAILURE case.
- Resolution: **resolved**
- Contracts: `ASYNC.SHUTDOWN`, `ASYNC.UNOBSERVED-FAILURE`
- Cases: `L08.shutdown.unobserved-failure`

## D-035

**Field defaults and construction order**

- Sources: `docs/okkama-spec-11072026.md section 11.3 ("Field defaults are evaluated before the constructor body")`
- Observed: okkama-pl (Perl) at git f1ad6d43: a default with an observable effect is evaluated even when the caller supplies the field.
- Intended contract: Before the constructor runs, self is filled with every field default. The constructor (shorthand bindings, then the body) binds the supplied values, replacing defaults. Afterwards every field must be bound, otherwise construction fails at runtime.
- Authority: Language owner decision (19.10.2026), refined the same day: defaults are always evaluated first.
- Conformance expectation: Mandatory OBJ.CONSTRUCTION-ORDER: defaults are evaluated even for supplied fields, a full or shorthand body sees the default or supplied value, and an unbound field after the constructor dies at runtime. The earlier reading (default skipped when supplied) and its defect DEF-008 are withdrawn.
- Resolution: **resolved**
- Contracts: `OBJ.CONSTRUCTION-ORDER`
- Cases: `L04.fields.default-evaluated-then-replaced`, `L04.ctor.full-body-sees-default`, `L04.shorthand.body-sees-supplied-value`, `L04.ctor.unbound-field-dies-after-body`

## D-036

**Iteration protocol names**

- Sources: `docs/okkama-spec-11072026.md sections 6.8 and 9.9 (`Iterable<T>`, `Cursor<T>` with current()/advanced())`; `std/collections/iteration.ok (`Iterating<T>`, `Cursor<T>` with item()/successor())`; `docs/std/stdlib-roadmap-v4.md`
- Observed: okkama-pl (Perl) at git f1ad6d43: recognises the std roles Iterating/Cursor with available/item/successor.
- Intended contract: Nominal iteration through the standard roles of the active std.
- Authority: The active std/ module plus roadmap v4; the specification text is stale.
- Conformance expectation: COLL.ITERABLE cases use Iterating/item/successor.
- Resolution: **resolved**
- Contracts: `COLL.ITERABLE`
- Cases: `L05.iterable.for-protocol-order`

## D-037

**Ordering relation of map keys**

- Sources: `docs/okkama-spec-11072026.md sections 6.6, 6.8 ("sorted by key")`; `docs/okkama-spec-11072026.md section 6.3 (key identity before host normalisation)`
- Observed: okkama-pl (Perl) at git f1ad6d43: int keys are ordered by their decimal text (`-1,10,100,9`).
- Intended contract: keys()/values() are sorted by key; for int keys the natural (numeric) order.
- Authority: Specification 6.6 read with the key-identity rule of 6.3.
- Conformance expectation: Mandatory COLL.MAP case expects numeric order; Perl fails it (DEF-003). The specification should state the order relation per key category.
- Resolution: **resolved (with a documentation request)**
- Contracts: `COLL.MAP`
- Cases: `L05.map.int-key-order`

## D-038

**Async native declarations**

- Sources: `docs/okkama-spec-11072026.md section 5.3 (async is not a supported native form)`; `docs/okkama-grammar-1.0.md context restrictions`; `docs/okkama-spec-11072026.md section 11.6 (async is supported for native declarations)`; `docs/okkama-grammar-1.0.ebnf (`native_action_decl = "native" async_mark? ...`)`; `std/runtime/*.ok (42 `native async` declarations)`
- Observed: okkama-pl (Perl) at git f1ad6d43: accepts `native async` declarations.
- Intended contract: Async native declarations are valid.
- Authority: Specification 11.6, the grammar production and active std usage.
- Conformance expectation: Async natives are accepted (NATIVE.DECL normal).
- Resolution: **resolved**
- Contracts: `NATIVE.DECL`
- Cases: `L06.native.decl-valid-forms`

## D-039

**Channel API names**

- Sources: `docs/okkama-spec-11072026.md section 18.2 and docs/okkama-llm-context-1.0.md (`Sender<T>`, `Receiver<T>`, `receive`)`; `std/async/channel.ok (`Sending<T>`, `Receiving<T>`, `item()`)`
- Observed: okkama-pl (Perl) at git f1ad6d43: implements the std names.
- Intended contract: The active std/ module API.
- Authority: std/ is the active library.
- Conformance expectation: L08 channel cases use Sending/Receiving/item.
- Resolution: **resolved**
- Contracts: `STD.ASYNC.CHANNEL`
- Cases: `L08.spawn.eager-start-and-channel`, `L08.channel.close-semantics`

## D-040

**Channel capacity zero**

- Sources: `docs/okkama-spec-11072026.md section 18.2 and docs/okkama-llm-context-1.0.md (capacity must be positive)`; `std/async/channel.ok (capacity zero is an unbuffered rendezvous; negative throws)`
- Observed: okkama-pl (Perl) at git f1ad6d43: capacity zero works as a rendezvous.
- Intended contract: Capacity zero is valid; negative capacities throw InvalidChannelCapacity.
- Authority: The active std/ module.
- Conformance expectation: Mandatory rendezvous and negative-capacity cases.
- Resolution: **resolved**
- Contracts: `STD.ASYNC.CHANNEL`
- Cases: `L08.channel.rendezvous`, `L08.channel.negative-capacity`

## D-041

**`Cancelled` as a catch type**

- Sources: `docs/okkama-spec-11072026.md section 9.6 (catch types: Error, concrete Error objects, NativeFailure)`; `docs/async-guide.md section 4.5`; `std/net/*.ok (`catch e is Cancelled`)`
- Observed: okkama-pl (Perl) at git f1ad6d43: `catch e is Cancelled` works.
- Intended contract: The builtin Cancelled is catchable.
- Authority: Async guide and std usage.
- Conformance expectation: L08 cases catch Cancelled.
- Resolution: **resolved**
- Cases: `L08.future.cancel-unstarted`, `L12.cancellation-runs-cleanup`

## D-042

**Regex API**

- Sources: `docs/okkama-spec-11072026.md section 6.9 (`import "regex" take Regex, Match`; no named groups)`; `std/regex/*.ok (Dialect 1 with named groups; FirstRegexMatch, RegexSubstitution, RegexParts)`
- Observed: okkama-pl (Perl) at git f1ad6d43: implements the std regex/* API.
- Intended contract: The active std/regex modules.
- Authority: std/ is the active library.
- Conformance expectation: L09 regex cases follow std/regex/*.
- Resolution: **resolved**
- Contracts: `STD.REGEX.PATTERN`
- Cases: `L09.regex.search-replace-split`

## D-043

**Builtin `order()` on scalars**

- Sources: `std/compare.ok (int, float, rat, byte, string and char implement Comparing<Self>)`; `docs/okkama-spec-11072026.md section 6.6 (builtin list omits order)`
- Observed: okkama-pl (Perl) at git f1ad6d43: `1.order(2) == Order.less` works.
- Intended contract: Natural builtin orderings implement compare.Comparing.
- Authority: std/compare.ok.
- Conformance expectation: L09 compare cases.
- Resolution: **resolved**
- Contracts: `STD.COMPARE`
- Cases: `L09.compare.builtin-order`

## D-044

**Formatting API**

- Sources: `docs/okkama-llm-context-1.0.md section 23 (`string Formatted("{}: {}", "Ada", 3)` with any values)`; `std/strings/format.ok (`Formatted(:pattern, :values)` over @string)`
- Observed: okkama-pl (Perl) at git f1ad6d43: implements the std API.
- Intended contract: The active std/strings/format module.
- Authority: std/ is the active library.
- Conformance expectation: L09 strings.formatted case.
- Resolution: **resolved**
- Contracts: `STD.STRINGS.FORMAT`
- Cases: `L09.strings.formatted`

## D-045

**Canonical spelling of Decimal values**

- Sources: `std/math/decimal.ok (two examples: 001.2000 -> 1.2, 1000 -> 1e3; the rule is the native decimal_canonical)`
- Observed: okkama-pl (Perl) at git f1ad6d43: `Decimal("-0.50").string()` is `-5e-1`; large positional values print in scientific form.
- Intended contract: Equivalent spellings share one canonical form; the general rule is not documented.
- Authority: Language owner decision (19.10.2026): decimals are not a language feature yet.
- Conformance expectation: VAL.DECIMAL-SPELLING is deferred; std/math/decimal values are compared by value.
- Resolution: **deferred**
- Contracts: `VAL.DECIMAL-SPELLING`
- Cases: `L09.decimal.canonical`, `L11.sqlite.exact-large-values`, `L12.exact-number-json-sqlite`

## D-046

**Implicit promotion between numeric categories**

- Sources: `docs/okkama-spec-11072026.md sections 6.1, 8.3, 8.4, 8.6, 8.7 (byte promotes to int; int promotes to float in float arithmetic; ordering accepts mixed integer categories and exact rat; byte indexes and range bounds)`; `docs/okkama-llm-context-1.0.md section 6`; `docs/compiler/OIR-1.md decision R02`
- Observed: okkama-pl (Perl) at git f1ad6d43 promotes silently: `1 < 1.5`, `1.5 + 1`, `byte(200) + 100`, `byte(3) > 2`, `rat(1, 2) + 1`, `xs[byte(1)]` and `[byte(1)..=3]` all run.
- Intended contract: Different numeric categories (int, byte, float, rat) interact only after an explicit cast: float(n), b.int(), n.rat(), x.float(). Mixed arithmetic, ordering, array indexes and range bounds without a cast are errors. Operations inside one category keep their rules (byte + byte and -byte give int, int / int gives rat).
- Authority: Language owner decision (21.10.2026): «нет, только явное преобразование, никаких штучек. если перл тут пропускает -- это ошибка перл-версии. концептуально разные типы могут взаимодействовать только после явного каста».
- Conformance expectation: Positive cases convert explicitly; mandatory negative cases reject each mixed form (detection stage left open as for other category errors). The specification sections listed above still describe promotion and need edits.
- Resolution: **resolved**
- Contracts: `VAL.ORDERING`, `VAL.BYTE`, `VAL.FLOAT`, `VAL.RAT`, `VAL.RANGES`, `VAL.INT-ARITH`, `COLL.ARRAY`
- Cases: `L02.ordering.numeric`, `L02.float.arithmetic`, `L02.int.byte-promotion`, `L02.byte.compare-and-index`, `L02.rat.arithmetic`, `L05.array.access`, `L13.invoice-calculation`, `L02.ordering.int-float-rejected`, `L02.ordering.byte-int-rejected`, `L02.float.plus-int-rejected`, `L02.byte.plus-int-rejected`, `L02.rat.plus-int-rejected`, `L02.range.byte-bound-rejected`, `L05.array.byte-index-rejected`, `L01.gram.precedence.associativity`, `L02.rat.division`

## D-047

**Map literals with keys of different types**

- Sources: `docs/okkama-spec-11072026.md section 6.3 ("Key identity includes its exact type: byte(1), 1, 1.0, true and \"1\" do not alias")`; `docs/okkama-spec-11072026.md section 6.2 (map keys may not be any)`
- Observed: okkama-pl (Perl) at git f1ad6d43 builds [1: "int", "1": "string"] as one map with two keys of different types.
- Intended contract: A map has exactly one key type (%K:V). Keys of different types in one literal are an error: the only common type would be any, and any is not a map key. Key identity still includes the exact type, so different maps keyed by 1 and "1" stay distinct.
- Authority: Language owner decision (21.10.2026): «у нас мапы строго тип:тип, ключи разных типов -- ошибка. можно только any, но any у нас не распаковывается в программе, только на слое нейтивов, поэтому нет, типы строго одно и то же. специфика перла пролезла».
- Conformance expectation: L05.map.key-types keeps only single-type keys; L05.map.mixed-key-types-rejected requires rejection.
- Resolution: **resolved**
- Contracts: `COLL.MAP`
- Cases: `L05.map.key-types`, `L05.map.mixed-key-types-rejected`
