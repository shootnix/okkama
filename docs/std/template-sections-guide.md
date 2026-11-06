# Bounded JSON template sections

## Contract before implementation

`SectionTemplate(source is string)` binds a TemplateTree and accepts explicit
`Json` data via `string(data)`. An overload `(source, maximum_iterations,
maximum_output, maximum_members)` sets nonnegative budgets (defaults 10000,
1048576 UTF-8 bytes, 1024 members per traversed object). The tree retains its
existing input/token/depth limits. Construction validates limits without parsing
or observing data. Every rendering compiles and validates the entire tree before
JSON observation. Without an explicit library, static partials are rejected even
inside skipped sections. [Library constructors](template-partials-guide.md) enable
bounded expansion. TextTemplate keeps its flat API.

Variables use JsonTemplateStrings: dotted members, current item `.`, typed
missing/non-scalar errors, no parent fallback, reflection or HTML escaping.
Sections select through `Json` member lookups only. Missing/non-object intermediate
paths are false; source exceptions are never translated into absence.

- Arrays iterate in order with each item as the current context.
- Objects render once with that object as context, including empty objects.
- Strings render once if nonempty; numbers (including zero) always render once;
  true renders once. These sections bind the selected scalar as current context.
- False, null, empty strings, empty arrays and absent paths are false.
- Inverted sections render once for false values, keeping their enclosing context.

Each executed body (including inverted bodies and empty bodies) spends one shared
iteration; nested sections share the same budget. For arrays, size is observed
once, negative sizes fail, and cardinality must fit remaining iterations before
item reads. Items are read in ascending order, once, immediately before their
body. Inverted arrays observe only size; inverted objects are always truthy and do
not read members. Traversed objects and positive object bodies bound member
sequences before reads, snapshot once and check duplicates before use. Custom JSON
values must emit exactly one event; caught duplicate-emission or downstream rendering errors stay invalid.

Output is preflighted per literal/interpolation against the remaining aggregate
UTF-8 byte budget before appending. String rendering returns only on success;
errors propagate without a partial result. Sources are borrowed. Temporary
buffers and the iteration ledger are closed on every path. No new native APIs.
Memory: output bytes plus at most one ledger byte per executed body, bounded
member snapshots and the existing depth-bounded syntax tree. Sources' own stored
strings are not counted as renderer allocations. Partial expansion has a separate depth budget and shares iterations/output.
[Streaming](template-section-output-guide.md) shares the string renderer and
adds no whole-result buffer; successful prefixes survive later failures.

- [x] RED tests for truth rules, contexts, order, absence and shared budgets.
- [x] Implementation, comments, embedded tests and runnable example.
- [x] GREEN focused tests; update roadmap and API.

## Verification commands

With Perl 5.42.2 selected, from the repository root:

```sh
(cd okkama-pl && prove -lv t/interpreter/292-std-v4-template-sections.t)
OKKAMA_STDLIB_DIR=std perl okkama-pl/bin/okkama -t std/template/sections.ok
OKKAMA_STDLIB_DIR=std perl okkama-pl/bin/okkama examples/std-v4/template-sections.ok
```

[Example](../../examples/std-v4/template-sections.ok),
evidence.
