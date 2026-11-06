# Network interface observations

`NetworkInterface(name is string, index is int, addresses is @IpInterface,
up is bool, loopback is bool)` is an immutable observation of one network
interface. It retains exact name, positive system index, ordered assigned
addresses/prefixes and independent up/loopback flags. Observations are `name()`,
`index()`, `addresses() is Sequencing<IpInterface>`, `up()` and `loopback()`.

The constructor accepts already captured data, including fixtures and alternate
providers. It does not enumerate devices, verify existence or configure the OS.
Names are nonempty and contain no NUL; Unicode, spaces and case are preserved.
An invalid name or non-positive index throws InvalidArgument. Index zero denotes
no selected interface and cannot identify an observed device. No IP scope name
grammar is imposed on an OS interface's display name.

The input array is immutable; observations retain its order and duplicates, with
no lazy backend attached. Empty addresses and an interface that is down are valid.
Flags are reported independently; a loopback interface may be down. IpInterface
retains each host address and exact prefix, including IPv6 cardinality boundaries.
Equality includes the captured observations, so two snapshots of one system index
can differ when its state changes. Older snapshots remain unchanged.

Tests before implementation cover empty/mixed-family address lists, ordered
duplicates, exact names/indexes, independent flags, identity, repeated cursor
observations and malformed constructor inputs.

SystemInterfaces and MissingNetworkInterface remain open. The current core.net
provider has no portable interface-enumeration capability. In particular, this
value implementation does not claim a system snapshot or invent prefixes from
hostname resolution. A future provider must supply actual system indexes and all
available assigned IPv4/IPv6 prefixes in one snapshot per enumeration call.
