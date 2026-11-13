<div align="center">

# 🧿 Okkama

**A statically typed object language where the Elegant Objects discipline
is enforced by the runtime — not by code review.**

[![CI](https://github.com/shootnix/okkama/actions/workflows/ci.yml/badge.svg?branch=develop)](https://github.com/shootnix/okkama/actions/workflows/ci.yml)
[![Packages](https://github.com/shootnix/okkama/actions/workflows/packages.yml/badge.svg?branch=develop)](https://github.com/shootnix/okkama/actions/workflows/packages.yml)
[![Release](https://img.shields.io/github/v/release/shootnix/okkama?include_prereleases&label=release)](https://github.com/shootnix/okkama/releases)
[![Conformance](https://img.shields.io/badge/conformance-1035%2F1035%20×%203%20engines-brightgreen)](conformance/)
[![License](https://img.shields.io/github/license/shootnix/okkama)](LICENSE)

[Install](#install) ·
[Quick start](#quick-start) ·
[The Book](docs/OkkamaBook.md) ·
[Книга](docs/OkkamaBook-RU.md) ·
[Guide for LLM agents](docs/LLM-GUIDE.md)

</div>

---

Okkama has no `null`, no inheritance, no setters and no hidden mutation —
and still reads like a modern expressive language. Absence is a typed
`Maybe<T>`, failures are enums you `match` on, choices are one-liners, and the
compiler checks that every case is handled.

```okkama
module shop v1.0.0;

import "io/stdout" take Stdout;

enum PaymentError is Error = (              // an error is just an enum with payloads
    declined(reason is string),
    timeout(ms is int)
);

object Gateway {
    Gateway();

    charge(cents is int) is string {
        confirm cents > 0 else throw PaymentError.declined("empty cart");
        "receipt #" + cents.string();
    }
}

main() is int {
    let out = Stdout();
    let gateway = Gateway();

    let receipt = try? gateway.charge(1500) else "no receipt";   // error → Maybe → fallback
    out.writeln(receipt);                                         // receipt #1500

    try {
        gateway.charge(0);
    }
    catch PaymentError.declined(reason) { out.writeln("declined: " + reason); }
    catch PaymentError.timeout(_) { out.writeln("try again later"); }

    out.writeln(receipt.length() > 10 ?? "paid" !! "unpaid");    // a ternary, typed
    0;
}
```


| ✨ | What you get |
|---|---|
| **Enums with payloads** | `enum Shape = (circle(radius is float), point);` and an exhaustive `match` |
| **`Maybe<T>` / `Presence<T>`** | explicit absence instead of `null`; `value else fallback`; *missing* ≠ *null* ≠ *value* |
| **Ternary and compact choice** | `ok ?? "yes" !! "no"` and `maybe ?? .some(v): v !! .none: 0` |
| **Errors are enums** | `enum DbError is Error = (...)`, generated `text()`, causes as payloads |
| **`try?` / `try!`** | Swift-style: an error becomes `Maybe.none`, or a panic when it "cannot happen" |
| **Catch by case** | `catch DbError.query(sql, code)`, `catch DbError.closed, NetError.reset`, `catch e is A, B` |
| **Immutable objects + successors** | state changes return a new object, and the compiler makes you use it |
| **[State views](#state-views--typestate-without-new-syntax)** | `Created` → `Authorized` → `Captured`: a wrong transition does not compile |

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
| Linux x86_64 (glibc 2.35+) | `okkama-<version>-linux-x86_64.deb`, `okkama-<version>-linux-x86_64.rpm` | `okkama-<version>-linux-x86_64.tar.gz` |

```sh
# Debian / Ubuntu
sudo apt install ./okkama-<version>-linux-x86_64.deb

# Fedora 36+, RHEL 10, openSUSE Tumbleweed (glibc 2.35+)
sudo dnf install ./okkama-<version>-linux-x86_64.rpm

# macOS: the .pkg installs into /usr/local; or, without root, from the archive:
tar -xzf okkama-<version>-macos-aarch64.tar.gz
./okkama-<version>-macos-aarch64/install.sh        # into ~/.local (--prefix DIR)
```

Every file has a `.sha256` next to it. The macOS packages are not signed yet:
open the `.pkg` with right-click → *Open*, and for an archive downloaded in a
browser run `xattr -dr com.apple.quarantine okkama-*` once.

**From source** (Rust 1.97+):

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
| Never use `null` | There is no `null` or `nil`. Absence is an explicit `Maybe<T>` / `Presence<T>` that you must unwrap. |
| Controlled field access | Fields are private through ordinary references. Caps expose explicit computed contracts; there is no setter syntax. |
| Objects are immutable | Fields bind during construction. After that — no mutation. |
| State change returns a new object | Successor actions (`is ^Type`) return a new object; the compiler rejects code that drops the result. |
| No inheritance | No `extends`, `isa`, `super`, `abstract`. Composition and roles instead. |
| Interfaces everywhere | Roles are nominal contracts, implemented explicitly with `does`. |
| Fake objects instead of mocks | Every role ships its own test-only `fake` — the reference test double. |
| Constructors bind and validate | Constructors bind fields and check invariants with `confirm ... else throw`. |
| Fail fast | `confirm` checks invariants; use `else throw` for typed errors, otherwise failure panics. |
| Objects manage resources | `destroy` runs deterministically when the owning scope exits — not when a GC feels like it. |

### What Okkama refuses to have

Okkama is defined as much by what it removes as by what it provides.
These are decisions, not gaps:

- ❌ `null` in any spelling — absence is a typed `Maybe<T>`, never a hidden value; no `?.` chaining
- ❌ inheritance — behavior is shared through roles and composition
- ❌ truthiness — every condition is a real `bool`
- ❌ implicit conversions — `1 == 1.0` is an *error*, not `false`
- ❌ getters, setters, public fields — objects expose behavior
- ❌ hidden state changes — mutations are visible in signatures (`is ^Type`) and at call sites
- ❌ standalone named/free functions — named behavior lives on objects;
  compact first-class computations use object-shaped anonymous actions
- ❌ catch-all `catch`, `finally`, untyped exceptions
- ❌ `while` loops, string interpolation, wildcard imports

## A taste of the language

### Enums with payloads — and the compiler counts the cases

```okkama
enum Shape = (
    circle(radius is float),
    rect(width is float, height is float),
    point
);

object Geometry {
    Geometry();

    area(shape is Shape) is float {
        match shape {
            case Shape.circle(radius) { 3.14159 * radius * radius; }
            case Shape.rect(width, height) { width * height; }
            case Shape.point { 0.0; }
        }                               // forget a case → "Non-exhaustive match: missing point"
    }
}
```

Variants are values: they compare by case and payload
(`Shape.rect(2.0, 3.0) == Shape.rect(2.0, 3.0)`), nest, and can be generic
(`enum Reading<T> = (empty, value(item is T));`).

### No null — absence is a type you have to open

```okkama
let name = user.nickname() else "anonymous";      // Maybe<string> → string, lazily

let prices is %string:int = ["apple": 120];
prices["fig"] else 0;                              // a missing key is never null

// PATCH semantics: "not sent", "sent as null" and "sent a value" are three states
patch.email()
    ?? .some(value): "set to " + value
    !! .none: "cleared"
    !! .missing: "unchanged";

let email = patch.email().fold() else "default";  // merge missing + none explicitly
```

`Maybe<T>` is `some | none`; `Presence<T>` adds `missing`. Nothing converts
implicitly: an empty string, `0` and `false` are present values, and
`Presence else …` is a compile error until you `fold()` it.

### Ternaries and compact choices

```okkama
let label = stock > 0 ?? "in stock" !! "sold out";          // bool → typed ternary

let text = cache.lookup(key)                                // enum → compact match
    ?? .some(hit): hit
    !! .none: "miss";

let shown = order.status()
    ?? .paid(amount): "paid " + amount.string()
    !! _: "not paid";                                        // `_` covers the rest
```

Both branches are lazy, the subject is evaluated once, and the branch types
must agree — no implicit common type, no truthiness.

### Errors are enums — catch by case, recover with `try?`

```okkama
enum DbError is Error = (
    connection(host is string),
    query(sql is string, code is int),
    closed
);
enum RepoError is Error = (db(cause is DbError), missing(id is int));

let user = try? repo.find(id) else "nobody";      // any thrown error → Maybe.none
let config = try! Config.parse(text);             // "cannot fail" → panic if it does

try {
    repo.find(id);
}
catch RepoError.missing(missing) { report(missing); }   // one case, payload bound
catch DbError.closed, DbError.connection(_) { reconnect(); }  // several cases
catch e is RepoError, DbError { log(e.text()); }   // several types: e is Error
```

`text()` is generated from the case: `RepoError.db(DbError.query("select", 42))`.
A thrown value of the right type but another case falls through to the next
`catch` of the same `try`. Panics are not errors and are never caught.

### Immutable objects and successors

```okkama
object Bottle {
    has opened is bool = false;
    Bottle();

    open() is ^Bottle {              // a successor: returns a NEW Bottle
        confirm !opened;             // fail fast — no partial results
        ^opened = true;              // stage the update; the original is untouched
    }

    is_open() is bool { opened; }
}

let opened = Bottle().open();        // `Bottle().open();` alone does not compile
confirm opened.is_open();
```

Successors are the only way to change state, and the compiler tracks their
results on every path: a dropped successor is a compile error. Combined with
roles they give [state views](#state-views--typestate-without-new-syntax).

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
Owners never convert implicitly to caps. See the working example
and Capabilities 0.0.1 guide.

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
See the role example and
[book section](docs/OkkamaBook-RU.md#221-доступ-через-роль).

</details>

<details>
<summary><b>Functional collections — eager, immutable, deterministic</b></summary>

```okkama
let total = [0, 2..=4, 9]                     // ranges expand in literals
    .filter() -> value { value.greater(1); }
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

See anonymous actions and action types for the full syntax.

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

## State views — typestate without new syntax

A protocol like *created → authorized → captured → refunded* is usually
guarded by `if (status == ...)` checks at runtime. In Okkama the existing
pieces — **roles**, **successors** and **enums** — compose into a typestate
pattern where an invalid transition simply does not type-check.

```okkama
// 1. One role per state: a state exposes only what is valid in it.
role Created    { id() is string; authorize(approve is bool) is Authorization; }
role Authorized { id() is string; capture() is ^Captured; }
role Captured   { id() is string; refund() is ^Refunded; }
role Refunded   { id() is string; }
role Declined   { id() is string; reason() is string; }

// 2. Alternative outcomes are an enum whose payloads are different views.
enum Authorization = (approved(payment is Authorized), declined(payment is Declined));

// 3. The real state: a private field of this enum, checked by every transition.
hidden enum Stage = (created, authorized, captured, refunded, declined(reason is string));
enum PaymentError is Error = (invalid_transition(action is string));

// 4. One immutable class implements every view — hidden: other modules cannot name it.
hidden object Payment does Created, Authorized, Captured, Refunded, Declined {
    has key is string;
    has stage is Stage = Stage.created;      // not in the constructor: unforgeable
    Payment(:key);

    capture() is ^Payment {                  // implements `capture() is ^Captured`
        confirm stage == Stage.authorized else throw PaymentError.invalid_transition("capture");
        ^stage = Stage.captured;
    }
    // ... authorize(), refund(), reason(), id()

    hidden approved() is ^Payment { ^stage = Stage.authorized; }     // internal transitions
    hidden rejected(reason is string) is ^Payment { ^stage = Stage.declined(reason); }
}

// 5. Callers only ever hold views.
match Payments().open("p-1").authorize(true) {
    case Authorization.approved(payment) { payment.capture().refund().id(); }
    case Authorization.declined(payment) { payment.reason(); }
}
```

The trick is a covariant successor: the role promises `capture() is ^Captured`,
the class implements it as `^Payment`, and a call through the role hands back a
**restricted `^Captured` view** of the new immutable object. Everything below
is rejected by `okkama check` — these are the compiler's actual messages:

| You write | The compiler says |
|---|---|
| `created.capture()` | `Role "Created" has no action "capture"` |
| `authorized.authorize(true)` | `Role "Authorized" has no action "authorize"` |
| `authorized.capture();` (result dropped) | `Successor action result must be used` |
| `let a is Authorized = created;` | `Cannot initialize variable "a" of type Authorized with Created` |
| `let p is Payment = created;` (downcast) | `Cannot initialize variable "p" of type Payment with Created` |
| `let back is Authorized = boxed;` (via `any`) | `Cannot initialize variable "back" of type Authorized with any` |
| `Payment(key: "x", stage: Stage.captured)` | `No constructor of "Payment" accepts (key: string, stage: Stage)` |
| `Payment(key: "x").approved()` | `Action "approved" on object "Payment" is hidden` |
| a `match` without the `declined` case | `Non-exhaustive match: missing declined` |
| `payment.refund()` in the `approved` branch | `Role "Authorized" has no action "refund"` |
| `payments.Payment(key: "x")` in another module | `Type "payments.Payment" is hidden in module "payments"` |
| `open() is Payment` in a public object | `Public action "open" of "Payments" exposes hidden type "Payment"` |

No annotations, no linear types, one contextual word (`hidden`): nominal roles
stay restricted through aliases, parameters, generics, `Maybe`, enum payloads
and `any`, and the class behind them cannot be named outside its module. What it
deliberately does not do: an old immutable state stays valid, so "authorize
twice" is a job for durable state and idempotency.

## Three engines, one language

The compiler lowers Okkama to one intermediate representation
(OIR-1) and runs it on any of three engines with
identical results — output, errors and exit status. The
[conformance suite](conformance/) and an engine parity check over the tests of
`std/` hold them to that.

```sh
okkama hello.ok                              # IR interpreter (default), in-process
okkama run --engine sparrow hello.ok         # Sparrow: whole-function Cranelift JIT
okkama run --engine jvm hello.ok             # JVM class files, run with `java`
okkama build hello.ok --target sparrow -o hello.spb && sparrow hello.spb
```

| | IR interpreter | Sparrow (JIT) | JVM |
|---|---|---|---|
| Start-up (`hello`) | 12 ms | **11 ms** | 53 ms |
| `fib(32)`, 7 M calls | 1.29 s | **30 ms** | 188 ms |
| Mandelbrot (float) | 0.55 s | **28 ms** | 147 ms |
| 1 M successor steps | 0.93 s | **25 ms** | 131 ms |
| 6-module application | 1.62 s | **96 ms** | 365 ms |
| Peak memory, application | 10 MB | 13 MB | 259 MB |
| Needs | nothing | nothing | JDK |

Median of ten cold runs on an Apple M2 Max (0.5.6, release build, start-up
included). Sparrow (its current version is called Tomato) compiles every function to
machine code (Cranelift) before it runs; calls are direct, async frames are
stackless and the collector is precise.

The same ten programs were also written in Go, Java, Kotlin and Python. Each
port prints exactly what the Okkama program prints. Geometric mean, Sparrow
against each (0.5.4):

| | cold run, start-up included | warmed up |
|---|---|---|
| Go 1.26 | 2.3× slower | 2.7× slower |
| Java 26 (HotSpot) | 1.43× faster | 2.9× slower |
| Kotlin 2.4 (HotSpot) | 1.72× faster | 2.7× slower |
| Python 3.9 | 5.9× faster | 8.6× faster |

Sparrow runs them in 8–25 MB, Go in 4–13 MB, Java in 44–209 MB and Kotlin in
47–433 MB.

## Documentation

| Document | What it is |
|---|---|
| 📖 [The Okkama Book](docs/OkkamaBook.md) | The textbook: philosophy, every language construct, from first principles |
| 📖 [Книга Okkama (RU)](docs/OkkamaBook-RU.md) | The book in Russian |
| 🐦 [The Sparrow Book](docs/SparrowBook/SparrowBook.md) | Sparrow assembly (`.spa`) and SPB: every instruction, the verifier, how Sparrow runs it |
| 🐦 [Книга Sparrow (RU)](docs/SparrowBook/SparrowBook-RU.md) | The Sparrow book in Russian |
| 🛡️ [Trust model](docs/security-model.md) | Which input is safe to compile, verify and run today, per boundary, with the open issues |
| 🤖 [Guide for LLM agents](docs/LLM-GUIDE.md) | Working on this repository: layout, build, tests, benchmarks, architecture |
| 🧪 [Conformance suite](conformance/README.md) | Executable specification: 1021 cases with expected observations |

Where documents disagree, the source of truth is the conformance suite
(`conformance/`) and the specification — in that order.

## Status

Okkama is **experimental**: the language and the standard library still
evolve, and nothing is stable yet. The implementation runs the whole language
and standard library: the conformance suite passes 1021/1021 cases on each
engine, and every deterministic program in `std/` (in test mode)
behaves identically on all three. See
[Appendix B of the Book](docs/OkkamaBook.md#appendix-b-implementation-status)
for the feature table.

## Development

```sh
make build                        # okkama, sparrow, blade, okkama-native (release)
make test                         # cargo test --workspace
make dist                         # packages for this machine into dist/
cd conformance && ./ocs run --adapter adapters/rust/adapter.json --profile full-release -j 8
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
├── okkama-rs/        Rust implementation: compiler, JVM backend, Sparrow (Tomato),
│                     IR interpreter, runtime and natives, LSP, fmt, lint, blade
├── std/              standard library
├── docs/             the Book (EN/RU) and the guide for LLM agents
├── conformance/      conformance suite (OCS) and its runner
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
  [guide for LLM agents](docs/LLM-GUIDE.md) first.

## License

[GPL-3.0](LICENSE) © Alexander Ponomarev
