# Locale data and messages

All formatting/translation policy is explicit. No `setlocale`, host locale
formatting or ambient message domain is used. Ordinary `Locale` tags retain the
existing grammar in [locale/tag](locale-tag-guide.md); full BCP 47 extensions and
IANA alias canonicalization remain separate work.

## Pinned data

`tools/data/cldr-46.1` contains upstream CLDR JSON **46.1.0**, Unicode **16.0.0**,
its Unicode license and SHA-256 manifest. `python3 tools/generate-cldr.py` checks
every input hash, rejects unrecognized plural syntax, and deterministically
regenerates `std/locale/internal/{plurals,parents,likely}.ok`. Consumers run entirely
in Okkama; there is no download or catalog code evaluation at runtime.

`LocaleDataVersion().cldr()` / `.unicode()` report those exact versions. This does
not yet bundle currency/ISO, number, date or collation data; no version is invented
for an absent dataset.

`LocaleFallbacks(origin)` implements `Sequencing<LocaleDataKey>`: explicit CLDR
parent first, otherwise remove the last represented subtag, and finish at `root`.
Depth is bounded to 32. JSON's parent spelling `und` denotes CLDR root and is
translated to the data key `root`. That key is deliberately distinct from a
language tag: `LocaleDataKey("root").locale()` throws `MissingDataLocale`; other
keys normalize through LocaleFromLanguageTag. Example: en-AU, en-001, en, root;
zh-Hant, root. RFC language negotiation uses a different policy.

`locale/likely::LikelyLocale(origin).value()` explicitly adds CLDR likely subtags,
preserving supplied language (except unknown `und`), script, region and variants.
The full 7,737-record table uses compact string literals and a binary search.
It is not invoked implicitly by matching or message lookup.

## Plural numbers

Import `PluralNumber`, `PluralCategoryOf`, `PluralCategory`, `PluralKind` from
`i18n/plural`. `PluralNumber("1.00")` preserves visible precision; `.number()` is
its Decimal numeric value. `.operand(name)` exposes exact BigInt i/v/w/f/t/c/e;
c/e are zero for this plain-decimal profile. `n` is evaluated internally without
float conversion. Accept optional minus, integer digits and optional nonempty
fraction; reject whitespace, plus, exponents, compact notation and spellings over
1,024 scalars with `InvalidPluralNumber`. Magnitude is absolute for CLDR operands.

`CanonicalPluralNumber(decimal).value()` expands the canonical coefficient and
exponent into the same bounded profile. **Decimal already removed visible trailing
zeroes**: callers needing the distinction between 1 and 1.0 supply PluralNumber
from the original spelling. This intentionally revises the old Decimal-only
roadmap candidate.

`PluralCategoryOf(number, locale, kind).value()` returns zero/one/two/few/many/other
using the pinned 219 cardinal / 104 ordinal locale rules. CLDR parent fallback
selects regional rules before removing subtags; unsupported/root languages use
other. Relations use exact integer operands/remainders, and nonintegral n is not
in an integer range. One is a category name, not a universal equality-to-1 rule.

## Catalogs

`MessageKey(context, identity)` compares context then identity exactly; identity
is nonempty and both coordinates are bounded to 1,024 scalars. Translation text
is literal and may be empty. `SingularMessage(key,text)` and
`PluralMessage(key,forms[,kind])` implement `MessageEntry`; kind defaults to
cardinal. Forms are a Sequencing of `PluralForm(category,text)`, not a nominal-enum
key map. Snapshot 1..6 forms once, reject repeated categories, require other.
Each form/text is bounded to 65,536 scalars.

`MessageCatalog(locale,entries)` implements `Translating`: `membership(key)`,
`text(key)`, `plural(key,count is PluralNumber)`. It snapshots at most 4,096 entries
once with ascending indexing, reproduces their immutable keys/forms/text, rejects
duplicates, bounds aggregate text to 1,048,576 scalars, and builds a key-sorted
index. Query lookup is binary. Missing forms use other. A missing key throws
`MissingMessage` carrying that exact key. Singular/plural misuse throws
`WrongMessageEntry` with expected_plural()/actual_plural(). Borrowed sources are
never closed or observed after construction; ordinary resource ownership still
applies. No file access, implicit templating, source-identifier fallback or global
translation state.

`FallbackTranslation(primary,fallback)` chooses primary if its key exists,
including an empty translation. Only absence falls back; wrong projections and
other errors propagate. The selected catalog supplies its own plural rule.
Compose a translated result with TextTemplate explicitly when interpolation is
wanted. GNU gettext/MO/PO is a separate pending wire-format profile.

## Verification

Selection RED/GREEN,
CLDR GREEN,
catalog RED,
catalog GREEN.
CLDR coverage includes 1,084 independent published integer/decimal samples, not
expected values computed by the rule compiler itself.

Sources: [pinned CLDR JSON](https://github.com/unicode-org/cldr-json/tree/46.1.0),
[CLDR plural operands](https://unicode.org/reports/tr35/tr35-numbers.html#Operands),
[likely subtags](https://unicode.org/reports/tr35/tr35.html#Likely_Subtags),
[locale inheritance](https://unicode.org/reports/tr35/tr35.html#Parent_Locales).

## Preferred languages

`locale/system::PreferredLocales(environment[,preferences])` implements
Sequencing<Locale>. The default `SystemLanguagePreferences` consumes
`runtime/locale::preferred_language_tags()` on Apple/Windows. Native transport
uses CoreFoundation UTF8 conversion of a copied preferred-language array and
GetUserPreferredUILanguages with UTF16LE multi-SZ; CF ownership is released before
return and Windows buffer growth retries are bounded to three. At most 64 tags,
255 scalars each. On POSIX the native endpoint returns an empty array and does
not interpret the environment. No process locale/environment mutation.

Inject `FixedLanguagePreferences(tags,platform_owned)` through the
PreferredLanguageReading role for deterministic tests. When platform_owned is
true, preference order comes exclusively from that snapshot. Otherwise the
explicit project policy tries LANGUAGE first (colon list), then LC_ALL,
LC_MESSAGES, LANG; an invalid/empty source falls through. Charset/modifier suffixes
must be nonempty ASCII alphanumeric/hyphen/underscore tokens; strip them, replace
underscores with hyphens, and parse with complete Locale. This language-only
profile does not infer script/currency from POSIX modifiers. C/POSIX and absence
map to invariant `und` (its CLDR data chain ends at root). Preserve order and remove
canonical duplicate tags (including aliases); invalid tags are skipped without truncation.
Environment/source budgets fail with InvalidPreferredLocales; provider failure
uses PreferredLocaleUnavailable. IANA-alias deduplication is verified by the complete-tag continuation.

Native fake snapshots, strict Windows decoder contract and **live Darwin**
snapshot pass. Live Windows API execution remains a Windows CI gate; the decoder
unit test is not described as live Windows verification.
Evidence.
[Apple API](https://developer.apple.com/documentation/corefoundation/cflocalecopypreferredlanguages()),
[Windows API](https://learn.microsoft.com/en-us/windows/win32/api/winnls/nf-winnls-getuserpreferreduilanguages).

## Localized lists

[locale/list](locale-list.md) binds explicit kind/width to pinned CLDR patterns,
with regional/root fallback and literal item text. 725 locale files plus root
are compiled into 292 deduplicated profiles; all profiles have independent
source-data pair/four-item coverage. Number/date/currency/collation remain open.

## Complete tag continuation

[locale/tag](locale-tag-guide.md) now implements bounded complete BCP 47 syntax
and pinned IANA preferred values. LocaleDataVersion.registry() is 2026-09-17.
CLDR fallback starts from the suffix-free base coordinate; explicit likely
expansion preserves extensions/private use. Private-only/opaque language tags
stay unchanged during likely expansion and use und for data fallback.


LocaleDataVersion also exposes currencies() (ISO active-list snapshot 2026-09-17),
icu() (76.1), uca() (16.0.0) and collation_cldr() (46.0). The ordinary CLDR tables
remain 46.1.0 / Unicode 16.0.0. The separate collation version is intentional,
not a claim that ICU 76.1 uses CLDR 46.1. Consumer consistency is checked directly
by t/interpreter/709_locale_data_versions.t.
