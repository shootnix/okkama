# Exact finite-float decimal bridge

`numbers/float::ShortestFloat(origin is float).value()` returns a Decimal with
the fewest significant digits found by the bounded precision search (1..17),
roundtripping to the original binary64 bits. Decimal's canonical scientific
spelling is returned; this is not a promise of the fewest printed characters.
negative() retains the original sign bit, including negative zero; Decimal itself
normalizes zero. Both projections are immutable and repeatable.

Builtin float.string() is currently Perl stringification and can lose a significant
digit, so localized numeric output must not use it as a Decimal bridge. This object
reads IEEE bits through existing binary codecs, constructs exact rational rounding
midpoints, and accepts a decimal candidate only inside the nearest-even interval.
Power-of-two binades, smallest normal/subnormal and the maximum-finite overflow
boundary are handled explicitly. Boundary equality is allowed only for an even
significand. No float comparison decides candidate membership.

The existing core.format sprintf_values native primitive proposes precision-g
strings; exact BigInt cross-products verify them. A new std/runtime/format
declaration exposes that already-registered symbol; interpreter/native code is
unchanged. Candidates are not gcd-normalized just to compare interval membership.
Search is at most 17 candidates; binary power budget 1,100 bits, decimal candidate
power budget 1,400 bits. NaN/infinity remain outside builtin finite float.
Failure to find a candidate throws InvalidFloatRepresentation; native failures
propagate. No process locale mutation or host locale database lookup.

Independent IEEE packing verifies 11 boundary/sign fixtures and 64 seeded finite
bit patterns; smaller significant precisions are checked for boundary fixtures.
RED/GREEN and inline evidence.
[Localized number formatting/parsing](locale-number.md) now consumes this bridge.
