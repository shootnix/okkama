# Message header values and rendering

MessageHeaderName validates nonempty ASCII 33..126 excluding colon. string()
retains spelling; Comparing.order() ignores ASCII case. Object identity still
includes spelling; use order() or MessageHeaders lookup for field-name matching.
MessageHeader validates a logical ASCII value: printable characters and HTAB,
with CRLF accepted only immediately before SP/HTAB and removed by unfolding.
No trimming occurs. InvalidMessageHeader reports the scalar offset within the
name/value being validated and a diagnostic reason. Unicode values require a
separate MIME encoded-word or SMTPUTF8 capability; this API does not invent one.

MessageHeaders snapshots one Sequencing<MessageHeader> (size once, ordered items
once), preserving duplicates and field order. entries() is immutable; membership,
first and values match names through order(). first throws MissingMessageHeader;
values returns empty when absent. addition returns a new bounded value, leaving
the original intact. Default maximum 1024 entries, explicitly replaceable through
the second constructor argument; invalid bounds/source sizes use InvalidArgument,
overflow uses MessageHeaderLimitExceeded. The retained limit governs addition.

RenderedMessageHeaders.bytes() emits name, colon, one space, the exact unfolded
value, and CRLF per field. It does not append a header/body separator. Empty
collections yield empty bytes. Folding inserts CRLF before whitespace runs,
aiming for 78 octets; it never changes whitespace or splits a non-whitespace
token. A boundary immediately preceded by backslash is conservatively excluded
to avoid splitting quoted-pairs. Every emitted line is <=998 octets excluding
CRLF; an indivisible run/prefix that cannot fit is rejected. This generic layer
does not validate field-specific address/date/MIME grammars or promise every
possible structured-field folding opportunity. Callers supply valid structured
field text from its corresponding formatter.
Rendering InvalidMessageHeader offsets refer to the unfolded serialized field
(including its name and colon); constructor errors refer to the supplied name
or value. The zero offset denotes a whole-line violation.

RenderedMessageHeaders(headers,maximum_bytes) replaces the default 1048576-byte
output budget. Check each line before appending; budget errors expose no partial
result. Source failures propagate; local buffers close on success/failure.
Construction retains the header collection; rendering occurs on bytes().
Repeated rendering is deterministic. There is no HTTP header reuse, native
parser or I/O. Full wire header-section parsing remains with message parsing.

Source: [RFC 5322, sections 2.1.1 and 2.2](https://www.rfc-editor.org/rfc/rfc5322.html#section-2.2).
