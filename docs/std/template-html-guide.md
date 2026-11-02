# Contextual HTML templates

## Contract before implementation

HtmlTemplate(source) binds source without parsing. The four-argument constructor
adds nonnegative maximum_input, maximum_nodes, maximum_output (defaults 65536
source scalars, 10000 instructions, 1048576 UTF-8 bytes). source() returns the
original spelling. string(Json) and write(Json,WritingText) validate the
complete program before any data/sink observation. Repeated calls compile anew.

This first slice accepts flat variables, current item '.', template comments and
literal delimiter escapes. Sections and partials are initially rejected explicitly.
A later slice will define typed HTML partial composition. No arbitrary objects,
raw/trusted interpolation, expressions or implicit JSON conversion.

A conservative HTML fragment grammar determines interpolation context. Tags and
attributes use ASCII case-insensitive names. Tags must be explicitly balanced,
with nesting <=64; HTML void elements need no close and alone accept '/>'. Only
ASCII-letter-start tag names and ASCII letters/digits/'-' attribute names are
accepted, with names bounded at 128 scalars. Attributes require whitespace separation. Literal unquoted values are
accepted, but interpolation is allowed only in quoted values of these names:
`id`, `class`, `title`, `alt`, `role`, `lang`, `dir`, `aria-*`, `data-*`.
Unknown attributes are literal-only. This whitelist excludes event handlers,
URLs, srcdoc, CSS, meta-refresh content and legacy parameter values.

Text interpolation uses EscapedHtml(value,false); quoted attributes use
EscapedHtml(value,true). Existing references in data are escaped again as data.
Literal authored markup is preserved, including dangerous literal URLs/scripts:
this is context validation and escaping, not browser sanitization.

All interpolation in tag/attribute names, unquoted attributes, comments,
declarations and raw/RCDATA content is rejected. Strict comments disallow '<'
and interior '>'; '--' occurs only in '-->'. Only <!DOCTYPE html> is accepted
(case-insensitive, single separating space). Processing instructions, SVG/MathML,
plaintext and NUL are rejected entirely. Script/style/title/textarea/xmp/iframe/
noembed/noframes/noscript accept literal content, with '<' only for their exact
case-insensitive closing tag (optional ASCII whitespace before '>'). This rejects
script escape/double-escape and browser-recovery ambiguities instead of guessing.
All unfinished tags, comments, quoted values and raw elements fail validation.

InvalidHtmlTemplate retains text() describing the context violation; template
syntax errors retain existing TemplateFailure coordinates. Missing/non-scalar
JSON values and source/sink errors retain their existing types. Output byte bounds
apply after escaping, and complete contributions precede bounded sink chunks.
A later output failure may leave earlier contributions visible. Borrowed sources
and destinations are never closed. Streaming keeps no aggregate result; a scalar
escape can temporarily occupy at most six times its input, itself bounded by the
remaining output budget. Owned temporary buffers close on every exit.

- [x] RED context, escaping, lazy validation, limits and borrowed output tests.
- [x] Okkama implementation with comments and embedded tests.
- [x] GREEN focused tests, example, roadmap and API.

The deliberately restricted grammar is informed by the WHATWG
[tokenizer states](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
and [attribute syntax](https://html.spec.whatwg.org/multipage/syntax.html#attributes-2).
It does not implement a browser DOM or the complete recovery algorithm.

[Example](../../examples/std-v4/template-html.ok),
RED/GREEN evidence.
