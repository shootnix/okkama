# Pinned IP properties

`IpAddress.properties() is Set<IpProperty>` computes an immutable overlapping set
from packed bytes. It never consults DNS, interfaces, routing tables or the
network. A property describes the address range, not current reachability.

The tables pin the IANA IPv4 and IPv6 Special-Purpose registries updated
2025-10-09 and IPv6 Address Space updated 2025-10-23, retrieved 2026-09-25.
[IPv4](https://www.iana.org/assignments/iana-ipv4-special-registry/),
[IPv6](https://www.iana.org/assignments/iana-ipv6-special-registry/),
[IPv6 space](https://www.iana.org/assignments/ipv6-address-space/).
Raw CSV files and their hashes are retained with the evidence. Updates require
reviewing changed assignments and rerunning the boundary tests, not runtime fetches.

Library mapping rules:

- unspecified: exactly 0.0.0.0 and ::; loopback: 127/8 and ::1; link_local:
  169.254/16 and fe80::/10; private_network: RFC1918 ranges and fc00::/7.
- documentation: IANA Documentation assignments, including 3fff::/20.
- multicast: 224/4 and ff00::/8. It is not labelled global unicast by this API.
- global: affirmative Globally Reachable on the most specific special-purpose
  assignment, otherwise ordinary IPv4 unicast or the assigned IPv6 2000::/3
  space. Blank/N/A/conditional registry values do not assert global reachability.
- reserved: Reserved-by-Protocol on a special-purpose assignment, otherwise
  Reserved-by-IETF IPv6 address space. A more specific assigned prefix overrides
  its enclosing address-space reservation.

All special-purpose assignments, including rows with no affirmative flags, are
retained. Longest prefix wins; special-purpose data wins equal-length ties over
the address-space fallback. Thus 192.0.0.9/32 overrides 192.0.0.0/24, and
2001:1::1/128 overrides 2001::/23. Shared space 100.64/10 is neither private_network
nor global. IPv4-mapped IPv6 is classified as its own IPv6 assignment; embedded
IPv4 properties are not inherited. An empty set does not assert routability.

The generated private net/ip_registry module contains data plus numeric prefix
matching. Its flag bits follow the IpProperty order: unspecified, loopback,
link_local, private_network, global, multicast, reserved, documentation. This
encoding is private; callers only receive the typed Set. Generation uses the
pinned files offline and does not alter the interpreter.

Tests first: expected sets for named ranges, overlaps, current exceptions,
conditional rows, mapped IPv6 and neighboring addresses; family separation,
prefix edges, repeated observations and set identity. Then implementation,
offline data regeneration check, focused value/property tests and inline examples.
