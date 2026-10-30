# Native Runtime Architecture

Дата среза: 15.07.2026
Статус: module-native linker, portable value/failure ABI, opaque handles and
bundled runtime providers through `core.regex` implemented.

BUG-04 update, 14.10.2026: homogeneous nonempty array/map literals establish
exact recursive tags in the interpreter before crossing this ABI. The shared
`exact_value_type` authority reads those tags; native `any` does not infer them
by walking untagged values. Empty/heterogeneous untagged containers remain
rejected, ordinary object reflection and opaque-handle restrictions are unchanged.

BUG-08 update: scalar map keys include byte/char/bool and nominal enums. Native
maps transport logical key values without stringifying their type; wrong enum
identity, duplicate logical keys and unknown enum cases are rejected. Handle,
object and Any key restrictions remain explicit.

## Назначение

Этот документ определяет переносимую границу между Okkama source/stdlib и host
runtime. Он не является описанием Perl API. Perl — первая реализация provider
side; Rust, C или другой runtime должны реализовать ту же логическую модель без
изменения Okkama declarations.

Практическая инструкция для добавления новой функции:
`docs/native-functions-tutorial.md`.

```text
application
    -> stdlib objects/roles
    -> native declarations
    -> native linker + logical value/error ABI
    -> provider implementation
    -> host/runtime/OS
```

## Design goals

- minimal universal substrate;
- stable module-qualified symbols;
- explicit typed declarations;
- deterministic link and invocation errors;
- language-neutral values/errors/handles;
- stdlib implemented mostly in Okkama;
- provider injection for tests and embedding;
- future Rust/C implementation without compatibility shims for Perl behavior;
- no arbitrary host reflection or package invocation from Okkama source.

## Non-goals

- making every OS syscall public;
- high-level HTTP/SQL/JSON/text-file operations in core;
- hiding platform differences by returning incorrect synthetic data;
- using native layer as an escape from Okkama visibility/type/lifecycle rules;
- defining async scheduler, Result type, generic system or plugin package manager
  in the first vertical.

## Core vocabulary

### Native declaration

A bodyless Okkama declaration that states a required host-provided operation or
opaque type.

```okkama
module runtime/time v1.0.0;

native monotonic_clock() is int;
```

It is part of source-level API and tooling. It is not a host callback and does
not identify Perl/Rust/C implementation details.

### Native symbol

Canonical runtime identity derived from declaration owner and signature.

Implemented fixed-symbol form:

```text
runtime/time@1::monotonic_clock/0
```

This string is diagnostic/manifest notation. Implementations may internally
use structured keys, but equality semantics must match the canonical fields.

### Native descriptor

Provider registration metadata:

```text
symbol
ABI major
parameter descriptors
return descriptor
fixed/variadic signature shape
provider identity
handler/private implementation pointer
capability flags where required
```

### Native provider

Host implementation bundle that registers descriptors for a domain. Core
providers ship with runtime; optional providers may implement database/curl or
other external-library boundaries.

### Native value adapter

Runtime-owned converter between Okkama values and logical native ABI values.
Providers do not directly inspect interpreter environments or AST internals.
The implemented Perl representation lives in `Okkama::Runtime::NativeValue`;
all conversion is centralized in `Okkama::Runtime::NativeValueAdapter`.

The bundled `core.debug` provider is the sole diagnostic exception to nested
`NativeValue::Any` opacity: it may recursively read the trusted logical payload
to produce text, but still receives no Okkama object, environment, AST node,
native handle resource, or language-level `Any` box. This privilege is not part
of the ordinary third-party provider contract.

### Native handle

Opaque nominal value referring to provider-owned external resource state.

### Native failure

Normalized failure data returned/thrown through adapter protocol, never raw
Perl `$!`, Rust `std::io::Error`, C `errno` or Windows error object.

## Declaration model

### Module native action

Selected universal primitive syntax:

```okkama
native read(handle is FileHandle, size is int) is @byte;
```

Decision: module-level native functions are the selected substrate syntax.
They are deliberately outside the object/action layer. Stdlib/application APIs
remain object/role based.

Properties:

- only at module scope initially;
- explicit typed parameters;
- explicit result where operation returns a value;
- semicolon instead of body;
- declaration owner module path/version is immutable metadata;
- no implicit `self`;
- ordinary import/take rules control source visibility;
- host implementation is mandatory when reachable/invoked.

Command-like native declarations without result may later omit result type, but
must have an explicit unit/undefined return contract in ABI; absence of Okkama
body is not absence of ABI result semantics.

### Native opaque object/type

Implemented syntax:

```okkama
native object FileHandle;
```

This declares a nominal language type but no fields/actions/constructors,
generic parameters or body. Only a provider can create valid values. User code
cannot forge, inspect, compare or erase it through `any`.

The provider creates an immutable logical `NativeValue::Handle` and keeps the
actual host resource in its `NativeHandleTable`. The language receives only an
opaque `Value::NativeHandle`. Identity consists of nominal type, provider,
authority, token and generation, but only the nominal type is visible to
source. Handles may cross exact typed parameters/results and typed array/map
values; they are forbidden as map keys.

`NativeRegistry` owns provider tables and permits only one active interpreter
at a time. Interpreter exit always drains all registered tables, including on
program failure. First close installs a tombstone before calling the finalizer;
later closes are no-ops, while other operations return normalized
`already_closed`. Host finalizer text and private identity never enter source
diagnostics.

Whether handles appear directly in application/stdlb API or remain behind
ordinary Okkama wrapper objects is a stdlib design choice. Recommendation:
low-level `runtime/*` declarations expose handles; high-level stdlib exposes
roles and wrapper objects with `destroy`.

### Native object members

Not required for first substrate. If later allowed, symbol identity must include
owner module + object + action + signature, and construction semantics must be
explicit. The removed ordinary-action override registry is not a native object
model and must not be reintroduced for this purpose.

## Module ownership

Native identity belongs to the declaring module, never the consumer's import
spelling.

Equivalent consumers:

```okkama
import "runtime/time" take monotonic_clock;
import "runtime/time";
```

must resolve the same provider symbol.

Import base directory, `.ok` suffix, local shadow path and basename namespace
are loader/source resolution details. They do not enter native ABI identity.

### Version policy

Recommendation:

- module major version participates in symbol identity;
- minor/patch describe source-compatible additions/fixes within major;
- provider descriptor declares supported ABI major;
- exact source signature fingerprint is validated;
- two major versions may coexist if module/import system later supports it.

The current module system does not resolve versions; Phase 01 only preserves and
validates declared major. Full version selection remains separate.

## Symbol structure

Logical fixed declaration key:

```text
module_path
module_abi_major
declaration_kind = function
declaration_name
fixed_arity
variadic = false
```

Future variadic declaration adds fixed prefix count and tail element descriptor.

Human-readable examples:

```text
runtime/time@1::monotonic_clock/0
runtime/fs@1::open/3
runtime/log@1::write/1+
```

Types need not make strings unwieldy, but descriptor fingerprint validation is
mandatory. Registering `open(string,int,int)->FileHandle` against a source
declaration with a different parameter/return shape fails before invocation.

## Link model

### Source declaration registration

Interpreter/compiler collects native declarations with provenance. Source
visibility remains governed by imports/namespaces.

### Provider registration

Embedding runtime constructs registry before program execution:

```text
core providers
optional approved providers
test-injected providers
```

Okkama source cannot name/load an arbitrary Perl module or shared library.

### Resolution timing

Two compatible modes are allowed:

1. eager link: validate every reachable native declaration at startup;
2. lazy link: resolve on first invocation with cached descriptor.

The current interpreter uses lazy resolution on invocation. Missing-symbol and
signature-mismatch diagnostics are deterministic and source-located. A future
compiler/runtime should prefer eager validation for deterministic startup
failure.

### Missing symbol

Native declaration has no Okkama fallback body:

```text
Native symbol "runtime/time@1::monotonic_clock/0" is not registered
```

Ordinary actions are unrelated: they always execute their Okkama bodies and
have no provider lookup or fallback convention.

## Invocation protocol

Logical flow:

```text
resolve source declaration
evaluate explicit arguments once in caller scope
validate Okkama declared types
adapt values to native ABI
resolve/validate provider descriptor
invoke handler
receive success or normalized failure
adapt success to Okkama value
validate declared return type
return to Okkama
```

Invariants:

- source-order exactly-once argument evaluation;
- provider cannot ask interpreter to re-evaluate AST;
- provider sees no caller environment;
- source token remains available for user-facing invocation diagnostics;
- private fields are not included unless explicitly passed as values by valid
  Okkama code;
- internal control wrappers cannot cross boundary;
- result validation is runtime-owned, not trusted to provider.

## Logical value ABI

The ABI is a conceptual tagged value model. Each runtime chooses memory layout.

### Primitive categories

```text
Int     signed range defined by language/runtime ABI
Float   floating-point contract defined separately
Rat     exact integer numerator + positive integer denominator
Byte    unsigned integer restricted to 0..255; distinct from Int
Bool
String  Unicode scalar sequence/encoding-independent language string
Char
Enum    nominal type id + case id
```

`Rat` transports numerator and denominator as decimal strings. This preserves
arbitrary precision without exposing a runtime-specific mutable big-number
object or coercing through `Float`. Numeric bit widths are not yet fully
specified by Okkama. System APIs that need exact widths must not assume Perl
integer range; integer-width decision is a prerequisite for production file
offsets, timestamps and process IDs.

### Collections

```text
Array<Value>
Map<Key, Value>
ByteSequence
```

Logical arrays/maps are immutable values at language boundary. Provider adapter
may borrow/read or copy according to runtime safety, but provider mutation must
not alter existing Okkama values.

The implemented M5 ABI uses explicit runtime-owned tags for every scalar and
enum. `Array` carries its element type and ordered logical values. `Map` carries
key/value types and a deterministic ordered list of key/value pairs rather than
exposing a Perl hash. Constructors and accessors defensively snapshot outer
storage; successful results are recursively validated and rebuilt as fresh
language-owned values.

`ByteSequence` is the specialized logical ABI form of a value crossing a
declared `@byte` boundary. It is not a second public language type. The declared
native signature selects this adapter; runtime inference alone cannot do so,
because an empty language array is valid for every `@T` boundary.

### Objects and roles

Ordinary Okkama objects should not automatically become generic provider
reflection records. Native declarations should prefer primitives, explicit
data objects with stable conversion descriptors, or handles.

Role values are runtime object values plus conformance information; providers
normally receive them only for capability injection/embedding APIs, not core
syscalls.

Current rule is stricter: ordinary object and role values cannot cross this
boundary at all. Future external resources cross only as explicit opaque
NativeHandle values; provider reflection over arbitrary objects is not an
implicit extension point.

### `any`

A top-level declared `any` is trusted-unboxed by the adapter into its concrete
logical tag before provider invocation. A concrete logical provider result may
re-enter through a declared `any` result and becomes an opaque language Any
value again. Nested `@any` and map values preserve each element's concrete type
through a private logical `Any(concrete_type, value)` envelope. Providers do
not receive a public accessor for the enclosed language/runtime value.

### Internal values forbidden at boundary

- lexical `Environment`/binding wrappers;
- AST nodes/tokens;
- `MustUseSuccessor`;
- `Varargs` wrapper itself;
- `VarargsExpansion`;
- loop-control exceptions;
- Perl callbacks/filehandles/references not represented by a logical category.

## Bytes

Byte I/O must not depend on text strings.

Chosen language-level representation:

```text
@byte = immutable array whose elements are builtin byte values 0..255
```

Required operations eventually exposed through stdlib/runtime helpers:

- length;
- bounded slicing;
- conversion from validated `@byte` if retained;
- text encode/decode with explicit encoding/error policy;
- equality/hash rules where appropriate.

`byte` is implemented as a distinct immutable runtime category with checked
`byte(value)` construction: numeric primitives are truncated toward zero,
then checked against `0..255`. `@byte(...)` builds an immutable array with
those same explicit conversions. There is no implicit narrowing or modulo
wrapping. Primitive call and method forms share `Runtime::Cast`; string
parsing and encoding remain separate APIs. Arithmetic promotes to ordinary
int/float/rat results.

### Logical byte descriptors

```text
Byte {
    value: integer 0..255
}

ByteSequence {
    length: non-negative integer
    octets: ordered sequence of Byte values
}
```

The logical descriptor does not prescribe Perl string, Rust slice, C pointer,
endianness-bearing word array or any other physical representation. A provider
ABI implementation may use contiguous packed storage, but its observable
meaning is exactly the ordered octet sequence and explicit length.

The Perl runtime stores `NativeValue::ByteSequence` as a packed immutable scalar.
Existing `length` and defensive `octets()` accessors remain available; `bytes()`
returns a scalar value for bundled provider use. Construction accepts either
validated `octets` or packed `bytes`, with an exact length, never both. Packed
input rejects references and Unicode scalars outside the octet range.

`ArrayValue` may retain packed byte storage; indexed access materializes one Byte,
while explicit `elements()` returns a new complete snapshot. Native byte adapters
and ByteMemory preserve packed storage without eagerly constructing VM Byte
objects. Ordinary arrays built from at least 64 actual Byte values also compact;
a declared byte tag cannot admit Int, Char, resource or other values. Generic
iteration and explicit snapshots may still materialize elements. Immutable scalar
sharing preserves independence from input replacement, exported-value edits and
later ByteMemory mutation/close. This changes physical storage, not logical tags,
capability transport or the Okkama syntax.

Scalar Byte and Int retain different ABI tags. An adapter must never narrow an
Int merely because its value happens to be in `0..255`.

### Input ownership

For a declared `@byte` argument the runtime:

1. evaluates the Okkama argument once;
2. validates the complete language value against `@byte`;
3. creates an ABI-owned immutable snapshot/view for this invocation;
4. preserves the original language array regardless of success or failure.

The provider may read the logical sequence for the duration of the call. It
must not mutate it or retain a borrowed view after the call returns. A provider
that needs longer ownership makes its own copy through its runtime API.

No provider receives the mutable host container underlying an Okkama array.

### Output ownership

A successful provider result transfers a logical ByteSequence to the runtime.
Before exposing it to Okkama, the runtime validates tag, length and every octet,
then creates fresh private language array storage containing Byte values.

The provider must not mutate transferred output afterward. A runtime may copy
or privately adopt physical storage, but it must make subsequent provider
mutation unobservable. Empty sequences and embedded zero octets are valid and
preserve exact length/order.

### Malformed provider boundary

These are provider/runtime contract violations, not recoverable native domain
failures:

- wrong logical tag for scalar Byte or ByteSequence;
- negative/inconsistent length;
- non-integer or out-of-range octet;
- provider mutation of borrowed input or transferred output;
- retaining a call-scoped borrowed view;
- raw host reference/string/slice escaping without adapter validation;
- adapter/provider exception that was not normalized as a declared native
  failure.

They produce an uncatchable provider/runtime error. Expected OS outcomes use
the separate normalized NativeFailure protocol; malformed provider data must
never be converted into a user-catchable byte/domain error.

Physical packed storage is therefore an implementation optimization, not a
second public Okkama type or an ownership escape hatch.

No Phase 01 clock work depends on this decision.

## Opaque handle architecture

### Language view

Handle has nominal native type and no inspectable fields:

```text
FileHandle
DirectoryHandle
ProcessHandle
SocketHandle
PollHandle
```

### Runtime view

Logical identity includes provider/runtime/kind/token/generation. Concrete host
resource remains in provider-owned table.

### Validation

Every operation checks:

- same runtime/provider;
- expected handle kind;
- token exists;
- generation was minted for this token; older generations are revoked;
- state permits operation.

### Close and lifetime

Implemented policy:

- high-level Okkama wrappers should expose explicit close and may invoke the
  same idempotent primitive from `destroy`;
- the first low-level close installs a tombstone before running the finalizer;
- aliases copy capability identity and never duplicate the host resource;
- repeated close is a successful no-op, while non-close operations return
  normalized `already_closed`;
- a failed finalizer is never retried because the underlying OS identity may
  already have been consumed or reused;
- registry shutdown finalizes all remaining open slots in deterministic table
  and acquisition order, continues after failures, and does not depend on host GC;
- one registry may be reused sequentially but cannot be owned concurrently by
  multiple interpreters.

Cancellation of outstanding future asynchronous operations remains part of
the later poll/async contract, not the handle identity model.

## Error ABI

Implemented logical provider failure:

```text
NativeFailure
    domain: stable provider/core domain
    code: stable code within domain
    message: diagnostic string
    diagnostic: optional immutable host diagnostic payload (not a language value)
```

Interpreter attaches public `operation` from the already resolved canonical
symbol; provider cannot spoof it. Optional trusted diagnostic data is retained
internally across sync/async conversion, wrapping, rethrow and secondary
failures. It adds no Okkama methods or reflective field access.

The filesystem provider records the failed filesystem `operation`, exact `path`
operand and `target_path` for two-path operations. Handle operations retain the
path used to open that handle; paths are not resolved to absolute paths or
recovered from the descriptor after a rename. The host `native_code` and
`native_reason` are optional diagnostics, not portable semantic identities.
Temporary creation failures may retain only the supplied parent directory
when the host library did not expose a generated child path. `cwd` has no path
operand. Validation errors need not have a host code or reason.

In the structured diagnostic model these facts appear under `details`, with
the canonical symbol retained as `native_symbol`. The public
`NativeFailure.operation()` remains that canonical symbol. Normal rendering
uses the same cause/stack/secondary traversal as other failures, escapes terminal
controls, renders valid UTF-8 paths as text and escapes undecodable path bytes.
The structured path itself retains the exact host operand. Rendering is bounded;
paths can contain sensitive information and are diagnostic output, not hidden
host/environment dumps.

### Portable core categories

At minimum:

```text
not_found
permission_denied
connection_refused
connection_reset
timed_out
address_in_use
address_not_available
network_unreachable
not_connected
broken_pipe
would_block
interrupted
invalid_input
invalid_data
message_too_large
unsupported
resource_exhausted
already_exists
already_closed
unexpected
```

### Mapping rule

Control flow branches on domain/code, never localized message or raw host code.

`runtime/network.udp_send` and `udp_connected_send` use `core.net/message_too_large` for their protocol
payload ceiling and host EMSGSIZE. `net/udp` translates it to DatagramTooLarge
with the sampled payload count. This operation-specific distinction does not
change the legacy generic errno mapping of EMSGSIZE to invalid_input.

`runtime/network.udp_connect` operates on an unpublished idle UDP state and
closes it on host failure or cancellation. The public acquisition request owns
that state before awaiting connect. Connected and unconnected packet operations
reject the other state mode; a connected send has no destination argument.
Fixed-peer filtering also discards foreign packets queued before connect and
normalizes IPv4/mapped IPv6 identity without rewriting public observations.

`runtime/network.multicast_join` creates a MembershipState for an existing
unconnected SocketState. First success pins socket-wide send settings; options
are snapshotted and rolled back before returning an acquisition failure. A
failed rollback or leave closes the parent, preserving the original failure.
Socket closure marks all memberships closed and lets the kernel release them;
individual close never repeats a leave. Memberships weakly reference native
socket state; the public MulticastMembership retains the UdpSocket owner using
the same allow-role composition as FileLock. No native handle getter is exposed.
Group/family/interface checks precede mutation; scoped numeric lookup validates
interface identity and Darwin/Linux SIOCGIFADDR reads its primary IPv4 address.

`runtime/network.unix_path_valid` validates filesystem address bytes without I/O.
The provider infers sockaddr_un path storage from two one-byte pack/unpack
probes, reserves a terminating NUL, rejects empty/NUL/oversized paths before
packing and verifies exact round-trip preservation. Host packing must not
silently truncate. Raw sockaddr stays inside the provider's private helper,
which future Unix bind/connect operations must reuse. Unsupported representation
is distinct from proof that the kernel supports socket acquisition.

`Core/Unix.pm` supplies actual nonblocking AF_UNIX streams through
`runtime/network`. PairState owns both unpublished SocketStates until public
wrapping succeeds; release tombstones that container and transfers ownership
to the two public connections. Failed wrapping closes both. ListenerState owns
the listening descriptor and captured path/device/inode. Its finalizer closes
readiness before guarded pathname cleanup; the parent directory must be protected
against concurrent replacement during acquisition/close (there is no atomic
POSIX conditional unlink). Accepted children own independent descriptors.
Read, write, connect and accept use readiness futures. One read and one write
may coexist; each direction rejects overlap. Reads retain EOF, use at most
65536-byte chunks and cancel without closing. Native write failure/cancellation
closes the socket because a prefix may have escaped; SIGPIPE is suppressed around
the host write. Connect verifies SO_ERROR and actual peer identity before
publication. Unix endpoint snapshots distinguish empty/unnamed from named paths.

### Language transport decision

Okkama has the typed recoverable-error model specified in
`docs/todo/typed-try-catch-13072026.md`. Fallible native APIs connect their
portable failure descriptors to this language channel:

- provider returns a normalized portable failure descriptor;
- runtime materializes a typed Okkama error value;
- that value travels through the catchable `panic`/`try`/`catch` channel;
- programming/runtime failures and malformed provider responses remain
  uncatchable.

The catch-visible builtin type is `NativeFailure`. It cannot be constructed in
source and exposes only immutable `.domain()`, `.code()`, `.operation()` and
`.message()` actions. A broad `catch ... is any` may handle it opaquely without
implicit access to the concrete value. Stdlib wrappers may translate stable
domain/code pairs into richer nominal errors.

Perl providers may use `Okkama::Runtime::NativeFailure::Errno` after capturing
`0 + $!` immediately. The helper accepts the number explicitly, never reads or
mutates ambient `$!`, maps characterized platform constants into the closed
portable taxonomy and maps unknown numbers to `unexpected`.

`core.fs` additionally builds an immutable scalar-only diagnostic payload. Its
normalizer obtains the reason from a localized copy of the saved numeric errno,
so intervening close/stat cleanup cannot replace the original code or reason.
No Perl exception object, address, stack or arbitrary metadata is retained.
Other backends may supply their own optional native code/reason. Programs must
branch on portable domain/category or typed stdlib errors, never on those host
details; neither numeric errno nor localized OS wording is normative.

Do not implement filesystem/network APIs with only fatal panic unless explicitly
accepted as a temporary non-production vertical.

## Provider interface

Provider registration is embedding/runtime API, not Okkama source API.

Logical pseudocode:

```text
registry.register(
    descriptor = {
        symbol,
        params,
        result,
        abi_major,
        provider_id,
    },
    handler,
)
```

Handler receives:

```text
NativeCallContext
    symbol
    args: ordered logical native values
```

The Phase 01 context is implemented as an immutable value with defensive
argument snapshots. It does not receive the interpreter, AST, caller
environment, receiver or private object state. Narrow adapter/handle/error
services and cancellation/deadline data may be added only when their portable
contracts exist. Interpreter, receiver and AST nodes must never be added to
the portable provider ABI.

### Provider loading

Core providers are assembled by runtime embedding code. Optional provider
loading requires policy/manifest/signature/version design and is not triggered
by arbitrary source strings.

The complete `runtime/time` provider family is registered as:

```text
runtime/time@1::wall_clock/0      [] -> int
runtime/time@1::monotonic_clock/0 [] -> int
runtime/time@1::sleep/1           [int] -> bool
runtime/time@1::timer_create/2    [int,int] -> TimerHandle
runtime/time@1::timer_wait/1      [TimerHandle] -> bool
runtime/time@1::timer_cancel/1    [TimerHandle] -> bool
runtime/time@1::timer_cancelled/1 [TimerHandle] -> bool
runtime/time@1::timer_deadline/1  [TimerHandle] -> int
runtime/time@1::timer_close/1     [TimerHandle] -> bool
```

Every value is integer nanoseconds. Wall clock uses the Unix epoch and may be
adjusted by the host. Monotonic time has an unspecified local origin, is used
only for ordering/differences, and never falls back to wall time. Sleep takes a
relative nonnegative duration, loops against a monotonic deadline across host
interruptions and may oversleep under scheduler load. Required clock/sleep
capabilities fail registration explicitly when unavailable.

`TimerHandle` is an authority-checked owned capability. Creation takes a
nonnegative relative delay and an interval: zero interval is one-shot, positive
interval is periodic. The private resource retains cancellation, completion
and next-deadline state. Periodic deadlines stay anchored to the original
fixed-rate schedule and advance past missed ticks without burst replay. Close
is idempotent; high-level destruction is the normal owner and registry shutdown
closes leaks. Cancellation does not interrupt a host sleep already executing
in the current synchronous interpreter, but prevents every later wait.

The public value layer is the pure-Okkama `time/duration` module. It represents
signed spans in the exact signed-64 nanosecond range and performs checked
addition, subtraction, integer scaling and named-unit projection without a
native call. Negative spans remain valid values; the `time/clock` adapter owns
the narrower nonnegative precondition at the `runtime/time.sleep` boundary.
`InvalidDuration` is a representation-overflow error, never a native failure.

Above that boundary, `time/instant` makes system/monotonic identity nominal,
`time/clock` exposes clock objects and the relative-sleep precondition, and
`time/date` performs pure Gregorian UTC projection. `time/timer` separates
state-only schedule constructors from acquisition projections and wraps
`TimerHandle` as a live object. Its high-level `cancel()` is a command and
`cancelled()` is the query; raw ABI transition booleans do not leak.

The implemented `core.memory` provider registers declaration-only
`runtime/memory`:

```text
runtime/memory@1::byte_memory_create/1  [@byte] -> ByteMemoryHandle
runtime/memory@1::byte_memory_bytes/1   [ByteMemoryHandle] -> @byte
runtime/memory@1::byte_memory_size/1    [ByteMemoryHandle] -> int
runtime/memory@1::byte_memory_consume/2 [ByteMemoryHandle,int] -> @byte
runtime/memory@1::byte_memory_append/2  [ByteMemoryHandle,@byte] -> int
runtime/memory@1::byte_memory_reset/1   [ByteMemoryHandle] -> bool
runtime/memory@1::byte_memory_closed/1  [ByteMemoryHandle] -> bool
runtime/memory@1::byte_memory_close/1   [ByteMemoryHandle] -> bool
```

This is generic owned byte state, not an I/O buffering provider. Its private
authority table retains exact bytes plus lifecycle only; close clears the
payload and is idempotent. Line splitting, UTF-8 conversion, cache capacity,
prefetch, flush, partial-write recovery, and wrapped-stream ownership belong to
the pure-Okkama `io/buffer` adapter.

The implemented `core.fs` provider owns a private `NativeHandleTable` and the
declarations in `stdlib/runtime/fs.ok`:

```text
runtime/fs@1::path_encode/1 string -> @byte
runtime/fs@1::path_decode/1 @byte -> string
runtime/fs@1::open/3   @byte,int,int -> FileHandle
runtime/fs@1::read/2   FileHandle,int -> @byte
runtime/fs@1::write/2  FileHandle,@byte -> int
runtime/fs@1::seek/3   FileHandle,int,int -> int
runtime/fs@1::flush/1  FileHandle -> bool
runtime/fs@1::truncate/2 FileHandle,int -> bool
runtime/fs@1::sync/1   FileHandle -> bool
runtime/fs@1::lock_shared/1 FileHandle -> bool
runtime/fs@1::lock_exclusive/1 FileHandle -> bool
runtime/fs@1::unlock/1 FileHandle -> bool
runtime/fs@1::close/1  FileHandle -> bool
runtime/fs@1::stat/1   @byte -> %string:int
runtime/fs@1::lstat/1  @byte -> %string:int
runtime/fs@1::opendir/1 @byte -> DirectoryHandle
runtime/fs@1::readdir/1 DirectoryHandle -> @byte
runtime/fs@1::rewinddir/1 DirectoryHandle -> bool
runtime/fs@1::closedir/1 DirectoryHandle -> bool
```

It performs one host read/write attempt, exposes partial progress, keeps file
storage binary, maps captured errno immediately, and never returns a Perl
filehandle or fd. `flush` covers runtime buffering; `sync` requests durable
storage. Truncation rejects negative sizes. Shared/exclusive locks are blocking
whole-file advisory locks and `unlock` releases the current lock.
Filesystem path identity remains byte-preserving. UTF-8 conversion is explicit
and fallible, and all metadata/mutation/directory operations consume the same
`@byte` representation. File and directory capabilities share one provider
authority table while retaining exact nominal type checks.

The public `fs/path` adapter is deliberately not another provider. Its
state-only `Path`, `PosixPath`, and `WindowsPath` objects implement lexical
join, normalization, parent and root classification in Okkama. `Path` selects
grammar through the public `Host` object when observed; explicit foreign paths
do not inspect the host. Text-originated paths project exact UTF-8 bytes, while
raw non-UTF-8 names returned by `runtime/fs.readdir` stay byte values and are
not forced through this text API. No lexical path action calls stat, resolves
links, changes cwd or otherwise races filesystem state.

`chdir` necessarily changes process-global cwd. It is supported by the default
single-runtime CLI provider; concurrent embeddings must serialize it or expose
it as `unsupported`. `umask` remains absent until a broader shared-process-state
policy exists.

The implemented `core.process` provider registers declaration-only
`runtime/process`: opaque PipeHandle and ProcessHandle capabilities, one-attempt
byte pipe reads/writes, half-close, shell-free byte argv spawn, wait/try-wait,
portable signal codes and process identity. `spawn_configured` additionally
sets the child working directory and replaces the child environment from
explicit raw-byte name/value pairs without mutating the parent. Empty or
NUL-containing directories, malformed pairs, invalid environment names and
duplicate names are rejected before child execution. Three distinct pipes explicitly
become child stdin/stdout/stderr; every other provider descriptor is
close-on-exec. An internal fork/exec backend is a Perl provider detail and
never public ABI. Exec failure crosses the setup handshake as a spawn failure,
not exit 127. Close or runtime shutdown force-terminates and reaps an owned
running child, preventing zombies.

The implemented `core.net` provider registers declaration-only `runtime/net`.
IP endpoints and received datagrams use versioned immutable byte envelopes,
never host sockaddr memory. The provider implements IPv4/IPv6 address
parse/format, forward/reverse DNS, blocking stream client/server operations,
datagrams, shutdown, local/peer inspection and a closed portable boolean option
set (reuse-address, keepalive, TCP no-delay). Forward resolution accepts
portable type `0` for transport-neutral DNS, `1` for stream and `2` for
datagram; `net/dns` uses zero so a name lookup cannot choose socket policy.
SocketHandle owns a binary,
close-on-exec descriptor in its authority table. Send/recv expose one-attempt
partial progress; SIGPIPE and expected DNS/errno outcomes are normalized.
The implemented `core.poll` provider registers declaration-only
`runtime/poll`. One opaque PollHandle observes exact FileHandle, PipeHandle and
SocketHandle capabilities through registry-owned provider adapters without
exposing host descriptors. Readable/writable masks, stable registration ids,
integer tags and deterministic event maps form the portable result ABI.
`poll_wait` consumes absolute `monotonic_clock()` nanosecond deadlines; `-1`
means unbounded wait. Host interruption is retried against that deadline while
explicit `poll_cancel` produces one consumed catchable `interrupted` failure.
The initial IO::Select backend is provider-private and can be replaced without
changing declarations. Raw nonblocking/duplication controls remain separate
future work.

The implemented `core.environment` provider exposes raw byte names/values and
never assumes Unix environment data is Unicode. Missing and empty remain
distinct; `environment_all` is a byte-sorted array of name/value pairs because
native maps cannot preserve byte keys. Set/remove mutate the ambient host
process and therefore require serialization in concurrent embeddings.

The public `env/var` adapter keeps this substrate private behind the
`Environment` role. `EnvVar` is a nominal strict-UTF-8 name validated on
observation; `EnvEntry` is a detached raw-byte snapshot. Raw access never
decodes, while text access strictly decodes and raises `InvalidEnvEncoding`.
Expected native failures are translated to domain errors and unexpected
provider details are sanitized. `Environment.fake()` owns immutable
instance-local state, preserves byte ordering and performs no environment ABI
call.

`core.process` additionally snapshots explicitly injected application argv as
`@@byte` and exposes current/parent integer identity. The CLI excludes the
source path and forwards only following arguments. `core.system` projects
closed portable platform/architecture strings and exact OS CSPRNG bytes. It
never exposes Config archname or falls back to Perl `rand`; unavailable secure
entropy is an explicit failure.

The public `proc/args` adapter is stateless over that provider-owned snapshot.
Its raw projection preserves exact bytes; its complete and indexed text
projections use strict UTF-8 and attach the argument index to domain failures.
Out-of-bounds access is a separate `MissingArgument`. The adapter never
invents argv entries, chooses a locale encoding or exposes provider failure
details.

The public `random` adapter does not add another host boundary. `Random.bytes`
delegates to the sanitized `crypto/random` capability; its integer, boolean
and float actions are Okkama composition over those bytes. Integer sampling
uses rejection over an overflow-safe unsigned two-word representation, so the
whole signed-64 domain remains inclusive and unbiased. `SeededRandom` is a
pure, explicitly non-cryptographic Okkama source with no native call or hidden
cursor. `Choice` and `Shuffled` retain an injected `RandomSource`, keeping
domain selection testable without replacing the production CSPRNG.

Large seeded byte values also exercise ownership reachability. That graph walk
is iterative post-order in the interpreter; it preserves deterministic child
before owner ordering without tying valid persistent-object depth to Perl's
call stack.

The implemented `core.regex` provider owns opaque `RegexHandle` programs for
the seven declaration-only `runtime/regex` operations. Its parser accepts only
the documented portable Unicode dialect and compiles a Thompson NFA with a
deduplicated active-state set; it never calls Perl's regex engine. Pattern,
state, input and repetition limits are enforced before unbounded resource use.
Malformed syntax and limits cross the existing normalized failure transport.
`invalid_offset` projects only the exact source offset (`-1` for valid syntax),
while `spans` returns the leftmost-longest full match followed by capture
groups in source order. `all_spans` repeats the same shape for ordered,
non-overlapping matches and advances safely after zero-length acceptance. All
offsets count Unicode code points; an unmatched group is `start = -1, end =
-1`, and a repeated capture records its last participating iteration.

The implemented `core.format` provider is the privileged boundary that may
render opaque `any` payloads. It supplies plain concatenation for standard
streams, positional `{}` fields, named `{identifier}` fields, and `{{`/`}}`
escapes. The three operations have distinct native names because native source
declarations do not overload.

The implemented `core.crypto` provider accepts exactly `sha256`, `sha384`, and
`sha512`. Digests and HMAC preserve byte identity. `secure_equal` visits every
position up to the longer input and includes the length difference in the
result, avoiding an early content-dependent return.

The implemented `core.tls` provider consumes a connected stream `SocketHandle`
and transfers ownership to an opaque `TlsHandle`; the source identity becomes
a closed tombstone. Client sessions require certificate-chain and server-name
verification. Server sessions accept explicit PEM certificate/key bytes.
Handshake is explicit and idempotent, reads/writes carry only encrypted-session
bytes, and the fixed peer projection exposes subject, issuer, Unix validity
bounds and leaf DER. Close and runtime shutdown own session teardown. The Perl
runtime declares `IO::Socket::SSL` and `Net::SSLeay` as dependencies; when the
engine is unavailable the stable declarations fail as `unsupported`.

## Core versus optional providers

### Core/universal substrate

- time;
- secure random;
- byte I/O;
- filesystem/directories;
- processes/pipes;
- portable regular expressions;
- value formatting;
- SHA-2 digest/HMAC primitives;
- sockets/DNS;
- TLS;
- polling;
- environment/process info.

Runtime may report a core operation unsupported on a platform when genuinely
unavailable; capability discovery rules must be explicit.

### Optional providers

- SQLite;
- PostgreSQL/libpq;
- MySQL client;
- curl;
- OS/platform-specialized unsafe operations.

High-level HTTP/DB APIs live above provider-specific primitives and shared
stdlib roles where possible.

## Stdlib layering

Recommended dependency direction:

```text
runtime/* native declarations
    -> sys/* minimal Okkama adapters
    -> io/fs/net/process/time roles and immutable wrappers
    -> text/buffering/protocol/domain libraries
    -> application
```

Examples:

- native `read(handle, size) -> @byte`;
- stdlib `Read.bytes(size)` returning immutable `@byte`;
- buffered decorator;
- decoder policy;
- `TextFile.line(number)`.

Native substrate must not add `read_text_file` merely for convenience.

`runtime/*` modules contain only low-level native declarations and opaque native
types. They do not mix those declarations with ordinary Okkama wrappers.
Wrappers/roles/policies live in separate stdlib modules. A future trust policy
may restrict direct application imports of `runtime/*`; this is deferred and is
not enforced by naming alone.

## Immutability and external effects

Okkama object can keep immutable handle identity while commands mutate external
resource state:

```okkama
writer.write(bytes); // `bytes` is a local variable of type @byte
writer.flush();
writer.close();
```

If object-local state changes (buffer position/cache/closed flag represented in
Okkama fields), it requires successor action. Provider-internal resource state
must still respect documented operation/lifecycle contract; native layer is not
a way to mutate private Okkama fields.

## Standard runtime inputs for `main`

`main` is not a general capability-adoption/dependency-injection boundary. If
parameters are introduced, they are restricted to standardized process entry
inputs: stdin, stdout, stderr and command-line arguments.

Possible deferred entry boundary:

```okkama
main(input is io.Read, output is io.Write, errors is io.Write, args is @string) is void {
    ...
}
```

Embedding runtime provides this fixed ordered standard set. Wiring uses position
and declared types/roles, never parameter names. Application code cannot request
arbitrary services by extending the parameter list.

Required startup flow:

```text
parse main signature
obtain standardized entry values from runtime
check count
check each concrete type/role
bind parameters in main root
assign ownership policy
execute main
destroy/release according to policy
```

This is not equivalent to native symbol lookup: capabilities are ordinary
runtime object values supplied to application entry.

## Fake/provider testing

Deterministic tests should inject provider registries, clocks, byte sinks and
handle tables. This does not require privileged source-level mock access.

Role fakes remain part of the existing test framework. Native architecture does
not change their semantics. Provider tests use injected registries/handlers;
Okkama stdlib tests may use role fakes under their existing test-mode rules.

## Varargs

Existing declaration varargs evaluation/type guarantees can be reused by native
declarations later:

- one final typed tail;
- left-to-right exactly once;
- stable selected declaration;
- separate fixed/tail logical arrays if provider ABI needs them.

Call-site spread is separate syntax and does not belong to core native linker.
Provider ABI always receives already evaluated ordered values.

## Security boundary

- Source can call only declared native symbols.
- Declaration can resolve only registered matching descriptor.
- Registry cannot be mutated by ordinary Okkama code.
- Provider receives only declared arguments and narrow runtime services.
- Handles are unforgeable and validated.
- Native calls do not bypass imports, type checks, roles or visibility.
- Optional provider availability is embedding policy.
- Debug host details are not stable user-facing error identity.
- No native symbol accepts host package/function name supplied by source.

## Concurrency and reentrancy

Future runtime/provider contract must specify:

- whether handlers may block;
- thread affinity of handles;
- reentrant calls into Okkama (recommendation: forbidden initially);
- cancellation/deadline propagation beyond the implemented synchronous poll
  boundary;
- async scheduler integration above the implemented portable poll substrate;
- callback lifetime and runtime shutdown.

Initial providers are synchronous and must not retain references to transient
argument/context objects after return unless a handle API explicitly owns data.

## Removed object-action override mechanism

The former optional `Object.action/N` and `Object.action/N+` host overrides
have been removed. Ordinary object actions always execute their Okkama bodies;
they are never interpreted as native declarations by registry convention.

Every host implementation must now have an explicit bodyless module-native
declaration, a canonical module-qualified symbol and a matching mandatory
provider descriptor. The old registry keys, interpreter dispatch branch,
panic-stub stdlib objects and their host providers must not be restored.

## Cross-runtime conformance

Future Perl/Rust/C runtimes should share test vectors for:

- canonical symbol construction;
- descriptor signature fingerprints;
- primitive/value conversion;
- byte sequences;
- normalized error domains/codes;
- handle wrong-kind/stale/closed behavior;
- clock units and monotonicity;
- partial I/O;
- path/environment encoding edge cases;
- provider missing/duplicate/mismatch diagnostics.

Tests should assert logical results, not Perl class names or Rust enum debug
strings.

## Architecture decisions still open

Typed try/catch, normalized provider failures, byte adaptation and opaque
handle lifecycle are complete. Remaining decisions belong to later concrete
system-provider milestones:

1. Integer widths and overflow contract.
2. Platform path/environment string encoding.
3. Outstanding async operation cancellation during handle close/shutdown.
4. Exact fixed standard main stream/argument contract, if implemented.
5. Optional provider loading/security/manifest format.
6. Future policy restricting direct application imports of `runtime/*`.

## Architecture acceptance

- Source declarations and host implementations are separately represented.
- Module/version/signature define symbol identity.
- ABI does not expose Perl-specific values or exceptions.
- Errors and handles are normalized/opaque.
- `@byte` and text are separate layers; providers may pack bytes internally.
- Stdlib owns high-level policies.
- Optional providers extend without entering universal substrate.
- Optional main inputs are limited to standardized positional streams/arguments.
- Existing runtime can migrate incrementally.
- Future Rust/C implementations can follow this document without reproducing
  Perl internals.

## Debug value provider

The mandatory declaration is:

```text
runtime/debug@1::data_dump/1 [any] -> string
runtime/debug@1::data_kind/1 [any] -> string
runtime/debug@1::data_type/1 [any] -> string
runtime/debug@1::data_size/1 [any] -> int
runtime/debug@1::data_leaf/1 [any] -> string
runtime/debug@1::data_names/1 [any] -> @string
runtime/debug@1::data_keys/1 [any] -> @any
runtime/debug@1::data_items/1 [any] -> @any
```

`core.debug` formats portable logical scalars, enums, byte sequences, typed
arrays and typed maps into compact deterministic Okkama-like text. It is a
diagnostic boundary, not serialization. The high-level `okkama.Dump` object is
ordinary Okkama code over this primitive. Objects and handles remain forbidden
at the `any` adapter boundary, and no universal `.dump()` action is introduced.

## Source stack metadata (runtime update)

Runtime diagnostics retain Okkama source frames, never the provider's Perl call
stack. The interpreter records the native declaration and the Okkama call site
before invoking a provider. When a native result is translated to a typed
`NativeFailure`, its private thrown carrier captures that context. Delayed native
completions restore the registration context before translating their result.
The existing provider ABI and typed Error value are unchanged.

Host tooling reads `stack_trace()` on a programming exception, panic carrier or
throw carrier. Each frame carries name, kind, module, execution location,
declaration location and call-site location. See the diagnostics section of
[the specification](../okkama-spec-11072026.md#20-diagnostics) for the schema.
Rethrow and cleanup aggregation preserve the original trace; secondary failures
retain their own snapshots. No stack data becomes a user-visible Okkama value.

### Typed failure causes across application handlers

An application Error escaping a selected catch retains the handled `Thrown`
carrier in its `cause()` metadata, including native failures. The inner
`native_cause` still points to the exact normalized native Error value; it is
not repurposed as the application cause link. Source trace and secondary
cleanup failures remain attached to their original carriers. Async handler
completion applies the same rule after suspension. Native programming failures
and panic do not become ordinary Error causes. Host tooling traverses `cause()`
without reflecting over application objects; this does not add methods to the
language's `Error` role. The existing process-worker string transport does not
transport this structured chain across a process boundary.

### Bounded slot reuse (14 October 2026)

Closed slots are recycled only within the same nominal handle type. Reuse
increments generation; old aliases remain closed and cannot access the new
resource. Future, unminted generations are rejected. Slots are bounded by the
high-water concurrent acquisitions per nominal type, rather than lifetime
acquisition count; this is not an exact host-heap byte budget.

Immutable arrays/maps retain a resource-free certificate computed from actual
values after defensive construction. Ownership traversal may skip certified
primitive graphs; declared type tags alone are not evidence. Unknown objects,
roles, actions, Futures and handles remain conservative. Array cursors retain
their origin in the ownership graph, including escaping resource elements.

## Production worker diagnostic wire

Spawn protocol 1 failure responses may include `diagnostic_version=1` and a
plain detached model. The parent keeps child category/type/source/stack/cause
and cleanup secondaries, and appends parent cleanup failures. Internal failure
identity survives local envelope copies, so one failure is not repeated; equal
text from different workers remains separate. Legacy text responses remain
accepted. This is diagnostic metadata, not application Error serialization or
permission to catch RuntimeError/Panic as Error.

The wire profile caps depth/nodes/items/strings and encoded model bytes; a worker
that exceeds it returns WorkerDiagnosticLimit. It does not bound arbitrary worker
stdout/stderr. Exact quotas and tests.

## Channel resource dependencies

Queue leases retain actual object graphs with data-only pin records. VM scope
cleanup postpones pinned resources; delivery/return transfers retention and the
last live owner performs exactly-once destroy. Capability retirement drains
unread buffers only when all capabilities finish; explicit close still preserves
buffered delivery. Weak active-count registries compact released lease/channel
slots. Language cleanup completes before native teardown, preserving native
handle close authority and generation checks. Contract and evidence.
