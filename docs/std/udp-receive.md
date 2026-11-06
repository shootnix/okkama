# UDP receive

`async datagram(limit is int) is Datagram` on `UdpSocket` receives one ordinary
UDP packet. Calling creates a lazy Future; awaiting it consumes at most one packet,
and repeated awaits return the same immutable observation. Separate calls receive
separate packets. Empty payloads are packets, never EOF.

The positive limit bounds the retained prefix. Zero and negative limits throw
`InvalidArgument` before I/O. A packet longer than the limit is consumed once,
its suffix discarded, and `truncated()` is true. Exact-size packets are not
truncated. The sender's numeric address, port and IPv6 scope are preserved.

The provider uses a fixed 65536-byte receive buffer, independent of the caller's
limit, then retains at most that limit. This covers ordinary IPv4/IPv6 UDP;
IPv6 jumbograms are outside this contract. No DNS or decoding occurs.

Only one receive may be pending per socket. Overlap throws
`NetworkFailure(reading, resource_exhausted, ...)` without
disturbing the first receive. Cancellation removes readiness and releases the
receive slot before another call can start; it never consumes a later packet.
There is no promise to undo a receive that already completed before cancellation.
Closing the socket wakes pending receives with `NetworkClosed`; subsequent
receives also throw `NetworkClosed`. Invalid limits take precedence over closure.
Other terminal native failures are translated through `NetworkError(reading, ...)`.

The socket owns the descriptor. A returned Datagram contains only values and
remains usable after socket closure. Deadlines compose externally through `Within`.
[Sending](udp-send.md) may wait independently on the same socket.
Connected UDP remains separate work.

## Example

[udp-receive.ok](../../examples/std-v4/udp-receive.ok) prints its assigned loopback
port and waits at most five seconds. Run from the repository root:

```sh
OKKAMA_STDLIB_DIR=std perl okkama-pl/bin/okkama examples/std-v4/udp-receive.ok
```

Send a packet from another terminal, replacing `PORT` with the printed port:

```sh
python3 -c 'import socket; s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM); s.sendto(b"hello",("127.0.0.1",PORT)); s.close()'
```

The example then prints `127.0.0.1`, `4`, and `truncated`. Packet observations
are deliberately performed after socket closure.
