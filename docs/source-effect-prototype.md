# Source effect analysis (experimental)

The executable projection in improvements-roadmap/workspace/effects/source reads
real current Okkama source through its parser and analyzer, then feeds the existing
R16 effect checker. For example, a computed cap calling a BillingClock role can
expose LoggingClock's Stderr IO even though cap/role syntax carries no purity rule.
A direct FixedClock.now() remains pure in this experiment. Reading a computed cap
again retains the call; the analysis does not introduce memoization.

Run from repository root:
```
python3 improvements-roadmap/workspace/effects/source/extract.py \
  improvements-roadmap/workspace/billing/experiments/effects_clock.ok \
  'research/effects_clock#1::ClockOwner::Computed.instant'
```
Exit1 with EFFECT_BUDGET is the expected IO/log/time rejection under PURE.
SUPPORTED.md lists the closed subset. Unknown native/unsupported syntax is TOP;
reviewed summaries are hash-guarded and restricted to named fixture inputs.
PURE permits throw and divergence. Role bounds are conservative and can reject a
particular pure implementation. This tool is not installed as compiler enforcement.
Source-pinned acceptance: improvements-roadmap/results/IMP-26/RESULT.md.
