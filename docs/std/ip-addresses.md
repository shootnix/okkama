# Lazy CIDR address traversal

`IpAddresses(origin is IpNetwork) does Iterating<IpAddress>` represents every
address of the inclusive interval, including its network/final addresses. It is
not a hosts-only iterator and does not implement int-valued Sizing or random
access. `cursor()` returns an independent immutable Cursor<IpAddress> at the first
address. Even IPv6 /0 construction and cursor creation use only fixed-width data.

Each `item()` observes the current address without advancing. `successor()` returns
a new cursor, preserving the previous one. After the final address it returns an
exhausted cursor whose available() is false and whose item()/successor() throw
IterationEnded. Full-width all-ones endpoints never overflow or wrap to zero.
Increasing an address uses at most 16 bytes and no host-sized address integer.

Each traversal stores its current and final addresses only; cursors do not retain
a growing chain of earlier positions. There is no I/O, resource ownership or
cancellation channel. Explicit map/materialization still consumes the entire
source; use Taken or the cursor for a bounded prefix of a huge IPv6 range.

Tests before implementation: small inclusive ranges, /31 and /127, byte carries,
IPv4/IPv6 all-ones endpoints, single-address prefixes, independent/repeated cursor
observations, typed exhaustion and a bounded prefix of IPv6 /0.
