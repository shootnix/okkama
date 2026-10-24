# `net/failure`: portable failure contract

NetworkFailure is an immutable observation of a failed network operation. Its
existing constructor `(operation is NetworkOperation, kind is NetworkFailureKind,
message is string)` remains unchanged. `operation()` and `kind()` expose the
attempt and category independently; `text()` preserves the supplied diagnostic,
including empty/Unicode text, without parsing it. No I/O or resource ownership.
Existing enum members and `NetworkClosed().text() == "network closed"` remain.

`NetworkError(operation is NetworkOperation, failure is NativeFailure)` binds an
explicit attempted operation to a normalized host failure. It is an adapter value,
not itself an Error. `applicable() is bool` observes whether this is a terminal
core.net failure. `value() is Error` applies the following policy lazily when
applicable, or throws InvalidArgument on misuse:

| Native code in domain `core.net` | Result |
| --- | --- |
| connection_refused | NetworkFailureKind.refused |
| connection_reset | NetworkFailureKind.reset |
| network_unreachable | NetworkFailureKind.unreachable |
| address_in_use | NetworkFailureKind.address_in_use |
| permission_denied | NetworkFailureKind.permission_denied |
| resource_exhausted | NetworkFailureKind.resource_exhausted |
| unsupported | NetworkFailureKind.unsupported |
| timed_out | `async/timeout.TimedOut` |
| would_block, interrupted | Not applicable; caller uses bare `throw;` |
| Every other portable code | NetworkFailureKind.other |

Mapped NetworkFailure values retain the caller-supplied NetworkOperation and
`failure.message()` exactly. The adapter never infers an attempted operation
from the native symbol or error message. The raw native symbol/domain/code,
errno, handles and provider objects are not stored in the resulting NetworkFailure.
No substring recognition or host-dependent numeric mapping belongs in std.

Foreign domains are not applicable, including their timed_out codes.
would_block/interrupted remain control outcomes for future nonblocking I/O loops;
this adapter neither retries nor converts them to terminal socket failures.
It catches no Error itself. Callers wrap only a native boundary and catch
NativeFailure; Cancelled, TimedOut and existing domain errors therefore pass
through unchanged. Native timed_out becomes the existing TimedOut type rather
than NetworkFailureKind.timed_out. The enum member remains for source compatibility
with explicitly constructed NetworkFailure values.

The current provider combines ECONNABORTED and ECONNRESET into connection_reset;
std preserves that information level and never guesses aborted from diagnostics.
The existing NetworkFailureKind.aborted remains valid for explicit observations.
Address-unavailable, broken-pipe, not-connected and resolver-specific failures
without a matching enum remain other; this slice adds no speculative categories.

Usage inside a NativeFailure catch at an explicit network boundary:

```okkama
catch failure is NativeFailure {
    let translated=NetworkError(NetworkOperation.connecting, failure);
    if translated.applicable() {throw translated.value();}
    throw;
}
```

The caller's bare rethrow preserves the original transport failure. Okkama does
not permit `throw failure;` for a NativeFailure value inside the adapter; the
current catch must own that rethrow. No hidden dynamic catch context is assumed.
Actual callers must select the operation they attempt. CPU-only tests inject
normalized native failures through a test registry. A real unsupported socket
family verifies the existing core.net boundary without opening a socket or using
external networking. Full TCP/DNS/poll/cancellation behavior remains in its own
roadmap sections.

Implementation references: `Runtime/NativeFailure.pm` defines the portable code
set; `Runtime/NativeFailure/Errno.pm` maps host errno before std; the existing
`Runtime/Native/Core/Net.pm` emits domain core.net. No provider change is needed.
