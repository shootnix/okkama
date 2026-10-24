# Checked HTTP endpoint selection

EndpointAllowing.allowed(url is Url, endpoint is IpEndpoint) is bool observes
both the effective URL and exact numeric candidate. AllEndpoints grants all;
ListedNetworks(origins is Sequencing<IpNetwork>) snapshots 0..1024 explicit CIDRs
once, rejects invalid counts before item/cursor access, and grants membership in
any supplied network. An empty allowlist denies all. Families remain nominal.
PublicEndpoints requires affirmative global IP properties and rejects unspecified,
loopback, link-local, private, multicast, reserved, documentation and scoped
endpoints. Mapped IPv6 keeps its own classification and is rejected, not silently
converted into a public IPv4 candidate. The pinned IANA classification is the
[existing IP properties contract](ip-properties.md), not current route reachability.

HttpEndpointSelection(url is Url, resolver is Resolving, access is EndpointAllowing)
validates HTTP/HTTPS scheme, supplied name/IP authority, absent userinfo and a
positive numeric destination port. Missing port uses 80/443; present empty port,
zero, absent/empty/IPvFuture host and other schemes throw InvalidHttpEndpoint
with fixed reasons before resolution. url() observes its original immutable URL.
Path/query/fragment are retained for policy observation but do not choose ports.

async endpoints() is Sequencing<IpEndpoint> is lazy. A numeric host bypasses DNS.
A name is resolved exactly once per started request through an owned FutureGroup.
At most 256 results are accepted; count validation precedes item/cursor access.
Each endpoint is sampled once and must retain the request-selected port. Then
each candidate is checked once, preserving source order and exact numeric value
in the returned immutable allowed subset. Duplicates are preserved. Empty results
or all-denied candidates throw EndpointDenied; limits throw EndpointSelectionLimit.
No socket or second lookup is performed. An unstarted cancellation is inert;
started cancellation cancels and awaits resolver cleanup. Domain resolver/access
failures retain their original type. The eventual connector must use these exact
checked values and recheck after every changed URL/proxy/redirect decision.

[Deterministic example](../../examples/std-v4/http-endpoints.ok) uses FixedResolution,
filters a private endpoint and performs no external DNS or connection.
