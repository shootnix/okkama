# Internal HTTP2 control values

http/internal/http2_controls parses/emits RST_STREAM, PING ACK and GOAWAY over structurally validated internal frames. Unknown uint32 error codes are retained as observations. Http2ResetFrame uses positive uint31 stream IDs; code 8 emits canonical CANCEL. Stream existence/idle/closed legality belongs to the live state machine.

Http2PingReply(frame).frames() echoes exactly eight opaque payload octets with ACK flag 1 and other flags zero, or returns no reply for a received ACK. It never creates an ACK feedback loop. Actual scheduling/priority and connection output remain the owner's responsibility.

Http2GoAway retains reserved-bit-masked last_stream, full uint32 code and explicit opaque debug bytes. Debug is never part of an error text/log. Outgoing Http2GoAwayFrame caps debug at 1024 octets before serialization; incoming bytes remain bounded by the already enforced receive-frame budget. Http2GoAwayProgress permanently records stopping admission and a nonincreasing last-stream ID across multiple GOAWAY frames. Increasing the peer's last ID is a connection PROTOCOL_ERROR. Protocol rules follow [RFC 9113 sections 6.4, 6.7 and 6.8](https://www.rfc-editor.org/rfc/rfc9113.html#section-6.8).

This module does not reset a live stream, stop an actual connection, decide retry eligibility, interpret diagnostic bytes or own transport. Those effects belong to the forthcoming connection engine. 487 five cases; structural compatibility 475/487 twelve assertions PASS. Inline/example PASS. Std-only; no repeated full regression after native HPACK.
