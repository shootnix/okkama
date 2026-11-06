# HTTP content coding names

ContentCoding(origin is string) denotes a concrete coding name or the explicit
`identity` no-encoding choice. It validates HTTP token grammar and stores ASCII
lowercase, so structural equality agrees with the case-insensitive identity.
Unknown extensions remain valid; a name does not prove an installed codec.
The exact `*` is rejected because it is a negotiation wildcard, not a concrete
transformation. InvalidContentCoding(origin) is a typed construction error.
string() is the canonical representation; construction/observation are pure,
linear in input bytes, with no resource or cancellation behavior.

Basis: [RFC 9110 8.4.1](https://www.rfc-editor.org/rfc/rfc9110.html#section-8.4.1)
and [12.5.3](https://www.rfc-editor.org/rfc/rfc9110.html#section-12.5.3).

ContentEncodedBody/ContentDecodedBody remain BLOCKED by the explicitly deferred
Stage 16 gzip/deflate/zstd implementations. Existing compression roles and fakes
are not algorithms. No identity-only decorator is presented as a functioning
coding pipeline, no unavailable codec is advertised, and no implicit decoding
or Content-Length rewriting occurs. When codecs return, stacked ordering,
size/expansion budgets, streaming cancellation and ownership need actual tests.
