# Explicit HTTP redirects

`Redirecting(origin is Exchanging,policy is RedirectPolicy)` implements Exchanging. Defaults: ten follows, HTTPS downgrade denied. Full RedirectPolicy(maximum,https_downgrade) accepts 0..20; zero rejects the first eligible redirect. It is never enabled by HttpClient itself.

Only 301/302/303/307/308 with a single Location are followed. A missing Location returns the response. Relative references resolve against the effective response request URL; fragments are removed before network target construction. Non-HTTP URLs, userinfo, duplicate/invalid Location and forbidden downgrade throw UnsafeRedirect. Rules follow [RFC 9110 section 15.4](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.4): 301/302 rewrite POST to GET; 303 uses GET except HEAD stays HEAD; 307/308 preserve method. Body-preserving follows require replayable(); body-rewriting follows substitute EmptyBody and remove payload framing/type/encoding/Expect fields. No speculative body opening is used to decide replayability.

Loop coordinates include method plus canonical effective scheme/host/port/path/query. Default ports, dot segments, empty path and fragments are normalized. Method is included so a valid same-URI POST-to-GET follow is allowed; repetition of the same request coordinates throws RedirectLoop. Budgets bound all history to twenty hops.

Stale Host is always removed and the consuming client derives a fresh one. Authorization, Cookie and Proxy-Authorization are stripped on origin change; same-origin explicit credentials remain. Each intermediate response is retained as ExchangeHop history; its body is explicitly closed before the next exchange. History is metadata with disposed intermediate body capabilities, not buffered payload snapshots. Nested origin history is preserved in order. Final response ownership escapes intact.

HttpClient now checks endpoint policy with the new URL and the existing actual remote endpoint before a pooled connection emits bytes. Fresh connections retain the existing per-candidate checks and no second lookup. Redirect tests verify both reusable empty and abandoned nonempty responses; denied follows create no additional TCP attempt.

Typed errors: TooManyRedirects, RedirectLoop, NonReplayableRedirect and UnsafeRedirect(reason). Cancellation follows the wrapped exchange/body lifetime. The wrapper does not own or close the supplied client itself.

Focused value tests: 465 (15 assertions); actual denial lifecycle: 466 (3); actual POST-to-GET follow on one socket: 467 (2). Client compatibility: 462/463 (8). Inline policy and executable loopback [example](../../examples/std-v4/http-redirect.ok) PASS. No interpreter/provider edits, no full regression run.
