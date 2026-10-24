# SAN, supplied chains and explicit root snapshots

`CertificateNames(origin is Certificate)` snapshots DNS/IP subjectAltName entries.
`dns()` returns Sequencing<string> with original ASCII case, wildcard spelling,
order and repetitions. These are certificate metadata strings, rather than
validated HostName targets. `ips()` returns Sequencing<IpAddress> decoded from
exact 4/16-byte SAN values. DNS names and IP addresses never enter each other's
sequence; URI and other GeneralName forms are not DNS identities. Missing SAN
returns empty sequences, with no common-name fallback. DNS strings do not resolve,
normalize through IDNA or establish matching/trust. Limits: 256 GeneralNames,
1..253 visible ASCII bytes per DNS entry, bounded nested DER parsing. NUL,
controls, non-ASCII DNS, invalid IP widths, empty/malformed SAN and trailing data
fail explicitly. Original certificate DER retains all extension data.

`CertificateChain(certificates is Sequencing<Certificate>)` snapshots a nonempty,
leaf-first supplied chain. Count 1..32 and total DER at most 8 MiB. Source size
and each item are sampled once; count failures precede item access. Order and
repetitions are retained: this denotes supplied untrusted certificates, not the
prospective path built by a verifier. Empty/negative sizes raise InvalidCertificate;
budgets raise CertificateLimitExceeded. `certificates()` returns the snapshot.
Construction does not establish issuer links, trust, signatures or valid purpose.

TrustStore is a noun role exposing `certificates() is Sequencing<Certificate>`.
`TrustedCertificates(certificates is Sequencing<Certificate>)` makes an explicit
local root selection. It snapshots 0..1024 certificates, total DER at most 16 MiB,
without asserting CA/profile/path validity. Empty means no anchors; it never
falls back to system defaults. It grants no filesystem/network capability.

`PemTrustStore(source is string)` snapshots a strict CERTIFICATE-only PEM document
into that explicit selection. Named argument is source; the earlier v3 origin
candidate is not the v4 signature. Input at most 8 MiB/1024 roots, with existing
strict PEM syntax and 64-character body lines. Each public DER certificate is
validated through Certificate. Malformed text/non-certificate PEM labels raise
fixed InvalidTrustStore reasons; counts/input/aggregate DER limits raise
TrustStoreLimitExceeded(kind). Private-key blocks are rejected before decoding
them as a certificate and are never included in diagnostics. Empty input means
an empty store. Source is not a filename and construction reads no files.

PemBlocks.cursor() now acquires one native bulk source snapshot after validation.
Immutable successors index it; repeated item observations no longer reparse the
complete document. Exact block sources/order, independent branches, exhaustion
and InvalidPem offsets are preserved. The explicit legacy PemBlockCursor
constructor remains available with its existing behavior. No global PEM cache
retains source text. Native direct malformed bulk input returns invalid_data.

Subsequent slices implement [extension metadata](certificate-extensions.md) and
[explicit system roots](system-trust-store.md). Chain verification with roots
separated from intermediates, explicit time/purpose and endpoint SAN rules, then
owned TLS transport remain next. The earlier normalized 191-root parsing probe
was explicit PemTrustStore construction, rather than system acquisition or
verification. Root snapshot startup has a measured VM cost;
PERF-16 investigation.

[Base certificate contract](x509-certificate.md),
evidence,
PEM performance investigation.
