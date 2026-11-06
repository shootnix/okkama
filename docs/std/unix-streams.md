# Unix streams and acquisition

UnixEndpoint is an immutable observation: UnixEndpoint() is unnamed;
UnixEndpoint(address is UnixAddress) is named. named() distinguishes them;
address() on an unnamed endpoint throws InvalidArgument. No empty Path is used.
UnixConnection.endpoints() returns ConnectionEndpoints<UnixEndpoint> captured
from the host and usable after closure. Abstract addresses remain unsupported.

UnixPaired().async result() acquires a fresh UnixPair per started Future, owning
two nonblocking connected UnixConnections. first()/second() retain their owners;
re-awaiting a Future does not create another pair. Unstarted cancellation
acquires nothing. Partial acquisition failure closes both unpublished handles.

UnixConnection.input() and output() return retained halves implementing
AsyncReading and AsyncWriting. They keep their connection alive. One pending
read and one pending write may coexist; overlap in the same direction reports
resource_exhausted. Reads require a positive limit, return at most that limit
(native chunks are at most 65536 bytes), allow short reads and retain EOF.
Writes sample ByteSource once after startup, write the complete bytes through
partial host writes, and return the same writing capability. Empty writes are
successful without emitting data. Re-await never repeats I/O. Pending readiness
is removed on cancellation/close. Read cancellation keeps the socket open;
started native write cancellation or terminal failure closes the connection,
since an externally visible prefix may already exist. Source failures before
native submission keep it open. SIGPIPE cannot terminate the interpreter.

UnixConnecting(remote).async connection() creates a fresh nonblocking stream
connection. Connect uses writable readiness and SO_ERROR plus peer observation;
would-block backlog retry never masquerades as successful connection. Failure
or cancellation closes the unpublished descriptor. Relative targets are resolved
to a stable absolute path at startup and byte limits are checked again.

UnixListening(local,backlog).async acceptance() creates an owned UnixAcceptance.
Backlog is nonnegative. No existing entry is unlinked or replaced during bind.
UnixAcceptance.connection() asynchronously accepts one fresh owned connection;
cancel leaves later clients available, close wakes pending accept. Accepted
connections survive listener close. local_address() is a captured absolute path.
Repeated awaits share the same acquisition/result; new calls acquire anew.

Listener close closes its descriptor then removes the pathname only if lstat
still reports the captured device/inode and socket type. Missing, replaced or
symlink entries are left untouched. Callers must control concurrent modifications
of the parent directory during acquisition and close: POSIX has no atomic unlink-if-inode, so
this identity check is not a security boundary against a concurrent rename.
Examples use private temporary directories. Descriptor closure still happens
when entry cleanup fails. No global chdir is performed.

Failures keep the listening/connecting/accepting/reading/writing/closing
operation (UnixListening combines bind and listen under listening);
unsupported AF_UNIX acquisition uses UnixSocketsUnsupported, closed
owners use NetworkClosed. Shared closure and deterministic destruction follow
existing socket ownership. No synchronous stream role, implicit flush, peer
credentials, abstract namespace or descriptor-passing API is added.

Host behavior: [unix](https://man7.org/linux/man-pages/man7/unix.7.html),
[connect](https://man7.org/linux/man-pages/man2/connect.2.html),
[unlink](https://man7.org/linux/man-pages/man2/unlink.2.html).
