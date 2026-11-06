# Maildir observations

`Maildir(path[,MaildirLimits])` is a lazy coordinate for an existing, trusted
local Maildir. Construction performs no I/O. path()/limits() are pure.
messages() checks the root and new/cur/tmp directories, enumerates new then cur
in lexical filename order, and returns an eager Sequencing<StoredMail> snapshot.
Traversal of the returned sequence performs no filesystem work. Each message
file is opened once, bounded reads use at most 65536 bytes and at most remaining
budget + 1, and handles/buffers are closed on every path. MIME parsing uses the
shared limits; aggregate wire bytes are bounded separately.

`MaildirLimits()` defaults to 1000 messages, 67108864 total wire bytes and
MimeLimits(). Its `(messages,wire_bytes,mime)` constructor accepts nonnegative
counts; getters expose the same coordinates. Message count is checked before
opening payload files. The existing DirectoryEntries provider snapshots each
directory in bulk: this slice bounds message materialization, **not** native
directory-list allocation. It is intended for trusted local mailbox directories.

`MaildirName(string)` validates one printable ASCII component of at most 255
bytes, with no slash, backslash, leading dot or malformed info suffix. It exposes
string(), identifier(), current(), flags(). A new name has no colon; a current
name has `:2,` followed by distinct letters from DFRST (draft, flagged, answered,
seen, deleted). Unsorted letters are accepted; flags follow filename order.
Unknown keyword letters/extensions fail explicitly instead of being discarded.
An empty current flag suffix is valid. Base identifiers may include Dovecot
size annotations; they are opaque data, never filesystem paths. The documented
layout follows [Dovecot's Maildir documentation](https://doc.dovecot.org/2.3/admin_manual/mailbox_formats/maildir/).
This profile does not read Dovecot index/keyword maps or assign IMAP Recent.

messages() ignores dotfiles, rejects directory/symlink/special entries, requires
new entries without info and cur entries with info, and rejects duplicate base
identifiers across both directories. It does not move new messages to cur or
change flags. Cooperating operations use the lock described below. External
writers that ignore this lock can still cause typed filesystem failures or
inconsistent observations.
Directory ancestors must be controlled by the caller. Separate lstat/open
checks are not a no-follow capability and do not protect against hostile path
replacement. No claim of network-filesystem consistency is made.

Invalid names/layout/duplicates throw InvalidMailbox(offset,reason).
Message/aggregate limits throw MailboxLimitExceeded; per-message MIME limits
retain MimeLimitFailure. File errors and malformed messages keep existing types.
Stored content retains Bcc and may canonicalize MIME wire spelling.

`Maildir.add(message)` delegates to `MaildirDelivery(maildir,message).deliver()`.
The latter also accepts a third Randomizing argument for deterministic tests;
production uses SecureRandom. Constructors have no effects. deliver() validates
the existing layout/count, renders the body once before creating a temporary
file, writes/flushes a private temporary file in tmp, then atomically publishes
a hard link in new. Names are 24 random bytes rendered as 48 hex characters,
with no host/time data. Existing base IDs and EEXIST collisions cause at most
32 attempts. Incorrect random output length is InvalidMailbox; exhausted retries
throw MailboxLimitExceeded("collisions"). A collision never replaces a message.
Cross-filesystem publication fails; no copy fallback is used. This revises the
old queue's literal rename proposal to an atomic no-replace link publication.

Publication is the commit point. Successful cleanup removes the temporary link.
`CommittedMaildirFailure(identifier,code,reason)` reports a cleanup failure after
commit, so the caller must not blindly retry delivery. Before commit, failures
attempt cleanup and preserve the original error. Atomic visibility is not an
fsync/crash-durability guarantee; no network-filesystem guarantee is added.
Message count is checked under the cooperative lock; it is not a quota for
external writers that ignore that lock. wire_bytes also caps the individual rendered delivery, not total disk
usage. Published files are never modified by this API.

`Maildir.remove(identifier)` validates a base identifier, finds exactly one
current entry through a bounded directory snapshot, then unlinks that entry.
MissingStoredMail(identifier) exposes identifier()/text(). No body is read;
other files and directories are untouched. As for reading, callers must own
the path namespace; external non-locking flag moves can produce FileSystemFailure.
Keyword mappings remain separate work; local [mbox sessions](mbox-session.md)
use their own module and storage contract.

## Cooperative locking and flag changes

All Maildir operations now coordinate through the permanent regular file
`.okkama-maildir.lock` in the root. messages() holds a shared lock; add/remove
and flag changes hold an exclusive lock through commit and cleanup. Acquisition
is immediate: contention throws UnavailableFileLock, with no sleep, spin or
implicit retry. The lock file is created when first needed and never deleted or
replaced. Reads consequently need access to that lock coordinate as well as the
message files. Static symlink/non-file lock entries are rejected. This advisory
protocol coordinates this library's users, not Dovecot/qmail or hostile writers.
The existing trusted-local namespace requirement remains in force.

`ChangedMaildirFlags(origin,identifier,flags).result()` performs an explicit
filesystem change and returns StoredMail. Construction validates a base ID and
snapshots the set once (max 256), rejecting keywords and Recent. result() holds
the exclusive lock, locates a unique source, reads/parses it within MIME/byte
limits, prepares the returned snapshot, then atomically renames into cur using
a canonical DFRST suffix. The base ID, file contents, inode and mtime survive.
Repeated calls with the same flags return the same state; a canonical source
already at its destination is not renamed. Existing duplicate identities,
invalid MIME, excessive length/bounds or unsupported flags fail before commit.
On rename failure, the original path remains. The returned snapshot is fully
prepared before rename; a subsequent lock-release failure is reported as
CommittedMaildirFailure. Cooperation is required to prevent an external
non-locking writer from racing the destination check/rename.
