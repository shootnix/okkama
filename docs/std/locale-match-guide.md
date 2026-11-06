# Basic language matching

`locale/match` implements the pure basic-range prerequisite used by HTTP.
`BasicLanguageRange(origin)` validates RFC 4647 section 2.1 grammar: a whole
wildcard, or 1..8 ASCII letters followed by zero or more hyphen-separated
1..8 ASCII alphanumeric subtags. At most 255 scalars and 32 subtags. This grammar
does not imply registry validation or a well-formed BCP 47 tag; one-letter and
unregistered ranges can be valid syntactic preferences.

`string()` is ASCII lowercase; `specificity()` counts subtags (wildcard 0).
`matches(locale is Locale)` follows basic filtering: wildcard, equal tag, or
prefix followed by a hyphen. Comparison folds ASCII case. Thus `de-DE` matches
`de-DE-1996`, but does not match `de-Deva` or `de-Latn-DE`.
`LanguagePrefixes(origin is Locale).values()` returns
`Sequencing<BasicLanguageRange>` in descending specificity, e.g. sr-latn-rs,
sr-latn, sr. It is an immutable snapshot of the represented tag's prefixes.

`InvalidLanguageRange(offset,reason)` implements Error with original zero-based
scalar `offset()`, `reason()` and `text()`. No trimming, extended wildcard,
POSIX translation, alias or registry resolution. `BasicRangeSubtag` is parsing
infrastructure. The candidate type is complete canonical Locale. Requested ranges stay literal;
canonicalize a complete requested tag explicitly before constructing its range
when alias policy is desired. There is no ambient host locale or CLDR expansion.

`FilteredLocales(requested is Sequencing<BasicLanguageRange>, available is
Sequencing<Locale>)` implements `Sequencing<Locale>`. It snapshots both sources
once using `size()` then ascending `item()` calls. At most 128 ranges and 512
candidates are accepted; negative/excessive sizes fail before indexing. Order is
preference position, then available position. Duplicate complete represented tags
keep their first available representative; an empty preference list yields an
empty sequence. `first()`/`last()` on empty results follow `EmptySequence`.

`LookedUpLocale(requested, available, fallback).value()` implements RFC 4647
lookup: ignore whole wildcards, test each progressively truncated basic range,
and remove a dangling extension singleton. First matching available item wins.
The explicit fallback must occur in the available set even when another range
matches; return its first available representative. Empty available input is an
`InvalidLocaleSelection`. Construction snapshots/validates eagerly; `value()`
selects purely and repeatably. Neither object calls or closes borrowed sources
again, opens resources, reads the host locale, or applies likely-subtag expansion.

Selection prepares immutable lowercase keys after the source snapshots.
FilteredLocales ranks only whole-subtag prefixes plus wildcard, retaining the
first matching preference and the first complete available tag. Small sets of
up to four ranges use short cached-key scans; larger sets use binary lookup.
LookedUpLocale retains candidate keys, the first fallback position and, for
more than eight candidates, a sorted prefix index with range minima of original
positions. Prefix queries return the first original candidate rather than the
lexically first exact key; repeated value() does not renormalize candidates.
Small available sets use a bounded scan of cached keys instead of building a
tree. Wildcard skipping, singleton truncation, eager fallback membership and
source observation order remain unchanged.

Index preparation has an explicit cost and O(A) retained structure for A
available candidates; no cardinality budget promises query or constructor
latency. See the prefix-index audit
and remaining CPU investigation.

Candidates use complete canonical Locale. Negotiation does not apply CLDR
likely-subtag matching. HTTP quality weights
remain in [http/negotiation](http-negotiation.md#accept-language-profile).

Evidence: RED,
GREEN: selection, tag and HTTP compatibility.

Sources: [RFC 4647 basic ranges](https://www.rfc-editor.org/rfc/rfc4647.html#section-2.1)
and [basic filtering](https://www.rfc-editor.org/rfc/rfc4647.html#section-3.3.1).
Evidence.
