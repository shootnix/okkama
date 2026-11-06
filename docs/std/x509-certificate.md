# Parsed X.509 certificates

`ParsedCertificate(origin is @byte).value()` and `Certificate(origin is @byte)`
validate a bounded certificate envelope and have Net::SSLeay decode the certificate
and its public key. The resulting immutable object exposes `der()`, `subject()`,
`issuer()`, `validity()` and `public_key()`. `PublicKey.bytes()` returns the complete
DER SubjectPublicKeyInfo, including its algorithm identifier, rather than only
the key's BIT STRING payload. It grants no signing or private-key capability.

Certificate identity consists of exact input DER and a deterministic private
facts snapshot. Parsing the same bytes twice produces equal objects; no live
OpenSSL pointer or mutable metadata cache participates in identity. Constructors
do not query a clock, hostname, resolver, trust store or network. Observations
acquire no persistent certificate/key handles. Parsing checks neither signature
integrity nor certificate trust, hostname, purpose or revocation. Even a changed
signature may be structurally parseable; an upcoming verifier must reject it.

The native reader checks definite/minimal tag and length encodings, exact input
consumption, canonical BOOLEAN/INTEGER/OID/BIT STRING envelopes and RDN SET order.
It validates certificate field layout, omitted explicit DEFAULT values, matching
inner/outer signature AlgorithmIdentifier encodings, version/optional-field order,
and exact UTC calendar validity.
Invalid dates cannot silently normalize through the host calendar library.
Validity endpoints use Unix wall-clock Instants, retain exact seconds and must
be ordered. UTCTime uses its 1950/2050 pivot; GeneralizedTime keeps its explicit
year. Canonical INTEGER serial syntax and exact time text are checked, while
issuance-profile restrictions on serial sign/width or choice of time type by
year are not parsing restrictions. Real system trust anchors include legacy
nonconforming values; metadata parsing must preserve them for explicit trust policy.

Names are read from original ASN.1 RDN groups, preserving repeated OIDs, member
order and group boundaries. UTF8String, PrintableString, IA5String, BMPString and
UniversalString are decoded strictly when a name is observed. Unknown encodings,
including T61String, raise `UnsupportedCertificateName`; no guessed Latin-1
conversion or flattened display string is returned. The certificate and its
exact DER remain usable. Bad supported text raises `InvalidCertificate` on that
observation. Separators, controls and Unicode are metadata, not display escaping.

Unknown extensions are retained internally as OID, critical flag and exact OCTET
STRING content. Duplicate OIDs and explicit DEFAULT FALSE critical flags fail.
Opaque extension values and algorithm-specific key/signature payloads are not
interpreted by the envelope reader. Extension semantics, empty-subject/SAN rules,
critical-extension acceptance, CA/key usage/purpose and signature verification
are subsequent work. This slice does not claim full RFC 5280 path validation.

Limits: input 1 MiB, envelope nesting 32, 8192 envelope nodes, 128 extensions,
128 RDN groups, 32 attributes per RDN, OID text 1024 scalars/128 arcs, decoded
attribute 4096 scalars and decoded name 65536 OID+value scalars. Private facts
input is bounded to 4 MiB. Limits produce `CertificateLimitExceeded`; malformed
certificate envelopes produce fixed `InvalidCertificate` reasons. Missing host
support produces `CertificateBackendUnavailable`. Other backend failures retain
their portable NativeFailure category. Raw input and host error queues are not
included in domain diagnostics. BIO, X509 and acquired EVP_PKEY resources are
freed on success and on failure after acquisition; the error queue is cleared.

Net::SSLeay 1.96 is installed in the current Perlbrew. Required functions are
loaded lazily; the provider adds no dependency to pure name/value modules.
No interpreter syntax or VSCode grammar changes are required.

[Example](../../examples/std-v4/x509-certificate.ok),
evidence,
investigation.
Source rules: [RFC 5280 certificate fields](https://www.rfc-editor.org/rfc/rfc5280.html#section-4.1),
[OpenSSL decoder caveats](https://docs.openssl.org/3.5/man3/d2i_X509/),
[Net::SSLeay interface](https://metacpan.org/pod/Net%3A%3ASSLeay).
