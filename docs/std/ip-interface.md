# IP interface value

`IpInterface(address is IpAddress, prefix is int)` is an immutable host address
with a CIDR prefix. It does not enumerate or configure operating-system devices.
`address()` preserves the supplied host bits; `network()` returns the normalized
IpNetwork. `string()` uses canonical host address text followed by `/prefix`.
IPv6 is unbracketed because this value has no port. Zones remain outside IpAddress.

Construction validates the family-specific prefix through IpNetwork and propagates
InvalidIpPrefix unchanged. No DNS, socket or other resource is acquired. Equality
includes the host address and prefix: two hosts in one network remain distinct.

Before implementation, tests cover IPv4 host preservation, IPv6 canonicalization,
both families' minimum/maximum prefixes, normalized network membership, identity,
and typed invalid-prefix diagnostics. Repeated observations preserve all values.
