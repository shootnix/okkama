# Bounded complete language tags

`Locale(language)` now accepts a complete BCP 47 tag as well as a primary
language. Existing named parameter and constructor arities remain compatible:
Locale(language,region) and Locale(language,script,region,variants) still validate
explicit coordinates, with empty optional script/region only in the four-argument
form. LocaleFromLanguageTag(source).value() remains lazy.

The parser implements RFC 5646 well-formed syntax: primary language, up to three
extlangs, optional script/region, ordered variants, distinct extension singletons,
private use and the pinned grandfathered set. Limits remain 255 input/canonical
scalars, 32 subtags and 28 variants. Duplicate variants and singletons fail without
case distinction. No whitespace trimming, POSIX suffixes or Unicode lookalikes.
Coordinate variant sources are snapshotted once and never closed.

IANA Language Subtag Registry file-date 2026-09-17 is vendored with SHA256 and
source provenance. Preferred language/script/region/variant values, whole-tag
preferred values and extlang prefix replacement are applied. Extensions are
ordered by singleton, case is normalized and private-use spelling is preserved
in lowercase. Comparison uses the complete resulting tag, so iw and he compare
equal while differing private-use/extension payloads remain different.

This is bounded well-formed syntax plus pinned preferred-value canonicalization,
not a claim that every subtag/prefix combination is registered and valid. Unknown
well-formed coordinates remain usable. Suppress-Script is not removed implicitly;
CLDR aliases and Unicode keyword value aliases are separate policies.

`language()/script()/region()` are typed missing projections. Private-only tags
and grandfathered tags without a preferred replacement retain their complete
opaque spelling and have no language coordinate. extlangs(), variants(),
extensions() and private_use() return immutable sequences. base_tag() excludes
extensions/private use for CLDR data lookup; absent language uses und.
suffix() preserves the complete extension/private-use suffix.

`LocaleExtensions(locale)` exposes attributes(), unicode() as an ordered
Sequencing<UnicodeLocaleKeyword>, transformations() and private_use(). Unicode
keyword values are sequences, including an empty boolean-key type. This revises
the unsupported map candidate. Unicode attributes and keyword keys are unique
and sorted; keys use alphanum+alpha grammar. Transformed extensions retain an
optional well-formed language followed by unique alpha+digit fields with nonempty
3..8-character types, sorted by field key. Unknown singleton payloads are retained
without claiming registration. Formatting modules must explicitly support a
keyword before applying it.

InvalidLocale carries a zero-based original scalar offset; duplicate extension
attributes/keys preserve their position in the original source. Aggregate bounds
use their boundary offset. Source I/O occurs only in generation; runtime uses
checked std tables and never reads a host locale or changes process state.

LocaleFallbacks strips the complete suffix before CLDR parent traversal.
LikelyLocale preserves extlangs, variants and suffix while explicitly expanding
language/script/region; private-only/opaque tags remain unchanged.
PreferredLocales deduplicates aliases in preference order. RFC 4647 basic ranges
remain syntactic literal preferences: alias-normalize a complete requested tag
before turning it into a range when that policy is desired.

[Implementation](../../std/locale/tag.ok), [pinned registry](../../tools/data/iana-language-2026-09-17/manifest.json),
independent registry/CLDR/HTTP evidence.
The original core-only delivery is retained as historical evidence.

[RFC 5646](https://www.rfc-editor.org/rfc/rfc5646.html),
[Unicode language/locale extensions](https://www.unicode.org/reports/tr35/#BCP_47_Conformance).
