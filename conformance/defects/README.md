# Defect records

Implementation defects found by normative cases. The case keeps its expectation;
the record documents the failing implementation.

| Defect | Implementation | Case | Summary |
|---|---|---|---|
| [DEF-001](DEF-001.md) | okkama-perl | `L01.gram.constructor.shorthand-trailing-comma` | parser rejects the trailing comma the EBNF allows in constructor shorthand |
| [DEF-002](DEF-002.md) | okkama-perl | `L03.variadic.overloaded` | an overloaded variadic action is accepted and runs |
| [DEF-003](DEF-003.md) | okkama-perl | `L05.map.int-key-order` | int map keys are ordered as text (`-1,10,100,9`) |
| [DEF-004](DEF-004.md) | okkama-perl | `L05.map.byte-keys-rejected` | byte values are accepted as map keys |
| [DEF-005](DEF-005.md) | okkama-perl | `L02.int.overflow-wraps` | int is not exact beyond −2^63…2^64−1 (MIN-1 fails) |
| [DEF-006](DEF-006.md) | okkama-perl | `L01.lex.crlf.raw-string-discards-cr` | CR is kept inside raw strings |
| [DEF-007](DEF-007.md) | okkama-perl | `L02.int.modulo-sign` | `%` is floored, not truncated |
| [DEF-008](DEF-008.md) | okkama-perl | — | **withdrawn**: D-035 was refined so that defaults are always evaluated first |
| [DEF-009](DEF-009.md) | okkama-perl | `L02.float.text-*` | float text uses 15 digits, no Go exponent rule, loses -0 |
| [DEF-010](DEF-010.md) | okkama-perl | `L02.string.trim-whitespace-set` | chomp removes a lone trailing CR |
| [DEF-011](DEF-011.md) | okkama-perl | `L02.*-rejected`, `L05.array.byte-index-rejected` | numeric categories are promoted implicitly instead of requiring a cast |
| [DEF-012](DEF-012.md) | okkama-perl | `L05.map.mixed-key-types-rejected` | a map literal accepts keys of different types |
