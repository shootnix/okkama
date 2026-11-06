# Multicast option requests

`MulticastOptions()` equals `MulticastOptions(hop_limit: 1, loopback: true)`.
The explicit constructor accepts positional or named arguments.
`hop_limit() is int` and `loopback() is bool` preserve exactly the requested
values. The immutable value's equality includes both settings.

`hop_limit` must be in `0..255`, inclusive. Zero is a valid explicit request;
negative values (including a host-default sentinel) and values above 255 throw
`InvalidArgument` with `multicast hop limit must be in 0..255: <value>`.
There is no clamping, wrapping or implicit inheritance from host defaults.
The loopback flag is independent of the hop limit and requests local delivery
of outgoing multicast packets; it does not select a loopback network interface.

Construction and observations do no I/O and acquire no resource. This value
does not select an IP family, group or interface, join a group, or prove that a
host supports the requested settings. Application and host failures belong to
[membership acquisition](multicast-membership.md).

IPv4 TTL and IPv6 multicast hops/loopback are socket settings. The first
successful membership pins the interface and options for the socket's lifetime;
later groups require the same policy. This value remains an effect-free request.

Host references:
[IPv4 multicast TTL](https://man7.org/linux/man-pages/man2/IP_MULTICAST_TTL.2const.html),
[IPv6 multicast hops](https://man7.org/linux/man-pages/man2/IPV6_MULTICAST_HOPS.2const.html),
[IPv6 multicast loopback](https://man7.org/linux/man-pages/man2/IPV6_MULTICAST_LOOP.2const.html).
