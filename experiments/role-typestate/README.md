# Role-oriented typestate experiments

Research question: can Okkama express typestate (state-dependent behaviour,
static rejection of invalid transitions) with the existing roles, successors,
ADTs and type checker, without language changes?

Report: [docs/research/role-oriented-typestate.md](../../docs/research/role-oriented-typestate.md).
Results and exact diagnostics: [results.md](results.md).

## Layout

| Path | Contents |
|---|---|
| `models/` | Library modules shared by the experiments (one per modelling pattern) |
| `positive/` | Programs that must compile and run; `recommended_payment.ok` is the self-contained recommended pattern |
| `negative/` | Programs the checker must reject, each for one documented semantic reason |
| `advanced/` | Adversarial and boundary probes: runtime-only rejections, violations that remain possible |
| `run.sh` | Runs everything and compares the outcome with each file's `// EXPECT:` header |
| `logs/` | Exact `check`/run output of the last `run.sh` (regenerated, ignored by git) |

Models:

| Module | Pattern |
|---|---|
| `models/payment_classes.ok` | A: one concrete class per state, transitions are ordinary actions |
| `models/payment_roles.ok` | B1: one role and one class per state, transitions return the next role |
| `models/payment_views.ok` | B2: one class does every state role; role transitions are successors `^NextRole` |
| `models/payment_adt.ok` | C: the state is one ADT value; operations match and throw |
| `models/payment_hybrid.ok` | D: B2 plus a payload ADT stage and ADT outcomes (recommended) |
| `models/payment_ident.ok` | Small model for generic experiments |

## Expectation headers

Every program starts with a comment describing the property it tests and one
`// EXPECT:` line:

```text
// EXPECT: run-ok                      check passes, every engine exits 0
// STDOUT: line1|line2                 optional expected stdout
// EXPECT: check-error: TEXT           `okkama check` fails and prints TEXT
// EXPECT: runtime-error: TEXT         check passes, the run fails with TEXT
// EXPECT: blocked: TEXT               known implementation defect, reproduced
```

`run-ok` in `advanced/` usually means "the violation is still possible":
the header comment says so explicitly.

## Running

```sh
experiments/role-typestate/run.sh                       # installed okkama, IR engine
OKKAMA=path/to/okkama ENGINES="ir jvm sparrow" experiments/role-typestate/run.sh
```

`OKKAMA_STDLIB_DIR` defaults to this repository's `std/`. Imports of the
models use relative paths (`import "../models/payment_views" ...`), resolved
from the importing file's directory.

The recorded results were produced on branch `nom` (commit `57d8b0aa`,
okkama 0.3.4 built from that tree): branch `develop` does not yet contain
payload enums, `Maybe`/`Presence` or compact `?? / !!` choices.
