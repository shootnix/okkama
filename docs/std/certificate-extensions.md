# Certificate extension metadata

`CertificateExtensions(origin is Certificate)` observes the certificate's private
immutable facts. `items()` returns Sequencing<CertificateExtension> in original
extension order. `extension(identifier is string)` looks up a canonical OID and
returns its critical flag and exact OCTET STRING payload via `bytes()`. Missing
identifiers raise MissingCertificateExtension(identifier); unknown extensions and
critical flags are preserved rather than silently accepted as verification policy.
`critical()` returns the ordered critical OIDs. Empty extension lists are valid.

`constraints()` decodes basicConstraints into CertificateConstraints. Absence or
an empty sequence yields authority=false and absent depth; TRUE yields a CA.
Present nonnegative path depth requires a CA; present zero remains distinct from
absence. Explicit DEFAULT FALSE, negative/nonminimal INTEGERs, wrong fields and
trailing bytes fail. Exact depths through 9223372036854775807 are retained; wider
positive integers raise CertificateLimitExceeded before unbounded big-integer
conversion. This metadata does not assert a usable signing CA.

`key_usages()` returns Set<CertificateKeyUsage>, with standard bit order:
digital_signature, content_commitment, key_encipherment, data_encipherment,
key_agreement, key_cert_sign, crl_sign, encipher_only, decipher_only. Absent KU
yields a typed empty set. A present BIT STRING must contain at least one of its
nine named bits, valid padding and minimal named-bit encoding; empty, trailing-zero
or out-of-range values fail. Interpretation/enforcement for a path is separate.

`purposes()` returns Set<CertificatePurpose>, preserving every canonical EKU OID,
including unknown values. CertificatePurpose(identifier is string).identifier()
is exact metadata identity; it is deliberately not a closed enum that drops
unrecognized purposes. Examples: TLS server 1.3.6.1.5.5.7.3.1, TLS client
1.3.6.1.5.5.7.3.2. Absent EKU yields an empty set; a present sequence must contain
1..128 OIDs. OrderedSet retains first occurrence; raw extension payload still
retains repetitions. Unknown requested purposes may be unsupported by a verifier;
metadata construction grants no bypass or permission.

Malformed known extension observations raise fixed InvalidCertificate reasons;
budgets raise CertificateLimitExceeded. CertificatePurpose/CertificateExtension
and identifier lookup validate OIDs using the established exact root/arc rules,
translating malformed OIDs into the certificate domain. CertificateExtension is
an immutable metadata entity; its public constructor does not verify payload
semantics or confer trust. Only known observers decode their relevant payload.
The provider uses bounded DER parsing; no live pointers or I/O occur in getters.

Together with the outer certificate guard, supported BC/KU/EKU payloads enforce
their canonical DER forms. Unknown extension content, algorithm-specific payloads
and full RFC 5280 path policy remain separate. No Certificate.valid()/matches()
method is introduced. Chain verification must enforce signatures, purpose,
constraints, critical extensions, explicit time and SAN identity; these observers
perform none of that verification.

[Example](../../examples/std-v4/certificate-extensions.ok),
evidence.
Rules: [RFC 5280 extensions](https://www.rfc-editor.org/rfc/rfc5280.html#section-4.2.1).
