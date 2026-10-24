# HTTP1 continue coordination

Http1Session supports one case-insensitive `Expect: 100-continue` value on a
HTTP1.1 request with content framing. Unsupported/ambiguous expectations,
HTTP1.0, empty/fixed-zero framing and known empty sources are rejected before
head emission. Shared Http1BodyCheck validates known lengths and budgets before
any head bytes; neither construction nor early final response opens the body.

The head is emitted first. Only the first 100 response opens/transmits the body;
103 or other informational responses do not grant permission. Repeated 100
responses are retained but cannot reopen/resend. Fixed/chunked output retains
the existing bounded source/sink ownership. Normal requests still use complete
message output. An early final response leaves the source unopened and forbids
connection reuse because its declared request content was never transmitted.

Continue waiting defaults to one second after head completion. The full constructor
`Http1Session(input, output, owner, limits, continue_wait is Duration, clock is Timing)`
accepts an injected nonnegative duration and monotonic clock. One absolute deadline
is retained across informational responses. The test advances time by two fifths
before 103 and verifies only three fifths remain for the next wait. Existing Before
semantics allow already-ready results; this is a waiting deadline, not CPU preemption.
Timeout raises TimedOut, closes transport and drains the head/source operation.
Parent cancellation likewise drains Before and source cleanup without uploading.

There is no automatic upload-on-timeout or 417 retry in this supplied-stream
session. A retry policy must acquire an appropriate new connection and preserve
method/source ownership. Dedicated upgrade/tunnel handling and Internet routing
remain separate. [RFC 9110 Expect](https://www.rfc-editor.org/rfc/rfc9110.html#section-10.1.1),
[RFC 9112 persistence](https://www.rfc-editor.org/rfc/rfc9112.html#section-9.3).

[Real TCP handshake example](../../examples/std-v4/http1-continue.ok),
ownership investigation,
evidence.
