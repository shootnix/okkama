# Owned HTTP1 sessions and response leases

`Http1Session(input is AsyncReading, output is AsyncWriting, owner is Closing
[, limits is Http1Limits])` adopts one explicit transport owner. Constructor
rejects an already closed owner. `async exchange(Request) is Http1Exchange`
returns the original request, parsed final Response and at most 16 informational
Http1ResponseHead values. `closed()` and `close()` implement Closing.

Exchange is lazy. An unstarted cancellation writes/reads nothing and acquires
no admission. A started request owns its complete-message output and response
head operations. A second exchange while a response body owns admission fails
with InvalidState without disturbing that body. There is no pipelining or
implicit queue, retry, route selection, DNS lookup, Host insertion or credential
forwarding. A started failure conservatively discards the transport.

The response exposes a one-shot BodyOpening with exact known/unknown framing
length. Its reader keeps admission until **verified EOF**, including fixed length,
chunk terminator and validated trailers. Reuse requires request and response
connection policy, a self-delimited body and no bytes beyond its boundary.
Unsolicited trailing bytes cannot become a later forged response. Empty and fixed
zero-length bodies are finalized before returning the exchange, so their unopened
body cannot retain admission. HEAD metadata does not create a payload.

Early reader close, dropping an unopened nonempty body, malformed framing,
source/sink failure, started cancellation and close-delimited EOF close the owner.
Closing an old reader after clean EOF cannot close a later active exchange.
Closing the session cancels pending exchange/body groups even if the supplied
owner's close action alone cannot wake its source. Await cancellation before
assuming cleanup has drained. Body reads reject overlap and require positive
limits; trailers are available only after clean EOF. EOF and aborted/explicitly
closed reader states remain distinct; repeated EOF is valid until reader close.

Each direction owns bounded startup groups; body reads retain at most one group.
Startup groups are settled and cleared before publishing the exchange, preventing
completed send sources/head tails from being retained by a live response reader.
Empty-body priming ownership is likewise cleared. No growing chunk Future history
or timing-budget relaxation is used. Failed owner closure leaves admission closed
but can be retried explicitly; successful close is idempotent.

Coordinated [100-continue upload](http1-continue.md) now sends the head before
opening the body and bounds waiting through an injected absolute deadline.
Unsupported/ambiguous expectations fail before effects. Informational responses
without an Expect request are also parsed and retained. Upgrade/CONNECT tunnel responses fail
explicitly and discard the ordinary session; a dedicated protocol handler must
own those bytes. This is supplied-stream HTTP1 machinery: it does not manufacture
IP/TLS/ALPN details or claim a complete Internet client, HTTP2 or a shared pool.

[Two exchanges on one real TCP connection](../../examples/std-v4/http1-session.ok),
ownership investigation,
evidence.
