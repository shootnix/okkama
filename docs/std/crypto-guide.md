# Cryptography: std objects over the host provider

`crypto/*` and the modern `digest/*` modules are Okkama objects. Every
primitive runs in the reviewed host provider: `okkama-native`, a Rust library
over RustCrypto with a C ABI. Std composes primitives and never reimplements
them. Programs see objects, roles and typed failures, never the host handle.

```okkama
let key = SecretBytes(SystemEntropy().bytes(32));
let cipher = ChaCha20Poly1305(key);
let nonce = RandomNonce(SystemEntropy(), 12).bytes();
let sealed = cipher.sealed(nonce, aad, plaintext);
let opened = cipher.opened(nonce, aad, sealed); // AuthenticationFailed if anything changed
```

## Principles

- **Secrets are capabilities.** Keys, seeds, passwords and derived secrets are
  `SecretBytes` (see [secret-bytes.md](secret-bytes.md)). They are exported to
  the host only for the operation that needs them, and outputs that are
  secrets come back as fresh `SecretBytes`.
- **Randomness is explicit.** Every key generation, salt, nonce or randomized
  signature takes an `Entropy`. `SystemEntropy()` is the host CSPRNG
  (`getrandom`), while `Entropy.fake()` is a deterministic source for tests
  and never secure. Nothing reads ambient randomness.
- **Algorithms are explicit.** There is no default hash or "best available"
  cipher: the object or enum names the algorithm (`HmacHash.sha256`,
  `RsaPssSigning(key, RsaHash.sha384, entropy)`).
- **Typed failures** (`crypto/error`):
  - `InvalidCryptoInput`: wrong key, nonce or parameter size, or bad
    encoding.
  - `AuthenticationFailed`: AEAD tag mismatch, RSA-OAEP padding, or a
    non-contributory X25519 key.
  - `CryptoUnavailable`: the host library is missing or failed.

  Messages never contain key or plaintext bytes. Signature checks answer
  `false` instead of throwing.
- **Constant time where it matters.** Compare tags and tokens with
  `ConstantTimeEqual(expected, supplied).value()` or `Hmac(...).matches(tag)`,
  never with `==`.

## Modules

| module | contents |
| --- | --- |
| `digest/sha2` | `Sha224`, `Sha256`, `Sha384`, `Sha512`, `Sha512_256` |
| `digest/sha3` | `Sha3_224`, `Sha3_256`, `Sha3_384`, `Sha3_512` |
| `digest/shake` | `Shake128(length)`, `Shake256(length)`: exact output length 1..1048576 |
| `digest/blake2` | `Blake2b(length)`, `Blake2s(length)`, keyed `Blake2bMac`, `Blake2sMac` |
| `digest/read` | `ReadDigest(Reading, digest)`, `AsyncReadDigest(AsyncReading, digest)` |
| `crypto/constant_time` | `ConstantTimeEqual` |
| `crypto/nonce` | `Entropy`, `SystemEntropy`, `RandomNonce`, `CounterNonce` |
| `crypto/hmac` | `Hmac(HmacHash, key[, message])`: SHA-2 and SHA-3 family |
| `crypto/kdf` | `HkdfSecret` (RFC 5869), `Pbkdf2Secret` (RFC 8018) |
| `crypto/aead` | `Sealing` role: `sealed`/`opened`, ciphertext ‖ tag |
| `crypto/aes_gcm` | `AesGcm(key)`: 128/256-bit key, 96-bit nonce |
| `crypto/chacha20_poly1305` | `ChaCha20Poly1305` (96-bit nonce), `XChaCha20Poly1305` (192-bit, random-nonce safe) |
| `crypto/key` | roles `PublicKey` (SPKI DER), `VerifyingKey`, `SigningKey` |
| `crypto/ed25519` | `Ed25519PrivateKey(seed:)` / `(entropy:)`, `Ed25519PublicKey` |
| `crypto/x25519` | `X25519PrivateKey(secret:)` / `(entropy:)`, `shared_secret(peer)` |
| `crypto/ecdsa` | `EcdsaPrivateKey(curve:, pkcs8:)` / `(curve:, entropy:)`, P-256/SHA-256 and P-384/SHA-384, RFC 6979 |
| `crypto/rsa` | 2048/3072/4096-bit keys, `RsaPssSigning`, `RsaPkcs1Signing`, `RsaOaepEncryption`/`Decryption` |
| `crypto/key_format` | PKCS#8 / SPKI PEM: `PrivateKeyPem`, `PublicKeyPem`, `ParsedPrivateKey`, `ParsedPublicKey` |
| `crypto/password` | Argon2id in PHC format: `PasswordHash`, `PasswordCheck`, `Argon2Policy` |

The digests follow the [digest/state](digest-guide.md#digeststate-contract)
contract. Each object is the digest of an ordered byte history, and
`state(bytes)` returns a successor over the appended history. The receiver and
other branches are unchanged, because the host state is cloned per branch.
`bytes()` is raw output; format it with `HexEncoded`. `digest/legacy` (MD5,
SHA-1) remains pure Okkama for compatibility uses only.

Constructors whose overloads share an arity are selected by argument name
(`Ed25519PrivateKey(seed: s)` versus `Ed25519PrivateKey(entropy: e)`).

## Usage notes

- **Nonces.** A nonce must never repeat under one AEAD key. Use
  `CounterNonce(prefix, counter, size)` for message counters, or
  `XChaCha20Poly1305` with `RandomNonce(entropy, 24)`.
- **Key agreement.** `X25519PrivateKey.shared_secret` is raw. Feed it to
  `HkdfSecret` and never use it directly as a key.
- **Passwords.** `Argon2Policy()` is the OWASP baseline (19 MiB, 2
  iterations, 1 lane). `PasswordHash` draws a fresh salt per string, and
  `PasswordCheck.outdated(policy)` tells when to rehash. PBKDF2 is offered
  for protocols that require it (SCRAM), not for stored passwords.
- **RSA** needs an explicit scheme. `key_format` therefore does not hand out
  RSA signing keys; build `RsaPssSigning`/`RsaPkcs1Signing` from
  `RsaPrivateKey(pkcs8:)`. Failed OAEP decryption does not reveal padding
  details.
- **PEM** is strict: one block with the expected label (`PRIVATE KEY`,
  `PUBLIC KEY`). Encrypted PKCS#8, PKCS#1 `RSA PRIVATE KEY` and SEC1
  `EC PRIVATE KEY` are rejected.
- **Certificates.** `crypto/x509` uses the same `PublicKey` role for SPKI.

## Host library and deployment

`okkama build` and `okkama run` embed the library for the build platform in
the artifact at `okkama/native/<os>-<arch>/libokkama_native.{dylib,so}`. At
first use it is extracted to the temporary directory under its content hash.
`OKKAMA_NATIVE_DIR` adds builds for other platforms to a JAR, and
`OKKAMA_NATIVE_LIB` points a run at a specific library. Without a library,
operations throw `CryptoUnavailable`. The JVM backend calls the library
through FFM, which requires JDK 22 or newer.

Verification:
- RFC/NIST known-answer vectors in `okkama-native/tests` and in the std inline
  tests;
- OpenSSL interoperability for Ed25519, ECDSA, RSA PKCS#1 v1.5, PSS and OAEP,
  and PEM;
- the same std tests on macOS and in a Linux container.

`db/postgres` builds its SCRAM-SHA-256 client on `Hmac`, `Pbkdf2Secret`,
`Sha256` and `ConstantTimeEqual` ([db-postgres.md](db-postgres.md)).
