# Explicit bounded partial expansion

## Contract before implementation

SectionTemplate gains `(source, partials is TemplateLibrary)` and
`(source, partials, maximum_iterations, maximum_output, maximum_members,
maximum_partial_depth)` constructors. Defaults are 10000, 1048576, 1024 and 32.
All budgets are nonnegative. Existing constructors without a library keep rejecting
partial syntax before data observation; supplying an empty library enables syntax
but active missing references throw MissingTemplate with the exact name.

The library is an explicit source catalog: each NamedTemplate retains TextTemplate,
and expansion reads its source() only, then compiles with TemplateTree defaults.
TextTemplate's flat grammar and flat-render budgets are not used in this mode.
The SectionTemplate aggregate budgets govern the rendering; each source is bounded
by 1048576 scalars, 20000 tokens and 64 syntactic section levels. This interpretation
permits structured source in existing NamedTemplate without changing library APIs.

Only reached partials are looked up and compiled, immediately before expansion.
A skipped reference does not observe its library entry or fail for a missing,
invalid or recursive body. Entire root/partial source syntax is validated before
that source observes data. Partial syntax errors retain partial-local coordinates.
Names are exact, case-sensitive library keys, never filesystem/module names.
The current JSON context passes through unchanged. Standalone removal follows
TemplateTree; this dialect does not propagate indentation into the partial body.

Root partial depth is zero. Each expansion increments depth, and siblings restore
the parent's depth. Direct/indirect recursion fails at the configured maximum.
Conditional recursion may terminate normally. Each expansion also spends one
shared iteration, even an empty body, so repeated or branching expansion cannot
bypass the work limit. Sections and partials share output and iteration budgets.
Active section nesting is bounded by 64 across partial boundaries, independently
of partial depth. Failures have typed depth/work/output errors. No result escapes
on failure, source/library ownership stays borrowed, and nothing opens files.

- [x] RED partial/context/recursion/budget tests.
- [x] Implement and document bounded expansion in std.
- [x] GREEN focused tests, example, API and roadmap.

[Example](../../examples/std-v4/template-partials.ok),
RED/GREEN evidence.
