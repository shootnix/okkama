# HTTP method values

`http/method::Method(origin is string)` denotes one validated immutable HTTP
method token. Construction rejects empty strings, non-ASCII characters and all
characters outside HTTP `tchar`, using `InvalidMethod`. `string()` preserves
spelling exactly. There is no network, resource, cancellation or lifetime effect;
repeated observations are identical. Validation is linear in input bytes; a
future wire framer must separately enforce its request-line budget.

`MethodSemantics(origin is Method)` observes known standard semantics:
`safe()` recognizes GET, HEAD, OPTIONS and TRACE; `idempotent()` additionally
recognizes PUT and DELETE. Unrecognized extensions conservatively return false
for both observations. These are protocol properties, not permission to retry
an exchange automatically. They do not forbid content on GET or DELETE.

The old completion candidate requested uppercasing. That would change extension
identities: HTTP method names are **case-sensitive**. Thus `get` is a valid token,
but is not the standard GET method. Names remain an open token space, not an enum.
This corrects the candidate contract rather than changing Okkama semantics.

Basis: [RFC 9110 §5.6.2](https://www.rfc-editor.org/rfc/rfc9110.html#section-5.6.2)
and [§9](https://www.rfc-editor.org/rfc/rfc9110.html#section-9).

Tests enumerate all 128 ASCII octets, standard and extension semantics, case
preservation, injection/Unicode/empty rejection and a long valid token. An inline
test and executable example accompany the module. TLS/URL are not dependencies
of this pure value; connecting and wire parsing remain separate sections.
