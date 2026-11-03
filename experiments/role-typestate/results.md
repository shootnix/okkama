# Results

Run: `OKKAMA=<okkama built from the working tree> ENGINES="ir jvm sparrow" ./run.sh`
on 2026-11-03, branch `nom`, okkama 0.3.4, after the compiler fixes listed in
[Defects found and fixed](#defects-found-and-fixed).

```text
passed: 91  failed: 0  (okkama: okkama 0.3.4, engines: ir jvm sparrow)
```

The first run (before the fixes, unchanged compiler at `57d8b0aa`) had 92
programs: four reproduced defects as `blocked`. Three of them now expect a
`check-error` or `run-ok`, and one (`c_successor_tail_ignored`) was removed
because the `^Role` rule now rejects it before its tail is reached.

"passed" means every program behaved as its `EXPECT` header says, including
reproduced defects and violations that remain possible. All `run-ok` and
`runtime-error` programs behave identically on the IR interpreter, JVM and
Sparrow. Static results come from the shared front end (`okkama check`).

Legend for the "class" column (task §10):

1. compile time — rejected by `okkama check`;
2. runtime — compiles, fails when executed;
3. API design — prevented only by how the model is exposed (imports, factory);
4. possible — compiles and runs;
5. external — involves systems no static check can see.

## Capability matrix

| Capability | Status | Evidence | Limitations |
|---|---|---|---|
| Role-restricted methods | SUPPORTED | `negative/b1_*`, `negative/b2_*`, `advanced/b_alias`, `advanced/b_param_role`, `advanced/b_generic_box` | Restricts the reference, not the object; concrete references see everything (`advanced/b_concrete_full_access`) |
| Successor returning concrete state type | PARTIALLY | `positive/b2_views_lifecycle`, `negative/c_successor_other_class` | Only `^Self`; a successor can never produce another class |
| Successor returning role type | PARTIALLY | Through a role signature (`role Created { authorize() is ^Authorized; }`) with a `^Self` implementation: `positive/b2_views_lifecycle`. Directly on an object: `negative/c_successor_role_direct` | Object-level `is ^Role` is rejected statically (before the fix: internal compiler error) |
| Static rejection of invalid method calls | SUPPORTED | 17 `negative/a_*`/`b1_*`/`b2_*` programs, `negative/d_*`, `negative/e_*` | Only for calls through the restricted type; Pattern C (ADT state) checks at runtime only |
| Alternative transitions through ADTs | SUPPORTED | `positive/d1_outcomes`, `advanced/d_nested` | Outcome values have no must-use obligation (`advanced/d_discard_outcome`) |
| Exhaustive pattern matching | SUPPORTED for payload/generic enums; PARTIALLY for payload-free enums | `negative/d_missing_variant`, `negative/d_statement_missing_variant`, `negative/d_compact_missing`; `advanced/d_plain_enum_nonexhaustive` | Statement `match` on a payload-free non-generic enum is not an error (registry.rs:695); `okkama lint` warns (`non-exhaustive-match`) |
| Role-typed generic ADT payloads | SUPPORTED | `advanced/d_generic_outcome`, `negative/d_generic_outcome_hidden` | — |
| Maybe/Presence with role types | SUPPORTED | `advanced/e_maybe_role`, `advanced/e_presence_role`, `negative/e_*` | Useful for boundary data, not for state |
| Prevention of old-state reuse | UNSUPPORTED | `advanced/c_reuse_old_state`, `advanced/c_rebind_concrete` | Immutable values fork history; no affine use |
| Prevention of duplicate transitions | UNSUPPORTED | `advanced/c_reuse_old_state`, `advanced/c_reuse_old_state_classes`, `advanced/g_double_authorize` | The in-memory runtime stage check cannot see it either |
| Preservation of state through generic functions | SUPPORTED (role type, also through a bounded generic) / UNSUPPORTED (must-use) | `advanced/b_generic_box`, `negative/f_bounded_generic_hidden`, `positive/f_bounded_generic_role`, `advanced/g_generic_launder` | Must-use is lost through `T` |
| Safe role conversions | PARTIALLY | `advanced/b_role_to_role_widen`, `advanced/b_role_to_concrete`, `advanced/b_any_roundtrip`, `advanced/b_narrow_ok` | Concrete → any implemented role (`advanced/b_concrete_to_wrong_role`); throw/catch recovers the concrete object (`advanced/b_throw_catch_recover`) |
| Must-use of transition results | SUPPORTED for successors; UNSUPPORTED for ordinary actions | `negative/c_discard_successor`, `negative/c_unused_successor_binding`, `negative/c_branch_unused_successor`, `negative/g_bound_successor`; `advanced/c_discard_ordinary_transition` | Any use discharges it (`advanced/c_must_use_weak`) |
| Prevention of state forging by construction | SUPPORTED for a private stage field (Patterns B2/D); UNSUPPORTED for class-per-state and ADT state | `advanced/b_forge_state_ctor`; `advanced/b_forge_state_class`, `advanced/c_adt_forge_state` | Module hiding is only a `take` convention (`advanced/b_take_hides_class`) |

## Experiment A — minimal transitions

Valid sequences (all engines): `positive/a1_classes_lifecycle` (Pattern A),
`positive/b1_roles_lifecycle` (B1), `positive/b2_views_lifecycle` (B2).

| Invalid call | Pattern A (classes) | Pattern B1 (roles + classes) | Pattern B2 (one class, role views) |
|---|---|---|---|
| Created → capture() | 1: `Object "Created" has no action "capture"` | 1: `Role "Created" has no action "capture"` | 1: same |
| Created → refund() | 1: `Object "Created" has no action "refund"` | 1: `Role "Created" has no action "refund"` | 1: same |
| Authorized → authorize() | 1: `Object "Authorized" has no action "authorize"` | 1: `Role "Authorized" has no action "authorize"` | 1: same |
| Captured → authorize() | 1: `Object "Captured" has no action "authorize"` | 1: `Role "Captured" has no action "authorize"` | 1: same |
| Refunded → capture() | 1: `Object "Refunded" has no action "capture"` | 1: `Role "Refunded" has no action "capture"` | 1: same |
| Declined → capture() | 1: `Object "Declined" has no action "capture"` | 1: `Role "Declined" has no action "capture"` | (D) `negative/d_declined_capture`: 1 |

All negative programs use correct syntax; each differs from a passing
positive program only in the invalid call. In B2 the checker also reports
`Successor action result must be used; binding "..."` as a follow-up: the
poisoned call no longer consumes the successor binding.

## Experiment B — interface restriction vs state restriction

| Program | Class | Result |
|---|---|---|
| `advanced/b_alias` | 1 | `Role "Created" has no action "capture"` |
| `advanced/b_param_role` | 1 | `Role "Authorized" has no action "refund"` (argument may be the full class) |
| `advanced/b_generic_box` | 1 | `Role "Created" has no action "capture"` after `Box<T>.get()` |
| `advanced/b_role_to_role_widen` | 1 | `Cannot initialize variable "widened" of type Authorized with Created` |
| `advanced/b_role_to_concrete` | 1 | `Cannot initialize variable "full" of type Payment with Created` |
| `advanced/b_any_roundtrip` | 1 | `Cannot initialize variable "again" of type Authorized with any` |
| `advanced/b_any_call` | 1 | `Can only call actions on objects` |
| `advanced/b_narrow_ok` | — | narrowing to a subset role is allowed (runs) |
| `advanced/b_forge_state_ctor` | 1 | `No constructor of "Payment" accepts (key: string, stage: Stage)` |
| `advanced/b_take_hides_class` | 3 | `Undefined action "SettledCharge"` with `take` import only |
| `advanced/b_concrete_full_access` | 2 | `InvalidTransition: invalid transition: capture` (runtime `confirm`) |
| `advanced/b_concrete_to_wrong_role` | 2 | `InvalidTransition: invalid transition: capture` |
| `advanced/b_forge_state_class` | 4 | B1: `SettledCharge(key: "forged").refund()` runs |
| `advanced/b_throw_role_rejected` | 1 | `Type "Created" cannot be thrown; only error enums and objects that do Error are catchable` |
| `advanced/b_throw_catch_recover` | 4 | role → `Error` → `throw` → `catch e is Payment` recovers hidden `capture()` |

Guarantees: **interface restriction** — yes, static, for role references.
**State restriction** — only at runtime, and only in models that keep the
state in a private field checked by `confirm` (B2/D). **Transition
consumption** — no.

## Experiment C — successor semantics

| Question | Answer | Evidence |
|---|---|---|
| Different concrete class? | No (1) | `negative/c_successor_other_class`: `Successor action "authorize" on object "Created" must return ^Created or a role it does`; `negative/c_successor_replacement`: `A successor action cannot return a replacement value; use bare \`return;\`` |
| Role instead of class? | Through role signatures yes; directly on an object no (1) | `positive/b2_views_lifecycle`; `negative/c_successor_role_direct`: `Successor action "authorize" on object "Payment" must return ^Payment; declare ^Authorized in a role signature and implement it as ^Payment`; `negative/c_successor_role_not_done` |
| Successor satisfies a role contract? | Yes, `^Self` where Self does the target role | `positive/b2_views_lifecycle`; `negative/c_role_successor_self_not_role`, `negative/c_role_successor_plain_impl`: `... must return ^Authorized for role "Created", got ^Fresh` / `got Authorized` |
| Object implementing several roles? | Yes; through the concrete type all actions are visible | `advanced/c_rebind_concrete`, `advanced/b_concrete_full_access` |
| Declared type retained? | Yes, `^Authorized` through roles, chained inference | `negative/b2_authorized_authorize` etc. |
| Rebinding preserves restrictions? | Role-typed locals cannot be rebound at all; concrete locals can | `negative/c_rebind_across_states`: `Cannot rebind variable "payment": role type "Created" cannot be rebound; bind the next state to a new name`; `advanced/c_rebind_concrete` |
| Old object usable after successor? | Yes (4) | `advanced/c_reuse_old_state` |
| Several successors from one object? | Yes (4) | `advanced/c_reuse_old_state`, `advanced/c_reuse_old_state_classes` |
| Successor silently discarded? | No (1), path-sensitive, also via bound action values | `negative/c_discard_successor`, `negative/c_unused_successor_binding`, `negative/c_branch_unused_successor`, `negative/g_bound_successor` |
| Ordinary-action transition discarded? | Yes (4) | `advanced/c_discard_ordinary_transition` |
| Strength of must-use | Any use discharges it (receiver call, boxing into `any`) | `advanced/c_must_use_weak` |
| Concrete vs role | Concrete: `^Payment`, all actions; role: `^Authorized`, restricted | `advanced/c_rebind_concrete` vs `negative/b2_*` |
| Tail value in a successor body | A value of the successor's own type is rejected (1); other values are discarded as in void actions | `negative/c_successor_tail_replacement`: `A successor action cannot return a replacement value; its final expression would be discarded` (before the fix it ran and printed `1`) |

## Experiment D — ADT outcomes

| Program | Class | Result |
|---|---|---|
| `positive/d1_outcomes` | — | three outcomes, each branch with a different state view |
| `negative/d_approved_refund` | 1 | `Role "Authorized" has no action "refund"` |
| `negative/d_declined_capture` | 1 | `Role "Declined" has no action "capture"` |
| `negative/d_missing_variant` | 1 | `Non-exhaustive match: missing failed` |
| `negative/d_statement_missing_variant` | 1 | `Non-exhaustive match: missing declined` |
| `negative/d_compact_missing` | 1 | `Non-exhaustive match: missing failed` (compact `?? / !!`) |
| `negative/d_wrong_payload_role` | 1 | `Variant payload must be Authorized, got Created` |
| `negative/d_payload_wrong_variant_arity` | 1 | `Wrong number of variant pattern bindings` |
| `advanced/d_generic_outcome` / `negative/d_generic_outcome_hidden` | — / 1 | `Outcome<Authorized>` keeps the role; `Role "Authorized" has no action "refund"` |
| `advanced/d_nested` | — | nested `Settlement`/`Authorization` matches |
| `advanced/d_discard_outcome` | 4 | outcome ADT discarded silently |
| `advanced/d_plain_enum_nonexhaustive` | 4 | statement `match` over payload-free enum misses a case and runs; `okkama lint`: `warning[non-exhaustive-match]: match over enum \`Stage\` does not handle captured; add the cases or an \`else\` branch` |
| `advanced/d_plain_enum_expr_nonexhaustive` | 1 | `Expression does not produce a value` (not an exhaustiveness message) |

## Experiment E — Maybe, Presence, `?? / !!`

| Program | Result |
|---|---|
| `advanced/e_maybe_role` | `Maybe<Authorized>`: `else throw` and compact choice keep `Authorized` |
| `advanced/e_presence_role` | `Presence<Authorized>`: three-way choice; `fold()` merges missing/none |
| `advanced/e_token` | optional token (`Maybe<string>`) decides the outcome at the boundary |
| `negative/e_maybe_role_hidden` | `Role "Authorized" has no action "refund"` |
| `negative/e_choice_branch_hidden` | `Role "Authorized" has no action "authorize"` |
| `negative/e_maybe_wrong_state` | `Generic parameter "T" for object "Maybe" cannot be both Authorized and Created` |
| `negative/e_presence_else` | `Optional fallback requires Maybe<T>; fold Presence explicitly` |
| `negative/e_choice_widen` | `Branches produce different types: Created and ^Authorized` |

## Experiment F — patterns

`positive/a1_classes_lifecycle` (A), `positive/b1_roles_lifecycle` (B1),
`positive/b2_views_lifecycle` (B2), `positive/c1_adt_lifecycle` (C),
`positive/d1_outcomes` and `positive/recommended_payment` (D).
Pattern C: `advanced/c_adt_capture_created` (2,
`InvalidTransition: invalid transition: capture`), `advanced/c_adt_forge_state` (4).

## Experiment G — adversarial (Pattern D unless noted)

| Attack | Class | Program / result |
|---|---|---|
| Transition before prerequisite (role) | 1 | `negative/b2_created_capture` |
| Transition before prerequisite (concrete) | 2 | `advanced/g_concrete_skip`: `invalid transition: capture` |
| Repeat a transition | 4 / 5 | `advanced/g_double_authorize`, `advanced/c_reuse_old_state` |
| Reuse an old immutable value | 4 | `advanced/c_reuse_old_state`, `advanced/c_rebind_concrete` |
| Operations through another role | 1 | `advanced/b_role_to_role_widen` |
| Widen to a less restrictive type | 1 / 2 | `advanced/b_role_to_concrete` (1); `advanced/g_wrong_view` (concrete → wrong view, 2: `invalid transition: code`) |
| Downcast to recover hidden operations | 1 / 4 | no cast syntax (`advanced/b_any_roundtrip`); throw/catch route (`advanced/b_throw_catch_recover`) |
| Multiple references to one entity | 4 | `advanced/b_concrete_to_wrong_role` (2 at use), `advanced/c_reuse_old_state` |
| Incorrect successor/result type | 1 | `negative/g_wrong_result_state`: `Action result must be Authorized, got Created`; `negative/c_role_successor_*` |
| Through a generic | 1 / 4 | `advanced/b_generic_box`, `negative/f_bounded_generic_hidden`; `advanced/g_generic_launder` |
| Lose state through an ADT | 1 | `negative/d_wrong_payload_role`, `negative/d_generic_outcome_hidden` |
| Combine incompatible entities | 4 | `advanced/g_mixed_entities` (capture of `a` paired with refund of `b`) |
| Transition after external failure | 5 | `advanced/g_retry_after_failure` (retry allowed by design) |
| Internal transition helper | 1 | `negative/g_internal_successor_leak`: `Action "approve" on object "Payment" is private` (before private successors: public, class 4) |

## Defects found and fixed

All four were classified BLOCKED in the first run and fixed in the compiler
afterwards (CHANGELOG, Unreleased → Fixed).

| Program | Before the fix | Now |
|---|---|---|
| `negative/c_successor_role_direct` | checker accepted object-level `is ^Role`; `internal compiler error: lowering produced invalid IR: @Payment.authorize: a successor action must return ^ of its object` | `check-error`: `... must return ^Payment; declare ^Authorized in a role signature and implement it as ^Payment` |
| `negative/c_successor_role_not_done` | same internal compiler error, for a role the object does not do | `check-error`: `... must return ^Created ...` |
| `negative/c_successor_tail_replacement` | tail value of the successor's type silently discarded (ran, printed `1`) | `check-error`: `A successor action cannot return a replacement value; its final expression would be discarded` |
| `positive/f_bounded_generic_role` | `lowering error: IR lowering of builtin role adapters (arrays and scalars as roles) is not implemented yet.` | `run-ok` on IR, JVM and Sparrow |

Regression coverage: conformance cases `L04.successor.object-role-result`,
`L04.successor.tail-replacement`, `L05.generic.bound-role-argument`; unit
tests in `okkama-compiler/tests/lowering_objects.rs` and `ide/tests.rs`.

## Other observations

- `okkama fmt` reflows multi-line payload enums by breaking inside a
  payload (`unavailable(\n    detail is string\n)`), so the experiments keep
  hand formatting.
- `docs/okkama-llm-context-1.0.md` §1 and §16 called a discarded successor a
  runtime error; the Rust front end rejects it statically (`analysis/flow.rs`).
  The context pack is corrected.
