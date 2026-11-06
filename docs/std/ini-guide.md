# Okkama INI dialect and object contract

Contract recorded before implementation. This is Okkama's explicit INI dialect,
not a promise of compatibility with every configuration-file convention.

## Values and printing protocol

IniEntry(name is string, value is string) represents one immutable string binding.
name()/value() observe its coordinates; print(to is IniWriting) emits one entry.
No conversion of numbers/booleans, interpolation, inheritance or normalization.

IniSection(name is string, entries is Sequencing<IniEntry>) snapshots entries in
source order; a third maximum_entries argument overrides the default 1024.
name(), entries(), membership(name), value(name) expose section identity and lookup.
MissingIniEntry(section,name) and DuplicateIniEntry(section,name) identify failures.

IniDocument(sections is Sequencing<IniSection>) snapshots sections in source order;
a second maximum_sections argument overrides the default 128. sections() and
section(name) expose the represented document. MissingIniSection(name) and
DuplicateIniSection(name) identify failures. No ambient DEFAULT section.

Names are exact case-sensitive Unicode strings, including empty names, literal
dots and whitespace. Quoted names in the dialect make all these coordinates
representable. No case folding or normalization. DEFAULT is an ordinary name.

Snapshot construction observes size() once, checks it before item access, and reads
each item exactly once in increasing order. InvalidIniLimit rejects negative
budgets before observing the source; InvalidIniSize rejects negative size();
IniLimitExceeded rejects excess cardinality. Zero permits empty containers.
Sources are borrowed and never closed. Snapshots retain immutable binding/section
objects; repeated lookup/printing does not revisit the source. Source errors
propagate unchanged. Duplicate validation is O(n²) within the explicit bound;
lookup is O(n). String coordinates are retained, not copied through JSON or maps.

IniContent defines print(to is IniWriting) is void. IniDocument and IniSection
implement it. IniWriting defines section(name is string) is void and
entry(name is string, value is string) is void. Documents emit each section,
then its entries; empty sections still emit their header. No hidden formatting,
I/O or automatic close. Observer failures propagate unchanged and stop emission.
This lets a document control its representation without a printer inspecting
arbitrary objects. The original v3 map-shaped entries API is replaced by ordered
nominal IniEntry bindings.

## Parser and canonical text

ParsedIni(origin is string).value() is IniDocument parses only on observation.
The explicit constructor also takes maximum_input, maximum_sections and
maximum_entries (total bindings); defaults 1048576 Unicode scalars, 128, 10000.
Each section additionally has at most 1024 entries. All budgets are nonnegative.
Input/budget/syntax/conflict failures are typed; no partial document escapes.
InvalidIni retains line(), column() and text(), with one-based scalar coordinates.

Input lines use LF or CRLF; a bare CR is invalid. Blank lines and lines whose
first non-space/tab character is # or ; are ignored. Every entry requires an
explicit preceding section header. No implicit/global/DEFAULT section.
Section headers are [name]; bindings are name=value. Bare names and bare values
trim outer ASCII spaces/tabs. Names/values beginning with a double quote use
single-line escaped strings instead: backslash, quote, b/t/n/f/r and uXXXX or
UXXXXXXXX scalar escapes. Raw control characters other than tab are rejected.
Quoted names can include delimiter characters, empty strings and escaped newlines.
Quotes do not introduce multiline/continuation syntax. No inline comments: # and
; in a bare value are literal text. Trailing material after a header or quoted
value is invalid. Duplicate decoded section/entry names are errors, not overwrites.

IniString(origin is IniContent).string() returns canonical text through IniWriting.
An optional maximum_output bounds Unicode scalars, default 1048576; zero accepts
an empty document. Every name/value is double-quoted with deterministic escapes;
each header/binding ends with LF; no extra blank separators or comments.
Empty document -> empty string. Printer state enforces valid section/entry order,
unique names, bounded output and sticky failure even for custom emitters.
Construction is lazy. Observer/custom-content failures propagate; no I/O.
Repeated rendering starts a fresh bounded emission, never returns partial text.

## Microsteps

- [x] Model contract tests and embedded tests RED.
- [x] Bounded nominal values/protocol GREEN, comments, inventory/example/commit.
- [x] Parser and printer grammar/budget tests plus native embedded tests RED.
- [x] Native state and pure Okkama projections GREEN; no new dependencies.
- [x] Order, Unicode, quotes/controls, duplicates, limits, callback failure,
  lifetime/aliasing and repeatability; no ambient interpolation.
- [x] Full regression for native changes; docs/evidence/example/commit.

Model validation: evidence,
**5 files / 582 tests, PASS**. [Example](../../examples/std-v4/ini-values.ok).
The model slice changes only std and tests/docs, so it does not repeat the full
interpreter regression. Parser/native work will require its own full check.

Codec validation: evidence.
59 external codec scenarios, six domain and two native embedded test groups.
Native failures include wrong indices, negative budgets, invalid Unicode scalars
and access after registry teardown. All 66 Unicode noncharacters remain valid.
Native parse/emit memory belongs to the registry until teardown; returned nominal
values contain their own strings.
[Executable codec example](../../examples/std-v4/ini-codec.ok).

InvalidIniLimit rejects negative constructor budgets before source observation;
zero is valid. InvalidIni covers parse syntax, conflicts and exhausted input/
cardinality budgets. IniOutputLimitExceeded covers output size;
InvalidIniEmission covers entry-before-section and invalid native scalar input.
DuplicateIniSection/Entry identify custom-emitter duplicate coordinates.
Custom source errors propagate unchanged. Once a writer operation fails, its
buffer is discarded and later observations repeat the first error.

Canonical escaping uses backslash, quote, b/t/n/f/r, and lowercase uXXXX for
remaining ASCII controls including DEL. Ordinary Unicode scalars are literal.
Limits count scalars, not UTF-8 bytes; CRLF occupies two input scalars.

Full regression of isolated commit d03aaa6 completed with exit 0; see the
evidence record. Final counts were
not retained because the log was relocated during execution.
