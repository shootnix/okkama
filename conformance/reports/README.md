# Reports

* `runs/<name>/` — `results.jsonl` (raw evidence per case), `summary.json`, `summary.md`
  (written by `ocs run`; not committed).
* `gate-*.json` — `ocs gate` verdicts and claim metadata.

Report files are excluded from the suite tree hash, so producing them does not
invalidate results.
