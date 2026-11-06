# Immutable checksums

## `checksum/state` contract

`Checksumming` represents a checksum over an ordered byte history. `value() is int`
returns the non-negative algorithm result. `state(origin is @byte) is ^Checksumming`
returns a fresh state with the supplied chunk appended, preserving the receiver
and the immutable byte array. Branching from an earlier state is valid; repeated
observations are deterministic. Empty chunks preserve the mathematical result.

The v3 candidate's plain return is refined to a successor return for Okkama's
explicit immutable-transition contract (Book chapters 19 and 22). Concrete
implementations return their own successor type. The role alone cannot enforce
the mathematics of arbitrary third-party implementations; each algorithm has
its own conformance tests. Its test-only fake counts bytes, starting at zero,
and is deliberately not an Adler or CRC implementation.

`InvalidChecksumInitial(initial is int, maximum is int)` does `Error` and exposes
`initial()`, `maximum()` and `text()`. Algorithm constructors reject an initial
value outside `0..maximum`, without masking or silently wrapping it.

No algorithm selection enum, native checksum provider, I/O, owned resource or
close action is involved. These checksums detect accidental corruption and are
not cryptographic digests. Text must first be encoded explicitly, for example
with `Utf8Encoded(text).bytes()`.

## Execution model for the concrete algorithms

Constructors bind an immutable byte array and initial integer, checking the
integer locally. `value()` computes lazily without mutation or memoization.
`state(chunk)` computes the receiver's value and binds that as the new initial
value together with `chunk`; the new chunk is evaluated on observation. Thus
successive states do not retain a linked chain of prior byte chunks. Work is
linear in the bytes examined; repeated `value()` recomputes the retained chunk.
No external source is read, consumed or closed.

The empty constructor (where provided) binds an empty array and the algorithm's
default initial value. Implementations and tests follow in roadmap order.

## `checksum/adler32` contract

`Adler32` implements `Checksumming`. Constructors are `Adler32()`,
`Adler32(origin is @byte)` and `Adler32(origin is @byte, initial is int)`.
The first two use seed 1; explicit seeds must be in `0..4294967295`.
`state(origin is @byte) is ^Adler32` follows the shared execution model above.

The seed packs low and high 16-bit sums. Both are reduced modulo 65521,
including on empty input. For each unsigned byte, the low sum adds that byte
and the high sum adds the new low sum, each modulo 65521. The final integer is
`(high << 16) | low`. The default empty result is 1; `Wikipedia` is 300286872.
This implements the algorithm in [RFC 1950, section 8](https://www.rfc-editor.org/rfc/rfc1950.html#section-8).
Continuation and arbitrary unsigned seed behavior match
[zlib's checksum API](https://zlib.net/manual.html#Checksum).

```okkama
let prefix = Adler32(Utf8Encoded("Wiki").bytes());
let complete = prefix.state(Utf8Encoded("pedia").bytes());
complete.value(); // 300286872; prefix is still the checksum of "Wiki"
```

The implementation uses scalar arithmetic with per-byte reduction, so no
large-input intermediate can overflow a signed 64-bit integer. It owns no
buffer and imports no checksum provider. Fixtures include independent Python
stdlib zlib outputs, all 256 byte values, seed boundaries and modular wraparound.

## `checksum/crc` contract

`Crc32` and `Crc16Ccitt` implement `Checksumming` with concrete successors.
They use the shared lazy observation and chunk-transition model, borrowing
immutable arrays without I/O or resource ownership.

| Object | Constructors | Parameters and result |
| --- | --- | --- |
| `Crc32` | `Crc32()`, `Crc32(origin is @byte)`, `Crc32(origin is @byte, initial is int)` | CRC-32/ISO-HDLC, width 32, polynomial `0x04C11DB7`, reflected input/output, xorout `0xFFFFFFFF`. Public initial defaults to 0 and denotes a previously finalized result; internal register starts at `initial ^ 0xFFFFFFFF`. Valid seeds/results: `0..4294967295`. |
| `Crc16Ccitt` | `Crc16Ccitt(initial is int)`, `Crc16Ccitt(origin is @byte, initial is int)` | Width 16, polynomial `0x1021`, MSB first, no reflection, no final XOR. Initial value is mandatory and directly initializes the register; seeds/results: `0..65535`. |

Both return the supplied initial value for empty input and reject invalid seeds
at construction with `InvalidChecksumInitial`. The public CRC-32 continuation
value follows the [RFC 1952 sample](https://www.rfc-editor.org/rfc/rfc1952.html#section-8)
and zlib's API, including pre/post conditioning. `Crc16Ccitt` matches
[Python binascii.crc_hqx](https://docs.python.org/3/library/binascii.html#binascii.crc_hqx).
The explicit parameters distinguish this CRC-16 from reflected/KERMIT/X-25 variants.

For `123456789`, CRC-32 is `0xCBF43926` (3421780262); CRC-16 is `0x29B1`
(10673) with initial `0xFFFF`, or `0x31C3` (12739) with initial 0.
The implementation processes eight bounded bit steps per byte, without a native
provider, hidden mutable table or algorithm enum. No ambiguous `Crc64` is added.

```okkama
let first = Crc32(Utf8Encoded("1234").bytes());
let whole = first.state(Utf8Encoded("56789").bytes());
whole.value(); // 3421780262
Crc16Ccitt(Utf8Encoded("123456789").bytes(), 65535).value(); // 10673
```
