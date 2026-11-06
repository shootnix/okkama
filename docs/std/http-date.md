# HTTP dates

HttpDate(origin is Instant) is a whole-second UTC value. Construction checks the
Stage 10 calendar range (years 1..9999) and floors fractional Unix seconds,
including negative fractions. value() returns that normalized Instant;
Comparing<HttpDate>.order() compares it. Canonical storage also makes structural
equality independent of discarded fractional precision. string() emits only
IMF-fixdate with English weekday/month and literal GMT, without locale or clock I/O.

ParsedHttpDate(origin is string[,reference is Instant]).value() parses strict
IMF-fixdate, obsolete asctime or RFC 850. Input is bounded to 33 scalars before
layout parsing. Weekday and calendar must agree; case, separators and whitespace
are exact. Leap seconds and impossible dates/times are InvalidHttpDate. Parsing
is deferred to value(); no trailing data, folding or trimming is implicit.

RFC 850 needs the explicit reference overload: otherwise value() throws
HttpDateReferenceRequired. No ambient wall clock is sampled. Resolve the last
two year digits to the most recent matching timestamp no more than 50 calendar
years after the reference, comparing month/day/time at the 50-year boundary.
The final year must remain in 1..9999. This provides a deterministic rolling
century window instead of a hard-coded 1970/2000 pivot. IMF/asctime need no reference.

Both values are immutable and resource-free; repeated queries are stable and
work is bounded by the fixed date grammar. The parser uses existing Stage 10
calendar/time invariants rather than host locale/date parsing.
Basis: [RFC 9110 5.6.7](https://www.rfc-editor.org/rfc/rfc9110.html#section-5.6.7).
