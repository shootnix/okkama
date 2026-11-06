# Immutable message digests

Delivery decision, 2026-09-25: remaining digest/crypto work is deferred in favor
of reviewed host implementations. The contracts and implemented legacy digests
below are retained; they do not authorize further pure-Okkama crypto work. See
[the queue policy](stdlib-completion-todo-v4.md#криптография-отложена--решение-2026-09-25).

## `digest/state` contract

`Digesting` represents a digest of an ordered byte history. `bytes() is @byte`
returns its raw, immutable result. `state(origin is @byte) is ^Digesting` appends
the supplied bytes into a fresh state, leaving the receiver, prior observations
and input array unchanged. Empty chunks preserve the result; states may branch.

The successor return refines the v3 candidate's ordinary return according to
Okkama Book chapters 19/22, as in `Checksumming`. Concrete objects return their
own successor types. An empty constructor binds empty input and computes the
empty-message digest lazily on `bytes()`, not at construction. Querying a digest
must not finalize or otherwise invalidate the original state for later append.

No implicit text encoding or hexadecimal conversion is part of this role:
use `Utf8Encoded(text).bytes()` on input and `HexEncoded(digest.bytes()).string()`
on output. The role has no I/O, close operation or algorithm-selection enum.
Each concrete algorithm must independently verify its mathematics and lifetime.

The role's test-only fake echoes the appended bytes, starting with an empty
array. This checks composition and immutable successor behavior; it is not a
hash function, nor a model of a concrete digest's output length or empty value.
The concrete empty-value/lazy-construction obligations are tested with algorithms.

```okkama
// digest is a Digesting dependency and text is already encoded into bytes.
let next_digest = digest.state([byte(0), byte(255)]);
HexEncoded(next_digest.bytes()).string();
// digest and any earlier bytes() result remain usable.
```

## `digest/legacy` contract

`Md5()` / `Md5(origin is @byte)` and `Sha1()` / `Sha1(origin is @byte)` implement
`Digesting`. They bind input without compression; `bytes()` computes 16 and 20
raw digest bytes respectively. `state(origin)` returns the concrete successor.
MD5 follows [RFC 1321](https://www.rfc-editor.org/rfc/rfc1321.html); SHA-1 follows
[RFC 3174](https://www.rfc-editor.org/rfc/rfc3174.html). Only whole-byte messages
are accepted. Both are **legacy interoperability only**, unsuitable for new
security decisions; see [MD5 considerations](https://www.rfc-editor.org/rfc/rfc6151.html)
and [SHA-1 considerations](https://www.rfc-editor.org/rfc/rfc6194.html).

Append compresses the receiver's complete 64-byte blocks, keeps its unpadded
chaining state and at most 63 trailing bytes, then binds the new immutable chunk
without compressing it yet. It does not retain a chain of old input arrays.
`bytes()` compresses the currently bound data and pads a temporary state; it
never replaces the receiver's unfinished state. Repeated queries recompute the
pending chunk, but append never replays already compressed history.

Message length is limited to 2305843009213693951 bytes (`2^61 - 1`), so its bit
length fits an unsigned 64-bit trailer. Overflow is rejected by
`DigestInputTooLong` before compression. The implementation owns no external
resource and introduces no native digest provider. It uses exact, masked
32-bit arithmetic represented in Okkama's signed 64-bit integers.

## `digest/blocks` internal contract

This module is shared implementation infrastructure for 64-byte digest blocks,
not an application algorithm-selection API.

- `DigestCompression64` specifies immutable `block(origin is @byte) is ^DigestCompression64`
  and raw `bytes()`. Blocks must have exactly 64 bytes; invalid width raises
  `InvalidDigestBlock`. Concrete compression states encode their output order.
- `DigestBlocks64(compression, origin, little_endian)` binds a compression state,
  bytes, and length-trailer byte order. `state(bytes)` appends with the lazy
  model above; `bytes()` finalizes a temporary state. Compression errors propagate
  unchanged; borrowed collaborators are never closed.
- `DigestPadding64(tail, length, little_endian).bytes()` returns one or two
  64-byte blocks: tail, `0x80`, zeros, then unsigned 64-bit bit length. `tail`
  must be shorter than 64 and agree with `length % 64`; incoherent coordinates
  raise `InvalidDigestPadding`, and out-of-range length raises `DigestInputTooLong`.
  The bit-length word uses a fixed-width shift, including its high-bit case.

The compression role's test-only fake counts blocks. Module tests check padding
and orchestration independently of MD5/SHA-1; algorithm tests use external known
answers, chunk boundaries and observation-before-append behavior.

The SHA-1 empty vector is also checked against the NIST CAVS 11.0 byte-oriented
`SHA1ShortMsg.rsp`, [distributed with cryptography's test vectors](https://raw.githubusercontent.com/pyca/cryptography/main/vectors/cryptography_vectors/hashes/SHA1/SHA1ShortMsg.rsp).
Its `Len = 0` record denotes empty input despite the placeholder `Msg = 00`.
