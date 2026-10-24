# Explicit HTTP proxies

`http/proxy_policy` exports ProxyPolicy(Url[,BasicCredentials]), url(),
authenticated() and credentials(); absent credentials raise MissingProxyCredentials.
The endpoint must be plain http without URI credentials, path beyond /, query or
fragment. TLS to the proxy itself and SOCKS require separate transport contracts.

`http/proxy` exports Proxied(proxy,resolver,tls,policy,clock), implementing
Exchanging and Closing: async response(Request) returns ClientResponse;
closed()/close() manage the entire client pool. It uses normal client deadlines,
physical admission, H1 body leases, H2 multiplexing and cancellation settlement.
The fixed proxy/TLS policy is part of each client instance; its pool keys retain
origin scheme/host/port. Caller Request snapshots remain unchanged.

HTTP targets use absolute-form H1 requests. HTTPS targets first send CONNECT
with authority-form host:port, then verify the origin certificate and SNI using
the target URL. Actual ALPN selects enabled H1/H2 inside the tunnel. Plain HTTP
forwarding requires enabled H1. [Absolute-form](https://www.rfc-editor.org/rfc/rfc9112.html#section-3.2.2)
and [CONNECT](https://www.rfc-editor.org/rfc/rfc9110.html#section-9.3.6)
follow their distinct wire contracts.

Explicit BasicCredentials authorize serialization to the configured proxy hop.
Proxy-Authorization never enters the origin TLS exchange; origin Authorization
never enters CONNECT. Caller Proxy-Authorization raises ProxyAuthorizationConflict
before DNS or I/O. ProxyConnectRejected.status() retains a rejected CONNECT status
without remote diagnostic text. No implicit credential discovery or 407 loop occurs.

ClientPolicy.access() checks the actual proxy endpoint before opening and on
reuse. Only proxy DNS is resolved locally: the remotely resolving proxy owns
origin-address filtering after forwarding/CONNECT. ExchangeDetails report the
actual proxy TCP peer and the selected origin protocol/TLS facts. Target TLS
identity remains the origin URL, never the proxy name.

CONNECT reading is bounded and accepts at most sixteen informational responses.
Any 2xx, including 204, transitions immediately after headers, ignoring received
Content-Length/Transfer-Encoding. This means no HTTP response body and a tunnel,
not an ordinarily reusable HTTP session. Reads do not consume the first tunnel
byte. Rejection, timeout or cancellation closes/drains the actual transport.
Connection timing includes proxy selection, TCP, CONNECT, target TLS and preface.

`http/environment_proxy` exports EnvironmentProxy(Environment).proxy(Url),
throwing MissingProxy when no proxy is selected. The consumer reads only the
explicit supplied environment snapshot. Lowercase names take precedence;
http_proxy ignores uppercase HTTP_PROXY, https_proxy permits HTTPS_PROXY,
all_proxy permits ALL_PROXY. An explicitly empty selected variable disables it.
This case distinction assumes a case-sensitive environment (the tested POSIX
profile); Environment normalizes names on Windows. URI credentials are rejected.

no_proxy/NO_PROXY supports *, apex/domain suffixes with label boundaries,
optional ports, exact IP and CIDR for literal URL addresses without hidden DNS.
IDNA Unicode names and ASCII edge OWS are supported. Rules are bounded to 8192
UTF-8 bytes, 256 entries and 1024 characters per item. InvalidProxy exposes fixed
configuration reasons; malformed controls and CIDR prefixes are typed failures.

Examples: [HTTP](../../examples/std-v4/http-proxy.ok),
[HTTPS](../../examples/std-v4/http-proxy-https.ok),
[H2 over CONNECT](../../examples/std-v4/http-proxy-h2.ok).
Evidence: final distinct coverage
37 files /339 assertions PASS, plus inline tests. Interpreter unchanged; no full
regression rerun. PERF-26
records bounded completed-Future retention during exact CONNECT reads.
