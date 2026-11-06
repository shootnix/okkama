# Internal HTTP2 connection input

`Http2ConnectionInput(input,receiver,controls,local_settings,initial_peer).run()` is a lazy, single-use consuming loop after the client preface. It borrows the physical input and owns each pending frame/block read until cancellation cleanup settles. Per-iteration groups release completed operations instead of accumulating the connection history.

`Http2ReceivingEvents` publishes synchronous HEADERS, DATA, reset, window, settings, local-settings acknowledgment, GOAWAY, stream-fault and EOF events. `Http2ControlQueuing` queues replies without awaiting socket writes in the sole reader. The owner must bound those queues and implement stream admission, message validity, flow accounting and transport lifetime. DATA includes padding in flow charge.

The loop installs the successor HPACK context before publishing a complete header block. A stream-scoped callback fault can queue RST_STREAM for that stream and continue decoding a sibling with the advanced table. `stream_fault` decides whether a reset is appropriate for the owner's stream state. Structural stream faults are drained by the frame reader and continue the loop; connection faults propagate without a false EOF notification. Interleaved or standalone CONTINUATION aborts the connection.

Peer SETTINGS entries are applied individually in wire order before one ACK, preserving intermediate window-overflow checks. A single outstanding initial local SETTINGS ACK updates the effective HPACK limit; duplicate ACK is a connection error. PING is echoed once, ACK is never echoed; GOAWAY cutoffs cannot increase. Valid deprecated PRIORITY and unknown extensions are ignored, PUSH_PROMISE is rejected by frame rules. SETTINGS deadlines and later local update queues remain the owning session's responsibility.

490 eight cases and 483 nine compatibility assertions PASS, including cancellation inside a CONTINUATION read. No interpreter/provider changes and no full regression rerun. This consuming reader is internal; a complete public H2 client still needs the common writer, stream buffers, flow scheduling, ownership and ALPN integration.

An optional HpackBudget constructor argument supplies session header policy. An additional cancellation-after-recovered-stream-error case brings focused input cases to nine.
