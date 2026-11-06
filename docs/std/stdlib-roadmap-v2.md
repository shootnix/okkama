# Okkama Standard Library — EO-first TODO (v2)

> Python = coverage. Go = package boundaries. Elegant Objects = API shape.
> Work top-to-bottom. `[ ]` -> `[x]`.
> v2, 08.08.2026 — supersedes `stdlib-roadmap.md`. Changes are marked with `> v2:` notes per stage.

## Changelog v1 -> v2

**Syntax fixes (examples now match the real grammar and shipped code):**
- `enum X { a; b; }` -> `enum X = (a, b);` (grammar `enum_decl`, `std/compare.ok`).
- callback params `action(A) is B` -> real action types `(A) is B` / `async (A) is B`.
- postfix `... is T async` -> prefix `async name(...) is T` (grammar `action_decl`, `stdlib/http/handler.ok`).
- `#` comments in samples -> `//` (`#` is the attribute marker, e.g. `#projected`).
- `Comparing.compare()` -> `compared()` everywhere (matches shipped `std/compare.ok`).
- `role Error` synced with shipped `std/error.ok` (`text()` + `code()`).

**Structural:**
- Added missing `role Projecting<T>` (Stage 01) — it was used ~80 times but never declared.
- New global rule: **total attribute -> plain method; partial/fallible or computed -> Projecting object.** Collapsed `Year`/`Month`/`Day`/`WeekdayOf` into `Date` methods, `PathName`/`PathStem`/`PathSuffix` into `Path` methods. Kept `UrlPort` (partial), `AbsolutePath`/`ResolvedPath` (I/O) as objects.
- New global rule: **resource views** (Random, clocks, File.stat, Environment, metrics) may answer differently over time; identity stays immutable, no setters.
- New global rule: **native bulk paths** for concrete collections — contracts stay decorator-shaped, hot paths must not force per-element Okkama dispatch (measured ~780µs/element per closure call; see `docs/todo/` perf records).
- **Live sources are not `Iterating`**: `FileWatch`, `TcpEndpoint`, `UnixEndpoint`, `Completed` now expose explicit `async` methods instead of a blocking `Cursor`.
- HTTP `Responding` restored to the battle-tested `(Request, Response)` signature (25.07.2026 migration, old stdlib).
- Stage 23: concrete `Sqlite`/`MemorySqlite` engines marked **deferred** (explicit decision of 22.07.2026, `docs/todo/db-sqlite-22072026.md`); contracts stay in 1.0.
- Role renames per the recorded best `-ing` candidates: `List` -> `Linking`, `Set` -> `Containing`, `Database` -> `Querying`, `Metric` -> `Measuring`. `Error`, `Image`, `Audio`, `ArchiveEntry` deliberately keep nouns (no gerund preserves semantics; candidates stay recorded).
- `Heap` no longer pretends to be `Sequencing` (heap-order `at()` leaked internals); now `first()`/`removed()`/`Sizing`.
- Name collisions resolved: Stage 30 `Responding` -> `ModuleResource`; Stage 32 `Span` -> `SourceSpan`; regex `Span` defined locally.
- Secret material rule: private keys expose no `bytes()`/`text()`; export only via explicit objects (`Pkcs8`).

**v2.2 (08.08.2026):** representation roles killed and parsing rehomed.
- `repr/text` and `repr/byte` REMOVED entirely: text is `string`, binary is `@byte` — native types, not roles (`"hello".text()` is absurd, and every role layer costs measurable interpreter time). All former `TextRepresenting` inputs are now `string`, all former `ByteRepresenting` inputs are now `@byte`; render/binary objects keep plain `text()`/`bytes()` methods with no role behind them. New **values are values** global rule.
- `strings/integer`/`strings/float`/`strings/boolean` REMOVED — the path couldn't decide if it was about strings or numbers. Parsing is not a runtime type conversion (so not a Stage 00 builtin either) — it is real work with real failure modes; it now lives in a top-level `parsing` module (next to `compare`/`size`/`error`) as `ParsedInt`/`ParsedFloat`/`ParsedBool`. Domain parsers stay in their domains (`DateFromIso` in time/iso, `Json` in encoding/json).
- `data/limited`/`data/slice`/`data/concatenated` REMOVED — once binary is just `@byte`, limiting/slicing/concatenation are array operations: new Stage 00 builtins `@T.slice`, `@T.concatenated`.
- `strings/concatenated` REMOVED (`Joined("", parts)` covers it); `encoding/json.JsonData` REMOVED (`Utf8(doc.json())` covers it).
- `%string:TextRepresenting` fields (Record, Template) are now plain `%string:string`.

**Removed:** `text/tokens` (no consumer, no methods; Python has none either), `data/buffered` (buffering a value-semantics `bytes()` is a no-op; wrapper layers measurably *slowed* I/O — see perf record), `Caching` role (mutable `put`, contradicts core rules; moved to Stage 35), `Encoding` parameter on `TextFile`/`TextProcess` (UTF-8 default; other encodings by composition `Utf8Text(DataFile(f))` — no codecs registry in 1.0).

**Added:** `string.pad_left`/`pad_right`, `Complex.text()`, `RoundedTo`, generic `MinOf`/`MaxOf`, `enum Level`, `Status`/`Method` sketches, `SmtpCredentials`, `MailHeaders`, crypto key objects, minimal `TomlDocument`/`IniDocument`/`XmlNode` sketches, `Random` seed-determinism contract, `Stdout.print`/`println` (role-overload ambiguity), DoD item for Perl-level regression tests.

**v2.1 (same day):** `IntegerText`/`FloatText`/`BooleanText` -> `ParsedInt`/`ParsedFloat`/`ParsedBool` — the `-Text` suffix means the render direction everywhere else, so these names claimed the opposite of what they do; new **direction naming** global rule fixes the vocabulary per direction.

---

## Global rules

- [ ] No `Reader`, `Writer`, `Parser`, `Encoder`, `Decoder`, `Formatter`, `Manager`, `Controller`, `Handler`, `Helper`, `Builder`, `Factory`, `Validator`, `Router`, `Dispatcher`, `Listener`, `Observer`, `Client`.
- [ ] Name objects by what they **are**, not by what they **do**.
- [ ] Role names prefer natural `-ing` forms; if no gerund preserves the semantics, keep the noun and record ordered `-ing` candidates beside the role.
- [ ] Prefer declarative/decorator objects: `DataFile`, `Sorted<T>`, `Filtered<T>`, `Gzip`, `Base64`, `Json`, `PrettyJson`, `RemoteResponse`.
- [ ] No utility/service objects whose identity is "performs operation X".
- [ ] No static/free utility functions where an object can represent the result.
- [ ] Immutable objects.
- [ ] Constructors bind only; no I/O/computation in constructors.
- [ ] No `null`, generic optional, setters, `with_*`, implementation inheritance.
- [ ] Typed errors; fail fast.
- [ ] Small contracts and small public surfaces.
- [ ] Effects belong to real resources/entities (`Stdout`, `File`, `Process`, `TcpConnection`, `Logging`), not worker objects.
- [ ] Use decorators instead of flags/setters where practical.
- [ ] Tests use fakes, not mocks.

> v2: new rules below.

- [ ] **Attribute access rule.** A *total* attribute (always present: `date.year()`, `url.host()`, `path.name()`) is a plain method. A *partial* attribute (may be absent — no `null`, so absence must be a typed error: `UrlPort`) or a *computed/effectful* projection (`AbsolutePath`, `WeekdayOf`-style calculations, anything doing I/O) is a separate `Projecting` object. This also explains the map-like case: open key sets (`Headers.value(name)`, `Row.value(column)`) are methods because the key is caller data, not API surface.
- [ ] **Resource views.** Objects representing live resources (`Random`, `SystemClock`, `File`, `Environment`, `Disk`, `Process`, sockets, metrics instruments) may answer differently over time; that is not mutation of the object's identity. They still forbid setters and still bind-only in constructors. Everything that is a *value* stays strictly immutable.
- [ ] **Native bulk paths.** Contracts stay decorator-shaped (`Mapped`, `Filtered`, `Sorted`), but concrete collections must route whole-collection operations through native runtime primitives (`string.split`, native sort, native slice) rather than forcing one Okkama-level dispatch per element. Measured on this interpreter: ~780µs per element per closure call; a decorator chain multiplies that per layer. Benchmark A/B before and after; see `docs/todo/` perf records (http/server case).
- [ ] **Live sources are not `Iterating`.** `Cursor.next()` is synchronous. Anything that *waits* for the outside world (socket accept, fs events, task completion) exposes explicit `async` methods instead of implementing `Iterating`.
- [ ] **Values are values (v2.2).** Text is `string`, binary is `@byte` — native types, not roles. There is no `TextRepresenting`/`ByteRepresenting`: an API that needs text takes `string`, one that needs binary takes `@byte`; result objects expose plain `text() is string` / `bytes() is @byte` methods. `"hello".text()` must never exist. Composition pays one explicit `.text()`/`.bytes()` per layer — honest, visible, and cheaper than role dispatch through interpreter layers.
- [ ] **Secret material.** Private-key objects expose neither `bytes()` nor `text()` at all — no ambient way to log a secret. Export is a deliberate act via explicit objects (`Pkcs8(key).bytes()`).
- [ ] **Direction naming.** A conversion object's name states its *result*, and each direction has a fixed vocabulary. Render: `-Text`/`-Data` suffix means the result is text/bytes (`TomlText`, `PlistData`, `Utf8Text`). Parse: `Parsed*` when the source format is the obvious/only one (`ParsedInt`, `ParsedMail`), `*From<Format>` when the format must be named (`DateFromIso`, `DateTimeFromRfc3339`), a bare noun when the parsed thing has its own strong identity (`Json`, `Toml`, `PemBlock`). Pure inverse decode: `Un*` (`Unhexed`, `Unbase64`, `Untarred`). Never use a suffix of one direction for the other.
- [ ] **Sample discipline.** Code samples in this document use the real grammar: `enum X = (a, b);`, action types `(A) is B`, prefix `async`, `//` comments.
- [ ] **Stdlib-unique names.** Public type names should be unique across the whole stdlib even though modules namespace them (`Span`, `Responding` collisions of v1 are resolved below).

---

# Stage 00 — language substrate

## - [x] builtin/Error

> v2: synced with shipped `std/error.ok` — the role has `code()` too, and five
> concrete errors already exist (`InvalidArgument`, `InvalidState`, `OutOfRange`,
> `UnsupportedOperation`, `NotImplemented`).
> naming candidates, best -> worst: Failing, Faulting, Erring — noun kept
> deliberately: `Error` is what the thing *is*.

```okkama
role Error {
    text() is string;
    code() is int;
}
```

## - [ ] builtin conversions

```okkama
int.string() is string
int.float() is float
int.byte() is byte        // throws OutOfRange
float.string() is string
float.int() is int        // throws OutOfRange
byte.int() is int
byte.char() is char
char.string() is string
char.byte() is byte       // throws OutOfRange (non-ASCII)
bool.string() is string
```

## - [ ] builtin arrays/maps

> v2.2: `slice`/`concatenated` added — with `ByteRepresenting` gone, limiting,
> slicing and joining binary data are plain array operations (they replaced
> the `data/limited`, `data/slice`, `data/concatenated` objects).

```okkama
@T.size() is int
@T.empty() is bool
@T.at(index is int) is T          // throws OutOfRange
@T.slice(start is int, end is int) is @T   // throws OutOfRange
@T.concatenated(other is @T) is @T

%K:V.size() is int
%K:V.empty() is bool
%K:V.contains(key is K) is bool
%K:V.at(key is K) is V            // throws MissingKey
%K:V.keys() is @K
%K:V.values() is @V
```

---

# Stage 01 — tiny contracts

## - [x] core/compare

> v2: shipped as `std/compare.ok` — the action is `compared`, not `compare`.

```okkama
enum Ordering = (less, equal, greater);

role Comparing<T> {
    compared(other is T) is Ordering;
}
```

## - [ ] core/projecting

> v2: NEW. This role was referenced ~80 times in v1 but never declared. It is
> the single most reused contract in the whole plan: every "computed value as
> an object" (`ParsedInt`, `AbsInt`, `Median`, `BinarySearch`, `AbsolutePath`,
> `RemoteResponse`, ...) does it. Implementations mark `value()` with
> `#projected` where the language's one-projection-per-object rule applies.

```okkama
role Projecting<T> {
    value() is T;
}
```

## - [x] core/size

```okkama
role Sizing {
    size() is int;
    empty() is bool;
}
```

## repr/byte, repr/text — REMOVED (v2.2)

> The representation roles are gone, both of them. Text is `string` and binary
> is `@byte` — native types; wrapping them in roles forced either
> `"hello".text()` (a role method on a literal — absurd) or an adapter object
> around every plain value. APIs now take `string`/`@byte` directly; result
> objects expose plain `text() is string` / `bytes() is @byte` **methods**
> with no role behind them. Composition pays one explicit `.text()`/`.bytes()`
> per layer — an honest, visible cost, and fewer interpreter-level layers
> (measured: every wrapper layer costs real time on this tree-walker).
> `@byte` values are whole values; live byte streams (sockets, stdin) keep
> their own `receive`/`bytes(size)` resource methods.
> `std/repr/` deleted.

---

# Stage 02 — iteration and sequences

## - [ ] collections/iteration

> v2: `next()` is synchronous by contract — see the "live sources" global rule.
> Anything that waits on the outside world must not implement `Iterating`.

```okkama
role Iterating<T> {
    cursor() is Cursor<T>;
}

object Cursor<T> {
    has_next() is bool;
    next() is T;                  // throws IterationEnded
}
```

## - [x] collections/sequence

```okkama
role Sequencing<T> does Iterating<T>, Sizing {
    at(index is int) is T;
    first() is T;                 // throws EmptySequence
    last() is T;                  // throws EmptySequence
}
```

## - [ ] collections/range

```okkama
object Range does Sequencing<int> {
    Range(stop is int);
    Range(start is int, stop is int);
    Range(start is int, stop is int, step is int);

    at(index is int) is int;
    first() is int;
    last() is int;
    size() is int;
    empty() is bool;
    cursor() is Cursor<int>;
}
```

## - [ ] collections/enumerated

```okkama
object Indexed<T> {
    index() is int;
    item() is T;
}

object Enumerated<T> does Iterating<Indexed<T>> {
    Enumerated(origin is Iterating<T>);
    cursor() is Cursor<Indexed<T>>;
}
```

## - [ ] collections/zipped

```okkama
object Pair<A,B> {
    first() is A;
    second() is B;
}

object Zipped<A,B> does Iterating<Pair<A,B>> {
    Zipped(left is Iterating<A>, right is Iterating<B>);
    cursor() is Cursor<Pair<A,B>>;
}
```

## - [ ] collections/chained

```okkama
object Chained<T> does Iterating<T> {
    Chained(origins is @Iterating<T>);
    cursor() is Cursor<T>;
}
```

## - [ ] collections/repeated

```okkama
object Repeated<T> does Iterating<T> {
    Repeated(value is T);         // infinite — combine with Taken
    cursor() is Cursor<T>;
}

object RepeatedTimes<T> does Sequencing<T> {
    RepeatedTimes(value is T, count is int);
    at(index is int) is T;
    first() is T;
    last() is T;
    size() is int;
    empty() is bool;
    cursor() is Cursor<T>;
}
```

## - [ ] collections/cycled

> v2: explicitly infinite — a bare `for` over `Cycled` never terminates; always
> compose with `Taken`/`TakenWhile`. Docs must lead with this.

```okkama
object Cycled<T> does Iterating<T> {
    Cycled(origin is Sequencing<T>);   // infinite
    cursor() is Cursor<T>;
}
```

---

# Stage 03 — collection decorators

> v2: all callback parameters switched from the nonexistent `action(A) is B`
> shape to real action types `(A) is B`. Composition warning: each decorator
> layer adds one Okkama-level dispatch per element — fine for correctness,
> expensive in hot loops; see the native-bulk-paths global rule.

## - [ ] collections/mapped

```okkama
object Mapped<A,B> does Iterating<B> {
    Mapped(origin is Iterating<A>, mapping is (A) is B);
    cursor() is Cursor<B>;
}
```

## - [ ] collections/filtered

```okkama
object Filtered<T> does Iterating<T> {
    Filtered(origin is Iterating<T>, condition is (T) is bool);
    cursor() is Cursor<T>;
}
```

## - [ ] collections/taken

```okkama
object Taken<T> does Iterating<T> {
    Taken(origin is Iterating<T>, count is int);
    cursor() is Cursor<T>;
}
```

## - [ ] collections/dropped

```okkama
object Dropped<T> does Iterating<T> {
    Dropped(origin is Iterating<T>, count is int);
    cursor() is Cursor<T>;
}
```

## - [ ] collections/taken-while

```okkama
object TakenWhile<T> does Iterating<T> {
    TakenWhile(origin is Iterating<T>, condition is (T) is bool);
    cursor() is Cursor<T>;
}
```

## - [ ] collections/dropped-while

```okkama
object DroppedWhile<T> does Iterating<T> {
    DroppedWhile(origin is Iterating<T>, condition is (T) is bool);
    cursor() is Cursor<T>;
}
```

## - [ ] collections/chunked

```okkama
object Chunked<T> does Iterating<@T> {
    Chunked(origin is Iterating<T>, size is int);
    cursor() is Cursor<@T>;
}
```

## - [ ] collections/flattened

```okkama
object Flattened<T> does Iterating<T> {
    Flattened(origin is Iterating<Iterating<T>>);
    cursor() is Cursor<T>;
}
```

## - [ ] collections/pairwise

```okkama
object Pairwise<T> does Iterating<Pair<T,T>> {
    Pairwise(origin is Iterating<T>);
    cursor() is Cursor<Pair<T,T>>;
}
```

## - [ ] collections/accumulated

```okkama
object Accumulated<T,R> does Iterating<R> {
    Accumulated(origin is Iterating<T>, initial is R, accumulation is (R, T) is R);
    cursor() is Cursor<R>;
}
```

## - [ ] collections/reduced

```okkama
object Reduced<T,R> does Projecting<R> {
    Reduced(origin is Iterating<T>, initial is R, reduction is (R, T) is R);
    value() is R;
}
```

---

# Stage 04 — concrete collections

## - [ ] collections/list

> v2: role renamed to the recorded best `-ing` candidate `Linking<T>`
> (was: role `List<T>`; remaining candidates, best -> worst: Chaining,
> Consing, Prepending, Listing). Concrete objects keep noun names — they are
> things, roles are capabilities. Reminder from the shipped interpreter:
> `prepended` returns a plain type, not `^Linking`, so list accumulators are
> NOT rebind-eligible (`acc = acc.prepended(x)` is rejected by design; use
> `reduce` or fresh `let`s — see `docs/todo/local-rebinding-22072026.md`).

```okkama
role Linking<T> does Sequencing<T> {
    head() is T;                       // throws EmptySequence
    tail() is Linking<T>;              // throws EmptySequence
    prepended(value is T) is Linking<T>;
}

object EmptyList<T> does Linking<T> { ... }
object LinkedList<T> does Linking<T> {
    LinkedList(value is T, following is Linking<T>);
    ...
}
```

## - [ ] collections/array

```okkama
object Array<T> does Sequencing<T> {
    Array(origin is @T);
    at(index is int) is T;
    first() is T;
    last() is T;
    size() is int;
    empty() is bool;
    cursor() is Cursor<T>;
}
```

## - [ ] collections/set

> v2: role renamed to the recorded best candidate `Containing<T>`
> (was: role `Set<T>`; remaining candidates: Collecting, Grouping,
> MembershipTesting, Setting).

```okkama
role Containing<T> does Iterating<T>, Sizing {
    contains(value is T) is bool;
    added(value is T) is Containing<T>;
    removed(value is T) is Containing<T>;
}

object HashSet<T> does Containing<T> {
    HashSet(values is Iterating<T>);
    ...
}
```

## - [ ] collections/union

> v2 note (kept from v1 intent): `Union`/`Intersection`/`Difference` are lazy
> views — constructors bind only; `contains`/`cursor` do the work.

```okkama
object Union<T> does Containing<T> {
    Union(left is Containing<T>, right is Containing<T>);
    ...
}
```

## - [ ] collections/intersection

```okkama
object Intersection<T> does Containing<T> {
    Intersection(left is Containing<T>, right is Containing<T>);
    ...
}
```

## - [ ] collections/difference

```okkama
object Difference<T> does Containing<T> {
    Difference(left is Containing<T>, right is Containing<T>);
    ...
}
```

## - [ ] collections/deque

```okkama
object Deque<T> does Sequencing<T> {
    Deque(values is Sequencing<T>);
    at(index is int) is T;
    first() is T;
    last() is T;
    size() is int;
    empty() is bool;
    cursor() is Cursor<T>;
}
```

- [ ] No separate `Stack`; list/deque already cover the use case.

## - [ ] collections/heap

> v2: was `does Sequencing<T>` with `at(index)`/`last()` — that leaks the
> internal heap-array order, which means nothing to a consumer. Now the heap
> exposes only what a heap *is*: cheapest element + persistent removal.
> A fully ordered walk is `Sorted<T>`'s job.

```okkama
object Heap<T> does Sizing {
    Heap(values is Iterating<T>, order is Comparing<T>);
    first() is T;                      // throws EmptySequence
    removed() is Heap<T>;              // without first; throws EmptySequence
    added(value is T) is Heap<T>;
    size() is int;
    empty() is bool;
}
```

## - [ ] collections/counter

```okkama
object Count<T> {
    item() is T;
    amount() is int;
}

object Counter<T> does Iterating<Count<T>>, Sizing {
    Counter(values is Iterating<T>);
    amount(value is T) is int;
    total() is int;
    size() is int;
    empty() is bool;
    cursor() is Cursor<Count<T>>;
}
```

## - [ ] collections/sorted

> v2: must sort through the native runtime sort, not element-by-element
> Okkama dispatch (native-bulk-paths rule). The `Comparing` callback is still
> per-pair Okkama-level — acceptable, but document the cost.

```okkama
object Sorted<T> does Sequencing<T> {
    Sorted(origin is Iterating<T>, order is Comparing<T>);
    at(index is int) is T;
    first() is T;
    last() is T;
    size() is int;
    empty() is bool;
    cursor() is Cursor<T>;
}
```

## - [ ] collections/sorted-by

```okkama
object SortedBy<T,K> does Sequencing<T> {
    SortedBy(origin is Iterating<T>, key is (T) is K, order is Comparing<K>);
    ...
}
```

## - [ ] collections/binary-search

```okkama
object BinarySearch<T> does Projecting<int> {
    BinarySearch(origin is Sequencing<T>, value is T, order is Comparing<T>);
    value() is int;                     // throws NotFound
}

object LowerBound<T> does Projecting<int> { ... }
object UpperBound<T> does Projecting<int> { ... }
```

## - [ ] collections/combinations

> v2: combinatorial explosion × per-element interpreter dispatch — document
> that these are for small n, and lazy cursors are mandatory (never
> materialize the full result set internally).

```okkama
object Combinations<T> does Iterating<@T> {
    Combinations(origin is Sequencing<T>, size is int);
    cursor() is Cursor<@T>;
}

object RepeatedCombinations<T> does Iterating<@T> {
    RepeatedCombinations(origin is Sequencing<T>, size is int);
    cursor() is Cursor<@T>;
}
```

## - [ ] collections/permutations

```okkama
object Permutations<T> does Iterating<@T> {
    Permutations(origin is Sequencing<T>);
    Permutations(origin is Sequencing<T>, size is int);
    cursor() is Cursor<@T>;
}
```

## - [ ] collections/product

```okkama
object Product<T> does Iterating<@T> {
    Product(origins is @Sequencing<T>);
    cursor() is Cursor<@T>;
}
```

---

# Stage 05 — strings and text

## - [ ] string builtin behavior

> v2: added `pad_left`/`pad_right` (Python `rjust`/`ljust`/`zfill` coverage).
> These builtins are the designated native bulk path — decorators and stdlib
> modules should call them instead of hand-rolling per-char loops.

```okkama
string.size() is int
string.empty() is bool
string.at(index is int) is char
string.slice(start is int, end is int) is string
string.contains(part is string) is bool
string.starts_with(prefix is string) is bool
string.ends_with(suffix is string) is bool
string.index(part is string) is int
string.last_index(part is string) is int
string.count(part is string) is int
string.lower() is string
string.upper() is string
string.title() is string
string.capitalize() is string
string.casefold() is string
string.trim() is string
string.trim(chars is string) is string
string.trim_left() is string
string.trim_right() is string
string.split(separator is string) is @string
string.lines() is @string
string.words() is @string
string.replace(old is string, new is string) is string
string.repeated(count is int) is string
string.reversed() is string
string.pad_left(width is int, fill is char) is string
string.pad_right(width is int, fill is char) is string
```

## - [ ] strings/joined

```okkama
object Joined {
    Joined(separator is string, values is Iterating<string>);
    text() is string;
}
```

> v2.2: `strings/concatenated` REMOVED — `Concatenated(parts)` is exactly
> `Joined("", parts)`; two names for one thing.

## - [ ] strings/indented

```okkama
object Indented {
    Indented(origin is string, prefix is string);
    text() is string;
}
```

## - [ ] strings/dedented

```okkama
object Dedented {
    Dedented(origin is string);
    text() is string;
}
```

## - [ ] strings/wrapped

```okkama
object Wrapped {
    Wrapped(origin is string, width is int);
    text() is string;
}
```

## - [ ] strings/shortened

```okkama
object Shortened {
    Shortened(origin is string, width is int, placeholder is string);
    text() is string;
}
```

## - [ ] parsing (top-level module)

> v2.2: was `strings/integer`/`strings/float`/`strings/boolean` — a path that
> couldn't decide whether it was about strings or numbers (v2.1 renamed only
> the objects; the module itself was the disease). Parsing is not a runtime
> type conversion, so it is not a Stage 00 builtin either — it is real work
> with real failure modes and belongs in the stdlib under the name of the
> activity: one small top-level `parsing` module, next to `compare`/`size`/
> `error`. Only the *scalar* parsers live here (their result types are
> builtins with no home module); domain parsers stay next to their domain
> (`DateFromIso` in time/iso, `Json` in encoding/json, `ParsedMail` in
> net/mail). Stub: `std/parsing.ok`.

```okkama
object ParsedInt does Projecting<int> {
    ParsedInt(origin is string);
    ParsedInt(origin is string, base is int);
    value() is int;                  // throws InvalidInteger
}

object ParsedFloat does Projecting<float> {
    ParsedFloat(origin is string);
    value() is float;                // throws InvalidFloat
}

object ParsedBool does Projecting<bool> {
    ParsedBool(origin is string);
    value() is bool;                 // throws InvalidBoolean
}
```

## - [ ] strings/diff

```okkama
object UnifiedDiff {
    UnifiedDiff(left is Sequencing<string>, right is Sequencing<string>);
    text() is string;
}

object Similarity does Projecting<float> {
    Similarity(left is string, right is string);
    value() is float;
}
```

---

# Stage 06 — Unicode and byte representations

> v2 naming note, resolved deliberately: decode-pair naming is intentionally
> non-uniform — `Un`-prefix where the operation is a pure inverse (`Unhexed`,
> `Unbase64`), `-Text` suffix where the result is a text value (`Utf8Text`
> reads better than `UnUtf8`), a distinct name where the decoded thing carries
> extra structure (`PemBlock` has `kind()`). Keep as is.

## - [ ] unicode/character

```okkama
object UnicodeCharacter {
    UnicodeCharacter(value is char);
    name() is string;
    category() is UnicodeCategory;
    combining() is int;
    numeric() is float;             // throws NotNumeric
}
```

## - [ ] unicode/normalized

```okkama
enum NormalizationForm = (nfc, nfd, nfkc, nfkd);

object Normalized {
    Normalized(origin is string, form is NormalizationForm);
    text() is string;
}
```

## - [ ] unicode/utf8

```okkama
object Utf8 {
    Utf8(origin is string);
    bytes() is @byte;
}

object Utf8Text {
    Utf8Text(origin is @byte);
    text() is string;               // throws InvalidUtf8 (with .offset())
}
```

## - [ ] unicode/utf16

```okkama
object Utf16 {
    Utf16(origin is string, order is ByteOrder);
    bytes() is @byte;
}

object Utf16Text {
    Utf16Text(origin is @byte, order is ByteOrder);
    text() is string;               // throws InvalidUtf16
}
```

## - [ ] encoding/hex

```okkama
object Hex {
    Hex(origin is @byte);
    text() is string;
}

object Unhexed {
    Unhexed(origin is string);
    bytes() is @byte;               // throws InvalidHex
}
```

## - [ ] encoding/base32

```okkama
object Base32 { Base32(origin is @byte); text() is string; }
object Unbase32 { Unbase32(origin is string); bytes() is @byte; }
```

## - [ ] encoding/base64

```okkama
object Base64 { Base64(origin is @byte); text() is string; }
object UrlBase64 { UrlBase64(origin is @byte); text() is string; }
object Unbase64 { Unbase64(origin is string); bytes() is @byte; }
object UnurlBase64 { UnurlBase64(origin is string); bytes() is @byte; }
```

## - [ ] encoding/base85

```okkama
object Base85 { Base85(origin is @byte); text() is string; }
object Unbase85 { Unbase85(origin is string); bytes() is @byte; }
```

## - [ ] encoding/quotedprintable

```okkama
object QuotedPrintable { QuotedPrintable(origin is @byte); bytes() is @byte; }
object UnquotedPrintable { UnquotedPrintable(origin is @byte); bytes() is @byte; }
```

## - [ ] encoding/pem

```okkama
object Pem {
    Pem(kind is string, origin is @byte);
    text() is string;
}

object PemBlock {
    PemBlock(origin is string);
    kind() is string;
    bytes() is @byte;
}
```

## - [ ] encoding/binary

```okkama
enum ByteOrder = (big, little, native);

object Uint16 does Projecting<int> { Uint16(origin is @byte, order is ByteOrder); value() is int; }
object Uint32 does Projecting<int> { Uint32(origin is @byte, order is ByteOrder); value() is int; }
object Uint64 does Projecting<int> { Uint64(origin is @byte, order is ByteOrder); value() is int; }
object Int16 does Projecting<int> { Int16(origin is @byte, order is ByteOrder); value() is int; }
object Int32 does Projecting<int> { Int32(origin is @byte, order is ByteOrder); value() is int; }
object Int64 does Projecting<int> { Int64(origin is @byte, order is ByteOrder); value() is int; }
object Float32 does Projecting<float> { Float32(origin is @byte, order is ByteOrder); value() is float; }
object Float64 does Projecting<float> { Float64(origin is @byte, order is ByteOrder); value() is float; }

object BinaryInt16 { BinaryInt16(value is int, order is ByteOrder); bytes() is @byte; }
object BinaryInt32 { BinaryInt32(value is int, order is ByteOrder); bytes() is @byte; }
object BinaryInt64 { BinaryInt64(value is int, order is ByteOrder); bytes() is @byte; }
object BinaryFloat32 { BinaryFloat32(value is float, order is ByteOrder); bytes() is @byte; }
object BinaryFloat64 { BinaryFloat64(value is float, order is ByteOrder); bytes() is @byte; }
```

---

# Stage 07 — actual I/O resources

## - [ ] io/stdout

> v2: bytes go to `write`, text goes to `print`/`println` — distinct names,
> not overloads, so the call site always says which of the two it means.
> Same for Stderr.

```okkama
object Stdout {
    write(data is @byte) is int;
    print(text is string) is int;
    println(text is string) is int;
    flush();
}
```

## - [ ] io/stderr

```okkama
object Stderr {
    write(data is @byte) is int;
    print(text is string) is int;
    println(text is string) is int;
    flush();
}
```

## - [ ] io/stdin

```okkama
object Stdin {
    bytes(size is int) is @byte;
    line() is string;
    text() is string;
}
```

## data/* — REMOVED

> v2: `data/buffered` removed — bytes have value semantics (whole value, one
> call), there is nothing to buffer, and the interpreter already proved that
> wrapper layers around I/O make things slower, not faster (BufferedInput
> experiment: −25-30% throughput; see perf records).
> v2.2: `data/limited`, `data/slice`, `data/concatenated` removed with the
> byte role — once binary is just `@byte`, these are plain array operations:
> `@T.slice(start, end)` and `@T.concatenated(other)` (Stage 00 builtins).
> Stage 07 is stdio only now.

---

# Stage 08 — math

## - [ ] math/abs

```okkama
object AbsInt does Projecting<int> { AbsInt(origin is int); value() is int; }
object AbsFloat does Projecting<float> { AbsFloat(origin is float); value() is float; }
```

## - [ ] math/min

> v2: added generic `MinOf`/`MaxOf` over `Iterating` + `Comparing` — v1 only
> had the variadic scalar forms, while all of statistics already consumed
> `Iterating<float>`; now both call shapes exist and the generic one works for
> any comparable T. (Note: a variadic constructor excludes other constructors
> by language rule, hence separate objects, not overloads.)

```okkama
object MinInt does Projecting<int> { MinInt(first is int, rest is ...int); value() is int; }
object MinFloat does Projecting<float> { MinFloat(first is float, rest is ...float); value() is float; }

object MinOf<T> does Projecting<T> {
    MinOf(values is Iterating<T>, order is Comparing<T>);
    value() is T;                   // throws EmptySequence
}
```

## - [ ] math/max

```okkama
object MaxInt does Projecting<int> { MaxInt(first is int, rest is ...int); value() is int; }
object MaxFloat does Projecting<float> { MaxFloat(first is float, rest is ...float); value() is float; }

object MaxOf<T> does Projecting<T> {
    MaxOf(values is Iterating<T>, order is Comparing<T>);
    value() is T;                   // throws EmptySequence
}
```

## - [ ] math/rounding

> v2: added `RoundedTo` — "round to N digits" had no path in v1 (Python
> `round(x, n)` coverage) short of going through Decimal.

```okkama
object Floor does Projecting<int> { Floor(origin is float); value() is int; }
object Ceiling does Projecting<int> { Ceiling(origin is float); value() is int; }
object Truncated does Projecting<int> { Truncated(origin is float); value() is int; }
object Rounded does Projecting<int> { Rounded(origin is float); value() is int; }
object RoundedTo does Projecting<float> { RoundedTo(origin is float, digits is int); value() is float; }
```

## - [ ] math/power

```okkama
object Power does Projecting<float> { Power(base is float, exponent is float); value() is float; }
object SquareRoot does Projecting<float> { SquareRoot(origin is float); value() is float; }  // throws NegativeRoot
object CubeRoot does Projecting<float> { CubeRoot(origin is float); value() is float; }
```

## - [ ] math/logarithm

```okkama
object NaturalLog does Projecting<float> { NaturalLog(origin is float); value() is float; }  // throws NonPositiveLog
object Log2 does Projecting<float> { Log2(origin is float); value() is float; }
object Log10 does Projecting<float> { Log10(origin is float); value() is float; }
```

## - [ ] math/trigonometry

```okkama
object Sine does Projecting<float> { Sine(origin is float); value() is float; }
object Cosine does Projecting<float> { Cosine(origin is float); value() is float; }
object Tangent does Projecting<float> { Tangent(origin is float); value() is float; }
object ArcSine does Projecting<float> { ArcSine(origin is float); value() is float; }
object ArcCosine does Projecting<float> { ArcCosine(origin is float); value() is float; }
object ArcTangent does Projecting<float> { ArcTangent(origin is float); value() is float; }
object ArcTangent2 does Projecting<float> { ArcTangent2(y is float, x is float); value() is float; }
```

## - [ ] math/integer

```okkama
object Gcd does Projecting<int> { Gcd(values is ...int); value() is int; }
object Lcm does Projecting<int> { Lcm(values is ...int); value() is int; }
object Factorial does Projecting<int> { Factorial(origin is int); value() is int; }  // throws OutOfRange
object IntegerSquareRoot does Projecting<int> { IntegerSquareRoot(origin is int); value() is int; }
object FloorDivision does Projecting<int> { FloorDivision(left is int, right is int); value() is int; }  // throws DivisionByZero
object FloorModulo does Projecting<int> { FloorModulo(left is int, right is int); value() is int; }      // throws DivisionByZero
```

## - [ ] math/rational

```okkama
object Rational does Comparing<Rational> {
    Rational(numerator is int, denominator is int);   // throws DivisionByZero
    plus(other is Rational) is Rational;
    minus(other is Rational) is Rational;
    multiplied(other is Rational) is Rational;
    divided(other is Rational) is Rational;
    compared(other is Rational) is Ordering;
    text() is string;
}
```

## - [ ] math/decimal

```okkama
object Decimal does Comparing<Decimal> {
    Decimal(origin is string);              // throws InvalidDecimal
    plus(other is Decimal) is Decimal;
    minus(other is Decimal) is Decimal;
    multiplied(other is Decimal) is Decimal;
    divided(other is Decimal) is Decimal;
    compared(other is Decimal) is Ordering;
    text() is string;
}
```

- [ ] Add specialized decimal decorators (`Quantized`, `RoundedDecimal`, `SquareRootDecimal`) instead of bloating `Decimal`.

## - [ ] math/complex

> v2: added `text()` — Complex was the only numeric type without a text form.

```okkama
object Complex {
    Complex(real is float, imaginary is float);
    plus(other is Complex) is Complex;
    minus(other is Complex) is Complex;
    multiplied(other is Complex) is Complex;
    divided(other is Complex) is Complex;
    text() is string;
}

object Magnitude does Projecting<float> { Magnitude(origin is Complex); value() is float; }
object Phase does Projecting<float> { Phase(origin is Complex); value() is float; }
object Conjugate does Projecting<Complex> { Conjugate(origin is Complex); value() is Complex; }
```

## - [ ] math/statistics

```okkama
object Mean does Projecting<float> { Mean(values is Iterating<float>); value() is float; }  // throws EmptySequence
object GeometricMean does Projecting<float> { GeometricMean(values is Iterating<float>); value() is float; }
object HarmonicMean does Projecting<float> { HarmonicMean(values is Iterating<float>); value() is float; }
object Median does Projecting<float> { Median(values is Iterating<float>); value() is float; }
object Mode<T> does Projecting<T> { Mode(values is Iterating<T>); value() is T; }
object Variance does Projecting<float> { Variance(values is Iterating<float>); value() is float; }
object PopulationVariance does Projecting<float> { PopulationVariance(values is Iterating<float>); value() is float; }
object Deviation does Projecting<float> { Deviation(values is Iterating<float>); value() is float; }
object Covariance does Projecting<float> { Covariance(left is Iterating<float>, right is Iterating<float>); value() is float; }
object Correlation does Projecting<float> { Correlation(left is Iterating<float>, right is Iterating<float>); value() is float; }
```

---

# Stage 09 — randomness / UUID

## - [ ] random

> v2: `Random` is a resource view (stateful generator), allowed by the
> resource rule. NEW contract requirement: **same seed ⇒ byte-identical
> sequence**, stable across releases — without this, `testing/property`
> failures can never be reproduced from a reported seed. Write this as a
> tested guarantee, not a hope.

```okkama
object Random {
    Random();
    Random(seed is int);

    int(minimum is int, maximum is int) is int;
    float() is float;
    bytes(size is int) is @byte;
    choice(values is Sequencing<T>) is T;    // throws EmptySequence
}
```

## - [ ] random/distributions

```okkama
object NormalRandom does Projecting<float> {
    NormalRandom(origin is Random, mean is float, deviation is float);
    value() is float;
}

object ExponentialRandom does Projecting<float> {
    ExponentialRandom(origin is Random, rate is float);
    value() is float;
}

object Sample<T> does Sequencing<T> {
    Sample(origin is Random, values is Sequencing<T>, count is int);
    ...
}
```

## - [ ] uuid

```okkama
object UUID does Comparing<UUID> {
    UUID(origin is string);        // throws InvalidUuid
    compared(other is UUID) is Ordering;
    text() is string;
    bytes() is @byte;
}

object UUIDv4 {
    UUIDv4(random is Random);
    text() is string;
    bytes() is @byte;
}

object UUIDv7 {
    UUIDv7(clock is Timing, random is Random);
    text() is string;
    bytes() is @byte;
}
```

---

# Stage 10 — time

## - [ ] time/duration

```okkama
object Duration does Comparing<Duration> {
    nanoseconds() is int;
    plus(other is Duration) is Duration;
    minus(other is Duration) is Duration;
    compared(other is Duration) is Ordering;
    text() is string;
}

object Nanoseconds does Projecting<Duration> { Nanoseconds(value is int); value() is Duration; }
object Microseconds does Projecting<Duration> { Microseconds(value is int); value() is Duration; }
object Milliseconds does Projecting<Duration> { Milliseconds(value is int); value() is Duration; }
object Seconds does Projecting<Duration> { Seconds(value is float); value() is Duration; }
object Minutes does Projecting<Duration> { Minutes(value is float); value() is Duration; }
object Hours does Projecting<Duration> { Hours(value is float); value() is Duration; }
object Days does Projecting<Duration> { Days(value is float); value() is Duration; }
```

## - [ ] time/instant

```okkama
object Instant does Comparing<Instant> {
    plus(duration is Duration) is Instant;
    minus(other is Instant) is Duration;
    compared(other is Instant) is Ordering;
}
```

## - [ ] time/timing

```okkama
role Timing {
    now() is DateTime;
    monotonic() is Instant;
}

object SystemClock does Timing {
    now() is DateTime;
    monotonic() is Instant;
}
```

## - [ ] time/date

> v2: `Year`/`Month`/`Day`/`WeekdayOf` projection objects REMOVED — year,
> month, day, weekday are total attributes of what a Date *is* (attribute
> access rule). Four objects + four `.value()` calls to read one date was
> pure ceremony, and each Okkama-level object construction is expensive on
> this interpreter.

```okkama
enum Weekday = (monday, tuesday, wednesday, thursday, friday, saturday, sunday);

object Date does Comparing<Date> {
    Date(year is int, month is int, day is int);   // throws InvalidDate
    year() is int;
    month() is int;
    day() is int;
    weekday() is Weekday;
    plus(duration is Duration) is Date;
    compared(other is Date) is Ordering;
    text() is string;
}
```

## - [ ] time/datetime

> v2: added the total attributes (`date()`, clock fields) — v1 gave no way at
> all to read components out of a DateTime.

```okkama
object DateTime does Comparing<DateTime> {
    date() is Date;
    hour() is int;
    minute() is int;
    second() is int;
    nanosecond() is int;
    plus(duration is Duration) is DateTime;
    minus(other is DateTime) is Duration;
    compared(other is DateTime) is Ordering;
    text() is string;
}
```

## - [ ] time/zone

```okkama
object Zone {
    Zone(name is string);                    // throws UnknownZone
    text() is string;
}

object ZonedDateTime does Projecting<DateTime> {
    ZonedDateTime(origin is DateTime, zone is Zone);
    value() is DateTime;
}
```

## - [ ] time/iso

```okkama
object IsoDate { IsoDate(origin is Date); text() is string; }
object IsoDateTime { IsoDateTime(origin is DateTime); text() is string; }

object DateFromIso does Projecting<Date> {
    DateFromIso(origin is string);
    value() is Date;                         // throws InvalidDate
}

object DateTimeFromIso does Projecting<DateTime> {
    DateTimeFromIso(origin is string);
    value() is DateTime;                     // throws InvalidDateTime
}
```

## - [ ] time/rfc3339

```okkama
object Rfc3339 { Rfc3339(origin is DateTime); text() is string; }

object DateTimeFromRfc3339 does Projecting<DateTime> {
    DateTimeFromRfc3339(origin is string);
    value() is DateTime;                     // throws InvalidDateTime
}
```

---

# Stage 11 — filesystem

## - [ ] fs/path

> v2: `PathName`/`PathStem`/`PathSuffix` collapsed into `Path` methods (total
> attributes). The four `Projecting` path objects stay — they compute or do
> I/O (`ResolvedPath` hits the filesystem), which is exactly what the rule
> reserves objects for.

```okkama
object Path does Comparing<Path> {
    Path(origin is string);

    parent() is Path;
    name() is string;
    stem() is string;
    suffix() is string;                      // "" when none
    joined(part is string) is Path;
    compared(other is Path) is Ordering;
    text() is string;
}

object AbsolutePath does Projecting<Path> { AbsolutePath(origin is Path); value() is Path; }
object NormalizedPath does Projecting<Path> { NormalizedPath(origin is Path); value() is Path; }
object ResolvedPath does Projecting<Path> { ResolvedPath(origin is Path); value() is Path; }     // throws MissingFile
object RelativePath does Projecting<Path> { RelativePath(origin is Path, base is Path); value() is Path; }
```

## - [ ] fs/file

```okkama
object File {
    File(path is Path);

    exists() is bool;
    stat() is FileStat;                      // throws MissingFile
    remove();                                // throws MissingFile
    move(destination is Path) is File;
}
```

## - [ ] fs/data-file

```okkama
object DataFile {
    DataFile(file is File);
    bytes() is @byte;                        // throws MissingFile
}
```

## - [ ] fs/text-file

> v2: `Encoding` parameter dropped — the role was never defined and 1.0 has no
> codecs registry (deliberately; see Stage 35). `TextFile` is UTF-8. Other
> encodings, when they exist, come by composition: `Utf16Text(DataFile(f), order)`.

```okkama
object TextFile {
    TextFile(file is File);
    text() is string;                        // throws MissingFile, InvalidUtf8
}
```

## - [ ] fs/written-file

```okkama
object WrittenFile does Projecting<File> {
    WrittenFile(file is File, data is @byte);
    value() is File;
}

object AppendedFile does Projecting<File> {
    AppendedFile(file is File, data is @byte);
    value() is File;
}
```

## - [ ] fs/directory

```okkama
object Directory does Iterating<Path> {
    Directory(path is Path);

    exists() is bool;
    cursor() is Cursor<Path>;                // throws MissingDirectory
    remove();
    move(destination is Path) is Directory;
}
```

## - [ ] fs/files

```okkama
object Files does Iterating<File> {
    Files(directory is Directory);
    cursor() is Cursor<File>;
}

object Directories does Iterating<Directory> {
    Directories(directory is Directory);
    cursor() is Cursor<Directory>;
}
```

## - [ ] fs/globbed

```okkama
object Globbed does Iterating<Path> {
    Globbed(root is Directory, pattern is string);
    cursor() is Cursor<Path>;
}
```

## - [ ] fs/tree

```okkama
object Tree does Iterating<Path> {
    Tree(root is Directory);
    cursor() is Cursor<Path>;
}
```

## - [ ] fs/temp

```okkama
object TemporaryFile does Projecting<File> {
    TemporaryFile(directory is Directory, prefix is string);
    value() is File;
}

object TemporaryDirectory does Projecting<Directory> {
    TemporaryDirectory(parent is Directory, prefix is string);
    value() is Directory;
}
```

## - [ ] fs/stat

```okkama
object FileStat {
    size() is int;
    mode() is FileMode;
    modified() is DateTime;
    kind() is FileKind;
}

enum FileKind = (file, directory, link, other);
```

## - [ ] fs/compare

```okkama
object SameFile does Projecting<bool> {
    SameFile(left is File, right is File);
    value() is bool;
}

object SameContent does Projecting<bool> {
    SameContent(left is @byte, right is @byte);
    value() is bool;
}
```

## - [ ] fs/mmap — LOW PRIORITY

> v2: v1's `MappedFile.bytes() is @byte` materialized the whole file, which is
> just `DataFile` with extra steps — the entire point of mmap is windowed
> access without full materialization. Redefined with a window; keep low
> priority (the Perl tree-walker will emulate it with seek+read anyway).

```okkama
object MappedFile {
    MappedFile(file is File);
    window(offset is int, size is int) is @byte;   // throws OutOfRange
    size() is int;
}
```

## - [ ] fs/watch

> v2: was `Iterating<FileEvent>` — but a filesystem watch *waits*; a
> synchronous `Cursor.next()` would block the interpreter (live-sources rule).
> Now an explicit async resource.

```okkama
object FileWatch {
    FileWatch(path is Path);
    async next_event() is FileEvent;
    close();
}

object FileEvent {
    path() is Path;
    kind() is FileEventKind;
}

enum FileEventKind = (created, modified, removed, renamed);
```

---

# Stage 12 — OS / process

## - [ ] os/environment

```okkama
object Environment {
    contains(name is string) is bool;
    value(name is string) is string;         // throws MissingVariable
    variables() is %string:string;
}

object HomeDirectory does Projecting<Directory> {
    HomeDirectory(environment is Environment);
    value() is Directory;
}

object SearchPath does Sequencing<Path> {
    SearchPath(environment is Environment);
    ...
}
```

## - [ ] os/system

```okkama
object System {
    operating_system() is OperatingSystem;
    architecture() is string;
    hostname() is string;
    cpu_count() is int;
}
```

## - [ ] os/user

```okkama
object User {
    name() is string;
    home() is Directory;
    id() is int;
}

object CurrentUser {
    name() is string;
    home() is Directory;
    id() is int;
}
```

## - [ ] os/group

```okkama
object Group {
    name() is string;
    id() is int;
    users() is Sequencing<User>;
}
```

## - [ ] os/process

```okkama
object Command {
    Command(program is Path, arguments is Sequencing<string>);
    text() is string;
}

object Process {
    Process(command is Command);

    id() is int;
    running() is bool;
    async wait() is Exit;
    terminate();
}
```

## - [ ] os/output-process

> v2: `Encoding` parameter dropped from `TextProcess` (same reasoning as
> `TextFile` — UTF-8, composition for the rest).

```okkama
object OutputProcess {
    OutputProcess(command is Command);
    bytes() is @byte;                        // throws ProcessFailed
}

object TextProcess {
    TextProcess(command is Command);
    text() is string;                        // throws ProcessFailed, InvalidUtf8
}
```

## - [ ] os/pipeline

```okkama
object Pipeline {
    Pipeline(commands is Sequencing<Command>);
    bytes() is @byte;
}
```

## - [ ] os/signal

```okkama
enum Signal = (interrupt, terminate, kill, hangup, quit);
```

## - [ ] os/executable

```okkama
object Executable does Projecting<Path> {
    Executable(name is string, environment is Environment);
    value() is Path;                         // throws MissingExecutable
}
```

## - [ ] os/disk

```okkama
object Disk {
    Disk(path is Path);
    total() is int;
    free() is int;
    used() is int;
}
```

---

# Stage 13 — structured serialization

## - [ ] encoding/json

```okkama
role JsonRepresenting {
    json() is string;
}

object JsonNull does JsonRepresenting {
    json() is string;
}

object JsonBoolean does JsonRepresenting {
    JsonBoolean(value is bool);
    json() is string;
}

object JsonNumber does JsonRepresenting {
    JsonNumber(value is float);
    json() is string;
}

object JsonString does JsonRepresenting {
    JsonString(value is string);
    json() is string;
}

object JsonArray does JsonRepresenting, Sequencing<JsonRepresenting> {
    JsonArray(values is Sequencing<JsonRepresenting>);
    json() is string;
    at(index is int) is JsonRepresenting;
    first() is JsonRepresenting;
    last() is JsonRepresenting;
    size() is int;
    empty() is bool;
    cursor() is Cursor<JsonRepresenting>;
}

object JsonObject does JsonRepresenting {
    JsonObject(values is %string:JsonRepresenting);
    json() is string;
    contains(name is string) is bool;
    value(name is string) is JsonRepresenting;   // throws MissingKey
}

object Json does Projecting<JsonRepresenting> {
    Json(origin is string);
    value() is JsonRepresenting;                 // throws InvalidJson
}

// v2.2: JsonData removed — `Utf8(doc.json()).bytes()` says the same thing
// without a dedicated object.

object PrettyJson {
    PrettyJson(origin is JsonRepresenting, indent is int);
    text() is string;
}
```

## - [ ] encoding/csv

```okkama
object Csv does Iterating<@string> {
    Csv(origin is string, delimiter is char);
    cursor() is Cursor<@string>;                 // throws InvalidCsv
}

object CsvText {
    CsvText(rows is Iterating<@string>, delimiter is char);
    text() is string;
}
```

## - [ ] encoding/toml

> v2: `TomlDocument` was a black box in v1 — minimal shape added. Values map
> onto the JSON value model where possible; only tables/arrays-of-tables are
> TOML-specific. Define fully before implementing.

```okkama
object TomlDocument {
    contains(name is string) is bool;
    value(name is string) is JsonRepresenting;   // dotted keys; throws MissingKey
    tables(name is string) is Sequencing<TomlDocument>;
}

object Toml does Projecting<TomlDocument> {
    Toml(origin is string);
    value() is TomlDocument;                     // throws InvalidToml
}

object TomlText {
    TomlText(origin is TomlDocument);
    text() is string;
}
```

## - [ ] encoding/ini

> v2: minimal `IniDocument` shape added (was a black box).

```okkama
object IniDocument {
    sections() is Sequencing<string>;
    contains(section is string, name is string) is bool;
    value(section is string, name is string) is string;  // throws MissingKey
}

object Ini does Projecting<IniDocument> {
    Ini(origin is string);
    value() is IniDocument;                      // throws InvalidIni
}

object IniText {
    IniText(origin is IniDocument);
    text() is string;
}
```

## - [ ] encoding/xml

> v2: minimal node model added (was a black box). Elements only in 1.0 —
> no DTD, no namespaces resolution beyond prefixed names, no XPath.

```okkama
object XmlNode {
    name() is string;
    attribute(name is string) is string;         // throws MissingAttribute
    attributes() is %string:string;
    children() is Sequencing<XmlNode>;
    inner_text() is string;
}

object Xml does Projecting<XmlNode> {
    Xml(origin is string);
    value() is XmlNode;                          // root; throws InvalidXml
}

object XmlText {
    XmlText(origin is XmlNode);
    text() is string;
}
```

## - [ ] encoding/html

> v2: reuses the XML node model (lenient parse); `HtmlDocument` black box
> replaced.

```okkama
object Html does Projecting<XmlNode> {
    Html(origin is string);
    value() is XmlNode;                          // lenient; never throws on recoverable markup
}

object EscapedHtml {
    EscapedHtml(origin is string);
    text() is string;
}
```

## - [ ] encoding/plist — LOW PRIORITY

> v2: Apple-specific; keep for Python-breadth parity but implement last in
> this stage. `PlistDocument` maps onto the JSON value model.

```okkama
enum PlistFormat = (xml, binary);

object Plist does Projecting<JsonRepresenting> {
    Plist(origin is @byte);
    value() is JsonRepresenting;                 // throws InvalidPlist
}

object PlistData {
    PlistData(origin is JsonRepresenting, format is PlistFormat);
    bytes() is @byte;
}
```

---

# Stage 14 — regex / lexical text

## - [ ] regex/pattern

```okkama
object Pattern {
    Pattern(expression is string);               // throws InvalidPattern

    matches(text is string) is bool;
    first(text is string) is Match;    // throws NoMatch
    all(text is string) is Sequencing<Match>;
}
```

## - [ ] regex/match

> v2: `Span` defined here locally (v1 referenced an undefined type that
> collided with tracing's and the compiler's spans — see the unique-names rule).

```okkama
object Match {
    text() is string;
    start() is int;
    end() is int;
    group(name is string) is string;             // throws MissingGroup
}
```

## - [ ] regex/replaced

```okkama
object Replaced {
    Replaced(origin is string, pattern is Pattern, replacement is string);
    text() is string;
}
```

## - [ ] regex/split

```okkama
object SplitByPattern does Sequencing<string> {
    SplitByPattern(origin is string, pattern is Pattern);
    ...
}
```

> v2: `text/tokens` REMOVED — it had no methods, no described consumer, and
> Python ships no generic text tokenizer either (only point tools like shlex).
> Okkama's own tokens live in Stage 32 where they belong.

---

# Stage 15 — checksums / digests

## - [ ] hash/adler32

```okkama
object Adler32 does Projecting<int> {
    Adler32(origin is @byte);
    value() is int;
}
```

## - [ ] hash/crc32

```okkama
object Crc32 does Projecting<int> {
    Crc32(origin is @byte);
    value() is int;
}
```

## - [ ] hash/crc64

```okkama
object Crc64 does Projecting<int> {
    Crc64(origin is @byte);
    value() is int;
}
```

## - [ ] hash/fnv

```okkama
object Fnv1a32 does Projecting<int> { Fnv1a32(origin is @byte); value() is int; }
object Fnv1a64 does Projecting<int> { Fnv1a64(origin is @byte); value() is int; }
```

## - [ ] crypto/md5

```okkama
object Md5 {
    Md5(origin is @byte);
    bytes() is @byte;
}
```

## - [ ] crypto/sha1

```okkama
object Sha1 {
    Sha1(origin is @byte);
    bytes() is @byte;
}
```

## - [ ] crypto/sha2

```okkama
object Sha224 { Sha224(origin is @byte); bytes() is @byte; }
object Sha256 { Sha256(origin is @byte); bytes() is @byte; }
object Sha384 { Sha384(origin is @byte); bytes() is @byte; }
object Sha512 { Sha512(origin is @byte); bytes() is @byte; }
```

## - [ ] crypto/sha3

> v2: `Sha3_224` -> `Sha3224` etc — these were the only underscored type
> names in the whole stdlib (`Sha256`, `Uint16`, `Log2` set the convention).

```okkama
object Sha3224 { Sha3224(origin is @byte); bytes() is @byte; }
object Sha3256 { Sha3256(origin is @byte); bytes() is @byte; }
object Sha3384 { Sha3384(origin is @byte); bytes() is @byte; }
object Sha3512 { Sha3512(origin is @byte); bytes() is @byte; }
```

## - [ ] crypto/hmac

```okkama
enum DigestAlgorithm = (sha256, sha384, sha512, sha1, md5);

object Hmac {
    Hmac(origin is @byte, key is @byte, algorithm is DigestAlgorithm);
    bytes() is @byte;
}

object ValidHmac does Projecting<bool> {
    ValidHmac(origin is @byte, key is @byte, signature is @byte, algorithm is DigestAlgorithm);
    value() is bool;                    // constant-time comparison
}
```

---

# Stage 16 — compression

## - [ ] compression/gzip

```okkama
object Gzip {
    Gzip(origin is @byte);
    Gzip(origin is @byte, level is int);
    bytes() is @byte;
}

object Gunzipped {
    Gunzipped(origin is @byte);
    bytes() is @byte;                   // throws InvalidGzip
}
```

## - [ ] compression/zlib

```okkama
object Zlib { Zlib(origin is @byte); bytes() is @byte; }
object Unzlib { Unzlib(origin is @byte); bytes() is @byte; }
```

## - [ ] compression/deflate

```okkama
object Deflated { Deflated(origin is @byte); bytes() is @byte; }
object Inflated { Inflated(origin is @byte); bytes() is @byte; }
```

## - [ ] compression/bzip2

```okkama
object Bzip2 { Bzip2(origin is @byte); bytes() is @byte; }
object Unbzip2 { Unbzip2(origin is @byte); bytes() is @byte; }
```

## - [ ] compression/xz

```okkama
object Xz { Xz(origin is @byte); bytes() is @byte; }
object Unxz { Unxz(origin is @byte); bytes() is @byte; }
```

## - [ ] compression/zstd

```okkama
object Zstd { Zstd(origin is @byte); bytes() is @byte; }
object Unzstd { Unzstd(origin is @byte); bytes() is @byte; }
```

---

# Stage 17 — archives

> v2: delete the stale `docs/std/archive-tar.md` draft — it predates this plan
> and uses the exact job-title vocabulary (`open`/`create`/`extract`/`add`)
> the global rules forbid.

## - [ ] archive/entry

> naming candidates, best -> worst: ArchivingEntry, CarryingArchiveData,
> ContainingArchiveData, ProvidingArchiveData, Archiving — noun kept: an entry
> *is* a named piece of data, no gerund says that better.

```okkama
role ArchiveEntry {
    name() is string;
    data() is @byte;
}
```

## - [ ] archive/tar

```okkama
object Tar {
    Tar(entries is Iterating<ArchiveEntry>);
    bytes() is @byte;
}

object Untarred does Iterating<ArchiveEntry> {
    Untarred(origin is @byte);
    cursor() is Cursor<ArchiveEntry>;   // throws InvalidTar
}
```

## - [ ] archive/zip

```okkama
object Zip {
    Zip(entries is Iterating<ArchiveEntry>);
    bytes() is @byte;
}

object Unzipped does Iterating<ArchiveEntry> {
    Unzipped(origin is @byte);
    cursor() is Cursor<ArchiveEntry>;   // throws InvalidZip
}
```

## - [ ] archive/file-entry

```okkama
object FileEntry does ArchiveEntry {
    FileEntry(name is string, file is File);
    name() is string;
    data() is @byte;
}

object DataEntry does ArchiveEntry {
    DataEntry(name is string, data is @byte);
    name() is string;
    data() is @byte;
}
```

---

# Stage 18 — cryptography

## - [ ] crypto/random

```okkama
object SecureRandom {
    bytes(size is int) is @byte;
    int(maximum is int) is int;
}
```

## - [ ] crypto/keys

> v2: NEW — v1 used `Ed25519PrivateKey`/`EcdsaPrivateKey`/`RsaPrivateKey` and
> the public keys everywhere without defining them. Rules (see global secret
> rule): private keys have NO `bytes()`/`text()` methods — no ambient way to
> print or log a secret; export is a deliberate act through an explicit
> object. Public keys may expose `bytes()`.

```okkama
object Ed25519PrivateKey {
    Ed25519PrivateKey(seed is @byte);     // throws InvalidKey
    public() is Ed25519PublicKey;
}
object Ed25519PublicKey {
    Ed25519PublicKey(origin is @byte);    // throws InvalidKey
    bytes() is @byte;
}
object Ed25519Keys {
    Ed25519Keys(random is SecureRandom);
    public() is Ed25519PublicKey;
    private() is Ed25519PrivateKey;
}

// EcdsaPrivateKey/EcdsaPublicKey, EcdhPrivateKey/EcdhPublicKey,
// RsaPrivateKey/RsaPublicKey: same shape (private: no bytes()/text();
// public: bytes(); generation object takes SecureRandom).

object Pkcs8 {
    Pkcs8(key is Ed25519PrivateKey);                 // explicit, deliberate export
    bytes() is @byte;
}

object PrivateKeyFromPkcs8 does Projecting<Ed25519PrivateKey> {
    PrivateKeyFromPkcs8(origin is @byte);
    value() is Ed25519PrivateKey;                    // throws InvalidKey
}
```

## - [ ] crypto/kdf

```okkama
object Hkdf {
    Hkdf(secret is @byte, salt is @byte, info is @byte, size is int, algorithm is DigestAlgorithm);
    bytes() is @byte;
}

object Pbkdf2 {
    Pbkdf2(password is @byte, salt is @byte, iterations is int, size is int, algorithm is DigestAlgorithm);
    bytes() is @byte;
}
```

## - [ ] crypto/aead

```okkama
object AesGcmCiphertext {
    AesGcmCiphertext(key is @byte, nonce is @byte, plaintext is @byte, associated is @byte);
    bytes() is @byte;
}

object AesGcmPlaintext {
    AesGcmPlaintext(key is @byte, nonce is @byte, ciphertext is @byte, associated is @byte);
    bytes() is @byte;                                // throws InvalidCiphertext
}

object ChaChaCiphertext {
    ChaChaCiphertext(key is @byte, nonce is @byte, plaintext is @byte, associated is @byte);
    bytes() is @byte;
}

object ChaChaPlaintext {
    ChaChaPlaintext(key is @byte, nonce is @byte, ciphertext is @byte, associated is @byte);
    bytes() is @byte;                                // throws InvalidCiphertext
}
```

## - [ ] crypto/ed25519

```okkama
object Ed25519Signature {
    Ed25519Signature(key is Ed25519PrivateKey, message is @byte);
    bytes() is @byte;
}

object ValidEd25519 does Projecting<bool> {
    ValidEd25519(key is Ed25519PublicKey, message is @byte, signature is @byte);
    value() is bool;
}
```

## - [ ] crypto/ecdh

```okkama
object SharedSecret {
    SharedSecret(private is EcdhPrivateKey, public is EcdhPublicKey);
    bytes() is @byte;
}
```

## - [ ] crypto/ecdsa

```okkama
object EcdsaSignature {
    EcdsaSignature(key is EcdsaPrivateKey, digest is @byte);
    bytes() is @byte;
}

object ValidEcdsa does Projecting<bool> {
    ValidEcdsa(key is EcdsaPublicKey, digest is @byte, signature is @byte);
    value() is bool;
}
```

## - [ ] crypto/rsa

```okkama
object RsaCiphertext {
    RsaCiphertext(key is RsaPublicKey, plaintext is @byte);
    bytes() is @byte;
}

object RsaPlaintext {
    RsaPlaintext(key is RsaPrivateKey, ciphertext is @byte);
    bytes() is @byte;                                // throws InvalidCiphertext
}

object RsaSignature {
    RsaSignature(key is RsaPrivateKey, digest is @byte);
    bytes() is @byte;
}
```

## - [ ] encoding/asn1

```okkama
object Asn1 does Projecting<Asn1Value> {
    Asn1(origin is @byte);
    value() is Asn1Value;                            // throws InvalidAsn1
}

object Asn1Data {
    Asn1Data(origin is Asn1Value);
    bytes() is @byte;
}
```

## - [ ] crypto/x509

```okkama
object Certificate {
    Certificate(origin is @byte);         // throws InvalidCertificate
    bytes() is @byte;
    text() is string;
    valid(at is DateTime) is bool;
    matches(host is string) is bool;
}
```

## - [ ] crypto/tls

```okkama
object TlsConfig {
    TlsConfig(verification is PeerVerification);
    // pin/override roots via decorating configs, not setters
}

enum PeerVerification = (required, disabled);

object TlsConnection {
    TlsConnection(origin is TcpConnection, config is TlsConfig);

    async receive(size is int) is @byte;
    async send(data is @byte) is int;
    peer() is Address;
    close();
}
```

---

# Stage 19 — concurrency

## - [ ] async/task

```okkama
object Task<T> {
    done() is bool;
    cancelled() is bool;
    async result() is T;                 // throws Cancelled
    cancel() is bool;
}
```

## - [ ] async/all

```okkama
object All<T> does Projecting<@T> {
    All(tasks is @Task<T>);
    async value() is @T;
}
```

## - [ ] async/any

```okkama
object Any<T> does Projecting<T> {
    Any(tasks is @Task<T>);
    async value() is T;
}
```

## - [ ] async/timed

```okkama
object Timed<T> does Projecting<T> {
    Timed(task is Task<T>, limit is Duration);
    async value() is T;                  // throws TimedOut
}
```

## - [ ] async/completed

> v2: was `Iterating<Task<T>>` — but "as they complete" *waits*, so the
> live-sources rule applies. Explicit async method instead of a Cursor.

```okkama
object Completed<T> {
    Completed(tasks is @Task<T>);
    remaining() is int;
    async next_done() is Task<T>;        // throws IterationEnded
}
```

## - [ ] async/channel

```okkama
object Channel<T> {
    async send(value is T);              // throws ClosedChannel
    async receive() is T;                // throws ClosedChannel
    closed() is bool;
    close();
}
```

## - [ ] sync policy

- [ ] Keep mutex/RW-lock/condition/atomic primitives runtime-internal initially.
- [ ] Do not copy Go's synchronization package until Okkama has a concrete immutable-object use case.
- [ ] Re-evaluate after process/async semantics are stable.

---

# Stage 20 — networking primitives

## - [ ] net/ip

```okkama
object Ip does Comparing<Ip> {
    Ip(origin is string);      // throws InvalidIp

    compared(other is Ip) is Ordering;
    text() is string;
    bytes() is @byte;
    private() is bool;
    loopback() is bool;
}
```

## - [ ] net/network

```okkama
object Network {
    Network(origin is string); // throws InvalidNetwork
    contains(address is Ip) is bool;
    text() is string;
}
```

## - [ ] net/address

```okkama
object Address {
    Address(host is string, port is int);
    text() is string;
}
```

## - [ ] net/dns

No `Resolver`. Constructors bind only — resolution happens lazily on first
access (`at`/`cursor`), per the no-I/O-in-constructors rule.

```okkama
object HostAddresses does Sequencing<Ip> {
    HostAddresses(host is string);
    at(index is int) is Ip;
    first() is Ip;
    last() is Ip;
    size() is int;
    empty() is bool;
    cursor() is Cursor<Ip>;              // throws DnsFailure
}

object HostName {
    HostName(address is Ip);
    text() is string;                    // throws DnsFailure
}

object MxRecord {
    priority() is int;
    host() is string;
}

object SrvRecord {
    priority() is int;
    weight() is int;
    port() is int;
    host() is string;
}

object MxRecords does Iterating<MxRecord> {
    MxRecords(host is string);
    cursor() is Cursor<MxRecord>;
}

object TxtRecords does Iterating<string> {
    TxtRecords(host is string);
    cursor() is Cursor<string>;
}

object SrvRecords does Iterating<SrvRecord> {
    SrvRecords(service is string, protocol is string, host is string);
    cursor() is Cursor<SrvRecord>;
}
```

## - [ ] net/tcp

```okkama
object TcpConnection {
    TcpConnection(address is Address);

    async receive(size is int) is @byte;     // throws TcpFailure
    async send(data is @byte) is int;
    peer() is Address;
    close();
}
```

## - [ ] net/tcp-endpoint

No server/listener object.

> v2: was `Iterating<TcpConnection>` — accepting a connection *waits*
> (live-sources rule). Explicit async accept instead of a Cursor.

```okkama
object TcpEndpoint {
    TcpEndpoint(address is Address);
    async accept() is TcpConnection;         // throws ClosedEndpoint
    close();
}
```

## - [ ] net/udp

```okkama
object Datagram {
    data() is @byte;
    source() is Address;
}

object UdpEndpoint {
    UdpEndpoint(address is Address);

    async receive(size is int) is Datagram;
    async send(data is @byte, destination is Address) is int;
    close();
}
```

## - [ ] net/unix

```okkama
object UnixConnection {
    UnixConnection(path is Path);

    async receive(size is int) is @byte;
    async send(data is @byte) is int;
    close();
}

object UnixEndpoint {
    UnixEndpoint(path is Path);
    async accept() is UnixConnection;
    close();
}
```

---

# Stage 21 — URL / MIME / mail

## - [ ] net/url

> v2: `scheme`/`host`/`path` were already methods (total); `UrlPort`/`UrlQuery`
> stay objects (partial — a URL may have neither). This asymmetry is the
> attribute-access rule working as intended, now stated instead of implied.

```okkama
object Url {
    Url(origin is string);         // throws InvalidUrl

    scheme() is string;
    host() is string;
    path() is string;
    text() is string;
}

object UrlPort does Projecting<int> {
    UrlPort(origin is Url);
    value() is int;                          // throws MissingPort
}

object UrlQuery does Projecting<Query> {
    UrlQuery(origin is Url);
    value() is Query;                        // throws MissingQuery
}

object ResolvedUrl does Projecting<Url> {
    ResolvedUrl(base is Url, relative is Url);
    value() is Url;
}
```

## - [ ] net/query

```okkama
object Query {
    Query(origin is string);

    contains(name is string) is bool;
    value(name is string) is string;         // throws MissingKey
    values(name is string) is @string;
    text() is string;
}
```

## - [ ] net/mime

```okkama
object MediaType {
    MediaType(origin is string);   // throws InvalidMediaType

    type() is string;
    subtype() is string;
    parameter(name is string) is string;     // throws MissingParameter
    text() is string;
}
```

## - [ ] net/multipart

```okkama
object Multipart does Iterating<Part> {
    Multipart(origin is @byte, boundary is string);
    cursor() is Cursor<Part>;                // throws InvalidMultipart
}

object Part {
    name() is string;
    data() is @byte;
    media_type() is MediaType;
}
```

## - [ ] net/mail/address

```okkama
object MailAddress {
    MailAddress(origin is string); // throws InvalidMailAddress

    name() is string;
    email() is string;
    text() is string;
}
```

## - [ ] net/mail/message

> v2: `MailHeaders` was referenced but never defined — minimal shape added
> (same access idiom as HTTP `Headers`).

```okkama
object MailHeaders {
    MailHeaders(values is %string:@string);
    contains(name is string) is bool;
    value(name is string) is string;         // throws MissingHeader
    values(name is string) is @string;
    added(name is string, value is string) is MailHeaders;
}

object MailMessage {
    MailMessage(headers is MailHeaders, body is @byte);
    bytes() is @byte;
}

object ParsedMail does Projecting<MailMessage> {
    ParsedMail(origin is @byte);
    value() is MailMessage;                  // throws InvalidMail
}
```

## - [ ] net/smtp

No `SmtpClient`.

> v2: `SmtpCredentials` defined (was referenced only).

```okkama
object SmtpCredentials {
    SmtpCredentials(username is string, password is string);
    username() is string;
    // password never exposed; consumed internally by SmtpDelivery
}

object SmtpDelivery does Projecting<MailReceipt> {
    SmtpDelivery(
        endpoint is Address,
        message is MailMessage,
        credentials is SmtpCredentials
    );

    async value() is MailReceipt;            // throws SmtpFailure
}
```

---

# Stage 22 — HTTP

## - [ ] net/http/method-status

> v2: NEW — `Method` and `Status` were used throughout v1 but never defined.
> Shapes follow the battle-tested old stdlib (`http/request.ok`, `http/status.ok`).

```okkama
enum Method = (get, head, post, put, patch, delete, options, connect, trace);

object Status {
    Status(code is int);                     // throws InvalidStatus
    code() is int;
    reason() is string;
    success() is bool;
    redirect() is bool;
    error() is bool;
}
```

## - [ ] net/http/request

```okkama
object Request {
    Request(method is Method, url is Url, headers is Headers, body is @byte);
    bytes() is @byte;
}

object RequestMethod does Projecting<Method> {
    RequestMethod(origin is Request);
    value() is Method;
}

object RequestUrl does Projecting<Url> {
    RequestUrl(origin is Request);
    value() is Url;
}

object RequestHeaders does Projecting<Headers> {
    RequestHeaders(origin is Request);
    value() is Headers;
}

object RequestBody {
    RequestBody(origin is Request);
    bytes() is @byte;
}
```

## - [ ] net/http/response

```okkama
object Response {
    Response(status is Status, headers is Headers, body is @byte);
    bytes() is @byte;
}

object ResponseStatus does Projecting<Status> {
    ResponseStatus(origin is Response);
    value() is Status;
}

object ResponseHeaders does Projecting<Headers> {
    ResponseHeaders(origin is Response);
    value() is Headers;
}

object ResponseBody {
    ResponseBody(origin is Response);
    bytes() is @byte;
}
```

## - [ ] net/http/remote-response

Replaces HTTP Client.

```okkama
object RemoteResponse does Projecting<Response> {
    RemoteResponse(request is Request);
    RemoteResponse(request is Request, timeout is Duration);
    async value() is Response;               // throws HttpFailure, TimedOut
}
```

## - [ ] net/http/headers

> v2: constructor added (v1 had only `added`, no way to make one).

```okkama
object Headers {
    Headers(values is %string:@string);
    contains(name is string) is bool;
    value(name is string) is string;         // throws MissingHeader
    values(name is string) is @string;
    added(name is string, value is string) is Headers;
}
```

## - [ ] net/http/cookie

```okkama
object Cookie {
    Cookie(origin is string);      // throws InvalidCookie

    name() is string;
    value() is string;
    text() is string;
}
```

## - [ ] net/http/cookies

```okkama
object Cookies does Iterating<Cookie> {
    Cookies(origin is Headers);
    cursor() is Cursor<Cookie>;
}
```

## - [ ] net/http/form

```okkama
object Form {
    Form(fields is %string:@string);
    bytes() is @byte;
}

object ParsedForm {
    ParsedForm(origin is @byte);

    contains(name is string) is bool;
    value(name is string) is string;         // throws MissingField
    values(name is string) is @string;
}
```

## - [ ] net/http/routes

No router/handler worker objects.

> v2: **`Responding` restored to the two-argument signature** migrated to in
> the old stdlib on 25.07.2026 (user's explicit design goal): the incoming
> prototype `Response` lets a route build error/fallback responses via its
> `^`-marked `answer(...)` successor. v1 silently reverted to the
> one-argument shape that migration replaced. Also: `Route` was undefined in
> v1 — minimal shape below; port the 9 battle-tested route types from
> `stdlib/http/router.ok` instead of redesigning from scratch.

```okkama
role Responding {
    async response(request is Request, response is Response) is Response;
}

object Route {
    Route(method is Method, pattern is string);
    matches(request is Request) is bool;
}

object Routes does Responding {
    Routes(routes is %Route:Responding);
    async response(request is Request, response is Response) is Response;
}
```

## - [ ] net/http/endpoint

No `HttpServer`.

```okkama
object HttpEndpoint {
    HttpEndpoint(endpoint is TcpEndpoint, resource is Responding);
    async serve();
    close();
}
```

> v2 implementation note: header parsing must use the native bulk path
> (single big read + native `string.split` on the `"\r\n\r\n"` boundary,
> longest-valid-UTF-8-prefix decode) — the per-byte and per-element-closure
> versions were both measured slower on this interpreter (2.3x difference;
> see perf records).

## - [ ] net/http/static

```okkama
object StaticFiles does Responding {
    StaticFiles(root is Directory);
    async response(request is Request, response is Response) is Response;
}
```

## - [ ] net/http/json-response

```okkama
object JsonResponse does Projecting<Response> {
    JsonResponse(status is Status, value is JsonRepresenting);
    value() is Response;
}
```

## - [ ] net/http/redirect

```okkama
object Redirect does Projecting<Response> {
    Redirect(status is Status, location is Url);
    value() is Response;
}
```

## - [ ] net/http/auth

```okkama
object BasicCredentials {
    BasicCredentials(username is string, password is string);
    text() is string;
}

object BearerToken {
    BearerToken(token is string);
    text() is string;
}
```

## - [ ] net/http/proxy

```okkama
object ProxiedResponse does Projecting<Response> {
    ProxiedResponse(request is Request, proxy is Url);
    async value() is Response;
}
```

## - [ ] net/websocket

> v2: `WebSocketMessage` defined (was referenced only) — a small role with a
> text and a binary carrier, so `send`/`receive` stay typed without flags.

```okkama
role WebSocketMessage {
    data() is @byte;
}

object TextMessage does WebSocketMessage {
    TextMessage(origin is string);
    data() is @byte;
    text() is string;
}

object DataMessage does WebSocketMessage {
    DataMessage(origin is @byte);
    data() is @byte;
}

object WebSocket {
    WebSocket(url is Url);

    async receive() is WebSocketMessage;     // throws ClosedWebSocket
    async send(message is WebSocketMessage);
    close();
}
```

## - [ ] net/robots

```okkama
object Robots {
    Robots(origin is string);

    allowed(agent is string, url is Url) is bool;
    crawl_delay(agent is string) is Duration;
    site_maps() is Sequencing<Url>;
}
```

## - [ ] net/jsonrpc

```okkama
object JsonRpcResponse does Projecting<JsonRepresenting> {
    JsonRpcResponse(endpoint is Url, method is string, parameters is JsonRepresenting);
    async value() is JsonRepresenting;       // throws JsonRpcFailure
}
```

---

# Stage 23 — database

> v2: **concrete engines are DEFERRED** — explicit user decision of
> 22.07.2026 (`docs/todo/db-sqlite-22072026.md`): a real `Sqlite` needs a
> byte-for-byte file-format engine (pager, B-tree, WAL, locking) plus a real
> SQL executor; `MemorySqlite` needs the same executor. Each is a multi-week
> project. Contracts, `SqlQuery`, `Rows`/`Row`, `Transaction`, `Effect` and
> the Stage 27 `FakeDatabase` stay in 1.0 so application code and tests can
> be written against the role now. Do not silently start an engine without
> re-confirming scope.

## - [ ] database/sql/querying

> v2: role renamed to the recorded best candidate `Querying`
> (was: role `Database`; remaining candidates: Persisting, DataAccessing,
> Transacting, Storing).

```okkama
role Querying {
    rows(query is SqlQuery) is Rows;
    transaction() is Transaction;
    execute(query is SqlQuery) is Effect;
}
```

## - [ ] database/sql/query

```okkama
object SqlQuery {
    SqlQuery(sql is string, parameters is @SqlValue);

    text() is string;
    parameters() is @SqlValue;
}
```

## - [ ] database/sql/rows

```okkama
object Rows does Iterating<Row> {
    cursor() is Cursor<Row>;
}
```

## - [ ] database/sql/row

```okkama
object Row {
    contains(column is string) is bool;
    value(column is string) is SqlValue;     // throws MissingColumn
    columns() is @string;
}
```

## - [ ] database/sql/transaction

```okkama
object Transaction does Querying {
    rows(query is SqlQuery) is Rows;
    transaction() is Transaction;
    execute(query is SqlQuery) is Effect;
    commit();
    rollback();
}
```

## - [ ] database/sql/effect

```okkama
object Effect {
    affected() is int;
    inserted_id() is int;                    // throws NoInsertedId
}
```

## - [ ] database/sqlite — DEFERRED (see stage note)

```okkama
object Sqlite does Querying {
    Sqlite(file is File);
    ...
}

object MemorySqlite does Querying { ... }
```

- [ ] Keep PostgreSQL/MySQL drivers third-party initially.

---

# Stage 24 — config / CLI / terminal

## - [ ] config/environment

```okkama
object EnvironmentConfig {
    EnvironmentConfig(environment is Environment, prefix is string);

    contains(name is string) is bool;
    text(name is string) is string;          // throws MissingKey
    int(name is string) is int;              // throws MissingKey, InvalidInteger
}
```

## - [ ] config/toml

```okkama
object TomlConfig {
    TomlConfig(document is TomlDocument);

    contains(name is string) is bool;
    text(name is string) is string;
    int(name is string) is int;
}
```

## - [ ] config/ini

```okkama
object IniConfig {
    IniConfig(document is IniDocument);

    contains(name is string) is bool;
    text(name is string) is string;
    int(name is string) is int;
}
```

## - [ ] cli/arguments

```okkama
object Arguments does Sequencing<string> {
    Arguments();

    at(index is int) is string;
    first() is string;
    last() is string;
    size() is int;
    empty() is bool;
    cursor() is Cursor<string>;
}
```

## - [ ] cli/options

```okkama
object Options {
    Options(arguments is Arguments, specification is OptionSpecification);

    contains(name is string) is bool;
    value(name is string) is string;         // throws MissingOption
    rest() is @string;
}
```

## - [ ] cli/command

```okkama
role Executing {
    run(arguments is Arguments) is Exit;
}
```

Concrete objects should be domain commands such as `Compile`, `Test`, `Format`, never `CommandHandler`.

## - [ ] terminal

```okkama
object Terminal {
    width() is int;
    height() is int;
    interactive() is bool;
    password(prompt is string) is string;
}
```

## - [ ] terminal/styled

```okkama
object Bold { Bold(origin is string); text() is string; }
object Dim { Dim(origin is string); text() is string; }
object Underlined { Underlined(origin is string); text() is string; }
object Red { Red(origin is string); text() is string; }
object Green { Green(origin is string); text() is string; }
object Yellow { Yellow(origin is string); text() is string; }
object Blue { Blue(origin is string); text() is string; }
```

---

# Stage 25 — logging

## - [ ] log/record

> v2: `%string:any` -> plain `%string:string` (v2 first tried
> `TextRepresenting`; v2.2 made it strings — values are values). `any` was one
> of only two typed-hole escapes in the whole plan; everything a log renders
> ends up as text anyway. `enum Level` defined (was referenced only).

```okkama
enum Level = (debug, info, warning, error);

object Record {
    Record(level is Level, message is string, fields is %string:string);
    text() is string;
}
```

## - [ ] log/log

```okkama
role Logging {
    add(record is Record) is void;
}
```

## - [ ] log/text

```okkama
object TextLog does Logging {
    TextLog(output is Stdout);
    add(record is Record);
}
```

## - [ ] log/json

```okkama
object JsonLog does Logging {
    JsonLog(output is Stdout);
    add(record is Record);
}
```

## - [ ] log/file

```okkama
object FileLog does Logging {
    FileLog(file is File);
    add(record is Record);
}
```

## - [ ] log/filtered

```okkama
object FilteredLog does Logging {
    FilteredLog(origin is Logging, minimum is Level);
    add(record is Record);
}
```

## - [ ] log/prefixed

```okkama
object PrefixedLog does Logging {
    PrefixedLog(origin is Logging, prefix is string);
    add(record is Record);
}
```

## - [ ] log/rotating

```okkama
object RotatingLog does Logging {
    RotatingLog(origin is FileLog, maximum is int, backups is int);
    add(record is Record);
}
```

---

# Stage 26 — templates / locale

## - [ ] template/text

> v2: `%string:any` -> plain `%string:string` (same evolution as `log/record`
> — templates render text; the typed hole bought nothing).

```okkama
object Template {
    Template(source is string, values is %string:string);
    text() is string;                        // throws MissingValue
}
```

## - [ ] template/html

```okkama
object HtmlTemplate {
    HtmlTemplate(source is string, values is %string:string);
    text() is string;                        // automatic escaping
}
```

## - [ ] locale

> v2 scope note: no ICU/CLDR in 1.0 (see Stage 35). Locale data is a small
> built-in table (decimal separator, digit grouping, currency placement) for
> a curated set of locales; unknown locale is a typed error, not a fallback.

```okkama
object Locale {
    Locale(origin is string);      // throws UnknownLocale
    text() is string;
}
```

## - [ ] locale/number

```okkama
object LocalInteger {
    LocalInteger(value is int, locale is Locale);
    text() is string;
}

object LocalDecimal {
    LocalDecimal(value is Decimal, locale is Locale);
    text() is string;
}
```

## - [ ] locale/currency

```okkama
object LocalCurrency {
    LocalCurrency(amount is Decimal, currency is string, locale is Locale);
    text() is string;
}
```

## - [ ] locale/messages

> v2: plural handling is explicit-forms-per-catalog (each catalog entry lists
> its language's forms), not a CLDR plural-rules engine.

```okkama
object Messages {
    Messages(locale is Locale, catalog is MessageCatalog);

    text(key is string) is string;           // throws MissingMessage
    plural(key is string, amount is int) is string;
}
```

---

# Stage 27 — testing

## - [ ] testing/assertions

Prefer language-native test assertions over an `Assert` helper.

- [ ] equality assertion
- [ ] boolean assertion
- [ ] ordering assertion
- [ ] typed-error assertion
- [ ] no-error assertion

## - [ ] testing/fake-clock

```okkama
object FakeClock does Timing {
    FakeClock(now is DateTime, monotonic is Instant);

    now() is DateTime;
    monotonic() is Instant;
}
```

Time evolution must use successors, not setters.

## - [ ] testing/fake-database

```okkama
object FakeDatabase does Querying {
    FakeDatabase(rows is %SqlQuery:Rows);

    rows(query is SqlQuery) is Rows;
    transaction() is Transaction;
    execute(query is SqlQuery) is Effect;
}
```

## - [ ] testing/fake-log

```okkama
object FakeLog does Logging {
    add(record is Record);
}
```

Do not introduce shared mutable in-memory storage solely to make this fake work; use scenario/predefined objects if necessary.

## - [ ] testing/http-resource

```okkama
object StaticResource does Responding {
    StaticResource(response is Response);
    async response(request is Request, response is Response) is Response;
}
```

## - [ ] testing/property

> v2: reproducibility is a hard contract — a failing property must be
> re-runnable from its reported seed byte-for-byte (depends on the `Random`
> determinism guarantee added in Stage 09).

```okkama
role Sampling<T> does Iterating<T> {}

object RandomInts does Sampling<int> {
    RandomInts(random is Random, count is int, minimum is int, maximum is int);
    cursor() is Cursor<int>;
}

object Property<T> does Projecting<bool> {
    Property(samples is Sampling<T>, condition is (T) is bool);
    value() is bool;
}
```

## - [ ] testing/benchmark

```okkama
object Measurement {
    duration() is Duration;
    iterations() is int;
}

object Benchmark does Projecting<Measurement> {
    Benchmark(action is () is int, iterations is int, clock is Timing);
    value() is Measurement;
}
```

## - [ ] testing/fuzz

```okkama
object FuzzCases<T> does Iterating<T> {
    FuzzCases(seed is T, mutation is (T) is T, count is int);
    cursor() is Cursor<T>;
}
```

---

# Stage 28 — diagnostics / observability

## - [ ] diagnostics/stack

```okkama
object StackTrace does Iterating<StackFrame> {
    text() is string;
    cursor() is Cursor<StackFrame>;
}
```

## - [ ] diagnostics/cpu-profile

```okkama
object CpuProfile {
    CpuProfile(duration is Duration);
    bytes() is @byte;
}
```

## - [ ] diagnostics/memory-profile

```okkama
object MemoryProfile {
    bytes() is @byte;
}
```

## - [ ] diagnostics/timing

```okkama
object TimedAction does Projecting<Duration> {
    TimedAction(action is () is int, clock is Timing);
    value() is Duration;
}
```

## - [ ] diagnostics/warning

```okkama
object Warning does Error {
    text() is string;
    code() is int;
}
```

## - [ ] metrics

> v2: role renamed to the recorded best candidate `Measuring` (was: role
> `Metric`; remaining candidates: Observing, Instrumenting, Reporting,
> Recording). `Gauge.set` renamed `report` — `set` is literally the forbidden
> setter vocabulary; the semantics (instrument records an observation) were
> never a setter anyway. These are instruments/resources, not worker objects
> — the resource-view rule covers their changing readings.

```okkama
role Measuring {
    name() is string;
}

object Counter does Measuring {
    name() is string;
    increment();
    add(value is int);
}

object Gauge does Measuring {
    name() is string;
    report(value is float);
}

object Histogram does Measuring {
    name() is string;
    observe(value is float);
}
```

## - [ ] tracing/span

Keep tracing API intentionally small until semantics are concrete.

```okkama
object Span {
    child(name is string) is Span;
    event(name is string);
    finish();
}
```

---

# Stage 29 — image / audio

## - [ ] image

> naming candidates, best -> worst: Imaging, PixelProviding,
> RasterRepresenting, Picturing, Rendering — noun kept deliberately: an image
> *is* a raster of pixels; "Imaging" describes a profession.

```okkama
role Image {
    bounds() is Rectangle;
    pixel(point is Point) is Color;
}
```

## - [ ] image/png

```okkama
object Png {
    Png(origin is Image);
    bytes() is @byte;
}

object PngImage does Image {
    PngImage(origin is @byte);    // throws InvalidPng (on first access)
    bounds() is Rectangle;
    pixel(point is Point) is Color;
}
```

## - [ ] image/jpeg

```okkama
object Jpeg {
    Jpeg(origin is Image, quality is int);
    bytes() is @byte;
}

object JpegImage does Image {
    JpegImage(origin is @byte);
    bounds() is Rectangle;
    pixel(point is Point) is Color;
}
```

## - [ ] image/gif

```okkama
object Gif {
    Gif(origin is Image);
    bytes() is @byte;
}

object GifImage does Image {
    GifImage(origin is @byte);
    bounds() is Rectangle;
    pixel(point is Point) is Color;
}
```

## - [ ] image/color

```okkama
object RGB {
    red() is int;
    green() is int;
    blue() is int;
}

object RGBA {
    red() is int;
    green() is int;
    blue() is int;
    alpha() is int;
}
```

## - [ ] audio/wav

> v2: role keeps the noun `Audio` — the recorded best candidate `Sampling`
> now collides with `testing/property`'s `Sampling<T>` role (unique-names
> rule), and the rest (AudioRepresenting, SoundRepresenting, Sounding) don't
> beat the noun. `AudioSample` defined (v1's `Sample` collided with
> `random/distributions.Sample<T>`).

```okkama
object AudioSample {
    channel(index is int) is float;          // -1.0 .. 1.0
}

role Audio {
    samples() is Iterating<AudioSample>;
    rate() is int;
    channels() is int;
}

object Wav {
    Wav(audio is Audio);
    bytes() is @byte;
}

object WavAudio does Audio {
    WavAudio(origin is @byte);    // throws InvalidWav (on first access)
    samples() is Iterating<AudioSample>;
    rate() is int;
    channels() is int;
}
```

---

# Stage 30 — reflection / modules / versions

## - [ ] reflection policy

- [ ] No general runtime reflection in stdlib 1.0.
- [ ] No private-field inspection.
- [ ] No invocation by string.
- [ ] No mutation through reflection.
- [ ] Compiler tooling may inspect source/compiler models, not live object internals.

## - [ ] module/resource

> v2: renamed `Responding` -> `ModuleResource` — v1 accidentally reused the
> HTTP role's name for an unrelated object. Note the overlap with Stage 33
> `embed`: consider merging (embed = compile-time-embedded, module/resource =
> shipped-with-package); decide before implementing either.

```okkama
object ModuleResource {
    ModuleResource(module is Module, path is Path);

    bytes() is @byte;                        // throws MissingResource
    text() is string;
}
```

## - [ ] module/metadata

```okkama
object ModuleMetadata {
    name() is string;
    version() is Version;
    dependencies() is Sequencing<Dependency>;
}
```

## - [ ] version

```okkama
object Version does Comparing<Version> {
    Version(origin is string);     // throws InvalidVersion

    compared(other is Version) is Ordering;
    text() is string;
}
```

## - [ ] version/constraint

```okkama
object VersionConstraint {
    VersionConstraint(origin is string);   // throws InvalidConstraint

    matches(version is Version) is bool;
    text() is string;
}
```

---

# Stage 31 — big numbers / FFI

## - [ ] numbers/bigint

```okkama
object BigInt does Comparing<BigInt> {
    BigInt(origin is string);      // throws InvalidInteger

    plus(other is BigInt) is BigInt;
    minus(other is BigInt) is BigInt;
    multiplied(other is BigInt) is BigInt;
    divided(other is BigInt) is BigInt;      // throws DivisionByZero
    compared(other is BigInt) is Ordering;
    text() is string;
}

object BigPower does Projecting<BigInt> {
    BigPower(base is BigInt, exponent is BigInt);
    value() is BigInt;
}

object BigGcd does Projecting<BigInt> {
    BigGcd(left is BigInt, right is BigInt);
    value() is BigInt;
}
```

## - [ ] numbers/bigrational

```okkama
object BigRational does Comparing<BigRational> {
    BigRational(numerator is BigInt, denominator is BigInt);  // throws DivisionByZero

    plus(other is BigRational) is BigRational;
    minus(other is BigRational) is BigRational;
    multiplied(other is BigRational) is BigRational;
    divided(other is BigRational) is BigRational;
    compared(other is BigRational) is Ordering;
    text() is string;
}
```

> v2: `Caching<K,V>` REMOVED from 1.0 — its `put(key, value)` was a mutating
> method contradicting the core rules, and v1 itself hedged on whether the
> whole thing belongs in the language. Decision made instead of deferred:
> out of core (recorded in Stage 35). If it ever returns, `put` must be
> non-destructive (`put(key, value) is Caching<K,V>`, like `Containing.added`)
> or the cache must be an external process resource.

## - [ ] ffi/library

```okkama
object Library {
    Library(path is Path);

    symbol(name is string) is ForeignFunction;   // throws MissingSymbol
    close();
}
```

## - [ ] ffi/function

```okkama
object ForeignFunction {
    call(arguments is @ForeignValue) is ForeignValue;
}
```

## - [ ] ffi/pointer

```okkama
object Pointer<T> {
    address() is int;
    value() is T;
}
```

- [ ] Keep raw mutation out until memory/FFI semantics are deliberately designed.

---

# Stage 32 — Okkama language tooling

## - [ ] okkama/source

> v2: `Span` -> `SourceSpan` — v1's name collided with tracing's `Span`
> (unique-names rule; the compiler one renames because "span of source" needs
> the qualifier anyway).

```okkama
object Source {
    Source(path is Path);
    text() is string;                        // throws MissingFile
}

object Position {
    file() is Path;
    line() is int;
    column() is int;
}

object SourceSpan {
    start() is Position;
    end() is Position;
}
```

## - [ ] okkama/tokens

No Lexer.

```okkama
object Tokens does Sequencing<Token> {
    Tokens(source is Source);

    at(index is int) is Token;
    first() is Token;
    last() is Token;
    size() is int;
    empty() is bool;
    cursor() is Cursor<Token>;
}
```

## - [ ] okkama/program

No Parser.

```okkama
object Program {
    Program(tokens is Tokens);

    modules() is Sequencing<ModuleNode>;
    diagnostics() is Sequencing<Diagnostic>;
}
```

## - [ ] okkama/typed

No TypeChecker worker.

```okkama
object TypedProgram {
    TypedProgram(origin is Program);

    program() is Program;
    diagnostics() is Sequencing<Diagnostic>;
}
```

## - [ ] okkama/formatted

No Formatter.

```okkama
object FormattedSource {
    FormattedSource(origin is Source);
    text() is string;
}
```

## - [ ] okkama/documentation

```okkama
object Documentation {
    Documentation(program is Program);
    text() is string;
}
```

## - [ ] okkama/bytecode

No generic Compiler worker.

```okkama
object Bytecode {
    Bytecode(program is TypedProgram);
    bytes() is @byte;
}
```

## - [ ] okkama/disassembly

```okkama
object Disassembly {
    Disassembly(bytecode is Bytecode);
    text() is string;
}
```

---

# Stage 33 — project / package / embed

## - [ ] okkama/project

```okkama
object Project {
    Project(root is Directory);

    name() is string;
    version() is Version;
    modules() is Sequencing<Module>;
}
```

## - [ ] okkama/package

```okkama
object Package {
    Package(project is Project);
    bytes() is @byte;
}
```

## - [ ] okkama/build

No Builder.

```okkama
object Build does Projecting<BuildResult> {
    Build(project is Project, target is BuildTarget);
    value() is BuildResult;
}
```

## - [ ] embed

```okkama
object EmbeddedFile {
    EmbeddedFile(path is string);

    bytes() is @byte;                        // throws MissingResource
    text() is string;
}

object EmbeddedDirectory does Iterating<EmbeddedResource> {
    EmbeddedDirectory(path is string);
    cursor() is Cursor<EmbeddedResource>;
}
```

---

# Stage 34 — platform-specific

## - [ ] os/unix

- [ ] UID/GID objects.
- [ ] POSIX permissions.
- [ ] resource limits.
- [ ] pipes only after a proper domain model exists.
- [ ] raw `fork()` only after Okkama process/async semantics are final.
- [ ] raw `ioctl()` stays FFI/platform-low-level, not general stdlib.

## - [ ] os/windows/registry

```okkama
object RegistryKey {
    RegistryKey(root is RegistryRoot, path is string);

    contains(name is string) is bool;
    value(name is string) is RegistryValue;  // throws MissingValue
    close();
}
```

---

# Stage 35 — deliberately excluded from core stdlib

- [ ] NO pickle-style arbitrary object serialization.
- [ ] NO shelve/dbm/marshal runtime-object persistence.
- [ ] NO generic utility modules (`utils`, `helpers`, `common`).
- [ ] NO generic Reader/Writer/Parser/Encoder/Decoder abstractions.
- [ ] NO ORM / ActiveRecord.
- [ ] NO reflection-based DI.
- [ ] NO mocking framework.
- [ ] NO FTP/Telnet/NNTP/POP in core initially.
- [ ] NO GUI toolkit in core initially.
- [ ] NO YAML in core.
- [ ] NO PostgreSQL/MySQL drivers in core initially.
- [ ] NO mutable synchronization zoo until Okkama concurrency semantics demand it.

> v2 additions:

- [ ] NO ICU/CLDR engine — locale data is a small curated built-in table; plural forms are explicit per catalog (Stage 26).
- [ ] NO codecs registry — UTF-8/UTF-16 objects only; other encodings are third-party (Stages 06/11/12).
- [ ] NO generic text tokenizer (`text/tokens` removed; Okkama's own tokens are Stage 32).
- [ ] NO in-memory `Caching` role in 1.0 (removed from Stage 31; a future cache is an external/process resource or a non-destructive persistent structure).
- [ ] NO stream-buffering decorators over value-semantics byte data (`data/buffered` removed; measured slower, semantically empty).
- [ ] NO representation roles — text is `string`, binary is `@byte`; `repr/text` and `repr/byte` removed in v2.2 (values are values).

---

# Stage 36 — coverage checklist

## Python breadth

- [ ] text processing
- [ ] binary data
- [ ] numeric/math
- [ ] iteration/combinatorics
- [ ] collections
- [ ] time/date/calendar/timezone
- [ ] filesystem/path/temp/glob/watch/mmap
- [ ] OS/environment/users/process/signals
- [ ] compression
- [ ] archives
- [ ] SQLite/database contracts
- [ ] JSON/CSV/TOML/INI/XML/HTML
- [ ] base encodings/binary encodings/PEM/ASN.1
- [ ] checksums/digests/crypto
- [ ] DNS/IP/TCP/UDP/Unix sockets
- [ ] URL/MIME/mail/SMTP
- [ ] HTTP/WebSocket/robots/RPC
- [ ] async/concurrency
- [ ] CLI/terminal
- [ ] logging
- [ ] config
- [ ] templates
- [ ] locale/i18n
- [ ] testing/property/fuzz/benchmark
- [ ] diagnostics/profiling/metrics/tracing
- [ ] image/audio basics
- [ ] resources/module metadata/versioning
- [ ] FFI/platform
- [ ] language/compiler tooling

## Go structure

- [ ] cohesive packages
- [ ] package paths generally <= 2 levels
- [ ] small cross-package contracts
- [ ] OS-specific APIs isolated
- [ ] testing support close to domain
- [ ] source/compiler models first-class
- [ ] no package becomes a dumping ground
- [ ] adjacent packages compose without orchestration objects

## EO review for every public object

- [ ] Named by what it **is**, not what it **does**.
- [ ] No avoidable `-er/-or` job-title name.
- [ ] Declarative object possible instead? (`Sorter -> Sorted`, `FileReader -> DataFile`)
- [ ] No utility/static/free procedural API.
- [ ] Immutable (or an honest resource view — never a mutable value).
- [ ] Constructor only binds.
- [ ] No getters/setters as exposed state plumbing.
- [ ] Total attribute -> method; partial/computed -> `Projecting` object (v2 rule).
- [ ] Inputs are native values: text parameters are `string`, binary parameters are `@byte` — no representation roles (v2.2 rule).
- [ ] Behavior, not data-bag design.
- [ ] Small public surface.
- [ ] Contract exists where polymorphism matters.
- [ ] Decorators preferred over flags/settings.
- [ ] Typed, fail-fast errors.
- [ ] No hidden null/optional assumption.
- [ ] Fakes instead of mocks.
- [ ] No implementation inheritance.
- [ ] No runtime reflection/casting.
- [ ] Composition instead of orchestration.
- [ ] Hot paths route through native bulk operations, verified by A/B benchmark (v2 rule).
- [ ] Name unique across the whole stdlib (v2 rule).

---

# Definition of done per item

- [ ] EO naming review
- [ ] API contract approved
- [ ] implementation
- [ ] positive tests
- [ ] boundary tests
- [ ] typed-error tests
- [ ] immutability tests
- [ ] composition test with adjacent stdlib object
- [ ] Perl-level regression test in `okkama-pl/t/interpreter/` (v2 — matches the established workflow; embedded `test` blocks can't exercise cross-module imports, see `docs/todo/` note of 25.07.2026)
- [ ] runnable example
- [ ] minimal docs
- [ ] no duplicate responsibility elsewhere
