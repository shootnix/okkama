# Design study: established language conformance suites

The suite format was designed after inspecting three mature suites. Only their
public design documents were read; **no foreign test, harness or framework code
was copied** into this repository, so no third-party licence obligations arise.

| Suite | Repository | Inspected revision (HEAD, 19 Oct 2026) | Documents read | Licence |
|---|---|---|---|---|
| Test262 (ECMAScript) | https://github.com/tc39/test262 | `d53ebc8b1931d9f1541e43d59650776e4b766248` | `INTERPRETING.md`, `CONTRIBUTING.md`, `LICENSE` | BSD-style (Ecma) |
| Ruby Spec | https://github.com/ruby/spec | `0816047460cc5c4ca23d09794b48e8638a7047cc` | `README.md`, `CONTRIBUTING.md`, `LICENSE` | MIT |
| Raku Roast | https://github.com/Raku/roast | `bed6022ba0e9560ed20c481c19430d8931896cd3` | `README.md`, `LICENSE` | Artistic 2.0 |

## Comparison and what was borrowed

| Concern | Test262 | Ruby Spec | Raku Roast | Okkama Conformance Suite (OCS) |
|---|---|---|---|---|
| Tests in the language under test | Yes (JS files + YAML frontmatter) | Yes (Ruby with MSpec DSL) | Yes (Raku with `Test`) | Yes: every case is an ordinary `.ok` program; metadata lives in a JSON sidecar so sources stay unmodified. |
| Implementation-independent expectations | Frontmatter + harness assertions | `should` matchers, version guards | Shared files; implementations may *fudge* (skip/todo) via comments | Expectations live only in `case.json` and expected files. There is no per-implementation fudging inside sources; development exclusions never count as passes. |
| Negative cases | `negative: {phase, type}` with phases parse / resolution / runtime | `should.raise(Class)` | `dies-ok`, `throws-like` | `expect.outcome` reject/failure/error with explicit **stage** (parse / check / runtime) and **logical category**. Borrowed from Test262: phase is part of the expectation and a wrong phase fails. |
| Failure classification | Pass/fail per test; harness-defined | Pass/fail/error per example | TAP; fudged tests marked dubious | Eight statuses (PASS, FAIL, UNSUPPORTED, NOT_RUN, BLOCKED, TIMEOUT, CRASH, HARNESS_ERROR); only PASS counts. |
| Isolation | New realm per test; `raw` flag disables harness | Leak checks (`CHECK_LEAKS`) | Process per file | Fresh process and temporary working directory per case; copies of sources; source hashes verified unchanged. |
| Fixtures / non-test files | `_FIXTURE` files must never run standalone | Fixture directories | Helper packages | Support files must be listed by their case; discovery fails on undiscovered or unlisted `.ok` files. Borrowed from Test262's `_FIXTURE` rule. |
| Harness | `assert.js`, `sta.js` injected unless `raw` | MSpec | `Test` module | No injection. Minimal cases use exit status and stdout only; richer cases import the small `conformance/check` Okkama module, whose own failure paths are tested. |
| Asynchronous completion | `async` flag; completion via `print` | Not central | Promise tests in-language | Async programs must print explicit completion observations; missing output fails. Concurrent output uses declared partial orders, never sleeps. |
| Language versions | Spec ids (`esid`), `features` | `ruby_version_is` guards | Branches per language version (`6.c`, `6.d`) | `release.json` pins suite version, language contract document and std root; gate claims name all of them. |
| Implementation adapters | Hosts provide `$262`, `print` | `mspec -t <ruby>` | `fudgeall <impl>` | Versioned external adapter protocol (`ocs-adapter/1`) — the runner never links an implementation and adapters never see case ids or expectations. |
| Standard library organisation | Built-ins under `test/built-ins` | `core/` vs `library/` | Synopsis-numbered dirs | Levels L09–L11 for std, and per-module coverage contracts generated from an audited inventory of `std/`. |
| Timing | Implementation-chosen timeout | — | `ROAST_TIMING_SCALE` multiplier | `--timeout-scale` multiplier recorded in every report (borrowed from Roast); timeouts never satisfy expected errors. |

## Principles taken, semantics not taken

* From Test262: negative expectations name a phase; fixtures never execute
  standalone; a `raw`-like rule keeps negative syntax cases unwrapped.
* From Ruby Spec: organise by language construct versus library owner, and keep
  a separate command-line/process boundary (Okkama L00 and L10).
* From Roast: an implementation may track progress separately, but a release
  claim requires running unmodified files; timing is scalable rather than
  hard-coded.
* Not taken: JavaScript realms, strict-mode duplication, Ruby version guards and
  Raku fudge comments inside sources — they would let implementation knowledge
  leak into the specification.
