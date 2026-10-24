# Internal HTTP2 client preface and peer SETTINGS

`http/internal/http2_preface::Http2ClientPreface(input,output,local).result()` is one lazy, memoized client-side operation over borrowed AsyncReading/AsyncWriting. It emits the exact 24-octet client magic followed by one non-ACK SETTINGS frame, reads exactly the server's first frame, applies its non-ACK SETTINGS, and sends an empty SETTINGS ACK. Repeated result awaits do not resend or reread. The next frame remains on the stream for the connection loop, including the later ACK of our own initial settings.

The caller must select HTTP2 before starting: verified TLS with exact ALPN `h2`, or explicit prior knowledge for cleartext. This helper neither negotiates TLS nor exposes a usable HTTP2 exchange/session. Caller owns physical transport disposal after cancellation, malformed peer, partial write or network failure. Per-operation groups drain pending reads/writes before cancellation completes; no input starts after a failed prefix write and no ACK is emitted for invalid settings. Deadlines belong to the owning connection/client.

`http/internal/http2_settings::Http2PeerSettings` retains received protocol advertisements separately from bounded public `Http2Settings`. Full uint32 table/stream/header-list observations allocate no corresponding storage. Default table/window/frame are 4096/65535/16384. Explicit flags distinguish unlimited initial concurrency/header-list values from advertised values (including zero); default getter placeholders are UINT32_MAX. Consumers must honor the flags. Push values describe SETTINGS only; they grant no push capability in this implementation.

`ReceivedHttp2Settings(frame,previous,from_server).value()` checks structural framing and non-ACK type, processes values in wire order and preserves omitted settings. Unknown identifiers are ignored. Duplicates take the last valid value, but an earlier invalid value cannot be masked. Initial-window overflow is connection FLOW_CONTROL_ERROR (3); invalid maximum frame size or push value is connection PROTOCOL_ERROR (1). Server ENABLE_PUSH=0 is valid, 1 is forbidden; client 0 and 1 are valid. The server wire emitter omits ENABLE_PUSH by choice, not because zero is forbidden. These rules follow [RFC 9113 sections 3.4 and 6.5](https://www.rfc-editor.org/rfc/rfc9113.html#section-6.5).

This value fold is separate from the implemented live client session/window/HPACK engine. Live SETTINGS updates must be applied in wire order; collapsing to the last values loses intermediate window effects. See [owned H2 client](http2-client.md) for the current client profile. Server session integration and dynamic local SETTINGS updates remain open.

Tests: 477 six assertions; 478 ten lifecycle/wire assertions; 479 three real loopback CLI assertions. Executable `examples/std-v4/http2-preface.ok` validates exact client magic/SETTINGS and actual ACK over one socket, using explicit cleartext prior knowledge. It is an internal wire example, not an HTTP exchange.

## Server side

`Http2ServerPreface(input,output,local).result()` is a separate borrowed operation for an already selected server-side H2 stream. It sends server SETTINGS first (no client magic, no ENABLE_PUSH), verifies the exact client magic incrementally, accepts only a first non-ACK client SETTINGS frame, applies it with client direction and writes ACK. Read requests stop exactly at each boundary. Repeated awaits share the result; cancellation drains pending read/write work. The caller supplies deadline and transport disposal.

`Http2ConnectionInput(...,budget,false)` receives client SETTINGS. Its older overloads remain client-side. An additional final `Http2ReceivingBarrier` collaborator may await `published()` after complete HEADERS publication, so a supervised stream owner can acquire its body buffer before DATA. The barrier must not wait for application completion. Header decoding context remains advanced even when a subsequent stream-level semantic failure occurs.

Server foundation evidence: 4 files / 35 assertions PASS, including client compatibility. A server session is still a separate consuming task.
