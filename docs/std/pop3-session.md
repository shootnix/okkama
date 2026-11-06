# POP3 transaction session — contract

The session consumes an exclusively owned **already authenticated** transport.
It neither authenticates nor certifies TLS. `PopServer`/`PopConnecting`, verified
TLS and USER/PASS remain blocked by the existing HostName/IDNA, TCP/TLS and
SecretBytes dependencies. There is no raw-command or credential-string API.

`mail/pop3` provides immutable protocol observations: `PopNumber(int)` (positive
31-bit supported message-number profile), `PopIdentifier(string)` (1..70 visible
ASCII bytes), `PopMessage(number,size,identifiers)` (zero or one identifier),
`PopCapabilities(PopReply)`, `PopListingLimits(entries,decimal_digits)` (defaults
4096/40), and checked STAT/LIST/UIDL representations. Sizes use BigInt. UIDL
absence has an explicit predicate and typed missing-identifier error. Numbers
are session positions, not reusable UID identities; identifiers are scoped by
maildrop/account externally. Duplicate UID strings are allowed: identical
message copies can have the same UIDL. Duplicate message numbers are rejected.

`mail/pop3_session::PopSession(input,output,owner,capabilities,prefix,reply_limits,
listing_limits)` owns `owner does Closing`, serializes complete operations and
retains coalesced unread bytes. The caller supplies a post-auth CAPA snapshot;
a negative CAPA explicitly means unavailable optional capabilities. Constructors
perform no I/O. `mailbox()` is a live capability retaining the session;
`mailbox().state()` asynchronously returns an immutable STAT snapshot with
count()/size(). `messages()` joins LIST and optional UIDL under the same lock,
with exact number-set matching; absence of advertised UIDL is explicit in every
message. `bytes(number)` retrieves exact RETR content; `message(number)` explicitly
parses it using existing MailMessage/MIME limits. `delete(number)` marks a message
via DELE; `PopDeleteReset(session).reset()` sends RSET. Both are async void.
`PreviewedPopMail(mailbox,number,lines).bytes()` caches one lazy TOP result,
including failure/cancellation. TOP requires advertisement and nonnegative lines.

`session.quit()` is async void and requests UPDATE, then closes the transport.
Only a positive response confirms completion. Negative or lost QUIT completion
must not be interpreted as rollback: some deletions may already have happened.
There is no implicit retry. `close()`/destroy abort without QUIT and never request
publication of pending deletes. Close is idempotent; destroy is best-effort.
A Future is reusable; separate calls to session actions represent new commands.

All waits use ordinary cancellable async roles and compose with `Within`.
Cancellation before lock acquisition emits no command. Once an exchange starts,
write/read/framing/semantic failure or cancellation aborts the connection.
An aligned -ERR raises `PopCommandRejected` without poisoning the connection,
except QUIT always terminates it. Empty listings are valid. Listing entry/decimal
bounds are independent of reply line/body/wire limits; no unbounded read or
implicit pipelining. No wall-clock deadline is implicit.

Implementation and verification status is tracked in the completion queue.
Protocol basis: [RFC 1939](https://www.rfc-editor.org/rfc/rfc1939.html),
[CAPA, RFC 2449](https://www.rfc-editor.org/rfc/rfc2449.html).

## Operational limits and composition

LIST/STAT extension text after the numeric size is ignored as allowed by the
wire protocol; UIDL rows have exactly two fields. Matching UIDL rows preserves
LIST order and validates numbers even when the UIDL list is reordered. Persistent
seen-number sets and matching currently have quadratic worst-case work under the
explicit finite entry bound. No hashing, reflection or native POP3 backend is
introduced in this slice.

Use a typed `Within<@byte>` around a cached preview or other value-producing
operation, or bound a larger application workflow. `Within<T>` currently needs
an explicit type for some Future generic arguments. Effect-only operations stay
async void; the library does not invent a `Future<void>` value type. Trusted
protocol consumers may bound `session.run(typed_command)` to observe an actual
`PopReply`, including for DELE/RSET/QUIT; this is a protocol response, not a
fabricated business result. The same session lock and abort rules apply.

LIST/STAT/UIDL structural errors abort the session. A MailMessage/MIME parsing
failure after an entirely framed RETR is an application content error; it does
not lose wire alignment. `PopSession.close()` delegates cancellation of pending
transport I/O to its owner; owners must implement their normal Closing contract.
No implicit timeout, network reconnection or deletion retry is provided.

## Verification

Evidence: focused 4 files / 73 checks
PASS, two inline groups and example PASS. Full existing-suite regression ran
564 files / 10285 assertions; all POP3 tests pass, and the seven failing-file
signatures reproduce on clean baseline ddb3a7b. The full suite is not green.
