# Opaque owned secret bytes

`crypto/secret::SecretBytes(origin is @byte)` implements Closing. It stores up to 1048576 bytes in a provider-owned opaque SecretState; zero length is valid. Ordinary observations expose only size()/closed(), and explicit close() is idempotent. No bytes(), string(), text(), ByteSource or streaming conversion is provided. Ordinary equality, including self-equality, fails through the existing opaque-handle comparison rule.

`ExportedSecret(SecretBytes).bytes()` explicitly copies the payload into an ordinary immutable byte array. The export capability retains its owner across scope returns. Already exported copies remain valid after closure; subsequent size/export observations throw SecretClosed. Oversized construction throws SecretLimit before invoking the provider; the provider independently bounds actual input count before packing/minting. Native handle authority/type checks retain the existing sealed-capability model.

Source owns lifetime through SecretBytes.destroy/close. Native closure mutates its private scalar with zeros, drops it, and tombstones aliases. This is best-effort cleanup: a garbage-collected Perl runtime cannot guarantee complete zeroization of copies, temporary buffers or explicitly exported arrays. SecretBytes is a redaction/access capability, not encryption or process isolation. No new dependency is introduced.

Private `runtime/secret` bridge is registered by the native crypto family. This changes runtime provider code and requires the full regression gate. Focused native tests: 52-secret-bytes, thirteen assertions; interpreter/471, eight. Legacy native format/crypto compatibility passes. Inline export/alias-lifetime test passes.

ConstantTimeEqual and the remainder of the Stage 18 crypto API remain open; this slice introduces no ordinary secret comparator or speculative host crypto APIs. Explicitly exported ordinary bytes are caller-owned data.

Full frozen-source regression: 672 files / 12185 assertions / 1072 seconds PASS; evidence.
