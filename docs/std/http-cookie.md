# Cookie values and explicit attributes

`http/cookie` implements pure values without an ambient origin or clock.
`CookieExpiry()` denotes a session cookie, with `persistent() == false` and
`instant()` throwing typed `MissingCookieExpiry`. `(origin is Instant)` is
persistent, and `instant()` returns the supplied immutable Instant exactly.
Unix epoch is a real persistent expiry, never a session sentinel. Past and
fractional instants retain the existing integral-nanosecond Instant/Duration
precision; there is no HTTP-date truncation, civil calendar limit or host clock
query. Equal represented session/absolute values have structural equality.
`MissingCookieExpiry()` implements Error with `text()`.

`CookieAttributes()` has omitted domain/path, session expiry, unspecified
SameSite and no flags. The full constructor takes `(domain is string, path is
string, expiry is CookieExpiry, same_site is SameSite, flags is Set<CookieFlag>)`.
It canonicalizes IDNA domains (one leading dot ignored; terminal dot rejected),
checks optional absolute ASCII path (no semicolons/controls), and snapshots the
bounded flag role into three booleans. Flag order does not affect value identity.
Observations: domain(), path(), expiry(), same_site(), flags(), string().

`CookieFlag` cases: secure, http_only, partitioned. `SameSite` cases: unspecified,
strict, lax, cross_site. The last renders `SameSite=None`; `none` is reserved in
Okkama. None and Partitioned require Secure. Pair input is bounded to 4096 ASCII
octets; attribute domain/path sources to 1024/4096 scalars. Values use cookie-octet
grammar (equals is legal data), optionally enclosed in one balanced pair of double
quotes. value()/wire retain those quotes without unescaping or decoding percent
sequences; names use HTTP token grammar and retain case.
Consumer nameless cookies are supported when value is nonempty.

`Cookie(name is string, value is string, attributes is CookieAttributes)` observes
name(), value(), attributes(), pair(), string(), diagnostic(). Prefix enforcement
is case-insensitive: __Secure- requires Secure; __Host- also requires omitted
Domain and explicit Path=/. Nameless values cannot bypass these checks.
`InvalidCookie(reason)` does Error with reason()/text(); failures omit payloads.
Default VM formatting is already type-only. diagnostic() redacts values; string()
and pair() are explicit wire projections and expose them intentionally. Expires
wire output alone floors the stored Instant to HTTP-date UTC seconds.

This profile follows the consumer rules of [6265bis draft-22](https://datatracker.ietf.org/doc/html/draft-ietf-httpbis-rfc6265bis-22)
and [Partitioned draft-01](https://datatracker.ietf.org/doc/html/draft-cutler-httpbis-partitioned-cookies-01),
both work in progress. Origin/domain matching, Max-Age/Expires parsing, expiry
decisions, jar limits and persisted storage remain separate consumers. Actual
URL and pinned PSL foundations are now implemented; no transport is needed for
these pure values.

Evidence,
[example](../../examples/std-v4/http-cookie-expiry.ok).

Values: evidence,
[example](../../examples/std-v4/http-cookie-values.ok).

## Cookie date parser

`http/cookie_date`: `ParsedCookieDate(origin is string).value() is Instant` applies
the cookie-date token algorithm, independently of strict HTTP-date weekday/layout
checks. Input is bounded to 4096 Latin-1 octets. ASCII delimiter ranges, decimal
fields and case-insensitive English month prefixes are fixed; the first recognized
time/day/month/year win. Years 00..69 map to 2000..2069, 70..99 to 1970..1999.
Years before 1601, impossible dates, missing fields and leap seconds are invalid.
Weekday/timezone words are ignored and conversion is always UTC, without locale
or clock access. Non-Latin-1 sources fail with `InvalidCookieDate(reason)`, which
observes reason()/text() and never echoes source. The bounded NFA handles the
grammar; no host date parser or new runtime capability is added.

Date evidence,
[example](../../examples/std-v4/http-cookie-date.ok).

## Set-Cookie and caller context

`http/cookie_policy` exposes `ParsedSetCookie(origin is string, url is Url,
reference is Instant).value() is Cookie`. This is an explicitly bounded strict
HTTP consumer profile: ASCII source with HTAB allowed for WSP, at most 8192 source
octets and 128 attributes; pair grammar is the Cookie contract. Empty/unknown
attributes are ignored. Attribute names are case-insensitive; duplicates choose
the last applicable value. Domain/Path values longer than 1024 octets are ignored.
Malformed Domain rejects the field; malformed age/date are ignored. Unknown
SameSite sets unspecified; invalid Path chooses default path. No comma splitting.

Max-Age wins over Expires independent of ordering. Only optional minus and ASCII
digits are accepted; positive values saturate at 400 days before overflow. Zero
and negative ages use the profile's earliest cookie date (1601-01-01 UTC).
Expires uses ParsedCookieDate and is capped at reference + 400 days. A positive
age retains the supplied reference's fractional precision. No clock is queried.
Domain must match the supplied canonical origin across a dot boundary; suffix
matching is disabled for addresses. Pinned ICANN+PRIVATE PSL rejects public-suffix
domains, except exact origin equality becomes host-only. HTTPS is required for
Secure. Prefix rules use the supplied Domain/Path before defaults or PSL exception,
so a derived Path=/ cannot grant __Host- privileges.

`CookieContext(site is Url, top_level is bool, method is Method)` observes those
three values and key(). The key is the explicit schemeful site: http/https plus
registrable domain from pinned PSL, falling back to the hostname itself; addresses
use canonical literal form. Ports and DNS root dots do not affect site identity.
HTTP/HTTPS origin and a DNS/IP host are mandatory; file, empty and IPvFuture hosts
fail with InvalidCookie. No browser frame-tree/context inference is performed.

Importable collaborators: CookieOrigin(origin is Url) observes url(), domain(),
path(), site(), address(), secure(); CookieDefaultPath(origin is string).string();
CookiePathMatch(cookie,request).matches(); CookieDomainMatch(cookie,request,address).
matches(). Paths match exact text or a prefix across a slash boundary; encoded
slashes remain encoded. These are policy observations, not DNS or transport effects.

Set-Cookie evidence,
[example](../../examples/std-v4/http-set-cookie.ok).

## Memory jar

`http/cookie_jar`: `CookieStoring` offers cookies(origin is Url, context is
CookieContext) is Sequencing<Cookie>, store(origin, headers is Headers, context)
is void, clear() is void. `MemoryCookieJar(clock is Timing[, limits is CookieLimits])`
implements it and Closing, also observing size(), limits(), closed(). The clock
is borrowed; close() drops records and never closes the caller's clock. Close
is idempotent; subsequent operations throw CookieJarClosed. No default clock.

`CookieLimits()` defaults to 3000 total / 180 per exact canonical cookie domain.
The explicit constructor requires 1 <= per_domain <= total <= 10000. All
partitions/paths/host-only variants count toward that domain's limit. Expired
records are removed first; domain pressure evicts nonsecure records before secure,
then least-recently-accessed; total pressure uses least-recently-accessed across
all records. Creation order breaks ties deterministically. These are policy
bounds, not a claim of acceptable latency at the maximum: see PERF-13.

Each Set-Cookie field is parsed independently; malformed fields are ignored.
Replacement identity includes name, domain, host-only flag, path and partition
key; replacement retains original creation time/order. Inclusive expiry removes
the same identity. HTTP origins cannot overlay or delete a matching Secure
cookie across matching domain/path scopes. SameSite Strict requires the request's
schemeful site to match the caller context; Lax and unspecified additionally
permit a top-level safe method. No Lax-Allow-Unsafe grace heuristic. None follows
the constructor's Secure requirement. Cross-site non-navigation creation permits
only None. Partitioned records use context.key() and cannot cross partitions.
HttpOnly is retained and allowed on HTTP retrieval; this API exposes no DOM access.

Retrieval returns an immutable snapshot ordered by descending path length, then
creation time and stable creation order; it survives later replacement/clear/close.
No Cookie header is sent implicitly. The jar uses the existing Atomic effect
capability to publish one immutable state per synchronous call; it has no suspension
or caller callbacks between reading and publishing state. Clock observation occurs
before the state read (tested with reentry). This guarantee is within one
interpreter; no cross-process persistence/thread guarantee. Revisions are bounded
to 2^53-1 and fail before publication rather than wrapping.

Jar evidence,
[example](../../examples/std-v4/http-cookie-jar.ok). Interpreter unchanged.

## Explicit persistence

`SavedCookieJar(origin is MemoryCookieJar, destination is FileEntry).result()`
returns FileEntry after writing; `LoadedCookieJar(origin is FileEntry, clock is
Timing).result()` returns a new MemoryCookieJar. Constructors only bind state.
Save observes the borrowed clock before reading one state snapshot and excludes
sessions and expired records. Load validates the whole file before creating the
jar and removes expired records using its explicit clock. Exact nanosecond
expiry/creation instants, replacement identity, order, access ranks, partition
keys, flags and configured limits round-trip. No session is restored or persisted.

The ASCII format is domain-specific, versioned and bounded: header fields are
OKKAMA-COOKIE-JAR, schema 1, pinned PSL version, total/per-domain limits, counter.
Rows contain fourteen tab-separated coordinates: name, value, canonical domain,
host-only, absolute path, partition key, SameSite, Secure, HttpOnly, Partitioned,
expiry/creation nanoseconds, creation/access order. Cookie grammar excludes tabs
and line breaks, so no implicit escaping is needed. Final LF is mandatory.
Timestamp decimals are canonical and at most 32 characters; revisions stay
within 2^53-1. Duplicate identities/creation orders, malformed coordinates/flags,
policy-version mismatch and domain/count violations are typed InvalidCookieStore
failures, exposing reason()/text() without cookie payloads. This file is an
explicit wire projection, not reflection or a serializer for arbitrary objects.

The file byte ceiling is 64 MiB; rows are at most 16 KiB and count at most the
stored total limit (itself <=10000). Load checks stat size first and also checks
each 64 KiB read chunk, so a changed/growing file cannot bypass the byte ceiling.
Byte ceilings are not heap/latency guarantees; the existing byte ABI still has
the costs described in PERF-09/PERF-13. Save checks output size before external
publication. Filesystem failures remain their original typed failures.

Save stages a private 0600 file inside a private directory in the destination's
parent, flushes and closes it, then uses same-filesystem rename to replace the
destination. POSIX atomic visibility and cleanup after rename failure are tested
on the current host; power-loss durability and other-platform atomic replacement
are not claimed. No fallback deletes the prior destination before replacement.
Failure preserves prior contents and deferred cleanup removes staging resources.
Permissions intentionally protect the explicit unencrypted cookie payload file.

Persistence evidence,
[example](../../examples/std-v4/http-cookie-persistence.ok). No interpreter change.
