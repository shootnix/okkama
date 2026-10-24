<div align="center">

# 🧿 Okkama

**A statically typed object language where the Elegant Objects discipline
is enforced by the runtime — not by code review.**

[![Status](https://img.shields.io/badge/status-experimental-orange)](#project-status)
[![Spec](https://img.shields.io/badge/language%20spec-1.0-blue)](docs/okkama-spec-11072026.md)
[![Conformance](https://img.shields.io/badge/conformance-776%2F776-brightgreen)](conformance/)
[![Rust](https://img.shields.io/badge/Rust-%E2%89%A5%201.85-orange?logo=rust)](okkama-rs/)
[![License](https://img.shields.io/badge/license-GPL--3.0-blue)](LICENSE)

[The Okkama Book](docs/OkkamaBook.md) ·
[Книга на русском](docs/OkkamaBook-RU.md) ·
[Specification](docs/okkama-spec-11072026.md) ·
[Grammar](docs/okkama-grammar-1.0.md) ·
[LLM Context Pack](docs/okkama-llm-context-1.0.md)

</div>

### Three engines, one language

```sh
okkama hello.ok                                  # JVM (default)
okkama run --engine sparrow hello.ok             # Sparrow VM with JIT, in-process
okkama run --engine ir hello.ok                  # IR interpreter
okkama build hello.ok --target sparrow -o hello.spb && sparrow hello.spb
```

The compiler lowers Okkama to one IR (OIR-1); the JVM backend, the Sparrow
register VM (Cranelift JIT, cycle collector) and the IR interpreter execute it
with identical results — the [conformance suite](conformance/) and the engine
parity check hold them to that. Details: [okkama-rs](okkama-rs/README.md),
[Sparrow ISA](okkama-rs/docs/sparrow-isa.md),
[runtime, GC and JIT](okkama-rs/docs/sparrow-runtime-roadmap.md),
[benchmarks](benchmarks/README.md).

---

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

## What is Okkama?

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
- ❌ `while` loops, `return`, string interpolation, wildcard imports

## Getting started

The implementation, `okkama-rs`, is written in Rust. Building needs a Rust
toolchain (1.85+). The JVM engine needs a JDK to run programs (JDK 22 or newer
enables the FFM natives: cryptography, SQLite, terminal, accounts,
collation); the Sparrow and IR engines need nothing else.

### Installation (macOS and Linux)

```sh
git clone <this repository>
cd okkama
make install                     # into ~/.local; PREFIX=/opt/okkama make install
```

`make install` rebuilds the JVM runtime when its Java sources changed (and
`javac` is available), builds `okkama` and the host crypto library with cargo,
and installs:

| path | contents |
| --- | --- |
| `<prefix>/bin/okkama` | compiler, runner (JVM, Sparrow, IR), `fmt`, `lint`, language server; the JVM runtime is built in |
| `<prefix>/bin/sparrow` | standalone Sparrow VM for `.spb` bytecode |
| `<prefix>/lib/okkama/std/` | a copy of the standard library |
| `<prefix>/lib/okkama/native/<os>-<arch>/` | host cryptography (`okkama-native`) |
| `<prefix>/lib/okkama/icu-76.1/lib/` | ICU for `locale/collation`, when built |

`okkama` finds these next to itself, so no environment variables are needed;
`okkama env` shows what it found. `make uninstall` removes the installation.
Then:

```sh
okkama hello.ok                  # build for the JVM and run
okkama -t hello.ok               # run its tests
okkama build hello.ok --target jvm -o hello.jar && java -jar hello.jar
okkama fmt -w hello.ok           # format
okkama lint hello.ok             # errors, warnings, lint
```

Editor support: `make vscode` packages and installs the VS Code extension
(highlighting plus the `okkama lsp` language server).

### Development checkout

```sh
make build                       # okkama, sparrow, okkama-native (release)
make test                        # cargo test --workspace
cd conformance && ./ocs run --adapter adapters/rust/adapter.json --profile full-release -j 8
python3 benchmarks/run.py        # engine benchmarks (benchmarks/README.md)
```

`tools/build-icu-collation.sh` builds the pinned ICU 76.1 into `.runtime/`
for `locale/collation`; without it those natives report `unsupported`.

The entry point is `main() is int` or `async main() is int`, without parameters.
The CLI uses its result as the process exit status: 0 means success, 1..255
are application-defined errors. Other types and out-of-range values are errors.
The result is not printed; defer/destroy and shutdown run before process exit.

Your first program — `hello.ok`:

```okkama
module hello v1.0.0;

main() is int {
    let greeting = "Hello, " + "Okkama!";
    confirm greeting.length() == 14;
    return 0;              // process exit status: success
}
```

```console
$ okkama hello.ok
$ echo $?
0
```

## A taste of the language

### No null — absence is an explicit decision

```okkama
let prices is %string:int = ["apple": 120, "pear": 140];

prices["apple"];             // 120
prices["fig"];               // runtime error — missing keys never return null
prices["fig"] else 0;        // 0 — the fallback is your explicit decision
```

### Object capabilities (`cap`): computed consumer contracts

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

### Roles: contracts that ship their own test double

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

### Typed failures — errors are objects, bugs are not values

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

### Functional collections — eager, immutable, deterministic

```okkama
let total = [0, 2..=4, 9]                     // ranges expand in literals
    .filter() -> value { value.more(1); }
    .map()    -> value { value * 2; }
    .reduce(0) -> sum, value { sum + value; };
```

### Anonymous actions — object-shaped lexical computations

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

### Deterministic cleanup — scope owns resources

```okkama
{
    let file = File(path: "data.txt");
    defer report.flush();       // LIFO, on scope exit
}                               // file's destroy block runs HERE — not "eventually"
```

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

## Project status

Okkama is **experimental**: the language and standard library continue to
evolve. The Rust implementation runs the whole language and standard library
on three engines with identical results: the conformance suite passes
776/776 cases on each of JVM, Sparrow and the IR interpreter, and every
deterministic program of `std/` (in test mode) and `examples/` produces the
same output, errors and exit status on all of them.

See [Appendix B of the Book](docs/OkkamaBook.md#appendix-b-implementation-status)
for the language feature table.

## Repository layout

```text
okkama/
├── okkama-rs/           # Rust implementation: compiler, JVM backend, Sparrow VM + JIT,
│                        # IR interpreter, runtime and natives, LSP, fmt, lint
├── std/                 # standard library; module contracts in docs/std/
├── docs/                # book (EN/RU), spec, grammar, LLM context, OIR, guides
├── conformance/         # conformance suite (OCS) and its runner
├── examples/            # gallery, cookbook, web and async examples
├── benchmarks/          # engine benchmarks and their runner
├── tools/               # data generators for std (CLDR, IANA, ISO, ICU, tzdb)
├── vscode-okkama/       # VS Code extension (highlighting + language server)
├── Makefile             # build, install, test, vscode
└── install.sh
```

## Editor support

- **VS Code** — the [`vscode-okkama`](vscode-okkama/) extension provides
  syntax highlighting and the `okkama lsp` language server (diagnostics,
  completion, signatures, references, rename, formatting); `make vscode`
  packages and installs it.
- **Anything TextMate-compatible** — grammar in
  [`okkama.tmLanguage.json`](vscode-okkama/syntaxes/okkama.tmLanguage.json).

## Contributing

Issues, questions, and design discussions are welcome:

- 🐛 Report a bug in the issue tracker — a minimal `.ok` program plus
  expected/actual behavior is perfect.
- 💡 Open a discussion issue for language design questions — Okkama says
  "no" a lot, and every "no" has a written rationale in the
  [Book](docs/OkkamaBook.md).
- 🤝 Pull requests: keep changes small and focused and include a test
  (a Rust test or a conformance case). `make test` and the conformance suite
  must stay green on every engine.
- 🤖 Using an AI assistant? Feed it the
  [LLM Context Pack](docs/okkama-llm-context-1.0.md) first — it contains
  everything a model needs to write correct Okkama.

## License

[GPL-3.0](LICENSE) © Alexander Ponomarev
