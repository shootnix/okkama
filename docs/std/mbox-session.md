# Local mboxrd sessions

`mail/mbox_store` builds on the [mboxrd codec](mbox.md). `Mbox(path[,MboxLimits])`
is a pure coordinate for an existing regular file; an empty file is a valid
empty archive. path()/limits() do no I/O. Missing files remain FileSystemFailure;
this API does not silently create an archive or accept symlinks/special files.

`Mbox.session()` / `MboxSession(origin is Mbox)` acquires an immediate exclusive
FileLock on the permanent regular file `path + ".okkama-mbox.lock"`, then reads
and parses the archive within its limits. Reads use chunks no larger than 65536
bytes or the remaining budget plus one. Every stream and scratch buffer is
closed; constructor failure releases the lease before propagating the error.
The sidecar is created when needed, never deleted/replaced, and remains locked
across replacement of the archive inode. Contention is UnavailableFileLock.

All participants must use this protocol and the same local path coordinate,
with trusted ancestors and a stable working directory for relative paths. Static
archive/lock symlinks and nonregular files are rejected. Separate lstat/open
checks do not protect against hostile path replacement. This is not a Dovecot,
system-spool, NFS or generic dotlock interoperability contract: mbox clients use
[different locking conventions](https://doc.dovecot.org/2.3/admin_manual/mailbox_formats/mbox/).

## Snapshots, IDs and successors

messages() returns an eager Sequencing<StoredMail>; records() returns an eager
Sequencing<MboxRecord>, preserving the explicit envelope separator metadata.
The returned sequences do not access the filesystem and remain usable after
mutation or close. Calling session methods after close throws InvalidState,
except idempotent close() and closed(). Status/X-Status/X-UID/X-Keywords remain
ordinary message headers; no untrusted header is interpreted as a storage flag
or identity. StoredMail.flags() is empty in this profile. Bcc is retained.

Each opened session receives a fresh 192-bit random namespace and assigns opaque
namespace/counter IDs to its messages. Duplicate messages have different IDs.
IDs survive removal of other entries and are never reused in a
successor lineage; reopening creates new IDs. They are not durable UIDs or paths.

```okkama
let session=Mbox(path).session();
defer session.close();
session=session.add(record); // MboxRecord includes explicit From_ metadata
let id=session.messages().first().identifier();
session=session.remove(id);
```

Both commands return `^MboxSession` and must be rebound. This revises the draft
v3 void-command signatures: Okkama fields are immutable and the ID/snapshot
state belongs in successors, not a native mutable metadata table. add accepts
MboxRecord rather than inventing sender/date metadata from MailMessage. The
existing codec defines separator validation and requires terminal CRLF MIME.

Session versions share the same lease. Explicit close on any version closes all
versions. The lease owns automatic cleanup; dropping one version does not close
a successor that escapes its scope. A saved older version remains an older
snapshot: mutations compare the exact current file bytes against that version's
original bytes, without hashes or positional-only identity. Different content
throws MboxConflict before replacement. Content restored byte-for-byte is
considered equivalent; this is not an external change-history tracker.

## Commit and failures

Each command checks the current bytes, builds bounded output and all returned
snapshots, then checks current bytes again after any borrowed body observation.
Adding observes the incoming body once; subsequent reads/renderings use parsed
snapshots. Count overflow fails before body observation. Exhausting the signed
64-bit session counter throws MboxLimitExceeded("identifiers"). Missing IDs throw
MissingStoredMail. No-op removals are not silently accepted.

A command writes/flushes a private temporary file in the archive's parent and
atomically renames it over the archive. There is no cross-filesystem copy
fallback. Rename is the commit point; before it, failures attempt cleanup and
leave the original file intact. Publication occurs while the session still holds
its permanent sidecar lock. Each command commits immediately; close is not an
implicit commit, and closing does not roll back prior successful commands.

If temporary-file cleanup fails after commit, CommittedMboxFailure(code,reason)
exposes code()/reason()/text(); the shared lease is closed and the caller must
reopen rather than blindly repeat the command. Other filesystem/MIME/bound errors
retain their types. Conflict checks detect stale cooperative snapshots and some
external changes; an external non-locking writer can still race the final check
and rename. Atomic visibility does not imply fsync/crash durability.

Replacement creates an owner-only 0600 file. Previous permissions, ownership,
ACLs, extended attributes, hard links and timestamps are not preserved; this
profile is for caller-owned local archives. Message semantic content and
separator metadata survive; MIME rendering may canonicalize wire spelling.
MboxLimits caps input/output and message count, and MimeLimits applies per
message. Snapshots/output are eager and bounded, not a streaming large-mailbox
implementation. There is no native/interpreter extension for these operations.
