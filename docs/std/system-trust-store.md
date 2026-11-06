# Explicit system root acquisition

`SystemTrustStore()` constructs an effect-free request.
`result() is TrustStore` explicitly reads host CA sources and returns an immutable
`TrustedCertificates` snapshot. Each call reacquires the sources; callers choose
the snapshot lifetime and reuse it for their client policy. The TrustStore role
exposes certificates, with no filesystem or network capability.

The initial provider reads the linked OpenSSL installation's `cert.pem` and
hashed `certs` directory. Nonempty `SSL_CERT_FILE` and `SSL_CERT_DIR` override
the corresponding defaults; directory lists use the platform separator. This
profile does not enumerate macOS Keychain or Windows certificate stores and
does not substitute a bundled Mozilla collection. A missing default location
is omitted; a selected override that cannot be read fails explicitly. There is
no fallback from an explicitly empty TrustedCertificates to system roots.

Discovery reads paths only. Selected files are opened nonblocking, checked as
regular files through the opened descriptor and marked close-on-exec. Hashed
entries must match eight hexadecimal digits followed by `.number`; CRLs and
unrelated files are excluded. Symlinks to regular certificate files are allowed.
Files/directories are closed on success and failure. Concurrent changes to the
host store can fail acquisition; the API does not promise an atomic filesystem
transaction across several sources.

Public CERTIFICATE and legacy X509 CERTIFICATE PEM blocks accept ASCII whitespace,
variable line widths and surrounding bundle comments. Base64 padding is checked
by exact re-encoding. Certificates retain exact DER, with first-occurrence
deduplication. Every selected certificate is parsed and host-decoded before
publication; a malformed selected certificate is never silently dropped.
Other PEM labels are ignored; TRUSTED CERTIFICATE causes `unsupported`, because
flattening its auxiliary allow/deny trust restrictions would lose policy.
CRLs, auxiliary trust and revocation are outside this profile.

Limits: 32 selected directories, 2048 selected files, 8 MiB per file, 16 MiB total
source bytes, 1 MiB per certificate, 1024 unique certificates and 16 MiB DER in
the public store. Limits raise `TrustStoreLimitExceeded("system roots")`.
Other native failures raise `SystemTrustUnavailable(reason)` with a portable
code, fixed text and no host paths or source contents. No actual roots yields
`not_found`; an unsupported backend yields `unsupported`.

Acquisition does not validate a peer chain, signature, purpose, time or hostname.
The private native byte-vector ABI exposes no live X509 pointer. Native parsing
and the public Certificate constructor currently each validate the DER; this is
bounded linear work. Remaining VM startup cost is tracked in
PERF-16.

[Example](../../examples/std-v4/system-trust-store.ok),
verification evidence,
investigation.
