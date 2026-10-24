# Bounded TCP connection racing

TcpRacing(origins is Sequencing<IpEndpoint>) snapshots 1..256 supplied numeric
destinations, uses TcpOptions(), a 250 ms connection stagger and SystemClock.
The full constructor takes origins, options, delay is Duration and clock is
Timing. Non-positive destination ports, invalid source counts and negative
delay throw InvalidTcpRace before acquisition. Source items are read once after
count validation; no unbounded cursor or DNS lookup is used. Zero delay requests
parallel starts. Duplicate destinations remain distinct owned attempts.

TcpRaceEndpoints is the consumed internal snapshot/interleaving collaborator.
It starts with the source's first family and preserves relative order within
each family while alternating IPv4/IPv6, then appending the remaining family.
There is no unconditional IPv4 or IPv6 preference. Its immutable items() sequence
contains the exact original endpoints, including zone and port. Network access
decisions belong to the preceding checked-route consumer.

async connection() is TcpConnection is lazy. The first candidate starts at once;
each next candidate starts when its predecessor fails or when one relative
stagger has elapsed after that predecessor starts. Early failure does not leave
later timers tied to their original input index. Injected Timing supplies the
monotonic delay; negative/unsupported clock failures remain failures and cannot
leave later candidates waiting forever on a predecessor-start signal.

The first successful connection wins. Every pending loser is cancelled and
awaited; every already-ready successful loser is closed before publication.
Each attempt retains its raw TcpConnecting Future before eager startup, so a
cancelled outer adaptation cannot abandon a ready result. Internal winner
ordinals distinguish attempts; native handles are never compared or exposed.
If acquisition/loser cleanup fails, no connection is published and the winner
is also reclaimed. Parent cancellation closes all unpublished descriptors and
detaches native readiness plus delay waits. Unstarted cancellation acquires none.
When every attempt fails, FirstSuccessful retains its typed failure ordering.
Repeated awaits of one Future reuse that outcome; a new invocation is a new race.

Family alternation and the default stagger are informed by
[RFC8305 sections 4–5](https://www.rfc-editor.org/rfc/rfc8305.html#section-4).
This general racing primitive is not a complete RFC8305 implementation: explicit
zero/small delays and immediate failure acceleration do not enforce its 10 ms
minimum spacing requirement. A strict resolver/client profile remains separate. It consumes a complete
provided address list; it does not implement streamed A/AAAA resolution, DNS
family timing, NAT64 discovery or a hostname overload. Cancellation/Within supplies
an explicit overall deadline; no hidden connect timeout or retry is installed.

Http1Connecting now uses this profile over its already checked endpoints. It
does not perform a second lookup or alter HTTPS identity/ALPN verification.
Earlier sequential limitation
is resolved for that consumer; broader host-resolution behavior remains separate.

[Loopback example](../../examples/std-v4/tcp-racing.ok),
investigation,
evidence.
