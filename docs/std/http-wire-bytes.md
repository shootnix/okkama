# Internal bounded HTTP wire bytes

http/internal/wire_bytes::HttpWireBytes(chunks is @@byte,maximum is int).bytes() joins ordered immutable octet chunks through the existing runtime/buffer provider. Maximum is 0..16777224 (largest HTTP2 payload plus header); chunk count <=1024. Aggregate budget is checked before buffer creation. Empty chunks/zero-length output are valid. WireByteLimit has fixed payload-free text. Every invocation creates/explicitly closes a private opaque buffer and returns an independent byte snapshot; no raw state or transport effect escapes.

This removes interpreted per-byte concatenation from HTTP2 frames, raw HPACK strings, preface, header-block table-size update and GOAWAY emission. It changes no wire bytes or public protocol/ownership contract. Fixed small header/settings maps remain intentional. 488 four cases prove budgets, independence and exact complete 16 KiB DATA/string content; focused consumer compatibility covers framing, HPACK, continuation, control and actual TCP headers. Inline PASS. PERF-23 investigation records 16 KiB serialization 2.248 -> 0.868 seconds and remaining VM traversal work. Std-only, no repeat full regression.

The native byte adapter now preserves compact immutable storage rather than
creating a VM Byte per octet at every boundary. Explicit enumeration and
snapshots can still materialize values, and some providers still convert through
octet arrays. HTTP framing, strict byte identity and source/snapshot independence
are unchanged; no end-to-end latency or memory bound is inferred from this alone.
