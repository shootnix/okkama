# Immutable GNU gettext catalogs and PO documents

`i18n/gettext` has no global `_()`, text domain, locale mutation, file access or
host expression evaluation. The caller supplies bytes, Unicode text and Locale.

`GettextCatalog(bytes,locale)` eagerly validates a **GNU MO revision 0.0** catalog,
in either byte order, and binds its immutable messages and plural expression.
Other revisions fail explicitly: revision 1/system-dependent strings are outside
this static-string profile. The optional hash table is range-checked; lookup uses
our own sorted keys. The supplied Locale is retained and does not override the
catalog's own Plural-Forms formula or infer anything from the process locale.

- membership(key is MessageKey) observes exact identity/context presence.
- text(key) retrieves a singular message; plural(key,count is Decimal) selects
  an indexed plural translation. MissingMessage and WrongMessageEntry are typed.
- metadata() returns GettextMetadata with ordered GettextHeader values(),
  case-insensitive membership(name)/text(name). Header payload remains in PO.
- GettextTranslation(catalog) implements the existing Translating role by
  converting PluralNumber.number() to Decimal. It composes with FallbackTranslation;
  invalid counts and malformed entries propagate instead of invoking fallback.

MO must contain exactly one metadata entry and one declared Content-Type charset.
Supported codecs are UTF-8/UTF8, US-ASCII/ASCII and ISO-8859-1/ISO8859-1/Latin1/
Latin-1, matched as ASCII names of 1..64 scalars without case sensitivity. Other charsets and bad
encoded text fail; no replacement, host codec selection or implicit conversion.
Plural entries require Plural-Forms and exactly the declared translation count.
Empty translations remain present entries.

GettextPluralRule(header).index(count) parses decimal constants, n, parentheses,
!, arithmetic, comparisons, &&, || and ?: with C precedence and lazy boolean/
conditional branches. Its portable arithmetic profile is **unsigned 64-bit**,
including modular addition, subtraction and multiplication, rather than the host
width of unsigned long. Counts must be nonnegative integral Decimal values within
0..18446744073709551615; no floating-point conversion or periodic large-number
guessing occurs. Division by zero and a result outside nplurals fail on selection.
There are at most 16 forms, 512 tokens, 2,048 expression scalars, 64 nesting levels
and 64 evaluation-tree levels. Literals have at most 20 digits and fit unsigned64.

MessageKey(context,identity) retains its existing convention: empty context means
absence. MessageKey(context,identity,has_context) additionally distinguishes an
explicit `msgctxt ""`; has_context() and catalog copying preserve that distinction.
The two forms order differently and never overwrite each other.

`ParsedPo(text).value()` eagerly parses an active PO document from already-decoded
Unicode. Its numeric C escape bytes use UTF-8 by default; ParsedPo(text,charset)
makes their encoding explicit for Latin-1/ASCII input. The metadata charset does
not silently reinterpret an already-decoded input string. Octal/hex bytes may span
adjacent quoted fragments or continuation lines before decoding. Named C escapes,
multiline strings, contexts and indexed plurals are supported. Translation indices
may appear in any order but must be unique and contiguous from zero.

PoEntry(context,identity,plural,translations,comments[,has_context]) retains its
coordinates and ordered translations. The optional sixth argument represents an
explicit empty context. Empty identity is reserved for a singular metadata entry;
NUL and the MO context separator cannot occur in message identities or contexts.

PoComments(records is Sequencing<PoComment>) preserves interleaved comment order.
PoComment(kind,text) uses translator, extracted, reference, flag, previous kinds.
Grouped projections are translator(), extraction(), references(), flags(),
previous_comments(); references preserve opaque reference-comment line text.
A five-sequence convenience constructor groups records in that same kind order.
Flags are ordered strings, not an unordered Set. Unknown flags remain present.
The current GNU `#=` alternative is read as flags; canonical output emits `#,`.
Whitespace within comment payloads is retained. Comments after a completed entry
attach to the next entry or remain document.trailing_comments() at EOF.

PoDocument(entries[,trailing_comments]) snapshots ordered nominal entries;
entries() includes the optional metadata entry. metadata() derives ordered header
values while preserving the original header payload. Duplicate full message keys
and case-insensitive metadata names fail. Obsolete `#~` input and domain directives
fail explicitly; preserving obsolete source belongs to separate translation tooling.

`PoString(document)` validates and retains its derived canonical Unicode text once;
string() repeats that projection. It preserves logical messages, context presence,
metadata payload, comment order and flags, with escaped string/control contents.
Plain runs are emitted together rather than through a native codec per scalar.
Caller-selected encoding and IO remain separate from this immutable value.

Budgets: MO input 16 MiB, 4,097 descriptors including metadata, 16,384 hash words,
1 MiB per referenced string and 4 MiB referenced bytes total. PO input/output
4 Mi scalar values, 65,536 physical lines, 1 Mi scalars per physical line,
8,192 quoted fragments per line/field and 65,536 lexical pieces in one quote.
Fields have at most 262,144 encoded bytes and 65,536 decoded scalars; message
identity/context/plural source coordinates have at most 1,024 scalars. Comments
are at most 4,096 records per entry and 4,096 scalars per record. Decoded document
content, including comments, is limited to 1 Mi scalar values; headers to 128 fields.
InvalidCatalog(offset,reason) reports MO byte positions or PO/formula scalar
positions; whole-catalog semantic failures use zero. Temporary builder cells close
on both success and failure; persistent fragments avoid repeated whole-field copies.

Tests 691–695 cover boundary rules and original catalog compatibility. Twelve
checksum-pinned GNU msgfmt 1.0 fixtures in both byte orders cover six languages,
17 representative counts (including unsigned64 maximum), contexts and Latin-1.
Their expected results come from Python GNUTranslations independently of Okkama.
GNU recompilation of all six reconstructed PO documents yields identical complete
translation catalogs, including metadata. Regenerate with
`python3 tools/generate-gettext-fixtures.py` when intentionally updating evidence.
[Runnable example](../../examples/std-v4/gettext-po.ok).

Primary format references: [GNU MO](https://www.gnu.org/software/gettext/manual/html_node/MO-Files.html),
[PO entries](https://www.gnu.org/software/gettext/manual/html_node/PO-File-Entries.html),
[plural forms](https://www.gnu.org/software/gettext/manual/html_node/Plural-forms.html),
[flag evolution](https://www.gnu.org/software/gettext/manual/gettext.html).

Plural formulas use iterative operator/value stacks. Parentheses, unary negation
and pending conditional branches share the existing 64-level nesting budget;
token, source and evaluation budgets remain unchanged. Binary precedence and
right-associative lazy ternary evaluation are preserved. Parsing deeply nested
parentheses does not consume the Perl call stack.
