# Explicit HTML template library

## Contract before implementation

NamedHtmlTemplate(name, value is HtmlTemplate) associates a nonempty exact Unicode
name with an HTML template. TextTemplate and arbitrary sources are not accepted.
Construction and lookup retain values without parsing or rendering them.

HtmlTemplateLibrary() is empty. Sequence constructors snapshot ascending items
once, after one size observation; optional maximum_templates defaults to 1024.
Negative limits/sizes and excess cardinality fail before item reads; zero permits
empty input. Duplicates are checked after the completed snapshot. No trimming,
case folding, normalization, filesystem or module lookup. Source errors escape
unchanged; sources are never closed. Lookup/membership/names never revisit them.

value(name) returns HtmlTemplate or MissingHtmlTemplate retaining name(). names()
preserves declaration order. Other failures are DuplicateHtmlTemplate,
InvalidHtmlTemplateName, InvalidHtmlTemplateLibraryLimit,
InvalidHtmlTemplateLibrarySize and HtmlTemplateLibraryLimitExceeded.
Source text can be invalid until rendering; this library does not certify source
syntax. Returned values outlive the library. Memory O(count) references, duplicate
validation O(count squared), lookup O(count), all within the explicit bound.

- [x] RED nominal-type, snapshot, duplicate, absence and budget tests.
- [x] Implementation, comments and embedded tests.
- [x] GREEN focused tests, example and API.

[Example](../../examples/std-v4/template-html-library.ok),
evidence.
HTML partial composition is deferred; this library alone does not expand tags.
