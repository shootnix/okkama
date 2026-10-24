# Certificate validity and basic constraints

`crypto/x509/value` supplies pure cohesive metadata used by the upcoming
certificate/trust/TLS providers. CertificateValidity(starts is Instant, ends is
Instant) preserves exact wall-clock instants and rejects reversed periods with
InvalidCertificate. Endpoints are inclusive; equality is representable, and
historical instants are allowed. MonotonicInstant is a different nominal domain.
Construction never reads a current clock or decides whether a certificate is
currently valid/trusted.

CertificateConstraints(authority is bool) has no path-depth limit.
CertificateConstraints(authority is bool, maximum_depth is int) requires a CA
and nonnegative depth. `authority()`, `limited()` and `maximum_depth()` observe
that cohesive value; absent maximum_depth throws MissingCertificateDepth.
Present zero differs from absence: it limits non-self-issued intermediate CAs,
rather than forbidding the final end-entity certificate. The withdrawn v3
three-argument boolean/sentinel candidate is replaced by two explicit constructors.

These values do not parse DER, enforce key usage, verify signatures, build a chain
or establish TLS identity. CertificateExtensions/chain verification must enforce
the remaining constraints together. These constructors emit fixed reasons through
InvalidCertificate.reason(), without including certificate input bytes.
[RFC 5280 basic constraints](https://www.rfc-editor.org/rfc/rfc5280.html#section-4.2.1.9).

[Example](../../examples/std-v4/x509-values.ok),
evidence.
