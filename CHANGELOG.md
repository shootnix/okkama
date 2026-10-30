# Changelog

All notable changes to Okkama. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow [Semantic Versioning](https://semver.org/). A release version lives in
`okkama-rs/Cargo.toml` and in the `vX.Y.Z` tag.

## [Unreleased]

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
