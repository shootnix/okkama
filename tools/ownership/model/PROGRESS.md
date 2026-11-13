# #582 execution record

- [x] Read OIR-2 draft 2, all 24 review findings and corpus documentation.
- [x] Read 25 handwritten corpus programs, including every TAP expectation.
- [x] Implement textual protocol, executor and bounded exhaustive verifier.
- [x] Positive and negative fixtures for B1–B11 and review 1–24.
- [x] Hand translate all 133 checks of the 25 selected corpus programs.
- [x] Verify traces, checkpoints, errors, parser round trips and invariants.
- [x] Document scope, limitations and nine proposed specification clarifications.

Constraints: only Python 3 / standard library; no compiler, cargo, Docker,
runtime or git commands; all writes confined to this directory; no commit.
The shared Future cache and fresh fake-storage lifetime remain proposals.
G0 approval and G1–G5 of the compiler roadmap are outside issue #582.

Acceptance: 110 unique fixtures; 73 accepted on 75 finite paths; 37 rejected
with the expected verifier rule and message. Of the accepted cases, eight
depend on #581 and one on the proposed fake-storage lifetime. These are
conditional executable expectations, not an approval of those proposals.
All 25 corpus sources / 133 literal TAP log checks match. No runtime executed.
See results.json for source hashes, checkpoints and reproducible results.

Implementation decisions and specification issues are recorded in README.md.
The fixture authoring and read-only inspection helpers were removed after
materialization: the checked-in IR and manifest are the sole fixture sources.
