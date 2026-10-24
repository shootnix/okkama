# Okkama stdlib API — development draft

Document version: **0.4.76-dev**<br>
Date: **22 July 2026**<br>
Status: **working draft, not a compatibility promise**  
Target: **Okkama 1.x**

## 1. What this document is

This is the workspace for the future bundled Okkama standard-library API. It
records three concrete things:

1. the stdlib file and module structure;
2. public roles, objects and actions;
3. complete Okkama usage examples.

The stdlib itself is the language core library. There is no second `core/`
namespace and no compulsory abstraction layer containing generic `Result` or
`Option` objects. The small opt-in `Iterable<T>` / `Cursor<T>` protocol exists
only to let stdlib collections participate in the language's ordinary `for`.

The current contents of `stdlib/` are an implementation playground, not the
source of truth for this design. Existing paths and object names may be kept,
renamed or removed after the target API is accepted. This document must not
justify a design merely because a similarly named draft file already exists.

The goal is a batteries-included library in the practical sense of Go or
Python: a fresh Okkama installation must be sufficient for files, processes,
environment, sockets, HTTP, data formats, databases, cryptography, testing and
ordinary application development.

## 2. API rules

### 2.1 Elegant Objects

- Objects are behaviour, not public data structures.
- Object names are nouns. An action that returns a value names that value or
  property (`bytes`, `line`, `written`, `closed`); a command that changes the
  world uses a verb (`write`, `flush`, `close`) and does not return a service
  counter.
- No mechanical getters or setters.
- Immutable copy-setters are still setters. Public action names `with` and
  `with_*` are forbidden, and the same fluent-builder design must not be hidden
  behind synonyms or boolean toggles.
- Objects are immutable unless their identity is inherently a live resource
  such as a file, socket, transaction or process command.
- Constructors only bind retained state. Computation, normalization and
  externally observable validation belong to actions.
- Roles describe capabilities and ship with usable fakes where external state
  is involved.
- Composition is preferred to universal base objects and utility namespaces.
- Configuration is composed from typed state objects. Behavioral variants use
  decorators; conceptual state transitions use successor actions. Persistent
  collections may return their own type only through domain-result names such
  as `prepended`, `enqueued`, or `union`.
- Domain errors are objects. Raw host exceptions, errno and Perl values never
  form the public API.
- `runtime/*` is allowed to remain function-shaped because it is the narrow
  typed host ABI. The ordinary stdlib builds object APIs above it.

### 2.2 Naming and granularity

- Do not create a directory merely to classify one file.
- Small fundamental APIs may use direct modules (`math`, `errors`, `random`).
  Real families use directories (`string/*`, `env/*`, `template/*`).
- A directory is justified by an actual family of independently useful
  modules: `fs/*`, `http/*`, `db/*`, `socket/*`.
- Do not split mathematics by Okkama primitive type. `int` and `float` are
  language types, not separate library architectures.
- Do not invent a `text` concept. Okkama has `string`, `char` and `@byte`.
- High-level modules may depend on lower-level modules, never the reverse.

### 2.3 Status labels

- **runtime** — implemented native ABI;
- **stdlib** — implemented in bundled Okkama;
- **proposed** — accepted namespace, API still being designed;
- **questionable** — retained for investigation, may be removed;
- **exploratory** — no accepted API yet.

## 3. Target file structure

This is a namespace plan, not a claim that every file already exists.

```text
stdlib/
├── math.ok
├── money.ok
├── errors.ok
├── random.ok
├── os.ok
├── regex.ok
├── format.ok
│
├── string/
│   ├── parse.ok
│   └── trim.ok
│
├── template/
│   ├── plain.ok
│   └── html.ok
│
├── env/
│   ├── var.ok
│   └── cwd.ok
│
├── collections/                 # persistent structures proven beyond builtins
│   ├── iteration.ok
│   ├── list.ok
│   ├── set.ok
│   ├── queue.ok
│   └── stack.ok
│
├── io/
│   ├── closeable.ok
│   ├── input.ok
│   ├── output.ok
│   ├── buffer.ok
│   ├── stdin.ok
│   ├── stdout.ok
│   └── stderr.ok
│
├── fs/
│   ├── path.ok
│   ├── file.ok
│   ├── dir.ok
│   ├── meta.ok
│   └── temp.ok
│
├── proc/
│   ├── cmd.ok
│   └── args.ok
│
├── time/
│   ├── duration.ok
│   ├── instant.ok
│   ├── date.ok
│   ├── clock.ok
│   └── timer.ok
│
├── net/
│   ├── address.ok
│   ├── dns.ok
│   └── url.ok
│
├── socket/
│   ├── socket.ok
│   ├── tcp.ok
│   ├── udp.ok
│   └── unix.ok
│
├── http/
│   ├── request.ok
│   ├── response.ok
│   ├── client.ok
│   ├── server.ok
│   ├── router.ok
│   ├── handler.ok
│   ├── headers.ok
│   └── status.ok
│
├── encoding/
│   ├── utf8.ok
│   ├── binary.ok
│   ├── base64.ok
│   ├── hex.ok
│   ├── json.ok
│   ├── csv.ok
│   └── toml.ok
│
├── crypto/
│   ├── random.ok
│   ├── digest.ok
│   ├── hmac.ok
│   └── tls.ok
│
├── sql/
│   ├── statement.ok
│   ├── query.ok
│   ├── value.ok
│   └── dialect.ok
│
├── db/
│   ├── driver.ok
│   ├── database.ok
│   ├── connection.ok
│   ├── transaction.ok
│   ├── row.ok
│   ├── result.ok
│   ├── sqlite.ok
│   ├── postgres.ok
│   ├── mysql.ok
│   └── migration.ok
│
├── log/
│   ├── logger.ok
│   ├── event.ok
│   └── formatter.ok
│
├── test/
│   ├── more.ok
│   ├── suite.ok
│   ├── fake_clock.ok
│   └── benchmark.ok
│
└── runtime/
    ├── environment.ok
    ├── process.ok
    ├── fs.ok
    ├── stdio.ok
    ├── time.ok
    ├── net.ok
    ├── poll.ok
    ├── regex.ok
    ├── utf8.ok
    ├── crypto.ok
    ├── tls.ok
    ├── format.ok
    ├── system.ok
    └── debug.ok
```

Decisions embodied by the tree:

- `env/*` is independent of `proc`; environment variables and cwd are ambient
  process state, not filesystem objects;
- `os` exposes normalized host identity and selects host path semantics;
- `proc` has commands and arguments, not `proc/process`;
- HTTP is not nested under `net`;
- sockets have an explicit public family rather than being hidden in `net`;
- `cwd` is an explicit `env/cwd` capability;
- `sql` describes SQL; `db` owns a DBI-style driver contract, connections,
  execution and transactions;
- database protocols are Okkama code over general byte, socket, TLS and
  filesystem primitives; `runtime` has no database-aware API;
- there is one general `math` module, not one module per numeric type;
- no `core`, `text` or `concurrent` namespace.

## 4. Module index

| Module | Status | Public purpose |
|---|---|---|
| `string/*` | proposed | Parsing and policy-bearing transformations above builtin `string` behavior |
| `math` | proposed | General numeric algorithms and mathematical objects |
| `money` | stdlib | Currency-bearing integer minor units, exact rational rates and explicit rounding |
| `errors` | proposed | Standard error roles and reusable domain failures |
| `random` | stdlib | Secure ranges/fractions plus injectable deterministic choices and permutations |
| `os` | stdlib | Normalized OS, architecture and host path conventions |
| `env/var` | stdlib | Typed text/raw-byte access to the ambient environment with an isolated role fake |
| `env/cwd` | stdlib | Explicit ambient current-working-directory capability with an immutable `FixedCwd` test double |
| remaining `env/*` | proposed | Later ambient capabilities |
| `template/plain` | stdlib | Pure named string templates with ERB/Mojolicious-style lexical tags |
| `template/html` | stdlib | HTML rendering that escapes by default; `SafeHtml` is the only bypass |
| remaining `template/*` | proposed | Later structural template rendering |
| `regex` | stdlib | Portable regular expressions, matches and literal replacement |
| `format` | stdlib | Positional/named value formatting with typed source errors |
| `collections/iteration`, `collections/list`, `collections/queue`, `collections/stack` | stdlib | Immutable traversal and persistent sequence structures |
| remaining `collections/*` | questionable | Only structures proven useful beyond builtin arrays/maps |
| `io/input`, `io/output`, `io/closeable`, `io/buffer`, `io/stdin`, `io/stdout`, `io/stderr` | stdlib | I/O roles, owned memory buffers, buffering decorators and standard streams |
| remaining `io/*` | proposed | Files, pipes and later concrete transports |
| `fs/path` | stdlib | Host-selected and explicit POSIX/Windows lexical path values |
| `fs/meta` | stdlib | Portable `FileKind`/`Permissions` metadata read through `lstat` |
| `fs/file` | stdlib | Whole-file and opened-file byte/string behaviour |
| `fs/dir` | stdlib | Directory creation, listing, walking and removal |
| `fs/temp` | stdlib | Uniquely named temporary files/directories with explicit ownership and `keep()` transfer |
| `proc/args` | stdlib | Immutable raw-byte and strict UTF-8 application argument snapshots |
| `proc/cmd` | stdlib | Child command construction, execution, and pipe-based I/O supervision (POSIX only) |
| `time/duration`, `time/instant`, `time/clock`, `time/date`, `time/timer` | stdlib | Exact spans, clock-bound values, UTC civil time and owned live timers |
| `time/tzdb` | proposed | Bundled versioned IANA rules plus explicit, testable host-zone resolution |
| `net/address` | stdlib | Validated immutable IP, socket endpoint and unresolved host/port values |
| `net/url`, `net/dns` | stdlib | RFC-style network URLs and injectable forward/reverse name resolution |
| remaining `net/*` | proposed | Network values only; sockets and HTTP remain separate namespaces |
| `socket/socket` | stdlib | Shared socket role, explicit option enums, and timeout vocabulary |
| `socket/tcp` | stdlib | TCP connections and listeners as stream I/O objects |
| `socket/udp` | stdlib | UDP datagram sending and receiving |
| `socket/unix` | stdlib | Local Unix-domain stream sockets with explicit unsupported-host failure |
| `http/status` | stdlib | Validated HTTP status codes and status-class behaviour |
| `http/headers` | stdlib | Immutable case-insensitive HTTP header collections |
| `http/request` | stdlib | Immutable HTTP request method, URL, headers and body |
| `http/response` | stdlib | Streamed HTTP responses and explicit body materialization |
| `http/handler` | stdlib | Role-based request handling with chain-of-responsibility composition |
| `http/router` | stdlib | Method/path dispatch with named path parameters |
| `http/client` | stdlib | Hand-rolled HTTP/1.1 client over TCP/TLS with redirect following |
| remaining `http/*` | proposed | HTTP client, server, router and protocol vocabulary |
| `encoding/utf8`, `encoding/binary`, `encoding/base64`, `encoding/hex`, `encoding/json`, `encoding/csv`, `encoding/toml` | stdlib | Strict text/byte conversion, canonical immutable binary encodings, exact JSON values, dialect-driven streaming CSV and a practical TOML subset |
| `crypto/random`, `crypto/digest`, `crypto/hmac` | stdlib | Injectable stateless OS-CSPRNG byte capability, reviewed SHA-2 digests over bytes/streams, and constant-time-verified HMAC |
| `crypto/tls` | stdlib | TLS policy, handshakes, encrypted sockets and peer certificates |
| `sql/value` | stdlib | Exact bindable SQL scalar wrappers and an explicit SQL null |
| `sql/dialect` | stdlib | Reviewed differences in placeholders, identifiers and SQL rendering |
| `sql/statement` | stdlib | Immutable data-changing SQL statements with bound parameters |
| `sql/query` | stdlib | Immutable row-producing SQL queries with bound parameters |
| `db/result` | stdlib | Non-row database execution outcome (affected rows, optional last insert id) |
| `db/row` | stdlib | Typed access to one database result row |
| `db/transaction` | stdlib | The `Transaction` role and `InactiveTransaction`; concrete implementers are database drivers, still ahead |
| `db/connection` | stdlib | The `Connection` role and `ConnectionClosed`/`DbFailure`; concrete implementers are database drivers, still ahead |
| `db/driver` | stdlib | The `Driver` role and `DriverFailure`; concrete implementers are database drivers, still ahead |
| `db/database` | stdlib | Stable application-facing `Database` facade composed explicitly with a `Driver`; `DatabaseUnavailable` |
| `db/sqlite`, `db/postgres`, `db/mysql` | deferred | Real embedded SQLite file-format engine and Postgres/MySQL wire-protocol drivers — each is a multi-week undertaking on its own (B-tree/WAL/locking, or full wire-protocol handshakes, plus a real SQL parser/executor for arbitrary text passed through `Statement.sql()`/`Query.sql()`); explicitly deferred rather than attempted at reduced fidelity, see `db-sqlite-22072026.md` |
| `db/migration` | stdlib | Ordered transactional schema changes and migration history, against any `Connection` implementer |
| remaining `db/*` | exploratory | Reserved for a future pass at `db/sqlite`/`db/postgres`/`db/mysql` |
| `log/event` | stdlib | Immutable structured log events and severity levels |
| `log/formatter` | stdlib | Deterministic line or JSON rendering of log events |
| `log/logger` | stdlib | Injectable log destinations: output, file, silent, context-enriching, in-memory |
| remaining `log/*` | proposed | Structured logging destinations |
| `test/more` | stdlib | Reusable assertions over the contextual language test capability |
| `test/fake_clock` | stdlib | Deterministic, explicitly advanceable/settable `Clock` implementation for tests |
| `test/benchmark` | stdlib | Injectable-clock repeatable duration measurement over a `BenchmarkCase` |
| `test/suite` | stdlib | `TestCase`/`Fixture` roles, `Suite` grouping and running them, `SuiteResult` aggregate outcome |
| `runtime/*` | runtime | Typed ABI to host providers |

## 5. Initial APIs

The signatures below are the starting point for review. `proposed` means the
names and protocols can still change.

### 5.1 `string/*`

Status: **proposed**

There is no `Text` object. This module works with the language's existing
`string`, `char` and `@byte` types.

Fundamental literal transformations belong to builtin immutable receivers:

- `string.split(separator is string) is @string`;
- `@string.join(separator is string) is string`;
- `string.replace(target is string, replacement is string) is string`;
- `string.replace_first(target is string, replacement is string) is string`.

The former `Split`, `Joined`, `Replaced`, `ReplacedFirst`, `Removed` and
`InvalidSeparator` candidates are removed. They were one-shot wrappers around
primitive value behavior and introduced neither reusable policy nor domain
identity. Removal is expressed as `source.replace(target, "")`.

Policy-bearing operations such as regex replacement, CSV tokenization,
template rendering and locale-aware matching remain ordinary stdlib objects.
`Trimmed` remains a separate candidate pending the final whitespace-policy
review.

Example direction:

```okkama
module app v1.0.0;

main() is string {
    let words is @string = "  red,green,blue  ".trim().split(",");
    words.join(" | ");
}
```

### 5.2 `math`

Status: **proposed**

One module contains general numeric behaviour without inventing a `number`
supertype. Basic exact operations belong to builtin numeric receivers:

- `int.abs() -> int`, `int.sign() -> int`, `int.min(int) -> int`,
  `int.max(int) -> int`, `int.clamp(int, int) -> int`;
- the same actions on `float`, preserving `float` except `sign() -> int`;
- mixed `int`/`float` arguments are rejected rather than widened.

Candidate objects for operations with a fixed result category:

- `Floor`, `Ceil`, `Round` — `float -> int`;
- `Power`, `SquareRoot`, `Logarithm` — explicit `float -> float`;
- `Statistics`, only if a cohesive protocol emerges from real use cases.

```okkama
module app v1.0.0;

import "math" take Round;

main() is int {
    let bounded is int = 120.clamp(0, 100);
    let rounded is int = value Round(12.6);
    bounded + rounded;
}
```

### 5.3 `money`

Status: **stdlib** (`stdlib/money.ok`)

`Currency(code, minor_factor)` defines the identity and subdivision of a
currency. `Money(minor, currency)` stores only signed 64-bit integer minor
units. All posted values implement `Monetary`. Arithmetic is represented by
composable immutable objects: `MoneySum`, `MoneyDifference`, and
`MoneyNegated`. Sum and difference throw `CurrencyMismatch` when currencies
differ and reject signed 64-bit overflow.

`MoneyScale` and `MoneyDivide` produce exact `rat` values through
`MonetaryCalculation`; they do not select a rounding rule. Posting such a
result requires `RoundedMoney` with a stateful `HalfEven`, `HalfUp`, or
`TowardZero` object. `MoneyAllocation(money, parts).shares()` preserves the
original total and distributes remaining minor units from the first share
onward.

```okkama
module app v1.0.0;

import "money" take
    Currency, Money, MoneyScale, RoundedMoney, HalfEven,
    MoneyAllocation;

main() is string {
    let usd = Currency("USD", 100);
    let discount = MoneyScale(Money(1999, usd), rat(85, 100));
    let posted = RoundedMoney(
        HalfEven(discount.value()),
        discount.currency(),
    );
    MoneyAllocation(posted, 3).shares()
        .map() -> share { share.string(); }
        .join(", ");
}
```

The API intentionally has no `float` parameter. Currency is always explicit,
and rounding is required exactly where a rational result becomes indivisible
minor units. A non-decimal currency factor is rendered as an exact rational
value rather than an approximate decimal.

### 5.4 `errors`

Status: **proposed**

This module is the shared error vocabulary, not a generic success wrapper.

Candidate roles and objects:

```okkama
role CodedError {
    domain() is string;
    code() is string;
}
```

- `InvalidInput`, `NotFound`, `PermissionDenied`, `AlreadyExists`;
- adapters that turn stable `NativeFailure` values into stdlib error objects;
- no universal `Result<T>` unless the language eventually adopts that model
  for a concrete reason.

Example:

```okkama
try {
    operation();
}
catch failure is NotFound {
    recover(failure);
}
```

### 5.5 `os`

Status: **stdlib** (`stdlib/os.ok`)
Depends on: `runtime/system` and the builtin `Error` role

`os` is the public, typed view of the host operating system. It must not return
Perl's raw `$^O`, `Config` strings or ad-hoc path separators.

Public values and objects:

```okkama
enum Platform = (
    linux, macos, windows,
    freebsd, openbsd, netbsd, dragonfly,
    solaris, aix, unknown
);

enum Architecture = (
    x86_64, x86, arm64, arm,
    ppc64, ppc, riscv64, s390x, unknown
);

enum PathStyle = (posix, windows);
```

- `Host()` — the current normalized platform and architecture; construction
  is stateless and does not query the provider;
- `Posix()` / `Posix(platform, architecture)` — explicit foreign POSIX
  descriptions; the zero-argument form retains unknown concrete identity;
- `Windows()` / `Windows(architecture)` — explicit foreign Windows
  descriptions;
- `UnsupportedPlatform` — failure when an operation has no safe `unknown`
  behaviour.

Candidate protocol:

```okkama
role OperatingSystem {
    platform() is Platform;
    architecture() is Architecture;
    path_style() is PathStyle;
    line_separator() is string;
    path_list_separator() is string;
}

```

Ordinary filesystem code does not inspect `PathStyle` or concatenate a
separator. `fs/path.Path` uses `Host().path_style()` internally:

```okkama
module app v1.0.0;

import "fs/path" take Path;
import "os" take Host;

main() is @any {
    let host = Host();
    let log = Path("var").joined("log").joined("okkama.log");
    [host.platform(), log];
}
```

Known Windows hosts expose `PathStyle.windows`, `"\r\n"`, and `";"`; every
known non-Windows platform exposes `PathStyle.posix`, `"\n"`, and `":"`.
`Host.platform()` and `architecture()` may return their explicit `unknown`
cases, but path-derived actions throw `UnsupportedPlatform` instead of
guessing. Explicit `Posix()` remains usable when only a foreign path grammar
is known. A `Posix` value cannot retain `Platform.windows`.

Code that parses a path for a known foreign format chooses that format
explicitly and does not pretend it belongs to the host filesystem:

```okkama
import "fs/path" take WindowsPath, PosixPath;

let artifact = WindowsPath("C:\\build\\artifact.zip");
let archive = PosixPath("usr/local/share");
```

The implemented OS/path layer now defines normalized platform/CPU identity,
POSIX versus Windows syntax, and root, drive, UNC, separator and path-list
separator semantics. Future filesystem layers still need to define:

- case-sensitivity/case-preservation capabilities where they can be known;
- native line ending as an explicit formatting choice, never automatic string
  rewriting;
- hostname and user/home-directory APIs only after their failure and encoding
  contracts are designed.

### 5.6 `env/var`

Status: **stdlib** (`stdlib/env/var.ok`)
Depends on: `runtime/environment`, `encoding/utf8`

Public objects:

- role `Environment` — injectable environment capability with an isolated,
  byte-sorted test fake;
- `Env` — current process environment capability;
- `EnvVar` — one validated UTF-8 variable name;
- `EnvEntry` — a detached name/raw-value snapshot;
- `MissingEnvVar`, `InvalidEnvVar`, `InvalidEnvEncoding` — domain errors.

Protocol:

```okkama
role Environment {
    value(variable is EnvVar) is string;
    bytes(variable is EnvVar) is @byte;
    has(variable is EnvVar) is bool;
    set(variable is EnvVar, value is string) is void;
    set_bytes(variable is EnvVar, value is @byte) is void;
    remove(variable is EnvVar) is bool;
    variables() is @EnvEntry;
}
```

Example:

```okkama
module app v1.0.0;

import "env/var" take Environment, Env, EnvVar, EnvEntry;

main() is int {
    let environment is Environment = Env();
    let home = EnvVar("HOME");

    if environment.has(home) {
        environment.value(home);
        environment.bytes(home);
    }

    let snapshot is @EnvEntry = environment.variables();
    snapshot.size();
}
```

The mutating nature is explicit: this object represents the real ambient
process environment. Missing and present-empty are distinct. `bytes` and
`set_bytes` preserve opaque host bytes; `value` and `EnvEntry.string` perform
strict UTF-8 projection and throw `InvalidEnvEncoding` instead of replacement.
Names are nonempty UTF-8, cannot contain NUL or `=`, and are validated when an
action observes them, keeping every constructor state-only. `variables()` is
a detached byte-sorted snapshot. `Environment.fake()` has the same observable
contract, owns no ambient state and never calls `runtime/environment`.

### 5.7 `template/*`

Status: **`template/plain` implemented; structural and HTML templates proposed**<br>
Depends on: builtin `string` actions and `Error`

The first version is deliberately small: named string insertion, comments,
literal opening-tag escaping, strict source validation and deterministic typed
value errors. It is pure Okkama and never evaluates embedded Okkama code.

Objects:

- `Template` — immutable source whose `string(values)` action produces the
  expected value without a verb-returning action;
- `TemplateBinding` and `TemplateValues` — exact string bindings;
- `InvalidTemplate`, `MissingTemplateValue`, `DuplicateTemplateValue` — typed
  failures.

```okkama
module app v1.0.0;

import "template/plain" take Template, TemplateBinding, TemplateValues;

main() is string {
    let values = TemplateValues(
        TemplateBinding("name", "Ada"),
        TemplateBinding("language", "Okkama"),
    );
    Template("Hello, <%= name %> from <%= language %>!").string(values);
}
```

`<%= name %>` is normal output, `<%== name %>` bypasses an escaping policy,
`<%# ... %>` is a non-nesting comment, and `<%%` emits literal `<%`. Plain
templates use the identity escaping policy, so both output tags preserve exact
strings; the distinction is retained for the future HTML adapter. `<% ... %>`
control tags are reserved and rejected in v1 rather than passed to `eval`.

### 5.8 `io/*`

Status: **implemented**

```okkama
role Input {
    bytes(limit is int) is @byte;
}

role StringInput {
    line() is string;
}

role Output {
    write(value is @byte) is void;
    flush() is void;
}

role StringOutput {
    write(data is ...any) is void;
    writeln(data is ...any) is void;
    flush() is void;
}

role Closeable {
    closed() is bool;
    close() is void;
}
```

Owned files, buffers, process pipes and sockets compose the applicable I/O
role with `Closeable`. Borrowed process standard streams deliberately do not
claim lifecycle ownership. `EndOfInput`, `InvalidReadSize`, and
`IncompleteWrite` are typed protocol errors supplied beside the roles.

### 5.9 `fs/*`

Status: **proposed**  
Depends on: `runtime/fs`, `os`, `io/*`, `errors`

Public objects:

- `Path`, `File`, `Dir`, `Meta`, `TempFile`, `TempDir`.

Current working directory belongs to `env/cwd`, not `fs`. It is an explicit
ambient capability, not an incidental static function:

```okkama
module app v1.0.0;

import "env/cwd" take Cwd;
import "fs/path" take Path;

main() is Path {
    let cwd = Cwd();
    let previous = cwd.path();
    cwd.change(Path("/tmp"));
    previous;
}
```

`Cwd` is implemented and accepted (§6.3); ambient mutation stays public
through it rather than being replaced by implicit per-command working
directories.

### 5.10 `proc/*`

Status: **mixed** (`proc/args` is stdlib; remaining modules are proposed)

- `proc/cmd`: `Cmd`, `RunningCmd`, `ExitStatus`, `Pipe`;
- `proc/args`: `Args`, an immutable snapshot of application arguments.

There is no object or module named `proc/process`.

Application arguments require no command object:

```okkama
module app v1.0.0;

import "proc/args" take Args;

main() is @@byte {
    let arguments = Args();
    if arguments.size() > 0 {
        arguments.at(0);
    }
    arguments.bytes();
}
```

`Args.bytes()` preserves exact runtime bytes. `strings()` and `at()` decode
strictly as UTF-8 and never replace malformed input. The source-file path is
not an application argument.

```okkama
let command = Cmd(
    Path("git"),
    Arguments(["status", "--short"]),
    CommandEnvironment([]),
    CurrentDirectory(),
);
let status = command.run();
status.success();
```

### 5.11 `time/*`

Status: **mixed** — `time/duration` is stdlib; the remaining modules are
proposed.

- `Duration`, `Instant`, `Date`, `Clock`, `Timer`;
- `SystemClock` and `MonotonicClock` implementations of `Clock`; the
  deterministic `FakeClock` implementation lives in `test/fake_clock`
  (§6.12), not in this namespace.

```okkama
let timeout is Duration = duration Seconds(30);
let deadline = MonotonicClock().now().after(timeout);
deadline;
```

### 5.12 `socket/*`

Status: **proposed**  
Depends on: `runtime/net`, `runtime/poll`, `io/*`, `net/address`, `errors`

Public objects:

- `TcpSocket`, `TcpListener`, `UdpSocket`, `UnixSocket`;
- `SocketAddress`, `SocketOptions`, `SocketTimeout`;
- shared `Input`/`Output` behaviour only where stream semantics apply.

```okkama
module app v1.0.0;

import "socket/tcp" take TcpSocket;
import "net/address" take Address;

main() is TcpSocket {
    let socket = TcpSocket(Address("example.com", 443));
    socket.connect();
    socket;
}
```

Connection, ownership, closing and timeout behaviour must be fixed before this
API becomes `stdlib`.

### 5.13 `http/*`

Status: **proposed**  
Depends on: `socket/*`, `net/url`, `io/*`, `time/*`, `crypto/tls`, `errors`

Public objects:

- `Request`, `Response`, `Client`, `Server`;
- `Router`, `Route`, and first-class async handler actions;
- `Headers`, `Status`, `Method`, `Body`;
- redirect, timeout and TLS policy objects.

Client example:

```okkama
let request = Get(Url("https://example.com/data.json"));
let response = Client().execute(request);
response;
```

Router direction:

```okkama
let router = Router([
    GetRoute(PathPattern("/health"), health.response),
    PostRoute(PathPattern("/orders"), orders.create),
]);

Server(Address("0.0.0.0", 8080), router).listen();
```

The router dispatches to role-based handler objects. It must not depend on
global callbacks or reflection.

### 5.14 `encoding/*`

Status: **partly implemented**

- `utf8`, `base64`, `hex` convert explicit `string`/`@byte` values;
- `json`, `csv`, `toml` parse and render structured formats;
- decoding failures carry byte offset, line and column where meaningful;
- encoding is deterministic.

```okkama
let document is Json = json JsonSource(source);
let customer = CustomerJson(document).customer();
```

Parsing JSON and constructing a valid domain object remain separate jobs.

### 5.15 `sql/*` and `db/*`

Status: **exploratory**

Responsibilities are deliberately separate:

- `sql/*` represents statements, parameters, queries and dialect rendering;
- `db/*` owns the DBI-style driver contract, connections, transactions,
  execution and rows;
- database drivers are Okkama modules built on byte codecs, sockets, TLS and
  filesystem syscalls; there is no database-aware host ABI.

Candidate roles:

```okkama
role Driver {
    connect() is Connection;
    dialect() is Dialect;
}

role Connection {
    execute(statement is Statement) is DbResult;
    query(query is Query) is @Row;
    transaction() is Transaction;
}

role Transaction {
    execute(statement is Statement) is DbResult;
    commit() is void;
    rollback() is void;
}
```

Example direction:

```okkama
let database = Database(Sqlite(Path("app.db")));
let connection = database.connect();
let query = Query("select id, name from users where active = ?", [true]);
let users = connection.query(query).map() -> row {
    UserRow(row).user();
};
users;
```

Raw string interpolation must never be the parameter API. A driver is selected
by importing and composing an object explicitly; there is no global driver
registry, reflection or string-based loading. Dialect boundaries and
transaction cleanup need separate design passes.

## 6. Proposed objects and actions by module

This section is the complete proposed public inventory for the tree above.
Dependencies are direct dependencies, not the whole transitive closure.
Signatures are design-level Okkama signatures; they become contracts only when
the module status changes to `stdlib`.

### 6.1 Fundamental modules

#### `errors`

Purpose: common domain failure vocabulary and native-failure adaptation.  
Depends on: nothing.

- role `CodedError` — `domain() is string`, `code() is string`;
- role `LocatedError` — `line() is int`, `column() is int`;
- `InvalidInput` — `text() is string`;
- `NotFound` — `text() is string`;
- `PermissionDenied` — `text() is string`;
- `AlreadyExists` — `text() is string`;
- `Unsupported` — `text() is string`;
- `TimedOut` — `text() is string`;
- `NativeError` — `text() is string`, `domain() is string`,
  `code() is string`, `operation() is string`.

This is shared failure vocabulary, not a universal result container.

#### `math`

Purpose: general numeric algorithms independent of primitive-type namespaces.  
Depends on: `errors`.

Builtin receiver behaviour implemented during the `math` milestone:

- exact `int` receiver — `abs() is int`, `sign() is int`,
  `min(other is int) is int`, `max(other is int) is int`,
  `clamp(minimum is int, maximum is int) is int`;
- exact `float` receiver — `abs() is float`, `sign() is int`,
  `min(other is float) is float`, `max(other is float) is float`,
  `clamp(minimum is float, maximum is float) is float`.

- `Floor` — `#projected value() is int`;
- `Ceil` — `#projected value() is int`;
- `Round` — `#projected value() is int`;
- `Power` — `#projected value() is float`;
- `SquareRoot` — `#projected value() is float`;
- `Logarithm` — `#projected value() is float`;
- `InvalidMathDomain` — `text() is string`.

There is no `number` return type, no `math/int` or `math/float` module, and no
implicit mixed-category widening. `byte` must be converted explicitly before
using this API.

#### `random`

Purpose: high-level random ranges, choices and permutations.  
Depends on: `crypto/random`, `collections/list` and the builtin `Error` role.
Implementation status: **stdlib** (`stdlib/random.ok`).

- role `RandomSource` — `bytes(size is int) is @byte`,
  `int(minimum is int, maximum is int) is int`, `bool() is bool`,
  `float() is float`;
- `Random()` — secure implementation of every `RandomSource` action;
- `SeededRandom(seed is int)` — pure deterministic implementation of every
  `RandomSource` action;
- `Choice<T>(values is @T)` and
  `Choice<T>(values is @T, source is RandomSource)` — `#projected value() is T`;
- `Shuffled<T>(values is @T)` and
  `Shuffled<T>(values is @T, source is RandomSource)` — `#projected values() is @T`;
- `InvalidRandomRange` — `text() is string`, compatibility endpoint
  projections `min() is int`, `max() is int`.

`Random.bytes` retains the `crypto/random` size and failure contract. Its
inclusive integer action uses unbiased rejection sampling across the complete
signed 64-bit `int` domain; equal endpoints consume no entropy. `float()`
returns one of the exactly representable 53-bit fractions in `[0.0, 1.0)`,
and `bool()` has equal false/true outcomes.

`SeededRandom` is deliberately not cryptographic. It is a pure query object:
the same seed, action and arguments always produce the same value, and a
shorter byte request is a prefix of a longer one. It has no hidden cursor or
mutation. A future sequential generator must expose its successor state in
its public contract instead of silently changing this object.

`Choice` and `Shuffled` use secure `Random()` by default or an explicit
`RandomSource` for deterministic domain logic and tests. Constructors only
bind candidates and source; selection and immutable permutation happen in the
projection actions. An empty choice and a reversed integer range throw
`InvalidRandomRange`. Singleton choice/shuffle and an equal-endpoint integer
range do not query their source.

```okkama
import "random" take RandomSource, Random, SeededRandom, Choice, Shuffled;

main() is void {
    let secure is RandomSource = Random();
    let repeatable is RandomSource = SeededRandom(42);

    let die is int = secure.int(1, 6);          // inclusive
    let enabled is bool = secure.bool();
    let ratio is float = secure.float();        // 0.0 <= ratio < 1.0
    let color is string = value Choice(
        ["red", "green", "blue"],
        repeatable,
    );
    let order is @int = values Shuffled([1, 2, 3, 4], repeatable);
}
```

#### `os`

Purpose: typed host platform, architecture and path-style information.  
Depends on: `runtime/system` and the builtin `Error` role.
Implementation status: **stdlib** (`stdlib/os.ok`).

- enums `Platform`, `Architecture`, `PathStyle`;
- role `OperatingSystem` — `platform() is Platform`,
  `architecture() is Architecture`, `path_style() is PathStyle`,
  `line_separator() is string`, `path_list_separator() is string`;
- `Host()` — stateless provider-backed `OperatingSystem` for the running host;
- `Posix()` / `Posix(platform, architecture)` — state-only explicit POSIX
  descriptions;
- `Windows()` / `Windows(architecture)` — state-only explicit Windows
  descriptions;
- `UnsupportedPlatform` — `text() is string`.

All native spellings are exhaustively mapped to closed enums. The runtime's
literal `unknown` becomes the corresponding enum case; any other spelling is
a provider invariant panic. Unknown host path conventions throw
`UnsupportedPlatform`. POSIX uses LF/colon; Windows uses CRLF/semicolon.

#### `regex`

Purpose: portable compiled regular-expression behaviour.  
Depends on: `runtime/regex` and the builtin `Error` role.
Implementation status: **stdlib** (`stdlib/regex.ok`).

- `Regex` — `matches(value is string) is bool`,
  `contains(value is string) is bool`, `first(value is string) is Match`,
  `all(value is string) is @Match`, `replaced(value is string, replacement is
  string) is string`;
- `Match` — `string() is string`, `start() is int`, `end() is int`,
  `groups() is @string`;
- `InvalidRegex` — `text() is string`, `offset() is int`;
- `MissingMatch` — `text() is string`.

`Regex(pattern)` is state-only: it retains the ordinary string and performs no
native call. Each public action acquires one private compiled `RegexHandle`,
uses it for that operation and closes it deterministically. `contains` is an
unanchored search; `matches` requires the complete input. `first` returns the
leftmost-longest match or throws `MissingMatch`; `all` returns non-overlapping
leftmost-longest matches in source order.

`Match` is detached from the native handle. Its offsets count Unicode code
points and `string()` slices the full match. `groups()` excludes the full
match and preserves capture order. An unmatched optional capture becomes
`""`; this is intentionally indistinguishable from a participating empty
capture in the `@string` API.

`replaced` replaces every non-overlapping match with the replacement string
literally: `$1`, backslashes and braces have no interpolation meaning.
Zero-length matches advance the next search by one code point while preserving
the skipped source character; a terminal empty match is included.

Malformed patterns become `InvalidRegex` when an action first prepares the
pattern. `offset()` is the exact zero-based Unicode source position (including
the pattern length for an unexpected EOF). Provider diagnostics and handles
never escape this module.

```okkama
let words = Regex("([A-Za-z]+)");
let first = words.first("Hi, Ada");

first.string();                 // "Hi"
first.groups();                 // ["Hi"]
words.replaced("Hi, Ada", "_"); // "_, _"
```

#### `format`

Purpose: positional and named interpolation of Okkama values.  
Depends on: `runtime/format` and the builtin `Error` role.
Implementation status: **stdlib** (`stdlib/format.ok`).

- `Formatted(pattern is string, values is ...any)` — `#projected string() is string`;
- `NamedFormatted(pattern is string, values is %string:any)` —
  `#projected string() is string`;
- `PrintfFormatted(pattern is string, values is ...any)` —
  `#projected string() is string`;
- `InvalidFormat` — `text() is string`, `offset() is int`;
- `MissingFormatValue` — `text() is string`.
- `InvalidPrintf` — `text() is string`.

`Formatted` consumes empty `{}` fields from its variadic tail in encounter
order. The tail is retained internally as `@any`; callers pass values directly
and do not materialize an auxiliary collection.
`NamedFormatted` accepts exact ASCII identifiers matching
`[A-Za-z_][A-Za-z0-9_]*`; repeated fields are valid. Both accept `{{` and
`}}` as literal braces and allow unused supplied values. Strings and chars use
display form; all other values retain the runtime's stable source-like form.

Pattern validation and missing-value detection execute in Okkama before the
trusted provider renders opaque `any` values. `InvalidFormat.offset()` is the
first offending zero-based Unicode code-point offset. `MissingFormatValue`
names the missing positional index or exact named key. Constructors only bind
the pattern and immutable collection (including variadic-tail materialization);
no parsing or native dispatch occurs during construction.

```okkama
let named is %string:any = ["name": "Ada", "count": 3];

string Formatted("{}: {}", "Ada", 3);               // "Ada: 3"
string NamedFormatted("{name}: {count}", named);     // "Ada: 3"
string PrintfFormatted("exact=%8r", rat(1, 3));      // "exact=     1/3"
```

### 6.2 Strings and templates

#### `string/parse`

Purpose: strict conversion from builtin strings to scalar values.  
Depends on: `errors`.
Implementation status: **stdlib**, currently shipped as `strings/parse`.

- `ParsedInt` — `#projected value() is int`;
- `ParsedFloat` — `#projected value() is float`;
- `ParsedBool` — `#projected value() is bool`;
- `ParsedByte` — `#projected value() is byte`;
- `InvalidInteger`, `InvalidFloat`, `InvalidBoolean`, `InvalidByte` —
  `text() is string`, `source() is string`.

Parsing is strict and locale-independent. Whitespace policy belongs to the
caller or an explicitly composed `Trimmed` object.

The accepted grammar is deliberately the scalar spelling already used by
Okkama rather than the host Perl parser:

- `ParsedInt`: `-?[0-9]+`;
- `ParsedFloat`: `-?[0-9]+\.[0-9]+` (a decimal point and digits on both sides
  are required; exponent notation is not accepted);
- `ParsedBool`: exactly `true` or `false`;
- `ParsedByte`: `[0-9]+` whose numeric value is in `0..255`.

Leading zeroes are accepted. A leading `+`, surrounding whitespace,
locale-specific digits, `NaN`, `Inf`, and alternate boolean spellings are
rejected. The four error objects preserve the complete input through
`source()` and format it as `invalid <type>: <source>` through `text()`.

```okkama
module app v1.0.0;

import "strings/parse" take
    ParsedInt, ParsedFloat, ParsedBool, ParsedByte,
    InvalidInteger, InvalidFloat, InvalidBoolean, InvalidByte;

main() is any {
    let count is int = value ParsedInt("-42");
    let ratio is float = value ParsedFloat("3.0");
    let enabled is bool = value ParsedBool("true");
    let octet is byte = value ParsedByte("255");

    try {
        value ParsedInt(" 42");
    }
    catch error is InvalidInteger {
        error.source(); // " 42"
    }
}
```

#### `string/trim`

Purpose: whitespace and line-ending transformations of builtin strings.  
Depends on: nothing.

- `Trimmed` — `#projected string() is string`;
- `LeftTrimmed` — `#projected string() is string`;
- `RightTrimmed` — `#projected string() is string`;
- `Chomped` — `#projected string() is string`.

The module name refers to transformations of the builtin `string`; it does not
introduce a separate `Text` type.

#### Builtin string transformations

Purpose: immutable literal splitting, joining and replacement without
ceremonial stdlib wrappers.
Depends on: nothing.

- `string.split(separator is string) is @string`;
- `@string.join(separator is string) is string`;
- `string.replace(target is string, replacement is string) is string`;
- `string.replace_first(target is string, replacement is string) is string`.

`split` preserves leading, adjacent and trailing empty parts. Matching is
literal, left-to-right and non-overlapping. An empty separator splits the
source into Unicode code-point strings; an empty source therefore produces an
empty, still-tagged `@string`. With a nonempty separator, an empty source
produces `[""]`.

`join` requires exact string elements and performs no implicit conversion. An
empty array produces `""` and a one-element array produces that element.

`replace` processes every literal non-overlapping match; `replace_first`
processes at most the first. An empty target addresses code-point boundaries:
`"ab".replace("", "-")` is `"-a-b-"` and
`"ab".replace_first("", "-")` is `"-ab"`. Removal needs no separate action:
`source.replace(target, "")`.

All four actions are positional-only, exact-arity and immutable. Regex,
capturing, quoting/escaping, locale rules and structured tokenization belong
to higher-level stdlib modules.

#### `template/plain`

Purpose: deterministic named-placeholder rendering into ordinary strings.<br>
Implementation status: **stdlib** (`stdlib/template/plain.ok`).<br>
Depends on: builtin `string` actions and `Error`.

- role `TemplateValueSource` — `names() is @string`, `value(name is string)
  is string`; added when `template/html` was implemented so a second,
  runtime-sized values source could reuse `Template`'s renderer (see
  `template-html-21072026.md`) without a second constructor on
  `TemplateValues`, which its variadic constructor forbids;
- `Template(value is string)` — `string(values is TemplateValueSource) is
  string`;
- `TemplateBinding(name is string, value is string)` — `name() is string`,
  `value() is string`;
- `TemplateValues(values is ...TemplateBinding)` — implements
  `TemplateValueSource`; also `bindings() is @TemplateBinding`, `has(name is
  string) is bool`, `value(name is string) is string`;
- `InvalidTemplate` — `text() is string`, `offset() is int`;
- `MissingTemplateValue` — `text() is string`, `name() is string`.
- `DuplicateTemplateValue` — `text() is string`, `name() is string`.

The lexical grammar is intentionally closed:

- `<%= name %>` inserts a named string through the template's escaping policy;
- `<%== name %>` inserts through the explicit raw channel;
- `<%# ... %>` is a non-nesting comment and may span lines;
- `<%%` emits literal `<%` without opening a tag;
- ordinary `{`, `}`, and `%>` remain literal source characters;
- `<% ... %>` control tags are reserved and fail in v1.

Placeholder names are case-sensitive ASCII `[A-Za-z_][A-Za-z0-9_]*`; only
ASCII space and tab may surround a name. Output is not recursively reinterpreted, source and
replacement line endings are preserved, surplus bindings are allowed, and
duplicate names are rejected rather than choosing the first or last value.
The complete source is validated before value lookup. `InvalidTemplate.offset`
is the first zero-based Unicode code-point offset. Constructors only retain
state; parsing and rendering happen in `string()` and require no native ABI.

#### `template/html`

Purpose: HTML-aware template rendering with escaping by default.  
Implementation status: **stdlib** (`stdlib/template/html.ok`); complete
contract and verification are recorded in `template-html-21072026.md`.

Depends on: `template/plain`, `errors`.

- `HtmlTemplate` — `render(values is HtmlValues) is string`;
- `HtmlBinding(name is string, value is SafeHtml)` — `name() is string`,
  `value() is SafeHtml`;
- `HtmlValues(values is @HtmlBinding)` — implements
  `template/plain.TemplateValueSource`; also `bindings() is @HtmlBinding`;
- `SafeHtml` — `string() is string`;
- `EscapedHtml` — `#projected safe() is SafeHtml`;
- `InvalidHtmlTemplate` — `text() is string`, `offset() is int`.

Ordinary strings are escaped. Only explicit `SafeHtml` bypasses escaping.
`HtmlTemplate.render` runs the caller's source through the same `Template`
lexer/renderer `template/plain` uses, so `<%=`/`<%==` stay lexically
identical tags for both modules; escaping is decided once, when a binding's
`SafeHtml` value is produced (`EscapedHtml` escapes, a bare `SafeHtml`
asserts trust), not by which tag substitutes it. Only `InvalidTemplate`
(a lexical/offset error) is translated to the HTML-domain
`InvalidHtmlTemplate`; `MissingTemplateValue`/`DuplicateTemplateValue` are
reused from `template/plain` unchanged; they are not HTML-specific concepts.

### 6.3 Environment and collections

#### `env/var`

Purpose: typed access to ambient process environment variables.  
Implementation status: **stdlib** (`stdlib/env/var.ok`).
Depends on: `runtime/environment`, `encoding/utf8`.

- role `Environment` — `value(variable is EnvVar) is string`,
  `bytes(variable is EnvVar) is @byte`, `has(variable is EnvVar) is bool`,
  `set(variable is EnvVar, value is string)`,
  `set_bytes(variable is EnvVar, value is @byte)`,
  `remove(variable is EnvVar) is bool`, `variables() is @EnvEntry`;
- `Env` — real process implementation of `Environment`;
- `EnvVar` — `string() is string`, `bytes() is @byte`;
- `EnvEntry` — `variable() is EnvVar`, `bytes() is @byte`,
  `string() is string`;
- `MissingEnvVar`, `InvalidEnvVar`, `InvalidEnvEncoding` —
  `text() is string`.

`EnvVar(name)` and `EnvEntry(variable, bytes)` only bind state. Their actions
enforce the portable environment representation: a name is nonempty UTF-8
without NUL or `=`, and a value contains no NUL. Raw value bytes do not need to
be UTF-8. Text projections decode strictly; raw projections never replace or
normalize bytes. `has` distinguishes missing from a present empty value,
`remove` reports prior presence, and `variables` returns detached entries in
runtime byte order. The role fake maintains the same ordering and snapshot
semantics in instance-local storage.

#### `env/cwd`

Purpose: explicit capability for inspecting and changing ambient cwd.  
Implementation status: **stdlib** (`stdlib/env/cwd.ok`); complete contract
and verification are recorded in `env-cwd-21072026.md`.

Depends on: `runtime/fs`, `fs/path`, `errors`.

- role `WorkingDirectory` — `path() is Path`, `change(path is Path)`;
- `Cwd` — real ambient implementation of `WorkingDirectory`;
- `FixedCwd` — immutable implementation for tests;
- `InvalidWorkingDirectory` — `text() is string`.

`Cwd` is stateless and always re-queries the host: `path()` decodes a fresh
`cwd()`/`path_decode` round trip into a `Path`, and `change(path)` encodes
and calls `chdir`. A `chdir` failure (nonexistent target, permission denied,
...) becomes `InvalidWorkingDirectory`, sanitized the same way
`crypto/random` sanitizes provider failures; a `cwd()`/`path_decode` failure
reading the ambient directory is instead an invariant panic, matching how
`SystemClock.now()` treats an unreadable host clock — the caller supplied no
input that could be at fault. `FixedCwd(start)` never changes: `change()`
always throws `InvalidWorkingDirectory`, so it can also be injected into code
under test to assert that a code path never attempts to change directories.

#### `collections/iteration`

Purpose: nominal immutable traversal protocol for user-defined `for` sources.
Implementation status: **stdlib** (`stdlib/collections/iteration.ok`).
Depends on: no runtime module.

- role `Cursor<T>` — `available() is bool`, `current() is T`,
  `advanced() is Cursor<T>`;
- role `Iterable<T>` — `cursor() is Cursor<T>`.

`cursor()` starts an independent traversal. A cursor is an immutable position;
`advanced()` returns the next position. `available() == false` is ordinary
exhaustion and `for` does not call `current()` or `advanced()` afterward.
Normal body completion and `next` advance once; `last` exits without
advancing. Implementing similarly named actions without `does Iterable<T>`
does not enable iteration. This protocol covers repeatable synchronous
collections, not consumable I/O streams.

```okkama
import "collections/iteration" take Cursor, Iterable;

object CountdownCursor does Cursor<int> {
    has value is int;
    CountdownCursor(:value);

    available() is bool { value > 0; }
    current() is int { value; }
    advanced() is Cursor<int> { CountdownCursor(value: value - 1); }
}

object Countdown does Iterable<int> {
    has start is int;
    Countdown(:start);
    cursor() is Cursor<int> { CountdownCursor(value: start); }
}

main() is void {
    for Countdown(start: 3) -> value {
        value;
    }
}
```

#### `collections/list`

Purpose: immutable persistent head-linked value sequencing.
Implementation status: **stdlib** (`stdlib/collections/list.ok`).
Depends on: `collections/iteration` and the language's builtin `Error` role;
no runtime module.

- role `List<T>` — `prepended(value is T) is List<T>`, `head() is T`,
  `tail() is List<T>`, `empty() is bool`, `size() is int`;
- `EmptyList<T>` — the null-free empty implementation;
- `LinkedList<T>` — a non-empty node retaining its previous list as the tail;
- both implementations also do `Iterable<T>` through an internal immutable
  `ListCursor<T>`;
- `EmptyListAccess` — `text() is string`,
  `requested_operation() is string`.

`prepended` constructs a new head and never changes an existing list. Head,
tail, emptiness and size are constant-time queries; each linked node computes
and retains its size during construction. Empty `head` and `tail` throw
`EmptyListAccess`. Lists therefore work directly as `for` sources in
head-to-tail order. Broader sequence operations remain a separate API
milestone.

#### `collections/set`

Purpose: immutable uniqueness and set algebra when builtins are insufficient.
Status: **questionable**. Depends on: `collections/list`,
`collections/iteration`.

- `Set<T>` — `has(value is T) is bool`, `added(value is T) is Set<T>`,
  `removed(value is T) is Set<T>`, `union(other is Set<T>) is Set<T>`,
  `intersection(other is Set<T>) is Set<T>`, `difference(other is Set<T>) is
  Set<T>`, `empty() is bool`, `size() is int`.

The module survives only if builtin arrays/maps cannot express these semantics
cleanly and efficiently. A first implementation would be an iterable linear
set over `List<T>`; array materialization remains unavailable until the
general `Iterable<T>` boundary can build typed arrays.

#### `collections/queue`

Purpose: immutable first-in-first-out value sequencing.
Implementation status: **stdlib** (`stdlib/collections/queue.ok`).
Depends on: `collections/list`, `collections/iteration`, and the builtin
`Error` role.

- role `Queue<T>` — `enqueued(value is T) is Queue<T>`, `head() is T`,
  `tail() is Queue<T>`, `empty() is bool`, `size() is int`;
- `EmptyQueue<T>` — public null-free empty queue;
- internal `PersistentQueue<T>` — non-empty two-list representation;
- internal `QueueCursor<T>` — FIFO traversal;
- `EmptyQueueAccess` — `text() is string`,
  `requested_operation() is string`.

The non-empty queue retains a dequeue-order front list, a reverse enqueue-order
rear list and an exact count. Enqueue prepends to rear. Removing the final
front item reverses rear into the next front. This preserves old queue values
and gives amortized O(1) enqueue/tail. `for` visits oldest to newest.

#### `collections/stack`

Purpose: immutable last-in-first-out value sequencing.
Implementation status: **stdlib** (`stdlib/collections/stack.ok`).
Depends on: `collections/list`, `collections/iteration`, and the builtin
`Error` role.

- role `Stack<T>` — `pushed(value is T) is Stack<T>`, `top() is T`,
  `popped() is Stack<T>`, `empty() is bool`, `size() is int`;
- `EmptyStack<T>` — public null-free empty stack;
- internal `ListStack<T>` — non-empty semantic wrapper over `List<T>`;
- internal `StackCursor<T>` — traversal through `Stack<T>` behavior;
- `EmptyStackAccess` — `text() is string`,
  `requested_operation() is string`.

Stack does not duplicate linked nodes: push/top/pop delegate to
`prepended/head/tail`. Its cursor uses only the public Stack contract rather
than assuming extra behavior from the retained `List<T>` role. `for` visits
top to bottom.

### 6.4 I/O

#### `io/input`

Purpose: value-named byte and line input capabilities.
Implementation status: **stdlib** (`stdlib/io/input.ok`).
Depends on: the language's builtin `Error` role; no runtime module.

- role `Input` — `bytes(limit is int) is @byte`;
- role `StringInput` — `line() is string`;
- `EndOfInput` — `text() is string`;
- `InvalidReadSize` — `text() is string`.

`Input.bytes(limit)` names its result rather than the operation used to obtain
it. `limit` is a positive byte maximum, not a requested exact count. Zero and
negative limits throw `InvalidReadSize`; fewer bytes may be returned and an
empty byte sequence means EOF.

`StringInput.line()` returns the final unterminated line or one line without
its portable line ending. It throws `EndOfInput` only when no characters
remain, so an empty line is still a successful empty string. It deliberately
does not expose a bounded string query: a byte limit can split valid UTF-8,
while a character limit requires a decoder with retained stream state.

#### `io/output`

Purpose: common byte and string output capabilities.  
Implementation status: **stdlib** (`stdlib/io/output.ok`).
Depends on: the language's builtin `Error` role; no runtime module.

- role `Output` — `write(value is @byte)`, `flush()`;
- role `StringOutput` — `write(data is ...any)`,
  `writeln(data is ...any)`, `writef(pattern is string, data is ...any)`,
  `flush()`;
- `IncompleteWrite` — `text() is string`, `written() is int`.

`write`, `writeln`, `writef`, and `flush` are commands and therefore expose no success
counter. If a concrete output accepts only a prefix and cannot continue, it
throws `IncompleteWrite(written: count)`; the noun `written()` reports that
failure detail. `StringOutput.write` displays and concatenates every argument;
`writeln` appends exactly one `"\n"`, including when called without arguments.
`writef` applies `PrintfFormatted` and appends no implicit newline.
Commands return the receiver through ordinary Okkama chaining semantics, not
as a domain result.

All four I/O roles ship deterministic test-mode fakes. The `Input` fake also
enforces the positive byte limit.

#### `io/closeable`

Purpose: lifecycle ownership independent from data transport.
Implementation status: **stdlib** (`stdlib/io/closeable.ok`).
Depends on: nothing.

- role `Closeable` — `closed() is bool`, `close()`.

Its fake uses identity-local `fake.storage`: `close()` is idempotent,
`closed()` observes the real fake transition, and independent fakes remain
isolated. Files, sockets, pipes and owned buffers may compose `Closeable` with
an I/O role. `Stdin`, `Stdout`, and `Stderr` are borrowed process resources and
must not implement it.

```okkama
module app v1.0.0;

import "io/input" take StringInput, EndOfInput;
import "io/output" take StringOutput, IncompleteWrite;

object Copier {
    has source is StringInput;
    has target is StringOutput;

    Copier(:source, :target);

    copy_line() is void {
        try {
            target.writeln(source.line());
        }
        catch error is EndOfInput {
            target.flush();
        }
        catch error is IncompleteWrite {
            panic error.text();
        }
    }
}
```

#### `io/buffer`

Purpose: in-memory and buffered implementations of I/O roles.  
Implementation status: **stdlib** (`stdlib/io/buffer.ok`).<br>
Depends on: `runtime/memory`, `runtime/format`, `format`, `io/input`, `io/output`,
`io/closeable`, `encoding/utf8`, builtin `Error`.

- `BufferAllocation()` and `BufferAllocation(value is @byte)` —
  `#projected buffer() is Buffer`;
- `Buffer` — implements `Input`, `Output`, and `Closeable`;
  `bytes(limit is int) is @byte`, `write(value is @byte)`, `flush()`,
  `closed() is bool`, `close()`, plus `bytes() is @byte`, `size() is int`,
  `reset()`;
- `StringBufferAllocation()` and `StringBufferAllocation(value is string)` —
  `#projected buffer() is StringBuffer`;
- `StringBuffer` — implements `StringInput`, `StringOutput`, and `Closeable`;
  `line() is string`, variadic `write`/`writeln`/`writef`, `flush()`,
  `closed() is bool`, `close()`, plus `string() is string`, `reset()`;
- `InputBuffering(source is Input, size is int)` —
  `#projected input() is BufferedInput`;
- `BufferedInput` — implements `Input` and `Closeable`;
  `bytes(limit is int) is @byte`, `closed() is bool`, `close()`;
- `OutputBuffering(target is Output, size is int)` —
  `#projected output() is BufferedOutput`;
- `BufferedOutput` — implements `Output` and `Closeable`;
  `write(value is @byte)`, `flush()`, `closed() is bool`, `close()`;
- `ClosedBuffer` — `text() is string`;
- `InvalidBufferCapacity` — `text() is string`, `capacity() is int`.

These are live owned resources, so allocation is explicit. Every ordinary
constructor above only retains state; `#projected buffer()`, `#projected input()` or `#projected output()`
acquires the private byte-memory capability. The concrete constructors taking
`ByteMemoryHandle` or internal cache objects are implementation boundaries,
not application construction APIs.

`Buffer` stores unread bytes. `bytes(limit)` consumes at most `limit`, while
zero-argument `bytes()` returns a detached unread snapshot. `size()` is the
unread count, writes append, and `reset()` discards everything. `StringBuffer`
applies the same cursor to strict UTF-8 strings. `line()` finds LF in pure
Okkama, consumes through it, then applies `chomp`; at EOF it returns the final
unterminated tail and subsequently throws `EndOfInput`.

`BufferedInput` performs one read of `max(limit, capacity)` only when its
private cache is empty; a cache hit never touches the source. Consequently a
query may return fewer bytes than its requested limit even while the source
has more data, exactly as the `Input` contract permits. `BufferedOutput`
drains when its cache reaches capacity and on `flush`/`close`. If the target
throws `IncompleteWrite`, only `written()` bytes are consumed and the exact
unwritten suffix remains retryable.

Both decorators are non-owning: closing them clears/closes their private
cache, never the wrapped `Input` or `Output`. Close is idempotent. Output close
flushes first and remains open if flushing fails.

```okkama
import "io/buffer" take
    BufferAllocation, Buffer,
    StringBufferAllocation, StringBuffer,
    OutputBuffering, BufferedOutput;

main() is @byte {
    let bytes is Buffer = buffer BufferAllocation([byte(1), byte(2)]);
    bytes.write([byte(3)]);
    bytes.bytes(2); // consumes [byte(1), byte(2)]
    bytes.bytes();  // detached unread snapshot: [byte(3)]

    let text is StringBuffer = buffer StringBufferAllocation("one\ntwo");
    text.line();    // "one"
    text.writeln("!");
    text.string();  // "two!\n"

    let target is Buffer = buffer BufferAllocation();
    let output is BufferedOutput = output OutputBuffering(target, 4096);
    output.write([byte(65)]);
    output.close();
    target.bytes(); // [byte(65)]; target remains open
}
```

#### `io/stdin`

Purpose: process standard input as bytes or strict UTF-8 strings.  
Implementation status: **stdlib** (`stdlib/io/stdin.ok`).
Depends on: `runtime/stdio`, `runtime/utf8`, `io/input`.

- `Stdin` — implements `StringInput`; `line() is string`;
- `StdinBytes` — implements `Input`; `bytes(limit is int) is @byte`.

`Stdin.line()` strictly decodes complete byte lines as UTF-8 and applies
`string.chomp()`. CRLF, LF and CR are removed once; a blank line remains `""`.
It throws `EndOfInput` only after all bytes are consumed. `StdinBytes` exposes
the same provider buffer without decoding, so byte and line queries may be
mixed without duplicating or dropping data. Neither object is closeable.

#### `io/stdout`

Purpose: process standard output with byte, string and formatting actions.  
Implementation status: **stdlib** (`stdlib/io/stdout.ok`).
Depends on: `runtime/stdio`, `runtime/utf8`, `runtime/format`, `format`,
`io/output`.

- `Stdout` — implements `StringOutput`; `write(data is ...any)`,
  `writeln(data is ...any)`, `writef(pattern is string, data is ...any)`,
  `flush()`;
- `StdoutBytes` — implements `Output`; `write(data is @byte)`, `flush()`.

The objects are separate because a fixed `write(@byte)` action cannot coexist
with variadic `write(...any)`. `Stdout` formats values, encodes UTF-8, and
checks the private native count before completing the command. `StdoutBytes`
preserves exact octets. Both are borrowed and not closeable.

#### `io/stderr`

Purpose: process diagnostic output with the standard output protocol.  
Implementation status: **stdlib** (`stdlib/io/stderr.ok`).
Depends on: `runtime/stdio`, `runtime/utf8`, `runtime/format`, `format`,
`io/output`.

- `Stderr` — implements `StringOutput`; `write(data is ...any)`,
  `writeln(data is ...any)`, `writef(pattern is string, data is ...any)`,
  `flush()`;
- `StderrBytes` — implements `Output`; `write(data is @byte)`, `flush()`.

Both target standard error. They never alias standard output.

```okkama
module app v1.0.0;

import "io/input" take EndOfInput;
import "io/stdin" take Stdin;
import "io/stdout" take Stdout;
import "io/stderr" take Stderr;

main() is void {
    let input = Stdin();
    let output = Stdout();

    output.write("name: ");
    try {
        output.writeln("hello, ", input.line());
    }
    catch error is EndOfInput {
        Stderr().writeln(error.text());
    }
}
```

### 6.5 Filesystem and processes

#### `fs/path`

Purpose: host-native and explicit foreign path construction without separator concatenation.  
Depends on: `os`, `encoding/utf8`, `collections/list` and the builtin `Error`
role.
Implementation status: **stdlib** (`stdlib/fs/path.ok`).

- role `PathValue` — `joined(part is string) is PathValue`,
  `parent() is PathValue`, `normalized() is PathValue`,
  `absolute() is bool`, `string() is string`,
  `bytes() is @byte`;
- `Path(value is string)` — host-native implementation of `PathValue`,
  selected by `Host` when an action is evaluated;
- `PosixPath(value is string)` and `WindowsPath(value is string)` — explicit
  foreign `PathValue` syntaxes;
- `InvalidPath` — `text() is string`;
- `MissingPathParent` — `text() is string`.

All constructors only retain their source. `string()`, `absolute()`, and every
derived action validate when observed. `string()` preserves the valid source
exactly; only `normalized()` returns canonical separators and components.
Normalization is lexical: it removes `.` and repeated separators, resolves
`..` without filesystem access, clamps it at a root, removes trailing
separators except for roots and maps an empty relative path to `.`. It is
immutable and idempotent.

`joined(part)` accepts a relative path fragment, including multiple relative
components. It rejects POSIX roots and every rooted, drive-qualified or UNC
Windows fragment instead of silently replacing the base. An empty fragment is
an identity operation. `parent()` is computed from the normalized value:
single relative components have parent `.`, while `.`, empty normalized
values, POSIX/drive/root-relative roots, bare drive contexts and UNC share
roots throw `MissingPathParent`.

POSIX paths reject NUL and are absolute when rooted by `/`; backslash is an
ordinary character. Windows paths accept `/` and `\` as source separators and
canonicalize to `\`. Drive-rooted and well-formed UNC share paths are
absolute; drive-relative (`C:name`) and root-relative (`\name`) paths are not.
Windows control characters, `"<>|?*`, misplaced/multiple colons and incomplete
UNC roots throw `InvalidPath`; device namespaces are outside this portable
grammar. `bytes()` is the exact UTF-8 encoding of `string()` and performs no
normalization. Explicit foreign paths are syntax values; their bytes do not
claim compatibility with the host filesystem.

`Path` asks `Host().path_style()` only from its actions. Unknown hosts throw
`UnsupportedPlatform` instead of defaulting to POSIX. The module is pure
Okkama and never calls `runtime/fs` or tests path existence.

```okkama
import "fs/path" take PathValue, Path, PosixPath, WindowsPath;

main() is string {
    let log is PathValue = Path("var")
        .joined("log")
        .joined("okkama.log");
    let archive = PosixPath("/srv//data/../archive").normalized();
    let artifact = WindowsPath("C:/build/./out/app.exe").normalized();

    log.bytes();
    archive.string();  // "/srv/archive"
    artifact.string(); // "C:\\build\\out\\app.exe"
}
```

#### `fs/meta`

Purpose: portable filesystem metadata and permissions.  
Implementation status: **stdlib** (`stdlib/fs/meta.ok`); complete contract
and verification are recorded in `fs-meta-21072026.md`.

Depends on: `fs/path`, `runtime/fs`, `time/instant`, `errors`.

- enum `FileKind` — file, directory, link, other;
- `Meta` — state-only `Meta(path is PathValue)`; `kind() is FileKind`,
  `size() is int`, `modified() is Instant`, `permissions() is Permissions`;
- `Permissions` — state-only `Permissions(mode is int)`; `readable() is
  bool`, `writable() is bool`, `executable() is bool`, `mode() is int`;
- `MissingPath` — `text() is string`.

`Meta` reads through `runtime/fs.lstat`, not `stat`: following symlinks
would make `FileKind.link` unreachable, since a followed link always
resolves to what it points at. `modified()` is built from `lstat`'s
`modified_sec` as `ClockIdentity.system` nanoseconds, matching what
`SystemClock` means by wall-clock time. `Permissions` decodes only the
owner's `rwx` bits from the raw POSIX mode; `runtime/fs`'s stat map carries
no `uid`/`gid`/process-identity, so "does the current process have this
access" cannot be computed portably, and owner bits are the closest
consistently meaningful reading across POSIX and Windows.

Only a `not_found` `lstat` failure becomes catchable `MissingPath`: `Meta`'s
input path is caller-supplied, so a nonexistent target is an ordinary,
retryable outcome. Any other native failure code (permission denied,
unexpected) is an invariant panic instead, since fs/meta declares no broader
error vocabulary for it; a future filesystem module may widen this if it
turns out to matter in practice.

#### `fs/file`

Purpose: whole-file and opened-file byte/string behaviour.  
Implementation status: **stdlib** (`stdlib/fs/file.ok`); complete contract
and verification are recorded in `fs-file-21072026.md`.

Depends on: `fs/path`, `fs/meta`, `io/input`, `io/output`, `io/closeable`,
`runtime/fs`, `runtime/memory`, `encoding/utf8`, `errors`.

- `File(path is PathValue)` — `exists() is bool`, `meta() is Meta`,
  `bytes() is @byte`, `string() is string` (throws `InvalidFileEncoding`
  for non-UTF-8 content), `write(value is @byte)`, `write_string(value is
  string)`, `append(value is @byte)`, `remove()`; every whole-file
  operation that needs the file to already exist throws `MissingFile`
  otherwise;
- `OpenedFile(path is PathValue)` — implements `Input` and `Output` (opens
  read/write, creating the file if missing, without truncating existing
  content); does `Closeable`; `seek(offset is int) is int`, `flush()`,
  `close()`; `closed()` is backed by a `runtime/memory.ByteMemoryHandle`
  sentinel (no native "is this FileHandle closed" query exists) and is
  idempotent, mirroring the real native handle table's own idempotent
  `close`;
- `MissingFile`, `InvalidFileEncoding` — `text() is string`.

#### `fs/dir`

Purpose: directory creation, listing, walking and removal.  
Implementation status: **stdlib** (`stdlib/fs/dir.ok`); complete contract
and verification are recorded in `fs-dir-21072026.md`.

Depends on: `fs/path`, `fs/meta`, `fs/file`, `os`, `runtime/fs`, `errors`.

- `Dir(path is PathValue)` — `exists() is bool`, `create()` (throws
  `errors.AlreadyExists` for an existing path), `create_all()`
  (idempotent, creates every missing ancestor), `entries() is @DirEntry`
  (single level, scans in a bounded-window-doubling scan — see below),
  `remove()` (throws `DirectoryNotEmpty` if non-empty, `MissingDirectory`
  if absent), `remove_all()` (recursive, symlink-safe — see below);
- `DirEntry` — `path() is Path`, `meta() is Meta`;
- `DirectoryWalk(path is PathValue)` — `entries() is @DirEntry` (full
  recursive traversal, every entry at every level, symlink-safe);
- `MissingDirectory`, `DirectoryNotEmpty` — `text() is string`.

Recursive removal must remain visibly destructive and must never follow
symbolic links by accident. Both `remove_all()` and `DirectoryWalk`
classify each entry via `Meta`'s `lstat`-backed `kind()`, so a symlink
pointing at a directory is never mistaken for one to recurse into — it is
always removed/reported as a leaf entry.

`Dir.entries()`'s bounded scan starts at a 256-entry window and doubles
(rescanning from the start via `rewinddir`) until a scan completes inside
its own window — the same bounded-window-doubling-retry idiom already
used by `encoding/csv`/`encoding/toml`'s lookahead slicing, since a
directory's entry count has no cheap upfront size hint the way a
document's byte length does.

#### `fs/temp`

Purpose: uniquely named temporary files/directories with explicit ownership.  
Implementation status: **stdlib** (`stdlib/fs/temp.ok`).
Depends on: `fs/path`, `fs/file`, `fs/dir`, `random`, `encoding/hex`, `errors`,
`runtime/fs`, `runtime/memory`. Not `env/cwd`: the draft listed it, but
`TempRoot` takes its containing directory explicitly rather than defaulting
to an ambient current directory, matching the "no hidden ambient state"
Phase 7 gate.

- `TempFile(path is Path)` — `file() is File`, `path() is Path`,
  `keep() is File`;
- `TempDir(path is Path)` — `dir() is Dir`, `path() is Path`,
  `keep() is Dir`;
- `TempRoot(directory is PathValue)` — `file(prefix is string) is TempFile`,
  `dir(prefix is string) is TempDir`;
- `TemporaryEntryFailure` — `text() is string`.

Temporary objects remove owned entries during destruction unless ownership is
transferred explicitly with `keep()`; `keep()` returns the underlying
`File`/`Dir` and the temporary wrapper's own destroy afterward leaves it
untouched. `TempRoot.file`/`.dir` generate a unique name (an 8-byte random hex
suffix joined to the given prefix) and retry on a genuine name collision;
`open()` uses the native exclusive-create flag for files, and `mkdir`'s own
atomicity covers directories, so no window exists where two temp entries
could observe or clobber each other's path. A missing `directory` reports
`TemporaryEntryFailure`, not a panic, since a bad caller-supplied root is an
anticipated, not a host-level, failure.

#### `proc/args`

Purpose: immutable application argument snapshots with explicit decoding.  
Implementation status: **stdlib** (`stdlib/proc/args.ok`).
Depends on: `runtime/process`, `encoding/utf8`.

- `Args` — `bytes() is @@byte`, `strings() is @string`, `size() is int`,
  `at(index is int) is string`;
- `InvalidArgumentEncoding` — `text() is string`, `index() is int`;
- `MissingArgument` — `text() is string`, `index() is int`.

`Args()` is stateless: the runtime provider already owns the defensive
application-argument snapshot. `bytes` preserves every exact argument,
including empty and non-UTF-8 values. `strings` decodes in ascending order and
reports the first malformed argument's zero-based index. `at` performs the
same strict decoding for one value and throws `MissingArgument` for every
negative or upper-bound index. The CLI source path is excluded.

#### `proc/cmd`

Purpose: construction, execution and supervision of child commands.  
Implementation status: **stdlib** (`stdlib/proc/cmd.ok`), POSIX only (native
`spawn`/`spawn_configured` return `unsupported` on Windows today).  
Depends on: `runtime/process`, `fs/path`, `env/cwd`, `env/var`, `io/input`,
`io/output`, `encoding/utf8`. Not `time/duration`: nothing in the checklist's
final signatures takes a timeout; `errors` is used only implicitly (`Error`
is builtin, needs no import).

- `Arguments(values is @string)` — `values() is @string`;
- `EnvironmentBinding(variable is EnvVar, value is string)` —
  `variable() is EnvVar`, `value() is string`;
- `CommandEnvironment(bindings is @EnvironmentBinding)` —
  `bindings() is @EnvironmentBinding`;
- role `CommandDirectory` — `path() is Path`; `CurrentDirectory()` reads
  `env/cwd.Cwd().path()` at call time, `ExplicitDirectory(path is Path)`
  wraps a caller-given path;
- `Cmd(executable is Path, arguments is Arguments,
  environment is CommandEnvironment, directory is CommandDirectory)` —
  `run() is ExitStatus`, `start() is RunningCmd`. Always calls native
  `spawn_configured`, never plain `spawn`: `CommandEnvironment` is a closed,
  explicit binding list with no "inherit ambient" sentinel, so a caller who
  wants inheritance builds it themselves (e.g. from `env/var.Env().variables()`);
- `RunningCmd` — `wait() is ExitStatus`, `try_wait() is ExitStatus` (throws
  `MissingExitStatus` while the child is still running), `terminate()`
  (portable signal 1 / SIGTERM), `kill()` (portable signal 2 / SIGKILL),
  `input() is Output`, `output() is Input`, `errors() is Input`,
  `id() is int`, `close()` (closes all three pipes and the process handle;
  no separate `destroy`, matching `fs/file.OpenedFile`'s explicit-`Closeable`
  precedent rather than `time/timer.Timer`'s destroy-only one, since the
  checklist explicitly wants a public `close()` here);
- `ExitStatus` — `success() is bool`, `code() is int`, `signalled() is bool`;
- `CommandFailure`, `MissingExitStatus` — `text() is string`.

### 6.6 Time

#### `time/duration`

Purpose: typed time spans and duration arithmetic.  
Depends on: the builtin `Error` role.
Implementation status: **stdlib** (`stdlib/time/duration.ok`).

- `Duration(nanoseconds is int)` — `nanoseconds() is int`,
  `plus(other is Duration) is Duration`,
  `minus(other is Duration) is Duration`, `times(factor is int) is Duration`,
  `less(other is Duration) is bool`;
- `Nanoseconds`, `Milliseconds`, `Seconds`, `Minutes`, `Hours` —
  state-only `amount is int` constructors and
  `#projected duration() is Duration`;
- `InvalidDuration` — `text() is string`.

`Duration` is a signed span in the closed nanosecond range
`-9223372036854775808..9223372036854775807`. Negative values are valid: they
are needed for differences and inverse shifts. Arithmetic and named-unit
projection are exact and throw `InvalidDuration` before an intermediate value
can overflow; no wrapping, saturation or float conversion is allowed.

Every constructor only retains its `int`. `nanoseconds`, arithmetic and unit
projection perform validation and calculation. The module is pure Okkama and
does not call `runtime/time`; `time/clock` alone bridges a nonnegative
`Duration` to native `sleep`.

```okkama
let timeout is Duration = duration Seconds(30);
let retry is Duration = timeout.plus(duration Milliseconds(500));
let rewind is Duration = (duration Minutes(2)).times(-1);

retry.nanoseconds(); // 30500000000
rewind.less(Duration(0)); // true
```

#### `time/instant`

Purpose: clock-bound points in time and interval measurement.  
Implementation status: **stdlib** (`stdlib/time/instant.ok`).<br>
Depends on: `time/duration`, `errors`.

- enum `ClockIdentity` — `system`, `monotonic`;
- `Instant(nanoseconds is int, clock is ClockIdentity)` —
  `nanoseconds() is int`, `clock_identity() is ClockIdentity`,
  `after(duration is Duration) is Instant`,
  `before(duration is Duration) is Instant`, `since(other is Instant) is
  Duration`, `less(other is Instant) is bool`;
- `IncompatibleClocks` — `text() is string`.

Construction retains only the exact value and nominal identity. Shifts use
checked `Duration` arithmetic and preserve identity. `since` means receiver
minus argument. Wall and monotonic instants cannot be ordered or subtracted;
both operations throw `IncompatibleClocks`.

#### `time/date`

Purpose: civil dates, date-times, weekdays and time-zone projection.  
Implementation status: **stdlib** (`stdlib/time/date.ok`).<br>
Depends on: `math`, `time/instant`, `time/duration`, `errors`.

- `Date` — `year() is int`, `month() is int`, `day() is int`,
  `plus_days(days is int) is Date`, `weekday() is Weekday`,
  `string() is string`;
- `DateTime` — `date() is Date`, `hour() is int`, `minute() is int`,
  `second() is int`, `instant() is Instant`, `string() is string`;
- `TimeZone` — `name() is string`, `at(instant is Instant) is DateTime`;
- enums `Month`, `Weekday`;
- `InvalidDate`, `UnknownTimeZone` — `text() is string`.

`Date` is the proleptic Gregorian calendar for years `1..9999` and renders as
`YYYY-MM-DD`; `0001-01-01` is Monday. `plus_days` is exact across leap,
century, month and year boundaries. `DateTime` retains the source instant and
derives fields with floor semantics for negative Unix nanoseconds.

The v1 initial zone policy accepts exactly `TimeZone("UTC")`. Other names are
`UnknownTimeZone` until bundled, versioned time-zone data and its update policy
are accepted. The host's local zone/database is never consulted implicitly.
Monotonic instants cannot be projected to civil time.

#### `time/tzdb`

Purpose: deterministic named-zone projection over a bundled IANA database and
explicit discovery of the host's configured zone.<br>
Implementation status: **proposed**; vertical backlog in
`docs/todo/time-tzdb-21072026.md`.<br>
Depends on: `runtime/timezone`, `os`, `time/duration`, `time/instant`,
`time/date`, `errors`.

- role `TimeZoneDatabase` — `version() is string`,
  `known(name is string) is bool`,
  `canonical_name(name is string) is string`,
  `offset(name is string, instant is Instant) is Duration`;
- `BundledTimeZones` — implements `TimeZoneDatabase` from immutable data shipped
  with the same stdlib release;
- `TimeZone` — state-only named-zone facade with `name() is string` and
  `at(instant is Instant) is DateTime`;
- role `LocalTimeZoneIdentity` — `identifier() is string`,
  `territory() is string`;
- `HostTimeZoneIdentity` — explicit `runtime/timezone` adapter;
- `HostTimeZone` — `#projected time_zone() is TimeZone`;
- `UnknownTimeZone`, `InvalidTimeZoneData`, `UnsupportedHostTimeZone` —
  `text() is string`.

`time/date` remains the pure Gregorian/date-time layer. `time/tzdb` depends on
it and owns the named-zone facade, so the two modules never import each other.
The current UTC-only `TimeZone` and `UnknownTimeZone` are migrated from
`time/date` when this module is implemented. `TimeZone(name)` stores only the
name and bundled-database object; rule lookup and offset calculation happen in
actions. An overload accepting `TimeZoneDatabase` supplies deterministic test
data without changing the ordinary API.

The bundled snapshot is the only source of transition rules. Its exact IANA
release is returned by `version()` and updated only with a versioned stdlib
release; running a program never downloads data and never silently reads the
host's tzdata. Links and aliases resolve to a canonical IANA name. Rules cover
every instant representable by Okkama's signed-64 nanosecond `Instant`; Unix
leap-second semantics remain unchanged.

Host-zone selection is opt-in:

```okkama
import "time/clock" take SystemClock;
import "time/tzdb" take TimeZone, HostTimeZone;

main() is string {
    let qostanay = TimeZone("Asia/Qostanay")
        .at(SystemClock().now());
    let local = (time_zone HostTimeZone())
        .at(SystemClock().now());

    qostanay.string();
    local.string();
}
```

`runtime/timezone` exposes only the platform-native configured identifier and
territory hint. `HostTimeZone` maps those through bundled IANA/CLDR mapping
data. It never asks the host for offsets or transition rules. A missing or
unmappable host identity throws `UnsupportedHostTimeZone`; an explicitly
unknown IANA name throws `UnknownTimeZone`.

#### `time/clock`

Purpose: injectable wall and monotonic time capabilities.  
Implementation status: **stdlib** (`stdlib/time/clock.ok`).<br>
Depends on: `runtime/time`, `time/instant`, `time/duration`, `errors`.

- role `Clock` — `now() is Instant`, `sleep(duration is Duration)`;
- `SystemClock` — wall-clock implementation;
- `MonotonicClock` — interval/deadline implementation;
- `OffsetClock(source is Clock, offset is Duration)` — `now() is Instant`,
  `sleep(duration is Duration)`;
- `InvalidSleepDuration` — `text() is string`.

All constructors are state-only. System/monotonic provider calls happen in
`now`; `OffsetClock.now` derives a new instant from its source and preserves
the source identity, while relative sleep is delegated without applying the
offset. Negative spans remain valid durations but are specifically rejected at
the sleep boundary. Unexpected provider failures are sanitized invariant
panics.

#### `time/timer`

Purpose: cancellable one-shot and periodic waits built from clocks.  
Implementation status: **stdlib** (`stdlib/time/timer.ok`); complete contract
is recorded in `docs/todo/time-timer-contract-21072026.md`.<br>
Depends on: `runtime/time`, `time/duration`, `time/instant`, `errors`.

- `ScheduledTimer(duration is Duration)` — `#projected timer() is Timer`;
- `Timer` — `wait()`, `cancel()`, `cancelled() is bool`,
  `deadline() is Instant`;
- `ScheduledPeriodicTimer(interval is Duration)` —
  `#projected timer() is PeriodicTimer`;
- `PeriodicTimer` — `wait()`, `cancel()`, `cancelled() is bool`,
  `interval() is Duration`;
- `CancelledTimer`, `InvalidTimerDuration` — `text() is string`.

Timers are live owned resources, like files and sockets, rather than immutable
calendar values. Schedule constructors retain only a duration; projection
performs explicit acquisition, and resource constructors only bind an opaque
`TimerHandle`. Destruction closes that handle exactly once, with runtime
shutdown as the leak owner.

`cancel()` is an idempotent command. `cancelled()` is the separate named query;
the former draft `cancel() is bool` was rejected because a high-level command
must not leak a service counter. Waiting after cancellation throws
`CancelledTimer`. A fired one-shot cannot subsequently become cancelled.
Periodic timers start after one interval, keep fixed-rate cadence and skip
missed ticks to the first future deadline without burst catch-up or
fixed-delay drift.

```okkama
import "time/duration" take Milliseconds, Seconds;
import "time/timer" take
    ScheduledTimer, Timer,
    ScheduledPeriodicTimer, PeriodicTimer;

main() is bool {
    let timeout is Timer = timer ScheduledTimer(duration Milliseconds(250));
    timeout.deadline();
    timeout.wait();

    let heartbeat is PeriodicTimer =
        timer ScheduledPeriodicTimer(duration Seconds(1));
    heartbeat.wait();
    heartbeat.cancel();
    heartbeat.cancelled(); // true
}
```

### 6.7 Network and sockets

#### `net/address`

Purpose: validated IP and socket endpoint values.  
Implementation status: **stdlib** (`stdlib/net/address.ok`).<br>
Depends on: `runtime/net`, builtin `Error`.

- enum `AddressFamily` — ipv4, ipv6, unix;
- `IpAddress(family is AddressFamily, text is string)` —
  `family() is AddressFamily`, `string() is string`,
  `bytes() is @byte`;
- `SocketAddress(address is IpAddress, port is int)` —
  `address() is IpAddress`, `port() is int`,
  `string() is string`;
- `HostPort(host is string, port is int)` — `host() is string`,
  `port() is int`;
- `InvalidAddress`, `InvalidPort` — `text() is string`.

`IpAddress` accepts only the `ipv4` and `ipv6` cases. The `unix` case is
reserved for the future Unix-socket address object and is not an IP address.
Its constructor only retains family and source text. The query actions
validate numeric syntax through the portable `runtime/net` boundary;
`string()` produces canonical spelling and `bytes()` returns only the four or
sixteen network-order address octets. The versioned runtime endpoint envelope
and its zero port remain private and are never retained as hidden constructor
output.

Ports use the closed `0..65535` domain. IPv4 socket strings are
`address:port`; IPv6 strings are `[address]:port`, so their boundary is
unambiguous. `HostPort` is deliberately unresolved: it validates a nonempty
host without surrounding or embedded ASCII space/tab/line-feed characters
and a valid port when queried, but DNS policy belongs to `net/dns`. Native
parse/format failures become `InvalidAddress` without provider diagnostics.

```okkama
import "net/address" take AddressFamily, IpAddress, SocketAddress, HostPort;

main() is string {
    let loopback = IpAddress(AddressFamily.ipv6, "0:0:0:0:0:0:0:1");
    let endpoint = SocketAddress(loopback, 8080);
    let upstream = HostPort("api.example.test", 443);

    endpoint.string(); // "[::1]:8080"
    upstream.host();   // unresolved: "api.example.test"
}
```

#### `net/dns`

Purpose: forward and reverse name resolution through an injectable role.  
Implementation status: **stdlib** (`stdlib/net/dns.ok`).<br>
Depends on: `runtime/net`, `net/address`, builtin `Error`.

- role `Resolver` — `addresses(host is HostPort) is @SocketAddress`,
  `name(address is IpAddress) is string`;
- `Dns` — system implementation of `Resolver`;
- `FixedDns(addresses is %string:@IpAddress)` and
  `FixedDns(addresses is %string:@IpAddress, names is %string:string)` —
  deterministic implementation for tests;
- `UnresolvedHost`, `MissingReverseName` — `text() is string`.

`Dns.addresses()` passes socket type zero to `runtime/net.resolve`: DNS returns
validated IP endpoints without silently choosing TCP or UDP policy. The
requested `HostPort.port()` is applied to every result, provider order is
preserved after native deduplication, and an empty/provider failure becomes
`UnresolvedHost` without leaking host diagnostics. Reverse lookup begins from
a validated numeric `IpAddress`; a numeric fallback is not a reverse name and
becomes `MissingReverseName`.

`FixedDns` owns immutable forward and optional reverse maps. Forward keys are
the exact unresolved host strings, reverse keys are canonical
`IpAddress.string()` values. It performs no native calls and applies the port
from each query rather than retaining an endpoint in its constructor.

```okkama
import "net/address" take AddressFamily, IpAddress, HostPort;
import "net/dns" take Resolver, Dns, FixedDns;

main() is string {
    let system is Resolver = Dns();
    let endpoints = system.addresses(HostPort("example.com", 443));

    let fixed is Resolver = FixedDns([
        "api.test": [IpAddress(AddressFamily.ipv4, "192.0.2.10")],
    ]);
    fixed.addresses(HostPort("api.test", 8443))[0].string();
    // "192.0.2.10:8443"
}
```

#### `net/url`

Purpose: parsing, validation, composition and resolution of URLs.  
Implementation status: **stdlib** (`stdlib/net/url.ok`).<br>
Depends on: builtin string actions, `collections/list`, `strings/parse`,
`encoding/utf8`, `net/address`, builtin `Error`.

- `Url` — `scheme() is string`, `authority() is Authority`,
  `path() is string`, `query() is QueryParameters`, `fragment() is string`,
  `resolved(reference is UrlReference) is Url`, `string() is string`;
- `Authority` — `host() is string`, `port() is int`, `userinfo() is string`;
- `QueryParameter(name is string, value is string)` — `name() is string`,
  `value() is string`;
- `QueryParameters(values is @QueryParameter)` —
  `values() is @QueryParameter`, `all(name is string) is @string`,
  `string() is string`;
- `UrlReference` — `string() is string`;
- `InvalidUrl`, `InvalidUrlEncoding` — `text() is string`,
  `offset() is int`;
- `MissingUrlPort` — `text() is string`.

Every constructor only retains source state. Observation performs parsing,
validation, UTF-8 conversion and canonical rendering. `Url` deliberately
models an absolute network URL with `scheme://authority`; relative and
network-path references belong to `UrlReference`. Scheme and reg-name host
ASCII are lowercased, IPv6 literals are canonicalized, percent hex is
uppercased, and raw Unicode component symbols are encoded as UTF-8 `%HH`.
This version does not perform IDNA mapping: a Unicode reg-name is represented
by its UTF-8 percent encoding, not silently converted to a host-specific name.

Percent decoding is strict UTF-8 and reports a source-string offset. Query
parameters retain order, duplicates and empty values. They are URI queries,
not HTML form data: `+` is a literal plus and a programmatic space renders
`%20`. `resolved()` implements RFC 3986 reference resolution including the
dot-segment algorithm. Absence of userinfo, query or fragment is exposed by
the API's existing empty-value vocabulary; absence of an explicit authority
port throws `MissingUrlPort` rather than returning a magic integer.

```okkama
import "net/url" take Url, UrlReference, QueryParameter, QueryParameters;

main() is string {
    let base = Url("https://Example.COM/a/b?q=one&q=two");
    let target = base.resolved(UrlReference("../c#part"));
    let query = QueryParameters([
        QueryParameter("name", "Ada Lovelace"),
        QueryParameter("tag", "one"),
        QueryParameter("tag", "two"),
    ]);

    target.string();          // "https://example.com/c#part"
    query.all("tag");         // ["one", "two"]
    query.string();           // "name=Ada%20Lovelace&tag=one&tag=two"
}
```

#### `socket/socket`

Purpose: shared socket lifecycle, options and timeout vocabulary.  
Implementation status: **stdlib** (`stdlib/socket/socket.ok`); complete
contract and verification are recorded in `socket-socket-21072026.md`.

Depends on: `net/address`, `time/duration`, `errors`. (Concrete socket
resources — and with them `runtime/net`/`runtime/poll` — belong to
`socket/tcp`/`socket/udp`/`socket/unix`; this module is pure shared
vocabulary with no native surface of its own.)

- role `Socket` — `local() is SocketAddress`;
- enums `AddressReuse`, `KeepAlivePolicy`, `PacketDelay` (each `enabled`/
  `disabled`) — explicit socket option choices rather than boolean toggles;
- `SocketOptions(address_reuse is AddressReuse,
  keep_alive is KeepAlivePolicy, packet_delay is PacketDelay)` —
  queries expose those three typed policies;
- `SocketTimeout(connection is Duration, input is Duration, output is
  Duration)` — `connection() is Duration`, `input() is Duration`,
  `output() is Duration`;
- `SocketClosed` — fixed text; `SocketFailure(reason is string)` — retains
  and exposes the given reason, for `socket/tcp`/`udp`/`unix` to throw with
  their own translated native-failure messages once they exist.

#### `socket/tcp`

Purpose: TCP connections and listeners as stream I/O objects.  
Implementation status: **stdlib** (`stdlib/socket/tcp.ok`); complete
contract and verification are recorded in `socket-tcp-21072026.md`.

Depends on: `socket/socket`, `runtime/net`, `runtime/memory`, `io/input`,
`io/output`, `io/closeable`, `net/address`, `net/dns`, `errors`. (No
`time/duration` — see the `TcpConnection` note below: the native surface
has no way to bound an individual blocking `connect()` call, so no
timeout is accepted or enforced.)

- `TcpSocket(address is SocketAddress)` (creates, does not yet connect) or
  `TcpSocket(handle is SocketHandle, peer is SocketAddress)` (wraps an
  already-connected handle, used internally by `TcpListener.socket()`) —
  implements `Socket`, `Input`, `Output`, `Closeable`; `connect()`,
  `peer() is SocketAddress`, `shutdown_input()`, `shutdown_output()`,
  `bytes(limit is int) is @byte`, `write(value is @byte)`, `close()`;
- `TcpListener(address is SocketAddress)` — implements `Socket`,
  `Closeable`; `socket() is TcpSocket` (blocks on `accept()`),
  `address() is SocketAddress` (the real OS-assigned bound address, not
  the constructor's request — matters when port `0` was requested),
  `close()`;
- `TcpConnection(resolver is Resolver, host is HostPort)` — resolves
  `host` and tries every candidate address in order, returning the first
  one that connects; `#projected socket() is TcpSocket`;
- `TcpFailure` — `text() is string`.

`TcpConnection` does **not** accept or enforce a connection timeout: the
`runtime/net` native surface exposes no way to make a socket non-blocking
before `connect()`, so there is no way to bound an individual blocking
attempt from Okkama-level code. Only "ordered address attempts" (the
other half of the draft's stated purpose) is implemented; this is a
documented native-surface gap, not a silently-ignored parameter.

#### `socket/udp`

Purpose: UDP datagram sending and receiving.  
Implementation status: **stdlib** (`stdlib/socket/udp.ok`); complete
contract and verification are recorded in `socket-udp-21072026.md`.

Depends on: `socket/socket`, `runtime/net`, `runtime/memory`, `io/input`,
`io/output`, `io/closeable`, `net/address`, `errors`.

- `Datagram(from is SocketAddress, value is @byte)` — `source() is
  SocketAddress`, `bytes() is @byte`;
- `UdpSocket(address is SocketAddress)` — implements `Socket`,
  `Closeable`; `send(target is SocketAddress, value is @byte)` (throws
  `io/output.IncompleteWrite` on a short send), `datagram(limit is int)
  is Datagram` (throws `io/input.InvalidReadSize` for a non-positive
  limit), `connect(peer is SocketAddress)`, `close()`;
- `UdpFailure` — `text() is string`.

`UdpSocket.connect()` sets the OS-level default peer for the socket; it
does not change `send()`'s own signature, which always takes an explicit
target. Calling `sendto()`-style `send()` again on a socket that has
already been `connect()`-ed to the *same* peer is a real, confirmed
native/OS-level failure on this platform (not an Okkama bug) — see the
contract record for the direct raw-native reproduction. `connect()` and
explicit-target `send()` are two independent capabilities meant to be
used one at a time on a given socket, not combined.

#### `socket/unix`

Purpose: local Unix-domain stream sockets with explicit unsupported-host failure.  
Implementation status: **stdlib** (`stdlib/socket/unix.ok`); complete
contract and verification are recorded in `socket-unix-21072026.md`.

Depends on: `socket/socket`, `runtime/net`, `runtime/memory`, `fs/path`,
`io/input`, `io/output`, `io/closeable`, `errors`.

- `UnixSocket(path is PathValue)` — implements `Socket`, `Input`,
  `Output`, `Closeable`; `connect()`, `bytes(limit is int) is @byte`,
  `write(value is @byte)`, `close()`;
- `UnixListener(path is PathValue)` — implements `Closeable`; `socket()
  is UnixSocket`, `path() is Path`, `close()`;
- `UnsupportedUnixSocket` — `text() is string`.

This module fails explicitly on hosts without Unix-domain sockets — which,
as of this implementation, is *every* host: the current `runtime/net`
native provider has no `AF_UNIX` support at all (confirmed directly —
`_host_family` in `Okkama::Runtime::Native::Core::Net` only maps
`ipv4`/`ipv6`; attempting `socket(3, 1, 0)` cleanly returns a
`NativeFailure` with code `unsupported`, not a crash). Both constructors
attempt the real native call and translate that specific failure into
`UnsupportedUnixSocket`; every other action exists to satisfy its role
structurally but can never actually execute, since no instance can ever
be constructed on this substrate. This is a genuine, universal substrate
gap today, not a host-specific edge case — a future native provider
update adding `AF_UNIX` support (and a way to encode a filesystem path
into a `bind`/`connect`-ready address envelope, which also doesn't exist
yet) would need no changes to this module's already-correct-shaped
`connect()`/`bytes()`/`write()`/`socket()`/`path()` signatures.

### 6.8 HTTP

#### `http/status`

Purpose: validated HTTP status codes and status-class behaviour.  
Implementation status: **stdlib** (`stdlib/http/status.ok`); complete
contract and verification are recorded in `http-status-21072026.md`.

Depends on: `errors`.

- `Status(code is int, reason is string)` — `code() is int`,
  `reason() is string`, `informational() is bool`, `successful() is bool`,
  `redirect() is bool`, `client_error() is bool`, `server_error() is bool`;
  validates `code` is in `100..599` with a non-empty `reason`;
- named zero-arg objects for standard statuses — `Ok`, `Created`,
  `BadRequest`, `NotFoundStatus`, `InternalServerError` — each projects
  (`status Ok()`, ...) into a fully working `Status`, so the classification logic
  lives once, in `Status`, not duplicated per named status;
- `InvalidHttpStatus` — `text() is string`.

#### `http/headers`

Purpose: immutable case-insensitive HTTP header collections.  
Implementation status: **stdlib** (`stdlib/http/headers.ok`); complete
contract and verification are recorded in `http-headers-21072026.md`.

Depends on: `collections/list`, `errors`. (No `string/trim` module exists;
the builtin `.trim()` string action is what `Header`'s constructor uses to
strip optional surrounding whitespace from a value, per RFC 7230's OWS
rule — corrected from the draft's nonexistent-module dependency line.)

- `HeaderName(name is string)` — `string() is string`; rejects an empty
  name or one containing whitespace, `:`, `\r`, or `\n`, as
  `InvalidHeaderName`;
- `Header(name is HeaderName, value is string)` — `name() is HeaderName`,
  `value() is string`; trims `value`'s surrounding whitespace, then rejects
  a trimmed value containing `\r`/`\n` (header/response-splitting
  injection) as `InvalidHeaderValue`;
- `Headers(entries is @Header)` — `first(name is HeaderName) is string`,
  `all(name is HeaderName) is @string`, `has(name is HeaderName) is bool`,
  `entries() is @Header`; lookups compare names case-insensitively;
  `first` throws `MissingHeader` when nothing matches;
- `ExtendedHeaders(source is Headers, header is Header)` —
  `#projected headers() is Headers` appends one header, keeping the source's
  existing entries and order;
- `FilteredHeaders(source is Headers, name is HeaderName)` —
  `#projected headers() is Headers` removes every entry matching `name`
  case-insensitively;
- `InvalidHeaderName`, `InvalidHeaderValue`, `MissingHeader` —
  `text() is string`.

Header names compare case-insensitively without destroying their original
spelling.

#### `http/request`

Purpose: immutable HTTP request method, URL, headers and body.  
Implementation status: **stdlib** (`stdlib/http/request.ok`); complete
contract and verification are recorded in `http-request-21072026.md`.

Depends on: `http/headers`, `net/url`, `io/input`, `io/buffer` (test-only),
`errors`.

- enum `Method` — get, head, post, put, patch, delete, options, connect, trace;
- role `Request` — `method() is Method`, `url() is Url`,
  `headers() is Headers`, `body() is Input`;
- `HttpRequest(method is Method, url is Url, headers is Headers,
  body is Input)` — implements `Request`;
- `Get`, `Post`, `Put`, `Patch`, `Delete` — each takes `(url is Url,
  headers is Headers, body is Input)` and fixes its own `Method`,
  delegating everything else to an internally held `HttpRequest`;
- `InvalidRequest` — `text() is string`; exported vocabulary with no
  internal throw site in this module (see below).

#### `http/response`

Purpose: streamed HTTP responses and explicit body materialization.  
Implementation status: **stdlib** (`stdlib/http/response.ok`); complete
contract and verification are recorded in `http-response-21072026.md`.

Depends on: `http/status`, `http/headers`, `io/input`, `io/buffer`
(test-only), `encoding/utf8`, `encoding/json`, `errors`.

- role `Response` — `status() is Status`, `headers() is Headers`,
  `body() is Input`;
- `HttpResponse(status is Status, fields is Headers, content is Input)` —
  implements `Response`;
- `ResponseBytes(value is Response)` — `#projected bytes() is @byte`; reads the
  whole body via one bounded 16 MiB read plus a confirm-empty follow-up
  read, throwing `InvalidResponseBody` if the body turns out to be
  larger than that cap (never silently truncated);
- `ResponseString(value is Response)` — `#projected string() is string`; decodes
  `ResponseBytes`' result as UTF-8, translating `InvalidUtf8` into
  `InvalidResponseBody`;
- `ResponseJson(value is Response)` — `#projected json() is Json`; parses
  `ResponseString`'s result, translating `InvalidJson` into
  `InvalidResponseBody`;
- `InvalidResponseBody` — `text() is string`.

Body materialization is explicit because it may consume a stream and
allocate an unbounded value — "unbounded" is capped at 16 MiB in this
implementation (matching the same generous-but-finite cap
`runtime/tls`'s native provider already uses for `tls_read`), with a
typed, catchable failure rather than either silent truncation or a
genuinely unbounded allocation.

#### `http/handler`

Purpose: role-based request handling and composable handler chains.  
Implementation status: **stdlib** (`stdlib/http/handler.ok`).
Depends on: `http/request`, `http/response`.

- handler boundary — `async (Request) is Response`; a public bound object action
  such as `controller.response` retains its DI receiver and satisfies it;
- `HandlerFake(response is Response)` — configured deterministic fixture whose
  bound `response` action always returns the constructed response;
- `HandlerChain(handlers is @async (Request) is Response)` — `async response(request is Request) is
  Response` tries each handler in order and returns the first one that
  returns normally; a handler signals "not mine, try the next" by throwing
  `HandlerFailure` rather than returning a response, giving `HandlerFailure`
  a concrete "chain of responsibility" purpose rather than leaving it as
  unused vocabulary; `HandlerChain` itself throws `HandlerFailure` if every
  handler in the list declines (including an empty list);
- `HandlerFailure` — `text() is string`.

#### `http/router`

Purpose: method/path dispatch from requests to handler objects.  
Implementation status: **stdlib** (`stdlib/http/router.ok`).  
Depends on: `http/handler`, `http/request`, `http/response`, `http/status`,
`http/headers`, `io/buffer`, `net/url`, `collections/list`.

- `Router(routes is @Route)` — `async response(request is Request) is
  Response` tries each route in order and dispatches to the first whose
  `matches(request)` is true; throws `MissingRoute` if none match;
- role `Route` — `matches(request is Request) is bool`,
  `async response(request is Request) is Response`;
- `GetRoute(pattern is PathPattern, handler is async (Request) is Response)`,
  `HeadRoute`, `PostRoute`, `PutRoute`, `PatchRoute`, `DeleteRoute`,
  `OptionsRoute`, `ConnectRoute`, `TraceRoute` — method-specific routes, each
  matching on both HTTP method and `PathPattern`, delegating `handle` to the
  wrapped action unchanged (no automatic path-parameter injection — see
  below);
- `PathPattern(pattern is string)` — `matches(path is string) is bool`,
  `parameters(path is string) is @PathParameter` (**not** `%string:string`
  as drafted — see below); a `:name` segment matches any single non-empty
  path segment, every other segment must match literally; throws
  `InvalidRoutePattern` for a pattern with no leading `/` or an empty
  segment;
- `PathParameter` — `name() is string`, `value() is string`;
- `MissingRoute`, `InvalidRoutePattern` — `text() is string`.

Routes hold handler objects. No reflection or global callback registry is
required.

**Deviation from the draft:** `PathPattern.parameters` returns
`@PathParameter`, not the drafted `%string:string`. Okkama has no way to
construct a native map with a runtime-determined number of entries from
pure Okkama code — map literals require a syntactically fixed entry list,
and neither index-assignment nor any incremental "insert" operation exists
for `%K:V` (confirmed directly: `acc[key] = value;` inside a `.reduce()`
callback is a parse error). `@PathParameter` mirrors the exact same solution
`net/url.QueryParameter`/`QueryParameters` already uses for the structurally
identical "dynamic set of name/value pairs" problem. Full rationale in
`docs/todo/http-router-22072026.md`.

#### `http/client`

Purpose: HTTP execution with DNS, TLS and redirect policies.  
Implementation status: **stdlib** (`stdlib/http/client.ok`), HTTP/1.1 only
(no chunked transfer-encoding), TCP only (no real connect/read timeout — see
contract record).  
Depends on: `http/request`, `http/response`, `http/status`, `http/headers`,
`net/url`, `net/address`, `net/dns`, `socket/tcp`, `crypto/tls`, `io/input`,
`io/output`, `io/buffer`, `time/duration`, `fs/path`, `encoding/utf8`,
`encoding/binary`, `collections/list`, `strings/parse`. `errors` is used only
implicitly (`Error` is builtin, needs no import).

- role `HttpClient` — `execute(request is Request) is Response`;
- `ClientPolicy(timeout is Duration, redirects is int, tls is TlsPolicy)` —
  `timeout()`, `redirects()`, `tls()`; `timeout()` is declared and stored but
  not enforced — `runtime/net`'s `connect` exposes no non-blocking-socket
  setup to Okkama code, the exact same documented gap `socket/tcp.
  TcpConnection` already has;
- `Client(policy is ClientPolicy)` — a hand-rolled HTTP/1.1 client: connects
  over `socket/tcp`/`crypto/tls` depending on the URL scheme, serializes the
  request line/headers/body, parses the status line/headers/body
  (`Content-Length`-only; a response with no `Content-Length` is read as
  zero bytes rather than read-until-close), and follows `Location` redirects
  up to `redirects()` times before throwing `TooManyRedirects`;
- `ClientFake(response is Response)` — configured deterministic handler,
  always returns the constructed `response`, mirroring `http/handler.
  HandlerFake`'s reasoning for why this is a real object, not the built-in
  `HttpClient.fake()` role-fake mechanism;
- `HttpClientFailure`, `TooManyRedirects` — `text() is string`.

Full design rationale, the byte-level line/body reading approach, and a real
DNS-resolution timing finding specific to this development sandbox are
recorded in `docs/todo/http-client-22072026.md`.

#### `http/server`

Purpose: accepting HTTP connections and serving a handler/router.  
Implementation status: **stdlib** (`stdlib/http/server.ok`); sequential
(one request at a time), HTTP/1.1, `Content-Length`-only, plain TCP — see
below for why TLS is declared but not wired up.  
Depends on: `http/handler`, `http/request`, `http/response`, `http/status`,
`http/headers`, `net/address`, `net/url`, `socket/tcp`, `crypto/tls`,
`io/input`, `io/output`, `io/buffer`, `encoding/utf8`, `strings/parse`.
Not `http/router`:
`Server` takes any `async (Request) is Response` action. A router is supplied
as `router.response`; no direct coupling is needed.

- `Server(address is SocketAddress, handler is async (Request) is Response, policy is
  ServerPolicy)` — binds and listens immediately in the constructor
  (`TcpListener`); `address() is SocketAddress` reports the real bound
  address (useful when port `0` was requested); `listen()` is a blocking,
  unbounded accept loop and `async serve_one()` accepts one connection,
  parses one HTTP/1.1 request with iterative, operation-local buffers
  (avoiding one interpreted action frame per byte/header/body chunk),
  dispatches it to
  `handler`, writes the response, and closes the connection. There is no
  request counter or artificial request limit. Handlers are async all the way
  through `Router` and `Route`, while the current runtime still executes
  awaited actions sequentially. Future concurrent serving belongs at the
  accepted-connection boundary (`spawn serve_connection`) and does not require
  changing the blocking `listen()` API; a per-request
  error from the handler is caught and reported as `500 Internal Server
  Error` rather than crashing the server (`close()` closes the listener;
  the `HttpServerFailure` accept path still propagates, matching "no
  workflow silently swallows real setup failures");
- `ServerPolicy(backlog is int, tls is TlsPolicy)` —
  `backlog()`, `tls()`; `tls()` is stored and exposed but
  **not enforced** — `crypto/tls.ok` has no server-side TLS wrapper at all
  today (only `tls_client`, never `tls_server`, is used anywhere in that
  module), so there is nothing for `Server` to wire up yet; see the contract
  record;
- `HttpServerFailure` — `text() is string`.

The initial server is sequential, as the draft itself specified. Concurrency
is not smuggled into the stdlib through a hidden scheduler.

Full design rationale — including the verified TLS gap, the byte-level
request/response handling shared in spirit with `http/client.ok`, and how
a real two-process round trip was tested without any concurrency primitive
in Okkama itself — is recorded in `docs/todo/http-server-22072026.md`.

### 6.9 Encoding

#### `encoding/utf8`

Purpose: strict conversion between Okkama strings and UTF-8 bytes.  
Implementation status: **stdlib** (`stdlib/encoding/utf8.ok`).
Depends on: `runtime/utf8` and the language's builtin `Error` role.

- `Utf8Encoded` — `#projected bytes() is @byte`;
- `Utf8Decoded` — `#projected string() is string`;
- `InvalidUtf8` — `text() is string`, `offset() is int`.

`bytes Utf8Encoded(text)` emits canonical UTF-8 without a BOM or normalization.
`string Utf8Decoded(bytes)` first validates the immutable source and never produces
replacement characters. Overlong forms, surrogates, scalars above
`U+10FFFF`, bad continuations and truncated tails throw `InvalidUtf8`; its
offset is the start of the first ill-formed byte sequence.

```okkama
let bytes is @byte = bytes Utf8Encoded("Привет");
let text is string = string Utf8Decoded(bytes);
```

#### `encoding/binary`

Purpose: endian-explicit reading and construction of binary protocols in
ordinary Okkama code.

Implementation status: **stdlib** (`stdlib/encoding/binary.ok`).
Depends on: `collections/queue`, `collections/iteration`, and the language's
builtin `Error` role; no runtime module.

- `BinaryReader` — `size() is int`, `byte(offset is int) is byte`,
  `u16_be(offset is int) is int`, `u16_le(offset is int) is int`,
  `u32_be(offset is int) is int`, `u32_le(offset is int) is int`,
  `i32_be(offset is int) is int`, `i32_le(offset is int) is int`,
  `i64_be(offset is int) is int`, `i64_le(offset is int) is int`,
  `bytes(offset is int, size is int) is @byte`;
- role `BinaryPart` — `bytes() is @byte`;
- `BinaryBytes`, `BinaryByte`, `Unsigned16BigEndian`,
  `Unsigned16LittleEndian`, `Unsigned32BigEndian`, `Unsigned32LittleEndian`,
  `Signed32BigEndian`, `Signed32LittleEndian`, `Signed64BigEndian`, and
  `Signed64LittleEndian` — state-only binary-part objects;
- `BinarySequence(parts is @BinaryPart)` — `#projected bytes() is @byte`;
- `EndOfBinaryData` — `text() is string`, `offset() is int`;
- `InvalidBinaryValue` — `text() is string`.

`BinaryReader` never advances hidden cursor state. Binary output is a tree of
explicit values rather than a writer configured by fluent copy-setters. Each
part retains only its source value and computes checked bytes in `bytes()`;
`BinarySequence` linearly materializes its retained parts. The originally
proposed action name `take` was replaced by `bytes(offset, size)` because
`take` is an import keyword, not a legal action identifier. Protocol-specific
varints, frames and checksums belong to their defining protocol modules.

#### `encoding/base64`

Purpose: canonical base64 encoding and strict decoding.  
Implementation status: **stdlib** (`stdlib/encoding/base64.ok`).
Depends on: `encoding/binary` and the language's builtin `Error` role; no
native module.

- `Base64Encoded` — `#projected string() is string`;
- `Base64Decoded` — `#projected bytes() is @byte`;
- `InvalidBase64` — `text() is string`, `offset() is int`.

The codec implements exactly canonical padded RFC 4648 base64 with the
standard `+` and `/` alphabet. It rejects whitespace, line wrapping,
unpadded input, URL-safe characters, misplaced padding and non-zero unused
tail bits. Every malformed source throws `InvalidBase64` at a deterministic
zero-based character offset.

Base64 remains byte-oriented. Unicode text crosses the explicit UTF-8
boundary first:

```okkama
let encoded is string = string Base64Encoded(bytes Utf8Encoded("Привет"));
let decoded is string = string Utf8Decoded(bytes Base64Decoded(encoded));
```

Both projection objects retain immutable source values. Encoding builds one
short quartet string per three input bytes and joins once; decoding maps each
quartet to a `BinaryBytes` part and materializes their `BinarySequence` once.

#### `encoding/hex`

Purpose: canonical hexadecimal encoding and strict decoding.  
Implementation status: **stdlib** (`stdlib/encoding/hex.ok`).

Depends on: the language's builtin `Error` role; no native module.

- `HexEncoded` — `#projected string() is string`;
- `HexDecoded` — `#projected bytes() is @byte`;
- `InvalidHex` — `text() is string`, `offset() is int`.

Encoding always emits two lowercase `0-9a-f` symbols per byte. Decoding
accepts ASCII lowercase and uppercase hex, normalizing back to lowercase when
re-encoded. Empty input is valid. Whitespace, `0x`, separators, odd lengths
and non-ASCII characters are rejected rather than ignored.

```okkama
let encoded is string = string HexEncoded([byte(222), byte(173), byte(190), byte(239)]);
let decoded is @byte = bytes HexDecoded("DEADBEEF");
```

Odd length reports `offset == text.length()` before character inspection.
Otherwise `InvalidHex.offset()` is the exact zero-based code-point position of
the first invalid character. The codec maps immutable input directly to one
result and has no hidden host or native dependency.

#### `encoding/json`

Purpose: exact JSON values, deterministic rendering and located parsing errors.<br>
Status: **stdlib**.<br>
Depends on: `collections/list`, `encoding/utf8`.

- role `Json` — `encoded() is string`;
- `JsonMember(name is string, value is Json)` — `name() is string`,
  `value() is Json`;
- `JsonObject(members is @JsonMember)` — `value(name is string) is Json`,
  `has(name is string) is bool`,
  `names() is @string`, `encoded() is string`;
- `JsonArray(values is @Json)` — `values() is @Json`,
  `at(index is int) is Json`, `size() is int`, `encoded() is string`;
- `JsonString(value is string)` — `value() is string`,
  `encoded() is string`;
- `JsonNumber(value is string)` — retains an exact validated-on-query JSON
  number lexeme; `int() is int`, `float() is float`,
  `encoded() is string`;
- `JsonBool(value is bool)` — `#projected bool() is bool`, `encoded() is string`;
- `JsonNull()` — `encoded() is string`; JSON syntax value only, never Okkama
  `undefined`;
- `JsonSource(value is string)` — `#projected json() is Json`;
- `InvalidJson` — `text() is string`, `line() is int`, `column() is int`,
  `offset() is int`;
- `InvalidJsonNumber` — programmatic number grammar failure with
  `text() is string`, `offset() is int`;
- `InvalidJsonNumberConversion` — a valid exact JSON number has no requested
  bounded Okkama `int`/`float` projection; `text() is string`;
- `MissingJsonMember`, `DuplicateJsonMember` — `text() is string`,
  `name() is string`;
- `MissingJsonElement` — `text() is string`, `index() is int`.

```okkama
import "encoding/json" take
    Json, JsonMember, JsonObject, JsonArray,
    JsonString, JsonNumber, JsonBool, JsonNull, JsonSource;

main() is @string {
    let values is @Json = [
        JsonString("Ada"),
        JsonNumber("1e400"),
        JsonBool(true),
        JsonNull(),
    ];
    let generated = JsonObject([
        JsonMember("values", JsonArray(values)),
    ]);
    let parsed is Json = json JsonSource(
        "{\"emoji\":\"\\uD83D\\uDE00\",\"count\":3}",
    );
    [generated.encoded(), parsed.encoded()];
}
```

JSON numbers are not forced through Okkama `int` or `float` while parsing.
Their strict RFC 8259 decimal spelling is retained verbatim, so finite JSON
such as `1e400` remains valid and re-encodes exactly. `int()` and `float()` are
explicit fallible projections; conversion failure never invalidates the JSON
value. Objects preserve declaration/source order and reject duplicate names
instead of silently choosing first- or last-wins semantics. Strings accept
valid surrogate pairs, reject lone surrogates and emit compact JSON with
canonical control escapes. `InvalidJson.offset()` is zero-based in the
original Unicode source; `line()`/`column()` are one-based and treat CRLF as
one line break. Constructors retain state only; parsing, validation and
encoding occur in actions, and there is no native JSON ABI.

#### `encoding/csv`

Purpose: streaming CSV rows under an explicit dialect.  
Implementation status: **stdlib** (`stdlib/encoding/csv.ok`); complete
contract and verification are recorded in `encoding-csv-21072026.md`.

Depends on: `io/input`, `io/output`, `io/buffer`, `encoding/utf8`,
`collections/list`, `errors`. (Row/field accumulation was rewritten from
`collections/queue` to `collections/list` after implementation exposed a
real scale bug — see `encoding-csv-21072026.md`'s addendum.)

- `CsvDialect()` (RFC 4180 defaults) or `CsvDialect(delimiter is string,
  quote is string, line_separator is string, header is bool)` —
  `delimiter() is string`, `quote() is string`, `line_separator() is
  string`, `header() is bool`. Drafted as `delimiter()/quote() is char`;
  `char` has no conversion to/from `string` in the current implementation
  (confirmed empirically: no `.string()` action, `+` concatenation and `==`
  comparison both reject mixing `char` with `string`) and is used nowhere
  else in the stdlib, so both return single-character `string` instead,
  matching every other place in the stdlib that already represents a single
  character this way;
- `CsvReader(source is Input, dialect is CsvDialect)` — `rows() is @@string`;
- `CsvWriter(target is Output, dialect is CsvDialect)` — `write(row is
  @string)`, `flush()`;
- `CsvRow(header is @string, values is @string, line is int)` — `at(index is
  int) is string`, `value(name is string) is string`;
- `InvalidCsv` — implements `LocatedError`; `text() is string`.

`CsvReader` drains its `Input` fully before parsing (the same "read to
completion, then process" shape `crypto/digest`'s streaming constructor
uses) rather than parsing incrementally against arriving chunks; `rows()`'s
result is a fully materialized `@@string` regardless. Quoted fields follow
RFC 4180: a doubled quote (`""`) is a literal quote inside a quoted field,
and any content between a closing quote and the next delimiter/line
separator is rejected. A trailing line separator at end-of-input does not
produce a spurious empty final row; a genuinely blank line parses as one row
holding one empty field. `CsvWriter` quotes a field only when it actually
contains the delimiter, the quote character, or the line separator.
`CsvRow` is independent of `CsvReader`: it pairs an already-read header row
with an already-read data row (plus that row's source line, for error
messages) rather than being produced automatically, so `CsvDialect.header()`
is descriptive dialect metadata that calling code acts on, not a flag
`CsvReader` branches on internally.

#### `encoding/toml`

Purpose: exact TOML documents and deterministic parsing/rendering.  
Implementation status: **stdlib** (`stdlib/encoding/toml.ok`); complete
contract, scope reductions and verification are recorded in
`encoding-toml-21072026.md`.

Depends on: `time/date`, `regex`, `strings/parse`, `collections/list`,
`errors`.

- role `TomlValue` — `encoded() is string`;
- `TomlDocument` — `value(path is string) is TomlValue`,
  `has(path is string) is bool`, `string() is string`;
- `TomlEntry(name is string, value is TomlValue)` — `name() is string`,
  `value() is TomlValue`;
- `TomlTable(entries is @TomlEntry)` does `TomlValue` — `value(name is
  string) is TomlValue`, `encoded() is string`;
- `TomlArray` — `values() is @TomlValue`, `encoded() is string`;
- `TomlString`, `TomlInteger`, `TomlFloat`, `TomlBool`, `TomlDateTime` — exact
  values implementing `TomlValue`; each exposes its exact Okkama value and
  `encoded() is string`; `TomlDateTime` wraps `time/date.Date` (TOML's
  Local Date form only — see the contract record for why offset/local
  date-time and local-time-only are out of scope for this version);
- `TomlSource` — `#projected document() is TomlDocument`;
- `InvalidToml` does `Error`, `LocatedError` — `text() is string`.

This implementation deliberately covers a practical subset of TOML v1.0.0,
not the complete grammar: single-line documents only (no multi-line
`"""`/`'''` strings or arrays spanning lines), no `[[array of tables]]`, and
a dotted key inside a `[table]` block is kept as its own literal entry name
rather than exploding into further nested `TomlTable` structure (only
explicit `[table.path]` headers create real nesting). Every reduction is
deliberate and detailed in the contract record, most of them tracing back to
concrete language constraints hit while implementing: `char` has no
conversion to/from `string` in the current implementation (so dialect-style
single-character values use `string` throughout, consistent with
`encoding/csv`), and building a document from a runtime-sized flat entry
list without introducing quadratic-time or unbounded-recursion parsing (both
found and fixed directly, not merely anticipated) shaped most of the
internal design.

### 6.10 Cryptography

#### `crypto/random`

Purpose: the minimal operating-system secure byte source.  
Implementation status: **stdlib** (`stdlib/crypto/random.ok`).

Depends on: `runtime/system` and the language's builtin `Error` role.

- role `SecureByteSource` — `bytes(size is int) is @byte`; its test-only fake
  returns an exact deterministic zero-byte sequence;
- `SecureRandom` — implements `SecureByteSource`;
  `bytes(size is int) is @byte`;
- `InvalidSecureRandomSize` — `text() is string`, `requested() is int`,
  `maximum() is int`;
- `UnavailableSecureRandom` — `text() is string`.

`SecureRandom()` is stateless: size belongs to each query. Requests are
limited to `0..16777216`; zero succeeds without touching the entropy device,
while invalid sizes fail before native dispatch. Expected provider failures
are translated to `UnavailableSecureRandom` without exposing native messages,
and there is no pseudo-random fallback.

```okkama
let source is SecureByteSource = SecureRandom();
let nonce is @byte = source.bytes(32);
```

This is the primitive secure source. General ranges and sampling belong to the
top-level `random` module.

#### `crypto/digest`

Purpose: reviewed cryptographic digests over bytes and streams.  
Implementation status: **stdlib** (`stdlib/crypto/digest.ok`); complete
contract and verification are recorded in `crypto-digest-21072026.md`.

Depends on: `runtime/crypto`, `encoding/hex`, `encoding/base64`, `io/input`,
`io/buffer`, `errors`.

- role `Digest` — `bytes() is @byte`, `hex() is string`, `base64() is string`;
- `Sha256`, `Sha384`, `Sha512` — implement `Digest`; each has a
  `(value is @byte)` constructor and a `(source is Input, chunk_size is int)`
  streaming constructor that drains the input to completion before hashing;
- `DigestMismatch` — `text() is string`; exported vocabulary for callers
  verifying a computed digest against an expected value
  (`confirm computed.bytes() == expected else throw DigestMismatch();`);
- `UnsupportedDigest` — `text() is string`; the sanitized translation of any
  native digest-provider failure.

Weak historical algorithms are not included by default merely for
completeness. Okkama constructor selection is arity-based, not type-based, so
the bytes and streaming constructors could not share one arity; the streaming
form's explicit `chunk_size` both resolves that and lets callers bound
per-read memory use, mirroring `io/buffer`'s `InputBuffering(source, size)`.

#### `crypto/hmac`

Purpose: keyed message authentication and constant-time verification.  
Implementation status: **stdlib** (`stdlib/crypto/hmac.ok`); complete
contract and verification are recorded in `crypto-hmac-21072026.md`.

Depends on: `runtime/crypto`, `crypto/digest`, `encoding/hex`, `errors`.

- `HmacSha256(key is @byte, value is @byte)`, `HmacSha384`, `HmacSha512` —
  `bytes() is @byte`, `hex() is string`, `matches(expected is @byte) is
  bool`;
- `InvalidMacKey` — `text() is string`.

`matches` calls `runtime/crypto.secure_equal`, the same constant-time
comparison `runtime/crypto` was reviewed for; digests are not secret and so
`crypto/digest` has no equivalent, but a MAC's whole purpose is verifying
against a value an attacker may control, where a length/timing-leaking
comparison could help forge one. `HmacSha256`/`384`/`512` always call
`runtime/crypto.hmac` with one of their own three fixed algorithm strings,
so — exactly as in `crypto/digest` — any native failure is a provider
invariant violation, never a caller mistake; this module depends on
`crypto/digest` specifically to reuse its `UnsupportedDigest` for that
translation rather than declaring a redundant duplicate. `InvalidMacKey`
rejects an empty key before native dispatch.

#### `crypto/tls`

Purpose: TLS policy, handshakes, encrypted sockets and peer certificates.  
Implementation status: **stdlib** (`stdlib/crypto/tls.ok`); complete
contract and verification are recorded in `crypto-tls-21072026.md`.

Depends on: `runtime/net`, `runtime/tls`, `socket/socket`, `net/address`,
`io/input`, `io/output`, `fs/path`, `time/instant`, `errors`. (No
`socket/tcp`, no `time/duration` — see below: `TlsSocket` establishes its
own raw TCP connection directly via `runtime/net` rather than composing
an existing `socket/tcp.TcpSocket`, since that object's underlying
`SocketHandle` is intentionally never exposed publicly.)

- enum `PeerVerification` — required, disabled;
- `CertificateAuthority(location is Path)` — `path() is Path`;
- `ServerIdentity(value is string)` — `name() is string`;
- `TlsPolicy(authority is CertificateAuthority,
  server_identity is ServerIdentity, verification is PeerVerification)` —
  queries expose the three typed policy objects;
- `TlsSocket(address is SocketAddress, policy is TlsPolicy)` — implements
  `Socket`, `Input`, `Output`; `handshake()`,
  `peer_certificate() is Certificate` (throws `CertificateFailure` before
  a completed handshake), `close()`;
- `Certificate(subject, issuer, from is Instant, until is Instant,
  encoded is @byte)` — `subject() is string`, `issuer() is string`,
  `valid_from() is Instant`, `valid_until() is Instant`,
  `der() is @byte`;
- `TlsFailure`, `CertificateFailure` — `text() is string`.

`TlsSocket`'s constructor creates its own TCP socket and connects it
directly (via `runtime/net`, duplicating the small amount of connect
logic `socket/tcp.ok` already has, the same "duplicate rather than
couple to another module's private internals" precedent `socket/udp.ok`
and `net/dns.ok` already established), then wraps that raw handle with
`runtime/tls.tls_client`. `CertificateAuthority.path().bytes()` is passed
directly as `tls_client`'s `ca_file` parameter — that parameter is a
byte-encoded *file path* (matching `runtime/fs`'s `path is @byte`
convention), not inline PEM content, confirmed by reading
`Okkama::Runtime::Native::Core::TLS`'s handler directly. An empty path
means "use the system default CA bundle."

The current native TLS provider requires `IO::Socket::SSL`, which is not
installed in this development environment (the same reason
`t/interpreter/native/53-native-tls.t` already fails here, unrelated to
any change in this session) — every real TLS operation cleanly returns a
`NativeFailure` with code `unsupported` rather than crashing, confirmed
directly, and this module translates that into a typed `TlsFailure`. Full
handshake/certificate behavior is implemented per the native contract but
can only be exercised end-to-end in an environment with `IO::Socket::SSL`
installed.

### 6.11 SQL and databases

#### `sql/value`

Purpose: exact bindable SQL scalar values, including explicit SQL null.  
Implementation status: **stdlib** (`stdlib/sql/value.ok`); complete
contract and verification are recorded in `sql-value-21072026.md`.

Depends on: `time/date`, `time/instant`, `errors`.

- enum `SqlType` — integer, float, string, bytes, boolean, date, datetime,
  `null_value` (renamed from the draft's `null`: `null` is a reserved
  keyword in Okkama);
- role `SqlValue` — `sql_type() is SqlType`;
- `SqlInt` — `sql_type() is SqlType`, `int() is int`;
- `SqlFloat` — `sql_type() is SqlType`, `float() is float`;
- `SqlString` — `sql_type() is SqlType`, `string() is string`;
- `SqlBytes` — `sql_type() is SqlType`, `bytes() is @byte`;
- `SqlBool` — `sql_type() is SqlType`, `bool() is bool`;
- `SqlDate` — `sql_type() is SqlType`, `date() is Date`;
- `SqlDateTime` — `sql_type() is SqlType`, `datetime() is DateTime`;
- `SqlNull` — `sql_type() is SqlType`;
- `SqlValues(values is @SqlValue)` — `values() is @SqlValue`;
- `UnsupportedSqlValue` — `text() is string`.

`SqlNull` is an explicit database value and never implicit Okkama
`undefined`.

#### `sql/statement`

Purpose: immutable data-changing SQL statements with bound parameters.  
Implementation status: **stdlib** (`stdlib/sql/statement.ok`); complete
contract and verification are recorded in `sql-statement-21072026.md`.

Depends on: `sql/value`, `sql/dialect`, `errors`.

- role `Statement` — `sql(dialect is Dialect) is string`,
  `values() is SqlValues`;
- `RawStatement(source is string, parameters is SqlValues)` — explicit
  reviewed SQL with bound values; `sql()` ignores the dialect entirely;
- `Insert(table is string, columns is @string, parameters is SqlValues)`,
  `Update(table is string, columns is @string, parameters is SqlValues)`
  — immutable statement objects; both validate `columns.size() ==
  parameters.values().size()` at construction, else `InvalidStatement`;
  `Update` additionally requires at least one column;
- `Delete(table is string)` — unconditional (whole-table); `values()` is
  always empty;
- `InvalidStatement` — `text() is string`.

`Update`/`Delete` are deliberately unconditional (no `WHERE` clause) in
this version: no predicate/condition abstraction exists anywhere in this
draft yet (`sql/query`'s `Select` only ever takes the single `EveryRow`
predicate value, with no other implementer defined) — row-level
filtering for data-changing statements is out of scope until one exists.

#### `sql/query`

Purpose: immutable row-producing SQL queries with bound parameters.  
Implementation status: **stdlib** (`stdlib/sql/query.ok`); complete
contract and verification are recorded in `sql-query-21072026.md`.

Depends on: `sql/value`, `sql/dialect`, `errors`. (No `sql/statement` —
`sql/query` doesn't reuse anything from it; the draft's dependency line
was likely a copy from `sql/statement`'s own draft entry.)

- role `Query` — `sql(dialect is Dialect) is string`,
  `values() is SqlValues`;
- role `Rows` (internal, not in the original draft) —
  `clause() is string`; the shared shape `UnlimitedRows`/`RowWindow` both
  implement, letting `Select` render its row-window suffix without
  branching on which one it was given;
- `RawQuery(source is string, parameters is SqlValues)` — explicit
  reviewed SQL with bound values; `sql()` ignores the dialect entirely;
- `Select(columns is @ColumnName, table is TableName, matching is
  EveryRow, rows is Rows)` — state-only construction from its
  projection, table, predicate and row-window objects; implements
  `Query` directly; throws `InvalidQuery` for an empty projection;
- `EveryRow`, `UnlimitedRows`, and `RowWindow(limit is int, offset is
  int)` — explicit values for predicate, limit and offset policy; no
  partially configured query exists; `RowWindow` throws `InvalidQuery`
  for a negative limit or offset;
- `TableName`, `ColumnName` — `string() is string`; both throw
  `InvalidSqlIdentifier` for an empty value;
- `InvalidQuery`, `InvalidSqlIdentifier` — `text() is string`.

`EveryRow` is `Select`'s only concrete predicate value in this draft —
there is no `WHERE`/condition role or any other implementer defined
anywhere, so `Select` takes it as a plain concrete type, not a role.

#### `sql/dialect`

Purpose: reviewed differences in placeholders, identifiers and SQL rendering.  
Implementation status: **stdlib** (`stdlib/sql/dialect.ok`); complete
contract and verification are recorded in `sql-dialect-21072026.md`.

Depends on: `errors`. (No `sql/value` — none of this module's own actions
touch `SqlValue`; that dependency belongs to `sql/statement`/`sql/query`,
which bind values *through* a `Dialect`, not this module itself.)

- role `Dialect` — `placeholder(index is int) is string`,
  `identifier(value is string) is string`, `boolean(value is bool) is string`;
- `SqliteDialect`, `PostgresDialect`, `MysqlDialect` — concrete dialects:
  `?`/`?`/`$N` (1-based) placeholders respectively; `"..."`-quoted (SQLite,
  Postgres) vs. `` `...` ``-quoted (MySQL) identifiers, each doubling an
  embedded quote character to escape it; `1`/`0` (SQLite, MySQL) vs.
  `TRUE`/`FALSE` (Postgres) boolean literals;
- `UnsupportedSqlFeature` — `text() is string`; thrown by all three
  concrete dialects for a non-positive `placeholder` index, and exported
  for a future dialect to throw for any SQL feature it genuinely can't
  render.

#### `db/driver`

Purpose: the DBI-style contract implemented by database-specific Okkama
drivers.  
Implementation status: **stdlib** (`stdlib/db/driver.ok`) — the role and
error type only; every concrete `Driver` implementer is a database driver
(`db/sqlite`/`db/postgres`/`db/mysql`, Phase 9, still ahead), for the same
reason `db/connection`/`db/transaction` have no concrete implementation:
wire-protocol/file-format specifics are inherently database-specific.  
Depends on: `db/connection`, `sql/dialect`, `sql/statement`, `sql/query`,
`db/result`, `db/row`, `db/transaction`, `errors`.

- role `Driver` — `connect() is Connection`, `dialect() is Dialect`;
- `DriverFailure` — `text() is string`.

Drivers are imported and composed explicitly. There is no registry keyed by a
driver name, reflective loading or host-provided database object. Demonstrated
by this module's own embedded-test-only `FixtureDriver`/`FixtureConnection`
fixtures (not part of the public API), reusing `SqliteDialect` (already real,
from `sql/dialect`) rather than inventing a third fake dialect.

#### `db/database`

Purpose: stable application-facing database facade composed with a driver.  
Implementation status: **stdlib** (`stdlib/db/database.ok`) — a thin facade;
concrete `Driver`s are still-ahead database drivers.  
Depends on: `db/driver`, `db/connection`, `sql/dialect`, `sql/statement`,
`sql/query`, `db/result`, `db/row`, `db/transaction`, `errors`.

- `Database(driver is Driver)` — `connect() is Connection`,
  `dialect() is Dialect`;
- `DatabaseUnavailable` — `text() is string`.

`connect()` delegates straight to the wrapped `Driver`, catching only
`DriverFailure` and re-throwing it as `DatabaseUnavailable` with the same
text, so application code depends on one stable failure type regardless of
which driver is plugged in; `dialect()` delegates directly with no
translation, since dialect lookup itself cannot fail. Demonstrated by this
module's own embedded-test-only `FixtureDriver`/`FixtureConnection`/
`FailingDriver` fixtures (not part of the public API).

#### `db/connection`

Purpose: executing statements/queries and opening transactions.  
Implementation status: **stdlib** (`stdlib/db/connection.ok`) — the role and
error types only; every concrete `Connection` implementer is a database
driver (Phase 9, still ahead), for the same reason `db/transaction` has no
concrete implementation: connection/protocol state is inherently
database-specific.  
Depends on: `sql/query`, `sql/statement`, `sql/value`, `db/row`, `db/result`,
`db/transaction`, `runtime/memory` (only for this contract record's own test
fixtures, not for the role/error declarations themselves).

- role `Connection` — `execute(statement is Statement) is DbResult`,
  `query(query is Query) is @Row`, `transaction() is Transaction`,
  `close()`, `closed() is bool`;
- `ConnectionClosed`, `DbFailure` — `text() is string`.

The role owns no universal concrete connection. Each driver keeps its concrete
connection and protocol state private to its own module. Demonstrated by
this module's own embedded-test-only `FixtureConnection`/`FixtureTransaction`
fixtures (not part of the public API), mirroring `db/transaction.ok`'s own
`FakeTransaction` precedent.

#### `db/transaction`

Purpose: explicit atomic database work with rollback-on-destruction.  
Implementation status: **stdlib** (`stdlib/db/transaction.ok`) — the role and
error type only; every concrete `Transaction` implementer is a database
driver (`db/sqlite`/`db/postgres`/`db/mysql`, Phase 9, still ahead), since
transaction framing (`BEGIN`/`COMMIT`/`ROLLBACK` or a wire-protocol
equivalent) is inherently database-specific.  
Depends on: `sql/query`, `sql/statement`, `sql/value`, `db/row`, `db/result`,
`runtime/memory` (only for this contract record's own test fixture, not for
the role/error declarations themselves).

- role `Transaction` — `execute(statement is Statement) is DbResult`,
  `query(query is Query) is @Row`, `commit()`, `rollback()`,
  `active() is bool`;
- `InactiveTransaction` — `text() is string`.

Destroying an active transaction rolls it back; it never commits implicitly.
This module cannot enforce that contract itself (there is no concrete
connection to roll back yet); it is documented here and demonstrated by a
small internal fixture (`FakeTransaction`, this module's own embedded test
double, not part of the public contract) for the driver modules that will
implement the role for real to follow.

#### `db/row`

Purpose: typed access to one database result row.  
Implementation status: **stdlib** (`stdlib/db/row.ok`); complete contract
and verification are recorded in `db-row-21072026.md`.

Depends on: `sql/value`, `errors`.

- `Row(names is @string, values is @SqlValue)` — `value(name is string) is
  SqlValue` and `at(index is int) is SqlValue` both throw `MissingColumn`
  when nothing matches; `names() is @string`, `size() is int`;
- `MissingColumn`, `InvalidColumnType` — `text() is string`.
  `MissingColumn` is thrown directly by `Row`; `InvalidColumnType` is
  exported vocabulary for a future domain adapter to throw once it
  inspects a looked-up `SqlValue`'s `sql_type()` and finds it doesn't
  match what the adapter expected — `Row` itself never throws it (same
  "exported, not internally thrown" shape as `crypto/digest.DigestMismatch`).

Domain adapters such as `UserRow(row).user()` own type expectations; `Row`
does not reflectively construct arbitrary objects.

#### `db/result`

Purpose: non-row database execution outcome.  
Implementation status: **stdlib** (`stdlib/db/result.ok`); complete
contract and verification are recorded in `db-result-21072026.md`.

Depends on: `errors`.

- `DbResult(affected is int, last_id is int)` or `DbResult(affected is
  int)` — two arities: the two-argument constructor records a real last
  insert id, the one-argument constructor is for statements that never
  produce one; `affected() is int`, `last_id() is int` (throws
  `MissingLastId` unless constructed with one), `has_last_id() is bool`;
- `MissingLastId` — `text() is string`.

#### `db/sqlite`

Purpose: SQLite-compatible driver implemented in Okkama over filesystem
syscalls and the SQLite file/locking protocols.  
Implementation status: **deferred** — explicitly out of scope for this pass;
see `db-sqlite-22072026.md`. A real, byte-for-byte-compatible driver (pager,
B-tree with overflow pages, rollback journal and WAL, POSIX advisory
locking) plus a genuine SQL tokenizer/parser/executor for arbitrary text
(there is no structured-AST shortcut — `RawStatement`/`Insert`/`Select` all
render down to plain SQL strings via `.sql(dialect)`) is a multi-week
undertaking on its own, categorically larger than every other module
implemented this session, which wrapped existing OS/runtime primitives
rather than implementing a storage/query engine from first principles. The
design below is kept as-is for whenever this is picked back up.

Depends on: `db/driver`, `db/connection`, `db/transaction`, `db/row`,
`db/result`, `sql/query`, `sql/statement`, `sql/dialect`, `fs/path`,
`runtime/fs`, `encoding/binary`, `time/duration`, `errors`.

- enum `SqliteJournal` — rollback, wal;
- `Sqlite` — implements `Driver`; `connect() is Connection`,
  `dialect() is Dialect`;
- `SqliteOptions` — state-only composition of busy timeout, journal and a
  typed foreign-key policy;
- `SqliteFailure`, `SqliteCorrupt`, `SqliteBusy` — `text() is string`.

The driver owns its Okkama pager, journal/WAL, B-tree, SQL execution and
locking code. It may call only general filesystem ABI operations; it never
calls a host SQLite library.

#### `db/postgres`

Purpose: PostgreSQL wire-protocol driver implemented in Okkama over TCP.  
Implementation status: **deferred** alongside `db/sqlite`; see
`db-sqlite-22072026.md` for the shared rationale.

Depends on: `db/driver`, `db/connection`, `db/transaction`, `db/row`,
`db/result`, `sql/query`, `sql/statement`, `sql/dialect`, `net/address`,
`socket/tcp`, `crypto/tls`, `crypto/digest`, `crypto/hmac`, `encoding/binary`,
`encoding/utf8`, `time/duration`, `errors`.

- `Postgres` — implements `Driver`; `connect() is Connection`,
  `dialect() is Dialect`;
- `PostgresOptions` — state-only composition of credentials, database name,
  TLS policy, connect timeout and application identity;
- `PostgresFailure`, `PostgresAuthenticationFailure`,
  `PostgresProtocolFailure` — `text() is string`.

Framing, startup, authentication, parameter binding and row decoding are
Okkama code. The host sees socket, TLS and cryptographic primitive calls only.

#### `db/mysql`

Purpose: MySQL wire-protocol driver implemented in Okkama over TCP.  
Implementation status: **deferred** alongside `db/sqlite`; see
`db-sqlite-22072026.md` for the shared rationale.

Depends on: `db/driver`, `db/connection`, `db/transaction`, `db/row`,
`db/result`, `sql/query`, `sql/statement`, `sql/dialect`, `net/address`,
`socket/tcp`, `crypto/tls`, `crypto/digest`, `crypto/hmac`, `encoding/binary`,
`encoding/utf8`, `time/duration`, `errors`.

- `Mysql` — implements `Driver`; `connect() is Connection`,
  `dialect() is Dialect`;
- `MysqlOptions` — state-only composition of credentials, database name, TLS
  policy, connect timeout and a typed character set;
- `MysqlFailure`, `MysqlAuthenticationFailure`, `MysqlProtocolFailure` —
  `text() is string`.

Handshake, authentication, packet framing, prepared statements and row
decoding are Okkama code. No MySQL client library is exposed through runtime.

#### `db/migration`

Purpose: ordered transactional schema changes and migration history.  
Implementation status: **stdlib** (`stdlib/db/migration.ok`) — a real,
concrete implementation (unlike `db/connection`/`db/transaction`/
`db/driver`), composed against any `Connection` implementer; complete
contract and verification recorded in `db-migration-22072026.md`, including
a real interpreter bug found and root-caused (not fixed) while building it.  
Depends on: `db/connection`, `db/transaction`, `sql/statement`,
`encoding/utf8`, `collections/list`, `runtime/memory`, `errors`.

- role `Migration` — `version() is string`, `up() is @Statement`,
  `down() is @Statement`;
- `Migrations(migrations is @Migration)` — `pending(connection is
  Connection) is @Migration`, `apply(connection is Connection)`,
  `rollback(connection is Connection, count is int)`; rejects two
  migrations sharing a version at construction time; applies/rolls back
  each migration in its own `Connection.transaction()`, most-recently-applied
  first on rollback; applied-migration history is tracked in the
  `Migrations` instance's own owned `runtime/memory` cell, not read back
  from the database — this stdlib has no runtime role-to-concrete downcast
  from a queried `Row`'s `SqlValue`, and `sql/query`/`sql/statement` have no
  `WHERE`-clause support at all yet, so a durable, cross-process history
  read back through a real driver isn't achievable with today's SQL layer;
- `MigrationFailure`, `DuplicateMigration` — `text() is string`.

### 6.12 Logging and testing

#### `log/event`

Purpose: immutable structured log events and severity levels.  
Implementation status: **stdlib** (`stdlib/log/event.ok`); complete
contract and verification are recorded in `log-event-21072026.md`.

Depends on: `time/instant`, `errors`.

- enum `LogLevel` — trace, debug, info, warning, error, fatal;
- `LogField(name is string, value is string)` — `name() is string`,
  `value() is string`; rejects an empty `name` as `InvalidLogField`;
- `LogEvent(level is LogLevel, message is string, fields is @LogField,
  instant is Instant)` — queries expose those immutable values;
- `InvalidLogField` — `text() is string`.

#### `log/formatter`

Purpose: deterministic line or JSON rendering of log events.  
Implementation status: **stdlib** (`stdlib/log/formatter.ok`); complete
contract and verification are recorded in `log-formatter-21072026.md`.

Depends on: `log/event`, `time/instant`, `encoding/json`, `errors`.

- role `LogFormatter` — `format(event is LogEvent) is string`;
- `LineFormatter` — `[level] message key=value key2=value2` (bracketed
  level, then message, then space-separated fields; no trailing space
  when there are none); omits the instant — human-oriented, and most
  destinations already timestamp on their own;
- `JsonFormatter` — a complete, deterministic JSON record:
  `{"level":...,"message":...,"instant":...,"fields":{...}}`, with
  `fields` as a nested JSON object keyed by field name (an empty object
  when there are none); machine-oriented, includes everything.

Two duplicate field names in one `LogEvent` surface as
`encoding/json.DuplicateJsonMember` when `JsonFormatter` renders them —
`log/event.LogField` itself only rejects an empty name, not a duplicate
one, so this module doesn't add its own duplicate check; the existing
JSON encoder's own invariant already covers it.

#### `log/logger`

Purpose: injectable destinations for structured log events.  
Implementation status: **stdlib** (`stdlib/log/logger.ok`).  
Depends on: `log/event`, `log/formatter`, `io/output`, `io/buffer`, `fs/file`,
`fs/path`, `time/instant`, `encoding/utf8`, `encoding/base64`,
`collections/list`, `strings/parse`, `runtime/memory`. Not `time/clock`:
nothing here reads the ambient clock, every `LogEvent`'s `Instant` already
comes from the caller.

- role `Logger` — `write(event is LogEvent)`;
- `OutputLogger(destination is Output, formatter is LogFormatter)` — renders
  through the given formatter and writes one `"\n"`-terminated line per
  event;
- `FileLogger(destination is File, formatter is LogFormatter)` — same
  rendering, appended to a real file via `File.append`;
- `SilentLogger()` — deterministic no-op;
- `ContextLogger(source is Logger, additional is @LogField)` — merges
  `additional` after the event's own fields (original fields first, in
  order, context fields after, in order) and delegates to `source`;
- `MemoryLogger()` — `write(event is LogEvent)`, `events() is @LogEvent`.
  Round-trips every event exactly, including empty `fields` and characters
  in `message`/field values that collide with this module's own internal
  delimiters, by serializing each event into one delimited, Base64-escaped
  line in an owned `runtime/memory` cell and re-parsing on `events()` — see
  the contract record for why this is necessary and how it works;
- `LogFailure` — `text() is string`.

Full design rationale, including the `MemoryLogger` encoding scheme and why
it exists, is recorded in `docs/todo/log-logger-22072026.md`.

#### `test/more`

Purpose: reusable assertions above the language test capability.  
Implementation status: **stdlib** (`stdlib/test/more.ok`).<br>
Depends on: contextual language `test` capability, builtin `Error`.

- `Assertions()` — `ok(condition is bool)`,
  `is(actual is any, expected is any)`,
  `isnt(actual is any, expected is any)`, `fails(error is Error)`;
- `AssertionFailure(message is string)` — `text() is string`.

This module complements the language-provided `test` capability; it does not
replace the test runner. `Assertions` has no state. Its constructor performs
no work, while each action delegates to the capability of the currently
running test. A failed check is recorded and execution continues, exactly like
direct `test.ok`, `test.is`, `test.isnt` or `test.fail`. Calling an assertion
action outside a test has no capability and fails explicitly.

Both values passed to `is`/`isnt` cross the ordinary `any` parameter boundary.
The test runner compares their retained concrete snapshots with strict/deep
equality, including nested `@any`, without returning or narrowing either
payload to source code. Mixed incomparable categories and native handles keep
their existing errors. `fails(error)` unconditionally records `error.text()`;
it is useful in typed catch branches and in higher assertion objects.

`AssertionFailure` is a state-only Error value for higher reusable assertions.
It retains one message and calculates `"assertion failed: " + message` in its
`text()` action. Passing one to `fails` records that text without aborting the
test.

```okkama
import "test/more" take Assertions, AssertionFailure;

test "invoice total" {
    let assertions = Assertions();
    assertions
        .ok(true)
        .is([1, 2], [1, 2])
        .isnt("draft", "paid");

    // Explicit unconditional failure which still lets the test continue:
    // assertions.fails(AssertionFailure("missing fixture"));
}
```

#### `test/suite`

Purpose: grouping tests, fixtures and aggregate suite outcomes.  
Implementation status: **stdlib** (`stdlib/test/suite.ok`); complete
contract and verification recorded in `test-suite-22072026.md`.  
Depends on: `time/clock`, `time/duration`, `time/instant`, `errors`. (The
draft's `test/more`/`log/logger` dependency lines were stale — see the
contract doc.)

- role `TestCase` — `name() is string`, `run()`; not in the original
  draft's object list, added because `Suite` needs *some* way to represent
  one runnable case, and `Fixture` alone (setup/teardown, not the case
  itself) can't fill that role;
- role `Fixture` — `before()`, `after()`;
- `Suite(tests is @TestCase, around is @Fixture, clock is Clock)` —
  `run() is SuiteResult`; every `Fixture.before()` runs before each case,
  every `Fixture.after()` after, even when the case throws (no
  `finally` clause exists in this language's grammar — `try_stmt`/
  `try_expr` are `"try" block catch_clause+` only — so the `after()` loop
  is placed unconditionally after a `try`/`catch` that fully absorbs any
  `Error` into a `bool`, which reaches it in both outcomes without one);
  a case is failed if `run()` throws any `Error`, passed otherwise —
  independent of the language's own contextual `test` capability, since
  that capability (and `test/more.Assertions`, which delegates to it) is
  only available inside an actual `test { }` block run by the language's
  own test runner, never from ordinary runtime code a `Suite` executes;
- `SuiteResult` — `passed() is int`, `failed() is int`, `success() is bool`,
  `duration() is Duration`;
- `TestFailure` — `text() is string`; exported vocabulary for a `TestCase`
  to throw, though `Suite` accepts any thrown `Error`, not only this one.

#### `test/fake_clock`

Purpose: deterministic control of time in tests.  
Implementation status: **stdlib** (`stdlib/test/fake_clock.ok`); complete
contract and verification are recorded in `test-fake-clock-21072026.md`. The
module path is `test/fake_clock`, not the originally drafted
`test/fake-clock`: `-` cannot appear in an Okkama module path identifier.

Depends on: `time/clock`, `time/instant`, `time/duration`, `runtime/memory`,
`encoding/binary`, `errors`.

- `FakeClock` — implements `Clock`; `now() is Instant`,
  `sleep(duration is Duration)`, `advance(duration is Duration)`,
  `set(instant is Instant)`.

`FakeClock(start is Instant)` fixes the simulated `ClockIdentity` for the
object's lifetime. `sleep` is `advance` under a different name so it never
blocks a test, but still rejects a negative duration with the existing
`time/clock.InvalidSleepDuration` the same way a real clock would; `advance`
itself accepts a signed `Duration` so tests may rewind time. `set` moves to
an absolute instant and throws the existing `time/instant.IncompatibleClocks`
if its identity does not match the clock's own. Okkama objects cannot
reassign a field outside their constructor, so the current instant is not
stored as one; it lives in an owned `runtime/memory` byte cell, read and
written through `encoding/binary`'s big-endian codec, the same pattern
`io/buffer` uses for owned byte state.

#### `test/benchmark`

Purpose: repeatable duration measurement for benchmark cases.  
Implementation status: **stdlib** (`stdlib/test/benchmark.ok`); complete
contract and verification are recorded in `test-benchmark-21072026.md`.

Depends on: `time/clock`, `time/duration`, `time/instant`, `test/fake_clock`,
`errors`.

- role `BenchmarkCase` — `run()`;
- `Benchmark` — state-only `Benchmark(subject is BenchmarkCase, clock is
  Clock, iterations is int)`; `measure() is BenchmarkResult`;
- `BenchmarkResult` — state-only `BenchmarkResult(values is @Duration)`;
  `iterations() is int`, `total() is Duration`, `average() is Duration`,
  `minimum() is Duration`, `maximum() is Duration`;
- `InvalidBenchmark` — `text() is string`.

`Benchmark` times each `subject.run()` call against the injected `Clock` and
collects the samples into a `BenchmarkResult`; the clock is injected rather
than fixed to `MonotonicClock` so tests can drive it with `test/fake_clock`
and assert exact statistics instead of tolerating real timing noise.
`BenchmarkResult` is independently constructible from already-computed
samples, so its statistics (including the exact, non-float-lossy floor
division behind `average()`) are unit-tested without any clock at all.
`InvalidBenchmark` covers both a non-positive iteration count and a clock
that reports an earlier instant after a run than before it — the latter is
a real, testable path via an injected `test/fake_clock` case that rewinds.

### 6.13 Runtime ABI

`runtime/*` is intentionally function-shaped. Its public values are opaque
native handles, not ordinary domain objects. Every handle is validated by the
native adapter and can only be used by functions declaring its exact type.

#### `runtime/environment`

Purpose: raw-byte host environment ABI.  
Depends on: nothing.

- `environment_get(name is @byte) is @byte`;
- `environment_set(name is @byte, value is @byte) is bool`;
- `environment_remove(name is @byte) is bool`;
- `environment_all() is @@@byte`.

#### `runtime/process`

Purpose: argv, process identity, pipes and child-process ABI.  
Depends on: nothing.

- native objects `PipeHandle`, `ProcessHandle`;
- `process_arguments() is @@byte`, `process_current_id() is int`,
  `process_parent_id() is int`;
- `pipe() is PipeHandle`, `pipe_read(handle, size) is @byte`,
  `pipe_write(handle, bytes) is int`, `pipe_close_read(handle) is bool`,
  `pipe_close_write(handle) is bool`, `pipe_close(handle) is bool`;
- `spawn(program is @byte, arguments is @@byte, input is PipeHandle, output is
  PipeHandle, errors is PipeHandle) is ProcessHandle`;
- `spawn_configured(program is @byte, arguments is @@byte, input is
  PipeHandle, output is PipeHandle, errors is PipeHandle, directory is @byte,
  environment is @@@byte) is ProcessHandle`;
- `wait(handle) is %string:int`, `try_wait(handle) is %string:int`,
  `kill(handle, signal is int) is bool`, `process_id(handle) is int`,
  `process_close(handle) is bool`.

`spawn_configured` replaces the child environment with exact `[name, value]`
byte pairs and changes cwd only after `fork`; it never mutates parent ambient
state. This is the substrate for the composed `CommandEnvironment` and
`CommandDirectory` state used by `Cmd`.

#### `runtime/fs`

Purpose: raw paths, files, directories and filesystem metadata ABI.  
Depends on: nothing.

- native objects `FileHandle`, `DirectoryHandle`;
- `path_encode(path is string) is @byte`, `path_decode(path is @byte) is
  string`;
- `open(path is @byte, flags is int, mode is int) is FileHandle`,
  `read(handle is FileHandle, size is int) is @byte`,
  `write(handle is FileHandle, bytes is @byte) is int`,
  `seek(handle is FileHandle, offset is int, origin is int) is int`,
  `flush(handle is FileHandle) is bool`,
  `truncate(handle is FileHandle, size is int) is bool`,
  `sync(handle is FileHandle) is bool`,
  `lock_shared(handle is FileHandle) is bool`,
  `lock_exclusive(handle is FileHandle) is bool`,
  `unlock(handle is FileHandle) is bool`,
  `close(handle is FileHandle) is bool`;
- `stat(path is @byte) is %string:int`,
  `lstat(path is @byte) is %string:int`;
- `mkdir(path is @byte, mode is int) is bool`,
  `rmdir(path is @byte) is bool`, `unlink(path is @byte) is bool`,
  `rename(source is @byte, target is @byte) is bool`,
  `link(source is @byte, target is @byte) is bool`,
  `symlink(target is @byte, link_path is @byte) is bool`,
  `readlink(path is @byte) is @byte`;
- `cwd() is @byte`, `chdir(path is @byte) is bool`,
  `chmod(path is @byte, mode is int) is bool`,
  `chown(path is @byte, owner is int, group is int) is bool`;
- `opendir(path) is DirectoryHandle`, `readdir(handle) is @byte`,
  `rewinddir(handle) is bool`, `closedir(handle) is bool`.

All paths are raw `@byte`; integer flags and metadata maps stop at this layer.
Locks are advisory file locks with normalized shared/exclusive semantics.
`sync` requests durable host synchronization and is distinct from flushing an
Okkama output buffer. These remain general filesystem syscalls, not
database-specific host operations.

#### `runtime/stdio`

Purpose: raw standard-stream ABI.  
Depends on: nothing.

- `stdout_write(data is @byte) is int`;
- `stderr_write(data is @byte) is int`;
- `stdin_read(limit is int) is @byte`;
- `stdin_read_line() is @byte`.

`stdin_read` requires a positive limit and returns at most that many bytes;
empty means EOF. It shares a provider-owned byte buffer with
`stdin_read_line`, so calls may be interleaved. These procedural names and
native write counts stop at the ABI: object queries are `bytes()`/`line()`,
and object output commands do not expose counters.

#### `runtime/memory`

Purpose: minimal owned mutable byte state required by live Okkama resources.<br>
Depends on: nothing.

- native object `ByteMemoryHandle`;
- `byte_memory_create(value is @byte) is ByteMemoryHandle`;
- `byte_memory_bytes(handle is ByteMemoryHandle) is @byte`;
- `byte_memory_size(handle is ByteMemoryHandle) is int`;
- `byte_memory_consume(handle is ByteMemoryHandle, limit is int) is @byte`;
- `byte_memory_append(handle is ByteMemoryHandle, value is @byte) is int`;
- `byte_memory_reset(handle is ByteMemoryHandle) is bool`;
- `byte_memory_closed(handle is ByteMemoryHandle) is bool`;
- `byte_memory_close(handle is ByteMemoryHandle) is bool`.

This ABI is deliberately not an I/O buffer implementation. It knows no line
delimiter, prefetch size, output threshold, encoding, wrapped stream or retry
policy. It only owns byte memory that immutable ordinary fields cannot model
for a live command-oriented resource. All buffering behavior remains in
`io/buffer.ok`. Close releases the payload but retains a small authority
tombstone so `closed()` and idempotent `close()` remain observable until
runtime shutdown.

#### `runtime/time`

Purpose: nanosecond wall/monotonic clocks and sleeping ABI.  
Depends on: nothing.

- `wall_clock() is int`;
- `monotonic_clock() is int`;
- `sleep(duration is int) is bool`;
- native object `TimerHandle`;
- `timer_create(delay is int, interval is int) is TimerHandle`;
- `timer_wait(timer is TimerHandle) is bool`,
  `timer_cancel(timer is TimerHandle) is bool`,
  `timer_cancelled(timer is TimerHandle) is bool`,
  `timer_deadline(timer is TimerHandle) is int`,
  `timer_close(timer is TimerHandle) is bool`.

All values are integer nanoseconds. Clock identity is supplied by the
high-level `time/*` objects. Timer deadlines use the monotonic clock. A zero
interval denotes one-shot state; a positive interval denotes fixed-rate
periodic state. Handles retain private cancellation/next-deadline state and
are authority-checked and shutdown-owned.

#### `runtime/timezone`

Purpose: expose the configured host-zone identity without importing host time
zone rules into the language.<br>
Status: **proposed**. Depends on: nothing.

- `local_zone_identifier() is string`;
- `local_zone_territory() is string`.

The identifier remains platform-native: it is normally an IANA name on POSIX
and a Windows time-zone ID on Windows. The territory is an optional stable
mapping hint and is the empty string when the host cannot provide one. This
ABI never returns an offset, transition, abbreviation or parsed host tzfile.
Only the explicit high-level `HostTimeZoneIdentity` adapter calls it.

#### `runtime/net`

Purpose: address, DNS and socket syscall ABI.  
Depends on: nothing.

- native object `SocketHandle`;
- `ip_parse(family is int, text is string, port is int) is @byte`,
  `ip_format(address is @byte) is string`,
  `address_family(address is @byte) is int`,
  `address_port(address is @byte) is int`,
  `resolve(host is string, port is int, socket_type is int) is @@byte`,
  `reverse_name(address is @byte) is string`;
- `socket(family is int, socket_type is int, protocol is int) is SocketHandle`,
  `bind(handle is SocketHandle, address is @byte) is bool`,
  `listen(handle is SocketHandle, backlog is int) is bool`,
  `accept(handle is SocketHandle) is SocketHandle`,
  `connect(handle is SocketHandle, address is @byte) is bool`;
- `send(handle is SocketHandle, bytes is @byte) is int`,
  `recv(handle is SocketHandle, size is int) is @byte`,
  `sendto(handle is SocketHandle, address is @byte, bytes is @byte) is int`,
  `recvfrom(handle is SocketHandle, size is int) is @byte`,
  `datagram_address(datagram is @byte) is @byte`,
  `datagram_payload(datagram is @byte) is @byte`;
- `shutdown(handle is SocketHandle, direction is int) is bool`,
  `local_address(handle is SocketHandle) is @byte`,
  `peer_address(handle is SocketHandle) is @byte`,
  `socket_get_option(handle is SocketHandle, option is int) is bool`,
  `socket_set_option(handle is SocketHandle, option is int, enabled is bool) is
  bool`, `socket_close(handle is SocketHandle) is bool`.

Families, socket types, protocols, options and shutdown directions are stable
runtime integers; high-level modules replace them with typed objects/enums.

#### `runtime/poll`

Purpose: portable readiness registration and waiting ABI.  
Depends on: `runtime/fs`, `runtime/process`, `runtime/net`.

- native object `PollHandle`;
- `poll_create() is PollHandle`;
- `poll_add_file(poll is PollHandle, handle is FileHandle, interests is int,
  tag is int) is int`;
- `poll_add_pipe(poll is PollHandle, handle is PipeHandle, interests is int,
  tag is int) is int`;
- `poll_add_socket(poll is PollHandle, handle is SocketHandle, interests is
  int, tag is int) is int`;
- `poll_modify(poll is PollHandle, registration is int, interests is int,
  tag is int) is bool`,
  `poll_remove(poll is PollHandle, registration is int) is bool`;
- `poll_wait(poll, deadline is int) is @%string:int`;
- `poll_cancel(poll) is bool`, `poll_close(poll) is bool`.

#### `runtime/regex`

Purpose: portable compiled-regex handle ABI.  
Depends on: nothing.

- native object `RegexHandle`;
- `compile(pattern is string) is RegexHandle`;
- `invalid_offset(pattern is string) is int`;
- `contains(pattern, input is string) is bool`;
- `matches(pattern, input is string) is bool`;
- `spans(pattern, input is string) is @%string:int`;
- `all_spans(pattern, input is string) is @@%string:int`;
- `close(pattern) is bool`.

`spans` returns the leftmost-longest full match first and capture groups in
source order. Offsets count Unicode code points; an unmatched capture is
`["start": -1, "end": -1]`. `all_spans` repeats that projection for
non-overlapping matches and advances after zero-length matches.
`invalid_offset` returns `-1` for a valid pattern or the exact syntax offset;
it never exposes parser text. Replacement remains a high-level operation.

#### `runtime/utf8`

Purpose: strict host UTF-8 codec ABI.  
Depends on: nothing.

- `encode(value is string) is @byte`;
- `decode(value is @byte) is string`;
- `valid_prefix(value is @byte) is int`.

Encoding and decoding are strict; invalid direct decode input is a
`NativeFailure`. `valid_prefix` is total for `@byte`: it returns the complete
size for valid input or the start offset of the first ill-formed sequence.
The stdlib adapter uses that stable integer contract rather than parsing a
provider message.

#### `runtime/system`

Purpose: normalized host identity and secure-random ABI.  
Depends on: nothing.

- `system_platform() is string`;
- `system_architecture() is string`;
- `secure_random(size is int) is @byte`.

Platform and architecture return closed normalized spellings, never raw host
identifiers.

#### `runtime/debug`

Purpose: privileged deterministic diagnostic rendering.  
Depends on: nothing.

- `data_dump(value is any) is string`.

This is deterministic diagnostic rendering, not serialization or reflection.

#### `runtime/crypto`

Purpose: reviewed digest, HMAC and constant-time primitive ABI.  
Status: **runtime**. Depends on: nothing.

- `digest(algorithm is string, value is @byte) is @byte`;
- `hmac(algorithm is string, key is @byte, value is @byte) is @byte`;
- `secure_equal(first is @byte, second is @byte) is bool`.

The closed algorithm set is `sha256`, `sha384`, and `sha512`.
`secure_equal` visits every position up to the greater input length and folds
the length difference into its result. The ABI does not expose a host
cryptography library's arbitrary algorithm registry.

#### `runtime/tls`

Purpose: native TLS session and certificate-projection ABI.  
Status: **runtime**. Depends on: `runtime/net`, `runtime/fs`.

- native object `TlsHandle`;
- `tls_client(socket is SocketHandle, server_name is string, ca_file is @byte)
  is TlsHandle`;
- `tls_server(socket is SocketHandle, certificate is @byte, key is @byte) is
  TlsHandle`;
- `tls_handshake(handle is TlsHandle) is bool`,
  `tls_read(handle is TlsHandle, size is int) is @byte`,
  `tls_write(handle is TlsHandle, value is @byte) is int`,
  `tls_close(handle is TlsHandle) is bool`;
- `tls_peer_subject(handle is TlsHandle) is string`,
  `tls_peer_issuer(handle is TlsHandle) is string`;
- `tls_peer_valid_from(handle is TlsHandle) is int`,
  `tls_peer_valid_until(handle is TlsHandle) is int`;
- `tls_peer_der(handle is TlsHandle) is @byte`.

Creating a TLS session consumes the connected stream `SocketHandle`; its old
identity becomes a closed tombstone and `TlsHandle` becomes the sole owner.
Client mode always verifies the certificate chain and `server_name`. Server
mode accepts explicit PEM certificate/key bytes. Validity values are Unix
seconds and DER is the exact peer leaf certificate. The default Perl runtime
uses the reviewed `IO::Socket::SSL`/`Net::SSLeay` dependencies declared in
`okkama-pl/cpanfile`.

#### `runtime/format`

The target tree lists top-level `format`; its required lower ABI is retained
separately:

Purpose: privileged formatting of opaque `any` values.  
Depends on: nothing.

- `format_values(pattern is string, values is @any) is string`;
- `format_named(pattern is string, values is %string:any) is string`.
- `sprintf_values(pattern is string, values is @any) is string`.

`concat_values(values is @any) is string` is the narrow compatibility
operation used by standard output adapters. Pattern formatting accepts `{}`
or identifier fields respectively; `{{`/`}}` escape literal braces.

## 7. Module-entry template

Every future module entry must include:

```text
Module path:
Status:
Dependencies:
Purpose:
Public roles:
Public objects:
Public actions/signatures:
Errors:
Ownership/lifetime:
Complete example:
Implementation tests:
Open questions:
```

An entry without object signatures and a usage example is only a name in a
tree, not an API design.

## 8. Acceptance checklist

- [ ] The module solves a concrete standard-library task.
- [ ] Its directory is justified by more than categorization.
- [ ] Objects expose behaviour, not fields.
- [ ] Failure, ownership and cleanup semantics are explicit.
- [ ] No raw native handle, errno, Perl value or platform constant leaks out.
- [ ] External capabilities have a role suitable for fakes.
- [ ] At least one complete Okkama example exists.
- [ ] Focused tests exist.
- [ ] English and Russian books are updated when the API becomes available.
- [ ] Status and document version are updated.

## 9. Change log

### 0.4.34-dev — 21 July 2026

- Implemented pure-Okkama `encoding/json` with nominal heterogeneous values,
  deterministic compact encoding, strict recursive parsing and located
  Unicode errors.
- Kept JSON numbers as exact decimal lexemes so valid values outside Okkama
  numeric ranges survive parsing and encoding; explicit `int()`/`float()`
  projections fail through a typed conversion error.
- Rejected duplicate object names and out-of-range array access explicitly;
  constructors remain state-only and no host JSON parser or encoder is used.

### 0.4.33-dev — 21 July 2026

- Replaced the ambiguous proposed `template/text` namespace with implemented
  pure-Okkama `template/plain`.
- Defined ERB/Mojolicious-style `<%=`, `<%==`, `<%#`, and `<%%` lexical forms
  without embedded-code evaluation; control tags remain reserved for a typed
  future grammar.
- Replaced verb-returning `render()` with `string(values)`, made values
  variadic and state-only, and added exact missing/duplicate-name failures plus
  Unicode source offsets.

### 0.4.32-dev — 21 July 2026

- Implemented owned `Buffer`/`StringBuffer` resources and non-owning
  `BufferedInput`/`BufferedOutput` decorators with explicit acquisition
  projections and state-only constructors.
- Defined unread snapshots, one-prefetch input, capacity-triggered output,
  strict UTF-8 lines, idempotent lifecycle and exact partial-write retry.
- Added the narrow generic `runtime/memory` byte-state capability; line,
  encoding and buffering policy remain pure Okkama rather than native ABI.

### 0.4.31-dev — 21 July 2026

- Implemented state-only pure-Okkama `net/url` with strict UTF-8 percent
  handling, duplicate-preserving queries, bracketed IP literals and RFC 3986
  dot-segment reference resolution.
- Added explicit `MissingUrlPort`; no absent component is represented by a
  magic integer and query plus never acquires HTML-form semantics.
- Implemented injectable `Resolver`, system `Dns` and immutable `FixedDns`;
  extended `runtime/net.resolve` with transport-neutral socket type zero so
  DNS does not choose TCP/UDP policy.

### 0.4.30-dev — 21 July 2026

- Added the dependency-ordered `runtime/timezone` and `time/tzdb` backlog so
  named and local civil time cannot disappear behind the temporary UTC-only
  implementation.
- Fixed the ownership boundary: the runtime reports only host-zone identity,
  while stdlib owns bundled versioned IANA rules and host-ID mapping.
- Defined explicit `TimeZoneDatabase` injection, state-only `TimeZone`, and
  opt-in `HostTimeZone`; no program silently consults host tzdata or downloads
  rules at runtime.

### 0.4.29-dev — 21 July 2026

- Resolved the timer design as a live owned resource, following the same
  immutability exception as files, sockets and child processes.
- Added the narrow authority-checked `runtime/time.TimerHandle` ABI and exact
  create/wait/cancel/query/deadline/close lifecycle.
- Added state-only `ScheduledTimer`/`ScheduledPeriodicTimer` acquisition
  projections and owning `Timer`/`PeriodicTimer` objects.
- Replaced `cancel() is bool` with the `cancel()` command plus named
  `cancelled()` query; defined one-shot completion, cancellation and
  fixed-rate skip-missed periodic behavior.

### 0.4.28-dev — 21 July 2026

- Implemented exact clock-bound `Instant` values with closed system/monotonic
  identity, checked shifts and typed incompatible-clock failures.
- Implemented `Clock`, `SystemClock`, `MonotonicClock` and composed
  `OffsetClock` over `runtime/time`, including a specific negative-sleep error
  and sanitized provider failures.
- Implemented pure-Okkama proleptic Gregorian `Date`, UTC `DateTime` and the
  initial deterministic UTC-only `TimeZone` policy, with exact negative Unix
  floor semantics and signed-64 endpoint coverage.
- Blocked the mutable timer draft from fake implementation and recorded the
  immutable-successor versus live-resource design choice that 0.4.29 resolves.

### 0.4.27-dev — 21 July 2026

- Implemented pure-Okkama `fs/path` with the complete `PathValue` role,
  action-time host selection and explicit POSIX/Windows values.
- Defined state-only construction, immutable lexical normalization, strict
  relative joining, normalized parent semantics, UTF-8 bytes and typed path
  failures.
- Covered POSIX roots, Windows drive/root-relative/UNC forms, malformed input,
  unknown hosts, idempotence and exact Unicode byte projections.

### 0.4.26-dev — 21 July 2026

- Implemented the complete `RandomSource` contract, secure `Random`, pure
  deterministic `SeededRandom`, generic `Choice<T>` and immutable
  `Shuffled<T>` projections.
- Defined inclusive unbiased signed-64 ranges, 53-bit `[0.0, 1.0)` fractions,
  explicit source injection, entropy-free singleton operations and stable
  range failures.
- Replaced recursive ownership reachability with an order-preserving
  iterative walk after a large seeded byte value exposed host stack warnings.

### 0.4.25-dev — 21 July 2026

- Implemented `proc/args` as a stateless object over the runtime-owned argv
  snapshot, with exact raw bytes, strict UTF-8 strings, count and indexed
  access.
- Added indexed `InvalidArgumentEncoding` and `MissingArgument`, preserved
  empty/non-UTF-8 arguments, excluded the source path and sanitized provider
  failures.

### 0.4.24-dev — 21 July 2026

- Implemented `env/var` with the complete text/raw-byte `Environment` role,
  provider-backed `Env`, nominal `EnvVar`, detached `EnvEntry` and three typed
  domain failures.
- Kept constructors state-only, preserved missing versus empty and opaque raw
  values, made text decoding strictly UTF-8, and supplied an isolated sorted
  role fake with immutable snapshots.
- Fixed contextual `has(...)` action declarations without changing
  `has name is Type;` state syntax.

### 0.4.23-dev — 21 July 2026

- Implemented pure-Okkama signed `Duration` arithmetic and exact projections
  for nanoseconds, milliseconds, seconds, minutes and hours.
- Defined checked signed-64 bounds with typed `InvalidDuration` overflow; no
  float conversion, wrap or saturation occurs.
- Corrected action-boundary expected-type isolation after the duration tests
  exposed caller context leaking into a nested `reduce`.

### 0.4.22-dev — 21 July 2026

- Implemented typed `Platform`, `Architecture`, `PathStyle` and the complete
  `OperatingSystem` role through `Host`, `Posix` and `Windows`.
- Kept every constructor state-only, refused to guess path conventions for an
  unknown host, and sanitized provider failures/invariant spellings.
- Added `\r` string/char escapes so Windows CRLF is representable exactly in
  ordinary Okkama source.

### 0.4.21-dev — 21 July 2026

- Implemented state-only `Regex`, detached `Match`, typed `InvalidRegex` and
  `MissingMatch`, non-overlapping `all` and literal global `replaced`.
- Extended `runtime/regex` with typed `invalid_offset` and `all_spans`
  projections while keeping compiler and native-handle internals private.
- Defined Unicode offsets, unmatched capture strings, zero-length advancement
  and per-action deterministic handle ownership.

### 0.4.20-dev — 21 July 2026

- Replaced the caller-owned positional `@any` boundary with
  `Formatted(pattern, values...)`.
- Kept construction state-only: the variadic tail is materialized directly
  into immutable object state, while validation and rendering remain in
  `string()`.

### 0.4.19-dev — 21 July 2026

- Implemented state-only `Formatted` and `NamedFormatted` projections over
  the privileged opaque-`any` rendering boundary.
- Defined positional/named field grammar, escaped braces, Unicode offsets,
  permitted surplus values and exact missing index/key diagnostics.
- Kept all source validation in Okkama actions and sanitized unexpected
  provider failures as deterministic invariant panics.

### 0.4.18-dev — 21 July 2026

- Prohibited `with`/`with_*` actions and immutable copy-setters throughout the
  public stdlib design.
- Replaced fluent configuration with typed state composition and decorators;
  removed the equivalent `without`, boolean option toggles, query-builder and
  command-directory copy APIs.
- Replaced the fluent binary builder with composed `BinaryPart` values and
  removed the address-to-endpoint factory in favor of direct state-only
  `SocketAddress` construction.

### 0.4.17-dev — 21 July 2026

- Implemented `test/more` as a stateless assertion facade over the active
  language test capability, including non-aborting `isnt` and typed failures.
- Extended test-only equality to compare nested boxed `any` snapshots without
  exposing or narrowing their payloads to Okkama code.
- Allowed keyword-shaped namespace segments and contextual action labels where
  grammar is unambiguous, making `module test/more` and `Assertions.is`
  declarable without weakening ordinary identifier rules.

### 0.4.16-dev — 21 July 2026

- Implemented immutable validated `net/address` values over the portable
  `runtime/net` numeric-IP boundary.
- Kept runtime endpoint envelopes private: public `IpAddress.bytes()` exposes
  exactly four or sixteen address octets and socket strings bracket IPv6.
- Fixed the shared port domain, unresolved `HostPort` boundary and sanitized
  typed address/port failures; reserved Unix family for a future address type.
- Kept all three constructors state-only; parsing, canonicalization,
  validation and derived values execute exclusively in actions.

### 0.4.15-dev — 21 July 2026

- Repaired `crypto/random` as a stateless `SecureByteSource` adapter over the
  OS CSPRNG and added its deterministic test-only role fake.
- Added typed request-size validation and sanitized unavailable-source errors.
- Removed the nonconforming stored-size constructor and untyped projection;
  valid calls now return exactly the requested byte count.

### 0.4.14-dev — 21 July 2026

- Implemented pure-Okkama `encoding/hex` with immutable projection objects.
- Fixed lowercase canonical output and strict case-insensitive ASCII decoding.
- Added deterministic typed offsets for odd lengths and invalid characters;
  prefixes, separators, whitespace and Unicode remain rejected.

### 0.4.13-dev — 20 July 2026

- Implemented pure-Okkama canonical padded RFC 4648 `encoding/base64` with
  immutable projections and no native dependency.
- Added strict alphabet, padding and unused-tail-bit validation with typed
  deterministic `InvalidBase64` offsets.
- Preserved numeric-looking string map-key identity so reverse lookup keys
  such as `"0"` remain strings across `%string:V` boundaries.

### 0.4.12-dev — 20 July 2026

- Added strict immutable `encoding/utf8` projections for explicit
  `string`/`@byte` conversion and typed `InvalidUtf8` byte offsets.
- Extended `runtime/utf8` additively with one-pass `valid_prefix(@byte)` while
  preserving the existing encode/decode ABI identities.
- Rejected replacement decoding, overlong forms, surrogates, out-of-range
  scalars and truncated sequences without implicit normalization or BOM
  policy.

### 0.4.11-dev — 20 July 2026

- Extended the existing eager `map`, `filter` and `reduce` expressions to
  nominal standard `Iterable<T>` objects through ordinary cursor dispatch.
- Added persistent `Queue<@byte>` chunk traversal and one-pass typed byte
  materialization to the original binary construction implementation.
- Corrected generic evidence from already tagged arrays and maps so nested
  collection specializations such as `Queue<@byte>` remain exact.

### 0.4.10-dev — 20 July 2026

- Implemented pure-Okkama `encoding/binary` with cursor-free fixed-width
  endian reads and exact two's-complement decoding and encoding.
- Replaced the invalid reserved action name `take` with
  `bytes(offset, size)` and made writer successor types explicit.
- Added typed range/short-read failures and exact signed 64-bit boundary
  coverage without host packing or platform endianness.

### 0.4.9-dev — 20 July 2026

- Added the persistent two-list `Queue<T>` with FIFO iteration and amortized
  O(1) enqueue/tail.
- Added `Stack<T>` as explicit LIFO vocabulary composed over `List<T>` storage
  and traversal.
- Replaced the reserved draft action `with` with immutable-result names and
  deferred Set array materialization.

### 0.4.8-dev — 20 July 2026

- Added the nominal immutable `collections/iteration` protocol used by
  user-defined `for` sources.
- Made both persistent list implementations iterable through a private
  `ListCursor<T>` without changing list state or exposing implementation
  declarations to selective-import consumers.

### 0.4.4-dev — 20 July 2026

- Added lifecycle-aware fakes to all four I/O capability roles using
  identity-local `fake.storage`.
- Corrected recursive test materialization so mixed byte/bool/char container
  results are directly comparable without `Test::More` internal failures.

### 0.4.3-dev — 20 July 2026

- Implemented `io/input` and `io/output` as pure-Okkama capability modules.
- Fixed positive read limits, EOF/line semantics, complete-write behavior,
  chainable lifecycle actions, and the three typed protocol errors.

### 0.4.2-dev — 20 July 2026

- Promoted literal `split`, `join`, `replace` and `replace_first` to builtin
  immutable value behavior.
- Removed the ceremonial `Split`, `Joined`, `Replaced`, `ReplacedFirst`,
  `Removed` and `InvalidSeparator` object candidates.
- Defined empty-separator/empty-target behavior in Unicode code-point units.

### 0.4.0-dev — 19 July 2026

- Removed `runtime/database` and every database-specific host handle/action.
- Added explicit DBI-style `db/driver` composition with no reflection or
  global driver registry.
- Added proposed Okkama drivers for SQLite file/syscall semantics and the
  PostgreSQL/MySQL wire protocols.
- Added `encoding/binary` and general file truncate, durable sync and advisory
  locking primitives needed to implement protocols above the host boundary.

### 0.3.0-dev — 19 July 2026

- Added a complete per-module catalogue of direct dependencies, proposed
  roles, objects and actions.
- Documented the native functions and opaque handles below every `runtime/*`
  module.
- Added the missing runtime foundations for cryptography and TLS; these remain
  exploratory until their typed ABIs are reviewed.
- Recorded concrete ownership and failure rules for files, processes, sockets,
  transactions, temporary entries and streamed HTTP bodies.

### 0.2.0-dev — 19 July 2026

- Removed the redundant `core` namespace and its speculative contents.
- Removed `text` and `concurrent`.
- Replaced type-specific math files with one general `math` module.
- Preserved the shortened filesystem tree and added explicit `env/cwd`.
- Removed `proc/process`; moved environment to top-level `env`.
- Split `socket` and `http` from `net`; added `http/router`.
- Added `string`, `template`, `errors`, `sql` and `db`.
- Added typed `os` information and separated host paths from explicit foreign
  POSIX/Windows path parsing.
- Marked `collections` questionable rather than foundational.
- Replaced broad architecture prose with a concrete module index and API
  examples.

### 0.1.0-dev — 19 July 2026

- Created the first draft.
