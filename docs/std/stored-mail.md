# Stored mail snapshots

`MailFlags(entries is Sequencing<MailFlag>[,maximum_entries is int])` eagerly
copies flags, default maximum 256. The input size is read once and bounded
before items; items are observed once, in order. Each flag's kind and matching
projection are observed once, producing a validated SystemFlag or KeywordFlag.
Case-insensitive wire names determine uniqueness, retaining the first spelling
and order. Custom flag comparison/string methods are not trusted for identity.
Negative bounds/sizes throw InvalidArgument; excess input throws
MailboxLimitExceeded("flags"). The bound counts input entries before deduplication.

MailFlags implements Set<MailFlag>, Sequencing<MailFlag>, Iterating<MailFlag>:
size/item/first/last/cursor observe the snapshot; membership/addition/difference
use protocol identity. Adding an existing flag at capacity succeeds; adding a
new flag beyond capacity fails. Empty first/last and invalid indices retain the
array's existing typed failures. Result sets retain their original maximum.

`StoredMail(identifier,message,flags[,MimeLimits])` eagerly validates the opaque
identifier (1..1024 printable ASCII characters, no whitespace), snapshots at most
256 flags from Set<MailFlag>, renders the borrowed MailMessage once with the
specified MIME bounds, then parses its immutable byte snapshot.
The three-argument constructor retains message.limits(); the fourth argument
explicitly chooses different bounds. It exposes
identifier(), message(), flags(). Flags are acquired before body observation;
inconsistent set size/cursor data throws InvalidMailbox before reading the body.
It never interprets an identifier as a path, closes a borrowed body or performs
filesystem operations. Bcc is retained as stored message content. Rendering and
parsing may canonicalize folding/transfer syntax; this is a semantic snapshot,
not an archival copy of original wire bytes. Parsed bodies are immutable.

`InvalidMailbox(offset,reason)` exposes offset/reason/text;
`MailboxLimitExceeded(kind)` exposes kind/text. Both implement Error. MIME bounds
and malformed message errors keep their existing MimeLimitFailure/InvalidMail
types. A storage backend is responsible for source identity and commit policy;
constructing this value does not assert that it was persisted anywhere.
