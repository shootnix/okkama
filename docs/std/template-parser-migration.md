# Template parser migration to Okkama

The parser implementation lives in `std/`. The language interpreter and native
template provider remain unchanged. This is a prerequisite for std-only section
and partial support, not a claim that those features already render.

## Contract and checklist

- [x] Specify the parser contract in compile.ok before implementation.
- [x] Add failing embedded TemplateProgram tests before implementation (RED).
- [x] Parse the existing flat grammar with Okkama objects and successors.
- [x] Preserve literal escapes, variables, standalone comments, complete-before-
  observation validation, cardinality/input limits and original CRLF coordinates.
- [x] Keep TextTemplate constructors, source(), string() and write() compatible.
- [x] Check focused consumers, embedded tests and old/new parser equivalence.
- [x] Add section/inverted-section grammar and a bounded instruction tree; see [the tree guide](template-tree-guide.md).
- [x] Add section context and bounded string rendering; see [section guide](template-sections-guide.md).
- [x] Add explicit partial expansion with depth and aggregate-output budgets; see [partial guide](template-partials-guide.md).

`TemplateProgram(source, maximum_input, maximum_nodes).instructions()` returns an
immutable array of `TemplateInstruction` objects with text()/variable(). These
are internal collaborators (Okkama has no module-private declarations).
`TemplateScanner.following()` returns a successor holding the next source
position, persistent instruction list and literal fragments. Only completed
instructions escape. Fragments are joined once per literal instruction; comment
indentation scans backward only through adjacent horizontal whitespace.

Construction is lazy. Each observation reparses the source: there is no mutable
or native compilation cache. Program objects have normal scope ownership.
The existing runtime/template UTF-8 sizing primitive remains for output budgets;
none of its grammar/compiler entry points are used by TextTemplate anymore.

## Validation

- Six embedded blocks PASS. Original root tests that failed imported-constructor
  lookup now use local parser/budget objects; rendering stays covered by consumers.
- Four existing consumer files / 99 top-level tests PASS: text grammar, streamed
  output, named libraries and JSON adapters.
- Forty compatibility fixtures / 80 assertions PASS: same instructions or exact
  error offset/line/column/message as the unchanged native parser.
- No full interpreter regression, per the requested workflow.

Focused commands from the repository root, with Perl 5.42.2 selected:

```sh
OKKAMA_STDLIB_DIR=std perl okkama-pl/bin/okkama -t std/template/compile.ok
cd okkama-pl
prove -lv t/interpreter/281-std-v4-template-text.t t/interpreter/282-std-v4-template-output.t t/interpreter/285-std-v4-template-library.t t/interpreter/286-std-v4-template-json.t
perl t/interpreter/289-std-v4-template-parser-parity.t
```

Evidence is retained in docs/audits/evidence/template-parser/. The RED log also records the
pre-existing inline-import failures; the new TemplateProgram cases fail because
the object did not exist. All six final embedded blocks pass.
