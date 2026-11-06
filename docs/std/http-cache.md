# Private completed-response cache

`http/cache_memory` exports MemoryHttpCache(clock Timing,maximum BigInt[,authenticated bool])
and ResponseCaching. Maximum is an accounted storage budget in bytes (0..1 GiB),
including body, retained field/URL text and per-entry allowance, not a heap-size
measurement. Admission evicts oldest stored entries within this budget. Close
releases retained entries; CacheClosed reports later use. Default authentication
storage is disabled; explicit true partitions exact Authorization values. Cookie
values also partition conservatively even without Vary.

ResponseCaching.entry(Request) returns CachedHttpEntry for explicit revalidation;
response(Request) returns a fresh ClientResponse or MissingCachedResponse;
store(CachedHttpEntry) publishes a completed snapshot; invalidate(Request) removes
all variants of the URI. budget()/revision()/conditional_store(entry,revision)
let consuming adapters bound capture and reject publication across invalidation.
The withdrawn v3 synchronous store(Request,ClientResponse) is not revived: a live
response body cannot be synchronously consumed or proven complete by that API.

CachedHttpEntry(request,result,timing,bytes,trailers) binds an explicit completed
snapshot, including body/trailers/freshness/vary observations. Producers must
supply bytes only after clean EOF. Stored requests/responses retain immutable
metadata and replayable owned bytes, never the original live body capability.
This conservative initial profile stores complete GET 200 only. Range/partial
content, redirect history, no-store, Vary:* and unapproved authentication reject
storage. Vary comparisons preserve field order/value/absence; more normalization
is unnecessary for correctness. Matching entries prefer the newest Date.

Fresh response generation replaces Age, synthesizes missing Date from receipt,
and creates independent readers preserving clean trailers. Endpoints/TLS facts
are retained from the producing exchange; a cache hit does not imply a new
network connection. Missing explicit freshness uses zero lifetime. Stale bytes
remain selectable solely for a validating consumer.

`http/cache` exports Caching(origin Exchanging,cache ResponseCaching,clock Timing).
Returned network bodies remain lazy. The adapter captures at most the cache
budget while returning the original chunks. Capture overflow discards the capture
and continues the application stream. Publication occurs only after clean EOF
and available trailers. Early close, protocol failure, cancellation, or invalidation
prevents incomplete/obsolete publication. One supervised read child per operation
avoids read-result accumulation. Closing cancels pending futures and awaits actual
cleanup through the registered read group. Buffer length checks use size(), not
copying all accumulated bytes on every chunk.

Successful unsafe methods invalidate their URI immediately on response. The
revision barrier also prevents a GET started earlier from storing after that
mutation; unrelated invalidation conservatively rejects late publication too.
Caller range/preconditions/request bodies bypass cache handling; no-store bypasses
reuse and capture. no-cache forces validation; max-age/min-fresh constrain fresh local reuse.
Conditional validation and request max-age/min-fresh/only-if-cached are now
implemented. Explicit stale permissions and current-validation sharing are now implemented
within this documented conservative private GET200 profile.

Compose Caching inside cookie/auth adapters so it sees the actual outgoing Cookie
and Authorization. Place Redirecting outside for per-hop selection. Adapters that
internally hide authentication/cookie state cannot safely supply that state to
an outer cache. Shared caching, heuristic freshness and stale fallback are not
part of this private profile.

[Example](../../examples/std-v4/http-cache.ok) and independent TCP peer prove two
calls produce one exact network GET with clean EOF and no cleanup diagnostics.
Evidence: six files /40 assertions
PASS. No interpreter changes or full rerun. [Freshness](http-cache-metadata.md).

## Conditional validation

Stale selected entries produce If-None-Match from one valid ETag, otherwise
If-Modified-Since from a valid Last-Modified. These generated fields are internal;
returned ClientResponse retains the caller Request. A direct valid 304 is drained
and closed, then metadata is merged over the complete stored body and trailers.
Strong ETag matches strongly; weak ETag matches weakly. Last-Modified must remain
unchanged when supplied by 304. Mismatch raises InvalidCacheRevalidation from
`http/cache_validation`, preserving old stored bytes without refreshing them.
No validator means ordinary origin delegation. Caller preconditions still bypass
cache synthesis and are handled directly by the origin.

304 Content-Length does not replace representation length. Missing Date uses
validation receipt and absent Age resets to zero. Changed Content-Encoding cannot
relabel stored bytes. A revision change during validation rejects reuse across a
mutation. Full non-server-error replacement retires selected old variants, even
when new data is no-store. Response no-cache can be stored but must validate on
later use. 5xx are forwarded without implicit stale fallback or another retry.

Fixed hop fields and every Connection-nominated field are excluded from stored
headers. Admission examines original no-store before sanitization and refuses
contradictory Connection nominations of cache control metadata. Removal of absent
names cannot violate a tight field-count budget.

Request max-age and min-fresh admit a sufficiently fresh response. only-if-cached
misses raise MissingCachedResponse locally; no fabricated network endpoint is
attached to a synthetic response. This private API uses a typed local miss rather
than manufacturing an intermediary HTTP504 exchange. [RFC 9111 validation](https://www.rfc-editor.org/rfc/rfc9111.html#section-4.3).
Evidence: seven distinct
files /43 assertions PASS after all affected files are rerun. Std-only/no full.

## Concurrent admission

Exact method/URI/ordered header spelling-insensitive names and exact values form
a SHA-256 admission key. A collision only serializes unrelated work; actual cache
identity/Vary/auth comparisons remain independent. Registry contains 256 keys and
at most 256 live bookings per key; excess raises CacheAdmissionExceeded. Completed
keys retire when their last booking releases, with no permanent keyed lock map.

Fresh hits bypass hashing and locking. Misses/validation acquire a guard and check
cache again. A valid 304 refresh allows a waiting fresh request to use its new
entry. A full200 transfers the guard to the lazy captured body until clean EOF,
overflow or close. Read cancellation drains actual pending work before release.
Consume or close the first returned body before awaiting another identical call;
its live stream owns that key's admission. Unrelated request keys continue.
Current request/response no-cache validation is shared only with bookings that
joined before publication; a later call still validates again.

FutureGroup.cancel requests children in registration order. Its synchronous
cleanup may wake another waiter before that waiter's cancellation is requested.
To require no effect by a specific queued future, request that future's cancellation
before releasing the holder. The cache test verifies precisely that guarantee.
No atomic all-child revocation is promised by this adapter.

Evidence: 58 targeted HTTP files
/523 assertions PASS, unchanged 2112-file inventory; interpreter unchanged/no full.
PERF-27/28 record fresh-hit fixes
and remaining interpreted large-field validation.

## Explicit stale permission and no-cache sharing

A single max-stale accepts unlimited staleness without an argument or at most the
supplied delta-seconds. Request max-age/min-fresh still constrain selection;
response must-revalidate/no-cache/no-store and malformed age/date metadata prevent
stale reuse. No stale body is returned merely because the origin failed.

A completed current validation can satisfy identical callers already waiting on
its admission key, even when request or response says no-cache. Its result must
match actual accepted storage, including bytes, fields and validation timing.
Publication carries the cache revision; mutation before queued admission revokes
it. Later bookings see no prevalidated permission and perform normal validation.
Transient publication retires with its last booking and never shares a live reader.

Final policy evidence: eleven files
/59 assertions PASS with actual TCP and secondary cleanup. Std-only/no full rerun.
