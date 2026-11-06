# Private HTTP cache metadata

`http/cache_control`: CacheControl(Headers[,bytes,entries]) snapshots ordered
multiple Cache-Control fields with default 8192 bytes/128 directives. present(name),
count(name), arguments(name) retain case-insensitive directive names, duplicates,
quoted commas and absence of an argument. seconds(name) requires exactly one
argument and directive. CacheDeltaSeconds(string).value() rejects non-digits and
saturates at 2147483648. InvalidCacheControl has fixed reason diagnostics.
Received malformed metadata never authorizes fresh cache reuse.

`http/cache_freshness`: CacheResponseTiming(received Instant,started/finished
MonotonicInstant) preserves wall receipt for Date comparison and monotonic delay.
CacheFreshness(Headers,timing).age(now MonotonicInstant) returns Duration;
lifetime() returns explicit max-age or Expires-minus-Date, with a zero heuristic.
fresh(now) uses strict age < lifetime and forbids no-cache/no-store. Private cache
ignores s-maxage. Missing Date uses receipt time; duplicate/malformed Date, Age,
Cache-Control freshness or Expires makes the observation unusable. Response delay
and residence cannot be negative. Date in the future has zero apparent age.

Age follows the conservative maximum of apparent_age and Age+response_delay,
then adds monotonic residence. This prevents a later wall-clock adjustment from
making a stored response younger. [RFC 9111](https://www.rfc-editor.org/rfc/rfc9111.html#section-4.2.3).
These are bounded metadata collaborators; body ownership, Vary matching, actual
storage, conditional revalidation and concurrent collapse follow separately.
Evidence: five focused files
/90 assertions PASS and inline PASS; no interpreter change/full rerun.
