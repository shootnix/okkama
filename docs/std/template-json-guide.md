# Explicit JSON strings for templates

## Contract before implementation

`JsonTemplateStrings(origin is Json)` implements TemplateStrings explicitly;
TextTemplate does not gain implicit arbitrary-object or JSON conversion.
The optional constructor `(origin, maximum_members is int, maximum_output is int)`
sets per-traversed-object member count and per-scalar UTF-8 bytes. Defaults are
1024 and 1048576. Negative budgets throw InvalidTemplateJsonLimits before any
source observation; zero is valid. Construction only binds dependencies.

`string(path is TemplatePath)` follows exact path segments through JSON object
members of the `Json` document. No casts.
At most 64 path segments are accepted, checked before source observation;
TemplateJsonLimitExceeded reports excess depth/count. Empty/missing or non-object
intermediate nodes throw MissingTemplateValue retaining the full requested path.
There is no array indexing, parent fallback or coercion.

### Current-item context (contract before implementation)

`string(path is string)` accepts a template spelling: exactly `.` selects the
adapter's origin itself; other strings use TemplatePathFromString and resolve
only inside that origin. A section can bind a fresh adapter to each item without
inheriting parent names. No trimming is performed by this adapter (the compiler
already trims tags). Invalid paths fail before observing JSON.

The existing `string(path is TemplatePath)` overload keeps exact member identity:
`TemplatePath(["."])` names the literal dot member; it never selects the current
item. Current scalar formatting, UTF-8 limits, one-event checks and source error
propagation match named interpolation. Current arrays/objects are non-scalar
errors with diagnostic path `.`; they are not traversed or materialized. A zero
member budget therefore permits current scalar interpolation.

- [x] RED/GREEN current-item and contextual member tests.
- [x] Update API, roadmap, comments and runnable example.

[Context example](../../examples/std-v4/template-context.ok),
RED/GREEN evidence.
Section iteration and truth rules belong to [SectionTemplate](template-sections-guide.md).

Final strings are emitted literally (including empty, HTML and template syntax),
booleans as true/false, numbers as Decimal.string(), null as empty text. Final
arrays/objects throw NonScalarTemplateValue(path, kind), retaining path()/kind()
and diagnostic text(). These are JSON wire categories, not Okkama reflection.
Scalar output exceeding maximum_output throws TemplateOutputLimitExceeded before
writing bytes. TextTemplate retains its separate aggregate output budget.

Lookups read the `Json` document directly: one member lookup per path segment,
no snapshot of the members. A `Json` is immutable, so repeated lookups see the
same data. Duplicate member names cannot exist in a `Json`: `ParsedJson` and
`Json(fields)` reject them with DuplicateJsonMember. The member budget is checked
against the size of each traversed object. Storage is bounded by 64 traversal
levels and the output-byte limit. No I/O, environment, file access or implicit
HTML trust.

- [x] Contract before implementation.
- [x] RED/GREEN consumer tests and local embedded tests.
- [x] Example, v4/API and evidence prepared for commit.

[Example](../../examples/std-v4/template-json.ok),
evidence.
The root-inline imported-constructor limitation recorded for template/library
also applies here. Full behavior is covered by 24 consumer groups; embedded
tests check the local diagnostic contract without importing constructors.
