# #582 execution record — draft 3 follow-up

Baseline: draft 2 model had 110 fixtures, 25 corpus translations / 133 TAP
checks and nine specification questions. Draft 3 accepts all nine answers.

- [x] Read draft 3, normative M1–M9 and owner decisions #581/#583/#584.
- [x] Replace the freshness bit with the six provenance states and fresh admission checks.
- [x] Implement observation registry and preserve real object call boundaries.
- [x] Restrict scope split/transfer to their accessible holder domain.
- [x] Separate external_deps from full value.deps and implement retention views.
- [x] Implement stable keys, logical holder rename, owner epoch and checked refresh.
- [x] Preserve typed Error/kind and store cleanup-origin on the failure carrier.
- [x] Implement carrier attach/transfer/rehome/release, default handler owner,
      common cause/secondary ownership, cached borrows and unique failure take.
- [x] Record abort boundaries; verify cancellation A R versus pre-destroy defer R A.
- [x] Separate service-settle, explicit service schedules, protocol release and serial cleanup.
- [x] Remove fake storage and its fixture; support Cell/Atomic borrowed state.
- [x] Make all eight former #581-conditional fixtures normative.
- [x] Add positive/negative witnesses for every M1–M9 answer.
- [x] Recheck all 25 old programs / 133 checks: exact match.
- [x] Translate 16 more complete corpus programs, async first: 15 exact matches,
      cold-futures 9/10 with one explicitly recorded source/spec difference.
- [x] Verify the catalogue, all finite CFG paths, round trips, diagnostics,
      stable snapshots, roots, schedules and failure graphs.
- [x] Update README with new findings only and regenerate results.json.

Acceptance: 155 unique fixtures; 101 accepted / 103 finite positive paths;
54 expected verifier refusals; 21 unittests pass; no conditional fixtures.
41 corpus programs / 215 TAP checks translated: 40 programs / 214 checks
match exactly. The single difference is cold-futures TAP #9: draft 3 §10.1
closes non-retained inputs at action completion (AR), source expects empty log.
The source expectation and normative result are both retained in manifest and
results.json; no blanket corpus parity or G0 approval is claimed.

Only Python model commands executed; no runtime, cargo, Docker, programs,
network or commits. Writes confined to tools/ownership/model/. The specification
and corpus sources were read only. Active engine target: Sparrow. P1, runtime
parity, source/spec reconciliation outside this directory and compiler gates
G1–G5 are outside this model change. See README.md for scope and decisions.
