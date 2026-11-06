# Localized Gregorian date and time

`locale/date_time` implements immutable standard-style representations from
pinned CLDR 46.1.0 / Unicode 16.0.0 data. Date/time primitives retain their
[Gregorian domain contracts](../../std/time/date.ok), including years 1..9999.
There is no host locale, implicit timezone or locale-dependent date parser.

DateStyle and TimeStyle each contain full, long, medium, short.
`DateTimeStyle(date,time)` selects the two styles independently.

- `LocalizedDate(date,locale,date_style).string()` prints a Gregorian date.
- `LocalizedTime(time,locale,time_style).string()` prints a wall time.
- `LocalizedLocalDateTime(local,locale,styles).string()` prints a timezone-free pair.
- `LocalizedInstant(instant,zone is TimeZone,locale,styles).string()` prints an
  absolute instant interpreted through the explicitly supplied timezone.

Construction validates locale data and computes each immutable wall representation
once; repeated projections return its derived text. LocalizedInstant binds data
once but freezes the supplied zone observation separately on each string() call. Locale `u-ca` must be absent or gregory.
An explicit unsupported calendar throws InvalidLocalizedDateTime; Thai locale
alone never changes the Gregorian year into a Buddhist year. Locale `u-nu` selects
only bundled numeric systems, using the numeric module's supported digit sets.
Unsupported numbering throws InvalidLocalizedDateTime with the underlying failure
retained by the runtime. Other locale extensions do not select a timezone.

Gregorian month/weekday context and widths, eras, AM/PM, flexible day periods and
quote-escaped literal text come from CLDR. Numeric fields are substituted separately
from literal names; names are never interpreted as patterns. Two-digit year styles
use year modulo 100; other year fields retain the actual Gregorian year. CLDR's
Hawaiian short-date `M=romanlow` override is supported explicitly for months 1..12.
It does not enable arbitrary algorithmic numbering systems.

Wall time/full and wall time/long use the locale medium time template, retaining
seconds while omitting timezone fields. Short keeps its CLDR minute precision.
This same policy keeps LocalDateTime timezone-free. The date/time combination
uses the CLDR standard combination for the selected date style, not its separate
atTime variant. Both placeholder orders and quote-escaped connective literals
are supported. Standard styles omit fractional seconds without rounding the
underlying value. Flexible day-period selection still observes its exact
nanoseconds: exact midnight differs from a nearby instant. Supplemental day-period
rules fall back to the primary language independently of the calendar parent tree,
which matters for zh-Hant -> und calendar inheritance.

Absolute formatting freezes one observation of the supplied zone's name, offset
and daylight state and derives local fields from that offset. Standard timezone
names use CLDR z/zzzz data, historical UTC metazone intervals and pinned aliases.
The actual offset/daylight state comes from the explicit TimeZone (named Zone uses
bundled IANA tzdata 2026c). A free FixedZone label selects a known CLDR name only
when its observed state agrees with that named zone; an incompatible UTC label
with a nonzero offset falls back to localized GMT. UTC/GMT zero is verified without
another timezone provider query. Other candidate named identities are checked
against the existing bundled Zone API. Native failures propagate.

When no applicable localized name exists, the renderer uses CLDR localized GMT
and hour patterns. Short names use short GMT fallback; full names use long GMT.
Offsets retain second precision rather than pretending they are whole minutes.
Timezone names are bounded to 1..255 scalars; offsets must be whole seconds strictly
within one day. Standard Zone/FixedZone already guarantee those constraints.
Local fields outside Date's supported years retain the existing typed Date failure.
Output is bounded to 8192 scalars, compiled patterns to 128 pieces. Generated
schema/pattern failures throw InvalidLocalizedDateTime.

Compiled data: 726 Gregorian locale rows including resolved root, 386 profiles,
319 programs, 415 historical metazone keys and 28 bounded timezone-name shards.
The generator validates source hashes, quote balance, supported fields, numbering
overrides and interval ordering; it does not evaluate source patterns at runtime.
Run `python3 tools/generate-cldr-dates.py`. Provenance and data hashes are recorded
in `tools/data/cldr-46.1/manifest.json`.

[Unicode LDML date specification](https://unicode.org/reports/tr35/tr35-dates.html),
contract tests,
independent raw-field oracle,
[example](../../examples/std-v4/localized-date-time.ok).
