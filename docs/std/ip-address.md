# Numeric IP address value

First net/ip increment: `IpAddress(origin is string)` eagerly validates one
numeric IPv4 or IPv6 address. It exposes `version() is IpVersion`, packed network
order `bytes() is @byte`, canonical `string()` and `order(other is IpAddress) is
Order`. It implements Comparing<IpAddress>. Equality depends on the canonical
address, not the spelling supplied to the constructor.

Only ASCII address syntax is accepted. No leading/trailing whitespace, hostname,
port, brackets, prefix, zone, NUL, IPv4 shorthand, octal or hexadecimal IPv4 form
is accepted. IPv4 has four decimal octets without redundant leading zeros.
IPv6 permits compressed and embedded dotted IPv4 forms. The existing core.net
inet_pton/inet_ntop provider performs numeric conversion only, without DNS or
sockets. std/runtime/net contains the two existing provider declarations; it
does not add a runtime capability or expose the old socket interface.

Canonical IPv6 output uses lowercase, suppresses leading zeros and compresses
the first longest run of two or more zero words, per
[RFC 5952](https://www.rfc-editor.org/rfc/rfc5952.html). IPv4-mapped IPv6 uses a
dotted decimal suffix. `bytes()` exposes exactly 4 or 16 octets, not the provider's
private endpoint envelope. Ordering compares family first (v4 before v6), then
unsigned packed octets. No signed host integer conversion is involved.

Invalid spelling throws `InvalidIpAddress(origin)`, whose `text()` retains the
original diagnostic input. Unrelated native failures remain native failures.
Provider envelope validation prevents malformed metadata reaching the value API.

`properties() is Set<IpProperty>` now completes the address contract using
[pinned IANA classification](ip-properties.md); it is not approximated as the
negation of RFC1918 private ranges. IpNetwork, IpAddresses and IpInterface follow
afterward in roadmap order.

TDD checks: strict valid/invalid matrices, canonical aliases and equality,
zero compression ties/single zero, mapped IPv6, unsigned ordering, immutable
packed byte shape and real native declarations. No interpreter changes.
