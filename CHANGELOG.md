# Changelog

All notable changes to Okkama. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow [Semantic Versioning](https://semver.org/). A release version lives in
`okkama-rs/Cargo.toml` and in the `vX.Y.Z` tag.

## [Unreleased]

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

### Fixed

- Editor: a name typed where the checked text had a different name with the same first
  letters (`out.` in place of `obj.action(…)`) was resolved as the old one, so completion
  showed the wrong members and signature help went missing.
- Editor: signatures of variadic actions show the tail as `values is ...string` (it was
  `@string`).

### Changed

- **Import lookup is local-first.** The order is now:
  1. the importing file's directory;
  2. its package root;
  3. `OKKAMA_IMPORT_DIRS`;
  4. the standard library.

  Previously `OKKAMA_IMPORT_DIRS` came first. The standard library resolves its own
  imports only inside itself, so a file such as `io/stream.ok` in a program or in
  `OKKAMA_IMPORT_DIRS` no longer breaks std.

- **`fs/path`: `JoinedPath` is removed.** Its join is the variadic constructor
  `Path(parts is ...string)`: write `Path("etc", "okkama", "config.toml")` in place of
  `value JoinedPath("etc", "okkama", "config.toml")`. The rules are unchanged: the current
  platform's style, `InvalidPath` for an empty part list or an empty part, and a later
  absolute part replaces the parts before it. `Path(text)` and `Path(text, style)` keep
  their meaning.
- Overloads of one name and arity declare the same result also when one of them is
  variadic. An object's action that breaks a role signature shows a variadic tail as
  `...int` in the error (it was `@int`).
- Conformance: `L03.variadic.overloaded` and `L04.variadic-ctor.with-other-constructor`
  now expect success under the new overload rules, and DEF-002 is withdrawn.
- A program that would load two different files declaring the same module path (for
  example a local module shadowing std next to a `std.` path to it) is now rejected with a
  clear error instead of an internal compiler error.

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

[Unreleased]: https://github.com/shootnix/okkama/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/shootnix/okkama/releases/tag/v0.2.0
