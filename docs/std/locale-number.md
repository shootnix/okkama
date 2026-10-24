# Localized numeric representations

`locale/number` implements immutable printing and strict parsing from pinned CLDR
46.1.0 / Unicode 16.0.0 data. No host locale database or process locale mutation.

`NumberPrecision(minimum_integer, minimum_fraction, maximum_fraction, rounding)`
requires 1..1024 integer digits, 0..1024 fraction digits, minimum <= maximum.
`NumberFormat(notation, grouping, sign, precision)` combines explicit policies:

- notation: standard, scientific, engineering, compact_short, compact_long;
- grouping: never, automatic (CLDR minimum), always, minimum_two;
- sign: automatic, always, except_zero, negative_only, never.

`LocalizedInteger`, `LocalizedBigInteger`, `LocalizedDecimal`, `LocalizedFloat`
represent a value, Locale and NumberFormat; `string()` observes that representation.
All seven Rounding modes operate on exact decimal coefficients, before grouping,
digit substitution and padding. Float uses interval-verified ShortestFloat, retaining
its original negative-zero sign. Automatic/always preserve negative zero;
except_zero/negative_only suppress a minus after rounding to zero. Decimal itself
normalizes zero. This bridge rounds the exact shortest decimal, not the full binary
expansion of the float.

CLDR symbols, grouping widths, minimumGroupingDigits, scientific affixes and
compact category patterns are compiled at generation time. Engineering exponents
are floor multiples of three, including negative exponents. Carry can promote a
mantissa or a compact bucket, including ordinary 999.9 -> 1K with zero fractions.
Compact categories use the rounded mantissa and its visible minimum fraction;
explicit count=1 literals such as French mille precede general plural selection.
A compact pattern `0` requests ordinary formatting. No inverse is inferred for
compact notation. Scientific notation is explicit even for a CLDR locale whose
scientific skeleton is simply `#`.

Locale `u-nu` selects one of 77 bundled numeric systems, including supplementary
Unicode digits. native/traditio/finance resolve through the locale's CLDR data.
Unknown or algorithmic systems throw InvalidLocalizedNumber. CLDR fallback binds
symbols for the selected system while retaining the closest locale's grouping
pattern when that local system profile is absent. No unrequested likely expansion.

`LocalizedPercent(decimal, locale, format, PercentScale.percent|per_mille)` scales
by 100/1000 before rounding, using the locale percent affixes with the corresponding
symbol. `LocalizedSpecialNumber(NumberSpecial.not_a_number|positive_infinity|
negative_infinity, locale, sign)` explicitly prints CLDR nonfinite symbols; builtin
float remains finite. NaN has no sign.

`LocalizedDecimalFromString(text, locale, format).value()` returns Decimal;
`LocalizedIntegerFromString(text, locale, grouping).value()` returns BigInt.
Both are projected immutable values. The parser does not trim whitespace or accept
foreign digits/signs/separators. Ungrouped input is allowed; separators, when present,
must match all group widths and the locale minimum threshold. Fraction counts and
minimum integer width must match precision. Scientific parsing requires the locale
exponent symbol; engineering additionally requires an exponent divisible by three.
Explicit positive signs are accepted except for negative_only/never. Parsing is a
strict value grammar, not an exact canonical-output check: leading zeroes and
ungrouped spellings can be accepted. Decimal normalizes parsed negative zero.
Compact parsing always fails explicitly.

Budgets: input 1..4096 scalars, integer/coefficient up to 1024 digits, fraction up to
1024 digits, internal scaled coefficient up to 2048 digits, output up to 8192 scalars.
Huge positive exponents fail before power allocation; very small magnitudes are
rounded by direction without constructing enormous denominators. Invalid policy
throws InvalidNumberFormat; invalid input, unavailable data or representation
budget throws InvalidLocalizedNumber. Existing native failures propagate.

Generated data: 726 locale rows (including root), 204 symbol/pattern/compact
profiles, 77 numeric digit sets. SHA-checked sources and archive provenance live in
`tools/data/cldr-46.1/manifest.json`; regenerate with
`python3 tools/generate-cldr-numbers.py`. The generator validates pattern shapes,
quote balancing, delimiters and the pinned compact category-divisor assumption.

Contract tests,
independent source oracle,
[example](../../examples/std-v4/localized-numbers.ok).
