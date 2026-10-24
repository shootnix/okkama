# Mail flags

The first `mail/mailbox` slice represents immutable message flags, with no
mailbox I/O or network effects. `MailFlag` exposes kind(), string(), system(),
keyword() and order(other is MailFlag). `MailFlagKind` is system/keyword;
`SystemMailFlag` is answered/flagged/deleted/seen/draft/recent.

`SystemFlag(SystemMailFlag)` renders the canonical backslash-prefixed spelling.
Recent is an observation retained for legacy IMAP data, not permission to STORE
that flag or an IMAP4rev2 capability. `KeywordFlag(string)` preserves the exact
ASCII spelling of an unknown or standard keyword. It eagerly accepts 1..255
ASCII bytes using the IMAP atom grammar, excluding controls, space, parentheses,
braces-open, percent, asterisk, quote, backslash and closing square bracket.
The 255-byte cap is a library bound, not a protocol-imposed atom maximum.
The grammar and legacy Recent distinction follow
[RFC 9051](https://www.rfc-editor.org/rfc/rfc9051.html).

Wrong projections throw `WrongMailFlagKind(expected,actual)`, which exposes
expected(), actual(), text(). `InvalidMailFlag(offset,reason)` exposes offset(),
reason(), text(). Offsets are zero-based input scalar positions. `order()` uses
case-insensitive ASCII wire spellings, so system Seen differs from keyword Seen.
Structural language equality and generic OrderedSet are not overridden: callers
must use order() to compare protocol flags. The bounded MailFlags set and StoredMail snapshot are specified in
[stored-mail.md](stored-mail.md).

These flag values remain pure. [Maildir](maildir.md) supplies explicit local
storage operations separately; Mbox/MboxSession remain pending.
