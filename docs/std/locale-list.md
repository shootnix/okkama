# Localized lists

`locale/list::LocalizedList(values,locale,kind,style).string()` renders an
immutable text list. `ListKind` is conjunction/disjunction/unit; `ListStyle` is
long/short/narrow. Both choices are explicit.

The constructor snapshots size once and each item once, in order. Limits: 4,096
items, 65,536 Unicode scalars per item, 1,048,576 scalars in aggregate. Violations
throw InvalidLocalizedList before rendering. Empty lists produce empty text;
single items are returned unchanged. Pairs use CLDR's pair pattern; longer lists
compose start, middle and end patterns. Every item stays literal, including
braces, markup, newlines and empty strings. No escaping, markup parsing or
context-dependent grammar correction is inferred from item text.

Patterns are pinned to CLDR 46.1.0. The data snapshot contains 725 resolved JSON
locales and the release's root patterns, with root aliases resolved. Lookups use
LocaleFallbacks, preserving explicit CLDR parents and regional differences. An
unknown locale reaches root. Pattern choice is bound once during construction;
repeated string observations do not rebuild the table. The source generator
verifies all manifest hashes and accepts only ordered `{0}`/`{1}` placeholders.
Nested composition is expanded into one fragment join, avoiding repeated copies
of an ever-growing list. Data and policy remain in std; no host locale database.

```scss
import "locale/list" take LocalizedList,ListKind,ListStyle;
import "locale/tag" take Locale;
LocalizedList(["Ada","Grace","Barbara"],Locale("en"),
    ListKind.conjunction,ListStyle.long).string();
// Ada, Grace, and Barbara
```

[Unicode list-pattern specification](https://www.unicode.org/reports/tr35/tr35-general.html#ListPatterns),
[pinned source provenance](../../tools/data/cldr-46.1/manifest.json),
all 292 distinct profiles checked independently.
