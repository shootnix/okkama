# CIDR network contract

`IpNetwork(address is IpAddress, prefix is int)` represents one immutable inclusive
CIDR interval. Construction validates the family-specific prefix (0..32 or
0..128) and masks host bits; equivalent addresses in the same prefix produce equal
networks. `InvalidIpPrefix(prefix, version)` preserves `prefix()` and gives a stable
family-specific diagnostic in `text()`.

`network()` returns the first address, `broadcast()` the final address for either
family, and `prefix()` the prefix length. The IPv6 method name does not assert
that IPv6 has a broadcast transport. `membership(address)` rejects another family
and otherwise compares masked bytes, including both endpoints. `size() is BigInt`
returns 2^(address_width-prefix), including 2^128 for IPv6 /0. The power operation
has a fixed 129-bit bound. This is not the int-valued Sizing role.

All observations are pure and repeatable. Work/storage are bounded by 16 packed
octets; no address list, host-sized 128-bit integer, DNS, socket or filesystem
resource is involved. A packed-text helper uses the existing private numeric
provider envelope to construct canonical IpAddress values; the envelope is never
an application representation. IpNetworkBytes and PackedIpText are implementation
representations, not extra application networking capabilities.

Tests before implementation: /0, host prefixes, non-octet prefixes, masked identity,
inclusive endpoints, neighboring outsiders, family mismatch and mapped IPv6,
invalid prefix diagnostics, exact BigInt cardinality and repeated observations.
Lazy IpAddresses and IpInterface remain the following tasks.
