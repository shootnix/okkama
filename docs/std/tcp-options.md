# TCP option values

`TcpOptions()` and `TcpOptions(keep_alive is bool, no_delay is bool)` retain
connection settings. Defaults are false/false: no implicit keepalive and no
request to disable Nagle's algorithm. `keep_alive()` and `no_delay()` observe
exact booleans. No socket option is applied by constructing a value.

`TcpListeningOptions()` defaults to address_reuse=false, dual_stack=false and
backlog=128. The explicit constructor takes these three values and exposes
`address_reuse()`, `dual_stack()` and `backlog()`. The listener must request its
options before bind; dual-stack behavior is never silently enabled by this value.
The [numeric TCP provider](tcp-stream.md) now applies these requests before bind.

Backlog must be non-negative; InvalidArgument preserves the attempted count in
its diagnostic. Zero is a valid request, and any non-negative int is retained
without truncation. The OS may cap or otherwise interpret the requested backlog;
this pure value does not promise an effective queue size. Default 128 is a
portable library request, not a queried kernel setting.

Both entities are immutable and perform no I/O or capability checks. Equality
includes all settings. Unsupported settings must be reported by the eventual
socket operation rather than silently discarded in construction.

Before implementation, tests cover default/explicit equality, every boolean
combination, repeated observations, distinct settings, zero/large backlog,
negative backlog and exact InvalidArgument diagnostics.
