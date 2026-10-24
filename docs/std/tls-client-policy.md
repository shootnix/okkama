# Immutable TLS client policy

TlsClientPolicy(trust is TrustStore) snapshots explicit roots, enables TLS 1.2
and 1.3 and advertises no ALPN protocols. The full constructor accepts trust,
versions is Set<TlsVersion> and protocols is Sequencing<string>. trust(),
versions() and protocols() return their immutable snapshots. TlsVersion is a
closed enum (tls1_2/tls1_3); older protocol versions are not representable.

Construction reads only supplied values. It performs no handshake, system-root
acquisition, DNS or clock observation. Empty roots remain empty; no disabled
verification option or automatic system fallback is present. Each handshake
must receive and verify its own nominal server identity independently of this
reusable policy. System roots can be acquired explicitly before construction.

Version selection must contain one or two enabled versions. The finite domain
is sampled through size and membership; the snapshot checks cardinality without
trusting an unbounded custom cursor. ALPN snapshots preserve caller preference
order and exact case-sensitive octets. Each name is represented by Latin-1
codepoints as a lossless 8-bit string, length 1..255; NUL and high bytes are valid
opaque protocol octets. Unicode codepoints outside the octet range are rejected,
never UTF-8 encoded or normalized implicitly. At most 16 names are accepted,
giving a wire vector of at most 4096 bytes including length prefixes. Duplicate
names are rejected. Empty sequencing means no advertisement, not an empty name.

InvalidTlsPolicy(reason) gives fixed reasons without echoing source names.
Negative/excessive counts are rejected before item/cursor access; source items
are sampled once. Roots reuse TrustedCertificates count/DER limits.

This policy creates no usable connection. [TlsClientHandshake and its owned
transport](tls-client-transport.md) now consume these inputs, verify the actual
peer and implement ALPN results, application I/O, cancellation and shutdown.
