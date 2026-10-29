<div align="center">

# 🧿 Okkama

**A statically typed object language where the Elegant Objects discipline
is enforced by the runtime — not by code review.**

[![CI](https://github.com/shootnix/okkama/actions/workflows/ci.yml/badge.svg?branch=develop)](https://github.com/shootnix/okkama/actions/workflows/ci.yml)
[![Packages](https://github.com/shootnix/okkama/actions/workflows/packages.yml/badge.svg?branch=develop)](https://github.com/shootnix/okkama/actions/workflows/packages.yml)
[![Release](https://img.shields.io/github/v/release/shootnix/okkama?include_prereleases&label=release)](https://github.com/shootnix/okkama/releases)
[![Conformance](https://img.shields.io/badge/conformance-776%2F776%20×%203%20engines-brightgreen)](conformance/)
[![License](https://img.shields.io/github/license/shootnix/okkama)](LICENSE)

[Install](#install) ·
[Quick start](#quick-start) ·
[The Book](docs/OkkamaBook.md) ·
[Книга](docs/OkkamaBook-RU.md) ·
[Specification](docs/okkama-spec-11072026.md) ·
[Grammar](docs/okkama-grammar-1.0.md) ·
[LLM Context Pack](docs/okkama-llm-context-1.0.md)

</div>

---

Okkama has no `null`, no inheritance, no setters and no hidden mutation.
Objects are immutable; a change of state is a *new* object returned by a
successor action, and the runtime refuses programs that throw it away.

```okkama
module app v1.0.0;

object Bottle {
    has opened is bool = false;

    Bottle() {}

    open() is ^Bottle {              // a successor action: returns a NEW Bottle
        confirm !opened;             // fail fast — no partial results
        ^opened = true;              // stage the update; the original is untouched
    }

    is_open() is bool {
        opened;                      // the last expression IS the result
    }
}

main() is int {
    let bottle = Bottle();
    let opened = bottle.open();      // discarding a successor is a runtime error
    confirm opened.is_open();       // bottle.is_open() is still false
    return 0;
}
```

## Install

Prebuilt packages for **macOS (Apple Silicon and Intel)** and **Linux x86_64**
are on the [Releases](https://github.com/shootnix/okkama/releases) page:
tagged versions and a rolling `nightly` built from `develop`. Each package
contains `okkama`, `sparrow`, `blade`, the standard library and the host
crypto library.

| Platform | Installer | Archive |
|---|---|---|
| macOS, Apple Silicon | `okkama-<version>-macos-aarch64.pkg` | `okkama-<version>-macos-aarch64.tar.gz` |
| macOS, Intel | `okkama-<version>-macos-x86_64.pkg` | `okkama-<version>-macos-x86_64.tar.gz` |
| Linux x86_64 (glibc 2.35+) | `okkama-<version>-linux-x86_64.deb` | `okkama-<version>-linux-x86_64.tar.gz` |

```sh
# Debian / Ubuntu
sudo apt install ./okkama-<version>-linux-x86_64.deb

# macOS: the .pkg installs into /usr/local; or, without root, from the archive:
tar -xzf okkama-<version>-macos-aarch64.tar.gz
./okkama-<version>-macos-aarch64/install.sh        # into ~/.local (--prefix DIR)
```

Every file has a `.sha256` next to it. The macOS packages are not signed yet:
open the `.pkg` with right-click → *Open*, and for an archive downloaded in a
browser run `xattr -dr com.apple.quarantine okkama-*` once.

**From source** (Rust 1.85+):

```sh
git clone https://github.com/shootnix/okkama.git && cd okkama
make install                      # into ~/.local; PREFIX=/opt/okkama make install
```

`okkama` finds the standard library and native libraries next to itself, so no
environment variables are needed — `okkama env` shows what it found. The JVM
engine additionally needs a JDK (22+ for the FFM natives: cryptography,
SQLite, terminal, accounts, collation); the IR and Sparrow engines need nothing.

## Quick start

```sh
blade new hello && cd hello       # project skeleton: src/main.ok
okkama src/main.ok                # run it
```

`main() is int` is the entry point; its result is the process exit status.

```okkama
module hello v1.0.0;

import "io/stdout" take Stdout;

main() is int {
    Stdout().writeln("Hello, Okkama!");
    0;
}
```

```sh
okkama hello.ok                   # run (IR interpreter)
okkama -t hello.ok                # run the file's `test` blocks
okkama fmt -w hello.ok            # format in place
okkama lint hello.ok              # errors, warnings, lint findings
okkama build hello.ok --target jvm -o hello.jar && java -jar hello.jar
```

## Why Okkama

Okkama is built around four ideas:

```text
Objects expose behavior, not state.
Object state is declared with has.
Objects are immutable after construction.
State changes are represented by successor objects.
```

If you have read *Elegant Objects* by Yegor Bugayenko, this will feel
familiar. In Java or C# that discipline must be enforced by convention and
code review. Okkama turns it into **language rules that the runtime checks
for you** — the "right" object-oriented style is not a matter of taste; it is
the only style the language accepts.

| Elegant Objects principle | Okkama language rule |
|---|---|
| Never use `null` | There is no `null`, `none`, or `nil`. The words are parse errors. |
| Controlled field access | Fields are private through ordinary references. Caps expose explicit computed contracts; there is no setter syntax. |
| Objects are immutable | Fields bind during construction. After that — no mutation. |
| State change returns a new object | Successor actions (`is ^Type`) return a new object; the result **must** be used. |
| No inheritance | No `extends`, `isa`, `super`, `abstract`. Composition and roles instead. |
| Interfaces everywhere | Roles are nominal contracts, implemented explicitly with `does`. |
| Fake objects instead of mocks | Every role ships its own test-only `fake` — the reference test double. |
| Constructors bind and validate | Constructors bind fields and check invariants with `confirm ... else throw`. |
| Fail fast | `confirm` checks invariants; use `else throw` for typed errors, otherwise failure panics. |
| Objects manage resources | `destroy` runs deterministically when the owning scope exits — not when a GC feels like it. |

### What Okkama refuses to have

Okkama is defined as much by what it removes as by what it provides.
These are decisions, not gaps:

- ❌ `null` in any spelling — no optionals, no `?.` chaining
- ❌ inheritance — behavior is shared through roles and composition
- ❌ truthiness — every condition is a real `bool`
- ❌ implicit conversions — `1 == 1.0` is an *error*, not `false`
- ❌ getters, setters, public fields — objects expose behavior
- ❌ hidden state changes — mutations are visible in signatures (`is ^Type`) and at call sites
- ❌ standalone named/free functions — named behavior lives on objects;
  compact first-class computations use object-shaped anonymous actions
- ❌ `while` loops, string interpolation, wildcard imports

## A taste of the language

### No null — absence is an explicit decision

```okkama
let prices is %string:int = ["apple": 120, "pear": 140];

prices["apple"];             // 120
prices["fig"];               // runtime error — missing keys never return null
prices["fig"] else 0;        // 0 — the fallback is your explicit decision
```

<details>
<summary><b>Object capabilities (`cap`): computed consumer contracts</b></summary>

```okkama
object Customer {
    has given_name, family_name is string;
    Customer(:given_name, :family_name);
    cap BusinessCard {
        full_name is string { given_name + " " + family_name; }
    }
}
// Select explicitly, then read a computed field:
let card is Customer::BusinessCard = customer::BusinessCard;
card::full_name;
```

The same contract can be declared at module scope as
`cap Customer::BusinessCard { full_name is string { Customer::given_name + " " + Customer::family_name; } }`.
External caps require a local owner type and qualify private-field reads with
that type. Both declaration forms share the same nominal namespace.

Each read recomputes the field against the original immutable owner. Caps
retain the owner safely and expose only their declared computed fields.
Owners never convert implicitly to caps. See the [working example](examples/object-caps.ok)
and [Capabilities 0.0.1 guide](docs/object-caps-0.0.1-ru.md).

</details>

<details>
<summary><b>Roles: contracts that ship their own test double</b></summary>

```okkama
role Package {
    open() is ^Package;
    is_open() is bool;

    fake {                              // every contract carries its reference fake
        has opened is bool = false;
        open() is ^Package { ^opened = true; }
        is_open() is bool { opened; }
    }
}

object Bottle does Package { /* ... */ }   // conformance checked at registration

test "a package can be opened" {
    let package is Package = Package.fake();   // test mode only — no mock libraries
    test.ok(package.open().is_open());
}
```

Passing an object as a role restricts that reference to the role's declared
actions and overloads. `let p is Package = Bottle();` cannot call an extra
`Bottle` action or read its fields. Implementations still run on the original
object, with full access to their own state. The restriction survives aliases,
returns, collections and bound actions; it does not copy the object.
See the [role example](examples/role-boundaries.ok) and
[book section](docs/OkkamaBook-RU.md#221-доступ-через-роль).

</details>

<details>
<summary><b>Typed failures — errors are objects, bugs are not values</b></summary>

```okkama
object InvalidCents does Error {
    has cents is int;
    InvalidCents(:cents);
    cents_value() is int { cents; }
    text() is string { "invalid cents"; }
}

object Cash {
    has digit is int;
    has cents is int;
    Cash(:digit, :cents) {
        confirm cents >= 0 && cents < 100 else throw InvalidCents(cents: cents);
    }
}

try {
    let cash = Cash(digit: 1, cents: 250);
}
catch error is InvalidCents {
    error.cents_value();      // 250 — typed payload, no string parsing
}
```

</details>

<details>
<summary><b>Functional collections — eager, immutable, deterministic</b></summary>

```okkama
let total = [0, 2..=4, 9]                     // ranges expand in literals
    .filter() -> value { value.more(1); }
    .map()    -> value { value * 2; }
    .reduce(0) -> sum, value { sum + value; };
```

</details>

<details>
<summary><b>Anonymous actions — object-shaped lexical computations</b></summary>

```okkama
object Apply {
    has input is int;
    Apply(:input);

    value(operation is (int) is int) is int {
        operation(input);
    }
}

let double is (int) is int = (value is int) is int { value * 2; };

Apply(input: 10).value(double);                    // 20
Apply(input: 21).value((value is int) is int { value * 2; }); // 42
```

Anonymous actions are immutable values with lexical capture, modeled as
anonymous callable objects rather than a second global function system.

See [anonymous actions and action types](docs/anonymous-actions.md) for the full syntax.

</details>

<details>
<summary><b>Deterministic cleanup — scope owns resources</b></summary>

```okkama
{
    let file = File(path: "data.txt");
    defer report.flush();       // LIFO, on scope exit
}                               // file's destroy block runs HERE — not "eventually"
```

</details>

## Three engines, one language

The compiler lowers Okkama to one intermediate representation
([OIR-1](docs/compiler/OIR-1.md)) and runs it on any of three engines with
identical results — output, errors and exit status. The
[conformance suite](conformance/) and an engine parity check over all of
`std/` and `examples/` hold them to that.

```sh
okkama hello.ok                              # IR interpreter (default), in-process
okkama run --engine sparrow hello.ok         # Sparrow register VM + Cranelift JIT
okkama run --engine jvm hello.ok             # JVM class files, run with `java`
okkama build hello.ok --target sparrow -o hello.spb && sparrow hello.spb
```

| | IR interpreter | Sparrow + JIT | JVM |
|---|---|---|---|
| Start-up (`hello`) | 6 ms | **5 ms** | 43 ms |
| `fib(32)`, 7 M calls | 1.69 s | **0.16 s** | 0.18 s |
| Mandelbrot (float) | 0.73 s | **0.05 s** | 0.14 s |
| 1 M successor steps | 1.28 s | **0.11 s** | 0.13 s |
| 6-module application | 2.05 s | **0.41 s** | 0.52 s |
| Peak memory, application | 8 MB | 15 MB | 277 MB |
| Needs | nothing | nothing | JDK |

Median of five runs on an Apple M2 Max; the full ladder of eleven programs,
CPU time and memory are in [benchmarks](benchmarks/README.md). Sparrow is a
self-contained VM ([ISA](okkama-rs/docs/sparrow-isa.md)) with a baseline JIT,
in-place successor updates and a cycle collector
([runtime, GC and JIT](okkama-rs/docs/sparrow-runtime-roadmap.md)).

## Documentation

| Document | What it is |
|---|---|
| 📖 [The Okkama Book](docs/OkkamaBook.md) | The textbook: philosophy, every language construct, from first principles |
| 📖 [Книга Okkama (RU)](docs/OkkamaBook-RU.md) | The book in Russian |
| 📜 [Specification](docs/okkama-spec-11072026.md) | Normative language specification |
| 🧩 [Grammar (EBNF)](docs/okkama-grammar-1.0.md) | Parser construction target |
| 🤖 [LLM Context Pack](docs/okkama-llm-context-1.0.md) | Self-contained context for AI agents generating Okkama code |
| 🔌 [Native Functions Tutorial](docs/native-functions-tutorial.md) | How the language talks to the host OS |
| 🗺️ [Stdlib roadmap](docs/std/stdlib-roadmap-v4.md) | Current standard-library plan and module contracts |
| 🧪 [Conformance suite](conformance/README.md) | Executable specification: 776 cases with expected observations |
| ⚙️ [OIR-1](docs/compiler/OIR-1.md) | The compiler's intermediate representation |

Where documents disagree, the source of truth is the conformance suite
(`conformance/`) and the specification — in that order.

## Status

Okkama is **experimental**: the language and the standard library still
evolve, and nothing is stable yet. The implementation runs the whole language
and standard library: the conformance suite passes 776/776 cases on each
engine, and every deterministic program in `std/` (in test mode) and
`examples/` behaves identically on all three. See
[Appendix B of the Book](docs/OkkamaBook.md#appendix-b-implementation-status)
for the feature table.

## Development

```sh
make build                        # okkama, sparrow, blade, okkama-native (release)
make test                         # cargo test --workspace
make dist                         # packages for this machine into dist/
cd conformance && ./ocs run --adapter adapters/rust/adapter.json --profile full-release -j 8
python3 benchmarks/run.py         # engine benchmarks
```

`tools/build-icu-collation.sh` builds the pinned ICU 76.1 into `.runtime/`
for `locale/collation`; without it those natives report `unsupported`.

GitHub Actions run the tests on Linux and macOS and the conformance suite on
every engine for each push and pull request ([CI](.github/workflows/ci.yml)).
[Packages](.github/workflows/packages.yml) are built for every platform on
each push to `develop` (the `nightly` pre-release) and published as a release
for each `vX.Y.Z` tag, which must match the version in `okkama-rs/Cargo.toml`.

```text
okkama/
├── okkama-rs/        Rust implementation: compiler, JVM backend, Sparrow VM + JIT,
│                     IR interpreter, runtime and natives, LSP, fmt, lint, blade
├── std/              standard library (module contracts in docs/std/)
├── docs/             book (EN/RU), specification, grammar, OIR, guides
├── conformance/      conformance suite (OCS) and its runner
├── examples/         gallery, cookbook, web and async programs
├── benchmarks/       engine benchmarks and their runner
├── tools/            packaging and data generators for std (CLDR, IANA, ISO, ICU, tzdb)
└── vscode-okkama/    VS Code extension: highlighting + language server
```

## Editor support

The [`vscode-okkama`](vscode-okkama/) extension provides highlighting and the
`okkama lsp` language server — diagnostics, completion, signatures,
references, rename and formatting. `make vscode` packages and installs it. Any
TextMate-compatible editor can use
[`okkama.tmLanguage.json`](vscode-okkama/syntaxes/okkama.tmLanguage.json).

## Contributing

- 🐛 Bugs: a minimal `.ok` program with expected and actual behavior is perfect.
- 💡 Design questions are welcome — Okkama says "no" a lot, and every "no" has a
  written rationale in the [Book](docs/OkkamaBook.md).
- 🤝 Pull requests: small and focused, with a test (a Rust test or a
  conformance case). CI must stay green on every engine.
- 🤖 Using an AI assistant? Give it the
  [LLM Context Pack](docs/okkama-llm-context-1.0.md) first.

## License

[GPL-3.0](LICENSE) © Alexander Ponomarev
