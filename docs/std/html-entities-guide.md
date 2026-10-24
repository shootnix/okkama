# HTML character escaping

## EscapedHtml contract

`EscapedHtml` represents escaped HTML text with immutable source and quotation
policy. Constructors only bind coordinates; `string() is string` performs the
conversion on each observation, with no I/O, native imports or owned resources.

- `EscapedHtml(origin is string)` uses `quotes=false`.
- `EscapedHtml(origin is string, quotes is bool)` selects quotation escaping.
- Always replace `&` with `&amp;`, `<` with `&lt;`, `>` with `&gt;`.
- With `quotes=true`, also replace `"` with `&quot;` and `'` with `&#39;`.

Only original characters are escaped: replacement text is not escaped again.
Existing references are ordinary input: `&amp;` becomes `&amp;amp;`.
Empty input returns empty output. All other Unicode scalars and whitespace are
preserved exactly. Repeated observations agree and never modify the source.
There are no domain-specific errors. Work and auxiliary memory are linear in
source length; output contains at most six scalars per source scalar.

This is character escaping, not contextual HTML validation or sanitization.
It does not establish that a value is safe in URLs, CSS, JavaScript or arbitrary
HTML positions. No trusted-markup wrapper is introduced.

## UnescapedHtml contract — VERIFIED (recorded before implementation)

`UnescapedHtml(origin is string).string() is string` decodes character references
once using HTML text-context rules, never attribute-context restrictions. The
constructor only retains source. Empty input returns empty output; repeat calls
agree. Literal markup, whitespace, NUL and other Unicode scalars remain unchanged;
this operation does not parse a document or perform HTML input preprocessing.

Named references use the existing pinned 2231-entry table, longest matching name,
case sensitivity and only the table's permitted semicolonless spellings.
Decimal and x/X-prefixed hexadecimal numeric references accept a missing
semicolon. Missing digits and unknown references remain literal. Zero, surrogate
and out-of-range numeric references produce U+FFFD; the HTML C1 replacement table
applies. Other controls and noncharacters are retained. Decoded text is not
rescanned: `&amp;lt;` becomes `&lt;`, not `<`.

Rules: [WHATWG named references](https://html.spec.whatwg.org/multipage/parsing.html#named-character-reference-state)
and [numeric references](https://html.spec.whatwg.org/multipage/parsing.html#numeric-character-reference-end-state).

Implementation boundary: reuse runtime/html's existing text-reference decoder by
replacing literal `<` with `&lt;` before tokenization. No original ampersand is
escaped. With no literal `<` left, nonempty source yields exactly one text event;
decoding does not re-tokenize the result. Empty source bypasses tokenization.
No new native API, reflection, filesystem/network effects or provider changes.
Native failures propagate unchanged. The private immutable event handle is
registry-owned until interpreter teardown, as in html/events; each nonempty
observation allocates one. There is no close operation or scope-release promise.
Each observation uses O(n) text/event storage; repeated calls retain O(sum n)
native storage until teardown. This is a materializing, non-streaming API.

Tests cover independent golden recovery cases, literal markup, all existing
table entries as integration coverage, numeric boundaries, one-pass behavior,
escape/decode composition, repeated observations and results escaping a scope.

- [x] Record decoding contract before implementation.
- [x] Consumer RED then GREEN; embedded tests and example.
- [x] Update v4/API and save focused evidence.

[Decoding example](../../examples/std-v4/html-unescape.ok),
decoding evidence.

## Delivery

- [x] Record the contract before implementation.
- [x] Consumer tests RED: empty input, exact substitutions, quote policies,
  existing references, Unicode/whitespace, repeats and independent instances.
- [x] Pure Okkama implementation with contract comments and embedded tests GREEN.
- [x] Executable example, API inventory, roadmap and focused evidence.

[Example](../../examples/std-v4/html-entities.ok),
RED/GREEN evidence.
