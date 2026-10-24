# Numeric IP endpoints

`IpEndpoint(address is IpAddress, port is int)` and its three-argument overload
with `zone is string` describe an immutable numeric destination or bind address.
They acquire no socket, perform no DNS or service lookup, and do not resolve
interface names. Observations are `address()`, `port()`, `zone()` and `string()`.

Ports are integers in 0..65535; invalid values throw InvalidPort, whose port()
preserves the original integer. Zero is representable for OS-assigned binding;
connect/send consumers must reject it when their operation requires a destination
port. That operation-specific rule remains an open roadmap gate until implemented.

An empty zone means absent and is equal to the two-argument form. Nonempty zones
are allowed only for IPv6. This initial portable spelling accepts ASCII letters,
digits and `-._~`, preserves case/leading zeros, and rejects whitespace, NUL,
delimiters, percent escapes and non-ASCII text with InvalidIpv6Zone. The accepted
spelling is a library subset, not a claim about every OS's interface-name syntax.
No existence or scope-to-interface validation occurs in a value constructor.

Formatting is `192.0.2.1:80`, `[2001:db8::1]:80`, or `[fe80::1%eth0]:80`.
This is endpoint display text, not a URI serializer: the literal percent delimiter
is not URI escaping. Scoped address notation follows
[RFC 4007 section 11](https://www.rfc-editor.org/rfc/rfc4007.html#section-11).
Zone identifiers are local context, as described by
[RFC 9844](https://www.rfc-editor.org/rfc/rfc9844.html).
The bare IpAddress never acquires the zone or port. Identity includes all three
coordinates. An invalid port is reported before an invalid zone.

Tests before implementation cover both families, mapped IPv6, port boundaries,
constructor overloads, canonical formatting, identity, absent/numeric/named zones,
forbidden zone characters and typed diagnostics.

## Remaining HostName dependency

The repository has Unicode 16 normalization/category providers but no IDNA or
Punycode implementation. HostName/HostEndpoint remain open pending the choice of
an IDNA backend. Lowercasing ASCII alone would not close their IDNA contract.
[UTS #46](https://www.unicode.org/reports/tr46/) specifies the required mapping
and validation processing; [Libidn2](https://www.gnu.org/software/libidn/) is one
host-library candidate, not an added dependency. No new dependency or native
provider is introduced by this numeric endpoint task.

Backend audit on 2026-09-25: neither Net::IDN::Encode nor Net::LibIDN2 is installed
in the project's Perl 5.42.2 environment. The documented
[Net::LibIDN2 binding](https://metacpan.org/pod/Net::LibIDN2) exposes nontransitional
processing and A-label round-trip checking; it also needs the system C library.
Any selected backend still needs an explicit Unicode data version, STD3/label
validation policy, bounded input and conformance fixtures before HostName closes.
