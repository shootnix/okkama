# HTTP body opening and owned reading

BodyOpening is the nominal contract `length() is BigInt`, `replayable() is bool`,
`reading() is BodyReading`. BodyReading has `async bytes(limit) is @byte`,
`trailers() is Headers`, `closed() is bool`, `close() is void`. Concrete readers
also conform to AsyncReading and Closing. No user-visible Future<void> is added.

ByteBody(bytes) and EmptyBody() are immutable replayable values with exact known
lengths. Each reading() creates an independent owned reader at the beginning.
OneShotBody(reader) adopts an existing reader and permits one reading() through
an atomic claim; later claims throw BodyAlreadyOpened. Its length is unknown and
throws UnknownBodyLength. Dropping an unopened OneShotBody closes its adopted
reader; after opening, lifetime belongs to the returned reader.

StreamBodyReading(input is AsyncReading,owner is Closing,trailers is Headers)
adopts an exclusively used, **already body-delimited** source and a predeclared
trailer snapshot. It is not an HTTP wire framer and cannot decide connection reuse.
It serializes reads, enforces positive limits and refuses an oversized source
result. Each call returns a lazy reusable Future; re-await does not read twice.
Empty output is clean EOF and remains empty. Trailer observations are permitted
only after clean EOF, including after subsequent explicit close. Early close or
failure never makes trailers available. Clean EOF releases the owner; closed()
tracks explicit/abnormal body termination, so repeated EOF remains observable.

Unstarted/queued read cancellation emits no source read and leaves the owner
healthy. Started read failure/cancellation aborts it. close() is idempotent,
requests cancellation of the active source Future and closes the owner. Awaiting
the cancelled read drains nested cleanup; synchronous close itself cannot await
cleanup. Pending Future storage holds at most one read. Each read owns one FutureGroup;
its cancellation suppresses only expected child cancellation and retains typed
secondary cleanup failures. Completed reads never accumulate in a shared group. No implicit drain, retry,
full-stream buffer, connection pooling decision or background worker is created.
Byte readers store their supplied immutable bytes and only materialize requested
slices. Owners must implement their ordinary Closing contract.

Errors: BodyClosed, BodyAlreadyOpened, UnknownBodyLength, TrailersUnavailable;
nonpositive limits use InvalidReadLimit, excessive source output uses InvalidReading.
Original typed source errors and panic retain their normal runtime semantics.

FileBody is BLOCKED in this slice: v4 FileBytes currently exposes synchronous
native reads, not cancellable file operations or a pinned replay snapshot. A thin
async wrapper could block the scheduler (including for a changed path/FIFO) and
would not establish byte-identical replay. No full-file materialization or false
cancellation promise is substituted. Return with a suitable filesystem capability.
HTTP/1 bounded draining and HTTP/2 stream reset remain protocol-layer tasks.
