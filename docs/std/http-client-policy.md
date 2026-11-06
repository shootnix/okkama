# Explicit HTTP client policy

`http/client_policy` contains reusable values with no network effects.

- `HttpLimits()` defaults to 65536 head bytes, 8192 line bytes and 1073741824 body bytes. The full constructor takes `(header_bytes is int, line_bytes is int, body_bytes is BigInt)`. Headers are bounded to 4..1048576, lines to 1..header_bytes, bodies to 1..9007199254740991. `http1()` adapts exact decimal counts to Http1Limits with 128 fields; no rounding.
- `ClientTimeouts()` gives each independent stage 30 seconds. The full constructor takes three Duration values: connection, headers and body_idle. Negative values reject; zero means disabled and consumers must bypass Within for zero.
- `ClientPolicy()` enables HTTP/1.1, eight connections, these budgets and PublicEndpoints. The full constructor takes `(Set<HttpVersion>, HttpLimits, ClientTimeouts, connections is int, EndpointAllowing)`. It snapshots the three nominal protocol identities; connection count is 1..1024.
- `Http1ClientProtocols(ClientPolicy,TlsClientPolicy)` accepts only the currently implemented HTTP/1.1 consumer and empty or exact `http/1.1` ALPN advertisement. H2-only and mixed policies reject rather than downgrade. Actual negotiated ALPN remains checked by Http1Connecting.
- `InvalidClientPolicy` exposes fixed reason/text.

[HttpClient](http-client.md) now consumes these inputs to enforce deadlines and owned pooling. Seven focused assertions and one inline test pass. No interpreter/provider change; no full regression run for this slice.
