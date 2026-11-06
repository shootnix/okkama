# Optional stale HTTP1 retry

`Retrying(origin is Exchanging)` implements Exchanging. `async response(Request)`
tries once and allows exactly one replacement exchange only after
`HttpStaleConnection`, with `MethodSemantics.idempotent()` and
`request.body().replayable()` both true. No method conversion, status-code retry,
body buffering or third attempt occurs. This decorator is explicit; HttpClient
never enables replay by default.

The client raises HttpStaleConnection only for a reused HTTP1 conversation that
reached EOF, NetworkClosed, reset or aborted before any response octet. Its
`origin() is Error` preserves the underlying typed error; `text()` is fixed and
contains no request payload. The same signal applies to H1 selected by a mixed
policy. A fresh connection keeps its original error. One partial byte or any 1xx
head forbids classification, even if a final response head never arrives.
Timeouts, cancellation, protocol errors, TLS verification failures and broad TLS
invalid_data failures do not qualify. A valid TLS close_notify produces ordinary
EOF and is covered on a real verified HTTPS peer. Ambiguous unclean TLS failures
remain conservative until the provider has an unambiguous portable cause.

`http/transport_observation` wraps only the exclusive H1 physical read/write
halves. Observation is reset before starting each admitted exchange; direct
Http1Connecting.session()/acquisition() users retain original errors until
explicit begin(). Actual H2 conversations do not use a shared byte observation:
sibling stream bytes cannot establish retry safety for an individual stream.
H2 GOAWAY/unprocessed replay remains a distinct unimplemented policy.

Before the decorator retries, the failed client operation cancels/drains its
children, retires the old transport and releases admission. The new exchange
runs normal endpoint checks, DNS/connection/TLS verification and deadlines.
Replayable idempotent requests can have been partly emitted; the predicate
addresses repeated application, not proof of zero outbound bytes.

Wrap decorators in the order required by the application. In particular, a
stateful Digest authorization must produce a fresh nonce-count for a new attempt;
placing retry inside such an authorization can replay its generated header.

[Communication-failure semantics, RFC 9110 9.2.2](https://www.rfc-editor.org/rfc/rfc9110.html#section-9.2.2).
Evidence. Focused compatibility:
33 files /295 assertions PASS; real TCP and TLS CLI examples are part of the gate.
