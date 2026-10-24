# Deterministic configured name resolution

`Resolving` declares `async addresses(HostEndpoint) is Sequencing<IpEndpoint>`
and `async name(IpAddress) is HostName`. `FixedResolution(hosts[, names])` implements
these with explicit `HostAddresses(HostName, Sequencing<IpAddress>)` and
`AddressName(IpAddress, HostName)` records. Object-keyed maps from the v3 proposal
are withdrawn: the current language does not support arbitrary object map keys.

Construction snapshots each sequence once. Forward answers must contain 1..256
addresses; configurations accept 0..256 forward and 0..256 reverse records.
Size limits reject before observing items. Exact numeric duplicates are removed,
preserving the first occurrence and original IPv6/IPv4 order. Bounded sorting
avoids quadratic duplicate detection. Canonical duplicate hosts and reverse
addresses are rejected rather than choosing an ambiguous assignment.

Every forward result receives the requested port, including zero. Host identity
uses validated HostName ASCII/IDNA canonicalization; root-qualified `a.` remains
distinct from relative `a`. Missing forward/reverse records raise `UnknownHost`
or `UnknownAddress` with the requested identity. An empty successful forward
answer is not permitted. `ResolutionLimitExceeded.kind()` distinguishes address
and configuration budgets.

Async lookup is lazy and memoized per Future. Cancelling an unstarted lookup
does not alter configuration or later requests. This resolver performs no native
DNS, socket or system-name lookups; pure numeric/IDNA/ordering primitives are
still used. [System resolution](system-resolution.md) is a separate worker provider.

[Example](../../examples/std-v4/fixed-resolution.ok).
Evidence.
