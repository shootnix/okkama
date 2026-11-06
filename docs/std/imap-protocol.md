# IMAP4rev1 authenticated protocol layer

This layer operates on an **already authenticated transport** supplied by a trusted
caller. It does not connect, verify TLS, send credentials or process the initial
greeting. `ImapConnecting`, `ImapServer` and `FromMail(Mailbox)` remain blocked by
HostName/IDNA, TCP/TLS and mail authentication/address dependencies. Full IMAP is
not complete. [Queue](stdlib-completion-todo-v4.md),
evidence,
[executable example](../../examples/std-v4/imap-session.ok).

The accepted wire mode is IMAP4rev1 with modified UTF-7 mailbox names and the
existing bounded ASCII response framing. Rev2-only and UTF8=ONLY advertisements
are rejected before I/O. A dual-revision advertisement does not activate rev2;
MOVE, UIDPLUS and IDLE must each be advertised explicitly. There is no ENABLE or
UTF-8 negotiation. Unknown framing grammar, unknown backslash system flags,
LIST extensions and ESEARCH extensions outside the documented subset fail
explicitly. Literal content remains opaque bytes. Unsupported modes must not be
silently interpreted with the wrong mailbox encoding.

## Values and parsing

- `mail/imap_search`: `MailMatching.key()` returns a validated `ImapSearchKey`,
  never unchecked criteria text. `AllMail`, `UnseenMail`, `SinceMail(Date)`,
  `AndMail(left,right[,limits])`, `OrMail(left,right[,limits])` and
  `NotMail(origin[,limits])` are immutable representations. SINCE uses the
  supplied calendar date without timezone conversion. `ImapSearchLimits` bounds
  nodes/depth/wire bytes (defaults 256/32/8192). Empty/incorrect operator arities
  are rejected. Source children are observed once at construction.
- `mail/imap_uid`: `ImapUid(BigInt)` and `ImapUidValidity(BigInt)` are distinct
  nonzero unsigned 32-bit values; `ImapSequence(int)` is a session sequence
  number. `ImapUidSet(Sequencing<ImapUid>[,limits])` snapshots/deduplicates in
  first-occurrence order. Input entries and wire bytes default to 4096/65536.
  Empty results are valid; requesting a command argument for an empty set fails.
  `MailboxUids(name,validity,uids,account)` binds identifiers to all three
  coordinates. Account scope is an explicit nonempty caller-assigned string;
  callers must distinguish accounts. The three-argument unscoped constructor is
  a parsing/value helper and is refused by session actions.
- `mail/imap_data`: `ParsedImapData(response[,limits]).entries()` exposes bounded
  atom/quoted/literal/literal8/list values. NIL, empty strings and empty lists
  stay distinct. Defaults: 8192 tokens, 64 list levels, inside framing bounds.
  `ImapValue.bytes/string/entries` enforce the appropriate scalar/list access;
  string decoding is explicit UTF-8. Section attribute syntax remains one atom.
- `mail/imap_metadata`: `ImapMailbox(response)` parses basic LIST/LSUB, with
  `name`, optional `delimiter`, `hierarchical`, `selectable` and string
  `attributes`; hierarchy flags are not message flags. `ImapStatusCode` exposes
  optional bracketed status codes. `ImapSearchResult(response,tag,limits)` parses
  SEARCH and ESEARCH UID/ALL/COUNT/MIN/MAX, checks correlator, count and extrema.
  This full-identifier operation refuses nonempty ESEARCH results without ALL.
  `ImapUidRanges` expands bounded numeric intervals in ascending order regardless of endpoint
  direction before
  deduplication; wildcards and ranges exceeding the entry budget are rejected.
- `mail/imap_state`: `ImapMailboxState(result,read_only)` requires successful
  selection with FLAGS, EXISTS and UIDVALIDITY. Optional RECENT, UIDNEXT and
  first-unseen sequence have presence predicates. SELECT UNSEEN is not an unseen
  count. Successor `observed(response)` preserves old snapshots, processes
  EXISTS/RECENT/EXPUNGE/FLAGS/UIDVALIDITY, and invalidates stale observations.
  EXAMINE remains read-only even if the server sends READ-WRITE.
- `mail/imap_message`: `ImapFetch(response)` represents even unsolicited partial
  FETCH. `ImapMessage(response)` additionally requires UID. Accessors distinguish
  missing/NIL content from empty content. Headers and whole messages use existing
  mail parsers; body bytes are exact transport content, not MIME-decoded text.
  Attribute names are case-insensitive and duplicates are refused. At most 128
  attribute/value pairs are accepted; unknown attributes retain structural data.

## Commands and ownership

`mail/imap_command` exposes closed typed `ImapCommand` constructors for basic
commands, mailbox operations, UID search/fetch/store/copy/move/expunge and APPEND.
Use named arguments where constructor arities overlap. Command lines are bounded
at 65536 ASCII bytes and mailbox names are always quoted. FETCH always requests
UID; content uses BODY.PEEK. STORE rejects the observational Recent flag. APPEND
snapshots message bytes once, retains Bcc, limits content to 16 MiB and rejects
NUL in an ordinary literal. It uses one synchronizing literal, sent only after
one continuation. No raw command-string constructor or credential command exists.

`mail/imap_command_reading` collects one command through an exact `ImapTag`.
`ImapCommandReading(input,tag)` or `(input,tag,prefix,limits)` exposes one cached
lazy `Future<ImapCommandRead>`. The result retains untagged responses, completion,
consumed bytes and exact remainder; NO/BAD are aligned results and
`require_success()` raises `ImapCommandRejected`. Defaults are 4096 responses,
64 MiB aggregate wire bytes and 4096-byte read chunks, with separate existing
per-response limits. Both completion and continuation count toward the aggregate.
Unexpected tags, BYE (except LOGOUT), continuation, EOF and invalid input fail.
The borrowed reader is not closed by this adapter; its owner must discard
alignment after a started failure or cancellation.

`mail/imap_exchange` provides `ImapCommandChannel(input,output,owner,prefix,limits)`.
It owns `owner does Closing`, serializes whole exchanges, assigns monotonic tags
O1, O2, … only after lock acquisition, and transfers unread bytes to the next
exchange. `request(command).result()` is cached and never retries. A started
write/read/protocol failure or cancellation closes the transport; an unstarted
or queued cancellation emits nothing. NO/BAD preserve a healthy aligned channel.
`close()` is idempotent abort, and destruction performs best-effort abort.
Acquisition and authentication belong to the caller; give this channel exclusive
access to its transport. Low-level helpers are importable infrastructure, not an
alternative safe application session API.

`mail/imap_session` provides `ImapSession(channel,capabilities,account)` and cached
operation objects. Selection, validation and response observation share a lock.
`mailboxes()` lists typed observations; `mailbox(name)` selects and `examine(name)`
selects read-only. `SelectedMailbox.state()` is an asynchronous snapshot. A new
selection expires the previous selection before sending SELECT, including when
SELECT fails. Successful RENAME/DELETE conservatively expire selection.
`SelectedMailbox.close()` aborts its owning connection; it never emits the
expunging IMAP CLOSE command. Session destruction also aborts.

`SearchedMail(selected,matching).uids()` yields account-scoped `MailboxUids`.
UIDNOTSTICKY is explicitly unsupported for these reusable snapshots.
`FetchedMail(selected,uids,data).messages()` and
`ChangedMailFlags(selected,uids,flags,change).messages()` return observed messages,
not fabricated responses for absent UIDs. Actions check account/name/UIDVALIDITY
before I/O and again after observations. Read-only mutations are refused without
poisoning the channel. Unsolicited mailbox updates affect the next snapshot.

`mail/imap_changes` adds cached `CopiedMail`/`MovedMail(...).uids()`,
`ExpungedMail(selected,uids).identifiers()`, `AppendedMail(...).uids()` and
`CreatedImapMailbox.result`, `ImapMailboxDeletion.delete`,
`RenamedImapMailbox.result`, `ImapSubscription.apply`. Mailbox mutations return
the acknowledged requested name, not invented LIST metadata. MOVE has no implicit
copy/delete fallback; targeted UID EXPUNGE requires UIDPLUS. EXPUNGE output consists
of observed sequence numbers (possibly including unsolicited updates), not UID
identities. Assigned UIDs are available only with APPENDUID/COPYUID and advertised
UIDPLUS. Absent assignment is `ImapAssignedUids.known()==false`, not UID zero.
COPYUID mappings must be one-to-one subsets of the requested source UIDs.
Reversed range endpoints do not reverse mapping order. Single-message APPENDUID
requires a scalar UID, never a range or set.

## IDLE and cancellation

`MailboxEvents(selected)` is inert until its first `event()`/`next()` await. IDLE
must be advertised. Events retain the exact response and the resulting immutable
mailbox snapshot; sequence/UID/flags access is available only for applicable
response kinds. `close()` is **async**: signal stop, wait for continuation, send
DONE once, and drain tagged completion. Ordinary session operations perform the
same shutdown before sending their own command. The response read is not cancelled
merely to send DONE, so an in-flight literal keeps its framing state.

The event queue capacity equals the finite command response budget, preventing a
full queue from blocking a requested stop. IDLE ends or fails at those same
response/byte bounds. There is no automatic 29-minute restart or keepalive.
Cancelling an event wait leaves the owned IDLE running; explicitly close the event
stream afterwards. Cancelling a started graceful close aborts the transport.
Destroying an active stream aborts the session. Compose deadlines with `Within`;
no hidden wall-clock timeout or reconnect/replay policy is introduced.

Errors remain specific: invalid identifiers/data, bounds, missing observations,
stale UID scope, command rejection, unexpected tag/continuation, server close,
unsupported operations and channel closure. Original source errors propagate.
After a transport/protocol failure, create a new authenticated channel; no method
claims that retrying a failed mutation is safe.

Protocol references: [RFC 3501](https://www.rfc-editor.org/rfc/rfc3501.html),
[UIDPLUS](https://www.rfc-editor.org/rfc/rfc4315.html),
[IDLE](https://www.rfc-editor.org/rfc/rfc2177.html),
[ESEARCH](https://www.rfc-editor.org/rfc/rfc4731.html).
