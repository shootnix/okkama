# Migrating the new std library to v4

This guide applies to root `std/`. It does not rename the separate legacy
`stdlib/` APIs. Removed APIs have no compatibility aliases.

| Old API, removed | Current API | Semantic change |
| --- | --- | --- |
| `File.read()` | `File.text().line()` | nominal text-reading view |
| `File.read(limit)` | `File.binary().bytes(limit)` | nominal byte-reading view |
| `File.write/writeln/flush` | byte or text view's corresponding command | File owns closing; views compose stream roles |
| `file << source` | `file.binary() <- source` | byte capability is explicit; source adaptation belongs to Writing |
| `value RemovedTree(...)` | `DirectoryTree(...).remove()` | explicit void deletion; no query side effect |
| `value PreservedTemporaryFile(...)` | `TemporaryFilePreservation(...).preserve()` | explicit void preservation; `destination()` only describes target |
| `Environment.add/diff` | `addition/difference` | persistent snapshots; names do not suggest mutation |
| `StorageUsage(capacity, used, available)` | `StorageUsage(capacity, available)` | used is derived; contradictions rejected |
| `LinearRegression` | `FittedLine` | adds prediction/residual, keeps meaningful coefficients |
| `FileIdentity.device/inode` | `same(other)` | identity comparison does not export host coordinates |
| `Regex.state()` | no replacement getter | exact same-module observation collaborator accesses private state |
| regex/search error imports | regex/pattern `NoRegexMatch, RegexRangeOutsideInput` | error ownership follows the pattern observation; no re-export alias |
| `PemBlock.data()` | `bytes()` | implements ByteSource directly |
| `PercentFormatted`, `UnsupportedFormatValue` | `Formatted(pattern, values)` | exact `{}` slots with `@string`, no reflection/printf dependency |
| `HandlingJsonValue.handle` overloads | `JsonWriting` named actions | null_value, boolean, number, text, elements, members |
| `JsonValue.dispatch(to)` | `print(to)` | explicit output protocol |
| scalar JSON `value()` getters | protocol printing | scalar nodes control their emission |

New behavior includes `File.text().text()` (drain remaining text),
`BigDifference(...).value()`, `StorageUsage.fits(amount)`,
`FittedLine.prediction(at)` / `residual(at, observed)`,
`Utf8Output(origin is Writing)` and `CompactJson(origin is JsonValue)`.

## Files and lifetime

`File` does Closing. `FileBytes` does Reading/Writing;
`FileText` does ReadingText/WritingText and additionally exposes flush.
Constructors of the views accept `origin is File`, never an exposed native state.
All aliases share position and closed status. Views retain the state, so an
escaped view remains usable after the owner's variable leaves its scope.
Destroying a borrowed view does not close the owner's resource. Explicit owner
close invalidates all views and is idempotent. Scope cleanup is explicit in the std owners: `destroy { self.close(); }`.
Views retain the File object in the existing language ownership graph; native
handles alone do not participate in scope destruction. TemporaryFile,
TemporaryDirectory and FileLock also have scoped cleanup. Other capabilities
retain their documented explicit close/defer contract. No mutable fields are added.

Selecting a view does not switch the configured mode: `.binary()` on a text
file does not silently permit binary I/O. Read/write/encoding errors remain
portable typed errors. Closing an unused lazy file does not create it.

`line()` strips the line terminator and throws EndOfInput at EOF. `text()`
preserves remaining CR/LF exactly and returns `""` at EOF; it materializes the
remaining text. Use `bytes(limit)` when memory must be bounded by chunk size.

`TemporaryFilePreservation.destination()` neither moves nor closes anything.
`preserve()` refuses an existing destination; this is not atomic overwrite.
Failure retains temporary cleanup. Successful preservation closes the temporary
and prevents subsequent cleanup from deleting the permanent destination.

## Rendering without inspecting arbitrary objects

```okkama
let label = Formatted(pattern: "{}: {}", values: ["count", "42"]);
label.string(); // count: 42
```

Only `{}` is a slot. Other braces are literal; too few or too many values throw
InvalidFormat. Empty strings and adjacent slots are supported. InvalidFormat no
longer exposes a fictitious offset: count mismatch has no unique failing position.

`CompactJson` invokes JsonValue.print and emits the six explicit JsonWriting
operations. The writer preserves insertion order and exact Decimal values, uses
Decimal's documented canonical numeric spelling (which may be scientific),
escapes every JSON control character and preserves other Unicode scalars.
JsonNull is a JSON wire literal, never language absence. Duplicate member names,
including the empty string, throw DuplicateJsonMember.

## Validation and scope

The audit's initial signatures are historical evidence of what was reviewed.
Use the [current API inventory](stdlib-api-v4.md), [v4 roadmap](stdlib-roadmap-v4.md)
and [completion TODO](stdlib-completion-todo-v4.md) for new work.

Executable examples: [rendering](../../examples/std-v4/rendering.ok) and
[files and escaped views](../../examples/std-v4/files.ok). From `okkama-pl/`:

```sh
prove -l t/audit/16-stdlib-v4.t t/interpreter/250-std-v4-examples.t
OKKAMA_STDLIB_DIR=../std perl bin/okkama ../examples/std-v4/rendering.ok
OKKAMA_STDLIB_DIR=../std perl bin/okkama ../examples/std-v4/files.ok
```

Use the project's Perl 5.42 toolchain. Commands must run from `okkama-pl/`.

## Stage25 logging

Use Logging/LogWriting and explicit NamedLogging context. Fields now take ordered
JsonMember entries with immutable snapshots rather than a generic data map.
LevelThresholds takes LevelOverride entries; terminal styles take LogStyle entries.
QueuedLogs accepts either a synchronous LogWriting plus capacity/overflow/scope or
an asynchronous LogDelivering plus LogQueueOptions/scope, with different constructor
arities. Drain is an async void command and close does not finish the caller's scope.
Files append lazily; rotation owns a reserved generation namespace and requires
regular active/archive files. Syslog borrows UdpConnection and reports local send
completion. See [contract/signatures](logging.md) and loss/latency limits.

## Stage26 JSON reading, number spelling and web

`CompactJson` and `PrettyJson` now write numbers positionally while short: `404`
instead of `4.04e2`, `12.5` instead of `1.25e1`, `-0.125` instead of `-1.25e-1`.
The value is unchanged and exact; integers beyond 21 digits and fractions below
1e-6 keep the canonical form (`1e21`, `1e-7`). Byte-exact fixtures of JSON output
must be updated; parsers see the same numbers. `template/json` is unaffected.

Reading parsed JSON no longer needs custom JsonWriting visitors: use
`json/read.JsonView(value)` with `text/int/integer/number/boolean`, `member`,
`items` and the member shortcuts. See [JSON reading](json-reading.md).

New `web/*` composes controllers over http/routing and http/server. Existing
Handling-based servers keep working unchanged; WebRoutes is itself a Handling, so
it fits anywhere a Handling is expected (for example behind host dispatch). See [web](web.md).
Language: `throw` now accepts a successor result whose type does Error, as
`return` already did, e.g. `throw Exception(401, "expired").header("WWW-Authenticate", "Bearer")`.

`WebRequest.json(limit)` returns `RequestJson` (the JsonView reading API whose
failures are `Exception(422)`; `view()` gives the plain JsonView). The framework no
longer maps json/read errors to 422 on its own, so server-side JSON errors are
server errors. `http/routing.Routes` gained `Routes(routes, fallback, rejection is
RouteRejecting)`; the two-argument form keeps the empty 400/405 responses.

## Locale/i18n and testing profiles — 2026-10-13

Use explicit ranges with FilteredLocales/LookedUpLocale and explicitly request
LikelyLocale. CLDR LocaleFallbacks returns data keys ending at root; its root key
has no language projection. PreferredLocales consumes Environment and an optional
preference capability; it never calls setlocale. The initial core profile did not perform alias migration; the complete-tag
continuation below intentionally adds pinned preferred-value canonicalization.

PluralCategoryOf and Translating.plural accept PluralNumber to preserve visible
fraction digits. Adapt an existing Decimal with CanonicalPluralNumber only when
its canonical precision is the intended rule input. PluralMessage accepts a
sequence of PluralForm values, requires other, and defaults to cardinal.

New testing/* modules replace none of the old test/* imports automatically.
Checkers and mutable testing owners require a running test. Generated<T> reduction
items are Generated<T> values with their own reproduction text. Stub consumption
is next(), not an effect hidden behind value(). TestPosition uses source(), with
module retained only as a JSON property name. The structured model/report values
are explicit; runner filtering/capture/worker integration stays in the roadmap; JUnit XML
formatting is implemented in the continuation below.

## Sequence generation and localized lists — 2026-10-13

SequenceGeneration<T> adds inclusive length bounds and lazy reduction plans to
testing/generation. Existing scalar generators and Property APIs are unchanged.
LocalizedList uses explicit ListKind and ListStyle with pinned CLDR patterns.
See [generation](testing.md) and [list contracts](locale-list.md).

## XML values and JUnit continuation — 2026-10-13

New xml/name, xml/node and xml/write modules implement bounded immutable XML
serialization. XmlElement uses Sequencing<XmlAttribute> instead of the withdrawn
nominal-key map. XmlNode includes depth(); XmlName has a local-only constructor.
InvalidXml(offset,reason) is the writer error contract; parser line/column errors
remain a separate pending API. JUnitTestReport is additive; existing human/TAP/JSON
report behavior is unchanged. [XML](xml-values.md), [testing](testing.md).

## Complete language tag continuation — 2026-10-13

Locale(language) now also accepts a complete tag. The two/four-coordinate forms
and named parameters are preserved. IANA preferred values change represented
identities intentionally: Locale("iw").string() is now "he". Applications
persisting locale keys should explicitly review/recanonicalize their stored keys.
No implicit data migration or process locale mutation occurs. Full extension
identity is retained, while data fallback strips the suffix. Unicode keyword
access uses a sequence of nominal values, revising the old map candidate.
[Contract](locale-tag-guide.md).

## Finite float decimal bridge — 2026-10-13

Use numbers/float::ShortestFloat(float).value() when a Decimal must roundtrip to
its binary64 source; builtin float.string() remains legacy Perl stringification.
negative() preserves the sign of zero separately from Decimal normalization.
The std/runtime/format declaration uses the existing registered printf primitive;
no interpreter/native implementation or scalar behavior changed.
[Contract](shortest-float.md).


For localized ordering, decorate strings with CollatedString(text,locale,options)
and sort through Comparing<CollatedString>. Build the optional pinned ICU 76.1
backend with tools/build-icu-collation.sh. Keys are ephemeral; different locale
or options require separate sort domains. Builtin string ordering stays stable.
[Contract](locale-collation.md).


Use GettextCatalog(MO_bytes,locale) for GNU indexed plurals, and GettextTranslation
when a Translating role is required. Its plural() takes integral Decimal directly;
CLDR MessageCatalog keeps PluralNumber and category rules. MessageKey("",id,true)
represents an explicit empty GNU context. ParsedPo/PoString operate on explicit
Unicode values and preserve active source semantics. [Contract](gettext.md).


## Testing and monotonic domains — 2026-10-13

SystemClock's injected native provider now registers clock_domain(bool) alongside
wall_clock, monotonic_clock and sleep_nanoseconds. Return a stable domain per
fake system clock registry; separate manual clocks require distinct domains.
Explicit MonotonicInstant(Duration) values remain usable with each other in the
"explicit" domain and reject comparison with live clocks.

Use fake.reading.get/has/size and fake.writing.set/remove/clear in new role fakes.
set/clear are void; remove returns false for absence. Legacy fake.storage keeps
its original returns and missing-key behavior. Add an explicit fourth rendering
action to SequenceEquals<T> for bounded nominal-value diagnostics.

Existing -t output and Interpreter.test_results() remain compatible. Runner
options select fresh process workers and std reports; test_results(1) exposes
metadata and structured causes. SnapshotUpdate is explicitly constructed; no
ordinary invocation or environment setting rewrites expected snapshots.
See [testing.md](testing.md) for bounds, timeout grace and backend requirements.
