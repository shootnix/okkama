# JSON printing completion contract

`PrettyJson(origin is JsonValue, indentation is int)` renders through JsonWriting
without inspecting kind(), fields or concrete types. A conforming JsonValue.print()
selects exactly one of the six JsonWriting commands to represent its value; the
printer does not repair a custom implementation that violates that protocol.
Dependencies are immutable;
each string() uses a fresh buffer. Scalar spelling and escaping are delegated to
JsonTextOutput, shared with CompactJson. Member order and exact Decimal are preserved.

Nonempty containers use LF, one member/element per line, level*indentation spaces,
one space after a colon and no final LF. Empty containers stay [] and {}.
Indentation zero keeps LF and colon spaces. Negative indentation fails at construction
with InvalidJsonIndentation, whose indentation() preserves the invalid value.

Default maximum_depth is 128 (root value is depth 1), maximum_bytes is 1048576
(UTF-8 bytes including punctuation/indentation). An explicit four-argument constructor
accepts both limits; nonpositive limits throw InvalidJsonPrintLimits at construction.
Depth is checked before dispatching a child; output is checked before each append.
Exceeded limits throw JsonPrintDepthExceeded or JsonPrintBytesExceeded at string().
Limits do not sandbox arbitrary user print() code, bound its execution time, or
account for input values already materialized by the caller. CompactJson remains
unbounded; callers needing bounded rendering should use PrettyJson.

Test-first plan: six variants; exact nested golden text; indentation zero; all ASCII
controls and Unicode; exact large decimals; repeatability; parse/print/parse;
custom JsonValue whose kind() throws; invalid configuration; exact byte/depth
boundaries; huge indentation rejected without allocating the padding first.

## Verification and known runtime limitation

The first 11 tests failed before implementation (missing API). Fixtures were then
corrected for existing Decimal scientific canonical spelling and valid try-expression
syntax; neither change altered the specified PrettyJson behavior. Fourteen groups
now cover the contract, including recursive custom dispatch at the default limit.
That last test reaches the intended typed error but also exposes existing Perl
Deep recursion warnings. This runtime diagnostic issue remains in the completion TODO.

[Runnable example](../../examples/std-v4/pretty-json.ok).
From okkama-pl with Perl 5.42:

```sh
prove -l t/interpreter/252-std-v4-pretty-json.t t/interpreter/250-std-v4-examples.t
perl ../docs/audits/evidence/stdlib-v4/check.pl full
```

The four-argument PrettyJson constructor uses named arguments. Maximum_bytes limits
retained output, not UTF-8 conversion temporaries, caller-owned data or total RSS.

Full regression: **372 files / 3851 tests, PASS**, 254 wallclock seconds.
Existing explicit skips and the documented recursion warnings remain visible in full.log.

## Number spelling (2026-10-13)

`JsonTextOutput`, and therefore `CompactJson` and `PrettyJson`, no longer writes Decimal's canonical scientific form. Many JSON readers (Python `json`, Jackson, Go `interface{}`) turn any exponent spelling into floating point, so `404` printed as `4.04e2` lost its integer meaning in practice. `json/write.JsonNumberSpelling` writes the exact value positionally while that stays short, with the thresholds of JavaScript's `Number#toString`:

| Decimal | Written |
|---|---|
| `404`, `1000`, `-12000` | `404`, `1000`, `-12000` |
| integers up to 21 digits | all digits |
| `1e21` and larger exponents | canonical form, `1e21` |
| `1.5`, `-0.005`, `0.000001` | positional |
| below `1e-6` (`1e-7`) | canonical form |

The value written is always exact; nothing passes through float. `1e1000000` is never expanded. Fixtures that encoded the old spelling were updated deliberately (`252-std-v4-pretty-json.t`, `250-std-v4-examples.t`, `t/audit/16-stdlib-v4.t`). `template/json` renders scalars through its own visitor and still prints Decimal's canonical text.

`JsonObject` observes each captured member name once and validates duplicates
through stable sorted indices. Member order remains the original input order;
the earliest repeated input name wins, even if another duplicate sorts earlier.
String identity is exact, including empty names and distinct Unicode spellings.
Validation uses O(n log n) comparisons and O(n) temporary indices.
