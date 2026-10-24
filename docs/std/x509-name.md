# Grouped distinguished-name observations

NameAttribute(identifier is string, value is string) retains a canonical dotted
OID and exact decoded Unicode value. OIDs use roots 0/1/2, require at least two
arcs, disallow leading zeros and restrict the second arc to 0..39 for roots 0/1.
Root 2 supports exact large arcs without truncation. Values retain separators,
Unicode and controls as metadata; this module does not produce a display string.

RelativeDistinguishedName(attributes is Sequencing<NameAttribute>) snapshots a
nonempty group and exposes attributes(). DistinguishedName(relative_names is
Sequencing<RelativeDistinguishedName>) snapshots ordered groups and exposes
relative_names(). An empty DN is representable; an empty RDN is rejected.
Repeated OIDs and multivalued group membership are preserved. The withdrawn v3
flat constructor/attributes() candidate is replaced by explicit groups because
flat attribute lists cannot retain RDN membership. Observed member order is
preserved; these objects do not implement PKIX name matching or DER set sorting.

Each source size/item is sampled once. Limits: OID 1024 scalars/128 arcs, value
4096 scalars, group 1..32 attributes, name 0..128 groups, total 65536 identifier+
value scalars per group/name. Count limits reject before item access. Failures
carry fixed InvalidDistinguishedName reasons, excluding name data. Limits count
Unicode scalars, not DER or UTF-8 bytes; host parsing enforces wire limits separately.

These are decoded text metadata values. [Certificate parsing](x509-certificate.md)
integrates original DER groups and reports unsupported attribute encodings explicitly.
Signature verification and trust remain separate provider work.
[Example](../../examples/std-v4/x509-name.ok),
grouping investigation,
evidence.
