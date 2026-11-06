# Multicast membership

`MulticastJoining(socket is UdpSocket, group is IpAddress,
interface is NetworkInterface, options is MulticastOptions)` is a request over
an existing socket. Construction checks that the group has IpProperty.multicast,
otherwise throwing `InvalidMulticastGroup(group)`; it performs no socket I/O.
The error's `text()` contains the canonical address. A mapped IPv4 address has
its existing IPv6 classification, so mapped multicast is not accepted as IPv4.

`membership() is MulticastMembership` synchronously acquires one any-source
membership. Each call attempts a new acquisition. A duplicate live group on
that socket is rejected; closing it permits rejoining. Acquisition never
transfers or closes a healthy caller-owned socket on an ordinary failure.

The membership exposes `group() is IpAddress`, `interface() is NetworkInterface`,
`closed() is bool` and `close() is void`. Observations retain the original values
after close. Membership retains its UdpSocket owner; destroying the request
does not leave the group. Aliases share close, and scope destruction leaves the
group exactly once. Closing the socket ends all its memberships through the
kernel socket close; later membership close performs no second leave.

The first successful join pins the socket's outbound multicast interface,
hop limit and loopback flag for the rest of that socket's lifetime. Other groups
can join with the same interface index and settings. Conflicting requests fail
with InvalidArgument before host mutation, including after the last membership
closes. This preserves the meaning of later sends. Changing policy requires a
different socket. Join while a send is pending is rejected; pending receives
continue using the same packet API. UdpConnection is outside this API, and the
native boundary rejects connecting a socket that has multicast configuration.

The group must match the socket's IP family; IPv4 on an IPv6 dual-stack socket
is not part of this first membership API. The interface name/index must identify
the same current host interface. Snapshot flags and addresses are observations,
not authoritative configuration. The provider queries the primary IPv4 address
of that interface for IPv4 membership, and uses its index for IPv6. It never
falls back to the default interface. IPv4 interface lookup currently has explicit
Darwin/Linux ioctl layouts; other hosts report unsupported. Named scope lookup
uses numeric-only getaddrinfo and verifies a nonzero matching index.

Native acquisition snapshots the three prior socket settings before applying
the first policy. On any setting/join failure it restores every setting already
changed and returns the original failure. If rollback fails, the socket is
closed to avoid publishing partially changed state. A failed leave likewise
closes the socket, ending any remaining memberships; it returns the leave error.
Both cases leave all public aliases observing closed state. Successful leave
does not restore settings or close the socket.

Closed acquisition throws NetworkClosed. Native invalid_input becomes
InvalidArgument; other acquisition failures use NetworkOperation.binding, and
leave failures use closing. Error categories retain the existing NetworkError
translation. No new enum values or parser/runtime language semantics are needed.

Packets use UdpSocket.send/datagram, preserving their size, truncation,
cancellation and empty-packet contracts. Membership does not promise to filter
all other traffic received by the socket: unicast and platform multicast
delivery rules still apply. Source-specific multicast is excluded.

Implementation follows the existing File/FileLock allow-role composition:
UdpSocket allows the lifecycle role UdpMembership, and MulticastMembership
implements it. Its acquisition constructor is infrastructure used by membership().
No public native-state accessor is added and module imports remain acyclic.

Host references: [IPv4 join](https://man7.org/linux/man-pages/man2/IP_ADD_MEMBERSHIP.2const.html),
[IPv4 outbound interface](https://man7.org/linux/man-pages/man2/IP_MULTICAST_IF.2const.html),
[Darwin interface ioctl definitions](https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/bsd/sys/sockio.h).
