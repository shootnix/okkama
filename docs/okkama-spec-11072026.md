# Okkama Language Specification

Snapshot: 11.07.2026  
Last updated: 18.10.2026
Target language line: Okkama 1.0  
Document status: living implementation specification

This document describes the language as it is intended to work in the current
1.0 implementation line. When a feature is parser-only or not yet implemented
by the runtime, this document says so explicitly.

The implemented fixed-width bitwise operators (`& | ^ ~ << >> >>>`), based
integer literals and int/byte bit methods are specified in the
[bitwise guide](bitwise-guide.md) and its linked mathematical contract.
Stream writing now uses `<-`. Infix `^` is XOR; `is ^Type` and `^field = ...`
retain their successor semantics. The current precedence is also recorded in
[the grammar](okkama-grammar-1.0.md).

## 1. Purpose

Okkama is a statically typed object language built around immutable objects,
explicit state transitions, nominal roles, and block-scoped ownership.

The core idea is simple:

```text
Objects expose behavior, not state.
Object state is declared with has.
Objects are immutable after construction.
State changes are represented by successor objects.
Lifecycle cleanup is represented by destroy blocks.
```

Okkama deliberately avoids inheritance, nulls, optional chaining, and implicit
field access from outside an object.

## 2. Normative Terms

This document uses these terms precisely:

- **MUST** means required language behavior.
- **MUST NOT** means forbidden language behavior.
- **MAY** means allowed behavior.
- **Parser-only** means syntax can be parsed, but runtime execution is not part
  of the current implementation contract yet.
- **Runtime-supported** means parser and interpreter behavior are both covered
  by tests.
- **Object scope** means code executing inside an action, constructor
  block, or destroy block of an object.
- **Successor action** means an action whose return type is `^Type`.

## 3. Source File Shape

Every source file starts with a module declaration.

```okkama
module app v1.0.0;
```

The general file shape is:

```okkama
module module/name v1.0.0;

import "some/module";

object Thing {
    has value is int;

    Thing(value is int) {
        value = value;
    }
}

main() is int {
    Thing(value: 1);
    return 0;
}
```

Executable modules MUST declare exactly one parameterless entry point with
signature `main() is int` or `async main() is int`. Other result declarations
(including `void`, `any`, `byte`, `number`, and successor types) MUST be rejected
before executing the entry body. Object methods named `main` are ordinary
actions and do not have this restriction.

The explicit or implicit result MUST be an actual `int` in `0..255`.
Zero denotes success; 1 through 255 are application-defined errors.
Missing results, wrong value types, negative and oversized results MUST fail;
the implementation MUST NOT synthesize zero, unbox `any`, or truncate.
The CLI MUST use the validated value as its process exit status without
printing it, after lexical cleanup, task shutdown and native-resource cleanup.
A cleanup failure MUST NOT be reported as a successful return. In test mode
the entry declaration is still checked, but `main` is not executed; the
runner returns 0 for passing tests and 1 for failed tests.

Library modules MAY omit `main`.

Top-level executable statements are not allowed. Declarations only are allowed
at module scope.

## 4. Lexical Conventions

### 4.1 Identifiers

Lowercase identifiers name variables, actions, fields, parameters, and module
path segments.

```text
name
total_cents
_private_local
```

Type names start with an uppercase letter.

```text
Cash
Bottle
InvoiceStatus
```

Reserved keywords are not ordinary identifiers even when their lexeme otherwise
has a valid lowercase shape. They cannot name variables, fields, parameters,
constructors, or type declarations. Module path segments and action labels are
contextual namespaces: a keyword-shaped lowercase name is valid where the
grammar is unambiguous, as in `module test/more`, action `is(...)`, and
`assertions.is(...)`. In an object body, `has(...)` is an action label while
`has field is Type;` remains a state declaration. Built-in/special member
forms such as `value.last()`,
`test.ok()`, and `Role.fake()` follow the same contextual rule. Removed and
deferred reserved words remain rejected in every namespace.

An action name MUST NOT be exactly `with` or begin with `with_`. The rule
applies to object, role, fake, private, projection and native declarations and
to member calls. It deliberately does not reject unrelated words such as
`withdrawal`. This syntax barrier prevents old copy-setter APIs from entering
or being called from current source. Architecturally equivalent fluent
configuration under a different verb is rejected by stdlib review rather than
by identifier spelling.

### 4.2 Literals

Runtime-supported literals:

````okkama
10
3.14
"hello"
```raw "text" \n remains raw```
'x'
true
false
````

String literals support these escape sequences:

```text
\n    newline
\r    carriage return
\t    tab
\"    double quote
\\    backslash
```

Character literals support the corresponding single-character escapes `\n`,
`\r`, `\t`, `\'`, and `\\`. Escape sequences are decoded before the char literal's
exactly-one-character rule is checked.

An escaped quote does not terminate its literal. Unknown escape sequences and
a trailing backslash without an escaped character are lexer errors. A token's
source lexeme retains the original escape spelling, while its literal value
contains the decoded character or string.

Three backticks delimit a fenced raw string. Every character between the
delimiters is retained literally, including opening/closing newlines,
indentation, quotes, backslashes and comment-looking text. Fenced raw strings
are ordinary `string` values; they introduce no new runtime type or
precedence. The first following exact triple-backtick sequence closes the
literal. There is no escape syntax, interpolation, dedenting, trimming,
language tag or other modifier. An unterminated fence is a lexer error, and
one or two isolated backticks remain invalid characters.

Decimal token syntax exists, but full decimal runtime semantics are not yet
specified.

### 4.3 Reserved or Removed Words

The following words are not valid Okkama 1.0 syntax:

```text
null none nil optional ?. isa super abstract protected with setters mutable var
```

They may tokenize distinctly so the parser can produce clear errors, but code
MUST NOT use them.

`destroy` is not reserved. It is a valid object lifecycle block keyword.

Local variable bindings introduced by `let`, `for`, and `catch` accept
`[A-Za-z_][A-Za-z0-9_]*`, including `USER_NAME`, `A`, and `UserName`.
Names are case-sensitive; uppercase spelling does not imply a constant.
Keywords remain reserved. Field, parameter, action, and type declaration
names retain their existing rules.

## 5. Modules and Imports

### 5.1 Module Declaration

```okkama
module app v1.0.0;
module money/cash v1.0.0;
module money/cash;
```

Rules:

- The module declaration MUST be first.
- The module name is one or more lowercase identifiers separated by `/`.
- The version is optional. When present it MUST use `vMAJOR.MINOR.PATCH`.
  A module without a version has ABI major `0` (§5.9).
- A CLI invocation may supply multiple existing `.ok` source paths. They form
  one module declaration space, but each file retains its own import scope.
  Every file MUST declare the same module name and the same version (or every
  file no version). A mismatch is a semantic error, and the combined module
  MUST NOT declare more than one `main`.

### 5.2 Imports

Import syntax:

```okkama
import "money";
import "money" as cash_money;
import "money" take Cash;
import "io/stdout" as out take Stdout;
```

Rules:

- Imports MUST appear after the module declaration and before other
  declarations.
- Wildcard imports are not part of Okkama 1.0.
- Import paths are source paths, not module declaration names.
- If an import path has no `.ok` suffix, runtime lookup appends `.ok`.
  For example, `import "money";` resolves as `money.ok`, and
  `import "foo/bar";` resolves as `foo/bar.ok`.
- Explicit `.ok` suffixes are accepted as-is.
- Runtime lookup is local-first:
  1. the directory of the importing file;
  2. the root of the importing file's package (§5.2.2), when it has one;
  3. system import directories (`OKKAMA_IMPORT_DIRS`);
  4. the bundled Okkama standard library directory.
- A standard library module resolves its own imports only inside the standard
  library.
- A local module shadows a bundled standard library module with the same
  import path for the program's own imports.
- A plain import exposes declarations owned directly by the imported module
  only through a namespace derived from the final import-path component.
  Declarations visible to that module only through its own imports are not
  re-exported. For example,
  `import "io/stdout";` permits `stdout.Stdout()`. The same namespace resolves
  type annotations and applied generic types such as `shapes.Area` and
  `boxes.Box<int>`. The namespace selects the declaration. Runtime nominal
  identity is the defining module plus ABI-major version plus declared name,
  such as `shapes#1::Area` or `boxes#1::Box<int>`. The canonical prefix is an
  implementation identity and is omitted from ordinary source-facing
  diagnostics and formatting.
- `as` replaces the derived basename namespace with the requested namespace:
  `import "io/stdout" as io;` permits `io.Stdout()` and does not create the
  `stdout` namespace. When combined with `take`, the alias namespace contains
  only the selected directly owned declarations.
- `take` resolves listed names only against declarations owned directly by the
  imported module. A name found only in that module's imports is reported as
  missing.
- A `take` import without `as` exposes exactly the selected names as bare
  bindings and creates no namespace. Contract roles, signature types, body
  helpers, and arbitrary unselected siblings do not become consumer bindings.
- Selected declarations retain the transitive dependency closure required for
  registration and execution. That closure is private: internally imported
  objects, roles, enums, native declarations, types, and namespaces do not
  become consumer bindings.
- Constructors, actions, defaults, lifecycle blocks, and role fakes resolve
  source names in their defining module scope, not the caller's module scope.
- Shared transitive dependencies are registered once even when reached through
  multiple branches.
- Missing `take` names are runtime errors.
- Duplicate bare names introduced by `take` or local declarations are semantic
  errors. Namespace-qualified and hidden declarations from different modules
  coexist under distinct module-qualified nominal identities.
- Repeated imports of the same module are idempotent and do not duplicate
  declarations.
- Imported module `main()` is never executed by an import.
- Cyclic imports are rejected.

### 5.2.1 Standard Library Paths

A standard library declaration can be named in place, without an import, by a
path that starts with the reserved word `std`:

```okkama
let out = std.io.stdout.Stdout();
let path is std.fs.path.Path = std.fs.path.Path("/home/user");
let order = std.compare.Order.less;
let text = std.runtime.debug.data_dump(order);
```

Rules:

- The segments after `std` up to the first segment that starts with an
  upper-case letter form the module path (`std.fs.path.Path`: module
  `fs/path`, declaration `Path`); the rest of the chain is ordinary member
  access (`std.compare.Order.less`). A path of lower-case segments only names
  a native action of its module and is valid only when called
  (`std.runtime.debug.data_dump(v)`).
- In type positions the last segment is the type and the segments before it
  form the module path: `std.io.stdout.Stdout`, `@std.fs.path.Path`.
- A path behaves exactly like a namespace import of that module made by the
  same file (5.2): the module is resolved in the bundled standard library
  directory only, never in runtime import directories or the importing
  directory, and it is registered once per program. A path and an explicit
  import of the same standard library module name the same declarations and
  the same nominal types.
- A path that names a module without a declaration (`std.io.stdout` as a
  value), a missing module, or a missing declaration is a semantic error.
- `std` is reserved: it cannot name a variable, parameter, field, loop or catch
  binding, or an import alias, and it is not a value on its own.
- A program cannot use two different files that declare the same module path.
  A local module that shadows a standard library module (5.2) therefore cannot
  be combined with a `std.` path to that module.

### 5.2.2 Blade Projects and Dependencies

A directory containing `blade.toml` is a package root. A file belongs to the
package of the nearest such directory above it; a project is a package that
is not installed in another project's `pkg/`.

```toml
[module]
name = "app"
version = "1.0.0"

[dependencies]
web = "12.3.0"
json = { git = "https://example.org/json", version = "1.*" }
```

```okkama
module app;

import "pkg/web/router" take Router;
```

Rules:

- The version of a package lives in `blade.toml`. A module file of a package
  MUST NOT declare a version; one that does is a semantic error at the version.
- Dependencies are installed in `pkg/<name>/` of the project root, each with
  its own `blade.toml`. A package installed there belongs to that project.
- An import path starting with `pkg/` names a dependency. It resolves against
  the root of the project only, never against the importer's directory, system
  directories or the standard library.
- `pkg/<name>/…` MUST name a dependency declared in `[dependencies]` of the
  importing file's own package: the project for its files, the dependency for
  a dependency's files. Transitive dependencies are not importable. Outside
  any package `pkg/` has no special meaning and is an ordinary directory.
- Other imports of a package file resolve as in §5.2: beside the importing
  file, then from its package root, so a package keeps its internal imports
  wherever it is installed.

### 5.3 Module Native Declarations

A runtime substrate module MAY declare a required host-provided function with
`native`:

```okkama
module runtime/time v1.0.0;

native object TimerHandle;
native wall_clock() is int;
native monotonic_clock() is int;
native sleep(duration is int) is bool;
native timer_create(delay is int, interval is int) is TimerHandle;
native timer_wait(timer is TimerHandle) is bool;
native timer_cancel(timer is TimerHandle) is bool;
native timer_cancelled(timer is TimerHandle) is bool;
native timer_deadline(timer is TimerHandle) is int;
native timer_close(timer is TimerHandle) is bool;
```

A native declaration is a declaration, not an action body or a host-language
name. It MUST be module-scoped, explicitly typed, terminated by `;`, and MUST
NOT contain an Okkama body. Fixed typed parameters are supported. `main`,
`byte`, async declarations, variadic parameters, parameter defaults, action
value parameters/results, and native constructors are not supported native
action forms in the current implementation. Anonymous actions never cross the
native ABI.

A module may declare a bodyless opaque nominal capability type:

```okkama
native object FileHandle;
native open(path is @byte) is FileHandle;
native close(handle is FileHandle) is bool;
```

`native object` has no fields, actions, constructors, body or generic
parameters. Source can use it only as a declared type; only a matching native
provider can mint a value. Its runtime value is opaque, immutable, cannot cross
`any`, cannot be a map key, cannot be compared or reflected, and cannot expose
provider identity, token, generation or a host resource.

Provider-owned handle tables validate nominal type, provider authority, token,
generation and open/closed state. Close is idempotent and finalizes at most
once. Other operations after close return catchable `already_closed`. Runtime
shutdown closes every remaining handle, including after exceptional program
exit, without relying on host garbage collection.

An owning native adapter may atomically transfer an open resource into another
opaque handle. Transfer tombstones the source without running its finalizer;
the receiving table must immediately install the sole replacement finalizer.
`runtime/tls` uses this rule to consume a connected `SocketHandle`.

### 5.4 Basic Native Byte I/O

The core declaration module `runtime/fs` provides the first opaque-resource
vertical:

```okkama
native object FileHandle;
native object DirectoryHandle;
native path_encode(path is string) is @byte;
native path_decode(path is @byte) is string;
native open(path is @byte, flags is int, mode is int) is FileHandle;
native read(handle is FileHandle, size is int) is @byte;
native write(handle is FileHandle, bytes is @byte) is int;
native seek(handle is FileHandle, offset is int, origin is int) is int;
native flush(handle is FileHandle) is bool;
native truncate(handle is FileHandle, size is int) is bool;
native sync(handle is FileHandle) is bool;
native lock_shared(handle is FileHandle) is bool;
native lock_exclusive(handle is FileHandle) is bool;
native unlock(handle is FileHandle) is bool;
native close(handle is FileHandle) is bool;
```

Open flag bits are read `1`, write `2`, create `4`, truncate `8`, append `16`
and exclusive create `32`. Exactly one of read, write or their combination is
required. Mode is `0..0777`. Seek origins are start `0`, current `1`, end `2`.

Read and write each perform one provider attempt. Read returns at most the
requested bytes; an empty successful read is EOF. Write returns the written
prefix length and callers decide whether to retry a partial result.
Interruption before progress is a catchable `interrupted`. Flush does not imply
durable storage. Exclusive-create collision is `already_exists`. Raw errno,
filehandles and host exceptions remain outside the public language API.
Runtime diagnostics may retain the failed filesystem operation, exact path
operands and optional native code/reason. These survive wrapping and rethrow;
they do not add reflection or methods to `NativeFailure`. Portable logic uses
typed stdlib errors or `domain()`/`code()`, never OS text or numeric errno.
`truncate` requires a writable handle and a nonnegative size. `sync` requests
durable host synchronization. Shared/exclusive locks are blocking advisory
locks, require readable/writable capability respectively, and `unlock` is
idempotent.

All low-level filesystem paths are immutable raw `@byte` values. Text crosses
this boundary only through the fallible UTF-8 `path_encode`/`path_decode`
operations. `stat/lstat` return a stable `%string:int` projection containing
only `kind`, `size`, `permissions`, `accessed_sec`, and `modified_sec`.
Directory iteration uses opaque `DirectoryHandle`; `readdir` skips `.`/`..`,
returns a raw byte name, and signals EOF with an empty `@byte`. That result may
be passed directly to another path operation without decoding. `umask` is not
part of the ABI because it mutates process-global rather than runtime-local
state.

The declaration-only `runtime/stdio` module exposes raw process standard
streams:

```okkama
native stdout_write(data is @byte) is int;
native stderr_write(data is @byte) is int;
native stdin_read(limit is int) is @byte;
native stdin_read_line() is @byte;
```

`stdin_read` rejects non-positive limits and returns at most the requested
number of bytes; empty is EOF. It and `stdin_read_line` consume one shared
provider buffer, so interleaving them neither duplicates nor loses bytes.
Line reads preserve their terminator and return a final unterminated tail
before EOF. Standard output and error are distinct complete-write channels;
their byte counts and procedural names belong to the runtime ABI, not to the
high-level object API.

The declaration-only `runtime/memory` module exposes generic owned byte state:

```okkama
native object ByteMemoryHandle;
native byte_memory_create(initial is @byte) is ByteMemoryHandle;
native byte_memory_bytes(memory is ByteMemoryHandle) is @byte;
native byte_memory_size(memory is ByteMemoryHandle) is int;
native byte_memory_consume(memory is ByteMemoryHandle, limit is int) is @byte;
native byte_memory_append(memory is ByteMemoryHandle, value is @byte) is int;
native byte_memory_reset(memory is ByteMemoryHandle) is bool;
native byte_memory_closed(memory is ByteMemoryHandle) is bool;
native byte_memory_close(memory is ByteMemoryHandle) is bool;
```

It owns storage and lifecycle only. `io/buffer` builds `Buffer`,
`StringBuffer`, `BufferedInput`, and `BufferedOutput` above it in Okkama.
Line boundaries, UTF-8, capacities, prefetch, threshold draining,
partial-write recovery, flush and wrapped-stream ownership are not runtime ABI.
Acquisition is explicit through state-only projection objects; closing a
buffering decorator closes its private cache but not the wrapped I/O object.

The declaration-only `runtime/utf8` module defines the explicit Unicode-text
boundary:

```okkama
native encode(value is string) is @byte;
native decode(value is @byte) is string;
native valid_prefix(value is @byte) is int;
```

Encoding emits canonical UTF-8 without BOM insertion or normalization.
Decoding is strict and never replaces invalid input. `valid_prefix` returns
the complete input size when every byte forms well-formed UTF-8 scalar
sequences; otherwise it returns the byte offset at which the first ill-formed
or truncated sequence begins. Its result is always in `0..value.size()` and
it never fails for an ordinary `@byte` argument. It rejects standalone
continuations, overlong forms, UTF-16 surrogates, scalars above `U+10FFFF`,
invalid continuations and incomplete tails. `decode(value)` succeeds exactly
when `valid_prefix(value) == value.size()`.

The high-level `encoding/utf8` module exposes immutable projection objects:

```okkama
let bytes is @byte = bytes Utf8Encoded(text);
let text is string = string Utf8Decoded(bytes);
```

`Utf8Decoded` validates before decoding and throws `InvalidUtf8`, a builtin
`Error` implementation whose `offset()` is the first ill-formed sequence's
byte offset. Raw provider messages are not parsed or exposed as public
decoding policy.

The pure-Okkama `encoding/binary` module uses explicit binary values rather
than a mutable or fluent writer:

```okkama
let parts is @BinaryPart = [
    Unsigned16BigEndian(258),
    Signed32LittleEndian(-2),
];
let packet is @byte = bytes BinarySequence(parts);
let reader = BinaryReader(bytes: packet);
```

`BinaryByte`, `BinaryBytes`, every signed/unsigned endian value and
`BinarySequence` implement `BinaryPart`, whose `bytes()` action returns
`@byte`. Their constructors bind only source state. Numeric range checking and
encoding occur in `bytes()`, and a sequence materializes its retained parts in
order. Reads remain explicit by offset and endianness; no host byte order,
hidden cursor or copy-setter builder is observable.

The high-level `encoding/base64` module is a pure-Okkama byte codec:

```okkama
let encoded is string = string Base64Encoded(bytes);
let decoded is @byte = bytes Base64Decoded(encoded);
```

It accepts and emits only canonical padded RFC 4648 with the standard `+` and
`/` alphabet. Decoding rejects whitespace, URL-safe and unpadded forms,
misplaced padding, invalid alphabet symbols and non-zero unused tail bits.
Failures throw `InvalidBase64`, whose `offset()` is the deterministic
zero-based source character offset. Text requires an explicit `encoding/utf8`
projection before or after Base64; no implicit string/byte conversion exists.

The high-level `encoding/hex` module is likewise a pure-Okkama byte codec:

```okkama
let encoded is string = string HexEncoded(bytes);
let decoded is @byte = bytes HexDecoded(encoded);
```

Encoding emits exactly two lowercase `0-9a-f` characters per byte. Decoding
accepts lowercase and uppercase ASCII hex, but rejects whitespace, prefixes,
separators and non-ASCII symbols. Empty input is valid. Odd source length
throws `InvalidHex` at `text.length()` before inspecting characters; otherwise
`offset()` identifies the first invalid zero-based Unicode code-point
position. No implicit text encoding or host/native hex codec is involved.

The pure-Okkama `encoding/json` module models JSON with a nominal `Json` role
implemented by object, array, string, number, boolean and null values. Every
node exposes an exact `JsonKind` and compact deterministic `encoded()` output:

```okkama
let values is @Json = [
    JsonString("Ada"),
    JsonNumber("1e400"),
    JsonBool(true),
    JsonNull(),
];
let generated = JsonObject([
    JsonMember("values", JsonArray(values)),
]);
let parsed is Json = json JsonSource("{\"emoji\":\"\\uD83D\\uDE00\"}");
```

Every constructor binds state only. `encoded()` validates and emits compact
deterministic JSON; `JsonSource` performs strict recursive parsing in its
`#projected json()` projection. Object order is retained, duplicate names are rejected,
and array bounds fail through typed errors. JSON strings support every JSON
escape, combine valid UTF-16 surrogate pairs, reject lone surrogates and use
canonical control escapes when encoded.

`JsonNumber` retains the exact strict RFC 8259 decimal lexeme rather than an
Okkama `int` or `float`. Consequently a valid number outside the runtime's
numeric range, such as `1e400`, parses and re-encodes exactly. `int()` and
`float()` are separate checked projections and throw
`InvalidJsonNumberConversion` when the requested Okkama representation is not
available. Located parse failures expose a zero-based offset in the original
Unicode source and one-based line/column; CRLF is one line break but remains
two source code points. No host JSON parser or JSON-shaped native ABI exists.

`JsonReader(source)` is an immutable, schema-oriented cursor over this object
model. `required(name)` and `at(index)` return new readers and retain a
diagnostic path such as `$.users[0]`. Current-node `string()`, `int()`,
`float()` and `bool()` validate the JSON kind. The corresponding
`required_*` member conveniences fail on missing, null or wrong-kind members.
`optional_*(name, fallback)` returns its same-typed fallback only for an absent
member or explicit JSON null; a present value of the wrong kind throws
`UnexpectedJsonKind`. Encoding remains explicit through the six nominal JSON
value objects—`JsonReader` does not reflect over domain objects.

### 5.5 Native Processes and Pipes

The declaration-only `runtime/process` module exposes opaque `PipeHandle` and
`ProcessHandle`. `spawn(program is @byte, arguments is @@byte, input, output,
errors)` is shell-free and requires three distinct pipes. The child receives
the input read endpoint as stdin and output/error write endpoints as
stdout/stderr; the parent retains only the complementary endpoints.

`wait` and `try_wait` return exactly `%string:int` keys `state` and `code`.
State is running `0`, exited `1`, or signaled `2`; code is the exit code or a
portable signal code (terminate `1`, force `2`, interrupt `3`, unknown `0`).
No raw wait status or host signal number crosses. Spawn inherits current host
environment/cwd, never invokes a shell, and reports exec failure directly.
Every unrelated descriptor is close-on-exec. The runtime owns children and
force-terminates/reaps a still-running child on process close or shutdown.
Public `fork` and host-process `exit` are not part of this ABI.

`spawn_configured` adds `directory is @byte` and `environment is @@@byte`.
The directory is nonempty. Environment entries are exact `[name, value]`
pairs; names are nonempty and contain neither NUL nor `=`, values contain no
NUL, and duplicate names are invalid. The child replaces its environment and
changes cwd after `fork` and before `exec`; parent cwd/environment never
change. Setup and exec errors use the same close-on-exec handshake and the
canonical `runtime/process@1::spawn_configured/7` operation.

### 5.6 Native Networking and DNS

`runtime/net` declares opaque `SocketHandle` and blocking IPv4/IPv6 stream and
datagram operations. It never exposes host sockaddr layouts. A portable IP
endpoint is versioned `@byte`: `OK`, version `1`, family, big-endian port, and
exactly 4 or 16 address bytes. A received datagram is version `2` containing
one endpoint envelope followed by payload; dedicated extractors validate it.

Portable families are IPv4 `1` and IPv6 `2`; types are stream `1` and datagram
`2`; protocols are default `0`, TCP `1`, UDP `2`. DNS `resolve` returns ordered
deduplicated `@@byte` endpoint candidates, while `reverse_name`, `ip_parse`, and
`ip_format` normalize the remaining name/address operations. Portable boolean
options are reuse-address `1`, keepalive `2`, TCP no-delay `3`.

Stream/datagram send and receive make one host attempt and expose partial
progress. Empty stream receive is EOF. Sockets are binary, close-on-exec,
authority checked and finalized at runtime shutdown. The individual M11 I/O
operations remain synchronous; M12 readiness lets callers wait before making
that one attempt.

The high-level `net/address` module keeps that envelope private. Its
constructors only bind immutable input state and never call a native provider.
`IpAddress` actions accept only `AddressFamily.ipv4` or `.ipv6`, canonicalize
numeric syntax through `ip_parse`/`ip_format`, and expose exactly four or
sixteen network-order octets from `bytes()`. The `.unix` enum case is reserved
for a later Unix-domain address type and is invalid when an IP action is
queried.

`SocketAddress(address, port)` composes the address and inclusive portable
port domain `0..65535` directly. Its canonical text is
`192.0.2.1:80` for IPv4 and `[2001:db8::1]:80` for IPv6. `HostPort` retains an
unresolved nonempty host and validated port; it does not perform DNS, IDNA or
network access. Invalid input produces builtin-`Error` implementations
`InvalidAddress` or `InvalidPort`; native parse/format diagnostics never cross
the stdlib boundary. Validation and derived values are action work, not
constructor work.

### 5.7 Native Poll/Eventing

`runtime/poll` declares opaque `PollHandle`. Exact typed add operations observe
`FileHandle`, `PipeHandle` or `SocketHandle`; handles are not erased through
`any` and host descriptors never cross the ABI. Add returns a positive
registration id and retains a signed integer user tag. Modify preserves the id
while replacing mask/tag; remove invalidates it.

Portable masks are readable `1`, writable `2`, or both `3`. `poll_wait`
returns registration-sorted `@%string:int` entries containing exactly
`registration`, `tag`, and `events`. EOF/peer close is readable and the next
read distinguishes bytes from EOF. There is no host-specific public HUP bit.

Wait deadline is absolute monotonic nanoseconds: `-1` is unbounded, an elapsed
deadline is an immediate check, and values below `-1` are invalid. Host EINTR
is retried against the same deadline. `poll_cancel` contributes one pending
cancellation; the next/current wait consumes it as catchable NativeFailure
code `interrupted`. Poll close is idempotent, does not close observed handles,
and runtime shutdown finalizes leaks.

### 5.8 Native Environment, Process Information and Secure Random

`runtime/environment` represents environment names and values as opaque
`@byte`. Names are nonempty and cannot contain NUL or `=`; values may be empty
but cannot contain NUL. Missing get is `not_found`, distinct from a present
empty value. Remove returns whether a value existed. `environment_all()`
returns byte-sorted `@@@byte` entries, each exactly `[name, value]`; it is not a
text-keyed map. Mutation changes the ambient host process and is inherited by
later spawned children.

The high-level `env/var` module exposes that ABI through nominal objects:

```okkama
import "env/var" take Environment, Env, EnvVar, EnvEntry;

let environment is Environment = Env();
let home = EnvVar("HOME");

if environment.has(home) {
    environment.value(home); // strict UTF-8 string
    environment.bytes(home); // exact host bytes
}

let snapshot is @EnvEntry = environment.variables();
```

`EnvVar` and `EnvEntry` constructors only retain state. Names are checked when
an action observes them. Raw values need not be UTF-8; only `value()` and
`EnvEntry.string()` decode them, throwing `InvalidEnvEncoding` on malformed
input. `MissingEnvVar` distinguishes absence from an empty value.
`Environment.fake()` is available in test mode, owns instance-local state,
returns byte-sorted detached snapshots and never touches the ambient process.

`runtime/process.process_arguments()` returns a defensive `@@byte` snapshot of
explicit application arguments. The CLI source path is excluded. Current and
parent process ids are integer diagnostic identities, not resource handles.
This does not add parameters to language `main()`.

The high-level `proc/args` module provides explicit raw and text views:

```okkama
import "proc/args" take Args;

let arguments = Args();
arguments.bytes();
if arguments.size() > 0 {
    arguments.at(0);
}
```

`Args()` is stateless because the runtime already owns the immutable snapshot.
`bytes()` preserves exact bytes. `strings()` strictly decodes all values in
ascending order; `at(index)` strictly decodes one. Malformed UTF-8 throws
`InvalidArgumentEncoding` with the zero-based argument index. Negative and
upper-bound access throws `MissingArgument` retaining the requested index.
No replacement decoding or synthetic source/executable argument is added.

`runtime/system` returns closed portable ASCII platform and architecture
spellings, using `unknown` for an unrecognized host. `secure_random(size)`
returns exactly `size` OS-CSPRNG bytes, rejects negative size, caps one request
at 16 MiB, retries interruption and never falls back to a language PRNG. An
unavailable cryptographic source is an explicit failure.

The high-level `os` module turns the identity strings into a closed object
contract:

```okkama
import "os" take
    Host, OperatingSystem, Posix, Windows,
    Platform, Architecture, PathStyle;

let host is OperatingSystem = Host();
let foreign is OperatingSystem = Posix(
    Platform.freebsd,
    Architecture.arm64,
);

host.platform();
host.path_style();
foreign.path_list_separator();
```

`Host()` is stateless and consults `runtime/system` only when an action is
evaluated. All known non-Windows platforms have `PathStyle.posix`, LF and `:`;
Windows has `PathStyle.windows`, CRLF and `;`. The literal runtime identity
`unknown` remains `Platform.unknown`, but its path-style and separator actions
throw `UnsupportedPlatform` instead of guessing. `Posix` and `Windows` are
explicit foreign/test descriptions whose overloaded constructors only retain
enum state. `Posix(Platform.windows, ...)` is rejected when observed.

The pure-Okkama `fs/path` module builds lexical path values above that closed
OS convention:

```okkama
let native is PathValue = Path("var").joined("log");
let archive is PathValue = PosixPath("/srv//data/../archive").normalized();
let artifact is PathValue = WindowsPath("C:/build/./out/app.exe").normalized();
```

`Path`, `PosixPath`, and `WindowsPath` constructors retain only their source
string. Validation and host selection occur in actions. `Path` asks
`Host().path_style()` and propagates `UnsupportedPlatform` for an unknown host;
explicit foreign forms never consult the host. No path action tests existence,
resolves links or imports `runtime/fs`.

`string()` preserves a valid source spelling and `bytes()` is its exact UTF-8
encoding. Only `normalized()` changes spelling: it lexically removes repeated
separators and `.`, resolves `..` without crossing a root, removes non-root
trailing separators and maps an empty relative path to `.`. `parent()` uses
the normalized value. A single relative component has parent `.`, while a
value already at `.`, a path root, bare drive context or UNC share root throws
`MissingPathParent`.

`joined(part)` accepts only a relative fragment; empty is an identity. A
POSIX-rooted, Windows-rooted, drive-qualified or UNC argument throws
`InvalidPath` rather than discarding the base. POSIX rejects NUL and treats
backslash as an ordinary character. Windows accepts both source separators and
normalizes them to `\`; drive-rooted and complete UNC paths are absolute,
whereas `C:name` and `\name` are respectively drive-relative and root-relative.
Portable Windows validation rejects control characters, `"<>|?*`, malformed
colon placement, incomplete UNC roots and device namespaces.

The high-level `crypto/random` module turns that procedural boundary into a
stateless capability:

```okkama
let source is SecureByteSource = SecureRandom();
let nonce is @byte = source.bytes(32);
```

Its request domain is `0..16777216`. Invalid sizes throw
`InvalidSecureRandomSize` before native dispatch. A zero-byte request succeeds
without consulting an entropy device. Every provider failure for a valid
nonempty request becomes the fixed `UnavailableSecureRandom` error; native
domains, codes and messages do not cross the stdlib boundary. A successful
provider must return the exact requested count or trigger an invariant panic.
`SecureByteSource.fake()` is test-only and returns predictable zero bytes;
production never substitutes it or another PRNG for unavailable OS entropy.

The top-level `random` module composes that byte capability into the complete
`RandomSource` object contract:

```okkama
let secure is RandomSource = Random();
let repeatable is RandomSource = SeededRandom(42);
let die is int = secure.int(1, 6);
let fraction is float = secure.float();
let chosen is string = value Choice(["red", "green"], repeatable);
let order is @int = values Shuffled([1, 2, 3, 4], repeatable);
```

`Random.int(minimum, maximum)` includes both endpoints and samples without
modulo bias across the complete signed 64-bit `int` domain. Equal endpoints
consume no entropy. `bool()` is equiprobable, while `float()` consists of
53-bit discrete values in `[0.0, 1.0)`. `Random.bytes` preserves the exact
size and failure semantics of `SecureRandom`.

`SeededRandom(seed)` is an explicitly non-cryptographic, pure deterministic
query object. Equal seed, action and arguments produce equal values; a shorter
byte request is a prefix of a longer one. It does not hide a mutable stream
cursor. Any future sequential random object must expose state succession in
its public contract. `Choice<T>` and `Shuffled<T>` use `Random()` by default
or retain an explicitly injected `RandomSource`. Their constructors perform
no selection. Empty choice and reversed ranges throw `InvalidRandomRange`;
singleton choice and shuffle consume no entropy, and shuffling never mutates
the input array.

Unix environment/argv retain host bytes. Windows Unicode environment/argv use
a strict UTF-8 byte projection. No implicit decoding occurs in Okkama source.

### 5.9 Native Formatting, Cryptography and TLS

`runtime/format` is the privileged boundary for rendering opaque `any`
values. `concat_values(@any)` concatenates display forms for standard streams.
`format_values(pattern, @any)` consumes positional `{}` fields and
`format_named(pattern, %string:any)` consumes identifier fields. `{{` and
`}}` are literal braces. Malformed patterns and missing values are
`invalid_input`; extra values are allowed. Native declarations do not overload,
so concatenation and pattern formatting have distinct names.

The high-level `format` module exposes two immutable projections:

```okkama
let positional is string = string Formatted("{}: {}", "count", 3);
let named is string = string NamedFormatted(
    "{name}: {count}",
    ["name": "Ada", "count": 3],
);
```

`Formatted` materializes its variadic `...any` tail as private `@any` state;
the caller passes positional values directly. The `NamedFormatted` constructor
contextualizes its map literal as `%string:any`. `Formatted` accepts only empty `{}` fields;
`NamedFormatted` accepts ASCII `[A-Za-z_][A-Za-z0-9_]*` names. Fields are
consumed in encounter order, named fields may repeat, and surplus supplied
values are allowed. In both modes `{{` and `}}` emit literal braces.

Pattern validation and missing-value detection run in Okkama actions before
native rendering. Malformed syntax throws `InvalidFormat`; `offset()` is the
first offending zero-based Unicode code-point offset. Missing fields throw
`MissingFormatValue` whose text identifies the positional index or named key.
Constructors only retain source state. Once validation succeeds, a provider
failure is an invariant panic with deterministic text; host/provider messages
do not become public format errors.

The pure-Okkama `template/plain` module is a distinct string-template layer,
not another `runtime/format` adapter:

```okkama
let values = TemplateValues(
    TemplateBinding("name", "Ada"),
    TemplateBinding("language", "Okkama"),
);
let message is string = Template(
    "Hello, <%= name %> from <%= language %>!",
).string(values);
```

`Template`, `TemplateBinding`, and the variadic `TemplateValues` retain state
only. `Template.string(values)` validates and produces the result; the former
`render()` proposal is rejected because a value-returning query names the
expected `string`. Bindings are exact strings, placeholder names are
case-sensitive ASCII `[A-Za-z_][A-Za-z0-9_]*`, and only ASCII space/tab may
surround a tag name.

`<%= name %>` is normal output, `<%== name %>` is the explicit raw channel,
`<%# ... %>` is a non-nesting comment ending at the first `%>`, and `<%%`
emits literal `<%`. Plain output uses an identity escaping policy, so normal
and raw channels preserve the same exact value; the lexical distinction is
reserved for an HTML adapter. Ordinary braces and `%>` outside a tag are
literal. Every `<% ... %>` control tag is reserved and invalid in v1: no tag
executes Okkama or host code.

The complete source is validated before value lookup. Malformed or unclosed
syntax throws `InvalidTemplate` at the first zero-based Unicode code-point
offset. Missing and duplicate bindings throw `MissingTemplateValue` and
`DuplicateTemplateValue` with their exact name. Surplus bindings are allowed,
source/replacement line endings are unchanged, and inserted text is never
interpreted recursively. The module has no native ABI dependency.

`runtime/crypto` accepts only lowercase `sha256`, `sha384`, and `sha512` for
`digest` and `hmac`; all operands/results are exact bytes and other algorithms
are `unsupported`. `secure_equal` visits every position up to the greater
operand length and folds length difference into its result without an early
content return.

`runtime/tls` consumes a connected stream `SocketHandle` into an owning
`TlsHandle`. Client configuration always verifies the certificate chain and
the explicit server name; server configuration accepts explicit PEM
certificate/key bytes. Handshake is explicit and idempotent. Reads/writes
operate on decrypted/encrypted bytes with visible partial progress. The fixed
peer projection consists of subject and issuer strings, Unix-second validity
bounds, and leaf DER bytes. Close is idempotent and runtime shutdown owns
unfinished sessions. The Perl runtime declares `IO::Socket::SSL` and
`Net::SSLeay` as required provider dependencies.

Native identity belongs to the declaring module and is preserved across import
flattening. The canonical fixed-signature notation is:

```text
module/path@ABI_MAJOR::name/fixed_arity
runtime/time@1::monotonic_clock/0
```

Minor/patch module versions and consumer import spelling do not alter the ABI
identity. Parameter and result types form a separate exact descriptor
fingerprint. A source declaration is callable only when visible through the
ordinary module/import/`take` rules and when the runtime registry contains the
matching canonical provider descriptor. A provider registered without a
visible source declaration does not create a callable language name.

Missing and mismatched mandatory providers are runtime errors. Native
declarations have no Okkama fallback body:

```text
Native symbol "runtime/time@1::monotonic_clock/0" is not registered
Native symbol "..." provider signature does not match declaration
```

The executable fixed ABI accepts positional arguments only. Each argument is
evaluated once from left to right, checked against its declared type and
converted to an explicit logical native value. Supported categories are `int`,
`float`, `bool`, `string`, `char`, `byte`, enums, typed arrays, typed maps,
specialized `@byte` ByteSequence values and `any`. Ordinary object/role values
and interpreter-only wrappers cannot cross. Handlers receive an immutable
logical call context containing the canonical symbol and ordered logical
values; they do not receive the AST, interpreter, caller environment or private
object state. Results are recursively validated and materialized into fresh
language storage.

A handler MAY instead return a normalized `NativeFailure(domain, code,
message)` outcome. The runtime attaches the current canonical symbol as its
`operation`, materializes the builtin `NativeFailure` value and raises it
through the ordinary typed catchable channel. Source cannot construct this
value. A catch may inspect only `.domain()`, `.code()`, `.operation()` and
`.message()`, all returning `string`. Control flow MUST branch on stable
domain/code, never diagnostic message or raw host errno. Unexpected host
exceptions, unknown/malformed outcomes and malformed logical values remain
sanitized uncatchable provider failures.

The bundled declarations can be used as follows:

```okkama
import "runtime/time" take wall_clock, monotonic_clock, sleep;

main() is int {
    let instant = wall_clock();
    let started = monotonic_clock();
    sleep(1000000);
    monotonic_clock();
}
```

`monotonic_clock()` returns a nonnegative `int` measured in nanoseconds from an
unspecified runtime-local origin. Values are meaningful only for ordering and
differences within the same runtime. It is not wall time and has no calendar
meaning. `wall_clock()` returns signed integer nanoseconds since the Unix epoch
and may move when host civil time is corrected. `sleep(duration)` consumes
nonnegative relative nanoseconds, returns `true` after reaching its monotonic
deadline, retries host interruption, and may oversleep. A negative duration is
catchable `core.time/invalid_input`. Nanosecond units do not promise
nanosecond host resolution or accuracy.

`TimerHandle` is an opaque, authority-checked live resource. A zero interval
creates a one-shot and a positive interval creates a periodic timer. Relative
delay and interval inputs are nonnegative exact nanoseconds; deadline overflow
is `core.time/invalid_input`. Cancellation and close are idempotent. Closed
handles fail as `already_closed`, cancelled waits as `interrupted`. Periodic
state advances on `timer_wait` using anchored fixed-rate scheduling and skips
missed ticks.

The high-level `time/duration` module represents time spans independently of
any clock or provider:

```okkama
import "time/duration" take
    Duration, Nanoseconds, Milliseconds, Seconds, Minutes, Hours;

let timeout is Duration = duration Seconds(30);
let retry is Duration = timeout.plus(duration Milliseconds(500));
let rewind is Duration = (duration Minutes(2)).times(-1);
```

Every duration is a signed integer count in
`-9223372036854775808..9223372036854775807` nanoseconds. Negative spans are
valid. `plus`, `minus`, `times`, comparison and all named-unit projections are
pure and preserve exact integer arithmetic. A result outside the closed range
throws `InvalidDuration`; wrapping, saturation and intermediate float
conversion are forbidden. All constructors only bind their source `int`;
validation and scaling occur in actions. `time/duration` does not call
`runtime/time`. The clock adapter must reject a negative `Duration`
before passing relative nanoseconds to native `sleep`.

The implemented clock-bound layer is split deliberately:

- `time/instant` retains exact nanoseconds plus the closed
  `ClockIdentity.system`/`ClockIdentity.monotonic` identity;
- `time/clock` provides role `Clock` and `SystemClock`, `MonotonicClock`, and
  composed `OffsetClock` implementations;
- `time/date` provides proleptic Gregorian `Date`, UTC `DateTime`, and the
  initial UTC-only `TimeZone` policy;
- `time/timer` owns explicitly acquired live one-shot and periodic resources.

`Instant.after`/`before` MUST preserve identity. `since` and `less` MUST throw
`IncompatibleClocks` when identities differ. System instants are signed Unix
nanoseconds; monotonic instants have no calendar meaning. `Clock.sleep` MUST
reject negative or out-of-range relative deadlines as `InvalidSleepDuration`
before provider details can escape.

`Date` supports the proleptic Gregorian years `1..9999`; `0001-01-01` is
Monday and rendering is `YYYY-MM-DD`. Unix-nanosecond projection MUST use floor
semantics for negative instants. `TimeZone("UTC")` is the only accepted v1 zone
until Okkama ships versioned zone data; a host local zone or host tzdata lookup
MUST NOT be implicit. Monotonic instants MUST NOT become `DateTime` values.

Timers are inherently live resources and therefore use the same explicit
ownership exception as files and sockets. Schedule values have state-only
constructors and acquire through projection:

```okkama
let once is Timer = timer ScheduledTimer(duration Milliseconds(10));
let ticker is PeriodicTimer = timer ScheduledPeriodicTimer(duration Seconds(1));

once.wait();
ticker.cancel();
ticker.cancelled(); // true
```

`cancel()` is an idempotent command; the boolean observation is the separate
`cancelled()` action. Waiting after cancellation throws `CancelledTimer`.
Periodic timers are fixed-rate, first fire after one interval, and skip missed
ticks to the first future anchored deadline. They MUST NOT burst to replay
every missed tick or silently change to fixed-delay scheduling. Timer destroy
MUST close its exact opaque `runtime/time.TimerHandle`; runtime shutdown owns
leaked handles.

The former `Object.action/N` callback registry has been removed. Ordinary
actions always execute their Okkama bodies. Host implementations exist only
for explicit bodyless module-native declarations with mandatory providers.

## 6. Types

### 6.1 Built-In Runtime Types

Current runtime-supported value categories:

```text
int
byte
float
string
bool
any
array
map
range
object
action
undefined
```

The public type spelling used in source includes:

```okkama
int
byte
float
string
char
bool
any
@int
@any
%string:int
%string:any
Cash
^Cash
(int) is int
() is string
```

`char` literals are represented by a distinct runtime value. They infer as
`char`, format as source-like character literals, and compare by value only with
other `char` values.

`byte` is a distinct integer-like runtime value whose range is exactly
`0..255`. It is constructed with the checked exact bare call `byte(value)`:

```okkama
let zero is byte = byte(0);
let maximum is byte = byte(255);
```

The argument MUST be an `int`, `byte`, `float` or `rat`. Fractional values
are truncated toward zero before checking `0..255`; an existing `byte` is
preserved. There is no implicit narrowing, modulo wrapping, saturation or
byte literal syntax. `@byte(...)` constructs an immutable byte array using
these same explicit conversions (see section 6.6.1).
`byte` satisfies the numeric umbrella type `number`, but it does not satisfy
`int`; the reverse conversion is also never implicit. Because this is an
ordinary call expression in the grammar, no dedicated lexer or parser form is
introduced. The builtin applies only to the exact bare callee `byte`; member
calls named `byte` use ordinary action dispatch.

### 6.2 Top Type `any`

`any` is the explicit top type for ordinary defined values. It is a transport
boundary for heterogeneous APIs, not a switch to general dynamic semantics:

```okkama
let value is any = 42;
```

Crossing a declared `any` boundary stores an opaque immutable box retaining the
concrete runtime category. Concrete values are assignable to `any`, but `any`
is never implicitly assignable to a concrete type, even when its hidden value
has that type. Repeated `any` boundaries do not add nested boxes.

Language code may pass, store, return, catch, unpack, and iterate these boxes,
but may not inspect them through arithmetic, comparison, boolean, range,
member, call, indexing, map-key, or match operations. Explicit narrowing is a
future feature. `undefined`, internal variadic/expansion/successor/control
values, host callbacks, unsupported host objects, and cyclic containers cannot
be stored as `any`.

Explicit collections preserve the same opacity recursively:

```okkama
let values is @any = [1, "two", 3.0];
let row is %string:any = ["id": 7, "name": "Ada"];
```

Map values may be `any`; map keys may not, because the current map backend
cannot preserve typed key identity. Untyped heterogeneous literals keep their
ordinary concrete elements and do not infer `any` automatically. Role and fake
signatures require the exact spelling `any`; it does not introduce parameter or
return variance.

Trusted native dispatch unboxes only parameters declared exactly `any` and
elements declared `...any`. Nested boxes inside `@any` or `%K:any` remain
opaque, and arrays/maps cross as defensive snapshots. Native results still
cross the declared return boundary and are re-boxed for `is any`.

Action values cannot cross an `any` boundary: `let hidden is any = () is int { 1; };`
is an error. `any` uses the existing ordinary type-reference grammar; it is
not a keyword and requires no dedicated lexer or parser production.

### 6.3 Collection Shorthand Types

Array type:

```okkama
let xs is @int = [1, 2, 3];
```

Map type:

```okkama
let prices is %string:int = ["apple": 120, "pear": 140];
```

The map type `%K:V` means keys of type `K` and values of type `V`.

Runtime type checking validates collection contents recursively for `@T` and
`%K:V` at the primary typed boundaries: `let` declarations, object fields,
constructor parameters, action parameters, and action return values. A later
static semantic pass may diagnose these mismatches before execution.

`@byte` is an ordinary immutable array whose elements are distinct `byte`
values. It has the same indexing, iteration, structural equality, nesting and
deep boundary validation as every other `@T`; it is not a mutable buffer or a
second public sequence type. An empty array remains vacuously compatible with
`@byte`, following the general array rule. Map keys support `string`, `char`,
`bool`, `byte`, `int`, `float` and nominal enums. Key identity includes its exact
type: `byte(1)`, `1`, `1.0`, `true` and `"1"` do not alias, and equal enum case
spellings from different nominal enums remain distinct. Object, action,
resource-handle and `any` keys are not scalar keys. Nominal qualification `::`
is never interpreted as the `%K:V` separator. Keys retain their type through
index/has/keys, functional operations, any snapshots, native ABI and worker codec.

### 6.4 Successor Types

A successor type is written with `^`.

```okkama
open() is ^Bottle {
    ^opened = true;
}
```

`^Bottle` means the action returns a new successor object of type `Bottle`.

### 6.5 Generic Type Parameters

Objects and roles MAY declare one or more ordered type parameters:

```okkama
object Box<T> {
    has value is T;
    Box(:value);
    get() is T { value; }
}

role Echo<T> {
    echo(value is T) is T;
}
```

Parameters MUST be uppercase type names, unique within the declaration, and
MUST NOT shadow the object or role name. Object parameter scope includes
fields, full and shorthand constructors, actions, successor actions,
and `destroy`. Role parameter scope includes action signatures and the role's
fake block. Parameters do not leak into another declaration or runtime caller.

Applied object and role types MUST supply exactly the declared number of arguments.
`Pair<int>` is invalid for `Pair<K, V>`, and a non-generic type cannot receive
arguments. A parameter such as `T` denotes a concrete type and cannot itself be
applied as `T<int>`.

The semantic rule is exact concrete identity:

- one `T` denotes one concrete type throughout one specialization;
- repeated occurrences of `T` must resolve to that identical type;
- `int`, `float`, `rat`, and `byte` are distinct and never implicitly unify;
- inference does not introduce a numeric supertype, union, widening, or
  fallback `any`;
- mixed equations fail before the declaration body executes;
- explicit casts are the path between numeric types.

Therefore `Min(1, 2, 3)` may infer `T = int`, while `Min(1, 2.0)` must fail.
Object parameters may declare one nominal role bound:

```okkama
object Heap<T: Comparing<T>> {
    // Every inferred T must satisfy the exact applied role.
}
```

The bound is checked after exact constructor inference and before the
constructor body executes. Bounds do not widen, convert, or help infer a
missing type. Self-referential bounds such as `T: Comparing<T>` are valid.
Bounded role parameters are not yet supported. There is no special category
constraint such as `T: number`, no variance, wildcards, default parameters,
multiple bounds, or higher-kinded types.

Constructor inference collects exact equations from direct parameters, nested
arrays, maps and specialized objects. Full and named shorthand constructors
participate. Repeated occurrences MUST agree. Empty collections provide no
equation; a direct declared target MAY provide missing evidence:

```okkama
let bucket is Bucket<int> = Bucket(values: []); // field/parameter is @T
```

A declared expected type supplies a one-way constraint to generic construction
in typed local initializers, constructor field bindings (including `self.field`),
explicit returns and tail results, and call arguments whose parameter type is
unambiguous. The constraint also flows through grouping, branches and collection
elements, and through constructor arguments after known type parameters have
been substituted. An unresolved parameter of an enclosing call is not evidence
for a nested construction. No inference is taken from unrelated later uses,
no overload search is retried using guessed specializations, and no global
backtracking is performed.

The caller's construction target is consumed for generic inference before
entering a constructor body. It MUST NOT retag reductions or other expressions
inside that body. Typed local initializers and constructor field bindings
(`field = ...` and `self.field = ...`) establish their own expected type, using
the field's resolved generic bindings. Caller context is restored on both normal
exit and failure. Operator result expectations do not constrain their operands;
binary/unary operands infer independently, preserving short-circuit evaluation.
Similarly, a method/property result target does not flow into its receiver;
an index result target does not flow into the indexed source or index expression;
an if result target applies to its branches, not its condition. Selected index
fallbacks retain the result target.

Propagation is recursive for array and map literals: a concrete expected
generic argument can itself be a collection type. After substitution, `@T`
checks array elements against `T`, and `%K:V` checks map keys and values against
`K` and `V`. Nested literals and generic constructions receive those same
constraints. For example, `Box<@int>` supplies `T=@int` to a `value is T`
constructor parameter, so both `Box(value: [1, 2, 3])` and `Box(value: [])`
are valid in that context. A known incompatible element is a semantic error;
value-dependent elements retain runtime validation. This does not infer a
type for `Box(value: [])` without an expected type or other type evidence.

```okkama
object Store {
    has states is Channel<int>;
    Store() {
        states = Channel(capacity: 1); // expected Channel<int> supplies T=int
    }
}
```

A unique implemented role template may also supply the expected specialization,
as with a typed local. Distinct solutions from role templates are an error.
Async returns follow the same rule, including constructors whose arguments
contain `await`; suspension does not discard their expected type.

The analyzer records declared construction expectations, resolved generic
specializations and contextual collection types, and diagnoses conflicts
when argument evidence is statically known. The runtime consumes that metadata,
substitutes the enclosing object's specialization when necessary,
and still validates actual values. Value-dependent inference and previously
supported contextual collection operations retain runtime checks. Untagged
empty collections provide no conflicting evidence, even when the whole
collection occupies a single `T` slot (for example `Box<@int>`); retained type
tags still count as evidence and cannot be overridden.

Expected types never weaken actual evidence. If actual values contradict the
target, construction fails before the constructor body executes. An explicit
role equation is compatible with a concrete object equation only when that
object implements the exact role: `List<Json>` therefore remains specialized
as `List<Json>` when a `JsonBool` is prepended, rather than being narrowed to
`List<JsonBool>`. This is role assignability, not common-supertype inference;
two unrelated concrete equations still conflict. For calls that pass analysis,
runtime argument expressions are evaluated from left to right exactly once
before their value-dependent equations are solved.

Each runtime object retains its specialization. `Box<int>` and `Box<float>`
are distinct types across fields, action parameters/results, variadic action
tails, successors, lifecycle bodies and typed variable boundaries. Imports,
`take`, namespaces and dependency closure preserve declaration metadata and
specialization.

An object implements an exact applied role contract:

```okkama
object EchoBox<T> does Echo<T> {
    EchoBox();
    echo(value is T) is T { value; }
}
```

Conformance substitutes the role arguments before comparing parameter,
variadic-tail, return and successor signatures. Consequently `EchoBox<int>`
is assignable to `Echo<int>` but not `Echo<string>`. Assignability is preserved
inside typed arrays/maps, catch clauses, fields, constructor/action boundaries,
successors and imports. Generic constructor inference may collect equations
through an implemented role such as `Echo<T>`.

Generic fakes MUST conform symbolically to the role for every specialization;
a fake written with `int` cannot satisfy a signature written with `T` merely
because its first use happens to request `T = int`. A generic fake has no
argument from which to infer its parameters, so a declared target supplies the
specialization:

```okkama
let echo is Echo<int> = Echo.fake(); // test mode only
```

Without that target, fake construction fails rather than choosing `any` or a
default type. Independent target types create independent fake
specializations while sharing immutable declaration metadata.

The current source language infers construction and does not provide an
explicit `Box<int>(...)` callee form. Generic variadic full constructors are
runtime-supported and follow the same exact-type inference rules across the
fixed prefix and the variadic tail; see section 11.3 and
`docs/todo/variadic-constructors-14072026.md`.

### 6.5.1 Action Types and Action Values

An action signature is a public type expression:

```okkama
let double is (int) is int = (value is int) is int { value * 2; };
let answer is () is int = () is int { 42; };
let endpoint is async (Request) is Response = controller.response;
```

The parenthesized types are ordered parameter types; `is` introduces the
result type or the result-only marker `void`. An optional leading `async` is part of the exact signature and requires
`await` at every call. Empty, multiple, nested, collection, generic, object and
role types are valid:

```okkama
let compose is ((int) is int, (int) is int) is (int) is int =
    (first is (int) is int, second is (int) is int) is (int) is int {
        (value is int) is int { second(first(value)); };
    };
```

Anonymous actions declare parameter types and an explicit result in their
expression syntax. A typed `let`, object/role action parameter, or action result
checks the declared signature. An action value cannot be silently retyped. Compatibility is exact;
there is no parameter/result variance or implicit numeric widening.

Action types participate in generic substitution, role conformance and fixed
action overload selection. Anonymous actions have a declared signature before
overload selection. Action values cannot be placed in `any`,
cannot cross native declarations, and are not automatically converted to an
arbitrary user role.

Named public object actions are first-class bound values when member access has
no call parentheses:

```okkama
let controller = GreeterController(service);
let endpoint is async (Request) is Response = controller.response;
let response = await endpoint(request);
```

`controller.response` retains `controller` as its receiver; invocation restores
that receiver as `self`, including its dependency graph and generic bindings.
The exact action-type boundary selects an overload and validates parameter,
result, and async shape. Private actions cannot be exported as values. A bound
action keeps its receiver reachable for ownership and destruction purposes.

### 6.6 Builtin Value Behavior

Builtin arrays, maps, strings and exact numeric values are receivers with
portable immutable behavior. This is language/runtime semantics, not host
method lookup and not a native provider convention. Dispatch is by runtime
category (`array`, `map`, `string`, `int`, `float`, `rat`, `byte`); ordinary objects
are never affected by this and always use ordinary object action dispatch,
so an object action named e.g. `size` or `less` is never shadowed by or
confused with these builtins.

Runtime-supported today:

- `@T.size() is int` and `@T.slice(start, end) is @T`; an exact `@string`
  receiver additionally exposes `join(separator is string) is string`;
- `string.length() is int` (counts Unicode code points, not UTF-8 bytes),
  `string.slice(start, end) is string` (same code-point counting),
  `string.trim() is string`, `string.chomp() is string`,
  `string.split(separator is string) is @string`,
  `string.replace(target is string, replacement is string) is string`, and
  `string.replace_first(target is string, replacement is string) is string`;
- `%K:V.size() is int`, `%K:V.has(key) is bool`, `%K:V.keys() is @K`, and
  `%K:V.values() is @V` (`keys()`/`values()` are sorted by key and share the
  same pairing order);
- exact same-type numeric `.less`, `.less_or_equal`, `.more`, and
  `.more_or_equal` on `int`/`float`/`rat`/`byte`.
- explicit zero-argument casts: `int.rat()`, `byte.rat()`, `float.rat()`,
  `int.float()`, `float.int()`, `int.byte()`, `float.byte()`, `byte.int()`, `byte.float()`,
  `rat.int()`, `rat.byte()`, `rat.float()`, numeric
  `.string()`, and `bool.string()`. `rat.numerator()` and
  `rat.denominator()` return exact decimal strings.

`float.rat()` rationalizes the float's canonical decimal rendering; it does
not expose the implementation's binary floating-point storage fraction.

`float.int()` truncates toward zero. `int.byte()` requires `0..255`;
`float.byte()` truncates toward zero before applying that range check. Numeric
string casts use canonical numeric text, so an integral float has no forced
`.0`; boolean strings are exactly `"1"` and `"0"`. Cast results carry the exact
target runtime category and do not relax typed boundaries.

### 6.6.1 Primitive Cast Calls and Array Casts

`T(value)` MUST accept exactly one positional argument and return one value
of `T`. `@T(values...)` (where the dots here denote a grammar repetition)
MUST accept zero or more positional arguments and return an immutable `@T`.
The supported target names are exactly `int`, `float`, `byte`, `string`.
An empty array MUST retain its target element type without contextual typing.
That retained type MUST supply generic evidence at an `@T` parameter boundary.

| Target | Accepted sources | Conversion |
|---|---|---|
| `int` | `int`, `byte`, `float`, `rat` | Truncate toward zero and check signed-64 range |
| `float` | `int`, `byte`, `float`, `rat` | Binary64 rounding; reject non-finite results |
| `byte` | `int`, `byte`, `float`, `rat` | Truncate toward zero and check `0..255` |
| `string` | `string`, `int`, `byte`, `float`, `rat`, `bool` | Numeric text, canonical rational text, or `"1"`/`"0"` for bool |

```okkama
let label = string(101);
let whole = int(-3.9);               // -3
let packet = @byte(122, 11, 22);     // @byte
let empty = @byte();                // @byte
let singleton = @byte(112);          // @byte, one element
let nums is @int = [1, 2, 3, 4, 5];
let converted = @byte(nums...);
let framed = @byte(0, nums..., 255);
```

Named arguments MUST be rejected before evaluating operands. Scalar casts
MUST reject unpack. Array casts MUST accept unpack of array-valued expressions or variadic bindings in
any positional slot, including multiple unpacks. Each expands exactly one
level; an empty source contributes zero arguments. Otherwise operands MUST
evaluate exactly once, left-to-right,
before conversions are applied in the same order. The first failed conversion
MUST terminate the expression with a runtime diagnostic identifying its
one-based argument number, source and target types, and reason. No partial
array is returned. Operand effects already performed are not rolled back.
Cast failures use the ordinary runtime type-error channel, not a catchable
nominal `Error` value. Enclosing calls do not run when their cast operand fails.

`int` casts check `-9223372036854775808..9223372036854775807` without
rounding those bounds. `byte(255.9)` is 255; `byte(-0.5)` is 0. Int-to-float
rounding may lose integer precision and MUST NOT imply lossless conversion.
Numeric-looking strings MUST NOT be parsed; strings MUST NOT be encoded as
bytes; arrays MUST NOT expand without an explicit `name...`, and ranges
MUST NOT expand. Error argument numbers refer to the flattened list. `any`
MUST NOT be unboxed and object methods MUST NOT be invoked by primitive casts.

Existing primitive cast methods use the same conversion rules. Bare primitive
cast calls take intrinsic precedence over same-named local bindings, and native
declarations MUST NOT reuse these four names. Member calls remain ordinary
action dispatch. This feature adds no first-class type or function value,
user-object cast, destructuring declaration, implicit conversion at a typed
boundary, or change to the two-operand `rat(numerator, denominator)` constructor.

### 6.6.2 Literal String Operations

String splitting and replacement are literal, left-to-right, and
non-overlapping; their arguments are never regular expressions. `split`
preserves leading, adjacent, and trailing empty fields. An empty separator
splits a non-empty string into Unicode code points and splits the empty string
into an empty, typed `@string`; with a non-empty separator, splitting the
empty string produces `[""]`. `join` accepts only exact string elements and
returns `""` for an empty `@string`.

For replacement, an empty target denotes Unicode code-point boundaries:
`"ab".replace("", "-")` is `"-a-b-"`, while
`"ab".replace_first("", "-")` is `"-ab"`. Removal needs no separate
action: it is `replace(target, "")`. All these actions return new values and
never mutate their receiver.

`slice` is end-exclusive; a negative index, `start > end`, or `end` beyond
length are all hard errors, never clamped. Cross-numeric calls such as
`1.less(2.0)` fail; they never widen either operand — `int`, `float`, and
`byte` must match exactly. Every builtin member action is positional-only (named
arguments are rejected) and exact-arity. Array cast expressions have the
separate zero-or-more-operand contract described above. Calling an
unrecognized action on a builtin-supporting receiver reports `Type
"$category" has no builtin action "$name"`; a receiver category with no
builtin support at all (`range`, `enum`, `any`, `undefined`) still
reports the older `Can only call actions on objects`.

Nonempty homogeneous array and map literals establish exact runtime identity
when evaluated, after range materialization. All elements must have the same
exact type; maps additionally require one exact key type, recorded before host
key normalization. Only surviving entries contribute evidence for duplicate
keys of the same type. Nested identities are recursive. Empty or heterogeneous
literals remain untagged without an explicit target; no numeric widening,
common-role search, union or automatic `any` boxing occurs. Existing `any`
boxes contribute the type `any`, never their hidden payload. Inferred identities
are available to direct arguments, generics, typed overloads and native ABI.
The runtime dispatch category remains `array`/`map`, independently of exact
identity. Native adapters read established identity and never infer it by
walking an untagged value.

Arrays and maps are backed by an immutable typed runtime representation
(`ArrayValue<T>`/`MapValue<K,V>`, `lib/Okkama/Runtime/Value.pm`) so an
**empty** slice/keys/values result still retains its true element/key/value
type: crossing a declared `@T`/`%K:V` boundary tags the value, and an empty
tagged collection is checked against its tag rather than vacuously accepted
(a non-empty collection is always validated by walking its elements, exactly
as before). An untagged empty literal has no contradictory evidence and may
cross any compatible explicit boundary; that boundary then establishes its
tag, so a later incompatible empty boundary is rejected. See
`docs/todo/builtin-value-behavior-14072026.md` for the full design and
`t/interpreter/30-typed-collection-identity.t` for the test coverage.

### 6.7 Debug value formatting

The bundled `okkama` module exposes a compact deterministic debug formatter:

```okkama
import "okkama" take Dump;

let values is @int = [1..10];
Dump(values).string();
// [1, 2, 3, 4, 5, 6, 7, 8, 9]
```

`Dump(value).string()` is backed by the mandatory `runtime/debug.data_dump`
provider and supports numbers, `byte`, `bool`, `char`, `string`, enums, typed
arrays and typed maps, recursively including typed values boxed inside `any`.
Strings and chars use escaped source-like forms; maps use their deterministic
ordered-pair ABI. The result is intended for diagnostics and debugging, not
serialization or a parseable persistence format.

`Dump` never reflects Okkama object fields and never exposes native handles or
interpreter-only wrappers. It does not add a universal `.dump()` action to
language values. An untyped collection nested beneath `any` is rejected rather
than having its element/key/value identity guessed at the native boundary.

For looking inside a value while debugging, `debug/dumper` (after Perl's
`Data::Printer`) does reflect objects, private fields included:

```okkama
import "debug/dumper" take Dumper;

let dump = Dumper(order);
dump.type();               // "Order"
dump.size();               // fields of an object, elements of a collection
dump.all();                // multi-line dump of everything inside
dump.max_depth(1).all();   // nested values as `{...}` / `[...]`
```

Types are runtime types: object and enum names without module prefixes
(`Box<int>`, `Stdout`), `@T` and `%K:V` from the elements, `?` for an empty
collection and `(A | B)` for elements of different types. Actions, futures and
native objects are shown, not looked into. The dump is a diagnostic aid, not a
stable or parseable format, and it never changes the value. It is built in
Okkama over the `runtime/debug` primitives `data_kind`, `data_type`,
`data_size`, `data_leaf`, `data_names`, `data_keys` and `data_items`, which
describe one value each; every engine produces the same text.

### 6.8 Functional Collection Expressions

This syntax and all three eager operations are runtime-supported on arrays,
ranges, typed maps and objects implementing the standard nominal
`collections/iteration.Iterable<T>` role.

The first milestone contains exactly three eager immutable operations:

```okkama
let doubled = values.map() -> value {
    value * 2;
};

let selected = values.filter() -> value {
    value.more(0);
};

let total = values.reduce(0) -> accumulator, value {
    accumulator + value;
};
```

Arrays, ranges and `Iterable<T>` objects bind one element. Typed maps bind
`key, value`; map reduce binds `accumulator, key, value`. Array/range/Iterable
`map` and map `map` return a typed array. Array/range/Iterable `filter` returns
a typed array, while map `filter` preserves the typed map. Range and Iterable
results are eager materialized arrays; Iterable filter retains exact `T` even
when empty.

`map` produces one exact output element type. A direct typed `@U` target can
provide that type for an empty source; otherwise an empty source cannot infer
it. Without a target, the first result establishes `U` and every later result
is checked deeply against it. `@any` is selected only by an explicit target.
An action's declared `@U` result supplies the target to its own tail or explicit
return map, including nested branches, typed anonymous actions and await paths.
Declaration evidence is resolved with invocation-local generic bindings and
survives async rewriting; unrelated caller expectations do not replace it.
`filter` predicates return exact `bool`. `reduce` always has an initial value
and maintains one exact accumulator type supplied by a direct declared target
or the initial value. Receiver and initial are each evaluated once; reducing
an empty source returns the checked initial value unchanged without executing
the block.

For an `Iterable<T>`, the runtime obtains one cursor through ordinary action
dispatch and visits each item in `available`, `current`, body, `advanced`
order. A body or result failure stops before `advanced()` for that item. The
concrete standard role specialization supplies exact `T`; a structurally
matching local role does not enable functional operations.

The operation block is a dedicated lexical child expression, not a
first-class function, lambda or callable object. It captures its surrounding
lexical scope, receives fresh iteration bindings, and cannot use `next` or
`last`. Receiver and reduce initial expressions are evaluated once, then items
are visited deterministically in source order; maps use their established
sorted-key order. Errors stop at the first failing item and propagate through
ordinary typed throw handling.

Aliases `grep` and `fold`, lazy streams, file-line iterators, parallelism,
map-entry transformation and Okkama-visible mutable accumulators remain out of
scope. The custom iterable extension is specified in
`docs/todo/custom-iterable-functional-operations-20072026.md`.

### 6.9 Portable Regular Expressions

Regular expressions have no literal syntax. The public immutable API is an
ordinary stdlib object:

```okkama
import "regex" take Regex, Match, InvalidRegex, MissingMatch;

let identifier = Regex("^[A-Za-z_]\\w*$");
identifier.contains("name = value"); // search anywhere
identifier.matches("name");         // whole input only

let word is Match = Regex("([A-Za-z]+)").first("Hi, Ada");
word.string();                       // "Hi"
word.groups();                       // ["Hi"]
```

`Regex(pattern)` is state-only and retains the Unicode source string without a
native call. Each public action creates one private prepared value whose
`RegexHandle` owns the provider-side compiled program for that operation and
closes it deterministically before returning. `Match` values retain only the
input string and typed spans, never the handle. Handles are not inspectable or
constructible by source code.

The 1.0 dialect contains literal code points; escaped metacharacters and
`\\`, `\\n`, `\\r`, `\\t`, `\\x{HEX}`; dot except LF; absolute `^`/`$`;
alternation; capturing and `(?:...)` grouping; positive/negative character
classes with ascending ranges; ASCII `\\d`/`\\w`/`\\s` and their uppercase
negations; and greedy `?`, `*`, `+`, `{n}`, `{n,}`, `{n,m}`.

Lookaround, backreferences, named groups, Unicode properties, flags,
lazy/possessive quantifiers and host/executable extensions are invalid. The
default implementation compiles to a Thompson NFA and does not invoke the host
regex engine. Limits are 4,096 pattern code points, 16,384 states, 1,048,576
input code points and 4,096 for each explicit repetition number. Invalid
patterns cross the runtime boundary as sanitized `invalid_input`; the
high-level action converts them to `InvalidRegex` with an exact zero-based
Unicode source offset. Pattern/input limits use `resource_exhausted` inside the
runtime and become deterministic high-level invariant failures without host
text.

`runtime/regex.spans(handle, input)` returns an empty array for no match or the
leftmost-longest full match followed by capture groups in source order. Each
`%string:int` element contains code-point `start`/`end`; unmatched groups use
`-1` for both. `all_spans` returns non-overlapping matches in source order,
preserving absolute anchor semantics. Positive matches resume at their end;
zero-length matches resume one code point later and include the terminal
boundary once.

`Regex.first()` returns `Match` or throws `MissingMatch`; `all()` returns every
detached match. `Match.groups()` excludes the full match and maps an unmatched
capture to `""`. `replaced()` performs literal global replacement: `$1`,
backslashes and braces are ordinary replacement characters. Split,
capture-expanding/callback replacement, flags, regex literals and raw-byte
matching are deferred. Tagged NFA instructions retain bounded state-set
execution.

## 7. Values and Truth

### 7.1 Bool

Boolean literals:

```okkama
true
false
```

Boolean values are represented distinctly at runtime.

Conditions require runtime `bool` values. Okkama has no general truthiness:
numbers (including `byte`), strings, arrays, maps, ranges, and objects are not
implicitly converted to `bool` in conditions or boolean operators.

### 7.2 Undefined

There is no source-level `none`, `null`, or `nil`.

Some runtime operations may internally produce an undefined result, for example:

```okkama
if false {
    1;
}
```

User code MUST NOT spell a null-like value.

## 8. Expressions

### 8.1 Literals

```okkama
1;
1.5;
"hello";
'x';
true;
false;
```

### 8.2 Variables

```okkama
let amount = 10;
amount + 5;
```

Reading an undefined variable is a runtime error:

```text
Undefined variable "amount"
```

### 8.3 Arithmetic

Supported arithmetic operators:

```text
+ - * / %
```

Examples:

```okkama
1 + 2;
10 - 3;
4 * 5;
8 / 2;
9 % 4;
```

Arithmetic rules:

- `number + number` performs numeric addition;
- `string + string` performs string concatenation;
- mixed string/number operands for `+` are runtime errors;
- `-`, `*`, `/`, and `%` require numeric operands;
- string or scalar repetition with `*` is not part of the language.

`rat(numerator, denominator)` creates an immutable arbitrary-precision
rational from two `int`/`byte` operands. It normalizes by greatest common
divisor, keeps the denominator positive, and rejects a zero denominator.

`byte` is a numeric operand. Operations may mix it with `byte`, `int`, `rat`,
or `float`, but arithmetic never produces `byte`. For `+`, `-`, `*`, and `%`,
results are `rat` when a `rat` participates, otherwise `int` unless a `float`
operand requires `float`; unary minus preserves `rat` identity. `/` produces
`float` when a `float` participates and exact `rat` otherwise. `rat` and
`float` MUST NOT mix without an explicit `.float()` conversion. Numeric
category is part of the runtime value and is never inferred from its
fractional part.
Overflow and underflow therefore remain ordinary promoted numbers and never
wrap or clamp into `0..255`. Division and modulo by `byte(0)` use the ordinary
zero-divisor errors.

### 8.4 Comparison and Equality

Supported comparison operators:

```text
== != < <= > >=
```

Examples:

```okkama
1 == 1;
2 != 3;
3 > 1;
```

Numeric comparisons require numeric operands.

Current scalar equality rules:

- `bool` compares by value only with `bool`;
- `string` compares by value only with `string`;
- `int` compares numerically with `int`;
- `byte` compares numerically only with `byte`;
- `float` compares numerically with `float`;
- `rat` compares exactly with `rat`;
- `char` compares by value only with `char`;
- mixed scalar categories are runtime errors.

Mixed numeric-category equality is a runtime error. Numeric ordering operators
are broader than equality and accept `rat` with integer categories exactly;
`rat`/`float` ordering requires an explicit conversion.

Composite equality rules:

- arrays compare deeply by length and element order;
- maps compare deeply by key set and value equality;
- ranges compare by `start`, `end`, and `inclusive`;
- ordinary objects compare by concrete runtime type and all fields, including private
  fields;
- runtime-owned effect capabilities (Atomic, Channel, Lock, Event, Semaphore,
  Barrier, cancellation and future coordination capabilities) compare by their
  intrinsic state identity, not their private field shape or changing payload;
- the detached immutable BarrierState compares its parties, waiters, generation
  and broken coordinates by value;
- Futures and action values compare by execution/capture handle identity.
  An alias equals itself before and after execution; separately created bound
  references or closures are distinct. Comparison does not execute work;
- these rules apply recursively inside ordinary objects and collections;
- mixed runtime categories are runtime errors.

### 8.4.1 Stream write

The left-associative `<-` expression writes one byte source to one byte sink:

```okkama
sink <- source;
sink <- first <- second <- third;
```

Its standard protocol is declared by `io/stream`:

```okkama
role ByteSource {
    bytes() is @byte;
}

role Writing {
    write(source is ByteSource) is void;
    flush() is void;
}
```

An object is eligible on the left only when it formally performs `Writing`.
An object is eligible on the right only when it formally performs
`ByteSource`. Merely declaring an action named `write` or `bytes` does not
satisfy either condition. The built-in `string` and `@byte` types are the two
explicit source exceptions: a string is adapted to a `ByteSource` using strict,
locale-independent UTF-8, while `@byte` is adapted without changing, encoding,
or stringifying any octet. No other built-in value is converted implicitly.
In particular, numbers, booleans, enums, collections other than `@byte`, and
arbitrary objects require an explicit conversion or a formal `ByteSource`
implementation.

An explicit unpack source, `sink <- array_expression...`, MUST evaluate the
array expression once and send its elements through the ordinary write protocol
in order. Each element MUST satisfy the same string/`@byte`/`ByteSource` rules;
no numeric conversion or recursive flattening is introduced. Empty unpack MUST
validate and return the sink without calling `write`. Write failure stops the
remaining elements and chain sources; completed output is not rolled back.
Array-expression effects have already happened before element writes begin.
Postfix unpack retains its existing precedence:

```okkama
out <- @string(1, 2, 3)... <- endl; // writes 123 followed by a newline
```

This groups as `out <- (@string(1, 2, 3)...) <- endl`. Awaited sources require
parentheses: `out <- (await source.words())...;`. The interpreter also retains
legacy `ByteSink` behavior by feeding each returned sink into the next write.

After protocol validation, `sink <- source` uses the ordinary protocol action
call `sink.write(source)`. It does not search for another action, call
`string()`, append a line ending, flush, close, retry, or translate a thrown
error. The command consumes the complete source or throws `IncompleteWrite`.
`<-` discards any internal last-statement value from the command body and
returns the original `Writing` capability; `write` is not a builder.

Chaining is ordinary left association:

```text
sink <- first <- second
== (sink <- first) <- second
```

The sink expression is evaluated once, then each source once from left to
right. Each write completes before the following source is evaluated. If a
write throws, later sources and writes are not evaluated. The value of the
complete expression is the original sink capability.

`<-` binds less tightly than calls, member access, arithmetic, ranges,
comparisons, and equality, and more tightly than `&&` and `||`:

```okkama
out <- "value: " + value.string() <- "\n";
```

is parsed as:

```text
(out <- ("value: " + value.string())) <- "\n"
```

Example with stdout:

```okkama
import "io/stdout" take Stdout;

main() is int {
    let out = Stdout();
    out <- "input value: ";
    let value = Stdin().line();
    out <- "your value is " <- value <- "\n";
    return 0;
}
```

Explicit conversion remains required:

```okkama
let value is int = 42;
out <- "value is " <- value.string() <- "\n";
```

User-defined `ByteSource` and `Writing` objects participate through the same
nominal roles. The first implementation does not add module-level immutable
values, so there is no standard importable `out` value; callers construct
`Stdout()` explicitly. Files, sockets, and every existing I/O adapter are not
automatically migrated in the first version.

### 8.5 Boolean Operators

Supported boolean operators:

```text
! && ||
```

Examples:

```okkama
!opened;
ready && valid;
failed || expired;
```

Boolean operators require runtime `bool` operands. They do not coerce values
from other runtime categories.

### 8.6 Ranges

Ranges are runtime-supported.

Exclusive range:

```okkama
1..5
```

Inclusive range:

```okkama
1..=5
```

Rules:

- Range bounds MUST be integer-like values: `int` or `byte`.
- Range start MUST be less than or equal to range end.
- Chained ranges are forbidden.

Byte bounds are normalized to ordinary integers. Expanding such a range
therefore produces `int` values, not implicit byte values.

Examples:

```okkama
[1..5];   // [1, 2, 3, 4]
[1..=5];  // [1, 2, 3, 4, 5]
```

### 8.7 Arrays

Array literal syntax:

```okkama
[1, 2, 3]
```

Trailing comma is allowed:

```okkama
[1, 2, 3,]
```

Ranges expand inside arrays:

```okkama
[0, 2..=4, 9]  // [0, 2, 3, 4, 9]
```

Array indexes accept `int` and `byte`. A byte index is interpreted by its
integer value; indexing does not change the stored element type.

### 8.8 Maps

Map literal syntax uses brackets, not braces.

```okkama
[:]
["name": "Alex", "age": 41]
```

The canonical spelling of the empty map is `[:]`; `[]` is always the empty
array. Whitespace is insignificant, so `[ : ]` is equivalent to `[:]`.

Map entries are:

```text
expr ":" expr
```

That means keys are expressions. There is no implicit string-key shorthand.

Correct string key:

```okkama
["name": "Alex"]
```

Variable key:

```okkama
let key = "name";
[key: "Alex"]
```

If `key` is not defined, this is a runtime error.

Wrong if the programmer wanted a string key:

```okkama
[name: "Alex"]
```

This looks up variable `name`. It does not mean `"name"`.

Runtime diagnostics format maps using the same bracket form and quote string
keys, for example `["name": "Alex"]`.

### 8.9 Indexing

Arrays and maps are indexed with brackets.

```okkama
[10, 20, 30][1];           // 20
["name": "Alex"]["name"];  // "Alex"
```

Fallback syntax:

```okkama
["name": "Alex"]["missing"] else "fallback";
```

Rules:

- Array indexes MUST be integers.
- Map keys MUST be scalar values.
- Missing array indexes or map keys are runtime errors unless `else` fallback
  is present.

### 8.10 Calls

Calls use parentheses.

```okkama
cash.total();
cash.add_bonus(10);
cash.add_bonus(bonus: 10);
```

Rules:

- Positional and named arguments MUST NOT be mixed in one call.
- Duplicate named arguments are errors.
- Wrong arity is an error.
- Unknown action names are errors.

An action value is called with the same postfix syntax:

```okkama
let double is (int) is int = (value is int) is int { value * 2; };
double(10); // 20
```

Action-value calls are positional. Named arguments are rejected, arity is
exact, and a non-callable value produces a controlled language error. Async
action values use the same syntax under `await`:

```okkama
let endpoint is async (Request) is Response = controller.response;
let response = await endpoint(request);
```

Calling an async bound action without `await`, or awaiting a synchronous one,
is an error.

### 8.10.1 Bound Object Action Values

For an object value `receiver`, a public action member without call parentheses
produces a bound action value:

```okkama
receiver.action          // value
receiver.action(args)    // immediate named-action call
```

The value captures the receiver, action declaration, module scope, concrete
generic bindings, and async shape. It becomes callable after crossing an exact
action-type boundary. Overloaded members must have exactly one match for that
boundary; ambiguous or incompatible references are rejected.

### 8.10.2 Anonymous Action Expressions

An anonymous action uses the same declaration syntax as a named action,
with the action name omitted. Parameter types and the result are explicit,
and the body block is mandatory:

```okkama
() is int { 42; }
(value is int) is int { value * 2; }
(left is int, right is int) is int { left + right; }
```

It can be stored, passed, returned, nested and invoked. Evaluating it creates
an immutable action value containing its parameters, body, source position and
the current lexical environment. Every call creates a fresh child environment,
binds positional parameters and executes the block. A value result validates
and returns the last value; `is void` evaluates and discards it under the
ordinary action-result rules. Capturing a void invocation is a semantic error.

Anonymous values use ordinary argument syntax:

```okkama
receiver.apply(operation: (value is int) is int { value * 2; });
```

Action values always have an explicit result declaration, including `is void`
for effect-only actions. No trailing comma is permitted in anonymous parameter
lists. Missing parameter types, results, or bodies are syntax errors.
The old anonymous arrow syntax, arrow action types, and trailing member-call
sugar are rejected. `values.map() -> value { ... }` remains the dedicated
functional collection expression from section 6.8, and `for source -> binding`
remains the loop binding syntax.

Variadic anonymous parameters, successor results, named action-value calls,
implicit currying, partial application and async anonymous actions are not
supported. Ordinary named member calls may receive anonymous values through
named arguments. Async bound action types permit `async (T) is R` and `async (T) is void`.

### 8.11 If Expressions

`if` can be an expression when both branches are present.

```okkama
let value = if ready {
    1;
} else {
    0;
};
```

If expression branches are blocks.

### 8.12 Match Expressions

Match expression syntax is aligned with match statement syntax.

Runtime support exists for basic match behavior, but exhaustiveness and branch
type compatibility are future semantic-analysis tasks.

### 8.13 Loop Expressions

`loop` starts an unbounded explicit loop and is an expression:

```okkama
let answer = loop {
    let text = input.line();
    if text.size() > 0 {
        last text;
    }
};
```

The body tail value is ignored. `next` begins a fresh iteration, `last value`
ends the nearest loop and supplies the expression result, and bare `last`
ends it without a public value. A no-value result may be discarded, but a
value-demanding boundary such as `let`, an argument, a typed return, an
operator, a condition, or a collection element MUST reject it.

Each iteration has a fresh lexical child scope. Loop control crosses
`try`/`catch` without being caught and performs ordinary `defer` and `destroy`
unwinding. Direct `next` or `last` in a functional collection block is invalid;
an explicit nested `loop` inside that block establishes its own valid boundary.

## 9. Statements

### 9.1 Expression Statement

```okkama
cash.total();
```

The value of the last executed statement in a block is the block result.

### 9.2 Let Statement

Untyped let:

```okkama
let x = 10;
```

Typed let:

```okkama
let x is int = 10;
```

Rules:

- A `let` MUST have an initializer.
- A typed `let` checks that the initializer is assignable to the declared type.
- A `let` binding is local to the current environment.
- Re-declaring the same name in the same scope is an error.
- Shadowing in nested scopes is allowed.

### 9.2a Local Rebinding

A bare `name = expr;` (with no `let`) rebinds an existing local variable
rather than declaring a new one, legal only for a concrete object type
declaring at least one actual `^`-marked successor action. This is the same
grammar production used for constructor field binding; see 9.12 for the
complete rule, eligibility condition, ownership/destroy interaction, and
examples.

### 9.3 Block Statement

A bare block is a statement and creates a nested environment.

```okkama
{
    let local = 1;
}
```

Bindings declared inside the block do not leak outside.

This matters for object lifetime:

```okkama
{
    let resource = Resource();
} // resource leaves scope; destroy runs if Resource declares destroy
```

### 9.4 Confirm Statement

```okkama
confirm condition;
confirm condition else throw ErrorObject();
confirm condition else panic "Invariant broken";
```

Rules:

- If the condition succeeds, execution continues.
- A bare failed confirm is an uncatchable programming error.
- `else` accepts exactly `throw expr` or `panic expr`; the operand is lazy.
- `else throw` raises an expected typed failure. `else panic` raises an
  uncatchable emergency failure.

### 9.5 Throw and Panic

```okkama
throw InvalidValue(10);
throw;                         // rethrow inside a selected catch body
panic "Unreachable state";    // exact string only
```

`throw value` accepts only an ordinary object that nominally `does Error`, or
the system `NativeFailure`. The system role is available without import and
requires a public synchronous `text() is string`. Throwing scalars,
collections, enums, `any`, handles, or structurally similar objects is a
runtime error.

`throw;` rethrows the currently handled thrown error without replacing its
identity or origin. It is valid only while executing a selected catch body. A
new valued throw inside a catch replaces the active failure.

`panic` accepts only an exact string and is never catchable. Bare `panic;` is
invalid. Panic, internal interpreter failures, invalid operations and other
programming errors still unwind cleanup, then terminate with a diagnostic.

### 9.6 Typed Try and Catch

```okkama
let message is string = try {
    operation();
    "accepted";
}
catch error is InvalidValue {
    error.text();
};
```

Rules:

- `try` is dual-purpose, like `if`: at block level it is a statement, while
  in an initializer, argument, grouping or other expression position it
  yields the value of the normally completed try body or selected catch body.
  Both forms require one or more typed catch clauses.
- Only failures raised by `throw` (including implicit native throws)
  participate in matching. Panic and programming errors pass through.
- Every catch type is validated before the try body executes and must itself
  guarantee `Error`: `Error`, a concrete/applied object explicitly doing
  `Error`, or `NativeFailure`.
- Clauses are examined in source order; the first compatible declared type
  wins. Arbitrary roles, scalars, enums, collections and `any` are invalid
  catch boundaries.
- A non-matching failure propagates unchanged.
- The try body and every catch body are separate lexical child scopes. The
  selected binding exists only in its catch body; try locals and sibling catch
  bindings are not visible there.
- Normal completion, loop-control signals, and `return` skip catch matching.
- Unrelated objects owned by the try scope are destroyed before catch selection;
  owned typed Error payloads move into the matching handler's scope. Catch
  cleanup runs after the selected body and before an expression result leaves
  the form. Cleanup is attempted once in reverse construction order.
- A body failure has precedence over a cleanup failure. If a normally completed
  scope fails during cleanup, that failure propagates through its own channel.
- `finally`, guards and catch-all clauses are not supported.

### 9.7 If Statement

```okkama
if ready {
    1;
} else {
    0;
}
```

`else if` is supported:

```okkama
if a {
    1;
} else if b {
    2;
} else {
    3;
}
```

### 9.8 Match Statement

```okkama
match status {
    case Status.ready {
        1;
    }
    else {
        0;
    }
}
```

Multiple case values are allowed:

```okkama
match x {
    case 1, 2 {
        "small";
    }
    else {
        "large";
    }
}
```

### 9.9 For Statement

```okkama
for users -> user {
    user.name();
}
```

Current runtime supports one binding.

Ranges can be used as loop sources:

```okkama
for 1..=3 -> n {
    n;
}
```

User-defined immutable collections participate through the nominal standard
roles from `collections/iteration`:

```okkama
role Cursor<T> {
    available() is bool;
    current() is T;
    advanced() is Cursor<T>;
}

role Iterable<T> {
    cursor() is Cursor<T>;
}
```

An object is iterable only when it explicitly implements the standard
`Iterable<T>` role from the `collections/iteration` v1 module line; matching
action names are not structural conformance.
`for` evaluates the source once, obtains one cursor, and calls `available`,
`current`, and `advanced` through ordinary action dispatch. Each normal
iteration and `next` advances once. `last` exits without advancing. A false
`available()` is normal exhaustion and does not call `current()` or
`advanced()`.

Cursor objects are immutable positions. `advanced()` returns a new cursor;
the collection and previous cursor remain unchanged. The protocol is for
repeatable synchronous collections, not consumable I/O streams. Array, range
and variadic-tail iteration retain their existing intrinsic behavior. The
same nominal `Iterable<T>` recognition also enables eager
`map`/`filter`/`reduce`; an identity map with a direct `@T` target is the
standard linear materialization operation.

Both the source and `->` binding are mandatory. Source-less `for { ... }` is
invalid; use `loop { ... }` for an unbounded loop.

### 9.10 Next and Last

```okkama
next;
last;
next if should_skip;
last if done;
last value;
last value if done;
```

`next` skips to the next iteration of the nearest explicit or collection
loop. `last` exits that loop. A valued `last` supplies its result; in a
collection `for`, bare `last` preserves the previously accumulated body
result while valued `last` replaces it.

For conditional forms, the condition is evaluated first and exactly once.
When it is false, a `last` payload is not evaluated. When it is true, the
payload is evaluated exactly once. Loop control outside a loop is an error.

An anonymous-action call is a callable control boundary: `next` or `last`
cannot target a loop outside that action. A loop declared inside the anonymous
action establishes its own normal target.

### 9.11 Defer

Syntax:

```okkama
defer cleanup();
```

Reaching `defer` registers its expression on the current lexical
scope without evaluating it. See section 19 for execution, unwinding and
failure-precedence rules.

### 9.12 Field Bind Statement and Local Rebinding

Bare assignment syntax:

```okkama
name = expr;
```

This single grammar production carries two distinct meanings, chosen by
context rather than by different syntax:

1. **Constructor field bind**, when currently constructing an object and
   `name` names one of that object's own fields:

   ```okkama
   Cash(value is int) {
       digit = value;
       cents = 0;
   }
   ```

   This is a dedicated constructor field-bind statement, not general property
   assignment. It is valid in a constructor body and its lexical child blocks.
   Both explicit and bare forms cross the same declared field type and
   construction-completeness boundary.

2. **Local variable rebinding**, in every other case — outside a constructor
   entirely, or inside one for a name that is not one of the object's own
   fields (a constructor parameter or a `let`-bound local used only for
   staging):

   ```okkama
   let counter = Counter(value: 0);
   counter = counter.increment();
   ```

   A rebind is legal only when both hold:

   - `name` already denotes an existing local in an enclosing scope.
     Otherwise: `Undefined variable "name"`, the same error a read would
     produce.
   - That local's declared type is a **concrete object declaring at least
     one actual successor action** (an action declared `is ^Type`, 9.13/
     11.x) whose return type, after substituting the type's own generic
     parameters, is exactly that type or a role it implements. Otherwise:
     `Cannot rebind variable "name": type "Type" has no successor actions`.

   This is a deliberately hard, narrow rule: a *plain* (non-`^`) action that
   happens to return the same type does **not** qualify, and a role-typed
   declared local is never eligible either, even if some concrete
   implementer of that role has a successor action. For example, a
   persistent-structure method such as `List<T>.prepended(...)` returning
   plain `List<T>` — because `EmptyList<T>.prepended` must return a
   *different* concrete object (`LinkedList<T>`), which the `^` mechanism
   cannot express, since it can only stage a copy of the *same* receiver's
   own fields — does **not** make `List<T>` rebind-eligible. Eligibility is
   an explicit per-action opt-in by the type's author, not inferred from
   return-type shape; see `docs/todo/local-rebinding-22072026.md` for why an
   inferred rule was tried and rejected. A plain scalar, array, map, or enum
   type never qualifies either way, since nothing declares actions on those:
   `let n = 1; n = 2;` remains illegal.

   The new value must satisfy the exact same declared type as the existing
   binding, using the same coercion/matching rules as `let` (9.2). Otherwise:
   `Cannot rebind variable "name" of type Type with Actual`.

   A rebind always writes into the scope that actually owns the binding, not
   a new shadow in the rebinding statement's own scope — critical for a loop
   accumulating into a name declared before it, since each iteration runs in
   its own fresh child scope (10.x):

   ```okkama
   object Counter {
       has value is int;
       Counter(:value);
       increment() is ^Counter { ^value = value + 1; }
   }

   let counter = Counter(value: 0);
   for 1..=3 -> n {
       counter = counter.increment();
   }
   ```

   The previous value is not eagerly destroyed at the rebind point. If it was
   scope-owned and its type declares a `destroy` block, that block still runs
   at the eventual natural scope exit exactly as for any other owned value.

See `docs/todo/local-rebinding-22072026.md` for the full design rationale,
including the reverted broader draft.

The explicit receiver form is preferred when a field name could be confused
with a parameter or local:

```okkama
self.field = expr;
```

This remains a dedicated constructor field-bind statement, unaffected by
local rebinding. `other.field = expr` is forbidden, and actions, and
`destroy` cannot write through `self`.

### 9.13 Successor Assignment

Successor assignment syntax:

```okkama
^field = expr;
```

This is valid only inside successor actions, meaning actions declared with
`is ^Type`.

It is forbidden in:

- ordinary actions;
- constructors;
- `destroy`;
- module-level code.

## 10. Blocks, Environments, and Lifetime

Every block creates an environment.

Examples:

```okkama
main() is int {
    let x = 1;

    {
        let x = 2;
        x; // 2
    }

    x; // 1
}
```

Okkama uses lexical scope. A declaration body does not close over the runtime
environment of its caller.

The scope boundaries are:

- `main` executes in its own root environment;
- receiver, action arguments, constructor arguments, and explicit construction
  field expressions are evaluated in caller scope, left-to-right;
- an action body receives only `self`, receiver fields, declared parameters,
  and locals introduced by that body or its lexical children;
- an anonymous action captures the stable lexical bindings visible when its
  expression is evaluated, including an enclosing action's parameters and
  `self`; calling it later does not expose the caller's locals;
- a constructor body receives `self`, its declared parameters, construction
  field bindings, and its own lexical locals;
- `destroy` receives only `self`, receiver fields, and their own
  lexical locals;
- field defaults execute in declaration scope, not construction caller scope;
  they may use literals and globally resolved declarations, but do not
  implicitly see caller locals or other fields of the instance being built;
- block expressions, block statements, conditional branches, and loop
  iterations are lexical children of the body that contains them;
- imported declarations do not capture locals from the importing module.

Consequently, a callee cannot resolve a binding merely because it exists in a
caller frame:

```okkama
object Reader {
    Reader() {}

    read() is int {
        leaked; // Undefined variable "leaked"
    }
}

main() is int {
    let leaked = 42;
    Reader().read();
}
```

Values cross a declaration boundary only through explicit arguments,
construction fields, receiver fields, or future documented module-level
bindings. Environment restoration is required on both normal completion and
control/error unwind.

Anonymous actions are the deliberate exception to declaration bodies not
closing over caller frames: their value explicitly retains the lexical
environment in which the expression was created. The captured bindings are
stable values; this does not introduce mutable cells or hidden object state.

Objects bound with `let` are owned by the current environment. When the
environment exits, objects declared in that environment run their `destroy`
block if their type declares one.

Destroy order is last-in, first-out within the environment.

Ownership follows runtime object identity rather than binding names:

- introducing another binding for an object already reachable from the current
  lexical environment creates a borrowed alias, not a second owner;
- `self`, receiver fields and parameters are borrowed at action and lifecycle
  boundaries;
- arrays, map values, object fields and `any` transport preserve the identities
  they contain; repeated references do not duplicate ownership;
- fresh object identities first introduced inside a newly bound aggregate are
  owned by that binding's environment;
- a lazy ordinary async invocation exposes its receiver and evaluated arguments
  to the ownership graph. Returning or binding its Future retains fresh owned
  dependencies before startup, including when it is never started or cancelled
  before startup. Existing outer-scope dependencies remain borrowed;
- within a newly acquired aggregate, fresh identities register in creation
  order, independently of map keys or capture-binding enumeration. Cleanup runs
  in reverse registration order, with parent cleanup preceding owned children;
- owned typed Error payloads survive unwind and move into a matching catch
  scope. Rethrow, automatic causes and secondary carriers preserve the same
  identities. Unrelated try locals are cleaned before catch; uncaught payloads
  are drained before native registry shutdown. Borrowed outer resources do not
  acquire a second owner;
- every owned identity enters destruction at most once during its lifetime,
  including when its destroy block fails;
- successor values have fresh identities and independent lifetimes.

Example:

```okkama
object File {
    has path is string;

    File(path is string) {
        path = path;
    }

    destroy {
        // cleanup side effects go here
        path;
    }
}

main() is int {
    {
        let file = File(path: "data.txt");
    } // file.destroy runs here
    return 0;
}
```

Important:

- Temporary objects not bound by `let` do not currently have specified
  destructor ownership semantics.
- Synthetic runtime bindings such as action `self`, field bindings inside action
  environments, and parameters are not treated as owners.

## 11. Objects

### 11.1 Object Declaration

```okkama
object Cash {
    has digit is int;
    has cents is int;

    Cash(digit is int, cents is int) {
        digit = digit;
        cents = cents;
    }
}
```

Object names are type names and MUST start with an uppercase letter.

Two objects with the same name in one module are forbidden.

### 11.2 Fields

Fields are declared with `has`.

```okkama
has digit is int;
has cents is int = 0;
has label is string = "USD";
```

Multiple fields may share one declaration:

```okkama
has digit, cents is int;
```

Rules:

- Field names MUST be unique inside an object.
- Duplicate names in separate `has` declarations are errors.
- Duplicate names inside one `has` declaration are errors.
- Fields are private through ordinary object references.
- Object actions can read fields directly by name or explicitly as
  `self.field`.
- Constructors, and `destroy` can read available receiver state as
  `self.field`.
- A parameter/local may shadow a bare field name without changing the meaning
  of `self.field`.

External field access through an ordinary object reference is forbidden:

```okkama
Cash(digit: 1).digit; // error
```

### 11.2.1 Object Capabilities (`cap`) 0.0.1

An object may declare a named contract of synchronous computed fields:

```okkama
cap BusinessCard {
    full_name is string { given_name + " " + family_name; }
}
```

Module scope also permits `cap Customer::BusinessCard { ... }` for an object
owned by the same module. Its private-field reads require `Customer::given_name`.
This names the selected instance's field, without exposing an owner receiver.
Internal and external declarations share one nominal namespace. External caps
inherit owner type parameters, permit forward owners and linked source files,
and resolve imports in their own defining file; imported types cannot be extended.



Inside `Customer`, this declares the nominal type `Customer::BusinessCard`.
Acquire it explicitly with `customer::BusinessCard`; read with `card::full_name`
or `customer::BusinessCard::full_name`. The owner expression runs once per
acquisition. Operand type resolves `::`; identifier case does not select the
operation. A full owner never converts implicitly. Caps cannot become owners,
sibling caps or roles, cannot be constructed directly, and cannot be acquired
by unwrapping a restricted role reference.

Bodies use ordinary Okkama blocks, local bindings, control flow, result checks
and error handling. Private fields of the immediate owner are readable by name.
A same-named computed field reads the owner field; locals may shadow it.
There is no `self`, including in nested closures, no implicit owner method
receiver or substitute owner binding, and no implicit sibling-field lookup.
Nested objects retain their privacy. Owner writes and successor writes are
forbidden. Each field has a synchronous context with no `await`; explicit
capabilities may still have effects. A field may explicitly return a Future.

Acquisition is lazy; every read recomputes its body, with no value/error cache.
Typed errors and panic retain their existing categories. A cap retains the
entire owner lifetime graph, including hidden resources, and remains bound to
its original immutable snapshot after a successor. Locals, parameters, returns,
object fields, collections, generics, closures and local async/Future transport
preserve the exact cap type. `any` remains opaque. Returned values keep their
own contracts, including restricted roles and explicitly broad object results.

Identity includes the defining owner, module/ABI major and owner type arguments
(`Box<int>::Public`). Existing imports and aliases apply. Version 0.0.1 permits
only unique computed fields: no methods, constructors, roles, `has`, lifecycle
blocks, nested caps or async fields. Ordinary dot access does not provide an
alternative API. Equality, ordering, hashing, automatic formatting,
serialization and owner reflection are unavailable; structural/dynamic paths
cannot unwrap the owner. Diagnostic formatting may safely name the cap type.
This is an interface boundary with no general secrecy, purity or revocation
promise. Other full references retain their capabilities.

Object views have been removed. Roles and `#projected` keep their
separate contracts. OIR Profile 0 remains int-only and explicitly refuses
objects and caps. See the [working example](../examples/object-caps.ok) and
[full guide](object-caps-0.0.1-ru.md).

### 11.3 Constructors

Every object MUST declare at least one constructor. Constructors are never
generated from `has` fields. An object without a constructor is a semantic
registration error even when it is never constructed.

Constructors are declared inside the object and have the same name as the
object.

```okkama
object Cash {
    has digit is int;
    has cents is int;

    Cash(value is int) {
        self.digit = value;
        self.cents = 0;
    }
}
```

A stateless or default-only object still declares a zero-argument constructor:

```okkama
object Marker {
    Marker() {}
}
```

An object may explicitly expose selected typed fields as named-only constructor
inputs with field shorthand:

```okkama
object Member {
    has name is string;
    has id is int;
    has internal is int = 40;

    Member(:id, :name);
}

let member = Member(name: "Alice", id: 7);
```

Each `:field` refers to a `has` field on the same object and inherits its full
declared type. The field MUST have an explicit type. The listed names form a
closed public construction allow-list; fields not listed cannot be supplied by
the caller. A listed field without a default is required, while a listed field
with a default is optional. Object field order, shorthand list order, and named
call argument order are independent.

The bodyless form automatically assigns supplied values through normal checked
field binding:

```okkama
Member(:id, :name);
```

Shorthand may also have an explicit body. Automatic shorthand bindings happen
first, then the body runs and may initialize unlisted required fields or
deliberately overwrite an already-bound field:

```okkama
Member(:id, :name) {
    self.internal = id + 1;
}
```

`Marker();` is the zero-field shorthand form. It constructs a stateless object
with a named-only empty signature and is distinct in the AST from
`Marker() {}`.

An object may expose one or more public, synchronous, zero-argument actions as
named constructor projections. `#projected` uses the action name as the
prefix; `#projected(alias)` selects an explicit prefix:

```okkama
object ParsedInt {
    has source is string;
    ParsedInt(:source);

    #projected(value)
    integer() is int {
        42; // abbreviated derivation from source
    }
}

value ParsedInt(answer)                    // ParsedInt(answer).integer()
value numbers.ParsedInt(answer)            // qualified target
```

The prefix applies only to a constructor whose target is an object name or a
namespace-qualified object name. It does not apply to variables, member calls,
parenthesized expressions, or arbitrary values. Full,
shorthand, empty, variadic, overloaded, and generic constructor selection is
unchanged. Construction and its `confirm` checks complete before the marked action is
called. Arguments evaluate exactly once in their ordinary left-to-right order;
a construction failure never calls the projection.

The projection action MUST declare no parameters, MUST have an explicit
non-successor return type, and MUST return a type different from the enclosing
object type (`Object<T>` counts as the object type of a generic declaration).
An object may declare multiple projected actions, but every effective prefix
(the alias, or the action name when no alias is present) MUST be unique within
that object. These declaration rules and use of an undeclared prefix are
semantic errors before execution.

The directive marks one action declaration and thereby chooses the action
invoked by its prefix. Prefixes are resolved from the statically resolved
constructor target during semantic analysis; they are not variables, imported
bindings, dynamic operators, or role contracts. Unmarked fixed overloads of
the same action name remain callable normally; all overloads still obey the
ordinary common-return-type rule. Projection dispatch uses the ordinary
zero-argument action pipeline, including duplicate-signature ambiguity, typed
return checks, failures, and ownership.

Rules:

- Constructor calls use `Object(...)`.
- Named constructor projections use `prefix Object(...)` and are exactly
  equivalent to the corresponding construction followed by the action
  registered for that prefix.
- `#projected` registers the action name; `#projected(alias)` registers the
  lowercase alias. Effective prefixes are unique per object.
- Single `&` projection syntax is removed; `&&` remains boolean conjunction.
- Constructor name MUST match the enclosing object.
- Full constructor parameters are typed.
- Constructor parameter types are checked.
- Full constructors support named and positional arguments but MUST NOT mix
  them.
- Positional arguments bind to constructor parameters, never field order.
- Named arguments bind to constructor parameter names.
- Field shorthand calls accept named arguments only and reject positional
  arguments before evaluating them.
- Shorthand fields are validated after the complete object body is collected,
  so a referenced `has` declaration may appear before or after the constructor.
- Duplicate, unknown, or untyped shorthand fields are declaration errors.
- A bodyless shorthand is a declaration error when an unlisted field has no
  default, because it cannot produce complete object state.
- Constructor body uses bare or explicit `self.field` field bind statements.
- All fields MUST be initialized by the end of construction.
- Field defaults are evaluated before the constructor body and type-checked.
- Full constructors are selected like action overloads (11.8): by arity,
  by parameter names in a named call, and by parameter types with the same
  specificity rules. Shorthand constructors are selected by named-field
  compatibility: every supplied name must be allowed, every required name
  must be present and every supplied value must fit its field type.
- A call that fits a fixed full or a shorthand constructor never selects a
  variadic constructor.
- Multiple equally specific compatible signatures are ambiguous; declaration
  order never resolves the call. Two full constructors with the same
  parameter types are a declaration error unless both are fixed and their
  parameter names differ.
- A generic object infers its specialization from each constructor the call
  fits (fixed and shorthand first, then variadic); when they infer different
  specializations, only those under which a constructor accepts the
  arguments remain, and more than one is ambiguous.
- If no constructor is compatible, construction fails. There is no fallback
  that derives a signature from all `has` fields.

A full constructor may declare one variadic parameter as its unique final
parameter, exactly like an action's variadic tail (`docs/todo/variadic-constructors-14072026.md`):

```okkama
object Min<T> {
    has first is T;
    has rest is @T;

    Min(first is T, rest is ...T) {
        first = first;
        let materialized is @T = rest...;
        rest = materialized;
    }
}
```

Rules:

- a variadic constructor is a full constructor; field shorthand remains
  named-only and never declares a variadic tail;
- the fixed prefix may be empty, one, or several parameters; the variadic
  parameter is always last;
- calls are positional only; a named call against a
  variadic constructor is rejected before any argument is evaluated;
- the fixed prefix defines the constructor's minimum arity; a call with too
  few arguments reports the shortfall before the constructor body runs;
- a variadic constructor overloads with other constructors, including other
  variadic ones that differ in parameter types; a fitting fixed or shorthand
  constructor is preferred (see the rules above);
- each tail value is checked left to right against the declared element type,
  with a tail-relative position in the diagnostic;
- exact generic `T` inference applies across the fixed prefix and every tail
  value, exactly like other generic constructor arguments;
- inside the body, the variadic parameter is the same opaque `varargs<Type>`
  binding used by variadic actions, unpacked with `values...` as a `let` value,
  a complete constructor field value, or a `for` source.

### 11.4 Constructor Invariants

```okkama
object InvalidScore does Error {
    has value is int;
    InvalidScore(:value);
    text() is string { "invalid score: " + value.string(); }
}
object Score {
    has value is int;
    Score(value is int) {
        self.value = value;
        confirm value >= 0 else throw InvalidScore(value: value);
    }
}
```

A constructor MAY contain any number of `confirm` statements. They execute in
source order in the constructor's lexical scope, after any preceding bindings.
An unsuccessful condition evaluates its `else` branch; `else throw` raises the
specified Error object. Without `else`, an unsuccessful condition panics.
Constructors initialize state and validate it directly with `confirm`. They MUST
NOT invoke any public, private or async action on their unfinished receiver,
including in conditions, field initializers or nested lexical blocks. This
restriction follows receiver identity through aliases, role references, bound
actions and collaborators. The analyzer rejects direct `self` calls; runtime
rejects every action dispatch on a receiver whose construction is pending,
before evaluating its arguments. Initialized field reads and calls on parameters
or other completed objects remain allowed. Merely storing a callback that uses
`self` is allowed; it can execute those calls only after construction completes.
Every constructor overload MUST explicitly validate its own input invariants.
A failed constructor does not return an object or run its projection. Registered
defers and owned-resource cleanup still run; the unfinished receiver's `destroy`
is not invoked as though construction had succeeded. Successor actions do not
implicitly run constructors and MUST validate their own transitions.

### 11.5 Destroy Block

`destroy` is an object lifecycle block.

```okkama
object Resource {
    has name is string;

    Resource(name is string) {
        name = name;
    }

    destroy {
        name;
    }
}
```

Rules:

- At most one `destroy` block is allowed per object.
- `destroy` runs automatically when a local object leaves its block/environment
  scope.
- `destroy` is not callable as an action.
- `destroy` can read `self` and fields.
- `destroy` MUST NOT use successor assignment.
- `destroy` MUST NOT create a partially destroyed object state.
- `destroy` is not tied to Perl garbage collection.
- Creating aliases to `self` or its fields inside `destroy` does not transfer
  ownership and cannot re-enter destruction of the same object identity.

Failure behavior:

```okkama
destroy {
    confirm false;
}
```

If a destroy block fails, the runtime reports the failure as a runtime error.

### 11.6 Public Actions

Actions define object behavior.

```okkama
object Cash {
    has digit is int;
    has cents is int;

    Cash(digit is int, cents is int) {
        digit = digit;
        cents = cents;
    }

    total_cents() is int {
        digit * 100 + cents;
    }
}
```

Rules:

- Actions can read fields directly.
- Actions can call other actions through `self`.
- Actions can have typed parameters.
- Actions must explicitly declare their result: `is <type>` or `is void`.
- Value-producing action results are type-checked.
- A caller's contextual expected type applies to the completed call result and
  does not flow into expressions evaluated inside the action body.
Every named action, role signature and native action requires an explicit
result declaration: `is <type>` or `is void`. The module entry point is
restricted to `main() is int` (optionally `async`); `is void` is forbidden
there. Omitting a result declaration is a syntax error.
`void` is a result marker, not a type or a value: it cannot be used for fields,
parameters, variables, array elements, map entries or generic arguments.
A synchronous void call, or an await of an async void call, may run for its
effects. Using its absent result as a value (including an initializer, argument,
operator operand, chained receiver or typed result) is a semantic error when
its result contract is statically known, and is also guarded at runtime.

The final expression of a void body is evaluated and discarded. Its value
is not transferred to the caller; owned temporary objects are cleaned up at
the normal scope boundary, including `destroy` and `defer` behavior.
An ordinary typed action validates its implicit or explicit result.
Returning the receiver requires an explicit result type and `self` expression;
successor actions retain their existing `^Type` rules.

Action types permit `(T) is void`; anonymous actions explicitly declare
parameter types and their result, including `is void`. `async ... is void` and `async (T) is void` are supported for named actions,
roles, native declarations and bound callbacks. `void` is a result marker,
not a type: there is no user-visible `Future<void>`, null or Unit value.
`await command.commit();` waits for completion without producing a value.
Capturing that await result, passing it as an argument, or placing it in a
collection or value-returning tail is invalid. Typed throws, panic, cancellation,
`defer` and local cleanup follow the ordinary async rules.
A call creates a lazy reusable completion handle: `let pending = command.commit();`
may be followed by `await pending;` more than once. Its inferred internal
completion contract has no source-level type spelling. `spawn` starts it eagerly.
Async anonymous action expressions remain unsupported.


Example:

```okkama
touch() is Cash {
    self;
}

main() is int {
    Cash(digit: 1, cents: 25).touch().touch();
    return 0;
}
```


#### Explicit early return

`return expression;` evaluates exactly one expression and exits the current
named or anonymous action, including `main`. The expression may be a scalar,
array, map, object, or callable. `return 1, 2;` and `return args...;` are invalid;
return `[1, 2]` to return one array. The declared result type is still checked.
Implicit final-expression results remain supported. The module entry point
can return only an `int` in `0..255`; other actions follow their declared types.

```okkama
first_positive(values is @int) is int {
    for values -> value {
        if value > 0 { return value; }
    }
    return 0;
}
```

Blocks, loops, `if`, `match`, and `try`/`catch` do not form a return boundary.
A functional iteration block also returns from its enclosing action. An explicit
anonymous action forms its own boundary. `catch Error` cannot intercept return.
`return;` is valid in a `void` action; ordinary typed actions require a value.
In a successor action, bare `return;` finishes the staged fields and produces the
usual fresh successor; an explicit replacement expression is forbidden.
Return is invalid directly in constructors, `destroy`, module scope, or a test
body. An action declared inside such a body has its own return boundary.

The value is evaluated once **before cleanup**. All reached `defer` expressions
run from inner to outer scope, LIFO within each scope, before owned destruction.
Returned objects, including objects reached through collections and closure
captures, stay alive for the caller. Code after return does not run or register
defers. A cleanup error cancels a pending successful return; remaining cleanup
still runs and an abandoned owned result is destroyed. A return executed by a
deferred expression can replace a pending return, but cannot suppress an existing
error. Calling an action from `defer` only returns from that called action.

Async actions support `return await task;` and await deferred cleanup before
settling their Future. Cancellation while awaiting a return expression also
unwinds cleanup. Cancellation requested during deferred cleanup is delivered
after that cleanup completes, before any result escapes; the Future stays
pending meanwhile. See [runnable example](../examples/return-defer.ok) and the
implementation checklist.

### 11.6.1 Recursion

An action MAY call itself, directly or through other actions (mutual
recursion). Recursion depth is limited only by available memory: an
implementation MUST NOT impose a fixed call-depth limit smaller than its
memory allows (for example the default native thread stack).

```okkama
object MathBox {
    MathBox();
    sum(n is int) is int {
        if n == 0 { 0; } else { n + self.sum(n - 1); }
    }
    count(n is int, total is int) is int {
        if n == 0 { total; } else { self.count(n - 1, total + n); }
    }
}
```

A synchronous call of the action itself in tail position — its result is the
action's result, with no cleanup, `defer` or enclosing `try` pending after
it — runs in constant stack space: `count(10000000, 0)` above needs no more
memory than `count(1, 0)`.

An action that cannot complete without recursing is a semantic error. The
check considers the synchronous calls of the action and of the actions it
calls directly; an action is rejected when no path from its start reaches
completion — a result, `throw`, `panic` or any operation that may fail
(a call outside the recursion, `await`, division, indexing, a conversion,
…) — without first calling an action of the same recursion. Calls through
action values and roles are not considered recursive. The diagnostic names
the action:

```okkama
object Loop {
    Loop();
    spin(n is int) is int { n + self.spin(n + 1); } // error: infinite recursion
}
```

### 11.7 Private Actions

Private actions are declared after a `private:` section marker.

```okkama
object Cash {
    has digit is int;

    Cash(digit is int) {
        digit = digit;
    }

    reveal() is int {
        self.secret();
    }

    private:
    secret() is int {
        digit;
    }
}
```

Rules:

- Private actions cannot be called from outside.
- Private actions can be called through `self` inside an action of the same
  object. Constructors cannot invoke actions on their unfinished receiver,
  regardless of visibility or async shape (see 11.4).
- A private call through a different receiver remains forbidden, even for
  another instance of the same type. Private actions cannot be bound/exported
  as values.
- Called actions do not receive constructor field-binding capability. They
  may read initialized fields; reading missing state fails.
- Private actions participate in overload dispatch like public actions.
- Private actions can have parameters.
- Private actions can have return types.
- Private successor actions are explicitly not supported.

External call error:

```okkama
Cash(digit: 7).secret(); // error
```

### 11.8 Action Overloads

Actions can overload by arity and parameter types. Arity is the number of
parameters, and a variadic tail counts as one parameter: `add(value is int)`
and `add(values is ...int)` both have arity 1 and are different signatures,
because `...int` is not `int`.

```okkama
add(value is int) is int {
    value;
}

add(left is int, right is int) is int {
    left + right;
}

add(value is string) is int {
    value.length();
}
```

Rules:

- receiver and argument expressions evaluate exactly once, left to right;
- a fixed signature fits a call with exactly its number of arguments, a
  variadic one fits a positional call with at least its fixed prefix;
- a call that fits a fixed signature never selects a variadic one: variadic
  candidates are considered only when no fixed candidate fits;
- candidates first match arity and named parameter shape, then the
  evaluated runtime values are checked against substituted declared types;
- exact concrete/applied types are more specific than role compatibility;
- a compatible concrete or role type is more specific than `any`;
- a concrete collection element type is more specific than a shape containing
  `any`;
- one candidate is selected only when it is no less specific in every
  parameter position and strictly more specific in at least one;
- cross-specific and duplicate signatures remain ambiguous; declaration order
  is never a tie-breaker;
- named calls may distinguish same-type signatures by parameter names, while a
  positional call cannot;
- an opaque value already typed as `any` is not narrowed to its stored concrete
  metadata during selection;
- no compatible typed candidate and multiple equally specific candidates have
  distinct diagnostics containing the actual argument types;
- private access is checked after the typed signature is selected;
- a variadic tail compares its element type with every tail value, so
  `...int` is more specific than `...any` for `add(1, 2)`; a call with an
  empty tail, or an unpacked `@never` array, is ambiguous between variadic
  candidates that differ only in the tail type;
- two signatures with the same parameter types are a declaration error
  unless both are fixed and their parameter names differ;
- Overloads with the same name and arity must declare the same return type.

Constructors use the same selection (11.3). Native module functions do not
overload. The complete
specificity and evaluation contract is recorded in
`docs/todo/typed-action-overload-resolution-14072026.md`.

### 11.8.1 Variadic Action Parameters

An action or role signature may declare one typed variadic tail after any
fixed prefix:

```okkama
format(pattern is string, values is ...any) is string {
    let materialized is @any = values...;
    pattern;
}
```

Rules:

- the variadic parameter is written `name is ...Type`;
- it must be the unique final parameter and cannot have a default value;
- full constructors may declare a variadic final parameter under the
  restrictions in section 11.3;
- an action with a variadic parameter overloads with fixed and other
  variadic signatures under the rules of 11.8;
- calls are positional only and must provide every fixed-prefix argument;
- the tail contains zero or more values, each checked deeply against its
  declared element type;
- arguments are evaluated left to right exactly once;
- inside the action the bare parameter is an opaque `varargs<Type>` binding and
  cannot be used as an ordinary expression;
- `values...` is a dedicated one-level unpack expression for array or
  variadic parameter bindings (array-valued expressions are also allowed),
  not a numeric range or general iterable spread;
- unpack is supported as the complete RHS of `let`, as the complete value of a
  constructor field binding (`field = values...` or
  `self.field = values...`), as a `for` source, or as the final positional
  argument of a variadic action or full-constructor call, or any positional
  argument of an array cast (section 6.6.1), or the right source of `<-`.

Materialization creates a new ordinary array:

```okkama
let inferred = values...;
let checked is @int = values...;
```

Iteration reads the tail in call order without exposing mutable accumulator
state:

```okkama
for values... -> value {
    value;
}
```

A variadic binding can be forwarded without materializing an array:

```okkama
delegate(pattern is string, values is ...any) is string {
    string Formatted(pattern, values...);
}
```

Forwarding supplies only the target's variadic tail. The caller MUST provide
the whole fixed prefix explicitly; one final positional unpack may follow
ordinary explicit tail arguments. Its source MUST be an array or variadic
binding, and the ordinary target MUST have a variadic tail. Array casts are
the separate variable-arity intrinsic exception: they allow multiple unpacks
in any positional slots and perform their explicit element conversions.

Unpack reads its immutable source once. Values retain order and cross the
ordinary target tail's type boundary exactly as explicit tail arguments do,
without implicit casts. Empty sources supply zero values; a retained element
type supplies evidence for an empty generic constructor tail. An untyped empty
source requires other evidence or an explicit expected specialization.
Forwarding `any` elements MUST NOT unbox or nest their opaque boxes. Async
argument rewriting MUST preserve the unpack marker and the already-read source.

The operand MAY be an array-valued expression, evaluated exactly once. It
MUST undergo ordinary semantic and successor-use analysis. A bare variadic
binding is read directly without making it a first-class value. Named unpack,
array/map literal spread, return spread and ordinary non-final forwarding
remain invalid. Fixed-arity,
native and scalar cast calls MUST NOT acquire dynamic arity through unpack.
The separate `spawn` transfer boundary does not transport expansion values.

Role implementations and test fakes must match the fixed/variadic shape,
every fixed-prefix type, the variadic element type, and the return type.

Ordinary fixed and variadic object actions always execute their Okkama bodies.
They cannot be replaced by host callbacks. Host implementations require an
explicit bodyless module-native declaration and canonical provider symbol.

### 11.9 Successor Actions

A successor action returns a new object with staged field updates.

```okkama
object Bottle {
    has opened is bool = false;

    Bottle() {}

    open() is ^Bottle {
        ^opened = true;
    }
}
```

Rules:

- Successor actions MUST declare `is ^Type`.
- `^field = expr` is allowed only inside successor actions.
- The field must exist.
- The assigned value must match the field type.
- The original object is not mutated.
- The returned successor result is a must-consume value.
- Statically visible loss is a semantic error; runtime discard checks remain defensive.
- A successor MUST describe a domain transition and use a result-oriented
  domain name. Immutable field-copy setters and stepwise configuration
  builders are not valid object design; `with` and `with_*` action spellings
  are rejected by the parser.

Correct:

```okkama
let bottle = Bottle();
let opened = bottle.open();
let saved = [opened]; // aggregate storage consumes the successor
```

Incorrect:

```okkama
let bottle = Bottle();
bottle.open(); // error: successor result must be used
```

### 11.1 Static successor consumption

A forward, intra-action analysis tracks successor obligations by value origin,
not by a count of variable reads. `let unused = object.revision();` is an error
if the binding reaches a normal scope exit without consumption. A bare read
`unused;` in a discarded statement and copying into another dead binding do not
consume it. Aliases share the obligation; consuming one alias is sufficient.
Overwriting a binding does not consume the previous value if no surviving alias
is used. Already consumed successors remain reusable; this is not linear typing.

Consumers include explicit and tail returns, eligible local rebinds,
constructor and successor field bindings, receiver calls, call arguments, and
valid aggregate construction. Normal type, dispatch and rebind checks still
apply. Chaining a successor action consumes its receiver and creates a new
obligation. Block/branch results, index fallbacks, loop results and reductions
carry obligations to their enclosing consumer instead of discarding them.

At a branch join an obligation remains pending if any reachable normal path
leaves it pending. A conditional alias or closure cannot discharge a value on
paths where it did not receive that value. Literal boolean branches are known;
other conditions are not evaluated by the analyzer. Loops conservatively assume
no guaranteed iteration: body-only use cannot discharge an outer obligation.
New iteration-local obligations must be used before `next`, `last`, or iteration
exit, or be transferred as a loop result. No bound or termination proof is made.

A normal `return` must consume outstanding values. Explicit `throw` and `panic`
may abandon them because the action did not complete successfully. A catch is
analyzed from try-entry state: an operation could have failed before a use.
This intentionally rejects some programs that would require proving exactly
which operation can throw. It does not require persistence or business effects.
Reached `defer` expressions are credited at their own scope exit, using the
bindings after intervening rebinds. Creating a deferred discarded successor is
still invalid. Conservatively, an early return from a nested scope cannot rely
on an enclosing scope's deferred expression to discharge an obligation.

Creating an anonymous or bound action does not itself consume captured values.
A closure carries the captured obligations its body uses on all normal paths;
invoking it or passing/returning/storing it through a consumer discharges them.
A dead closure does not hide a dead successor. Independently created successors
inside anonymous bodies are checked as well.

Known `Future<^T>` results are tracked until consumption; awaiting one transfers
its obligation to the successor result. Passing the future across a typed
boundary counts as language-level use, without proving eventual task completion.
Calls whose successor contract cannot be determined statically retain runtime
safety checks. The analysis does not inspect arbitrary callees or perform
whole-program inference. Diagnostics retain the offending binding or call token,
including source file, line and column; the existing CLI prints a minimal form.

## 12. Roles

Roles define nominal behavior contracts.

```okkama
role Package {
    open() is ^Package;
    close() is ^Package;
}
```

Objects declare role implementation with `does`.

```okkama
object Bottle does Package {
    has opened is bool = false;

    Bottle() {}

    open() is ^Bottle {
        ^opened = true;
    }

    close() is ^Bottle {
        ^opened = false;
    }
}
```

Runtime role rules:

- role declarations are registered before object conformance checks;
- duplicate role declarations are runtime errors;
- a role may declare same-name/same-arity fixed signatures when their complete
  parameter type vectors differ and their return contract is the same;
- exact duplicate role signatures are runtime errors;
- `object does Role` requires the role to be declared;
- objects and fakes must implement every required typed role action;
- role action parameter types and fixed/variadic shape must match exactly;
- role action return types must be compatible;
- `^Concrete` satisfies `^Role` when `Concrete does Role`;
- concrete object values are assignable to role-typed runtime boundaries when
  the concrete object type explicitly implements the role;
- role-typed references dispatch only signatures declared by the selected
  role, invoking their implementation on the original value.

### 12.1 Restricted role references

A role-typed boundary creates a restricted reference to the original value,
without copying its state. This applies to locals, fields and defaults,
parameters, results, callbacks and collection elements. Aliases, bound actions,
generic inference and local async results retain the restriction. Built-in
values using nominal role adapters obey the same access rule.

Only the role's declared public action signatures are accessible. Concrete
fields, extra actions and extra overloads of an allowed action name are hidden;
Fields are inaccessible through roles. Named arguments use the role's parameter names,
independently of implementation names. Implementation bodies receive the full
original `self` and retain ordinary access to their own fields/private actions.
Other full references are unaffected.

Action results are checked against the selected role signature, including
covariant successor results: `^Concrete` returns a restricted `^Role` when the
selected role action declares it. Async futures carry that declared result
contract. Successor must-use requirements remain in force.

An ordinary role reference cannot become a full concrete reference or a named
object. It may cross another role boundary only if the original value
nominally implements that role and each target action signature is already
available through the current role. Parameter types, async/void flags and
fixed/variadic shape match exactly; returns use the existing role-return
compatibility rule. This permits capability narrowing, not structural
conformance or later widening. Standard iteration and functional operations
must likewise have access to the declared nominal iteration contract.

Unconstrained generic inference from a role reference uses its role type,
not the concealed concrete type. A role reference may be boxed into opaque
`any`; boxing preserves its restriction. Equality keeps the original value's
existing semantics. Ownership traverses the complete original object graph,
including hidden resources. The restriction is shallow: an allowed action's
result is accessible according to its own declared type.

Restricted role references are VM-owned values: local async tasks can retain
them, but the existing synchronous process-worker codec cannot transfer them.
No new native or process serialization ABI is introduced.

Checked exception routing is an explicit exception to ordinary conversion:
Only a role reference exposing the Error contract may enter `throw` routing.
`throw` retains the original Error object in the failure container, allowing
existing concrete typed catches. A role-typed catch binds a restricted
reference; in particular `catch e is Error` exposes only `text()`. Ordinary
role parameters/results do not acquire this exception-routing privilege.

## 13. Role Fake

Roles may contain a test-only fake block.

```okkama
role Package {
    open() is ^Package;

    fake {
        has opened is bool = false;

        open() is ^Package {
            ^opened = true;
        }
    }
}
```

Runtime behavior:

- role declarations may contain at most one `fake` block;
- fake blocks are registered as role metadata;
- `Role.fake(...)` is rejected in production mode;
- `Role.fake(...)` is available only in explicit test mode;
- a fake is represented as an object-like runtime value associated with the
  role;
- `has` declarations inside the fake define typed receiver state; every field
  must have a default because role fakes have no constructor parameters;
- fake field defaults are evaluated for each `Role.fake()` construction, and
  fake actions may read them or update successor state through `^field`;
- fake actions must satisfy the role contract by name, fixed/variadic shape,
  parameter types, and return type.

Every role fake instance owns a hidden mutable runtime KV storage. It is
available only as the direct receiver form `fake.storage.<operation>(...)`
inside actions declared by that fake:

```okkama
role Books {
    remember(id is int, title is string) is void;
    title(id is int) is string;

    fake {
        remember(id is int, title is string) is void {
            fake.storage.set(id, title);
        }

        title(id is int) is string {
            fake.storage.get(id);
        }
    }
}
```

The fixed positional-only API is `set(key, value)`, `get(key)`, `has(key)`,
`remove(key)`, `clear()`, and `size()`. `get` and `remove` on a missing key are
non-catchable runtime programming errors. Keys follow ordinary map-key
restrictions: supported scalar keys are accepted, while `any`, `byte`, and
composite keys are rejected. Values may be any runtime-supported Okkama value,
including objects and opaque `any`.

Each independent `Role.fake()` call creates an isolated storage identity.
Successors of one fake share that same identity; storage is not copied or
reset. It is not a field or first-class Okkama value, cannot be reached through
`self.storage`, and does not participate in equality, successor field state,
construction, role conformance, debug output, or nominal type identity.

Full test assertion/reporting APIs are outside this section.

## 14. Tests

Test blocks are declarations at module, object, or role scope.

```okkama
test "basic behavior" {
    test.ok(true);
}
```

Use explicit `async test` when the body needs `await`:

```okkama
object Worker {
    Worker();
    async value() is int { 7; }
}
async test "async result" {
    let result = await Worker().value();
    test.is(result, 7);
}
```

This declaration is available at module, object and role scope and uses the
same `-t` command. Ordinary tests remain synchronous. Async tests use the normal
scheduler and await/typed-failure machinery; assertions may contain await
directly, and the contextual `test` capability remains available to async
helpers. Repeated awaits follow the ordinary reusable-Future rules.

An async test completes without producing a value; its ordinary tail expression
is discarded, and `return` remains restricted to actions. The runner waits for
body completion and defer/destroy cleanup before recording pass/fail or starting
the next case. Pending spawned tasks are cancelled and drained under the normal
root-task policy; unobserved failures belong to that test. Throw, panic and
runtime failures fail the current case while subsequent cases still run.
There is no implicit test timeout. Async tests are pruned in production and
share discovery, naming and assertion rules with sync tests. No generated main
wrapper is needed.

Production parsing consumes test/fake headers and balanced bodies but omits
their AST nodes and does not parse their inner grammar. Lexical errors still
apply. Test parsing is selected explicitly with `okkama -t file.ok`; that mode
propagates through imports so imported fakes are available, while only tests
owned by the root module are executed.

The runner does not execute or require `main`. It discovers tests in source
order. Object and role tests are displayed as `Owner :: test name`; they gain
no implicit `self` or private access. Duplicate qualified names and an empty
root suite are errors.

Each test receives a contextual binding with no import:

```okkama
test.ok(condition);          // exact bool
test.is(actual, expected);   // strict/deep equality
test.isnt(actual, expected); // strict/deep inequality
test.fail("message");
```

Assertions evaluate arguments left-to-right exactly once. A failed assertion
is recorded without aborting its block. The trusted assertion boundary may
compare concrete snapshots carried by `any`, including nested `@any` and
typed-map values, but does not return or narrow those payloads to source code;
ordinary `any` equality remains forbidden. Strict category matching, deep
collection/object comparison, and native-handle rejection are otherwise
unchanged.

The capability is available throughout the dynamic execution of that test,
including actions declared in imported modules. It remains contextual and
non-first-class, is fresh for every case, and does not exist in production
execution. An uncaught throw, panic, or runtime error aborts only that test;
subsequent tests still run. Each test has a fresh root environment, and its
defer/destroy cleanup completes before reporting and before the next test. The
CLI prints deterministic RUN/PASS/FAIL records and returns 0 only when every
test passes, otherwise 1. Host/provider/bootstrap or shutdown corruption
remains a harness error. There is no `test.panics` in Okkama 1.0; use typed
`try`/`catch` for throw behavior. Panic is uncatchable.

The bundled `test/more` module exposes stateless `Assertions` actions `ok`,
`is`, `isnt`, and `fails`. They delegate to the current capability and preserve
its non-aborting behavior; `fails(error is Error)` records `error.text()`
unconditionally. `AssertionFailure(message)` is a state-only `Error` value
whose `text()` action computes a stable assertion diagnostic. Calling an
`Assertions` action outside a running test is an undefined-capability error.

## 15. Enums

Enum syntax:

```okkama
enum Status = (ready, failed, done);
```

Enum names use type-name syntax. Enum cases use lower-case identifier syntax.

Enum cases are referenced with member syntax:

```okkama
Status.ready
```

Runtime enum rules:

- enum declarations are registered before `main` executes;
- duplicate enum declarations are runtime errors;
- duplicate cases inside one enum are runtime errors;
- `Enum.case` evaluates to an enum runtime value;
- enum values infer as their enum type;
- typed runtime boundaries accept only values of the matching enum type;
- enum equality compares enum type and case;
- enum values work in `match` cases through normal equality.

## 16. Access Control

Fields are private. Only the owner's lexical fields and `self.field` read them.
Other objects, role implementations, root code and cap computations have no field
access grants. Collaborators call public owner behavior or receive deliberately
constructed typed values/resources. Public actions do not expose their owner's
other fields; role references and caps keep their declared restricted surfaces.

The former `allow` declaration is removed and rejected by the parser. There is
no compatibility switch. Migrate by moving the operation into its owner and
letting adapters delegate; never export a native handle merely to recover a
former grant. See [migration guide](allow-removal.md).

## 17. Object Construction

Object construction is call-based:

```okkama
Cash(digit: 1, cents: 25)
```

Named arguments use the same parenthesized call syntax.

## 18. Async and Await

`async` marks an action whose declared result is produced asynchronously:

```okkama
async loaded(service is Service) is string {
    let prefix = "loaded: ";
    let value = await service.load();
    prefix + value;
}
```

If an async action declares result `T`, its body is checked as producing `T`
but calling it produces `Future<T>`. `await Future<T>` suspends the current
async action until the future completes and then produces `T`.
For `is void`, the call instead carries completion without a value result.
Awaiting it is valid in effect context only; `void` is not a type argument.

```text
service.load()        : Future<string>
await service.load()  : string
loaded(service)       : Future<string>
```

Rules:

- an async action call is lazy and does not start its body until first awaited;
- a future starts at most once and caches either its value or its failure;
- repeated await returns the cached outcome without repeating the action;
- there is no implicit await. `Future<T>` cannot be used where `T` is required;
- await is legal only inside an async action, including `async main()`;
- the operand of await must be a `Future<T>` or an async void completion;
- awaiting a non-future is a static error;
- locals and lexical scopes survive suspension without replaying statements;
- a typed error completes the future exceptionally and is rethrown at await,
  where ordinary typed try/catch applies;
- value-producing await may appear inside larger expressions and supported block statements.
  Evaluation resumes at that exact operand without replaying earlier
  operands or statements;
- operands, call arguments, constructor fields, collection elements, match
  patterns, spawn arguments, and functional iterations retain their ordinary
  left-to-right order across suspension;
- short-circuit branches, index fallbacks, postfix conditions, confirm
  fallbacks, and conditional loop-control payloads remain lazy across
  suspension;
- async shape is part of role conformance. Object and fake implementations
  must agree with the role signature;
- async does not create a thread and does not make CPU-bound work nonblocking;
- one cooperative scheduler belongs to a runtime. A synchronous main does not
  drive it; async main drives it until the root future is ready;
- async actions do not provide CPU parallelism. `spawn` starts async actions
  as concurrent tasks in the current runtime and synchronous actions through
  the restricted process-parallel contract below;
- no detached tasks, cancellation, timeout syntax, race, join, or async
  iterators exist in the first version;
- bounded typed channels and the state-owner pattern are specified in
  [`async-channels.md`](async-channels.md).

`await` is a unary expression. Postfix operations on its result require
grouping:

```okkama
let text = (await pending).string();
let value = await service.load() + 1; // (await service.load()) + 1
```

### 18.1 Tasks and process workers with `spawn`

Spawning an async object action eagerly schedules that action on the current
runtime event loop. The existing `Future<T>` is reused rather than wrapped as
`Future<Future<T>>`:

```okkama
let owner is Future<int> = spawn Worker().run_async();
let result is int = await owner;
```

Async tasks share one VM and may therefore exchange VM-owned capabilities such
as channel endpoints. They provide cooperative concurrency around pending
`await` operations, not CPU parallelism.

Spawning a synchronous object action eagerly starts it in an isolated worker
process and produces `Future<T>`:

```okkama
async main() is int {
    let left is Future<int> =
        spawn Worker(seed: 7).calculate(input: 20);
    let right is Future<int> =
        spawn Worker(seed: 9).calculate(input: 6);

    (await left) + (await right);
}
```

If `Worker(...).calculate(...)` has result type `T`, the spawn expression has
type `Future<T>`. Receiver and argument expressions are evaluated eagerly,
left to right, in the parent runtime: constructor arguments precede action
arguments, and a failure stops later argument evaluation and prevents launch.
The action itself runs in a fresh worker VM. Multiple spawn expressions can
therefore execute CPU-bound or blocking synchronous actions in parallel before
either Future is awaited.

The first process-task contract is deliberately narrow:

- only `spawn Worker(...).action(...)` is valid;
- an async selected action is scheduled in the current runtime and may retain
  VM-owned values; a synchronous selected action uses the process boundary;
- captured receiver objects, direct action calls, anonymous actions and
  arbitrary blocks are not spawn targets;
- transferable values are `int`, `float`, `string`, `char`, `byte`, `bool`,
  typed or untyped arrays, and maps recursively containing transferable
  values;
- objects, Futures, actions/closures, native handles and other VM-owned values
  cannot cross the synchronous worker boundary; they remain valid for an
  async task because that task stays in the current VM;
- the result or failure is cached in the returned Future. Repeated await does
  not repeat the process action;
- worker failure is isolated in that Future and is raised when it is awaited;
- the worker receives the source file and the parent's explicit working
  directory. Relative imports and file operations therefore retain the
  command's current-directory semantics;
- cancellation, deadlines, detached tasks, shared mutable state and automatic
  object serialization are not part of the MVP.

### 18.2 Bounded typed channels

The `async/channel` module provides invariant `Channel<T>`, `Sender<T>`, and
`Receiver<T>`. Channels have positive bounded capacity, FIFO delivery and
waiters, async backpressure, explicit idempotent close, and typed
`ChannelClosed` and `InvalidChannelCapacity` failures.
Either a `Sender<T>` or `Receiver<T>` endpoint may initiate close.

Channels allow one spawned async task to own and rebind immutable application
state while clients communicate by typed commands and capacity-one reply
channels. The complete semantics, lifecycle rules, limitations, and executable
examples are specified in [`async-channels.md`](async-channels.md).

## 19. Defer

Syntax exists for:

```okkama
defer cleanup();
```

`defer` registers its expression only when execution reaches the statement.
The expression is not evaluated at registration time. It remains attached to
that concrete lexical scope and is evaluated while leaving the scope, with the
scope environment and its local values still alive.

Rules:

- each lexical environment owns an independent defer stack;
- deferred expressions execute in last-in-first-out order;
- an unvisited branch registers no defer;
- inner scopes finish their stacks before an enclosing scope starts its own;
- stacks run on normal completion, throw, panic, programming/runtime error,
  `next`, `last`, `return`, and async cancellation;
- defer runs before automatic `destroy` for objects owned by the same scope, so
  cleanup expressions may still call those objects;
- every registered defer runs even if an earlier cleanup in LIFO execution
  fails;
- expression results are discarded, but ordinary discarded-result restrictions
  still apply: a must-use successor cannot be ignored and a variadic expansion
  does not gain a new legal context;
- constructor, action, `destroy`, try/catch, loop-iteration and ordinary
  nested bodies use the same lexical rule.

Failure precedence is deterministic:

1. an existing body failure wins (a pending `return` is not a failure);
2. otherwise the first defer failure in LIFO execution order wins;
3. otherwise an automatic destroy failure wins.

Cleanup continues after a defer or destroy failure even when that failure is
not ultimately propagated. An enclosing `catch` observes the winning thrown
failure only after defer and destroy cleanup for the exited try scope has
completed.

### Resource dependencies and expression ownership

Fresh receivers/arguments without a caller owner are adopted before action
bindings; existing caller-owned inputs are borrowed. Anonymous actions adopt
fresh arguments. Unbound expression values have lexical ownership; action exit
cleans its inputs or transfers reachable results. Async continuations use their
explicit lexical target, including match/postfix/awaited results and defer.

Channel payloads retain actual resource identities independently of the sender's
scope. Delivery attaches retention to the receiving scope; escaping values
transfer it to the caller. Shared/repeated identities are destroyed once after
all retained owners finish. Explicit close preserves buffered delivery;
cancellation before handoff releases the undelivered retention. Ending all
channel capabilities cleans unread values, before native registry teardown.

Completed constructions with their own cleanup are tracked until first admission.
A later expression failure therefore cannot lose an earlier fresh resource.
Unfinished constructor self is excluded. VM-created successor identities and
Channel endpoints use the same admission tracking. Async successors retain the
explicit startup caller as result owner across suspension and foreign signals. A valued last transfers its reachable
result across cleanup; a valueless last remains ordinary loop control.

An ownership-free certificate for a completed object requires no destroy bodies,
intrinsic state or fake storage and certificates for all actual fields. The field
table is detached and frozen. Such a certificate is not inferred from a nominal
name or a declared generic/collection tag. Futures, actions, unknown host values
and resource-bearing graphs retain conservative traversal.

## 20. Diagnostics

Production process workers carry a detached diagnostic model with
`diagnostic_version=1` inside protocol 1 failure responses. Parent diagnostics
preserve child category/type/origin/stack/cause/secondary; new parent cleanup
failures are appended. Copies preserve failure identity for deduplication;
separate failures with identical text remain separate. Legacy response text is
compatible. This does not reconstruct application Error payloads or make
RuntimeError/Panic catchable as Error. Wire quotas and explicit limit fallback
are described in the worker report.


The implementation prefers explicit runtime errors. Common interpreter runtime
errors include source line numbers when the failing expression or statement has
a token location.

Location-aware runtime errors stringify as:

```text
Runtime error at line N: message
```

The runtime exception still exposes the plain semantic `message` separately
from the formatted string. Runtime failures, panic carriers and typed throw
carriers additionally expose `stack_trace()` to host tooling. This is not
an Okkama reflection API and does not change which failures `catch` can match.

The trace is captured when the failure carrier is created, before unwinding.
It is an array of source frames, innermost first, containing:

- `name`: concrete `Object.action`, `Object.new`, `Object.destroy`,
  `<anonymous>`, a native declaration name, or `main`;
- `kind`: `action`, `constructor`, `destroy`, `anonymous`, `native`, or `main`;
- `module`: the declaring Okkama module;
- `source_name`, `line`, and optional `column`: the fault location for the
  innermost frame and the active child's call site for each caller;
- `declaration` and `call_site`: separate source-location records, each with
  `source_name`, `line`, and `column` when known. The root main and implicit
  destruction have no explicit call site.

Lines and columns are one-based; columns count source characters, not terminal
cells. Imported files retain their resolved filenames; injected source loaders
without a resolved file retain their logical import names. Missing fault/call
locations fall back to the declaration location. Errors created outside Okkama
execution may have an empty trace. Synthetic tokens may lack columns.
Frames contain no arguments, locals, object references, or host/Perl frames.
Returned arrays and nested location records are defensive copies.

Async continuations restore their captured source context when resumed, including
loop scheduling, native completion and cleanup. A lazy call retains its original
invocation ancestry; awaiting the same future elsewhere does not rewrite its
origin or append another synchronous caller. Separate concurrent tasks do not
share a mutable call stack. Bare rethrow, failure-envelope flattening and adding
secondary cleanup failures preserve the original trace. Each secondary failure
retains its own trace at creation.

The CLI currently appends only a minimal rendering, for example:

```text
RuntimeError: Array index out of bounds
index: 9
length: 1
at Broken.value (domain.ok:4:25)
at Payments.execute (application.ok:5:39)
at main (main.ok:3:36)
```

The existing process-worker failure protocol still transports a rendered message;
this change does not add cross-process structured trace transport or merge worker
frames with the waiting process. Cooperative async calls use structured traces
within their runtime.

Examples:

```text
Runtime error at line 4: Undefined variable "missing"
Runtime error at line 7: Object "Box" has no action "missing"
Runtime error at line 10: Duplicate construction field "digit"
Runtime error at line 4: Array index out of bounds
Object "Cash" is already declared
Field "digit" is already declared on object "Cash"
Object "Cash" field "cents" was not initialized
Action "secret" on object "Cash" is private
Successor action result must be used
Successor assignment is not allowed inside destroy
```

Default executable diagnostics render the category/concrete type, message,
optional public code/context, saved source stack, retained cause chain and
secondary cleanup failures. Array bounds failures include index and length.
`-d` still enables the separate evaluator trace; diagnostics do not require it.
Source snippets and caret presentation remain future work. File and call-site
metadata are available independently of rendering.

The host `Interpreter.diagnostic(failure)` API returns a detached diagnostic
model with `category`, `type`, `message`, `details`, `stack`, `secondary` and
optional `origin`, `code`, `context`, `cause`. The renderer does not modify the
failure or change catch matching. Embedded interpreter calls retain their
original exception carriers and legacy stringification.

For an application Error, the default renderer reads the declared public
zero-argument synchronous `text()`, optional `code()` and `context()` through
a bounded data-only projection. Optional `diagnostic()` supplies an additional
text detail when different from `text()`. Supported expressions include
literals, own fields, local bindings, string concatenation, scalar comparison,
boolean conditions and data-only zero-argument helpers. This is a tooling
projection, not a new language purity rule or a change to action execution.
It never runs the evaluator, constructors, I/O, mutation, async work or arbitrary
native actions after failure. Unsupported projections retain the error type,
stack and cause with an explicit unavailable-message fallback. Private fields
are not automatically enumerated; context must be supplied by the application.

Projection work is bounded to 512 steps and depth 32. Cause traversal stops at
repeated carriers or depth 32; individual displayed strings are limited to
8192 characters with an explicit truncation marker. Terminal control characters
are escaped so error text cannot impersonate stack/cause lines. Unicode text
is retained. Native failures expose their existing message, portable code,
domain and operation; the renderer does not invent a separate OS cause.

### Typed failure cause chains

When a selected `catch` handler exits with a different typed failure, its
private `Thrown` carrier retains the handled carrier as `cause`. This records
failure *during handling*, including a new native failure or a failure raised
by a called action. It does not assert that the application explicitly declared
a causal relationship. The rule applies to synchronous and asynchronous
handlers, including resumption after `await`, and to try expressions.

The outer Error remains the value used for catch matching. The cause retains
the exact inner Error value, origin token, source stack, native metadata and
secondary cleanup failures. Existing inner chains are retained; an enclosing
handled failure is appended at their end. Bare rethrow and copies made for
cleanup do not add a self-cause. A successfully handled error does not become
the cause of a subsequent throw outside the handler. Panic, programming errors
and control signals are not converted into typed failures or causes.

This is runtime metadata, not an extension of the nominal `Error` role.
Host tooling can traverse `Thrown.cause()` until host `undef`; no Okkama `null`
or no-cause Error object is introduced. `native_cause` remains the adapter's
original native Error value; `secondary_failures` still describes cleanup
failures, not causes. The minimal diagnostic renders each retained carrier
under `Caused by:` with its own stack. It does not invoke arbitrary `text()`
actions or reflect over application fields.

An application that needs to retain a cause in an ordinary value can declare
an explicit wrapper with a required `original is Error` field and public
`cause() is Error { original; }`. Plain domain errors need not implement that
capability. Constructing such an object outside a handler does not synthesize
runtime origin/stack metadata or cause links; its explicit field is the value
protocol. The runtime never infers causality from a field or action name.
Historical application objects named `Failure` are not a built-in language
contract, and their string fields are not migrated automatically.

## 21. Complete Runtime-Supported Example

```okkama
module app v1.0.0;

object Bottle {
    has opened is bool = false;

    Bottle() {}

    open() is ^Bottle {
        confirm !opened;
        ^opened = true;
    }

    close() is ^Bottle {
        confirm opened;
        ^opened = false;
    }

    is_open() is bool {
        opened;
    }

    destroy {
        opened;
    }
}

object Cash {
    has digit is int;
    has cents is int = 0;

    Cash(digit is int, cents is int) {
        digit = digit;
        cents = cents;

        confirm self.digit >= 0;
        confirm self.cents >= 0;
    }



    total_cents() is int {
        digit * 100 + cents;
    }

    touch() is Cash {
        self;
    }

    private:
    fee(value is int) is int {
        value + 10;
    }

    add_fee(value is int) is int {
        total_cents() + self.fee(value);
    }
}

main() is int {
    let prices is %string:int = ["apple": 120, "pear": 140];

    {
        let bottle = Bottle();
        let opened = bottle.open();
        opened.is_open();
    }

    Cash(digit: 7, cents: 65).touch().add_fee(prices["apple"]);
    return 0;
}
```

## 22. Current Implementation Boundaries

Runtime-supported and tested:

- modules and `main`;
- imports, basename namespaces, `take`, and dependency closure;
- literals;
- arithmetic and comparisons;
- bool literals;
- arrays, maps, indexing, fallback;
- ranges;
- `let`, block scope, if, match, for, next, last;
- confirm, throw and panic;
- mandatory explicit object constructors and exact-arity overloads;
- field defaults and type checks;
- actions, parameters, named arguments, return checks;
- anonymous action expressions, lexical closures, action-value calls,
  `(T, ...) is R` types, and explicit parameter/result declarations;
- private actions;
- successor actions;
- destroy registration and automatic block/environment-scope execution.
- enums, roles, role conformance, and role fakes;
- builtin `byte`, immutable `@byte`, explicit `any`, and homogeneous/`any`
  variadic action tails;
- variadic full constructors with a positional fixed prefix and one final
  typed tail;
- builtin value behavior for `array.size()`/`.slice()` and exact-`@string`
  `.join()`, `string.length()`/`.slice()`/`.trim()`/`.chomp()`/`.split()`/
  `.replace()`/`.replace_first()`,
  `map.size()`/`.has()`/`.keys()`/`.values()`,
  and exact same-type numeric `.less()`/`.less_or_equal()`/`.more()`/
  `.more_or_equal()`, backed by an immutable typed `ArrayValue<T>`/
  `MapValue<K,V>` runtime representation that retains element/key/value
  identity even through empty results (see 6.6);
- typed `try`/`catch`, valued/bare throw and string-only panic semantics;
- explicit single-value `return` from the current action;
- lexical LIFO `defer` on normal, return, failure and loop-control exits, before
  automatic destruction;
- synchronous-contract `async` actions with mandatory direct `await`, exact
  role/fake conformance and no future or scheduler runtime;
- production-pruned test/fake AST, explicit `okkama -t` discovery, contextual
  `test.ok`/`test.is`/`test.isnt`/`test.fail`, boxed assertion equality,
  bundled `test/more`, isolated failures and deterministic status;
- bodyless module native declarations, canonical provider descriptors,
  injected/default provider dispatch, and the monotonic-clock vertical;
- isolated lexical declaration scopes for actions, constructors, defaults,
  destroy, and imported declarations.

Parser-supported but not fully runtime-specified:

- owner-only private fields and explicit public behavior;
- decimal runtime semantics.

Explicitly not part of Okkama 1.0:

- null/none/nil source values;
- optional types and optional chaining;
- inheritance;
- `isa`;
- `super`;
- abstract/protected class features;
- mutable object fields after construction;
- implicit string keys in map literals.
