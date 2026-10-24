# HTTP request target coordinates

`http/target` implements the four forms from [RFC 9112 section 3.2](https://www.rfc-editor.org/rfc/rfc9112.html#section-3.2)
as validated immutable coordinates, independently of transport/framing. Values
have canonical ASCII wire projections bounded to 8192 octets; raw parser sources
must be ASCII and at most 8192 octets. Constructors may accept IRI path input and
canonicalize it through the verified URL component grammar. Dot segments remain
literal here; URI reference resolution is a separate caller operation.

`RequestTargetKind=(origin,absolute,authority,asterisk)`. `RequestTarget` declares
kind(), path() string, query() UrlQuery, url() Url, authority() UrlAuthority and
string(). Origin coordinates are path/query; absolute is url; authority is
authority. Unavailable projections throw WrongRequestTargetKind(expected,actual),
observing those enum coordinates and text(). Asterisk has no other projection.
InvalidRequestTarget(reason) does Error with reason()/text(), without source data.

`OriginTarget(path is string,query is UrlQuery)` requires a nonempty absolute URI
path and retains absent/empty query independently. `(origin is UrlReference)` is
also accepted if scheme/authority/fragment are absent and path is absolute.
Origin-form can begin `//`: it is still a path, while RFC 3986 URI-reference would
interpret that prefix as authority. The withdrawn v3 reference() draft is therefore
replaced by explicit path()/query() coordinates; forcing a generic UrlReference
carrier would lose valid HTTP semantics. The double-slash overload/parser is tested.

`AbsoluteTarget(origin is Url)` requires HTTP/HTTPS, nonempty authority and no
userinfo or fragment (including empty markers). DNS/IP/IPvFuture syntax remains
validated by URL; this does not imply a connecting provider for every host kind.
`AuthorityTarget(origin is UrlAuthority)` requires nonempty host, no userinfo and
an explicit assigned port. Absent and explicitly empty ports are rejected.
`AsteriskTarget()` renders exactly '*'. No raw request/status line is exposed.

`RequestTargetMethod(method is Method,kind is RequestTargetKind).valid()` permits
authority-form only for CONNECT and requires it for CONNECT. Asterisk requires
OPTIONS. Case-sensitive extension methods retain independent semantics.
`ParsedRequestTarget(origin is string,method is Method).value() is RequestTarget`
selects the form with those rules, preserves the first query delimiter, rejects
literal fragments, whitespace/injection and wrong forms, and canonicalizes URL
coordinates. It does not establish direct/proxy routing or infer transport security.

`RequestTargetSnapshot(origin is RequestTarget).value()` reads only the declared
role, checks wire size before coordinate observations, creates a fresh validated
target and requires its wire projection to equal the source's. Nominal tags cannot
authorize arbitrary request-line injection through a hand-written string() method.
No reflection/downcast or hidden source methods are used.

Evidence,
[executable example](../../examples/std-v4/http-request-targets.ok).
