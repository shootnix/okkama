# Template objects: contract and completion plan

The completion queue prioritizes plain templates after CSV/TOML/INI and before
HTML. Old v3 JSON-reflection signatures are candidates, not the accepted API.
Plain rendering binds an explicit nominal source of string values, without
inspecting arbitrary application objects or opening files/environment.
[Sections](template-sections-guide.md), [partials](template-partials-guide.md) and
[structured streaming](template-section-output-guide.md) have separate implemented
contracts. Context-aware HTML templates remain a separate task.

## Paths (contract before implementation)

TemplatePath(segments is Sequencing<string>) is an immutable nonempty sequence of
nonempty exact Unicode segments. A three-argument constructor adds nonnegative
maximum_segments and maximum_length; defaults 64 segments and 4096 Unicode
scalars including diagnostic separator dots. Zero budgets cannot fit a valid path.
Constructor snapshots size() once, validates cardinality before any item(), and
reads indices in increasing order once. The first invalid item stops observation.
No borrowed-source close; source failures propagate unchanged.

segments() returns the snapshot. string() joins segments with dots for diagnostics.
It is not an identity encoding: ["a.b"] and ["a","b"] are different paths although
their diagnostic strings coincide. Dot characters inside directly supplied
segments remain literal. Spaces, case, combining marks are preserved.

order(other is TemplatePath) is Order compares segments lexicographically by
Unicode scalar order, then uses sequence length when one is a prefix. It
implements Comparing<TemplatePath>. Equal paths have equal segment sequences,
never merely equal diagnostic strings. Repeated queries do not revisit sources.

TemplatePathFromString(origin is string).value() is TemplatePath splits literal
dots without trimming/case folding. A three-argument constructor overrides
maximum_segments/maximum_length; configuration validation is immediate but
source syntax validation is lazy. Before split it rejects excess source length.
Empty input and leading/trailing/consecutive dots are invalid. A single "." is
reserved for the current section item by template syntax, not a named path;
the path parser rejects it. Direct nominal segments are not reparsed.

InvalidTemplatePath(reason is string).text() preserves its diagnostic.
InvalidTemplatePathSize rejects a negative source size.
InvalidTemplatePathLimit rejects negative budgets before source observation.
TemplatePathLimitExceeded rejects cardinality/text beyond the explicit budgets.
Objects own only immutable coordinates, no native/external resources or destroy.

## Microsteps

- [x] Path tests: empty/boundary input, dot identity, Unicode order, snapshot
  observation, failures, lazy validation, lifetime, builtin tests RED.
- [x] Path implementation and detailed comments GREEN.
- [x] Executable example, API inventory, roadmap and evidence; commit.
- [ ] Nominal string bindings and missing-value contract, tests first.
- [ ] Bounded plain-text compilation/interpolation contract, tests first.
- [ ] Sections/partials and streaming output: explicit model protocol and budgets.
- [ ] HTML contextual contract and tests before implementation.

Path implementation: [example](../../examples/std-v4/template-path.ok),
evidence. 38 consumer groups and
two embedded groups; std-only change, no repeated interpreter regression.

## Explicit string bindings (contract before implementation)

TemplateStrings is a nominal role with string(path is TemplatePath) is string.
It exposes only explicit named string values, not arbitrary objects, type tags,
reflection or a JSON cast. Custom implementations may provide strings; their
failures propagate unchanged. Plain interpolation will consume this role.

TemplateBinding(path is TemplatePath, value is string) binds exact immutable
coordinates. path() returns its nominal path; string() returns its exact value.
No escaping, trimming, interpolation, coercion or HTML trust is implicit.

TemplateBindings(values is Sequencing<TemplateBinding>) does TemplateStrings.
Optional maximum_bindings is a nonnegative int, default 1024. Empty collections
are valid, including limit zero. Snapshot size() once, reject negative size/excess
count before item reads, then read each ascending item once. Duplicate validation
runs on the completed snapshot; equal segment sequences are duplicate keys.
Diagnostic-string collisions are permitted: literal dotted segments and multiple
segments have different identity. No borrowed-source close or later observation.

string(path) finds the exact path or throws MissingTemplateValue(path).
DuplicateTemplateValue(path) identifies the repeated key. Both errors retain
path() and text(). InvalidTemplateBindingLimit rejects negative configuration
before source observation; InvalidTemplateBindingSize rejects negative size();
TemplateBindingLimitExceeded rejects excess count. Original source errors escape
unchanged. Returned strings and paths outlive the snapshot. Duplicate validation
is bounded O(n²), lookup O(n). Strings are retained coordinates; this count budget
does not limit text length. Compilation and rendering have separate text budgets.
No native resources or implicit I/O.

- [x] Bindings external + embedded contract tests RED.
- [x] Immutable snapshots and nominal string role GREEN.
- [x] Error/lifetime/budget tests, example, comments/API inventory and commit.

Binding evidence: 6 files / 637 tests, PASS.
[Executable example](../../examples/std-v4/template-bindings.ok). No native changes.

## Bounded plain interpolation (contract before implementation)

TextTemplate(origin is string) binds source lazily. source() returns the original
text; string(data is TemplateStrings) validates the complete source before the
first lookup, then queries each variable occurrence once in source order.
Each observation renders afresh; custom data failures propagate unchanged.
The four-argument constructor adds nonnegative maximum_input (Unicode scalars),
maximum_nodes (literal/variable instructions), maximum_output (UTF-8 bytes).
Defaults: 1048576, 10000, 1048576. InvalidTemplateLimits rejects negative budgets
at construction. Empty text with zero budgets returns "". A nonempty instruction
uses one node, even an empty variable result. Compile failures (syntax and budget)
are TemplateFailure(offset,line,column,message), zero-based scalar offset and
one-based original coordinates. No partial string result or external I/O.

Accepted first slice: {{ path }} variables, {{! comment }} comments, {{{{ -> {{,
}}}} -> }}. Variable boundary whitespace is ASCII space/tab only. Literal text,
including LF/CRLF and Unicode, is preserved. Comments may span lines. A comment
alone on a logical line (aside from ASCII space/tab) removes its indentation and
following line ending; inline comments remove only the tag. No other standalone
rules yet. Stray closing delimiters and unclosed tags fail. Variable paths must
satisfy the default TemplatePath bounds (64 segments, 4096 scalars); a single "."
is reserved for future current-item sections. Section/inverted/partial tags,
delimiter directives and unescaped/trusted markup forms fail explicitly until
their separate implementation; they are not silently rendered as literal text.

Values are inserted literally once, never recompiled, coerced or HTML escaped.
MissingTemplateValue retains the requested exact path. Output checking is
incremental and UTF-8 byte-based, including literals. TemplateOutputLimitExceeded
stops before later lookups; custom source exceptions propagate unchanged.
No complete oversized UTF-8 intermediate is allocated: a native scalar scan counts UTF-8 bytes
without constructing a byte array. Successfully rendered strings
outlive the template/data objects; repeated calls do not mutate either.

Native compilation is pure bounded memory with cached complete instructions or
diagnostic. Native handles stay registry-owned until teardown. Domain rendering
does not expose these handles or inspect arbitrary object fields.

- [x] Grammar/lookup/output-budget fixtures and embedded tests RED.
- [x] Native compilation and nominal rendering GREEN.
- [x] Native boundaries, complete-before-observation, scope/repeat, Unicode limits.
- [ ] Example, comments, API/TODO, full regression for new provider, commit.

## Bounded streamed plain output (contract before implementation)

TemplateOutput(origin is TextTemplate, data is TemplateStrings,
destination is WritingText).write() is an explicit repeatable command. It borrows
all dependencies, does not snapshot/observe them during construction, and never
flushes or closes the destination. This first API accepts the concrete bounded
TextTemplate; arbitrary rendering plugins remain outside its contract.

TextTemplate.write(data is TemplateStrings, destination is WritingText) performs
the same complete compilation and lookup ordering as string(data). It checks
the entire next literal/variable contribution against the remaining UTF-8 byte
budget before emitting any part of that contribution. Successful prefixes may
already be visible if a later lookup/write/budget fails; no rollback is promised.
Sink/source exceptions propagate unchanged and stop later lookup/emission.

Nonempty contributions are split into chunks of at most 1024 Unicode scalars,
hence at most 4096 UTF-8 bytes. No scalar is split; no empty write, writeln,
implicit newline, flush or close. Streaming retains only the current
contribution and an immutable byte budget, not all preceding output fragments.
Every contribution is resolved once; values are never recursively interpreted.
On success the concatenated output is byte-for-byte identical to string(data).
Repeated write() renders again and appends another complete result.

- [x] Stream order/chunk/budget/source/sink/lifetime tests plus embedded tests RED.
- [x] Shared byte-budget validation, bounded chunk emission and command GREEN.
- [x] Example, API/docs/TODO and commit; no full repeat for std-only changes.
