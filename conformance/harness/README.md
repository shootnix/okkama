# Harness

`okkama/conformance/check.ok` (module `conformance/check v1.0.0`) is the only
suite support code that runs inside test programs. It is ordinary Okkama: an
immutable `Checks` object whose checks are successor actions. For cases with
`"harness": true` the runner copies `okkama/conformance/` next to the program,
so `import "conformance/check" take Checks;` resolves through the language's
normal importer-relative lookup — no injection, globals or host hooks.

Each check prints exactly one line (`ok N - label` or
`not ok N - label: detail`); `done()` prints `checks: T, failed: F` and
returns 0 only when nothing failed. Because expected stdout lists every line, a
skipped, duplicated or broken check changes the output. A discarded check
result is a successor must-use error.

Its own behaviour, including deliberately failing checks, is verified by the
cases `L04.harness.all-pass`, `L04.harness.deliberate-failures` and
`L04.harness.discarded-check-is-error`, and by the mutation control
`harness-negative-control`. Minimal cases (L00–L03) do not use the harness.
