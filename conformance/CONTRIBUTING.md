# Authoring and contribution guide

## Before writing a case

1. Find the contract in `contracts/*.json`. If the behaviour has no contract,
   add one with a precise title, `spec_refs` and an explicit requirement for all
   four aspects (`required` or `n/a: <why>`).
2. Check `contracts/DISCREPANCY-LEDGER.md`. If the sources disagree, add or
   update a ledger entry (in `discrepancy-ledger.json`, then
   `python3 tools/render_ledger.py`). A disagreement without an authority is
   `unresolved`: write the case with `"status": "unresolved"` and cite the entry
   in `notes`; it runs in the `informative` profile and never counts.
3. Decide the expected observation **from the authority**, not from an
   implementation. Record how in `provenance.expected_basis`: the spec sentence,
   an exact calculation, a published test vector, or a controlled fixture. Run
   the implementation only afterwards; when it disagrees, investigate which side
   is wrong.

## Writing the program

* Use a real module declaration and a real entry point. Never rely on test
  helper rewriting (such as the Perl `probe` shorthand), interpreter internals,
  host objects or implementation-specific stack traces.
* L00 cases use no library and no harness; observe the exit status first.
* Keep negative syntax cases raw: do not wrap them in harness code.
* Print observations explicitly (`Stdout().writeln(...)`). Do not print values
  whose canonical text is unspecified (non-terminating floats, decimal
  spellings): compare them inside the program instead.
* Avoid keyword spellings as names (`next`, `role`, `none`, `match`, ...): a
  case that is rejected for an unrelated syntax reason passes for the wrong
  reason. Review the observed failure message of every negative case.
* A negative case must fail for exactly the reason in its title. Consume
  successors, declare every type, and keep the rest of the program valid.
* Use `harness: true` and `import "conformance/check" take Checks;` for richer
  cases. Every check prints one line, so the expected stdout lists every check.
* Never sleep to order concurrent work; use `Event`, channels and `await`, and
  express permitted schedules with `stdout.partial_order`.
* Use isolated resources only: the working directory, `TemporaryDirectory`,
  loopback fixtures and in-memory databases. Never touch personal files,
  credentials or public networks.

## Metadata

* `id` = `<level>.<slug>`; the directory name is the slug. IDs are permanent.
* Choose the narrowest correct `stages`/`categories`. Only list several when
  the contract genuinely leaves detection open, and say why in `justification`.
* Value-dependent failures that the specification assigns to runtime are
  expected at runtime with the effects that happen before them.
* Tag host requirements in `requires.environment`; `ocs validate` fails if a
  mandatory case falls outside every release profile.
* List imported and data files in `support`.

## When an implementation fails a correct case

Keep the case. Add `defects/DEF-NNN.md` (implementation, revision, case,
contract, authority, expected vs observed) and reference it from the case's
`defects` list. Never weaken an expectation to make an implementation green.

## Checks before submitting

```sh
./ocs validate
python3 runner-tests/test_runner.py
python3 runner-tests/test_schema_sync.py
./ocs run --adapter adapters/rust/adapter.json --profile full-release --case <your id prefix>
./ocs coverage
```
