# Structured template compilation in Okkama

## Contract before implementation

TemplateTree(source is string) represents a bounded source program. Optional
constructor `(source, maximum_input, maximum_tokens, maximum_depth)` accepts
nonnegative integer budgets, defaulting to 1048576 scalars, 20000 flat tokens and
64 nested sections. Constructors bind/validate limits only. nodes() parses the
complete source on every observation and returns an immutable @TemplateTreeNode.

This compiler extends the existing literal/variable/comment/escape grammar with
`{{# path}}...{{/ path}}`, `{{^ path}}...{{/ path}}`, and `{{> name}}`.
Whitespace around tag names is ASCII space/tab. Paths use the existing 64-segment,
4096-scalar rules; single `.` is allowed for the current section item. Partial
names are nonempty exact names up to 4096 scalars with no ASCII whitespace,
braces, parentheses or leading `#`, `^`, `=`, `&`, `>`. They do not name filesystem paths;
slashes and dots inside a name are literal library-name characters.

Sections close by exact trimmed opening path spelling. Unmatched, mismatched,
crossing and unclosed sections throw TemplateFailure at the offending closing
tag or unclosed opening tag. Coordinates refer to the original scalar input.
Standalone section/open/close/partial tags remove their horizontal indentation
and following LF/CRLF, exactly like standalone comments; this slice does not add
indentation propagation to a partial's rendered content.

Kinds are TemplateTreeKind.literal/variable/section/inverted/partial. Each
TemplateTreeNode retains kind(), text() (literal text or tag path/name), and
children() as @TemplateTreeNode. Only section/inverted nodes may have children.
Literal nodes contain nonempty text. Structural closing tags consume flat-token
budget but produce no tree node. Comments consume no tokens. Complete validation
precedes returning any tree; no data lookup, partial resolution or output occurs.

Root depth is zero; opening a section increments depth. Empty/flat programs allow
depth zero. InvalidTemplateLimits rejects negative budgets at construction.
TemplateFailure reports source/token/depth and structural errors. Scanner/tree
builders use persistent lists and successor objects, no native parser/cache.
Memory is bounded by input, token and nesting budgets. Returned nodes outlive the
source/compiler. Partial cycles cannot arise during compilation because no
library is consulted; expansion depth belongs to the later rendering contract.

TextTemplate retains its original flat grammar.
SectionTemplate consumes this tree for [section rendering](template-sections-guide.md);
[partial expansion](template-partials-guide.md) uses an explicit library.

- [x] Document structured grammar, representation and resource bounds.
- [x] RED/GREEN embedded grammar and boundary tests.
- [x] Preserve existing flat consumer/compatibility tests.

Result: 8 embedded tree blocks and 6 flat-parser blocks PASS; focused consumer
run is 5 files / 111 top-level tests PASS. The 40 old/new flat grammar fixtures
(80 assertions) still agree. The example prints `users`, `3`, `footer`.
RED/GREEN evidence is in docs/audits/evidence/template-tree/. This evidence covers compilation.
[Section rendering](template-sections-guide.md) has its own contract and tests;
[partial expansion](template-partials-guide.md) uses an explicit library.

## Verification commands

```sh
OKKAMA_STDLIB_DIR=std perl okkama-pl/bin/okkama -t std/template/tree.ok
OKKAMA_STDLIB_DIR=std perl okkama-pl/bin/okkama -t std/template/compile.ok
OKKAMA_STDLIB_DIR=std perl okkama-pl/bin/okkama examples/std-v4/template-tree.ok
cd okkama-pl
prove -lv t/interpreter/290-std-v4-template-tree.t t/interpreter/281-std-v4-template-text.t t/interpreter/282-std-v4-template-output.t t/interpreter/285-std-v4-template-library.t t/interpreter/286-std-v4-template-json.t
perl t/interpreter/289-std-v4-template-parser-parity.t
```

The root test runner cannot resolve imported catch-type identities, so embedded
tests catch builtin Error and assert behavior/coordinates. The consumer suite
additionally checks exact TemplateFailure/InvalidTemplateLimits types. No runner
or native provider changes are made.
