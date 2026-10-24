# Explicit localized currencies

`locale/currency` represents currency identity separately from locale and amount.
No currency is inferred from a region. These objects format amounts; they do not
perform money arithmetic, convert currencies, or supply exchange rates.

`Currency(code)` accepts exactly the 178 uppercase alphabetic codes in the pinned
SIX ISO 4217 current-currency/funds list published 2026-09-17. Lowercase, unknown
and historical-only codes throw InvalidCurrency. code() preserves the explicit
identity; order(other) is lexical code order. digits() returns the pinned ISO minor
units (0, 2, 3 or 4). The 13 codes whose ISO units are N.A., including XAU/XXX,
are valid identities but digits() throws MissingCurrencyDigits. No made-up default.
`CurrencyDataVersion().iso()/cldr()` exposes snapshot identities.

`CurrencyFormat(numbers is NumberFormat, display is CurrencyDisplay, accounting
is bool, currency_digits is bool)` is an immutable policy. Display selects symbol,
narrow_symbol, code or name. `LocalizedCurrency(amount is Decimal, currency,
locale, format).string()` represents that exact amount under the policy.

When currency_digits is true, ISO digits replace both minimum and maximum fraction
counts; minimum integer width and Rounding remain explicit. When false, the given
NumberPrecision is retained, including for currencies with undefined ISO units.
Rounding precedes CLDR grouping, symbol placement, plural selection and digits.
All five numeric notations are honored. No cash-rounding policy is inferred.

Symbol/code displays use CLDR standard or accounting affixes, monetary decimal/
group overrides, numeric-system extensions and Unicode bidi literals. Narrow
symbol falls back to the ordinary symbol; missing ordinary symbol falls back to
the explicit code. Currency patterns with alphaNextToNumber are selected using
the Unicode Letter category of the symbol character facing the number, so KWD
receives the appropriate gap while US$ keeps its symbol boundary. The finite
boundary categories are compiled from SHA-identified Unicode 16.0.0 data.

Short compact symbol/code output uses the CLDR short currency pattern. Long
compact output composes the decimal compact-long unit with the currency frame.
Because CLDR supplies no compact-accounting patterns, accounting compact output
composes the requested decimal compact unit with the ordinary accounting frame;
this preserves accounting placement and brackets even when short currency data
moves the symbol. Rounded carry and plural-category selection belong to the shared
numeric renderer. A compact frame contains one currency placement; it is not
wrapped in a second currency prefix. Positive explicit signs never inherit
negative accounting parentheses.

Name display uses the locale unit pattern and currency name selected by CLDR
cardinal plurals of the rounded mantissa, retaining visible minimum fractions.
Missing category falls back to other, then displayName, then the explicit code.
Unit patterns support either placeholder order without treating inserted text as
a pattern. Accounting applies to symbol/code displays; name display uses the
CLDR unit-pattern form with the requested numeric sign policy.

Bounds and numeric failures are inherited from [locale/number](locale-number.md):
1024 coefficient/integer/fraction digits, 2048 internal scaled digits, 8192 output
scalars. Currency-name plural operand is at most 1024 scalars. Native failures
propagate; malformed/unavailable generated representation throws
InvalidLocalizedNumber. All projections are immutable and repeatable.

Sources are pinned independently: [SIX ISO maintenance agency](https://www.six-group.com/en/products-services/financial-information/market-reference-data/data-standards.html),
[Unicode LDML currency patterns](https://unicode.org/reports/tr35/tr35-numbers.html#Currency_Formats),
CLDR 46.1.0 archive/root provenance in `tools/data/cldr-46.1/manifest.json`.
Parent deltas preserve every original package field across 726 source locales;
551 nonempty delta rows compile into 188 profiles and 480 translation rows.
Translation source is split into 28 bounded generated modules. This reduces large
source concentration but does not eliminate the lexer performance backlog.
Regenerate with `python3 tools/generate-cldr-currencies.py`; the shared LDML
compiler also reproduces locale/number unchanged.

Contract tests,
independent ISO source oracle,
[example](../../examples/std-v4/localized-currencies.ok).
