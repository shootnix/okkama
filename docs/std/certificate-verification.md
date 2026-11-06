# Explicit TLS certificate-path verification

VerifiedCertificateChain is an immutable verification request:
`VerifiedCertificateChain(chain is CertificateChain, trust is TrustStore,
host is CertificateIdentity, at is Instant, purpose is CertificatePurpose)`.
Construction snapshots trust.certificates() once into explicit bounded roots.
It performs no verification, filesystem access, DNS or clock acquisition.
`path() is CertificateChain` builds and verifies the selected leaf-to-anchor path;
`result() is Certificate` returns its leaf. Repeated observations recompute pure
verification against the same inputs; there is no global trust/result cache.

Use `DnsCertificateIdentity(HostName)` or `IpCertificateIdentity(IpAddress)`.
The CertificateIdentity role exposes kind() and string(), with a closed
CertificateIdentityKind enum (dns/ip). Raw strings do not select identity type.
Same-arity constructors currently dispatch by arity/names rather than nominal
parameter types; explicit identity entities preserve the distinction without
changing interpreter dispatch. Custom identity implementations still face the
native DNS/IP validation rules.

DNS reference names use the existing bounded IDNA HostName normalization,
case-insensitive ASCII and removal of the reference root dot. Only DNS SANs
participate; common name never supplies a missing SAN. Wildcards must occupy the
entire leftmost label, match exactly one label and sit below a registrable suffix
in the pinned ICANN+PRIVATE PublicSuffixList. Partial, multilabel, public-suffix
and private-suffix wildcards fail. Certificate DNS suffix spelling is validated
without repairing malformed certificate names. IP references match exact IP
SANs, never DNS strings or CN. Host matching is part of verified path policy.

The first purpose profile supports serverAuth (1.3.6.1.5.5.7.3.1) and clientAuth
(1.3.6.1.5.5.7.3.2). Other requested OIDs raise UnsupportedCertificatePurpose;
they do not select unrestricted verification. OpenSSL checks signatures, strict
X.509 path rules, CA/path-depth constraints, key usage, EKU, name constraints and
unsupported critical extensions. Supplied intermediates never acquire anchor
authority. Only supplied explicit roots enter the fresh store; no host defaults,
AIA fetching or network recovery are enabled.

Time is the exact supplied Instant, compared inclusively with each selected
certificate's validity, including the selected anchor as a conservative profile.
No floor-to-seconds conversion occurs. Native time checks are disabled without
setting USE_CHECK_TIME; candidates outside the exact interval are excluded and
the selected path is checked again. An unused expired extra does not invalidate
an otherwise valid path. If no time-eligible anchor/intermediate builds a path,
the outcome may be untrusted rather than naming an unused candidate's expiry.
Self-signed anchor signatures are checked. This initial profile builds complete
paths to self-signed roots; partial-chain trust termination is not enabled.
The anchor remains outside the RFC 5280 prospective path: strict known-payload
syntax checks apply to the selected leaf/intermediates, while anchor extensions
use OpenSSL's interpreted CA/KU/constraint policy. A legacy anchor's nonminimal
named-bit spelling is preserved rather than normalized or promoted to a wire
certificate requirement. The metadata KU observer remains strict. Tests cover
both anchor compatibility and rejection of the same spelling on an intermediate.
Inputs retain original DER through verification; no reserialized certificate
becomes the published identity.

Bounds: 1..32 supplied certificates/8 MiB DER, 0..1024 roots/16 MiB DER,
1 MiB per certificate, selected path at most 32 certificates/8 MiB DER. Count
and native byte-vector ceilings precede native materialization. Allocation/size
limits raise CertificateLimitExceeded; unavailable backends raise
CertificateBackendUnavailable. Invalid inputs use InvalidCertificate. Backend
unexpected failures retain NativeFailure rather than being accepted as trust.
All BIO, X509 references, stores, parameters, contexts and stacks are released
on success and failure, including get1_chain references.

CertificateVerificationFailed(reason is CertificateFailure) exposes a closed
reason enum: untrusted, invalid_signature, invalid_name, invalid_purpose,
unknown_critical_extension, invalid_chain, not_yet_valid, expired. Diagnostics
contain fixed domain detail, with no certificate text, host path or private key.
Revocation, arbitrary certificate-policy processing and configurable algorithm
security levels are outside this initial path profile. TLS transport must also
enforce its protocol/cipher security policy and bind the verified leaf to the
actual handshake peer before publishing a connection.

This module does not perform a TLS handshake. Existing legacy TLS uses the old
SocketHandle lifecycle; ownership transfer, asynchronous readiness, cancellation,
ALPN and shutdown require the subsequent v4 transport implementation.
