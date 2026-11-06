# Explicit cookie exchange adapter

`CookieContextualizing.context(Request) is CookieContext` supplies the site/top-level/method decision. `FirstPartyCookies()` deliberately treats each request as top-level first-party, using that absolute URL and current method; it does not infer a browser document/frame tree.

`CookieExchanging(Exchanging,CookieStoring,CookieContextualizing[,CallerCookiePolicy])` implements Exchanging. Construction and an unstarted cancelled response Future make no jar/network calls. On a started exchange, capture one caller context, select matching cookies immediately before delegation, and send their pair() values in jar order. Request preparation retains its body without opening/measuring it. An empty selection adds no Cookie header. Source count is 0..256; pair serialization charges two separator bytes per item against 65536. Overflow stops source observation immediately, before querying another cookie.

Default conflict policy is reject: any explicit caller Cookie requires an explicit policy. Preserve retains exactly one caller field and bypasses jar selection; replace removes it and sends only selected jar cookies. Every policy rejects duplicate caller Cookie fields; there is no implicit merge. CallerCookieConflict and CookieExchangeLimit expose fixed diagnostics without cookie values.

Store every Set-Cookie field against the effective response URL. A direct hop uses the same captured caller context for selection and storage. The supplied jar remains caller-owned; MemoryCookieJar updates are synchronous immutable-state publications with no suspension between reading and publication, so concurrent HTTP tasks serialize jar actions without a separate adapter lock. A custom CookieStoring implementation must uphold its synchronous contract.

For per-hop selection, composition order is explicit:

```scss
let exchanging = Redirecting(
    CookieExchanging(client, jar, FirstPartyCookies()),
    RedirectPolicy(),
);
```

The inner cookie adapter stores intermediate cookies before the outer redirect chooses its next request. Generated jar headers remain internal to the hop: a direct ClientResponse reports the original caller Request, so the next hop can select fresh jar cookies without mistaking generated Cookie for a caller override. Explicit caller fields follow their chosen policy; cross-origin redirects strip them. Putting CookieExchanging outside Redirecting stores intermediate/final history afterward, but cannot select cookies during hidden inner hops; that order is unsuitable when per-hop selection is required.

Final body ownership escapes unchanged after successful storage. Storage/context failure after acquisition disposes the final response body before propagating the original error. The adapter never closes the supplied jar or exchange owner itself.

Focused tests: 468 eight assertions; 469 seven; 470 two actual-wire assertions. Tests cover path ordering, multiple Set-Cookie fields, conflict rules, fixed context, redirect freshness, cancellation, bounded source access and acquired-body cleanup. Inline and executable [loopback example](../../examples/std-v4/http-cookie-client.ok) PASS. Interpreter/provider unchanged; no full regression run.
