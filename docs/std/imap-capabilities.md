# IMAP capability advertisements

[`mail/imap`](../../std/mail/imap.ok) provides eager immutable observations of
CAPABILITY data. It opens no connections and implements no commands or session
state. A snapshot is not evidence of TLS, authentication or an enabled extension.

## Values and observations

`ImapCapability(origin[,limits])` validates one nonempty ASCII IMAP atom.
string() preserves spelling; name() returns its ASCII-uppercase form. Atom
syntax follows [RFC 3501 §9](https://www.rfc-editor.org/rfc/rfc3501.html#section-9):
controls, space, parentheses, opening brace, wildcard characters, quote,
backslash and closing bracket are excluded. Opening bracket and closing brace
are allowed. Unknown names and parameters embedded in an atom remain opaque.
Registration with IANA is not checked.

`AUTH=` has additional validation: the prefix is case-insensitive, while its
mechanism uses the uppercase 1..20-character SASL name grammar (letters, digits,
hyphen and underscore) from [RFC 4422 §3.1](https://www.rfc-editor.org/rfc/rfc4422.html#section-3.1).
Malformed AUTH advertisements fail rather than becoming unknown tokens.

`ImapCapabilities(entries[,limits])` accepts `Sequencing<string>`. It observes
size() once, checks the count bound, and observes each item once. It requires
at least one `IMAP4rev1` or `IMAP4rev2` token in any position. Both may be present.
An unknown future revision is retained when a supported revision is also present,
but does not by itself satisfy the revision requirement.

- tokens() retains all ImapCapability values in source order, including duplicates.
- names() is a Set<string> of uppercase names, with distinct first-occurrence order.
- features() is a Set<ImapFeature> for explicitly advertised recognized tokens.
- authentication() retains distinct SASL mechanisms in first-occurrence order.
- bytes() emits one untagged CRLF CAPABILITY line, preserving token spelling/order
  while writing the response keyword in uppercase. The object does ByteSource.

The recognized feature enum contains imap4rev1, imap4rev2, start_tls,
login_disabled, idle, uid_plus, move, enable, utf8_accept, utf8_only, literal_plus,
literal_minus, sasl_ir, condstore, qresync, namespace, unselect and binary.
Unknown capabilities remain in tokens()/names() without acquiring semantics.
features() does not synthesize features implicit in a protocol revision or expand
relationships between advertised extensions; it reports explicit tokens only.

## One-line parser

`ParsedImapCapabilities(origin is @byte[,limits]).capabilities()` eagerly
parses exactly one `* CAPABILITY ...` line ending in CRLF. Keyword matching is
ASCII-insensitive. Separators are exactly one ASCII space. Tabs, extra spaces,
empty tokens, embedded controls, high bytes, missing CRLF and trailing bytes fail.

This parser does not search arbitrary text for capabilities. It does not accept
tagged OK completions, bracketed CAPABILITY response codes in greetings, ENABLED
responses, literals or multiple responses. A future response parser must extract
the appropriate tokens before constructing a snapshot. Parsing this untagged
line does not establish that the corresponding command completed successfully.

## Limits and lifecycle

`ImapCapabilityLimits()` defaults to 256 source tokens, 256 bytes per token and
65536 bytes for the complete response line. The three-argument constructor takes
nonnegative `(tokens,token_bytes,wire_bytes)`; negatives throw InvalidArgument.
Duplicate tokens count against every input bound. The complete line budget
includes the prefix, spaces and CRLF. A standalone ImapCapability applies only
token_bytes; the sequence/parser applies all three limits.

Token length is checked before byte conversion. The parser checks wire size
before decoding or splitting. Source count is checked before item observations;
negative source size throws InvalidArgument. `ImapCapabilityLimitExceeded(kind)`
exposes kind()/text(): tokens, token_bytes or wire_bytes. Grammar/revision failures
throw `InvalidImapCapabilities(reason)`, with reason()/text(). Empty capability
lists are invalid, rather than silently meaning an uninitialized connection.

All projections use owned snapshots. Scratch buffers are closed; no external
resource is acquired or retained. No native runtime changes are required.

Construct a fresh snapshot when capabilities are refreshed after STARTTLS or
authentication. Do not merge it with the older advertisement. Capability changes
and the distinction between advertisement and enabled state are described by
[RFC 9051 §7.2.2](https://www.rfc-editor.org/rfc/rfc9051.html#section-7.2.2).
The future session layer remains responsible for negotiation, verified TLS,
credential policy, command completion and invalidating old snapshots.

[Example](../../examples/std-v4/imap-capabilities.ok) ·
Tests ·
Evidence.
