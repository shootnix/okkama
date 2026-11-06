# Exact arbitrary-precision rational values

VERIFIED: numbers/bigrational represents a reduced ratio of BigInt coordinates.
BigRational(numerator is BigInt, denominator is BigInt) canonicalizes using gcd:
denominator is strictly positive; zero is 0/1. Zero denominator raises
ZeroBigRationalDenominator during construction. This is the same explicit
canonical-value construction compromise as BigInt, not deferred I/O.

The value implements Comparing<BigRational>. numerator()/denominator() return
canonical BigInt coordinates; order(other) compares exact cross-products;
string() always emits n/d, including denominator 1. Equal ratios have the same
canonical state and ordinary structural equality. No floating approximation,
conversion to builtin rat/int, NaN or infinity is involved.

BigRationalSum, BigRationalDifference, BigRationalProduct and BigRationalQuotient
retain named left/right operands. Each projected value() returns a normalized
BigRational without modifying either operand. Quotient construction is lazy;
value() raises BigRationalDivisionByZero for a zero numerator in the right operand.
Repeated observations are deterministic. All arithmetic delegates to existing
BigInt representations; there is no new native ABI or host-number coercion.
Like existing BigInt sum/product, this slice has no result-size/time budget;
large decimal inputs/intermediate products still consume proportional resources.


[Executable example](../../examples/std-v4/bigrational.ok) and
RED/GREEN evidence.

The tests use independently computed small integer fractions and explicit
80/100-digit decimal fixtures, not the BigInt runtime as an expected-value oracle.

From okkama-pl with Perl 5.42:

```sh
prove -l t/interpreter/264-std-v4-bigrational.t t/interpreter/251-std-v4-bigint-arithmetic.t t/interpreter/250-std-v4-examples.t t/audit/14-library-syntax.t t/audit/16-stdlib-v4.t
```

Full regression: **387 files / 4210 tests, PASS**.
