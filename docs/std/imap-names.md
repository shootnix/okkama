# IMAP4rev1 mailbox names

The pure values in [mail/imap_name](../../std/mail/imap_name.ok) prepare and
decode mailbox names without opening connections or sending commands.
They implement a strict canonical modified UTF-7 profile for IMAP4rev1.
Session negotiation, UTF8=ACCEPT, IMAP4rev2 Net-Unicode and literal transport
remain separate work.

## Codec values

`ImapUtf7Encoded(origin[,limits]).string()` eagerly produces modified UTF-7.
`ImapUtf7Decoded(origin[,limits]).string()` eagerly validates and decodes it.
Repeated observations return the same immutable string.

The encoding follows the mailbox naming convention in
[RFC 3501 §5.1.3](https://www.rfc-editor.org/rfc/rfc3501.html#section-5.1.3):
printable ASCII is literal except ampersand, which becomes `&-`; other scalars
form maximal runs of UTF-16BE encoded in unpadded Base64 with comma replacing
slash. Each encoded run starts with ampersand and ends with a hyphen.
For example, `台北` becomes `&U,BTFw-`. Supplementary scalars use UTF-16 surrogate
pairs. No BOM is inserted or removed, and text is not normalized or case-folded.

The decoder requires printable ASCII on the wire, explicit shift termination,
the modified alphabet, valid unused Base64 bits and well-formed UTF-16BE.
It also requires exact equality with canonical re-encoding. Consequently,
adjacent encoded runs, encoded printable ASCII and encoded ampersand (`&ACY-`)
are rejected. Always using `&-` for ampersand is this implementation's
canonical profile; no claim is made that every alternative rejected by the
canonical check is prohibited by the base RFC.

Codecs preserve all Unicode scalars, including controls, BOM-shaped scalars
and noncharacters. They do not apply mailbox naming policy.
`InvalidImapUtf7(offset,reason)` exposes offset()/reason()/text(). Alphabet and
direct-character errors identify the original wire scalar; malformed Base64
or UTF-16 identifies the shift's ampersand. A canonical mismatch reports 0.

## Mailbox name value

`ImapMailboxName(origin[,limits])` accepts decoded Unicode text and eagerly
stores string(), wire_name() and quoted(). string() preserves the original text;
wire_name() is modified UTF-7; quoted() encloses it in double quotes and escapes
every quote and backslash once. Even names such as `NIL` are quoted.

This name profile rejects ASCII controls U+0000..U+001F and U+007F before
encoding, with `InvalidImapMailboxName(offset)` identifying the source scalar.
The codecs can still decode such wire names for inspection. Empty text is
representable, including an empty LIST reference; this does not establish that
any particular server operation accepts an empty mailbox name.

No hierarchy separator, server-side name identity, mailbox existence, permission,
or INBOX casing is inferred. In particular, `%` and `*` remain unchanged:
quoting prevents command syntax injection but does not remove LIST wildcard
semantics. A future LIST pattern abstraction must handle that separately.
The object always represents the IMAP4rev1 encoding choice. It must not be used
as evidence that UTF-8 mode has been enabled; modern IMAP naming is described
separately in [RFC 9051 §5.1](https://www.rfc-editor.org/rfc/rfc9051.html#section-5.1).

## Bounds and ownership

`ImapNameLimits()` defaults to 4096 decoded Unicode scalars and 32768 wire bytes.
The two-argument constructor accepts nonnegative `(scalars,wire_bytes)` and
rejects negative bounds with InvalidArgument. Limits may be zero.

Encoding checks the scalar count before conversion and bounds output as it is
written. Decoding checks the input length before conversion, bounds each decoded
piece against the scalar budget, and then verifies canonical form. Temporary
UTF-16/Base64 runs are bounded by the supplied input limits. For mailbox values,
wire_bytes also includes the complete quoted argument: quotes and added
backslashes count. An empty codec fits a zero-byte limit; an empty quoted name
requires two bytes.

`ImapNameLimitExceeded(kind)` exposes kind()/text(), where kind is `scalars`
or `wire_bytes`. Scratch buffers are owned and closed. Values retain no external
resources and add no native provider or dependency.

[Executable example](../../examples/std-v4/imap-names.ok) ·
Tests ·
Evidence.
