# Bounded HTTP1 body writing

`http/http1_body_writing`: Http1BodyWriting(origin is AsyncWriting,body is
BodyOpening,plan is Http1BodyPlan,head is Headers) or
`(origin,body,plan,head,limits is Http1Limits,chunk_bytes is int)`.
result() is one memoized Future<AsyncWriting>. Chunk size is 1..65536, default
4096. Construction opens no body and performs no sink write. Tunnel/upgrade
plans require separate protocols. This low-level operation trusts the caller's
chosen plan; the actual head must advertise matching framing. head supplies
the Connection trailer prohibition context, not an inferred replacement plan.

A started nonempty operation checks available length metadata before opening
one BodyReading. UnknownBodyLength is accepted explicitly. A known fixed-length
contradiction fails before open/write. Claimed lengths above the body ceiling
are rejected before opening. Empty/HEAD framing never observes or opens the
hypothetical body. The supplied BodyOpening and sink remain borrowed until an
actual reader is opened; callers retain responsibility for an unused source.

Fixed output writes exactly its declared bytes, then requests one more byte to
verify source EOF. Premature or extra data is an unrecoverable framing failure;
already-emitted bytes cannot be rolled back. Chunked output emits lowercase hex,
CRLF/data/CRLF, then zero chunk plus validated, separately serialized trailers.
Unknown valid application trailer fields require their definition's permission;
the codec checks known forbidden fields/Connection nominations. Fixed and
close-delimited output reject nonempty trailers. Close-delimited output emits
payload through source EOF; the session owner must then terminate the connection.

Actual payload is counted cumulatively against body_bytes. Emitted chunk lines,
terminators and zero/trailer section together are bounded by head_bytes; lines
and trailer counts use line_bytes/fields. A limit may fail after a written prefix,
but no byte beyond that checked ceiling is emitted. Many tiny chunks cannot
bypass the metadata budget. Body reads and wire chunks are bounded to configured
chunk size; native ByteBuffer/slices assemble output without per-byte copying.

The opened reader closes on success, failure or started cancellation. Source read
and sink write have separate structured groups; each direction retains only its
current group, replaced after await. Thus completed chunk Futures do not accumulate
with total body size. Cleanup cancels both active directions and settles both,
including deferred source/sink cleanup, before publishing failure. A sink itself
is never closed. Unstarted cancellation performs neither open nor write; errors
and results are cached. A failed started operation invalidates the protocol
session; no implicit retry or buffering of the whole body is supplied.

Evidence,
[head/body Unix round-trip](../../examples/std-v4/http1-body-writing.ok).
