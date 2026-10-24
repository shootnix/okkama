# Received datagram value

`Datagram(source is IpEndpoint, bytes is @byte, truncated is bool)` represents
one captured datagram: its sender endpoint, retained payload and explicit
truncation observation. `source()`, `bytes()` and `truncated()` return these exact
values. Construction performs no socket I/O and does not verify the sender.

The payload is an immutable byte array: embedded NUL, non-UTF bytes and an empty
payload are valid. An empty UDP packet is a packet, not stream EOF. The truncation
flag is independent of payload length; an empty retained prefix can be marked
truncated. The value cannot recover discarded bytes or infer their original size.
No encoding, size cap, concatenation or inferred truncation is applied here.

Datagram implements the existing ByteSource role for its finite retained payload.
This permits explicit forwarding through a future datagram sender without making
the packet a Reading/AsyncReading byte stream. The destination of any forwarding
operation is supplied separately; ByteSource does not forward sender metadata.

Equality includes sender (address/port/zone), payload and truncation flag. Repeated
observations retain the same snapshot; constructing another packet cannot change
the first. IPv6 scope remains on the sender's IpEndpoint, not on IpAddress.

TDD covers binary/empty payloads, both truncation flags, scoped and canonical
senders, equality of equivalent input, distinct metadata/payloads and nominal
ByteSource consumption. Actual UDP receive/send boundaries and truncation
detection remain open until UdpSocket and its provider operations are implemented.
