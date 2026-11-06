# Asynchronous system name resolution

`SystemResolution()` implements the same Resolving role as FixedResolution.
Construction and unstarted lookup Futures perform no system lookup. A started
forward request submits the canonical ASCII HostName, exact root qualification,
numeric requested port and stream-address hint to an IO::Async resolver worker.
Results retain system order across IPv6/IPv4, remove exact duplicates and retain
IPv6 scopes. Every public endpoint receives the requested port, including zero.

The native answer accepts at most 256 raw records before conversion/deduplication.
No usable addresses raises UnknownHost; oversize raises ResolutionLimitExceeded.
Temporary EAI_AGAIN and other resolver errors raise DnsFailure, not UnknownHost.
Reverse lookup uses NI_NAMEREQD and validates its returned HostName; it does not
silently substitute the numeric input when a reverse name is absent.

Both directions use the worker primitive. In installed IO::Async 0.805, the
getnameinfo wrapper's numeric-service shortcut can perform the actual hostname
lookup synchronously. Using resolver.resolve avoids that shortcut without
patching CPAN or introducing another dependency.

Each started operation owns its native Future through a FutureGroup. Parent
cancellation cancels/drains language/native waiting and suppresses publication;
it does not promise to interrupt a libc call already executing in a shared worker.
That worker may finish later and is reused/retired by IO::Async. The installed
resolver has a 10-second worker safety timeout; earlier application deadlines
compose through Within. Worker timeout/failure is a DnsFailure. There is no
resolver-specific public timeout or raw socket/flags/wire-record API.

This is the host's configured system resolver, including its hosts file/search
policy, rather than a recursive DNS client. Results alone authorize neither TCP
nor HTTP access: endpoint policy must approve the exact numeric address before
connecting, without repeating DNS during connect.

[IO::Async resolver documentation](https://metacpan.org/pod/IO::Async::Resolver),
[local executable example](../../examples/std-v4/system-resolution.ok),
investigation.
