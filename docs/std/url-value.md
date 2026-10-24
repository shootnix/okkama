# Full URL values and reference resolution

`url/value` now provides immutable URL and authority values in addition to the
existing query/fragment objects. Parsing/printing and RFC 3986 resolution are
ordinary stdlib code except the measured dot-segment bulk operation; registered names use the verified
[HostName/IDNA boundary](host-policy.md). No DNS, filesystem, connecting or ambient
scheme-default policy is involved.

`UrlParts(scheme,authority,path,query,fragment)` validates the represented
coordinates. `scheme()`/`path()` are canonical strings; `authority()` is
UrlAuthority; `query()`/`fragment()` retain the existing explicit presence types.
`string()` recomposes them. Scheme is ASCII lowercase. Existing unreserved
escapes are decoded, reserved escapes remain uppercase escapes, and allowed IRI
characters become escaped UTF-8. No query sorting or repeated decoding occurs.
Path/query/fragment retain their distinct grammar. Generic parsing preserves
dot segments; explicit reference resolution removes them.

`Url(parts)` requires a scheme and observes `parts()`/`string()`;
`url(reference is UrlReference)` resolves a reference and returns Url.
`UrlReference(parts)` permits a missing scheme; `absolute()` distinguishes it.
`ParsedUrl(origin).value()` and `ParsedUrlReference(origin).value()` interpret
their retained source when called. Values outlive parsing scope and repeat
stably. Hierarchical, relative and opaque forms are supported; `//` alone
introduces authority. This is RFC URI/IRI syntax with a strict DNS-name profile,
not browser WHATWG URL parsing or scheme-specific application validation.

`UrlHost` is a role with `kind()`, `name()`, `address()` and `string()`.
UrlHostKind is empty/name/address/future. NamedUrlHost(HostName) and
AddressUrlHost(IpAddress) preserve their typed identity; IPv6 prints bracketed.
FutureUrlHost(raw unbracketed text) validates IPvFuture and prints lowercase
bracketed spelling. EmptyUrlHost represents the legal generic empty host,
including file:/// paths. Wrong projections throw UrlHostIsNotName or
UrlHostIsNotAddress. Scoped IPv6 literal syntax is currently rejected; scope
is not silently removed. Custom role implementations must supply consistent
canonical coordinates: UrlHostSnapshot enforces wire agreement before retaining
a fresh validated host during authority composition.

`UrlAuthority()` is absent; `(host)` or `(host,port is int)` is present.
The full constructor is `(host is UrlHost,userinfo is UrlUserInfo,port is UrlPort)`.
`present()`, `host()`, `userinfo()`, `port()`, `port_marker()` and `string()`
observe it; string omits the introducing //.
`ParsedUrlAuthority(origin).value()` explicitly interprets present authority text.
Bracketed IPv6/IPvFuture is not confused with port colons. Numeric addresses use
IpAddress canonical text; names use HostName after one strict UTF-8 host percent
decoding. Generic authority does not invent a port or validate a transport scheme.

`UrlUserInfo()` is absent; `(source)` is present, including explicit empty.
`present()`/`string()` retain that distinction and canonical escaped text.
`UrlPort()` is absent; `(decimal text)` represents delimiter presence, including
an empty port. Numeric text is ASCII, 0..65535 and canonicalized without leading
zeroes. `present()`/`string()` distinguish omitted/empty/numeric; `value()` throws
MissingUrlPort when no integer is assigned. Thus host, host: and host:0 differ.
MissingUrlAuthority covers observations requiring an absent authority.

Limits: 8192 input scalars for a parsed reference/authority, 8192 canonical ASCII
characters for composed URL output/path, 1024 path segments during composition
and dot traversal. Host names have the stricter IDNA/DNS limits above.
InvalidUrl exposes `offset()`, `reason()` and `text()`; full parsing translates
component syntax errors into original source scalar offsets. Composed values
report their own coordinate offsets. Budget failures are typed and do not weaken
earlier component validation. The standalone preexisting UrlQuery/UrlFragment
API remains unchanged.

UrlResolution follows RFC 3986 section 5.2, including strict scheme interpretation,
authority inheritance, empty-path query inheritance, non-inherited fragments,
base-directory merging, root clamping and repeated/trailing slash preservation.
UrlDotSegments delegates to a private bounded native text operation after
profiling identified repeated ownership walks over the initial persistent stack.
An independent input-buffer RFC oracle covers 442 paths, plus exact 1024-segment
and failure bounds. No native URL value/parser/host policy is introduced.
UrlAsciiText, UrlSchemeText, UrlHostSnapshot and UrlResolution are importable infrastructure because module
privacy is unavailable; applications use the represented values above.

Sources: [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986.html#section-5.2),
[RFC 3987](https://www.rfc-editor.org/rfc/rfc3987.html#section-2.2).
All 42 normal/abnormal section 5.4 cases are executable tests.
Evidence,
[example](../../examples/std-v4/url-resolution.ok).
