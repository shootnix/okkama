# Explicit named template library

## Contract (before implementation)

This slice supplies immutable named plain templates for future static partials.
It does not enable partial syntax, recurse through templates or inspect files.
It lives in template/library, avoiding the template/model -> compile import cycle.
The v3 generic rendering/JSON/limits signatures are not revived.

`NamedTemplate(name is string, value is TextTemplate)` binds a nonempty exact
Unicode name and a concrete plain template. `name()` and `value()` return these
coordinates. Empty name throws `InvalidTemplateName`; whitespace, dots, slashes,
case and normalization forms remain literal and are not paths or imports.
Neither construction nor lookup compiles or renders a template. A future partial
grammar may restrict its spelling without changing direct library identity.

`TemplateLibrary()` is the empty library. `TemplateLibrary(values is
Sequencing<NamedTemplate>)` snapshots at most 1024 entries; a two-argument
constructor overrides `maximum_templates is int`. Configuration must be
nonnegative (InvalidTemplateLibraryLimit); zero admits an empty source.
Read size() once, reject a negative size (InvalidTemplateLibrarySize) or excess
count (TemplateLibraryLimitExceeded) before any item(). Read ascending indices
once, then validate duplicate exact names on the completed snapshot.
DuplicateTemplate(name) retains the first repeated name through name()/text().
Source exceptions propagate unchanged; no cursor(), first(), last() or close().

`membership(name is string) is bool` tests exact presence; `value(name is string)
is TextTemplate` returns the template or throws `MissingTemplate(name)` with
name()/text(). Missing and duplicate diagnostics include the original name.
`names() is Sequencing<string>` returns declaration order. An empty lookup name
is simply absent; invalid declaration names fail at NamedTemplate construction.
Repeated lookups never observe the source again. Returned templates and names
outlive the library; there is no owned resource or destructor. Libraries retain
TextTemplate dependencies and their existing private native compilation states.

Snapshot memory is O(n) references, duplicate validation O(n²), lookup O(n),
bounded by the explicit count. This count does not budget retained template
source bytes; TextTemplate keeps its separate compile/render limits.

The proposed EmptyTemplateLibrary projection is unnecessary: TemplateLibrary()
represents the empty domain value directly. The checklist will record this as
an explicit API replacement, not the implementation of a phantom type.

- [x] Record contract and replacement of v3 candidates.
- [x] RED consumer tests.
- [x] GREEN implementation, comments and embedded tests.
- [x] Example, API/roadmap/evidence prepared for commit.

[Example](../../examples/std-v4/template-library.ok),
evidence.
Root test bodies currently fail imported constructor lookup (also reproduced on
the existing template/render module). Embedded tests cover local empty/budget/error
behavior; consumer tests cover imported TextTemplate, snapshots and rendering.
No interpreter changes were made to bypass this existing limitation.
