# Changelog

All notable changes to Okkama. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow [Semantic Versioning](https://semver.org/). A release version lives in
`okkama-rs/Cargo.toml` and in the `vX.Y.Z` tag.

## [Unreleased]

### Changed

- **One JSON module.** `import "json"` replaces `json/value`, `json/parse`, `json/read`,
  `json/write` and `json/codec` (removed). A value is one immutable `Json` over a node of
  the runtime's tree, parsed (`ParsedJson(text).value()`) or built in code (`Json(42)`,
  `Json([JsonField("id", 42)])`, `Json(items: [...])`, `Json()` for null, also from
  `BigInt` and `float`). Reads are one native call each: `text/int/number/boolean(name)`,
  `get`, `list`, `at`, `items`, `names`, `values`, `fields`, `has`, `optional(name)` with
  fallbacks; errors keep their RFC 6901 pointer, which the runtime computes only when
  asked. `string()` and `pretty(n)` write the whole tree in one native call, with the
  same text as before. A codec is a plain object with `value(Json)` and `json(T)`; the
  `JsonValue`/`JsonWriting`/`JsonReading` roles, `JsonView`, `JsonOption`, `CompactJson`,
  `PrettyJson` and `JsonCodec` are gone. See `docs/std/json.md` and the migration table in
  `docs/std/stdlib-v4-migration.md`.
- `web/reply.Json` is now `JsonReply(status, value)`; `web/request.RequestJson` wraps a
  `Json` (`value()` gives it) and gains `RequestJsonOptional`. `logging`, `testing`,
  `template/json`, `template/sections`, `template/html` and `config/document` take `Json`.
  `template/json` lost `InvalidTemplateJsonSize` and `InvalidTemplateJsonProtocol`, which
  only a custom `print()` could trigger.
- Imports: a std module resolves imports from the std root first, so `"json"` from
  `std/template/json.ok` is `std/json`; a file never imports itself, so `app/json.ok`
  importing `"json"` finds `std/json`.

### Added

- Natives `runtime/json.json_text_node`, `json_number_node`, `json_boolean_node`,
  `json_null_node`, `json_array_node`, `json_object_node`, `json_node_write`,
  `json_node_pointer`, `json_member_values`, `json_node_items`, `json_member_text`,
  `json_member_spelling`, `json_member_boolean`, `json_member_int` (Rust and JVM).
- `benchmarks/json`: parse, walk and write six document shapes (many small records, a
  few wide objects, deep nesting, long strings, numbers, orders) in Okkama on Sparrow
  and on the JVM, Python `json` and Perl `JSON::PP`, with a checksum check.

### Fixed

- Internal compiler error "block is unreachable" when both branches of an `if` end in a
  self tail call (`value Next(...)` in a `#projected` action, a recursive binary search):
  tail-call elimination now drops the join block it leaves without entries. It broke
  `collections/ordering` searches (`config/document`, `http/media`, `locale`), so seven
  cookbook labs failed to compile.

### Performance

- Parsing and writing JSON run in the runtime: on the six shapes Okkama parses at
  0.7–1.7× Python's time and writes at 0.3–1.3×, and is 7–1000× faster than JSON::PP.
  Walking a parsed document in Okkama code takes 2–7× Python's time on Sparrow and
  0.5–3× on the JVM.
- **Sparrow registers are shared.** Sparrow codegen gives SSA values whose lifetimes
  never overlap one register of their type, instead of one register per value: a
  `match` over six cases no longer keeps the registers of all six, and every call
  clears and releases fewer callee registers (`Sums.plus` of the JSON benchmark: 102
  registers before). A result never shares a register with its own operands.
- Sparrow JIT: `enum.case` and `enum.cmp` (enum constants, `match` and `==` on enums)
  run in machine code, `str.len` calls a direct helper; before, all three went through
  the generic interpreter step. Walking JSON documents on Sparrow is 25–35% faster
  (records 867 → 590 ms, deep 301 → 209, orders 189 → 138).

## [0.3.1] — 2026-10-31

### Added

- **PGO builds**: `make pgo` (and `make install-pgo`) builds the release binaries with
  profile-guided optimization (`okkama-rs/tools/pgo/build.sh`): an instrumented build, a
  training run of `tools/pgo/json.ok`, the benchmarks on Sparrow and IR and std inline
  tests, then the final build with the merged profile. Needs `rustup component add
  llvm-tools`. Against the plain release build: JSON 14–18% faster, the benchmarks 6%.
  `-C target-cpu`, thin LTO and `panic = "abort"` were measured too: no gain from the first
  two, and abort (5% on JSON) would turn internal errors into aborts while the JIT still
  catches panics as a fallback, so the profile keeps unwinding.
- **`\u{H}` escapes** in string and char literals: one to six hexadecimal digits name a
  Unicode scalar value, `"\u{1b}[1m"`, `"\u{e9}"`, `'\u{0}'`. Surrogates, values above
  `10FFFF`, missing braces and empty or overlong digit runs are lexer errors (spec §4.2).
  Until now control characters other than `\n`, `\r`, `\t` could not be written in source.
- **`json/codec`**: `role JsonCodec<T>` maps a domain type to JSON and back with a
  hand-written object, both directions side by side. `JsonList(item:)` handles arrays and
  nests, `JsonDecoded(codec:, source:)` and `JsonEncoded(codec:, origin:)` read and write
  text, and `JsonTextCodec`, `JsonIntCodec`, `JsonBooleanCodec`, `JsonDecimalCodec` cover
  arrays of scalars (`docs/std/json-codec.md`, case `L09.json.codec`).
- **Optional JSON members**: `JsonView.optional(name)` is a `JsonOption`; each reader takes
  the value for an absent or `null` member — `view.optional("nick").text("anonymous")`,
  `view.optional("address").optional("city").text("")`, `view.optional("tags").items()`.
  A present member of another kind is still `WrongJsonKind`. `JsonView.required(name)` is
  `member(name)`. There is no null-like value: a `JsonOption` holds zero or one view.
- **Scalar JSON members**: `JsonMember("id", 42)`, `JsonMember("name", "Ada")`,
  `JsonMember("vip", true)` and `Decimal` values wrap themselves by declared type;
  `JsonNumber(value is int)`.
- `json/parse.JsonNode` reads its own kind directly: `text()`, `spelling()`, `number()`,
  `boolean()`, `size()`, `item(index)`, `name(index)`, `names()`, `members()`,
  `membership(name)`, `member(name)`, `optional(name)`; `ParsedJson.node()` is the root node.
- Natives `runtime/json.json_member_position` (a member's index by name, one call),
  `runtime/json.json_quote` (a JSON string literal), `runtime/json.json_number_text`
  (the JSON spelling of a decimal), `runtime/json.json_repeated_name` (the first repeated
  name) and a native JSON output (`json_output`, `json_output_raw`, `json_output_text`,
  `json_output_number`, `json_output_member`, `json_output_string`; each append answers
  the UTF-8 size so far), in the Rust and JVM providers.
- `role json/value.JsonReading`: how `JsonView` reaches a value. `json/read` reads any
  `JsonValue` through `print()`, `json/parse` reads parsed nodes directly
  (`ParsedJson.reading()`, `JsonNode.reading()`). Readings report `JsonKindMismatch`,
  `MissingJsonMember` and `JsonNumberMismatch` without a position; `JsonView` reports
  them as `WrongJsonKind`, `MissingJsonValue` and `InvalidJsonNumber` at its pointer,
  with the same texts as before.
- `JsonObject(members is @JsonMember)`: a builtin array of members is kept as it is,
  without the defensive copy that `Sequencing<JsonMember>` still gets.

### Changed

- **Functional suffixes are postfix forms.** `-> bindings { … }` belongs to its
  `map`/`filter`/`reduce` call like a further member call, so it can stand inside any
  operator, argument or condition: `total + xs.reduce(0) -> s, x { s + x; }`,
  `ready && xs.filter() -> x { x > 3; }.size() > 0`. Before, it was accepted only as a
  whole expression. The top level of a `for` source still gives the arrow to the loop.
- `json/read.JsonView` of a value declared as `ParsedJson` or `JsonNode` reads the parsed
  tree directly instead of through `print()`, with the same values, pointers and errors
  (case `L09.json.parsed-reading`). Views of other `JsonValue`s are unchanged.

### Performance

Measured on an Apple M2 Max; all runs and tables are in
[benchmarks/reports/2026-10-31-v0.3.1](benchmarks/reports/2026-10-31-v0.3.1/README.md).
Reading 10 000 orders (2.7 MB) into domain objects and writing them back (1.5 MB), Sparrow:
7.0 s → 0.33 s (0.27 s with PGO); Python 3.9 with its C `json` takes 55 ms. For 50 000
orders (14 MB) 1.15 s (0.98 s with PGO) against Python's 0.38 s; parsing is faster than
Python's. The general benchmarks are unchanged except `05-strings` (1.23×); Sparrow with
the JIT is 2.36× faster than Python 3.9 on average over benchmarks 01–10.

| Stage, 10 000 orders | 0.3.0 | 0.3.1 | 0.3.1 PGO | Python 3.9 |
|---|---|---|---|---|
| parse | 21 ms | 21 ms | 17 ms | 19 ms |
| decode into objects | 1 934 ms | 156 ms | 128 ms | 12 ms |
| build the `JsonValue` tree and write the text | 5 030 ms | 150 ms | 125 ms | 24 ms |

- **Empty ownership regions are elided.** When a program has any `destroy`, every scope
  opened a region, and every call created one and walked its result on exit. A new OIR
  pass drops regions that nothing is ever adopted, tracked or transferred into (they
  can own nothing); `region.enter` and `region.unwind` become `undef`, and readers of
  the unwind result read the failure it passed on. All engines run the same OIR. Regions
  that are opened but stay empty at run time close without walking `keep`.
- **Native calls** cost about 46 ns instead of 200 ns on Sparrow: the provider of a
  native is resolved and checked against the ABI once per function, not per call;
  signatures, module and function names come from tables indexed by the native's
  number instead of a SipHash map and symbol splitting; providers are routed by a
  table, `runtime/json` by `match` on the native itself; and JIT code calls a native
  with arguments copied from its registers into an array on the stack, instead of a
  generic interpreter step and a vector per call.
- `JsonView` reads of parsed documents find a member by one native call, check scalar
  kinds in the native read itself, and spell their RFC 6901 pointer only when a
  failure or `pointer()` needs it.
- `CompactJson.string()` writes the whole document into one native JSON output, one
  native call per scalar or member name, instead of a byte buffer, a regex split per
  string and several `Decimal` natives per number. The output is byte-for-byte that of
  0.3.0.
- `JsonView` reads a parsed node with one object per value read: the node reading holds
  the native node itself, and a view is its children's link to the pointer.
- `JsonObject` finds a repeated member name by one native call instead of sorting names.
- The general benchmarks (`benchmarks/run.py`) are unchanged within noise; `05-strings`
  is 1.16× faster.

### Known limitations

- An array literal of values of different concrete types that share a role
  (`[JsonText("a"), JsonNull()]`) gets its element type from a single matching overload;
  when a constructor is overloaded for both `@T` and `Sequencing<Role>`, it is typed
  `@any` and matches neither. `JsonArray` therefore keeps its one `Sequencing<JsonValue>`
  constructor.

### Removed

- `conformance/contracts/stdlib-inventory.json`: the snapshot of the std public surface had
  fallen far behind `std/` and nothing read it. `tools/inventory_std.py` now writes only
  the per-module coverage contracts in `contracts/stdlib.json`.

## [0.3.0] — 2026-10-31

### Added

- **`std.` paths**: a standard library declaration can be named in place, without an
  import — `std.io.stdout.Stdout()`, `let p is std.fs.path.Path = …`,
  `std.compare.Order.less`, `std.runtime.debug.data_dump(v)`. A path is a hidden
  namespace import of that module (looked up in the bundled std only), so the module is
  loaded once per program and shares its types with explicit imports. `std` is reserved
  (spec §5.2.1).
- **Editor completion for `std.` paths** (`okkama lsp`, VS Code): after `std.` and each
  folder, the folders and modules of the bundled std at that level; after a module, its
  objects, roles, enums and native actions with their doc comments; after an enum, its
  cases. Whole paths (`io.stdout`, and `io.stdout.Stdout` once a letter is typed) are
  offered from anywhere in std.
- **Editor support for named arguments** (`okkama lsp`, VS Code): at the start of an
  argument (right after `(` or `,`, or while typing a name) the parameters not yet named
  are offered as `name:`, first in the list. This happens only where a named call is
  allowed: not after a positional argument, and not for variadic actions, builtin actions
  or action values. Signature help highlights the parameter by its name in a named call
  (`obj.action(zip: 1, name: …)`) and picks the overload by the names already given.
- **Optional module version**: `module app;` is valid. A module without a version has
  ABI major `0`. Files of one multi-file module declare the same version, or none in
  every file.
- **Blade projects** (spec §5.2.2). A directory with `blade.toml` is a package root.
  Its `[dependencies]` are installed in `pkg/<name>/` of the project root and are
  imported as `import "pkg/web/router" take Router;`.
  - Only dependencies declared by the importing package are importable this way;
    transitive dependencies are not.
  - Module files of a project declare no version, because it is set in `blade.toml`.
  - The compiler, `okkama lsp` and the REPL find the project from the file, so a
    program resolves the same way from any working directory.
- **blade manages dependencies** (`docs/blade.md`).
  - Commands: `blade install`, `blade install SOURCE [VERSION]`, `blade remove NAME`
    and `blade update [NAME...]`.
  - Sources: git repositories (`vX.Y.Z` tags) and local package directories
    (`path = "…"`).
  - Requirements: `12.3.0` (compatible), `12.*`, `=12.3.1`, `*`. Each package has one
    version per project, and a conflict names every requirement and who made it.
  - `blade.lock` records the version, git commit and SHA-256 of every package. A moved
    tag or changed files stop `install` until `update`.
  - Installation is all or nothing.
  - Packages with natives, module versions in files, foreign module names or symbolic
    links are refused.
- **Editor completion for dependency imports** (`okkama lsp`, VS Code). The completion
  list opens right after the quote and after each `/`:
  - `import "` offers `pkg/`;
  - `pkg/` offers the direct dependencies of the file's package, with their version
    requirement and source, and marks the ones not installed yet;
  - `pkg/web/` offers the folders and modules of the installed package.

  `"` and `/` open completion only inside import paths, so division does not.
- `blade new` writes `blade.toml` (it was `NAME.toml`), creates `pkg/` (it was `lib/`)
  and starts `src/main.ok` with `module NAME;`.
- **Variadic and typed overloads** (spec §11.3, §11.8).
  - An action or constructor may have several variadic overloads that differ in
    parameter types (`kind(values is ...int)`, `kind(values is ...string)`), next to
    fixed ones.
  - A variadic tail counts as one parameter of the arity, and `...int` differs from
    `int`.
  - A call that fits a fixed signature never selects a variadic one: next to
    `total(values is ...int)`, `total(a is int)` takes `total(1)`, while `total()`
    and `total(1, 2)` take the tail.
  - Constructors of one arity are selected by parameter types, like actions:
    `Duration(ms is int)` and `Duration(text is string)`.
  - A variadic constructor no longer has to be the only constructor:
    `Path(origin is string)` and `Path(parts is ...string)` can coexist.
  - A generic object infers its arguments from each constructor that fits the call.
  - Two variadic signatures with the same types are a declaration error, because names
    cannot tell them apart in a positional call. So are two constructors with identical
    signatures.
- Benchmarks compare Sparrow and the JVM with the same programs in Python, Perl and Ruby
  (`benchmarks/NN-*/main.{py,pl,rb}`, engines `python`, `perl`, `ruby` in
  `benchmarks/run.py`). The outputs of all engines must agree. `run.py` also records the
  install sizes of every engine.
- `benchmarks/export.py` turns a run into a release report: every run as CSV, statistics
  per benchmark and engine (mean, median, quartiles, min, max, standard deviation, CV, CPU,
  memory, instructions, IPC), speed ratios per engine and `REPORT.md`. Reports are kept
  in `benchmarks/reports/<date>-v<version>/`; the first one is for 0.3.0.

### Changed

- **Import lookup is local-first.** The order is now:
  1. the importing file's directory;
  2. its package root;
  3. `OKKAMA_IMPORT_DIRS`;
  4. the standard library.

  Previously `OKKAMA_IMPORT_DIRS` came first. The standard library resolves its own
  imports only inside itself, so a file such as `io/stream.ok` in a program or in
  `OKKAMA_IMPORT_DIRS` no longer breaks std.

- Overloads of one name and arity declare the same result also when one of them is
  variadic. An object's action that breaks a role signature shows a variadic tail as
  `...int` in the error (it was `@int`).
- Conformance: `L03.variadic.overloaded` and `L04.variadic-ctor.with-other-constructor`
  now expect success under the new overload rules, and DEF-002 is withdrawn.
- A program that would load two different files declaring the same module path (for
  example a local module shadowing std next to a `std.` path to it) is now rejected with a
  clear error instead of an internal compiler error.

### Removed

- **`fs/path`: `JoinedPath` is removed.** Its join is the variadic constructor
  `Path(parts is ...string)`: write `Path("etc", "okkama", "config.toml")` in place of
  `value JoinedPath("etc", "okkama", "config.toml")`. The rules are unchanged: the current
  platform's style, `InvalidPath` for an empty part list or an empty part, and a later
  absolute part replaces the parts before it. `Path(text)` and `Path(text, style)` keep
  their meaning.

### Performance

Results, measured by `benchmarks/run.py --runs 10` on an Apple M2 Max; all runs and
tables are in [benchmarks/reports/2026-10-31-v0.3.0](benchmarks/reports/2026-10-31-v0.3.0/README.md):

- Sparrow with the JIT is on average (geometric mean over benchmarks 01–10) 2.31× faster
  than Python 3.9, 2.34× faster than Perl 5.42, 1.72× faster than Ruby 2.6 and 1.81×
  faster than the JVM build of Okkama.
- Sparrow without the JIT went from 0.33× to 0.76× of Python.
- Biggest gains: `06-collections` is 4.0× faster with the JIT and 6.6× without it;
  `05-strings` is 2.4× and 4.3× faster; the interpreter is 3.9× faster on `01-arith`
  and 2.3× on `02-calls`.
- Still slower than the best of the other languages: `05-strings` (Perl is 1.9× faster),
  `06-collections` (Perl 1.8×) and `09-closures` (the JVM build 1.4×).
- Start-up of `00-hello` grew from 5.1 to 5.7 ms.

Changes:

- **Sparrow interpreter** (`OKKAMA_JIT=off`, and cold code under the JIT): constants,
  copies, small-integer arithmetic and comparisons, jumps, direct calls and returns run
  in the dispatch loop itself instead of the generic instruction step. Frame registers
  come from a pool instead of a new allocation per call. Branch moves of up to eight
  values snapshot on the stack, and plain values are copied and overwritten without
  `clone`/`drop` calls.
- **Loop constants are computed once.** A new OIR pass moves the `const` instructions of
  blocks that lie on a loop into the entry block of the function. Constants of
  branches outside loops stay in place. All engines run the same OIR, so error
  locations stay exact.
- **Sparrow JIT.**
  - Value equality of two small integers (generic `==` over `T = int`) is compiled
    inline.
  - A role call finds its target in a per-slot table indexed by the object
    declaration, without a helper call.
  - `builder.push` no longer copies the builder.
- **Freeing objects, arrays, maps and closures** queues only the last references to
  containers. Scalars and shared references are released in place, and an object
  without such fields frees no queue.
- **`collections/frequencies`** counts without rebuilding the frequency list for every
  input value (it was O(n·d) list rebuilds, now O(n·d) comparisons only).
- **`strings/parse::ParsedInt`** parses natively (`runtime/number_text`:
  `int_parse_valid`, `int_parse`) with the same accepted spellings and errors.
- Release builds use fat LTO with one codegen unit: `okkama` shrinks from 18.2 to
  14.3 MiB, `sparrow` from 12.3 to 10.2 MiB, and JIT code is faster.

### Fixed

- Editor: a name typed where the checked text had a different name with the same first
  letters (`out.` in place of `obj.action(…)`) was resolved as the old one, so completion
  showed the wrong members and signature help went missing.
- Editor: signatures of variadic actions show the tail as `values is ...string` (it was
  `@string`).

### Known limitations

- A `for` used as a value (`for xs -> x { last … if …; fallback; }` as the result of an
  action) over an empty array passes the checker but fails at run time with "A void
  result cannot be used as a value"; guard it with `if xs.size() == 0`.
- Calling a closure from JIT code still copies its captures; `09-closures` is slower on
  Sparrow than on the JVM build.

## [0.2.0] — 2026-10-30

### Added

- **Location and call stack of an uncaught failure** on the IR and Sparrow engines
  (JIT included): the source line with the failing span underlined, then the stack from
  the failure to `main`. Recursion collapses (`(×500)`); a long stack shows its start and
  end. `sparrow` and `okkama exec`, which have no sources, print the stack in Sparrow
  terms (function, instruction number and text). `OKKAMA_TRACE=off` prints the failure alone.
- **`debug/dumper`** — a dump of any value after Perl's Data::Printer:
  `Dumper(value).type()`, `.kind()`, `.size()`, `.fields()`, `.all()`, `.max_depth(n)`,
  `.max_items(n)`. Shows object fields (private ones too), array elements and map pairs;
  IR, Sparrow and JVM print the same text. New `runtime/debug` natives: `data_kind`,
  `data_type`, `data_size`, `data_leaf`, `data_names`, `data_keys`, `data_items`.
- **`okkama repl`** — interactive checks on the IR interpreter: statements, expressions
  (their value is printed with `Dumper`), declarations and imports. Variables live on
  between inputs; side effects of earlier inputs are not replayed. Commands: `:vars`,
  `:decls`, `:reset`, `:help`, `:quit`.
- Example `examples/debug-dumper.ok`.

### Changed

- Spec §6.7 describes `debug/dumper`, which, unlike `Dump`, shows object fields.
- An OIR module carries debug info (`DebugInfo`): the source of every function.
- `tools/engines/parity.py` compares engines with `OKKAMA_TRACE=off`.

### Known limitations

- The JVM engine still reports a failure without its location and stack.
- In the REPL, closures and futures are not kept between inputs, top-level `await` is not
  supported, and there is no line history.

## 0.1.0

First release of the Rust implementation: the compiler (lexer, parser, analysis, OIR), the
IR, Sparrow (VM with JIT) and JVM engines, the standard library, LSP, `fmt`, `lint`, the
`blade` project manager, and packages for macOS and Linux.

[Unreleased]: https://github.com/shootnix/okkama/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/shootnix/okkama/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/shootnix/okkama/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/shootnix/okkama/releases/tag/v0.2.0
