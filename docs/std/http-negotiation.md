# HTTP negotiation values

Quality(origin is rat) denotes an exact HTTP preference in 0..1 on a thousandth
grid. Construction rejects out-of-range values and fractions requiring more than
three decimal places; it does not round. Canonical private storage is an integer
number of thousandths. value() is the corresponding rat, acceptable() is false
only at zero, order(other is Quality) implements Comparing<Quality>, and string()
emits a canonical qvalue without redundant trailing zeroes. Structural equality
and comparison share the same validated identity.

ParsedQuality(origin is string).value() interprets only the RFC qvalue grammar:
0 or 1, optionally followed by a decimal point and zero to three digits; after
1 all digits must be zero. No signs, spaces, Unicode digits, exponent or float
conversion. Input is bounded to five scalars before decoding; interpretation is
deferred and stable. InvalidQuality(reason) implements Error.

WeightedValue(value is string[,quality is Quality]) represents one HTTP token
with a validated relative preference, default 1. value() preserves token spelling,
quality() returns Quality, string() emits `token;q=canonical-quality`. HTTP token
validation is shared with Method; arbitrary field fragments/quotes/media ranges
are not accepted. InvalidWeightedValue(reason) reports bad token syntax.
These intrinsic observations represent a coherent weighted association rather
than unrelated private storage. Token case/specificity policy belongs to the
concrete field profile, not to the common weight.

The old unqualified AcceptedValues candidate is not promoted: media ranges,
content codings, charsets and language ranges have different grammars, wildcard,
specificity and absence rules. The following explicit profiles provide bounded field parsers. A qvalue alone does not advertise codec availability or
select an HTTP response. The foundations need no network/provider effects or interpreter changes;
the language profile uses the explicit locale/match predicates below.
Basis: [RFC 9110 quality values](https://www.rfc-editor.org/rfc/rfc9110.html#section-12.4.2).

## Accept-Encoding profile

AcceptedContentCodings() denotes an absent field; the string constructors denote
a present field, including empty. present() observes presence; string() on absence
throws MissingNegotiationField. values() is an ordered immutable
Sequencing<WeightedValue> with canonical coding names. Parsing is eager, bounded
and uses the exact optional weight grammar. q is case-insensitive, but spaces
around `=` are not accepted. Empty recipient members are omitted and charged to
the entry budget; duplicate names, including wildcard, are InvalidNegotiation.
This strict duplicate policy rejects conflicting client weights instead of silently
choosing one. Canonical output emits explicit q for every member.

quality(coding is ContentCoding) follows the field rules: absence accepts all at
1; empty accepts only identity. Explicit entries take precedence over wildcard,
including zero. Identity defaults to 1 unless explicitly excluded or excluded by
`*;q=0`; an explicit identity entry wins over wildcard. Unknown extensions are
valid protocol names, not evidence that a corresponding codec is installed.

preference(available is Sequencing<ContentCoding>) returns only a supplied coding.
Highest positive quality wins, then client member order, then server input order.
Unlisted codings covered by wildcard inherit its client position; implicit
identity follows explicit members on equal quality. Absence uses server order.
NoAcceptableValue is a typed failure; identity is not invented as a fallback when
it is not supplied. Locale/media/charset policies and response effects are separate.

NegotiationLimits defaults: input bytes 8192, raw list slots 128, available choices
128. The explicit constructor binds all three nonnegative limits. Input budget is
checked before field decoding; choices size is checked before item observation.
NegotiationLimitExceeded(kind) reports bytes, entries or available. Canonical
output can grow by adding default q and separators, bounded by input/entry budgets;
the input budget does not pretend to be an output-header limit.
An immutable sorted lookup index detects duplicate names and uses binary search
for each supplied choice. Syntax work is bounded by input bytes. Name-comparison counts are O(N log N)
for index construction and O(M log N) for choice lookup; string lengths and
host runtime/ownership traversal add cost. This is not a whole-program speed claim.
Basis: [RFC 9110 Accept-Encoding](https://www.rfc-editor.org/rfc/rfc9110.html#section-12.5.3).

## Accept media profile

MediaRange() denotes `*/*` at quality 1. MediaRange(origin[,HttpMediaLimits])
parses a bounded HTTP media-range with optional bare qvalue; q is recognized
regardless of parameter order and rendered last. Quoted q and duplicate q are
invalid. The coordinate constructor binds type/subtype, HttpMediaParameters and
Quality; q cannot be an ordinary range parameter. type()/subtype()/parameters(),
quality()/string() observe the validated range, matches(HttpMediaType) checks
type wildcards and required parameter associations. Extension parameter values
remain literal; charset uses the HTTP media value's canonical token identity.

MediaSpecificity compares concrete type (2), subtype wildcard (1), universal (0),
then required parameter count. Each candidate receives the quality of its most
specific matching range, even if zero; equal specificity keeps the first client
occurrence. This prevents an exact zero from being bypassed by a broad positive
wildcard. Specificity is derived from the range, not a freely configurable rank.

AcceptedMediaTypes() denotes absence; string constructors denote present Accept,
including empty. present()/values()/string() mirror explicit field presence,
with MissingNegotiationField for rendering absence. quality(HttpMediaType) is
Quality; preference(Sequencing<HttpMediaType>) returns a supplied value or throws
NoAcceptableValue. Selection compares positive quality, then specificity, then
client order, then server order. Absence accepts all equally; empty accepts none.
List splitting is quote/escape aware, so commas inside parameters remain data.
Empty recipient slots are omitted and charged to the raw member budget.

MediaNegotiationLimits defaults to 8192 input octets, 128 raw members, 32 raw
parameters per member and 128 available choices. Explicit four-argument budgets
are nonnegative; input/choice sizes are checked before syntax/item observations.
Syntax errors translate to InvalidNegotiation, budgets to NegotiationLimitExceeded
(bytes, entries, parameters, available). Parsed preferences and supplied media
values are immutable; no MIME reinterpretation, rendering provider, response
status/body or network action is selected. Full body/response effects stay gated.
Basis: [RFC 9110 Accept](https://www.rfc-editor.org/rfc/rfc9110.html#section-12.5.1).


## Accept-Charset profile

`CharsetName(origin)` is a validated concrete HTTP token, ASCII case-folded;
`*` is rejected here and reserved for field matching. `string()` and
`Comparing<CharsetName>.order()` observe spelling identity. Registry aliases
(e.g. utf8 vs utf-8) are not inferred, and a name does not provide a codec.

`AcceptedCharsets()` is absent. `(origin[,NegotiationLimits])` is present,
including empty. `present()`, `values()` as `Sequencing<WeightedValue>`,
`string()`, `quality(CharsetName)` and `preference(Sequencing<CharsetName>)`
mirror the coding profile observations with charset-specific semantics:

- Absence accepts any supplied charset at quality 1; present empty accepts none.
- An exact entry takes precedence over wildcard, including quality zero.
- No implicit identity or ISO-8859-1 rule. An unlisted name without wildcard is zero.
- Positive quality, client position, then supplied server position choose a value.
- Canonical duplicate names/wildcards are invalid, matching the strict token profile.

Budgets, raw ignored comma slots, source preflight and typed failures are shared
with the coding profile. Common weighted syntax/index infrastructure keeps the
historical CodingField/CodingMember/CodingPreference/CodingIndex names for API
compatibility; WeightedTokenLookup supplies shared binary name lookup. CharsetChoice
retains only a supplied immutable name. Codec creation, text decoding and network
sending are separate effects.

[RFC 9110 section 12.5.2](https://www.rfc-editor.org/rfc/rfc9110.html#section-12.5.2)
marks Accept-Charset deprecated. This is an explicit legacy field consumer; no
client automatically emits it. Evidence
and [example](../../examples/std-v4/http-accepted-charsets.ok).


## Accept-Language profile

`WeightedLanguageRange(range is BasicLanguageRange[,quality is Quality])`
represents a weighted syntactic range, default quality 1; `range()`, `quality()`
and canonical `string()` observe it. `AcceptedLanguages()` is absent;
`(origin[,NegotiationLimits])` is present including empty. `present()`,
`values()` as `Sequencing<WeightedLanguageRange>`, `string()`, `quality(Locale)`
and `preference(Sequencing<Locale>)` observe/consume this field.

Matching is RFC 4647 basic filtering from [locale/match](locale-match-guide.md),
not lookup or likely-subtag expansion. Candidate values are limited to the
existing ordinary Locale subset. Ranges use the full basic-range ASCII syntax;
they need not denote registered or fully well-formed language tags.

The explicit HTTP selection policy is:

- Absent field accepts every supplied Locale at quality 1; present empty accepts none.
- The longest matching range determines candidate quality, including exact zero.
- Wildcard applies only when no non-wildcard range matches; unmatched is quality zero.
- Candidates rank by positive quality, range subtag specificity, client position,
  then supplied server position. This deterministic tie policy is not a universal
  RFC requirement or an inference about mutual intelligibility.
- Duplicate canonical ranges/wildcards are invalid, like the strict token profiles.

A Sorted/LowerBound index and descending LanguagePrefixes avoid full client-range
scans for each candidate. This describes key comparison strategy, not total VM
complexity: immutable graph/snapshot overhead remains covered by the existing
performance backlog.
Byte/raw comma-slot/available budgets and sequence size preflight share the token
profiles; ranges additionally cap length 255 and subtags 32. Typed field failures
and immutable lifetime rules are unchanged. LanguageField, LanguageMatch and
LanguageChoice are parsing/matching/selection infrastructure. No ambient locale,
CLDR fallback, registry alias, rendering or HTTP response effect is introduced.

Basis: [RFC 9110 section 12.5.4](https://www.rfc-editor.org/rfc/rfc9110.html#section-12.5.4),
[RFC 4647](https://www.rfc-editor.org/rfc/rfc4647.html#section-3.3.1).
Evidence,
[example](../../examples/std-v4/http-accepted-languages.ok).
