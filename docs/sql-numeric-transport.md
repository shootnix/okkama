# Exact NUMERIC in the maintained PostgreSQL adapter

The billing research connector reuses standard DbInteger(BigInt), DbDecimal and
Decimal. It is a mechanical PG adapter over the local bridge, not a new numeric
primitive or a general provider for all SQL types.

Finite NUMERIC values with no fractional part become DbInteger. This includes
scaled values such as42.000 and whole aggregates beyond int64. RowReader.int still
requires integer kind within signed64; a larger sum must be observed as BigInt:

```okkama
let exact = ObservedDbValue(row.value("total")).integer();
let text = exact.string();
```

Fractional values become DbDecimal and stay exact. DbDecimal bindings are sent as
Decimal text and psycopg Decimal, never a binary float. RowReader.int rejects a
fractional DbDecimal; it does not truncate it. NULL, missing column and exhausted
rows retain their distinct contracts. Malformed numeric/bool/NULL payloads fail as
data errors, with no silent boolean or numeric coercion.

The bridge records observed NUMERIC coefficient precision and scale as wire
metadata. These are value observations, not the declared SQL typmod. Standard
Decimal canonicalization normalizes trailing zeros; the typed DbValue API does
not promise to preserve a declared accounting quantum.

Nonfinite NUMERIC values are rejected as NUMERIC_INVALID/data. Coefficient digits,
expanded integer width (including sign) and transported/canonical value text are
bounded by4096; NUMERIC_LIMIT/VALUE_LIMIT are resource failures. Positive-exponent
expansion is checked before allocating zeros. Zero can canonicalize to0 without
expanding its exponent. Row/body/session budgets remain unchanged. Native SQLite
and other providers retain their own type/range contracts. Floating/bytea values
remain explicitly unsupported in this remote adapter as before; they never become
exact NUMERIC accidentally.

The E08 revenue consumer removes its fixture BIGINT casts. Per-invoice Money
remains signed64 and validated; grouped sums use BigInt observations and exact
JSON numbers. A report with two int64-max projection rows produces exact
18446744073709551614. This projection stress fixture is separate from the actual
USD/EUR financial pipeline, whose replay/rebuild checks retain the original R20
expectations. Source provenance, limits and all failed/successful runs are under
improvements-roadmap/results/IMP-24/.
