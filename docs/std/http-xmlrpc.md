# Bounded XML-RPC

`http/xmlrpc` provides immutable standard values, call/result codecs and explicit
HTTP client/handler adapters. Broad `XmlRpcValue` references expose kind and
`print(XmlRpcWriting)`; a recipient gets exactly one typed event, without casts
or escaping role restrictions. Integer/boolean/text/float/date/binary/array/struct
are supported. Containers snapshot bounded sequences; structs contain ordered
`XmlRpcMember` entries and reject duplicate exact names. This replaces the v3 map
candidate and avoids reflection or a generic runtime map provider.

Struct uniqueness sorts a separate immutable name list and compares adjacent
names; members keep source order. Name budgets and source failures still precede
duplicate validation. Empty names and exact Unicode identity are unchanged.

Integers are signed 32-bit. `XmlRpcFloat(float)` accepts an existing finite builtin
float. Parsed doubles use `XmlRpcDouble(Decimal)` and the writing event carries
Decimal: decoding preserves the decimal wire number instead of silently rounding
it to a machine float. Magnitude is bounded by the binary64 finite maximum; exact
precision and subnormal interpretation belong to the application/peer. Wire uses
decimal point notation, not exponents/NaN/Infinity. LocalDateTime has whole seconds
and no timezone; wire is YYYYMMDDTHH:MM:SS. Binary uses strict standard Base64,
accepting XML whitespace on input. Strings use Unicode/XML1 character rules and
only five predefined or valid numeric character references.

`XmlRpcCall(method,parameters)` checks the XML-RPC identifier alphabet and 256
character method budget. `XmlRpcSuccess(value)`/`XmlRpcFailure(fault)` implement
XmlRpcResult; wrong value/fault projection throws WrongXmlRpcResult. Fault structs
must contain exactly typed faultCode and faultString, with no extra members.

`XmlRpcLimits()` defaults to depth64, 4096 values, 1 MiB cumulative text and 1 MiB
decoded binary, and nil disabled. Explicit four-argument budgets accept zero and
bounded BigInt bytes; the fifth boolean explicitly enables nil after the caller
has negotiated it. The enum name is nil_value because nil is reserved syntax.
Wire budget is derived from escaping/Base64/structure and capped at 16 MiB;
member names are at most 1 KiB. Depth/value/text/binary failures are typed. Encoding
publishes no partial packet. A custom value must emit exactly one visitor event.

`XmlRpcCallWire`/`XmlRpcResultWire` return bounded UTF-8 packets;
`ParsedXmlRpcCall`/`ParsedXmlRpcResult` consume bounded snapshots. The protocol
grammar reuses existing plist lexical primitives without changing them. DTDs,
external entities, other processing instructions, attributes, mixed scalar/type
content, extra response parameters and arbitrary extension tags are rejected.
Only UTF-8 and the existing restricted XML1 declaration spellings are accepted.
No document/network/entity resolver or general XML subsystem is acquired.

`XmlRpcClient(exchange,endpoint,limits)` performs POST through supplied Exchanging;
it creates no transport, redirect/auth/cache/retry policy. Requests explicitly
supply User-Agent, Content-Type, Accept and Content-Length. Non-2xx status throws
XmlRpcHttpFailed before XML interpretation. Response media must be text/xml or
application/xml with absent/UTF-8 charset. Body reads are bounded and owned;
failure/cancellation closes and joins them.

`XmlRpcHandling(path,calling,limits)` matches an exact raw path, accepts only POST,
and calls a supplied XmlRpcCalling. Wrong path/method/media produce404/405/415;
bad input400; quota413. Application results/faults encode into200 with explicit
length/media; ordinary application failures retain normal server500 behavior.
Handler cancellation owns and joins application work.

HTTP bodies are deliberately materialized up to the wire cap before token parsing.
This is a bounded snapshot codec, not a streaming XML DOM. Counters are enforced
during recursive value decoding before publication; a scalar can temporarily
occupy its bounded wire representation. Use smaller explicit limits for untrusted
workloads. Performance/verification,
[example](../../examples/std-v4/http-xmlrpc.ok),
[XML-RPC specification](https://xmlrpc.com/spec.md).
