# Asynchronous stream contracts

`net/stream` begins with nominal roles AsyncReading and AsyncWriting and the
immutable generic ConnectionEndpoints value. Concrete socket halves, connection
ownership, overlapping-operation coordination and cancellation-safe partial I/O
remain separate open tasks; role declarations do not claim those implementations.

`AsyncReading.async bytes(limit is int) is @byte` requires a positive limit and
returns at most that many bytes. InvalidReadLimit from io/stream is the common
failure for non-positive limits. Short reads are valid; empty means orderly EOF,
and EOF remains empty on subsequent reads. Each call creates a lazy Future;
awaiting that same Future again observes its cached outcome, not another read.

`AsyncWriting.async write(source is ByteSource) is AsyncWriting` consumes the
entire source or throws and returns the same writing capability on success.
ByteSource is the existing nominal synchronous byte materialization contract;
source evaluation occurs inside the started asynchronous operation. A write can
have externally visible prefix effects before failure; callers cannot assume
rollback or safely replay an uncertain write. No flush() or synchronous Writing
conformance is provided. Await operations explicitly; the synchronous `<-`
operator does not adapt an AsyncWriting value.

Concrete implementations must propagate source/I/O failures and cancellation.
Deadlines are composed by Within around the operation's Future. The roles do not
own resources or enforce scheduling; socket-specific close/queue rules remain
open until StreamReading/StreamWriting are implemented and tested.

`ConnectionEndpoints<T>(local is T, remote is T)` retains the exact endpoints and
returns them through local_endpoint()/remote_endpoint(). No resolution, canonical
conversion or connection occurs. Swapping endpoints changes identity unless equal.
The common T is explicit when required by generic inference; no any payload schema
or raw sockaddr bytes appear in this observation.

TDD uses a ByteBuffer-backed test implementation to verify nominal async dispatch,
short reads/EOF, lazy single execution, complete ByteSource writes, typed failure
propagation and rejection of synchronous role implementations. These fixtures
exercise contracts; they are not public socket adapters or proof of network I/O.
