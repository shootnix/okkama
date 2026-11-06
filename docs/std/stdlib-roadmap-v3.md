# Okkama Standard Library — v3 workbook

> Historical scope. Current design: [roadmap v4](stdlib-roadmap-v4.md).
> These signatures and checkboxes are not the current std contract. See the
> [completion guide](stdlib-completion-todo-v4.md) for remaining work and design gates.

> Greenfield standard-library plan. This file is an implementation workbook,
> not an API essay. Complete one checkbox at a time from top to bottom.

## Explicit results in the current interpreter

Every action signature below explicitly declares a value result or `is void`.
Synchronous manipulators use `is void`; their invocation cannot supply a value.
Async manipulators are a temporary compatibility exception to the no-result
rule: `async void` is not supported, so they explicitly return their receiver
or implemented role. Implementations must end with an explicit `self` value.
This does not authorize value-returning synchronous command APIs or introduce
`Future<void>`. Planned async entries retain this limitation until a separate
resultless-completion design is adopted. Constructors are not actions and
keep their existing declaration syntax.

## Working rules

- [x] Python-level breadth is the coverage target.
- [x] Go-like cohesive modules are the structural target.
- [x] Elegant Objects shapes public APIs.
- [x] Domain sense wins over mechanical architecture.
- [x] Roles and objects name entities, never procedures or utility namespaces.
- [x] Every public object or role has at most five public action names.
- [x] Constructors do not count toward the five-action limit.
- [x] Inherited role actions do count toward the five-action limit.
- [x] Builtin primitive actions do not count toward the object limit.
- [x] Every object has one primary constructor that only binds encapsulated objects.
- [x] Secondary constructors only delegate to the primary constructor.
- [x] Constructors perform no validation, I/O, branching or computation.
- [x] Value objects are immutable.
- [x] Resources may expose changing observations and explicit lifecycle effects.
- [x] Immutable values may memoize observationally pure results.
- [x] Text is `string`; binary data is `@byte`.
- [x] There are no generic text/binary representation roles.
- [x] There is no generic `Projecting<T>` role.
- [x] A builtin conversion may return only a builtin runtime type.
- [x] Stdlib types never augment primitive receivers.
- [x] `int.seconds()`, `string.path()` and `string.uuid()` are forbidden.
- [x] Typed errors fail fast.
- [x] Effects belong to resources or explicit effect/result objects.
- [x] A builder returns a value, has a noun name and does not manipulate its represented entity.
- [x] A manipulator changes its represented entity, has a verb name and returns no value.
- [x] No action both manipulates and returns a value.
- [x] Boolean builders use a noun or an established predicate adjective, never an imperative verb.
- [x] Every object encapsulates at least one meaningful object; zero-argument atoms represent explicit domain values or policies, never utility namespaces.
- [x] Static methods, stateless algorithm roles and procedure-holder objects are forbidden.
- [x] Cursor traversal never blocks on external events.
- [x] Native bulk paths back hot collection and text operations.
- [x] Public names must be unambiguous within their module.
- [x] Public type names are unique across the entire stdlib.
- [x] Tests use fakes or real boundaries, not interaction-driven mocks.

## Definition of done for every module

- [ ] Approve every public signature.
- [ ] Verify every value-returning action is a noun builder with no effects.
- [ ] Verify every effectful action is a void manipulator named by a verb.
- [ ] Verify every secondary constructor delegates to one code-free primary constructor.
- [ ] Reject every static algorithm role, utility namespace and procedure-holder object.
- [ ] Implement every checked type and action.
- [ ] Add positive focused tests.
- [ ] Add boundary and empty-input tests.
- [ ] Add every declared typed-error test.
- [ ] Add immutability or resource-lifecycle tests.
- [ ] Add one composition test with an adjacent module.
- [ ] Add one Perl-level regression test in `okkama-pl/t/interpreter/`.
- [ ] Add a runnable example.
- [ ] Add minimal public documentation.
- [ ] Run `git diff --check`.

## Подробные руководства

- [Коллекции](../collections-guide.md): итерация, курсоры, ленивые views,
  persistent-структуры, сортировка и комбинаторика.
- [Ввод-вывод](../io-guide.md): байтовые и текстовые потоки, stdin/stdout,
  буферы и декораторы.
- [Файловая система](../fs-guide.md): пути, потоковые файлы, блокировки,
  каталоги, glob, ссылки, временные ресурсы и watch.
- [Операционная система](../os-guide.md): окружение, платформа, аккаунты,
  команды, процессы, capture, pipeline и storage.
- [Время](../time-guide.md): duration, часы, civil time, зоны, ISO и RFC 3339.
- [Async и concurrency](../async-guide.md): futures, cancellation,
  structured concurrency, каналы и синхронизация.

---

# Stage 00 — language substrate

## `core/error`

- [x] role `Error`
  - [x] `text() is string`
- [x] Treat `Error` as a runtime role; do not redeclare it in `std/error.ok`.
- [x] Remove `import "error" take Error` from stdlib modules; the role is always in scope.
- [x] object `InvalidArgument` does `Error`
  - [x] `InvalidArgument(message is string)`
  - [x] `text() is string`
- [x] object `InvalidState` does `Error`
  - [x] `InvalidState(message is string)`
  - [x] `text() is string`
- [x] object `OutOfRange` does `Error`
  - [x] `OutOfRange(message is string)`
  - [x] `text() is string`
- [x] object `UnsupportedOperation` does `Error`
  - [x] `UnsupportedOperation(message is string)`
  - [x] `text() is string`
- [x] Remove universal numeric `Error.code()`.
- [x] Keep protocol codes on protocol values such as `Exit` and HTTP `Status`.
- [x] Do not expose `NotImplemented` as stable stdlib behavior.

## `builtin/conversion`

- [x] integer conversions
  - [x] `int.string() is string`
  - [x] `int.float() is float`
  - [x] `int.byte() is byte`
- [ ] floating-point conversions
  - [x] `float.string() is string`
  - [x] Specify precision loss for `int.float()`.
  - [?] Do not add ambiguous `float.int()`; use explicit rounding behavior.
- [x] byte conversions
  - [x] `byte.int() is int`
- [ ] character conversions
  - [ ] `char.string() is string`
  - [ ] `char.codepoint() is int`
- [x] boolean conversions
  - [x] `bool.string() is string`
  - [x] Canonical results are `"true"` and `"false"`.
- [ ] Do not add `byte.char()` or `char.byte()`.
- [ ] Add a fallible character-from-codepoint stdlib value only when needed.

## `builtin/array`

- [x] `@T.size() is int`
- [ ] `@T.empty() is bool`
- [x] `@T.item(index is int) is T`
- [x] `@T.slice(start is int, end is int) is @T`
- [?] `@T.concatenation(other is @T) is @T`
- [x] Native `@T` satisfies `Sequencing<T>`.
- [ ] Slices use strict half-open `[start, end)` bounds.
- [ ] Negative indexes are rejected consistently.
- [ ] Equality is structural and ordered.

## `builtin/map`

- [x] `%K:V.size() is int`
- [ ] `%K:V.empty() is bool`
- [x] `%K:V.membership(key is K) is bool`
- [?] `%K:V.value(key is K) is V`
- [x] `%K:V.keys() is @K`
- [x] `%K:V.values() is @V`
- [ ] Native `%K:V` satisfies `Iterating<K>` and `Sizing`.
- [ ] Preserve insertion order.
- [ ] Equality compares key sets and values independently of insertion order.
- [ ] Document admissible key categories.

---

# Stage 01 — tiny contracts

## `core/compare`

- [x] enum `Order`
  - [x] `less`
  - [x] `equal`
  - [x] `greater`
- [x] role `Comparing<T>`
  - [x] `order(other is T) is Order`
- [x] object generic bounds
  - [x] Parse `object Value<T: Comparing<T>>`.
  - [x] Retain bounds in AST and module dependency closure.
  - [x] Validate that every bound names a role.
  - [x] Substitute inferred parameters and enforce the bound before construction.
  - [x] Reject bounded role parameters until role-specialization enforcement exists.
- [x] builtin values satisfying `Comparing<Self>`
  - [x] `int`, `float`, `rat`, `byte`
  - [x] `string`, ordered lexically
  - [x] `char`, ordered by scalar value
  - [x] Keep comparisons exact; do not widen numeric categories.
  - [x] Do not give `bool` a natural order.
- [x] Keep comparison intrinsic to a value or an explicit value decorator.
- [x] Remove the stateless binary-function role `Ordering<T>`.

## `core/size`

- [x] role `Sizing`
  - [x] `size() is int`
- [x] Use `value.size() == 0` instead of duplicating `empty()` on objects.

---

# Stage 02 — iteration and sequences

## `collections/iteration`

- [x] role `Cursor<T>`
  - [x] `available() is bool`
  - [x] `item() is T`
  - [x] `successor() is Cursor<T>`
- [x] role `Iterating<T>`
  - [x] `cursor() is Cursor<T>`
- [x] object `CursorItems<T>` does `Iterating<T>`
  - [x] `CursorItems(cursor is Cursor<T>)`
  - [x] `cursor() is Cursor<T>`
- [x] object `IterationEnded` does `Error`
  - [x] `IterationEnded()`
  - [x] `text() is string`
- [ ] Keep cursors immutable.
- [ ] Make exhausted `available() == false` normal completion.
- [ ] Make `item()`/`successor()` on exhaustion throw `IterationEnded`.
- [ ] Do not use cursor iteration for sockets, watches or event waits.
- [x] Make cursor normalization stack-safe for decorators that skip arbitrary runs, such as empty sources in `Chained`.
- [x] Migrate built-in `for` from `Iterable/current/advanced` to `Iterating/item/successor`.
  - [x] Migrate synchronous object traversal.
  - [x] Migrate asynchronous object traversal.
  - [x] Preserve `next` advancement and `last` short-circuit behavior.
  - [x] Require the nominal `collections/iteration.Iterating<T>` role.
- [x] Migrate bulk `map`/`filter`/`reduce` from `Iterable/current/advanced` to `Iterating/item/successor`.
- [x] Infer generic constructor parameters from typed action-value signatures.

## `collections/sequence`

- [x] role `Sequencing<T>`
  - [x] `cursor() is Cursor<T>`
  - [x] `size() is int`
  - [x] `item(index is int) is T`
  - [x] `first() is T`
  - [x] `last() is T`
- [ ] Add language support for role composition before declaring `Sequencing<T> does Iterating<T>, Sizing`.
- [x] Until role composition exists, require concrete sequences to explicitly do `Sequencing<T>, Iterating<T>, Sizing`.
- [x] object `EmptySequence` does `Error`
  - [x] `text() is string`
- [ ] Make `first()`/`last()` throw `EmptySequence` on empty input.

## `builtin/range`

- [ ] Native integer ranges satisfy `Sequencing<int>`.
- [ ] Stop is exclusive.
- [ ] Step may be negative.
- [ ] Zero step throws `InvalidArgument`.
- [ ] Compute `size()` without traversal.
- [ ] Remove the duplicate stdlib `Range` wrapper.

## `collections/enumeration`

- [x] object `Indexed<T>`
  - [x] `Indexed(index is int, item is T)`
  - [x] `index() is int`
  - [x] `item() is T`
- [x] object `Enumerated<T>` does `Iterating<Indexed<T>>`
  - [x] `Enumerated(origin is Iterating<T>)`
  - [x] `Enumerated(origin is Iterating<T>, start is int)`
  - [x] `cursor() is Cursor<Indexed<T>>`
- [x] object `EnumerationCursor<T>` does `Cursor<Indexed<T>>`
  - [x] `EnumerationCursor(origin is Cursor<T>, position is int)`
  - [x] `available() is bool`
  - [x] `item() is Indexed<T>`
  - [x] `successor() is Cursor<Indexed<T>>`
- [x] object `EnumerationOverflow` does `Error`
  - [x] `EnumerationOverflow(index is int)`
  - [x] `index() is int`
  - [x] `text() is string`
- [x] Keep enumeration cursors immutable.
- [x] Allow `int.max` for the last item and throw `EnumerationOverflow` only when another item exists.
- [x] Document zero-based, custom-start and immutable-cursor usage in the module.

## `collections/combining`

- [x] object `Pair<A,B>`
  - [x] `Pair(first is A, second is B)`
  - [x] `first() is A`
  - [x] `second() is B`
- [x] object `Zipped<A,B>` does `Iterating<Pair<A,B>>`
  - [x] `Zipped(left is Iterating<A>, right is Iterating<B>)`
  - [x] `cursor() is Cursor<Pair<A,B>>`
- [x] object `ZippedCursor<A,B>` does `Cursor<Pair<A,B>>`
  - [x] `ZippedCursor(left is Cursor<A>, right is Cursor<B>)`
  - [x] `available() is bool`
  - [x] `item() is Pair<A,B>`
  - [x] `successor() is Cursor<Pair<A,B>>`
  - [x] Stop at the shorter input.
- [x] object `StrictlyZipped<A,B>` does `Iterating<Pair<A,B>>`
  - [x] `StrictlyZipped(left is Iterating<A>, right is Iterating<B>)`
  - [x] `cursor() is Cursor<Pair<A,B>>`
- [x] object `StrictlyZippedCursor<A,B>` does `Cursor<Pair<A,B>>`
  - [x] `StrictlyZippedCursor(left is Cursor<A>, right is Cursor<B>)`
  - [x] `available() is bool`
  - [x] `item() is Pair<A,B>`
  - [x] `successor() is Cursor<Pair<A,B>>`
- [x] object `UnequalSizes` does `Error`
  - [x] `UnequalSizes()`
  - [x] `text() is string`
  - [x] Throw when only one strict-zip input has an item.
- [x] object `Chained<T>` does `Iterating<T>`
  - [x] `Chained(origins is Iterating<Iterating<T>>)`
  - [x] `cursor() is Cursor<T>`
- [x] object `ChainedCursorSearch<T>`
  - [x] `ChainedCursorSearch(origins is Cursor<Iterating<T>>)`
  - [x] `result() is Cursor<T>`
  - [x] Skip empty inner sources without reopening their cursors.
- [x] object `ChainedCursorCandidates<T>` does `Iterating<ChainedCursorCandidateValue<T>>`
  - [x] `ChainedCursorCandidates(origins is Cursor<Iterating<T>>)`
  - [x] `cursor() is Cursor<ChainedCursorCandidateValue<T>>`
- [x] object `ChainedCursorCandidate<T>` does `Cursor<ChainedCursorCandidateValue<T>>`
  - [x] `ChainedCursorCandidate(origins is Cursor<Iterating<T>>)`
  - [x] `available() is bool`
  - [x] `item() is ChainedCursorCandidateValue<T>`
  - [x] `successor() is Cursor<ChainedCursorCandidateValue<T>>`
- [x] object `ChainedCursorCandidateValue<T>`
  - [x] `ChainedCursorCandidateValue(origin is Iterating<T>, remaining is Cursor<Iterating<T>>)`
  - [x] `origin() is Iterating<T>`
  - [x] `remaining() is Cursor<Iterating<T>>`
- [x] object `ChainedItemCursor<T>` does `Cursor<T>`
  - [x] `ChainedItemCursor(current is Cursor<T>, remaining is Cursor<Iterating<T>>)`
  - [x] `available() is bool`
  - [x] `item() is T`
  - [x] `successor() is Cursor<T>`
- [x] object `ExhaustedChainedCursor<T>` does `Cursor<T>`
  - [x] `ExhaustedChainedCursor(origins is Cursor<Iterating<T>>)`
  - [x] `available() is bool`
  - [x] `item() is T`
  - [x] `successor() is Cursor<T>`
- [x] Keep all combining cursors immutable.
- [x] Normalize long runs of empty sources iteratively without consuming the host stack.
- [x] Document pair, ordinary zip, strict zip and chain use cases in the module.

## `collections/repetition`

- [x] object `Repeating<T>` does `Iterating<T>`
  - [x] `Repeating(value is T)`
  - [x] `cursor() is Cursor<T>`
- [x] object `RepeatingCursor<T>` does `Cursor<T>`
  - [x] `RepeatingCursor(value is T)`
  - [x] `available() is bool`
  - [x] `item() is T`
  - [x] `successor() is Cursor<T>`
  - [x] Return `self` from `successor()` because every observable state is identical.
  - [x] Document that it is infinite.
- [x] object `Repeated<T>` does `Sequencing<T>, Iterating<T>, Sizing`
  - [x] `Repeated(value is T, count is int)`
  - [x] `cursor() is Cursor<T>`
  - [x] `size() is int`
  - [x] `item(index is int) is T`
  - [x] `first() is T`
  - [x] `last() is T`
  - [x] Reject negative count with `InvalidArgument`.
  - [x] Reject indexes outside `[0, size())` with `OutOfRange`.
  - [x] Throw `EmptySequence` from `first()`/`last()` when count is zero.
- [x] object `RepeatedCursor<T>` does `Cursor<T>`
  - [x] `RepeatedCursor(value is T, position is int, count is int)`
  - [x] `available() is bool`
  - [x] `item() is T`
  - [x] `successor() is Cursor<T>`
  - [x] Validate public constructor invariants.
- [x] object `Cycled<T>` does `Iterating<T>`
  - [x] `Cycled(origin is Sequencing<T>)`
  - [x] `cursor() is Cursor<T>`
  - [x] Empty origin produces empty output.
  - [x] Non-empty origin produces infinite output.
- [x] object `CycledCursor<T>` does `Cursor<T>`
  - [x] `CycledCursor(origin is Sequencing<T>, position is int)`
  - [x] `available() is bool`
  - [x] `item() is T`
  - [x] `successor() is Cursor<T>`
  - [x] Wrap to index zero after the last item.
  - [x] Validate public constructor invariants.
- [x] object `EmptyCycleCursor<T>` does `Cursor<T>`
  - [x] `EmptyCycleCursor(origin is Sequencing<T>)`
  - [x] `available() is bool`
  - [x] `item() is T`
  - [x] `successor() is Cursor<T>`
  - [x] Validate that the origin is empty.
- [x] Keep all repetition cursors immutable.
- [x] Document finite repetition, infinite repetition and cycling use cases in the module.

---

# Stage 03 — iteration decorators

## `collections/views`

- [x] object `Mapped<A,B>` does `Iterating<B>`
  - [x] `Mapped(origin is Iterating<A>, mapping is (A) is B)`
  - [x] `cursor() is Cursor<B>`
  - [x] Cache one mapped value per cursor state.
- [x] object `Filtered<T>` does `Iterating<T>`
  - [x] `Filtered(origin is Iterating<T>, condition is (T) is bool)`
  - [x] `cursor() is Cursor<T>`
  - [x] Evaluate the condition once per inspected item.
  - [x] Skip long rejected runs without consuming the host stack.
- [x] object `Taken<T>` does `Iterating<T>`
  - [x] `Taken(origin is Iterating<T>, count is int)`
  - [x] `cursor() is Cursor<T>`
  - [x] Reject negative count.
- [x] object `Dropped<T>` does `Iterating<T>`
  - [x] `Dropped(origin is Iterating<T>, count is int)`
  - [x] `cursor() is Cursor<T>`
  - [x] Reject negative count.
  - [x] Return exhaustion when count exceeds source size.
- [x] object `TakenWhile<T>` does `Iterating<T>`
  - [x] `TakenWhile(origin is Iterating<T>, condition is (T) is bool)`
  - [x] `cursor() is Cursor<T>`
  - [x] Stop permanently at the first failed condition.
- [x] object `DroppedWhile<T>` does `Iterating<T>`
  - [x] `DroppedWhile(origin is Iterating<T>, condition is (T) is bool)`
  - [x] `cursor() is Cursor<T>`
  - [x] Resume at the first failed condition.
- [x] object `Chunked<T>` does `Iterating<@T>`
  - [x] `Chunked(origin is Iterating<T>, size is int)`
  - [x] `cursor() is Cursor<@T>`
  - [x] Reject non-positive size.
  - [x] Allow a shorter final chunk.
  - [x] Materialize only the current chunk.
- [x] object `Flattened<T>` does `Iterating<T>`
  - [x] `Flattened(origin is Iterating<Iterating<T>>)`
  - [x] `cursor() is Cursor<T>`
  - [x] Flatten exactly one level through `Chained<T>`.
- [x] object `Pairwise<T>` does `Iterating<Pair<T,T>>`
  - [x] `Pairwise(origin is Iterating<T>)`
  - [x] `cursor() is Cursor<Pair<T,T>>`
  - [x] Emit overlapping adjacent pairs.
- [x] object `Accumulated<T,R>` does `Iterating<R>`
  - [x] `Accumulated(origin is Iterating<T>, initial is R, action is (R,T) is R)`
  - [x] `cursor() is Cursor<R>`
  - [x] Emit the explicit initial value first.
- [x] object `Reduced<T,R>`
  - [x] `Reduced(origin is Iterating<T>, initial is R, action is (R,T) is R)`
  - [x] `#projected value() is R`
  - [x] Return the initial value for an empty origin.
- [x] object `Reversed<T>` does `Sequencing<T>, Iterating<T>, Sizing`
  - [x] `Reversed(origin is Sequencing<T>)`
  - [x] `cursor() is Cursor<T>`
  - [x] `size() is int`
  - [x] `item(index is int) is T`
  - [x] `first() is T`
  - [x] `last() is T`
  - [x] Preserve strict bounds and empty-sequence errors.
- [x] object `Sliced<T>` does `Iterating<T>`
  - [x] `Sliced(origin is Iterating<T>, start is int, stop is int, step is int)`
  - [x] `cursor() is Cursor<T>`
  - [x] Use half-open `[start, stop)` bounds.
  - [x] Reject negative start/stop and non-positive step.
  - [x] Keep reverse traversal in `Reversed` because `Iterating` cannot move backward.
- [x] Keep every view cursor immutable.
- [x] Document composition and slicing semantics in the module.
- [x] internal cursor-state objects
  - [x] object `CursorStep<T>`
    - [x] `CursorStep(current is Cursor<T>, following is Cursor<T>)`
    - [x] `current() is Cursor<T>`
    - [x] `following() is Cursor<T>`
    - [x] `item() is T`
  - [x] object `CursorSteps<T>` does `Iterating<CursorStep<T>>`
    - [x] `CursorSteps(cursor is Cursor<T>)`
    - [x] `cursor() is Cursor<CursorStep<T>>`
  - [x] object `CursorStepCursor<T>` does `Cursor<CursorStep<T>>`
    - [x] `available() is bool`
    - [x] `item() is CursorStep<T>`
    - [x] `successor() is Cursor<CursorStep<T>>`
  - [x] objects `MappedCursorSearch<A,B>`, `MappedCursor<A,B>`, `EmptyMappedCursor<A,B>`
    - [x] search `result() is Cursor<B>`
    - [x] active `available()`/`item()`/`successor()`
    - [x] exhausted `available()`/`item()`/`successor()`
  - [x] objects `FilteredCursorSearch<T>`, `FilteredCursor<T>`, `EmptyFilteredCursor<T>`
    - [x] search `result() is Cursor<T>`
    - [x] active `available()`/`item()`/`successor()`
    - [x] exhausted `available()`/`item()`/`successor()`
  - [x] object `TakenCursor<T>` does `Cursor<T>`
    - [x] `available() is bool`
    - [x] `item() is T`
    - [x] `successor() is Cursor<T>`
  - [x] object `DroppedCursorSearch<T>`
    - [x] `result() is Cursor<T>`
  - [x] objects `TakenWhileCursorSearch<T>`, `TakenWhileCursor<T>`, `EmptyTakenWhileCursor<T>`
    - [x] search `result() is Cursor<T>`
    - [x] active `available()`/`item()`/`successor()`
    - [x] exhausted `available()`/`item()`/`successor()`
  - [x] object `DroppedWhileCursorSearch<T>`
    - [x] `result() is Cursor<T>`
  - [x] object `ChunkedCursor<T>` does `Cursor<@T>`
    - [x] `available() is bool`
    - [x] `item() is @T`
    - [x] `successor() is Cursor<@T>`
  - [x] object `PairwiseCursor<T>` does `Cursor<Pair<T,T>>`
    - [x] `available() is bool`
    - [x] `item() is Pair<T,T>`
    - [x] `successor() is Cursor<Pair<T,T>>`
  - [x] objects `AccumulatedCursor<T,R>`, `ExhaustedAccumulatedCursor<R>`
    - [x] active `available()`/`item()`/`successor()`
    - [x] exhausted `available()`/`item()`/`successor()`
  - [x] object `ReversedCursor<T>` does `Cursor<T>`
    - [x] `available() is bool`
    - [x] `item() is T`
    - [x] `successor() is Cursor<T>`
  - [x] object `SlicedCursor<T>` does `Cursor<T>`
    - [x] `available() is bool`
    - [x] `item() is T`
    - [x] `successor() is Cursor<T>`
- [ ] Benchmark every decorator chain against its native bulk equivalent.

---

# Stage 04 — concrete collections and combinatorics

## `collections/list`

- [x] role `List<T>`
  - [x] `cursor() is Cursor<T>`
  - [x] `size() is int`
  - [x] `first() is T`
  - [x] `tail() is List<T>`
  - [x] `prefix(value is T) is List<T>`
  - [ ] After role composition is supported by the language, declare `List<T>` does `Iterating<T>, Sizing` instead of repeating their signatures.
- [x] object `EmptyList<T>` does `List<T>, Iterating<T>, Sizing`
  - [x] `EmptyList()`
  - [x] `cursor() is Cursor<T>`
  - [x] `size() is int`
  - [x] `first() is T`
  - [x] `tail() is List<T>`
  - [x] `prefix(value is T) is List<T>`
- [x] object `LinkedList<T>` does `List<T>, Iterating<T>, Sizing`
  - [x] `LinkedList(value is T, following is List<T>)`
  - [x] Cache `following.size() + 1` at construction.
  - [x] `cursor() is Cursor<T>`
  - [x] `size() is int`
  - [x] `first() is T`
  - [x] `tail() is List<T>`
  - [x] `prefix(value is T) is List<T>`
- [x] object `EmptyListCursor<T>` does `Cursor<T>`
  - [x] `EmptyListCursor(origin is List<T>)`
  - [x] Retain the empty origin as an honest generic type witness; do not invent a sentinel item.
  - [x] `available() is bool`
  - [x] `item() is T`
  - [x] `successor() is Cursor<T>`
- [x] object `LinkedListCursor<T>` does `Cursor<T>`
  - [x] `LinkedListCursor(value is T, following is List<T>)`
  - [x] `available() is bool`
  - [x] `item() is T`
  - [x] `successor() is Cursor<T>`
- [x] object `EmptyListAccess` does `Error`
  - [x] `EmptyListAccess(operation is string)`
  - [x] `operation() is string`
  - [x] `text() is string`
- [x] Keep lists and their cursors immutable and share existing tails.
- [x] Do not make linked lists random-access sequences.
- [x] Do not add the ceremonial `Array<T>` wrapper to v3.
- [x] Document construction, persistent sharing, iteration and failure behavior in the module.

## `collections/set`

- [x] role `Set<T>`
  - [x] `cursor() is Cursor<T>`
  - [x] `size() is int`
  - [x] `membership(value is T) is bool`
  - [x] `addition(value is T) is Set<T>`
  - [x] `difference(value is T) is Set<T>`
  - [ ] After role composition is supported by the language, declare `Set<T>` does `Iterating<T>, Sizing` instead of repeating their signatures.
- [x] object `OrderedSet<T>` does `Set<T>, Iterating<T>, Sizing`
  - [x] `OrderedSet(values is Iterating<T>)`
  - [x] `cursor() is Cursor<T>`
  - [x] `size() is int`
  - [x] `membership(value is T) is bool`
  - [x] `addition(value is T) is Set<T>`
  - [x] `difference(value is T) is Set<T>`
  - [x] Use strict runtime equality and document linear membership.
  - [x] Preserve the first occurrence of every value during construction.
- [ ] object `HashSet<T>` does `Set<T>, Iterating<T>, Sizing`
  - [ ] Define an explicit generic hashing/equality contract and admissible value rules.
  - [ ] Add runtime hash storage that accepts admissible object values, not only scalar map keys.
  - [ ] Keep insertion order independently from hash bucket order.
  - [ ] Do not publish `HashSet<T>` until `membership()` is genuinely hash-backed.
- [x] object `Union<T>` does `Set<T>, Iterating<T>, Sizing`
  - [x] `Union(left is Set<T>, right is Set<T>)`
  - [x] `cursor() is Cursor<T>`
  - [x] `size() is int`
  - [x] `membership(value is T) is bool`
  - [x] `addition(value is T) is Set<T>`
  - [x] `difference(value is T) is Set<T>`
- [x] object `Intersection<T>` does `Set<T>, Iterating<T>, Sizing`
  - [x] `Intersection(left is Set<T>, right is Set<T>)`
  - [x] `cursor() is Cursor<T>`
  - [x] `size() is int`
  - [x] `membership(value is T) is bool`
  - [x] `addition(value is T) is Set<T>`
  - [x] `difference(value is T) is Set<T>`
- [x] object `Difference<T>` does `Set<T>, Iterating<T>, Sizing`
  - [x] `Difference(left is Set<T>, right is Set<T>)`
  - [x] `cursor() is Cursor<T>`
  - [x] `size() is int`
  - [x] `membership(value is T) is bool`
  - [x] `addition(value is T) is Set<T>`
  - [x] `difference(value is T) is Set<T>`
- [x] object `SymmetricDifference<T>` does `Set<T>, Iterating<T>, Sizing`
  - [x] `SymmetricDifference(left is Set<T>, right is Set<T>)`
  - [x] `cursor() is Cursor<T>`
  - [x] `size() is int`
  - [x] `membership(value is T) is bool`
  - [x] `addition(value is T) is Set<T>`
  - [x] `difference(value is T) is Set<T>`
- [x] object `SetWith<T>` does `Set<T>, Iterating<T>, Sizing`
  - [x] `SetWith(origin is Set<T>, value is T)`
  - [x] Five `Set<T>` actions.
  - [x] Append only a genuinely absent value.
- [x] object `SetWithout<T>` does `Set<T>, Iterating<T>, Sizing`
  - [x] `SetWithout(origin is Set<T>, value is T)`
  - [x] Five `Set<T>` actions.
  - [x] Preserve traversal order while excluding the selected value.
- [x] internal immutable cursor and query objects
  - [x] object `SetWithCursor<T>` does `Cursor<T>`
  - [x] object `ConcatenatedSetCursor<T>` does `Cursor<T>`
  - [x] object `SetMembership<T>`
  - [x] object `SetSize<T>`
  - [x] object `AbsentFrom<T>`
  - [x] object `DifferentFrom<T>`
  - [x] object `DistinctList<T>`
- [x] Preserve first-insertion order during traversal.
- [x] Document construction, persistence, operation ordering and equality limitations in the module.

## `collections/deque`

- [x] object `Deque<T>` does `Iterating<T>, Sizing`
  - [x] `Deque(values is Iterating<T>)`
  - [x] `cursor() is Cursor<T>`
  - [x] `size() is int`
  - [x] `item(index is int) is T`
  - [x] `front() is DequeFront<T>`
  - [x] `back() is DequeBack<T>`
  - [x] Keep exactly five public actions.
  - [x] Do not claim `Sequencing<T>`: endpoint capabilities replace duplicate `first()`/`last()` actions.
  - [x] Keep middle indexing O(n), matching the linked persistent representation.
- [x] object `DequeFront<T>`
  - [x] Retain the immutable front/back list snapshot and cached size.
  - [x] `value() is T`
  - [x] `addition(value is T) is Deque<T>`
  - [x] `remainder() is Deque<T>`
- [x] object `DequeBack<T>`
  - [x] Retain the immutable front/back list snapshot and cached size.
  - [x] `value() is T`
  - [x] `addition(value is T) is Deque<T>`
  - [x] `remainder() is Deque<T>`
- [x] object `EmptyDequeAccess` does `Error`
  - [x] `EmptyDequeAccess(operation is string)`
  - [x] `operation() is string`
  - [x] `text() is string`
- [x] persistent two-list representation
  - [x] Store the front half in traversal order.
  - [x] Store the back half in reverse traversal order.
  - [x] Cache total size.
  - [x] Rebalance when either half grows beyond roughly twice the other.
  - [x] Preserve logical order across rebalancing.
  - [x] Keep cursors immutable.
  - [x] Preserve earlier deque versions after endpoint operations.
- [x] internal representation objects
  - [x] object `DequeItems<T>` does `Iterating<T>`
  - [x] object `ConcatenatedDequeCursor<T>` does `Cursor<T>`
  - [x] objects `ArrayItems<T>`, `ArrayCursor<T>` with an explicit empty-source type witness
  - [x] objects `ForwardList<T>`, `ReversedList<T>`
  - [x] object `DequeParts<T>`
  - [x] object `BalancedDeque<T>`
  - [x] object `NormalizedDeque<T>`
- [x] Validate every externally constructible representation state.
- [ ] Add module-private declaration/constructor visibility, then hide the representation constructor and internal helper objects from external `take`.
- [ ] Benchmark long alternating and one-sided operation chains to verify the amortized-O(1) balance policy.
- [x] Document construction, endpoint capabilities, persistence, complexity and failures in the module.
- [x] Do not add separate stack/queue types without a concrete narrower consumer.

## `collections/heap`

- [x] prerequisites
  - [x] Rename `Comparing.compared()` to `order()`.
  - [x] Add and enforce `T: Comparing<T>` object bounds.
  - [x] Make naturally ordered builtin values satisfy `Comparing<Self>`.
- [x] object `Heap<T: Comparing<T>>` does `Sizing`
  - [x] `Heap(values is Iterating<T>)`
  - [x] `size() is int`
  - [x] `top() is T`
  - [x] `remainder() is Heap<T>`
  - [x] `addition(value is T) is Heap<T>`
- [x] internal persistent representation
  - [x] role `HeapRepresenting<T>` with five actions
  - [x] object `EmptyHeapTree<T: Comparing<T>>`
  - [x] object `HeapTreeNode<T: Comparing<T>>`
  - [x] object `HeapChildren<T>` normalizes child ranks
  - [x] object `HeapBuilding<T: Comparing<T>>` keeps `reduce` accumulator identity stable
  - [x] Cache subtree size and right-spine rank.
  - [x] Restore the leftist invariant whenever a node is created.
  - [x] Share immutable subtrees between returned heap versions.
- [x] Implement a persistent leftist min-heap; do not expose or depend on heap-array order.
- [x] Keep `top()`/`size()` O(1) and `addition()`/`remainder()` O(log n).
- [x] Make empty `top()`/`remainder()` throw `EmptySequence`.
- [x] Document construction, ordering, persistence, complexity and failures in the module.
- [ ] Add module-private declarations/constructors, then hide `HeapRepresenting`, its variants and the representation constructor.

## `collections/frequencies`

- [x] object `Frequency<T>`
  - [x] `Frequency(item is T, amount is int)`
  - [x] `item() is T`
  - [x] `amount() is int`
  - [x] Reject a non-positive amount.
- [x] object `Frequencies<T>` does `Iterating<Frequency<T>>, Sizing`
  - [x] `Frequencies(values is Iterating<T>)`
  - [x] `cursor() is Cursor<Frequency<T>>`
  - [x] `size() is int`
  - [x] `amount(value is T) is int`
  - [x] `total() is int`
- [x] internal immutable accumulation
  - [x] object `FrequencyBuilding<T>`
  - [x] object `FrequencyRevision<T>`
  - [x] Keep concrete `Frequency` amounts instead of rescanning the input.
- [x] Define `size()` as distinct-item count.
- [x] Define `total()` as total input-item count.
- [x] Preserve first-occurrence order during traversal.
- [x] Use strict runtime equality until a generic hashing contract exists.
- [x] Document equality, traversal, complexity and empty behavior.
- [ ] Replace linear lookup only after the language has a sound generic `Hashing` contract and persistent map.
- [ ] Add module-private declarations, then hide the accumulation objects.

## `collections/ordering`

- [x] object `Sorted<T: Comparing<T>>` does `Sequencing<T>`
  - [x] `Sorted(origin is Iterating<T>)`
  - [x] `cursor() is Cursor<T>`
  - [x] `size() is int`
  - [x] `item(index is int) is T`
  - [x] `first() is T`
  - [x] `last() is T`
- [x] object `SortedBy<T,K: Comparing<K>>` does `Sequencing<T>`
  - [x] `SortedBy(origin is Iterating<T>, key is (T) is K)`
  - [x] Evaluate the key exactly once per input item.
  - [x] Preserve input order between equal keys.
  - [x] `cursor() is Cursor<T>`
  - [x] `size() is int`
  - [x] `item(index is int) is T`
  - [x] `first() is T`
  - [x] `last() is T`
- [x] object `PositionOf<T: Comparing<T>>`
  - [x] `PositionOf(origin is Sequencing<T>, item is T)`
  - [x] `#projected value() is int`
  - [x] Return the first matching position.
- [x] object `LowerBound<T: Comparing<T>>`
  - [x] `LowerBound(origin is Sequencing<T>, item is T)`
  - [x] `#projected value() is int`
- [x] object `UpperBound<T: Comparing<T>>`
  - [x] `UpperBound(origin is Sequencing<T>, item is T)`
  - [x] `#projected value() is int`
- [x] object `MissingItem` does `Error`
  - [x] `text() is string`
- [x] object `Ranked<T,K: Comparing<K>>` does `Comparing<Ranked<T,K>>`
  - [x] `Ranked(item is T, key is K)`
  - [x] `item() is T`
  - [x] `key() is K`
  - [x] `order(other is Ranked<T,K>) is Order`
- [x] internal immutable traversal and binary search
  - [x] objects `OrderedCursor<T>`, `RankedCursor<T,K: Comparing<K>>`
  - [x] objects `LowerSearching<T: Comparing<T>>`, `UpperSearching<T: Comparing<T>>`
  - [x] Validate every externally constructible cursor/search state.
  - [x] Keep lower/upper/position queries O(log n).
- [x] Require sorted values or their keys to satisfy `Comparing`.
- [x] Use `Ranked` when domain ordering must differ from a value's intrinsic ordering.
- [x] Route sorting through `runtime/ordering.BulkOrdering` and the host bulk intrinsic.
- [x] Use a stable host mergesort; do not expose a sorting algorithm in collections.
- [x] Document materialization, stable key ordering, projections and sorted-input preconditions.
- [x] Anonymous actions declare parameter and result types explicitly.
- [ ] Verify all `SortedBy<T,K>` constructor-inference paths with explicitly typed key actions; generic inference is not redesigned by the syntax migration.
- [x] Teach generic constructor inference to use retained empty-array element tags (arrays and array cursors now bind `T` for `Iterating<T>`, `Sequencing<T>` and `Cursor<T>` parameters, 20.09.2026).
- [ ] Add module-private declarations, then hide cursors and binary-search helpers.

## `collections/combinatorics`

- [x] object `Combinations<T>` does `Iterating<@T>`
  - [x] `Combinations(origin is Sequencing<T>, size is int)`
  - [x] `cursor() is Cursor<@T>`
  - [x] Reject negative size.
  - [x] Produce an empty iterator when size exceeds the origin size.
  - [x] Produce one empty result when size is zero.
- [x] object `CombinationCursor<T>` does `Cursor<@T>`
  - [x] `CombinationCursor(origin is Sequencing<T>, positions is @int, size is int, active is bool)`
  - [x] `available() is bool`
  - [x] `item() is @T`
  - [x] `successor() is Cursor<@T>`
- [x] combination state objects
  - [x] object `CombinationPositions`; `CombinationPositions(positions is @int, origin_size is int)`; `valid() is bool`
  - [x] object `CombinationPivot`; `CombinationPivot(positions is @int, origin_size is int)`; `value() is int`
- [x] object `CombinationsWithReplacement<T>` does `Iterating<@T>`
  - [x] `CombinationsWithReplacement(origin is Sequencing<T>, size is int)`
  - [x] `cursor() is Cursor<@T>`
  - [x] Reject negative size.
  - [x] Produce an empty iterator for a nonzero selection from an empty origin.
  - [x] Produce one empty result when size is zero.
- [x] object `ReplacementCombinationCursor<T>` does `Cursor<@T>`
  - [x] `ReplacementCombinationCursor(origin is Sequencing<T>, positions is @int, size is int, active is bool)`
  - [x] `available() is bool`
  - [x] `item() is @T`
  - [x] `successor() is Cursor<@T>`
- [x] replacement-combination state objects
  - [x] object `ReplacementPositions`; `ReplacementPositions(positions is @int, origin_size is int)`; `valid() is bool`
  - [x] object `ReplacementPivot`; `ReplacementPivot(positions is @int, origin_size is int)`; `value() is int`
- [x] object `Permutations<T>` does `Iterating<@T>`
  - [x] `Permutations(origin is Sequencing<T>)`
  - [x] `Permutations(origin is Sequencing<T>, size is int)`
  - [x] `cursor() is Cursor<@T>`
  - [x] Reject negative size.
  - [x] Produce an empty iterator when size exceeds the origin size.
  - [x] Produce one empty result when size is zero.
- [x] object `PermutationCursor<T>` does `Cursor<@T>`
  - [x] `PermutationCursor(origin is Sequencing<T>, positions is @int, size is int, active is bool)`
  - [x] `available() is bool`
  - [x] `item() is @T`
  - [x] `successor() is Cursor<@T>`
- [x] permutation state objects
  - [x] object `PermutationPositions`; `PermutationPositions(positions is @int, origin_size is int)`; `valid() is bool`
  - [x] object `PermutationPivot`; `PermutationPivot(positions is @int, origin_size is int)`; `value() is int`
  - [x] object `PermutationCandidate`; `PermutationCandidate(positions is @int, prefix is int, origin_size is int, start is int)`; `#projected value() is int`
  - [x] object `PermutationAvailableIndex`; `PermutationAvailableIndex(positions is @int, prefix is int, selected is int, origin_size is int, offset is int)`; `#projected value() is int`
  - [x] object `PrefixMembership`; `PrefixMembership(positions is @int, stop is int, value is int)`; `result() is bool`
  - [x] object `PrefixBefore`; `PrefixBefore(positions is @int, stop is int, value is int)`; `amount() is int`
- [x] object `CartesianProduct<T>` does `Iterating<@T>`
  - [x] `CartesianProduct(origins is Sequencing<Sequencing<T>>)`
  - [x] `cursor() is Cursor<@T>`
  - [x] Produce one empty result for zero dimensions.
  - [x] Produce an empty iterator when any dimension is empty.
  - [x] Advance the rightmost dimension fastest.
- [x] object `CartesianCursor<T>` does `Cursor<@T>`
  - [x] `CartesianCursor(origins is Sequencing<Sequencing<T>>, positions is @int, active is bool)`
  - [x] `available() is bool`
  - [x] `item() is @T`
  - [x] `successor() is Cursor<@T>`
- [x] cartesian-product state objects
  - [x] object `CartesianAvailability<T>`; `CartesianAvailability(origins is Sequencing<Sequencing<T>>)`; `result() is bool`
  - [x] object `CartesianPositions<T>`; `CartesianPositions(origins is Sequencing<Sequencing<T>>, positions is @int)`; `valid() is bool`
  - [x] object `CartesianPivot<T>`; `CartesianPivot(origins is Sequencing<Sequencing<T>>, positions is @int)`; `value() is int`
- [x] Keep every combinatorial result lazy.
- [x] Keep cursor state immutable and independent between successors.
- [x] Do not precompute total result counts or risk eager count overflow.
- [x] Validate active cursor shapes and index invariants.
- [x] Document explosive result sizes and recommend bounded consumption.
- [x] Make builtin `@T` satisfy `Sequencing<T>, Iterating<T>, Sizing` so cartesian dimensions need no ceremonial array wrapper.
- [ ] Add module-private declarations, then hide cursor-state helper objects.

---

# Stage 05 — strings, formatting and parsing

## `builtin/string`

- [x] `string.length() is int`
- [x] `string.empty() is bool`
- [x] `string.slice(start is int, end is int) is string`
- [?] `string.membership(part is string) is bool`
- [?] `string.prefix(value is string) is bool`
- [?] `string.suffix(value is string) is bool`
- [ ] `string.index(part is string) is int`
- [ ] `string.last_index(part is string) is int`
- [ ] `string.count(part is string) is int`
- [ ] `string.lowercase_text() is string`
- [ ] `string.uppercase_text() is string`
- [ ] `string.title_text() is string`
- [ ] `string.capitalized_text() is string`
- [ ] `string.casefolded_text() is string`
- [ ] `string.trimmed_text() is string`
- [ ] `string.trimmed_text(chars is string) is string`
- [ ] `string.trimmed_start() is string`
- [ ] `string.trimmed_start(chars is string) is string`
- [ ] `string.trimmed_end() is string`
- [ ] `string.trimmed_end(chars is string) is string`
- [ ] `string.parts() is @string`
- [ ] `string.parts(separator is string) is @string`
- [ ] `string.lines() is @string`
- [ ] `string.replacement(old is string, replacement is string) is string`
- [ ] `string.replacement(old is string, replacement is string, count is int) is string`
- [ ] `string.repetition(count is int) is string`
- [ ] `string.reversal() is string`
- [ ] `string.left_padding(width is int, fill is char) is string`
- [ ] `string.right_padding(width is int, fill is char) is string`
- [ ] `string.centered_text(width is int, fill is char) is string`
- [ ] `string.zero_filled_text(width is int) is string`
- [ ] `string.expanded_tabs(width is int) is string`
- [ ] `string.alphabetic() is bool`
- [ ] `string.alphanumeric() is bool`
- [-] `string.decimal() is bool`
- [-] `string.digit() is bool`
- [-] `string.numeric() is bool`
- [ ] `string.whitespace() is bool`
- [ ] `string.printable() is bool`
- [ ] `string.identifier() is bool`
- [ ] `string.lowercase() is bool`
- [ ] `string.uppercase() is bool`
- [ ] `string.titlecase() is bool`
- [ ] Indexing `text[index]` returns `char`.
- [ ] `len()` counts Unicode scalar values, not encoded bytes.
- [x] Pin Unicode 16.0.0 as a backend-independent language contract for case conversion and classification.
- [ ] Remove ambiguous `words()`.

## `strings/join`

- [x] builtin `@string.join(separator is string) is string`
- [ ] object `Joined`
  - [ ] `Joined(separator is string, values is Iterating<string>)`
  - [ ] `#projected string() is string`
- [ ] Verify builtin and generic results are identical.

## `text/layout`

- [x] object `Indented`
  - [x] `Indented(origin is string, prefix is string)`
  - [x] `#projected string() is string`
  - [x] Prefix nonblank lines only.
  - [x] Preserve LF, CRLF and CR line endings.
- [x] object `Dedented`
  - [x] `Dedented(origin is string)`
  - [x] `#projected string() is string`
  - [x] Remove the longest exact spaces-and-tabs prefix shared by nonblank lines.
  - [x] Clear indentation from blank lines.
- [x] object `Wrapped`
  - [x] `Wrapped(origin is string, width is int)`
  - [x] `#projected string() is string`
  - [x] Reject non-positive width.
  - [x] Collapse interword whitespace and trim the result.
  - [x] Keep overlong words intact instead of splitting them.
- [x] object `Shortened`
  - [x] `Shortened(origin is string, width is int, placeholder is string)`
  - [x] `#projected string() is string`
  - [x] Reject negative width.
  - [x] Reject a placeholder wider than width.
  - [x] Keep the longest whole-word prefix that fits with the exact placeholder.
- [x] native bulk boundary `runtime/layout`
  - [x] `indent(origin is string, prefix is string) is string`
  - [x] `dedent(origin is string) is string`
  - [x] `wrap(origin is string, width is int) is string`
  - [x] `shorten(origin is string, width is int, placeholder is string) is string`
  - [x] Register portable provider `core.layout`.
- [x] Count widths as Unicode scalar values, consistently with `string.length()`.
- [x] Document whitespace normalization, long-word behavior and validation.

## `strings/format`

- [x] object `PercentFormatted`
  - [x] `PercentFormatted(pattern is string, values is ...any)`
  - [x] `#projected string() is string`
- [x] object `InvalidFormat` does `Error`
  - [x] `offset() is int`
  - [x] `text() is string`
- [x] object `UnsupportedFormatValue` does `Error`
  - [x] `text() is string`
- [x] Support builtin scalars only.
- [x] Require explicit conversion for domain objects.
- [x] Do not add `printf`/`writef` to stdout.

## `strings/parse`

- [x] object `ParsedInt`
  - [x] `ParsedInt(origin is string)`
  - [x] `ParsedInt(origin is string, base is int)`
  - [x] `#projected value() is int`
- [x] object `ParsedFloat`
  - [x] `ParsedFloat(origin is string)`
  - [x] `#projected value() is float`
- [x] object `ParsedRat`
  - [x] `ParsedRat(origin is string)`
  - [x] `#projected value() is rat`
- [x] object `ParsedBool`
  - [x] `ParsedBool(origin is string)`
  - [x] `#projected value() is bool`
- [x] object `ParsedByte`
  - [x] `ParsedByte(origin is string)`
  - [x] `ParsedByte(origin is string, base is int)`
  - [x] `#projected value() is byte`
- [x] object `InvalidInteger` does `Error`
  - [x] `text() is string`
- [x] object `InvalidFloat` does `Error`
  - [x] `text() is string`
- [x] object `InvalidRational` does `Error`
  - [x] `text() is string`
- [x] object `InvalidBoolean` does `Error`
  - [x] `text() is string`
- [x] object `InvalidByte` does `Error`
  - [x] `text() is string`
- [x] Accept integer bases `2..36`.
- [x] Let base `0` explicitly enable prefix detection.
- [x] Never strip whitespace implicitly.
- [x] Make `ParsedBool` accept canonical `true`/`false` only.
- [x] Validate `ParsedByte` range `0..255`.

## `diff`

- [ ] object `UnifiedDiff`
  - [ ] `UnifiedDiff(left is Sequencing<string>, right is Sequencing<string>)`
  - [ ] `#projected string() is string`
- [ ] object `ContextDiff`
  - [ ] `ContextDiff(left is Sequencing<string>, right is Sequencing<string>)`
  - [ ] `#projected string() is string`
- [ ] object `MatchingBlocks`
  - [ ] Specify the block value type.
  - [ ] Implement sequence projection.
- [ ] object `MatchingRatio`
  - [ ] Specify and document the exact algorithm.
  - [ ] `#projected value() is float`
- [ ] Remove vague algorithm-free `Similarity`.

---

# Stage 06 — Unicode and encodings

## `unicode/character`

- [x] enum `UnicodeCategory`
  - [x] `uppercase_letter`
  - [x] `lowercase_letter`
  - [x] `titlecase_letter`
  - [x] `modifier_letter`
  - [x] `other_letter`
  - [x] `nonspacing_mark`
  - [x] `spacing_mark`
  - [x] `enclosing_mark`
  - [x] `decimal_number`
  - [x] `letter_number`
  - [x] `other_number`
  - [x] `connector_punctuation`
  - [x] `dash_punctuation`
  - [x] `open_punctuation`
  - [x] `close_punctuation`
  - [x] `initial_punctuation`
  - [x] `final_punctuation`
  - [x] `other_punctuation`
  - [x] `math_symbol`
  - [x] `currency_symbol`
  - [x] `modifier_symbol`
  - [x] `other_symbol`
  - [x] `space_separator`
  - [x] `line_separator`
  - [x] `paragraph_separator`
  - [x] `control`
  - [x] `format`
  - [x] `surrogate`
  - [x] `private_use`
  - [x] `unassigned`
- [x] object `UnicodeCharacter`
  - [x] `UnicodeCharacter(origin is char)`
  - [x] `name() is string`
  - [x] `category() is UnicodeCategory`
  - [x] `combining_class() is int`
  - [x] `numeric_value() is float`
- [x] object `NonNumericCharacter` does `Error`
  - [x] `text() is string`

## `unicode/normalization`

- [x] enum `NormalizationForm`
  - [x] `nfc`
  - [x] `nfd`
  - [x] `nfkc`
  - [x] `nfkd`
- [x] object `Normalized`
  - [x] `Normalized(origin is string, form is NormalizationForm)`
  - [x] `#projected string() is string`
- [x] Use the same backend-independent Unicode 16.0.0 contract as builtin strings.

## `encoding/utf`

- [x] enum `ByteOrder`
  - [x] `big`
  - [x] `little`
- [x] object `Utf8Encoded`
  - [x] `Utf8Encoded(origin is string)`
  - [x] `bytes() is @byte`
- [x] object `Utf8Decoded`
  - [x] `Utf8Decoded(origin is @byte)`
  - [x] `string() is string`
- [x] object `Utf16Encoded`
  - [x] `Utf16Encoded(origin is string, order is ByteOrder)`
  - [x] `bytes() is @byte`
- [x] object `Utf16Decoded`
  - [x] `Utf16Decoded(origin is @byte, order is ByteOrder)`
  - [x] `string() is string`
- [x] object `Utf32Encoded`
  - [x] `Utf32Encoded(origin is string, order is ByteOrder)`
  - [x] `bytes() is @byte`
- [x] object `Utf32Decoded`
  - [x] `Utf32Decoded(origin is @byte, order is ByteOrder)`
  - [x] `string() is string`
- [x] object `InvalidUtf8` does `Error`
  - [x] `offset() is int`
  - [x] `text() is string`
- [x] object `InvalidUtf16` does `Error`
  - [x] `offset() is int`
  - [x] `text() is string`
- [x] object `InvalidUtf32` does `Error`
  - [x] `offset() is int`
  - [x] `text() is string`
- [x] Never add ambiguous native byte order to serialized data APIs.

## `encoding/single_byte`

- [x] object `AsciiEncoded`
  - [x] `AsciiEncoded(origin is string)`
  - [x] `bytes() is @byte`
- [x] object `AsciiDecoded`
  - [x] `AsciiDecoded(origin is @byte)`
  - [x] `string() is string`
- [x] object `Latin1Encoded`
  - [x] `Latin1Encoded(origin is string)`
  - [x] `bytes() is @byte`
- [x] object `Latin1Decoded`
  - [x] `Latin1Decoded(origin is @byte)`
  - [x] `string() is string`
- [x] object `UnrepresentableAsciiCharacter` does `Error`
  - [x] `offset() is int`
  - [x] `text() is string`
- [x] object `UnrepresentableLatin1Character` does `Error`
  - [x] `offset() is int`
  - [x] `text() is string`
- [x] object `InvalidAsciiByte` does `Error`
  - [x] `offset() is int`
  - [x] `text() is string`

## `encoding/hex`

- [x] object `HexEncoded`
  - [x] `HexEncoded(origin is @byte)`
  - [x] `string() is string`
- [x] object `HexDecoded`
  - [x] `HexDecoded(origin is string)`
  - [x] `bytes() is @byte`
- [x] object `InvalidHex` does `Error`
  - [x] `offset() is int`
  - [x] `text() is string`
- [x] Encode with lowercase ASCII and no prefix.
- [x] Decode either ASCII letter case, but reject prefixes, whitespace and separators.
- [x] Report an unmatched final nibble at `origin.length()`.

## `encoding/base32`

- [x] object `Base32Encoded`
  - [x] `Base32Encoded(origin is @byte)`
  - [x] `string() is string`
- [x] object `Base32Decoded`
  - [x] `Base32Decoded(origin is string)`
  - [x] `bytes() is @byte`
- [x] object `InvalidBase32` does `Error`
  - [x] `offset() is int`
  - [x] `text() is string`
- [x] Use the uppercase RFC 4648 alphabet `A-Z2-7`.
- [x] Emit and require canonical `=` padding to a multiple of eight characters.
- [x] Reject lowercase, `0`/`1` aliases, whitespace, separators and nonzero unused bits.
- [x] Report structurally incomplete final quanta at `origin.length()`.

## `encoding/base64`

- [x] enum `Base64Alphabet`
  - [x] `standard`
  - [x] `url`
- [x] object `Base64Encoded`
  - [x] `Base64Encoded(origin is @byte)`
  - [x] `Base64Encoded(origin is @byte, alphabet is Base64Alphabet)`
  - [x] `string() is string`
- [x] object `Base64Decoded`
  - [x] `Base64Decoded(origin is string)`
  - [x] `Base64Decoded(origin is string, alphabet is Base64Alphabet)`
  - [x] `bytes() is @byte`
- [x] object `InvalidBase64` does `Error`
  - [x] `offset() is int`
  - [x] `text() is string`
- [x] Default to the standard RFC 4648 alphabet; make URL-safe selection explicit.
- [x] Emit and require canonical `=` padding for both alphabets.
- [x] Reject whitespace, mixed alphabets, misplaced padding and nonzero unused bits.
- [x] Report structurally incomplete final quanta at `origin.length()`.

## `encoding/base85`

- [x] enum `Base85Alphabet`
  - [x] `ascii85`
  - [x] `rfc1924`
- [x] object `Base85Encoded`
  - [x] `Base85Encoded(origin is @byte, alphabet is Base85Alphabet)`
  - [x] `string() is string`
- [x] object `Base85Decoded`
  - [x] `Base85Decoded(origin is string, alphabet is Base85Alphabet)`
  - [x] `bytes() is @byte`
- [x] object `InvalidBase85` does `Error`
  - [x] `offset() is int`
  - [x] `text() is string`
- [x] Keep ASCII85 raw: emit `z`, reject `y`, Adobe wrappers and whitespace.
- [x] Use the exact RFC 1924 alphabet without abbreviations.
- [x] Validate overflow and canonical partial groups.
- [x] Leave fixed-block Z85 out until a concrete consumer requires it.

## `encoding/quoted_printable`

- [x] object `QuotedPrintableEncoded`
  - [x] `QuotedPrintableEncoded(origin is @byte)`
  - [x] `bytes() is @byte`
- [x] object `QuotedPrintableDecoded`
  - [x] `QuotedPrintableDecoded(origin is @byte)`
  - [x] `bytes() is @byte`
- [x] object `InvalidQuotedPrintable` does `Error`
  - [x] `offset() is int`
  - [x] `text() is string`
- [x] Emit uppercase `=HH` and preserve only exact CRLF hard breaks.
- [x] Encode trailing space/tab and insert canonical `=\r\n` soft breaks.
- [x] Keep every physical line within 76 bytes without splitting `=HH`.
- [x] Reject malformed escapes, bare CR/LF, trailing literal whitespace and non-ASCII literals.

## `encoding/pem`

- [x] object `PemEncoded`
  - [x] `PemEncoded(kind is string, data is @byte)`
  - [x] `string() is string`
- [x] object `PemBlock`
  - [x] `PemBlock(origin is string)`
  - [x] `kind() is string`
  - [x] `data() is @byte`
- [x] object `PemBlocks` does `Iterating<PemBlock>`
  - [x] `PemBlocks(origin is string)`
  - [x] `cursor() is Cursor<PemBlock>`
- [x] object `InvalidPem` does `Error`
  - [x] `offset() is int`
  - [x] `text() is string`
- [x] Emit uppercase labels, padded standard Base64, 64-character lines and LF.
- [x] Accept LF/CRLF, but reject bare CR, prose and whitespace-only separator lines.
- [x] Require matching boundaries and strict canonical Base64 bodies.
- [x] Define `PemBlock` as exactly one block and `PemBlocks` as zero or more blocks.
- [x] Report Unicode-scalar offsets into the source text or rejected encoding label.

## `encoding/binary`

- [x] enum `Signedness`
  - [x] `signed`
  - [x] `unsigned`
- [x] enum `FloatWidth`
  - [x] `bits32`
  - [x] `bits64`
- [x] object `IntegerFromBytes`
  - [x] `IntegerFromBytes(origin is @byte, signedness is Signedness, order is ByteOrder)`
  - [x] `#projected value() is int`
- [x] object `IntegerBytes`
  - [x] `IntegerBytes(value is int, size is int, signedness is Signedness, order is ByteOrder)`
  - [x] `bytes() is @byte`
- [x] object `FloatFromBytes`
  - [x] `FloatFromBytes(origin is @byte, width is FloatWidth, order is ByteOrder)`
  - [x] `#projected value() is float`
- [x] object `FloatBytes`
  - [x] `FloatBytes(value is float, width is FloatWidth, order is ByteOrder)`
  - [x] `bytes() is @byte`
- [x] Reject non-positive integer byte size.
- [x] Reject input width mismatches.
- [x] Use two's complement for signed integers and magnitude for unsigned integers.
- [x] Reject integer truncation and decoded values outside signed 64-bit `int`.
- [x] Use finite IEEE 754 binary32/binary64 and reject narrowing overflow, infinity and NaN.
- [x] Preserve signed zero and subnormal IEEE representations.
- [x] Keep byte order explicit; never expose machine-native order.
- [x] Do not add `BinaryLayout`, `Packed` or `Unpacked` without typed heterogeneous product values.
- [x] Prefer protocol-specific objects composed from typed scalar encodings over a string format DSL.

---

# Stage 07 — I/O resources and streams

## `io/stream`

- [x] role `ByteSource`
  - [x] `bytes() is @byte`
- [x] role `Writing`
  - [x] `write(source is ByteSource) is void`
  - [x] `flush() is void`
- [x] role `Reading`
  - [x] `bytes(limit is int) is @byte`
- [x] role `WritingText`
  - [x] `write(values is ...string) is void`
  - [x] `writeln(values is ...string) is void`
- [x] role `ReadingText`
  - [x] `line() is string`
  - [x] `text() is string`
- [x] role `Closing`
  - [x] `closed() is bool`
  - [x] `close() is void`
- [x] object `IncompleteWrite` does `Error`
  - [x] `IncompleteWrite(written is int)`
  - [x] `written() is int`
  - [x] `text() is string`
- [x] object `InvalidReadLimit` does `Error`
  - [x] `InvalidReadLimit(limit is int)`
  - [x] `limit() is int`
  - [x] `text() is string`
- [x] Make builtin `string` an UTF-8 `ByteSource` only inside `<-`.
- [x] Make builtin `@byte` a lossless `ByteSource` only inside `<-`.
- [x] Make `<-` left-associative and evaluate each operand once.
- [x] Make `<-` discard the command body's internal result and return the original `Writing` capability.
- [x] Make `Writing.write` all-or-`IncompleteWrite`.
- [x] Make empty `Reading.bytes()` result mean stable EOF.
- [x] Reject non-positive read limits with `InvalidReadLimit`.

## `io/stdout`

- [x] object `Stdout` does `Writing, WritingText`
  - [x] `Stdout()`
  - [x] `write(source is ByteSource) is void`
  - [x] `flush() is void`
  - [x] `write(values is ...string) is void`
  - [x] `writeln(values is ...string) is void`
- [x] object `Stderr` does `Writing, WritingText`
  - [x] `Stderr()`
  - [x] `write(source is ByteSource) is void`
  - [x] `flush() is void`
  - [x] `write(values is ...string) is void`
  - [x] `writeln(values is ...string) is void`
- [x] Encode text as strict UTF-8; concatenate arguments without separators.
- [x] Make `writeln` append exactly one LF after all arguments.
- [x] Make empty `write()` emit nothing and empty `writeln()` emit one LF.
- [x] Verify `Stdout() <- "hello" <- "\n"`.
- [x] Do not add `print`, `println`, `printf` or `writef` aliases.

## `io/stdin`

- [x] object `Stdin` does `Reading`
  - [x] `Stdin()`
  - [x] `bytes(limit is int) is @byte`
- [x] object `Utf8Input` does `ReadingText, Closing`
  - [x] `Utf8Input(origin is Reading)`
  - [x] `line() is string`
  - [x] `text() is string`
  - [x] `closed() is bool`
  - [x] `close() is void`
- [x] object `EndOfInput` does `Error`
  - [x] `text() is string`
- [x] object `ClosedInput` does `Error`
  - [x] `text() is string`
- [x] Keep decoder buffer/lifecycle mutation in an opaque backend-neutral runtime handle.
- [x] Treat the wrapped `Reading` as non-owned; `Utf8Input.close()` closes only its decoder state.
- [x] Split lines on LF, remove one preceding CR and preserve all other characters.
- [x] Return a final non-empty line without a newline.
- [x] Make `text()` preserve remaining newlines without normalization.
- [x] Decode strict UTF-8 across raw read boundaries.
- [x] Throw `EndOfInput` only after all buffered text is consumed.

## `io/text`

- [ ] Move the generic `Utf8Input` adapter from `io/stdin` to `io/text`
  without changing its public contract.
- [ ] Keep `io/stdin` responsible only for `Stdin` and input-specific errors.
- [ ] object `Utf8Output` does `WritingText`
  - [ ] `Utf8Output(origin is Writing)`
  - [ ] `write(values is ...string) is void`
  - [ ] `writeln(values is ...string) is void`
- [ ] Treat the wrapped `Writing` as non-owned.
- [ ] Encode every argument as strict UTF-8 without ambient locale state.
- [ ] Concatenate `write` arguments without separators.
- [ ] Append exactly one LF after all `writeln` arguments.
- [ ] Keep flushing on the underlying `Writing` capability.

## `io/buffer`

- [x] object `ByteBuffer`
  - [x] `ByteBuffer()`
  - [x] `ByteBuffer(initial is @byte)`
  - [x] `reading() is Reading`
  - [x] `writing() is Writing`
  - [x] `content() is BufferContent`
  - [x] `closing() is Closing`
- [x] object `BufferReading` does `Reading`
  - [x] `BufferReading(state is BufferState)`
  - [x] `bytes(limit is int) is @byte`
- [x] object `BufferWriting` does `Writing`
  - [x] `BufferWriting(state is BufferState)`
  - [x] `write(source is ByteSource) is void`
  - [x] `flush() is void`
- [x] object `BufferClosing` does `Closing`
  - [x] `BufferClosing(state is BufferState)`
  - [x] `closed() is bool`
  - [x] `close() is void`
- [x] object `BufferContent` does `Sizing`
  - [x] `BufferContent(state is BufferState)`
  - [x] `bytes() is @byte`
  - [x] `size() is int`
- [x] object `ClosedBuffer` does `Error`
  - [x] `text() is string`
- [x] Share one opaque `BufferState` across every narrow capability view.
- [x] Make `BufferContent` a live non-consuming view whose `bytes()` returns a snapshot.
- [x] Make close idempotent, clear retained bytes and invalidate every existing view.
- [x] Remove mutable `reset()`.
- [x] Do not add a separate `TextBuffer`.
- [x] Compose text behavior from `writing() <- string`, `Utf8Input(reading())` and `Utf8Decoded(content().bytes())`.

## `io/decorators`

- [x] object `BufferedReading` does `Reading, Closing`
  - [x] `BufferedReading(origin is Reading)`
  - [x] `BufferedReading(origin is Reading, capacity is int)`
  - [x] `bytes(limit is int) is @byte`
  - [x] `closed() is bool`
  - [x] `close() is void`
  - [x] Use default capacity 8192 and reject non-positive explicit capacity.
  - [x] Read `max(limit, capacity)` only when the private cache is empty.
  - [x] Retain excess bytes and return at most the caller's limit.
  - [x] Treat origin as non-owned; close and clear only the cache.
- [x] object `LimitedReading` does `Reading`
  - [x] `LimitedReading(origin is Reading, limit is int)`
  - [x] `bytes(size is int) is @byte`
  - [x] Reject negative total limits and allow zero as immediate EOF.
  - [x] Share one budget across calls and consume only bytes actually returned.
- [x] object `ChainedReading` does `Reading`
  - [x] `ChainedReading(origins is Sequencing<Reading>)`
  - [x] `bytes(size is int) is @byte`
  - [x] Advance only after the current origin returns EOF.
  - [x] Skip long empty-origin runs iteratively.
- [x] object `ByteSnapshot` does `ByteSource`
  - [x] `ByteSnapshot(origin is @byte)`
  - [x] `bytes() is @byte`
- [x] object `TeeWriting` does `Writing`
  - [x] `TeeWriting(destinations is Sequencing<Writing>)`
  - [x] `write(source is ByteSource) is void`
  - [x] `flush() is void`
  - [x] Materialize the source exactly once and write one snapshot in sequence order.
  - [x] Stop on the first failure without rolling completed destinations back.
  - [x] Write repeatedly when the destination sequence contains duplicates.
- [x] object `Discarding` does `Writing`
  - [x] `Discarding()`
  - [x] `write(source is ByteSource) is void`
  - [x] `flush() is void`
  - [x] Consume `source.bytes()` exactly once and retain nothing.
- [x] object `ClosedReading` does `Error`
  - [x] `text() is string`
- [x] object `InvalidReading` does `Error`
  - [x] `InvalidReading(message is string)`
  - [x] `text() is string`
- [x] Enforce positive per-call read sizes and reject origins that exceed them.
- [x] Keep limited/chained mutation in specialized opaque runtime counters.

---

# Stage 08 — numbers and mathematics

## `builtin/integer-math`

- [ ] `int.magnitude() is int`
- [ ] `int.factorial() is int`
- [x] `int.sqrt() is int`
- [ ] `int.gcd(other is int) is int`
- [ ] `int.lcm(other is int) is int`
- [ ] `int.combinations(chosen is int) is int`
- [ ] `int.permutations(chosen is int) is int`

## `builtin/float-math`

- [ ] `float.magnitude() is float`
- [ ] `float.floor() is int`
- [ ] `float.ceiling() is int`
- [ ] `float.truncation() is int`
- [ ] `float.rounding() is int`
- [ ] `float.rounding(digits is int) is float`
- [ ] `float.power(exponent is float) is float`
- [ ] `float.square_root() is float`
- [ ] `float.cube_root() is float`
- [ ] `float.exponential() is float`
- [ ] `float.log() is float`
- [ ] `float.log(base is float) is float`
- [ ] `float.log2() is float`
- [ ] `float.log10() is float`
- [ ] `float.sine() is float`
- [ ] `float.cosine() is float`
- [ ] `float.tangent() is float`
- [ ] `float.arc_sine() is float`
- [ ] `float.arc_cosine() is float`
- [ ] `float.arc_tangent() is float`
- [ ] `float.arc_tangent2(x is float) is float`
- [ ] `float.hyperbolic_sine() is float`
- [ ] `float.hyperbolic_cosine() is float`
- [ ] `float.hyperbolic_tangent() is float`
- [ ] `float.degrees() is float`
- [ ] `float.radians() is float`
- [ ] `float.finite() is bool`
- [ ] `float.infinite() is bool`
- [ ] `float.nan() is bool`
- [ ] Add inverse hyperbolic functions.
- [ ] Add `log1p`, `expm1` and fused multiply-add.
- [ ] Add IEEE remainder and float decomposition.
- [ ] Add error, complementary-error, gamma and log-gamma functions.

## `builtin/rat-math`

- [ ] `rat.magnitude() is rat`
- [ ] `rat.floor() is int`
- [ ] `rat.ceiling() is int`
- [ ] `rat.truncation() is int`
- [ ] `rat.rounding() is int`
- [ ] `rat.power(exponent is int) is rat`
- [ ] Do not add stdlib `Rational`.

## `math/rounding`

- [x] enum `Rounding`
  - [x] `toward_zero`
  - [x] `away_from_zero`
  - [x] `floor`
  - [x] `ceiling`
  - [x] `half_even`
  - [x] `half_up`
  - [x] `half_down`

## `math/aggregate`

- [x] object `Minimum<T: Comparing<T>>`
  - [x] `Minimum(values is Iterating<T>)`
  - [x] `#projected value() is T`
- [x] object `Maximum<T: Comparing<T>>`
  - [x] `Maximum(values is Iterating<T>)`
  - [x] `#projected value() is T`
- [x] object `Sum`
  - [x] `Sum(values is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] object `AccurateSum`
  - [x] `AccurateSum(values is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] object `AccurateTotal`
  - [x] `AccurateTotal(sum is float, correction is float)`
  - [x] `addition(value is float) is AccurateTotal`
  - [x] `value() is float`
- [x] object `Product`
  - [x] `Product(values is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] object `SumOfProducts`
  - [x] `SumOfProducts(left is Iterating<float>, right is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] object `Hypotenuse`
  - [x] `Hypotenuse(values is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] object `Distance`
  - [x] `Distance(left is Sequencing<float>, right is Sequencing<float>)`
  - [x] `#projected value() is float`
- [x] object `ScaledSquares`
  - [x] `ScaledSquares(scale is float, squares is float)`
  - [x] `addition(value is float) is ScaledSquares`
  - [x] `value() is float`
- [x] Make minimum/maximum require `Comparing<T>` and throw `EmptySequence` for empty input.
- [x] Define empty identities: sum `0.0`, product `1.0`, hypotenuse `0.0`.
- [x] Use one-pass Neumaier compensated summation for `AccurateSum`.
- [x] Use scaled sums of squares for `Hypotenuse` and `Distance`.
- [x] Reject unequal paired aggregate sizes with `UnequalSizes`.
- [x] Keep square root behind the backend-neutral runtime boundary until builtin float math owns it.

## `math/decimal`

- [x] object `Decimal` does `Comparing<Decimal>`
  - [x] `Decimal(origin is string)`
  - [x] `coefficient() is BigInt`
  - [x] `exponent() is int`
  - [x] `order(other is Decimal) is Order`
  - [x] `string() is string`
- [x] object `InvalidDecimal` does `Error`
  - [x] `InvalidDecimal(origin is string)`
  - [x] `text() is string`
- [ ] object `DecimalSum`
  - [ ] `DecimalSum(left is Decimal, right is Decimal)`
  - [ ] `#projected value() is Decimal`
- [ ] object `DecimalDifference`
  - [ ] `DecimalDifference(left is Decimal, right is Decimal)`
  - [ ] `#projected value() is Decimal`
- [ ] object `DecimalProduct`
  - [ ] `DecimalProduct(left is Decimal, right is Decimal)`
  - [ ] `#projected value() is Decimal`
- [ ] object `DecimalDivision`
  - [ ] `DecimalDivision(left is Decimal, right is Decimal, scale is int, rounding is Rounding)`
  - [ ] `#projected value() is Decimal`
- [ ] object `Quantized`
  - [ ] `Quantized(origin is Decimal, exponent is int, rounding is Rounding)`
  - [ ] `#projected value() is Decimal`
- [x] Do not add ambient or thread-local decimal context.
- [x] Parse, normalize and compare base decimal values without a `float` round trip.

## `numbers/bigint`

- [x] object `BigInt` does `Comparing<BigInt>`
  - [x] `BigInt(origin is string)`
  - [x] `sign() is int`
  - [x] `bits() is int`
  - [x] `order(other is BigInt) is Order`
  - [x] `string() is string`
- [x] object `InvalidBigInt` does `Error`
  - [x] `#projected text() is string`
- [ ] object `BigSum`
  - [ ] `BigSum(left is BigInt, right is BigInt)`
  - [ ] `#projected value() is BigInt`
- [ ] object `BigDifference`
  - [ ] `BigDifference(left is BigInt, right is BigInt)`
  - [ ] `#projected value() is BigInt`
- [ ] object `BigProduct`
  - [ ] `BigProduct(left is BigInt, right is BigInt)`
  - [ ] `#projected value() is BigInt`
- [ ] object `BigQuotient`
  - [ ] `BigQuotient(left is BigInt, right is BigInt)`
  - [ ] `#projected value() is BigInt`
- [ ] object `BigRemainder`
  - [ ] `BigRemainder(left is BigInt, right is BigInt)`
  - [ ] `#projected value() is BigInt`
- [ ] object `BigPower`
  - [ ] `BigPower(base is BigInt, exponent is int)`
  - [ ] `#projected value() is BigInt`
- [ ] object `BigGcd`
  - [ ] `BigGcd(left is BigInt, right is BigInt)`
  - [ ] `#projected value() is BigInt`
- [ ] object `BigLcm`
  - [ ] `BigLcm(left is BigInt, right is BigInt)`
  - [ ] `#projected value() is BigInt`

## `numbers/bigrational`

- [ ] object `BigRational` does `Comparing<BigRational>`
  - [ ] `BigRational(numerator is BigInt, denominator is BigInt)`
  - [ ] `numerator() is BigInt`
  - [ ] `denominator() is BigInt`
  - [ ] `order(other is BigRational) is Order`
  - [ ] `string() is string`
- [ ] Specify big-rational sum result.
- [ ] Specify big-rational difference result.
- [ ] Specify big-rational product result.
- [ ] Specify big-rational quotient result.

## `math/complex`

- [ ] object `Complex`
  - [ ] `Complex(real is float, imaginary is float)`
  - [ ] `real() is float`
  - [ ] `imaginary() is float`
  - [ ] `magnitude() is float`
  - [ ] `phase() is float`
  - [ ] `conjugate() is Complex`
- [ ] object `ComplexSum`
  - [ ] `ComplexSum(left is Complex, right is Complex)`
  - [ ] `#projected value() is Complex`
- [ ] object `ComplexDifference`
  - [ ] `ComplexDifference(left is Complex, right is Complex)`
  - [ ] `#projected value() is Complex`
- [ ] object `ComplexProduct`
  - [ ] `ComplexProduct(left is Complex, right is Complex)`
  - [ ] `#projected value() is Complex`
- [ ] object `ComplexQuotient`
  - [ ] `ComplexQuotient(left is Complex, right is Complex)`
  - [ ] `#projected value() is Complex`
- [ ] object `ComplexPower`
  - [ ] `ComplexPower(base is Complex, exponent is Complex)`
  - [ ] `#projected value() is Complex`
- [ ] object `ComplexString`
  - [ ] `ComplexString(origin is Complex)`
  - [ ] `#projected string() is string`
- [ ] Specify complex exponential results.
- [ ] Specify complex logarithmic results.
- [ ] Specify complex trigonometric results.

## `statistics`

- [x] object `Mean`
  - [x] `Mean(values is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] object `GeometricMean`
  - [x] `GeometricMean(values is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] object `HarmonicMean`
  - [x] `HarmonicMean(values is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] object `Median`
  - [x] `Median(values is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] object `MedianLow`
  - [x] `MedianLow(values is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] object `MedianHigh`
  - [x] `MedianHigh(values is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] enum `QuantileMethod`
  - [x] `exclusive`
  - [x] `inclusive`
- [x] object `Quantiles`
  - [x] `Quantiles(values is Iterating<float>, count is int)`
  - [x] `Quantiles(values is Iterating<float>, count is int, method is QuantileMethod)`
  - [x] `values() is @float`
- [x] object `Mode<T>`
  - [x] `Mode(values is Iterating<T>)`
  - [x] `#projected value() is T`
- [x] object `Modes<T>` does `Sequencing<T>`
  - [x] `Modes(values is Iterating<T>)`
  - [x] `cursor() is Cursor<T>`
  - [x] `size() is int`
  - [x] `item(index is int) is T`
  - [x] `first() is T`
  - [x] `last() is T`
- [x] object `SampleVariance`
  - [x] `SampleVariance(values is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] object `PopulationVariance`
  - [x] `PopulationVariance(values is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] object `SampleDeviation`
  - [x] `SampleDeviation(values is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] object `PopulationDeviation`
  - [x] `PopulationDeviation(values is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] object `Covariance`
  - [x] `Covariance(left is Iterating<float>, right is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] object `Correlation`
  - [x] `Correlation(left is Iterating<float>, right is Iterating<float>)`
  - [x] `#projected value() is float`
- [x] object `LinearRegression`
  - [x] `LinearRegression(left is Iterating<float>, right is Iterating<float>)`
  - [x] `slope() is float`
  - [x] `intercept() is float`
- [x] object `StatisticsError` does `Error`
  - [x] `#projected text() is string`
- [x] Preserve first-appearance ordering for modes.
- [x] Never filter NaN implicitly.
- [x] Route numerically sensitive bulk algorithms through a portable runtime boundary.

## `math/constants`

- [x] Record that the language does not yet support immutable module values.
- [x] Represent finite constants as zero-state projected value objects.
- [x] Add `math.pi` or object `Pi`.
- [x] Add object `Tau`.
- [x] Add object `Euler`.
- [x] Exclude `Infinity` while builtin `float` is finite-only.
- [x] Exclude `NotANumber` while builtin `float` is finite-only.
- [x] Do not add a `Math` utility object.

---

# Stage 09 — randomness and UUID

## `random/source`

- [x] role `Randomizing`
  - [x] `integer(start is int, stop is int) is int`
  - [x] `float() is float`
  - [x] `bytes(size is int) is @byte`
- [x] object `PcgRandom` does `Randomizing`
  - [x] `PcgRandom(seed is int)`
  - [x] `integer(start is int, stop is int) is int`
  - [x] `float() is float`
  - [x] `bytes(size is int) is @byte`
- [x] object `SecureRandom` does `Randomizing`
  - [x] `SecureRandom()`
  - [x] `integer(start is int, stop is int) is int`
  - [x] `float() is float`
  - [x] `bytes(size is int) is @byte`
- [x] object `InvalidRandomRange` does `Error`
  - [x] `start() is int`
  - [x] `stop() is int`
  - [x] `#projected text() is string`
- [x] object `InvalidRandomSize` does `Error`
  - [x] `requested() is int`
  - [x] `maximum() is int`
  - [x] `#projected text() is string`
- [x] object `UnavailableRandom` does `Error`
  - [x] `#projected text() is string`
- [x] Use half-open integer bounds `[start, stop)`.
- [x] Use half-open float bounds `[0.0, 1.0)`.
- [x] Freeze PCG-XSH-RR 64/32 initialization, output and byte-order semantics.
- [x] Test the named PCG algorithm sequence against a golden prefix.
- [x] Use rejection sampling for unbiased integers across the complete int domain.
- [x] Make aliases of one `PcgRandom` share its advancing stream.
- [x] Never add a seed constructor to `SecureRandom`.

## `random/distribution`

- [x] role `Distribution<T>`
  - [x] `sample() is T`
- [x] object `UniformIntegers` does `Distribution<int>`
  - [x] `UniformIntegers(random is Randomizing, start is int, stop is int)`
  - [x] `sample() is int`
- [x] object `UniformFloats` does `Distribution<float>`
  - [x] `UniformFloats(random is Randomizing, start is float, stop is float)`
  - [x] `sample() is float`
- [x] object `NormalDistribution` does `Distribution<float>`
  - [x] `NormalDistribution(random is Randomizing, mean is float, deviation is float)`
  - [x] `sample() is float`
- [x] object `ExponentialDistribution` does `Distribution<float>`
  - [x] `ExponentialDistribution(random is Randomizing, rate is float)`
  - [x] `sample() is float`
- [x] object `GammaDistribution` does `Distribution<float>`
  - [x] `GammaDistribution(random is Randomizing, shape is float, scale is float)`
  - [x] `sample() is float`
- [x] object `BetaDistribution` does `Distribution<float>`
  - [x] `BetaDistribution(random is Randomizing, alpha is float, beta is float)`
  - [x] `sample() is float`
- [x] object `LogNormalDistribution` does `Distribution<float>`
  - [x] `LogNormalDistribution(random is Randomizing, mean is float, deviation is float)`
  - [x] `sample() is float`
- [x] object `VonMisesDistribution` does `Distribution<float>`
  - [x] `VonMisesDistribution(random is Randomizing, mean is float, concentration is float)`
  - [x] `sample() is float`
- [x] object `ParetoDistribution` does `Distribution<float>`
  - [x] `ParetoDistribution(random is Randomizing, shape is float, scale is float)`
  - [x] `sample() is float`
- [x] object `WeibullDistribution` does `Distribution<float>`
  - [x] `WeibullDistribution(random is Randomizing, shape is float, scale is float)`
  - [x] `sample() is float`
- [x] object `Choices<T>` does `Distribution<T>`
  - [x] `Choices(random is Randomizing, values is Sequencing<T>)`
  - [x] `sample() is T`
- [x] object `WeightedChoices<T>` does `Distribution<T>`
  - [x] `WeightedChoices(random is Randomizing, values is Sequencing<T>, weights is Sequencing<float>)`
  - [x] `sample() is T`
- [x] object `Samples<T>` does `Distribution<@T>`
  - [x] `Samples(random is Randomizing, values is Sequencing<T>, count is int)`
  - [x] `sample() is @T`
- [x] object `PermutationSamples<T>` does `Distribution<@T>`
  - [x] `PermutationSamples(random is Randomizing, values is Sequencing<T>)`
  - [x] `sample() is @T`
- [x] object `InvalidDistribution` does `Error`
  - [x] `#projected text() is string`
- [x] Use Box-Muller, Marsaglia-Tsang and Best-Fisher as named algorithms.
- [x] Make `Samples` choose without replacement.
- [x] Scale weights before summing to avoid avoidable overflow.

## `uuid/value`

- [x] enum `UuidVariant`
  - [x] `rfc`
  - [x] `microsoft`
  - [x] `reserved`
  - [x] `future`
- [x] object `Uuid` does `Comparing<Uuid>`
  - [x] `Uuid(origin is string)`
  - [x] `version() is int`
  - [x] `variant() is UuidVariant`
  - [x] `order(other is Uuid) is Order`
  - [x] `string() is string`
  - [x] `bytes() is @byte`
- [x] object `UuidFromBytes`
  - [x] `UuidFromBytes(origin is @byte)`
  - [x] `#projected value() is Uuid`
- [x] object `InvalidUuid` does `Error`
  - [x] `text() is string`
- [x] Accept strict `8-4-4-4-12` ASCII hex text and normalize to lowercase.
- [x] Define `version()` as the raw four-bit version field for every variant.
- [x] Define ordering as unsigned most-significant-byte-first ordering.

## `uuid/generation`

- [x] object `Uuid3`
  - [x] `Uuid3(namespace is Uuid, name is string)`
  - [x] `#projected value() is Uuid`
- [x] object `Uuid5`
  - [x] `Uuid5(namespace is Uuid, name is string)`
  - [x] `#projected value() is Uuid`
- [x] object `Uuid8`
  - [x] `Uuid8(high is int, middle is int, low is int)`
  - [x] Define `high`, `middle` and `low` as unsigned 48-, 12- and 62-bit fields.
  - [x] `#projected value() is Uuid`
- [x] object `Uuid4s` does `Distribution<Uuid>`
  - [x] `Uuid4s(random is SecureRandom)`
  - [x] `sample() is Uuid`
- [ ] object `Uuid7s` does `Distribution<Uuid>`
  - [ ] `Uuid7s(clock is Timing, random is SecureRandom)`
  - [ ] `sample() is Uuid`
- [ ] Add a monotonic per-millisecond counter to `Uuid7s`.
- [x] Defer `Uuid7s` implementation until `time/clock` provides `Timing` and wall-clock milliseconds.
- [x] Exclude UUID 1 and UUID 6 from v3 until a privacy-preserving explicit node-identity capability is approved.
- [x] Encode UUID names as exact UTF-8 before MD5 or SHA-1 hashing.
- [x] Verify UUID 3 and UUID 5 against external namespace-name vectors.

---

# Stage 10 — time, dates and calendars

## `time/duration`

- [x] enum `DurationUnit`
  - [x] `nanosecond`
  - [x] `microsecond`
  - [x] `millisecond`
  - [x] `second`
  - [x] `minute`
  - [x] `hour`
  - [x] `day`
- [x] object `Duration` does `Comparing<Duration>`
  - [x] `Duration(value is rat, unit is DurationUnit)`
  - [x] `seconds() is rat`
  - [x] `nanoseconds() is BigInt`
  - [x] `plus(other is Duration) is Duration`
  - [x] `minus(other is Duration) is Duration`
  - [x] `order(other is Duration) is Order`
- [x] object `ScaledDuration`
  - [x] `ScaledDuration(origin is Duration, factor is rat)`
  - [x] `#projected value() is Duration`
- [x] object `DurationString`
  - [x] `DurationString(origin is Duration)`
  - [x] `#projected string() is string`
- [x] object `InvalidDuration` does `Error`
  - [x] `#projected text() is string`
- [x] Reject fractional nanoseconds.
- [x] Define `day` as exactly 86,400 seconds.
- [x] Use canonical lossless `<rat>s` duration strings.
- [x] Do not add numeric-to-Duration primitive casts.

## `time/clock`

- [x] object `Instant` does `Comparing<Instant>`
  - [x] `Instant(origin is Duration)`
  - [x] `unix_duration() is Duration`
  - [x] `plus(duration is Duration) is Instant`
  - [x] `minus(other is Instant) is Duration`
  - [x] `order(other is Instant) is Order`
- [x] object `MonotonicInstant` does `Comparing<MonotonicInstant>`
  - [x] `MonotonicInstant(origin is Duration)`
  - [x] `reading() is Duration`
  - [x] `plus(duration is Duration) is MonotonicInstant`
  - [x] `minus(other is MonotonicInstant) is Duration`
  - [x] `order(other is MonotonicInstant) is Order`
- [x] role `Timing`
  - [x] `instant() is Instant`
  - [x] `monotonic_instant() is MonotonicInstant`
  - [x] `async wait(duration is Duration) is Timing`
- [x] object `SystemClock` does `Timing`
  - [x] `SystemClock()`
  - [x] `instant() is Instant`
  - [x] `monotonic_instant() is MonotonicInstant`
  - [x] `async wait(duration is Duration) is Timing`
- [x] object `InvalidWait` does `Error`
  - [x] `#projected text() is string`
- [x] Prevent conversion of monotonic instants into civil time.

## `time/date`

- [x] enum `Weekday`
  - [x] `monday`
  - [x] `tuesday`
  - [x] `wednesday`
  - [x] `thursday`
  - [x] `friday`
  - [x] `saturday`
  - [x] `sunday`
- [x] object `Date` does `Comparing<Date>`
  - [x] `Date(year is int, month is int, day is int)`
  - [x] `year() is int`
  - [x] `month() is int`
  - [x] `day() is int`
  - [x] `weekday() is Weekday`
  - [x] `order(other is Date) is Order`
- [x] object `ShiftedDate`
  - [x] `ShiftedDate(origin is Date, days is int)`
  - [x] `#projected value() is Date`
- [x] object `DaysBetween`
  - [x] `DaysBetween(left is Date, right is Date)`
  - [x] `#projected value() is int`
- [x] object `DateOrdinal`
  - [x] `DateOrdinal(origin is Date)`
  - [x] `#projected value() is int`
- [x] object `OrdinalDate`
  - [x] `OrdinalDate(origin is int)`
  - [x] `#projected value() is Date`
- [x] object `InvalidDate` does `Error`
  - [x] `#projected text() is string`
- [x] Use the proleptic Gregorian calendar for years `1..9999`.
- [x] Define `0001-01-01` as Monday and ordinal `1`.
- [x] Define `DaysBetween(left, right)` as signed `right - left`.
- [x] Do not depend on host calendars or time-zone data.

## `time/day`

- [x] object `TimeOfDay` does `Comparing<TimeOfDay>`
  - [x] `TimeOfDay(hour is int, minute is int, second is int, nanosecond is int)`
  - [x] `hour() is int`
  - [x] `minute() is int`
  - [x] `second() is int`
  - [x] `nanosecond() is int`
  - [x] `order(other is TimeOfDay) is Order`
- [x] object `InvalidTimeOfDay` does `Error`
  - [x] `#projected text() is string`
- [x] Reject leap-second field value `60`.
- [x] Keep date, offset and time-zone semantics outside `TimeOfDay`.

## `time/local-datetime`

- [x] object `LocalDateTime` does `Comparing<LocalDateTime>`
  - [x] `LocalDateTime(date is Date, time is TimeOfDay)`
  - [x] `date() is Date`
  - [x] `time() is TimeOfDay`
  - [x] `plus(duration is Duration) is LocalDateTime`
  - [x] `minus(other is LocalDateTime) is Duration`
  - [x] `order(other is LocalDateTime) is Order`
- [x] Use floor semantics when negative arithmetic crosses midnight.
- [x] Treat every local civil day as exactly 86,400 seconds.
- [x] Keep offset, time-zone and instant interpretation outside this module.
- [x] Throw `InvalidDate` when arithmetic leaves years `1..9999`.

## `time/zone`

- [x] role `TimeZone`
  - [x] `name() is string`
  - [x] `offset(at is Instant) is Duration`
  - [x] `daylight_saving(at is Instant) is bool`
  - [x] `abbreviation(at is Instant) is string`
  - [x] `instant(origin is LocalDateTime, fold is Fold) is Instant`
- [x] object `Zone` does `TimeZone`
  - [x] `Zone(name is string)`
  - [x] Implement every `TimeZone` action.
- [x] object `FixedZone` does `TimeZone`
  - [x] `FixedZone(name is string, offset is Duration)`
  - [x] Implement every `TimeZone` action.
- [x] object `ZonedDateTime`
  - [x] `ZonedDateTime(instant is Instant, zone is TimeZone)`
  - [x] `instant() is Instant`
  - [x] `zone() is TimeZone`
  - [x] `local_datetime() is LocalDateTime`
  - [x] `offset() is Duration`
- [x] object `ZoneObservation`
  - [x] `ZoneObservation(origin is ZonedDateTime)`
  - [x] `daylight_saving() is bool`
  - [x] `abbreviation() is string`
- [x] enum `Fold`
  - [x] `earlier`
  - [x] `later`
- [x] object `InstantFromLocal`
  - [x] `InstantFromLocal(origin is LocalDateTime, zone is TimeZone, fold is Fold)`
  - [x] `#projected value() is Instant`
- [x] object `TimeZoneDatabaseVersion`
  - [x] `TimeZoneDatabaseVersion()`
  - [x] `#projected string() is string`
- [x] object `UnknownTimeZone` does `Error`
  - [x] `#projected text() is string`
- [x] object `InvalidTimeZone` does `Error`
  - [x] `#projected text() is string`
- [x] object `MissingLocalTime` does `Error`
  - [x] `#projected text() is string`
- [x] Bundle pinned IANA timezone database `2026c`.
- [x] Record the official archive URL, SHA-256 and public-domain license.
- [x] Expose the bundled timezone database version.
- [x] Make absolute-to-local conversion total within civil years `1..9999`.
- [x] Resolve local folds explicitly and reject gaps with `MissingLocalTime`.
- [x] Test DST transitions, gaps, folds and recurring future rules.
- [x] Never consult host tzdata or the configured host zone.

## `time/iso`

- [x] object `IsoDate`
  - [x] `IsoDate(origin is Date)`
  - [x] `#projected string() is string`
- [x] object `DateFromIso`
  - [x] `DateFromIso(origin is string)`
  - [x] `#projected value() is Date`
- [x] object `IsoLocalDateTime`
  - [x] `IsoLocalDateTime(origin is LocalDateTime)`
  - [x] `#projected string() is string`
- [x] object `LocalDateTimeFromIso`
  - [x] `LocalDateTimeFromIso(origin is string)`
  - [x] `#projected value() is LocalDateTime`
- [x] object `IsoFraction`
  - [x] `IsoFraction(origin is int)`
  - [x] `#projected string() is string`
- [x] object `InvalidIsoDate` does `Error`
  - [x] `#projected text() is string`
- [x] object `InvalidIsoLocalDateTime` does `Error`
  - [x] `#projected text() is string`
- [x] Require exact `YYYY-MM-DD` date syntax.
- [x] Require `YYYY-MM-DDTHH:MM:SS` with optional `1..9` digit fraction.
- [x] Trim only trailing fractional zeroes in canonical output.
- [x] Reject offsets, zones, whitespace and reduced precision.
- [x] Preserve `InvalidDate` and `InvalidTimeOfDay` for impossible fields.

## `time/rfc3339`

- [x] object `Rfc3339`
  - [x] `Rfc3339(origin is Instant, offset is Duration)`
  - [x] `#projected string() is string`
- [x] object `InstantFromRfc3339`
  - [x] `InstantFromRfc3339(origin is string)`
  - [x] `#projected value() is Instant`
- [x] object `InvalidRfc3339` does `Error`
  - [x] `#projected text() is string`
- [x] object `InvalidRfc3339Offset` does `Error`
  - [x] `#projected text() is string`
- [x] Require uppercase `T`/`Z`, seconds and an explicit offset.
- [x] Accept fractional precision `1..9` without rounding.
- [x] Require numeric offsets to contain whole minutes below 24 hours.
- [x] Parse `-00:00` as zero displacement and format zero as canonical `Z`.
- [x] Preserve signed nanoseconds before the Unix epoch.

## `time/format`

- [x] object `FormattedDate`
  - [x] `FormattedDate(origin is Date, pattern is string)`
  - [x] `#projected string() is string`
- [x] object `DateFromFormat`
  - [x] `DateFromFormat(origin is string, pattern is string)`
  - [x] `#projected value() is Date`
- [x] object `FormattedLocalDateTime`
  - [x] `FormattedLocalDateTime(origin is LocalDateTime, pattern is string)`
  - [x] `#projected string() is string`
- [x] object `LocalDateTimeFromFormat`
  - [x] `LocalDateTimeFromFormat(origin is string, pattern is string)`
  - [x] `#projected value() is LocalDateTime`
- [x] object `InvalidTimePattern` does `Error`
  - [x] `offset() is int`
  - [x] `#projected text() is string`
- [x] object `InvalidFormattedDate` does `Error`
  - [x] `offset() is int`
  - [x] `#projected text() is string`
- [x] object `InvalidFormattedLocalDateTime` does `Error`
  - [x] `offset() is int`
  - [x] `#projected text() is string`
- [x] Support `%Y`, `%m`, `%d`, `%H`, `%M`, `%S`, `%f` and `%%`.
- [x] Define `%f` as exactly nine nanosecond digits.
- [x] Require all civil fields when parsing; let `%f` default to zero.
- [x] Match every non-directive Unicode scalar literally.
- [x] Preserve `InvalidDate` and `InvalidTimeOfDay` for impossible fields.
- [x] Never read locale from ambient process state.
- [x] Defer textual month/day names until formatting accepts an explicit locale value.

## `calendar`

- [x] object `YearMonth` does `Comparing<YearMonth>`
  - [x] `YearMonth(year is int, month is int)`
  - [x] `year() is int`
  - [x] `month() is int`
  - [x] `order(other is YearMonth) is Order`
- [x] object `MonthDates` does `Sequencing<Date>`
  - [x] `MonthDates(origin is YearMonth)`
  - [x] `cursor() is Cursor<Date>`
  - [x] `size() is int`
  - [x] `item(index is int) is Date`
  - [x] `first() is Date`
  - [x] `last() is Date`
- [x] object `MonthDateCursor` does `Cursor<Date>`
  - [x] `MonthDateCursor(origin is MonthDates, position is int)`
  - [x] `available() is bool`
  - [x] `item() is Date`
  - [x] `successor() is Cursor<Date>`
- [x] object `CalendarWeek` does `Sequencing<Date>`
  - [x] `CalendarWeek(start is Date)`
  - [x] `cursor() is Cursor<Date>`
  - [x] `size() is int`
  - [x] `item(index is int) is Date`
  - [x] `first() is Date`
  - [x] `last() is Date`
- [x] object `CalendarWeekCursor` does `Cursor<Date>`
  - [x] `CalendarWeekCursor(origin is CalendarWeek, position is int)`
  - [x] `available() is bool`
  - [x] `item() is Date`
  - [x] `successor() is Cursor<Date>`
- [x] object `CalendarMonth` does `Sequencing<Sequencing<Date>>`
  - [x] `CalendarMonth(origin is YearMonth, first_weekday is Weekday)`
  - [x] `cursor() is Cursor<Sequencing<Date>>`
  - [x] `size() is int`
  - [x] `item(index is int) is Sequencing<Date>`
  - [x] `first() is Sequencing<Date>`
  - [x] `last() is Sequencing<Date>`
- [x] object `CalendarMonthCursor` does `Cursor<Sequencing<Date>>`
  - [x] `CalendarMonthCursor(origin is CalendarMonth, position is int)`
  - [x] `available() is bool`
  - [x] `item() is Sequencing<Date>`
  - [x] `successor() is Cursor<Sequencing<Date>>`
- [x] object `CalendarMonthLayout`
  - [x] `CalendarMonthLayout(origin is YearMonth, first_weekday is Weekday)`
  - [x] `first_ordinal() is int`
  - [x] `size() is int`
- [x] object `IsoWeek` does `Comparing<IsoWeek>`
  - [x] `IsoWeek(year is int, week is int)`
  - [x] `year() is int`
  - [x] `week() is int`
  - [x] `first() is Date`
  - [x] `last() is Date`
  - [x] `order(other is IsoWeek) is Order`
- [x] object `LeapYear`
  - [x] `LeapYear(year is int)`
  - [x] `#projected value() is bool`
- [x] object `WeekdayIndex`
  - [x] `WeekdayIndex(origin is Weekday)`
  - [x] `#projected value() is int`
- [x] object `InvalidYearMonth` does `Error`
  - [x] `text() is string`
- [x] object `InvalidCalendarMonth` does `Error`
  - [x] `text() is string`
- [x] object `InvalidIsoWeek` does `Error`
  - [x] `text() is string`
- [x] Keep `MonthDates` restricted to dates belonging to the month.
- [x] Make every `CalendarMonth` row exactly seven dates and include adjacent months.
- [x] Use Monday-through-Sunday ISO weeks and validate week 53.
- [x] Reject calendar rows that would leave the supported `Date` range.

---

# Stage 11 — filesystem

## `fs/path`

- [x] enum `PathStyle`
  - [x] `posix`
  - [x] `windows`
- [x] object `Path` does `Comparing<Path>`
  - [x] `Path(origin is string)`
  - [x] `Path(origin is string, style is PathStyle)`
  - [x] `Path(parts is ...string)`
  - [x] `parent() is Path`
  - [x] `child(part is string) is Path`
  - [x] `absolute() is bool`
  - [x] `order(other is Path) is Order`
  - [x] `string() is string`
- [x] object `NativePathStyle`
  - [x] `NativePathStyle()`
  - [x] `#projected value() is PathStyle`
- [x] object `PathStyleCode`
  - [x] `PathStyleCode(origin is PathStyle)`
  - [x] `#projected value() is int`
- [x] object `InvalidPath` does `Error`
  - [x] `InvalidPath(message is string)`
  - [x] `text() is string`
- [ ] object `PathName`
  - [ ] `PathName(origin is Path)`
  - [ ] `name() is string`
  - [ ] `stem() is string`
  - [ ] `suffix() is string`
  - [ ] `suffixes() is @string`
- [ ] object `PathParts`
  - [ ] `PathParts(origin is Path)`
  - [ ] `drive() is string`
  - [ ] `root() is string`
  - [ ] `segments() is @string`
- [ ] object `NormalizedPath`
  - [ ] `NormalizedPath(origin is Path)`
  - [ ] `#projected value() is Path`
- [ ] object `AbsolutePath`
  - [ ] `AbsolutePath(origin is Path, base is Path)`
  - [ ] `#projected value() is Path`
- [ ] object `RelativePath`
  - [ ] `RelativePath(origin is Path, base is Path)`
  - [ ] `#projected value() is Path`
- [ ] object `ResolvedPath`
  - [ ] `ResolvedPath(origin is Path)`
  - [ ] `#projected value() is Path`
- [x] Keep `Path` purely lexical.
- [x] Make the variadic `Path(parts is ...string)` select the current platform's path grammar internally.
- [x] Keep explicit `PathStyle` only for parsing paths belonging to another platform.
- [x] Reject an empty `Path(parts...)` parts list and empty individual parts.
- [x] Let a later absolute part replace every previously accumulated part.
- [x] Do not normalize `.` or `..` while joining.
- [x] Do not consult the filesystem while joining.
- [x] Define `Path.order()` by lexical spelling without exposing path style.
- [ ] Let only `ResolvedPath` consult the filesystem.

## `fs/file`

- [x] object `FileEntry`
  - [x] `FileEntry(path is Path)`
  - [x] `path() is Path`
  - [x] `existence() is bool`
  - [x] `stat() is FileStat`
  - [x] `remove() is void`
  - [x] `move(destination is Path) is void`
- [x] object `FileContent`
  - [x] `FileContent(origin is FileEntry)`
  - [x] `bytes() is @byte`
- [ ] Specify an ergonomic boolean observation for comparing the contents of
  two `FileEntry` values without a generic `value` projection.
- [ ] Compare file content incrementally with bounded binary chunks.
- [ ] Stop content comparison at the first unequal chunk or unequal EOF.
- [ ] Propagate missing-file, wrong-kind, permission and I/O failures.
- [ ] Remove redundant byte-array `SameContent` instead of materializing files.
- [x] enum `FileMode`
  - [x] `reading`
  - [x] `writing`
  - [x] `appending`
  - [x] `reading_and_writing`
  - [x] `reading_and_appending`
- [x] role `FileLocking`
  - [x] `closed() is bool`
  - [x] `close() is void`
- [x] object `File` does `Writing, WritingText`
  - [x] `File(mode is FileMode, path is Path)`
  - [x] `File(mode is FileMode, path is Path, encoding is string)`
  - [x] `File(mode is FileMode, path is Path, binary is bool)`
  - [x] `File(mode is FileMode, path is Path, encoding is string, binary is bool)`
  - [x] `read() is string`
  - [x] `read(limit is int) is @byte`
  - [x] `write(source is ByteSource) is void`
  - [x] `write(values is ...string) is void`
  - [x] `writeln(values is ...string) is void`
  - [x] `flush() is void`
  - [x] `close() is void`
- [x] enum `SeekOrigin`
  - [x] `start`
  - [x] `current`
  - [x] `end`
- [x] object `FilePosition`
  - [x] `FilePosition(origin is File)`
  - [x] `offset() is int`
  - [x] `seek(offset is int, from is SeekOrigin) is void`
  - [x] `truncate(size is int) is void`
- [x] object `InvalidFileOperation` does `Error`
  - [x] `text() is string`
- [x] object `UnsupportedFileEncoding` does `Error`
  - [x] `text() is string`
- [x] object `FileSystemFailure` does `Error`
  - [x] `code() is string`
  - [x] `text() is string`
- [x] Make open lazy until the first resource action.
- [x] Keep exactly five public action names on `File`: `read`, `write`,
  `writeln`, `flush` and `close`; constructor and action overloads do not add names.
- [x] Default omitted encoding to strict UTF-8 and omitted `binary` to `false`.
- [x] Reject an explicit encoding when `binary` is `true`.
- [x] Accept exact portable encoding names `utf-8`, `utf-16-le`, `utf-16-be`,
  `utf-32-le`, `utf-32-be`, `ascii` and `latin-1`.
- [x] Reject every other encoding name with `UnsupportedFileEncoding`.
- [x] Never infer byte order, consume a BOM or emit a BOM implicitly.
- [x] Make `read()` return one decoded physical line without its LF or preceding CR.
- [x] Make `read()` return a final non-empty unterminated line before `EndOfInput`.
- [x] Make `read(limit)` the bounded binary API and return empty bytes at stable EOF.
- [x] Reject non-positive binary read limits with `InvalidReadLimit`.
- [x] Reject `read()` in binary mode and `read(limit)` in text mode.
- [x] Reject text writes in binary mode and byte writes in text mode.
- [x] Reject every read from a write-only mode and every write from a read-only mode.
- [x] Make `writing` truncate or create, `appending` create and append, and
  `reading_and_writing` require an existing file without truncating.
- [x] Make `reading_and_appending` create when missing and force every write to EOF.
- [x] In update modes, make a write occur at the logical position after decoded
  read-ahead rather than at the runtime provider's hidden buffered position.
- [x] Flush pending encoded output before a following read in an update mode.
- [x] Make `writeln` append exactly one encoded LF after all arguments.
- [x] Make `close()` idempotent and flush pending output.
- [x] Keep byte offsets, seeking and truncation unavailable in text mode.
- [x] Throw `InvalidFileOperation` for an operation excluded by mode or binary/text layer.
- [x] Document that `FileContent.bytes()` materializes the complete file in memory.
- [x] Do not expose blocking file lines through `Iterating` or `Cursor`.
- [x] Put a complete chunk-by-chunk reading example in `fs/file` module documentation.
- [x] Put a complete line-by-line reading example in `fs/file` module documentation.
- [x] Put complete binary and UTF-8 text writing examples in `fs/file` documentation.
- [x] State that chunked reading bounds library buffering by the requested chunk size.
- [x] State that line reading bounds library buffering by the longest physical line.
- [x] Make `existence()` false only for a missing path.
- [x] Propagate permission and I/O errors from `existence()`.
- [x] Share `FileState` with `FilePosition` through field-only
  `allow FilePosition`; do not expose a sixth action or private methods.

Constructor forms for the module documentation:

```okkama
let text = File(mode: FileMode.reading, path: path);
let utf16 = File(
    mode: FileMode.reading,
    path: path,
    encoding: "utf-16-le",
);
let binary = File(
    mode: FileMode.reading,
    path: path,
    binary: true,
);
let explicit = File(
    mode: FileMode.reading,
    path: path,
    encoding: "utf-8",
    binary: false,
);
```

Canonical chunk-by-chunk example for the module documentation:

```okkama
let file = File(mode: FileMode.reading, path: path, binary: true);
defer file.close();

loop {
    let chunk is @byte = file.read(64 * 1024);
    last if chunk.size() == 0;
    consume(chunk);
}
```

Canonical line-by-line example for the module documentation:

```okkama
let file = File(mode: FileMode.reading, path: path);
defer file.close();

loop {
    try {
        consume(file.read());
    }
    catch error is EndOfInput {
        last;
    }
}
```

The first example never materializes the complete file. The second keeps only
decoder read-ahead and the current physical line; a file containing one single
20 GiB line necessarily requires a 20 GiB string to return that line, so such
input must be processed through bounded binary chunks instead.

Canonical binary writing example for the module documentation:

```okkama
let file = File(mode: FileMode.writing, path: path, binary: true);
defer file.close();

file <- bytes;
file.flush();
```

Canonical strict UTF-8 text writing example for the module documentation:

```okkama
let file = File(mode: FileMode.appending, path: path);
defer file.close();

file.writeln("completed: ", task.string());
file.flush();
```

## `fs/lock`

- [x] enum `FileLockMode`
  - [x] `shared`
  - [x] `exclusive`
- [x] enum `FileLockWaiting`
  - [x] `wait`
  - [x] `immediate`
- [x] object `FileLock` does `Closing, FileLocking`
  - [x] `FileLock(origin is File, mode is FileLockMode)`
  - [x] `FileLock(origin is File, mode is FileLockMode, waiting is FileLockWaiting)`
  - [x] `closed() is bool`
  - [x] `close() is void`
- [x] object `UnavailableFileLock` does `Error`
  - [x] `text() is string`
- [x] Lock the opened handle rather than resolving and reopening its path.
- [x] Make shared locks coexist only with shared locks.
- [x] Make exclusive locks conflict with every other lock.
- [x] Make `wait` block until acquisition and `immediate` throw `UnavailableFileLock`.
- [x] Make every lock whole-file and advisory on every platform.
- [x] Normalize platform-specific lock behavior inside the runtime provider.
- [x] Reject a second live lock over the same `File` state.
- [x] Make `close()` idempotent and release the lock.
- [x] Release every live lock when its underlying opened file closes.

## `fs/stat`

- [x] enum `FileKind`
  - [x] `file`
  - [x] `directory`
  - [x] `symbolic_link`
  - [x] `socket`
  - [x] `fifo`
  - [x] `block_device`
  - [x] `character_device`
  - [x] `other`
- [x] object `FileStat`
  - [x] `kind() is FileKind`
  - [x] `size() is int`
  - [x] `mode() is PermissionMode`
  - [x] `times() is FileTimes`
  - [x] `identity() is FileIdentity`
- [x] object `FileTimes`
  - [x] `access() is Instant`
  - [x] `modification() is Instant`
  - [x] `change() is Instant`
  - [x] `creation() is Instant`
- [x] object `FileIdentity`
  - [x] `device() is int`
  - [x] `inode() is BigInt`
- [x] Compare filesystem identity through structural equality of `FileIdentity`:
  `left.stat().identity() == right.stat().identity()`.
- [x] object `Permissions`
  - [x] Expose `readable()`, `writable()` and `executable()` booleans.
- [x] object `PermissionMode`
  - [x] `owner() is Permissions`
  - [x] `group() is Permissions`
  - [x] `others() is Permissions`
  - [x] `special() is int`
- [x] object `MissingCreationTime` does `Error`
  - [x] `text() is string`
- [ ] object `ChangedMode`
  - [ ] `ChangedMode(origin is FileEntry, mode is PermissionMode)`
  - [ ] `#projected value() is FileEntry`
- [ ] object `TouchedFile`
  - [ ] `TouchedFile(origin is FileEntry, at is Instant)`
  - [ ] `#projected value() is FileEntry`

## `fs/copy`

- [ ] enum `Copying`
  - [ ] `content`
  - [ ] `content_and_mode`
  - [ ] `available_metadata`
- [ ] object `CopiedFile`
  - [ ] `CopiedFile(origin is FileEntry, destination is Path, copying is Copying)`
  - [ ] `#projected value() is FileEntry`
- [ ] object `CopiedTree`
  - [ ] `CopiedTree(origin is Directory, destination is Path, copying is Copying)`
  - [ ] `#projected value() is Directory`
- [ ] Document exactly which metadata is portable.
- [ ] Use native fast-copy paths where available.

## `fs/directory`

- [x] role `DirectoryLocating`
  - [x] `path() is Path`
- [x] object `Directory` does `DirectoryLocating`
  - [x] `Directory(path is Path)`
  - [x] `path() is Path`
  - [x] `existence() is bool`
  - [x] `create() is void`
  - [x] `remove() is void`
  - [x] `move(destination is Path) is void`
- [x] object `DirectoryEntry`
  - [x] `DirectoryEntry(path is Path)`
  - [x] `path() is Path`
  - [x] `kind() is FileKind`
  - [x] `stat() is FileStat`
- [x] object `DirectoryEntries` does `Iterating<DirectoryEntry>`
  - [x] `DirectoryEntries(origin is DirectoryLocating)`
  - [x] `cursor() is Cursor<DirectoryEntry>`
- [x] object `DirectoryEntryCursor` does `Cursor<DirectoryEntry>`
  - [x] `DirectoryEntryCursor(parent is Path, names is @string, position is int)`
  - [x] `available() is bool`
  - [x] `item() is DirectoryEntry`
  - [x] `successor() is Cursor<DirectoryEntry>`
- [x] object `Descendants` does `Iterating<DirectoryEntry>`
  - [x] `Descendants(origin is DirectoryLocating)`
  - [x] `cursor() is Cursor<DirectoryEntry>`
- [x] object `DescendantCursor` does `Cursor<DirectoryEntry>`
  - [x] `DescendantCursor(current is Cursor<DirectoryEntry>, following is List<Cursor<DirectoryEntry>>)`
  - [x] `available() is bool`
  - [x] `item() is DirectoryEntry`
  - [x] `successor() is Cursor<DirectoryEntry>`
- [x] object `DescendantCursorFrom`
  - [x] `DescendantCursorFrom(current is Cursor<DirectoryEntry>, following is List<Cursor<DirectoryEntry>>)`
  - [x] `#projected value() is Cursor<DirectoryEntry>`
- [x] object `ExhaustedDescendantCursor` does `Cursor<DirectoryEntry>`
  - [x] `ExhaustedDescendantCursor()`
  - [x] `available() is bool`
  - [x] `item() is DirectoryEntry`
  - [x] `successor() is Cursor<DirectoryEntry>`
- [x] object `RemovedTree`
  - [x] `RemovedTree(origin is DirectoryLocating)`
  - [x] `#projected value() is Path`
- [x] object `DirectoryNotEmpty` does `Error`
  - [x] `DirectoryNotEmpty(path is Path)`
  - [x] `path() is Path`
  - [x] `#projected text() is string`
- [x] Sort entries lexicographically with one native bulk sort.
- [x] Make `Directory.remove()` reject non-empty directories.
- [x] Treat symlinks as leaves during recursive traversal/removal.
- [x] Snapshot only the directory currently entered during recursive traversal.
- [x] Remove separate `Files` and `Directories` collection types.

## `fs/glob`

- [x] object `MatchingPaths` does `Iterating<Path>`
  - [x] `MatchingPaths(root is DirectoryLocating, pattern is string)`
  - [x] `cursor() is Cursor<Path>`
- [x] object `MatchingPathCursor` does `Cursor<Path>`
  - [x] `MatchingPathCursor(paths is @string, position is int)`
  - [x] `available() is bool`
  - [x] `item() is Path`
  - [x] `successor() is Cursor<Path>`
- [x] object `InvalidGlobPattern` does `Error`
  - [x] `InvalidGlobPattern(pattern is string, message is string)`
  - [x] `pattern() is string`
  - [x] `#projected text() is string`
- [x] Use `/` as pattern separator on every OS.
- [x] Support `*`, `?` and character classes.
- [x] Support character ranges and leading `!` class negation.
- [x] Support recursive `**` only as complete segments.
- [x] Match case-sensitively and include leading dots on every OS.
- [x] Return lexicographically sorted paths.
- [x] Snapshot the complete result because global sorting requires materialization.
- [x] Return matching symlink paths but never recurse through them.

`pattern` is always relative to `root`. Matching never depends on the process
working directory. Every yielded `Path` contains the complete path obtained by
joining the matching relative name to `root.path()`.

List Okkama source files immediately inside `src`:

```okkama
let project = Directory(path: Path("."));
let sources is @Path = MatchingPaths(
    root: project,
    pattern: "src/*.ok",
).map() -> path { path; };
```

`*`, `?` and character classes match within one path segment; they never match
`/`. This example finds numbered migration files such as
`migrations/012-create-users.sql`:

```okkama
let migrations is @Path = MatchingPaths(
    root: Directory(path: application_root),
    pattern: "migrations/[0-9][0-9][0-9]-*.sql",
).map() -> path { path; };
```

Use a complete `**` segment for recursive traversal. The result is already
lexicographically sorted, so callers do not need a second `Sorted` decorator:

```okkama
let tests is @string = MatchingPaths(
    root: Directory(path: project_root),
    pattern: "**/*-test.ok",
).map() -> path { path.string(); };
```

Glob results are ordinary `Path` values and compose with the rest of `fs`:

```okkama
let output = Stdout();
for MatchingPaths(
    root: Directory(path: cache_root),
    pattern: "**/*.cache",
) -> path {
    let entry = FileEntry(path: path);
    output.writeln(path.string(), " ", entry.stat().size().string());
}
```

Patterns use `/` even when `root` is a Windows path:

```okkama
let reports = MatchingPaths(
    root: Directory(path: Path("C:\\work")),
    pattern: "reports/202?/quarter-[1-4].csv",
);
```

If a matching entry is a symbolic link, its own path may be yielded. A `**`
search never enters that link, so entries reachable only through its target do
not appear in the result.

## `fs/link`

- [x] object `SymbolicLink`
  - [x] `SymbolicLink(path is Path)`
  - [x] `path() is Path`
  - [x] `existence() is bool`
  - [x] `target() is Path`
  - [x] `create(target is Path) is void`
  - [x] `remove() is void`
- [x] object `HardLink`
  - [x] `HardLink(source is FileEntry, destination is Path)`
  - [x] `create() is void`
- [x] object `NotSymbolicLink` does `Error`
  - [x] `NotSymbolicLink(path is Path)`
  - [x] `path() is Path`
  - [x] `#projected text() is string`
- [x] object `InvalidHardLinkSource` does `Error`
  - [x] `InvalidHardLinkSource(path is Path)`
  - [x] `path() is Path`
  - [x] `#projected text() is string`
- [x] Keep raw link targets distinct from resolved paths.
- [x] Make `SymbolicLink.existence()` inspect the link itself, so broken links exist.
- [x] Never require a symbolic-link target to exist during creation.
- [x] Never replace an existing destination while creating either kind of link.
- [x] Make `SymbolicLink.remove()` remove only the link, never its target.
- [x] Reject directory and symbolic-link sources for `HardLink` portably.

Create a relative symbolic link. `target` is stored exactly as supplied and is
interpreted relative to the directory containing the link by the filesystem:

```okkama
let current = SymbolicLink(
    path: releases.path().child("current"),
);
current.create(target: Path("v3.2.0"));

current.path().string();   // /srv/app/releases/current
current.target().string(); // v3.2.0
```

`target()` deliberately returns the raw target rather than silently resolving
it. A caller that needs a lexical absolute-or-relative resolution can state it
explicitly:

```okkama
let raw is Path = current.target();
let resolved is Path = if raw.absolute() {
    raw;
}
else {
    current.path().parent().child(raw.string());
};
```

A broken symbolic link still exists because `existence()` inspects the link
entry rather than following its target:

```okkama
let link = SymbolicLink(path: Path("cache/latest"));
if link.existence() {
    Stdout().writeln("stored target: ", link.target().string());
}
```

Removing a symbolic link never touches its target:

```okkama
let link = SymbolicLink(path: deployment.path().child("current"));
if link.existence() {
    link.remove();
}
```

Create a hard-linked snapshot of an existing file. The result is an ordinary
`FileEntry` and composes with the rest of `fs`:

```okkama
let destination = backup.path().child("state.db");

HardLink(
    source: FileEntry(path: database.path().child("state.db")),
    destination: destination,
).create();

let snapshot = FileEntry(path: destination);

Stdout().writeln(
    snapshot.path().string(),
    " ",
    snapshot.stat().size().string(),
);
```

Both `create` commands fail when their destination already exists. They never
replace an existing file or link as a side effect.

## `fs/temp`

- [x] object `TemporaryFile` does `Reading, Writing, Closing`
  - [x] `TemporaryFile(directory is DirectoryLocating, prefix is string)`
  - [x] `bytes(limit is int) is @byte`
  - [x] `write(source is ByteSource) is void`
  - [x] `flush() is void`
  - [x] `closed() is bool`
  - [x] `close() is void`
- [x] object `TemporaryFilePath`
  - [x] `TemporaryFilePath(origin is TemporaryFile)`
  - [x] `#projected value() is Path`
- [x] object `PreservedTemporaryFile`
  - [x] `PreservedTemporaryFile(origin is TemporaryFile, destination is Path)`
  - [x] `#projected value() is FileEntry`
- [x] object `TemporaryDirectory` does `Closing`
  - [x] `TemporaryDirectory(parent is DirectoryLocating, prefix is string)`
  - [x] `path() is Path`
  - [x] `closed() is bool`
  - [x] `close() is void`
- [x] Create temporary resources eagerly with unpredictable names and owner-only access.
- [x] Treat `prefix` as one filename component rather than a path.
- [x] Delete temporary resources on idempotent `close()`.
- [x] Make preservation consume and close the temporary file.
- [x] Never replace an existing preservation destination.
- [x] Preserve across filesystems with bounded copying.
- [x] Recursively clean temporary directories without following symlinks.

## `fs/mmap` — low priority

- [ ] object `MemoryMap` does `Reading, Writing, Closing`
  - [ ] `MemoryMap(file is FileEntry, offset is int, size is int)`
  - [ ] `bytes(limit is int) is @byte`
  - [ ] `write(source is ByteSource) is void`
  - [ ] `flush() is void`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `MappedPosition`
  - [ ] Specify position/seek surface within five actions.
- [ ] Provide an explicit emulation on platforms without mmap.

## `fs/watch`

- [x] enum `FileEventKind`
  - [x] `created`
  - [x] `modified`
  - [x] `removed`
  - [x] `renamed`
- [x] object `FileEvent`
  - [x] `FileEvent(path is Path, kind is FileEventKind)`
  - [x] `FileEvent(path is Path, kind is FileEventKind, previous_path is Path)`
  - [x] `path() is Path`
  - [x] `kind() is FileEventKind`
  - [x] `previous_path() is Path`
- [x] object `FileWatch` does `Closing`
  - [x] `FileWatch(path is Path)`
  - [x] `async event() is FileEvent`
  - [x] `closed() is bool`
  - [x] `close() is void`
- [x] object `MissingPreviousPath` does `Error`
  - [x] `path() is Path`
  - [x] `text() is string`
- [x] Make `previous_path()` throw `MissingPreviousPath` when inapplicable.
- [x] Watch an existing file or an existing directory and its immediate children.
- [x] Keep directory watching non-recursive.
- [x] Permit exactly one pending `event()` call per watch.
- [x] Let providers coalesce changes that occur between observations.
- [x] Never expose file-watch events through synchronous `Cursor`.

---

# Stage 12 — environment, platform and processes

## `os/environment`

- [x] object `Environment`
  - [x] `Environment()`
  - [x] `Environment(values is %string:string)`
  - [x] `membership(name is string) is bool`
  - [x] `variable(name is string) is string`
  - [x] `variables() is %string:string`
  - [x] `add(name is string, value is string) is Environment`
  - [x] `diff(name is string) is Environment`
- [x] object `HomeDirectory`
  - [x] `HomeDirectory(environment is Environment)`
  - [x] `#projected value() is Directory`
- [x] object `SearchPath` does `Sequencing<Path>`
  - [x] `SearchPath(environment is Environment)`
  - [x] `cursor() is Cursor<Path>`
  - [x] `size() is int`
  - [x] `item(index is int) is Path`
  - [x] `first() is Path`
  - [x] `last() is Path`
- [x] object `MissingEnvironmentVariable` does `Error`
  - [x] `MissingEnvironmentVariable(name is string)`
  - [x] `name() is string`
  - [x] `text() is string`
- [x] object `InvalidEnvironmentVariable` does `Error`
  - [x] `InvalidEnvironmentVariable(message is string)`
  - [x] `text() is string`
- [x] object `MissingHomeDirectory` does `Error`
  - [x] `MissingHomeDirectory()`
  - [x] `text() is string`
- [x] Make `Environment()` return an immutable snapshot of the process environment.
- [x] Make `Environment` copy its input map.
- [x] Make `variables()` return an immutable snapshot.
- [x] Make `variable()` throw `MissingEnvironmentVariable` for an absent name.
- [x] Reject empty names and names containing `=` or NUL.
- [x] Reject values containing NUL.
- [x] Select the executable-search-path separator inside the runtime provider.
- [x] Preserve search-path ordering and duplicate entries.
- [x] Interpret an empty search-path component as `Path(".")`.
- [x] Make a missing `PATH` produce an empty `SearchPath`.

`Environment()` reads the process environment once and detaches the resulting
snapshot from every later ambient change:

```okkama
let environment = Environment();

if environment.membership("CI") {
    output.writeln("running in CI");
}

if environment.membership("EDITOR") {
    output.writeln("editor: ", environment.variable("EDITOR"));
}
```

An absent variable is different from a present variable containing an empty
string. Use `membership()` for optional inspection; `variable()` is the strict
accessor:

```okkama
let token is string = try {
    environment.variable("APPLICATION_TOKEN");
}
catch error is MissingEnvironmentVariable {
    throw ConfigurationError(
        "APPLICATION_TOKEN must be defined"
    );
};
```

`Environment` is immutable. `add()` and `diff()` produce new environments and
leave their origin unchanged:

```okkama
let child is Environment = environment
    .add("LANG", "C.UTF-8")
    .add("APPLICATION_ENV", "production")
    .diff("SSH_AUTH_SOCK")
    .diff("APPLICATION_TOKEN");
```

A completely explicit environment does not inherit ambient variables:

```okkama
let isolated = Environment(values: [
    "LANG": "C.UTF-8",
    "LOG_LEVEL": "debug",
]);
```

Home-directory selection is derived from the supplied environment using the
current platform's rules. It never reads ambient state behind the caller's
back:

```okkama
let home is Directory = value HomeDirectory(
    environment: environment,
);

let configuration = home.path()
    .child(".config")
    .child("okkama");
```

Failure to derive a home directory is explicit:

```okkama
let home is Directory = try {
    value HomeDirectory(environment: environment);
}
catch error is MissingHomeDirectory {
    throw ConfigurationError("home directory is unavailable");
};
```

`SearchPath` parses `PATH` with the separator selected by the runtime for the
current OS. Callers never choose between `:` and `;`. Iteration preserves entry
order and duplicates:

```okkama
let search = SearchPath(environment: environment);

for search -> directory {
    output.writeln(directory.string());
}

if search.size() > 0 {
    output.writeln("first PATH entry: ", search.first().string());
}
```

For example, `/bin:/opt/bin:/bin` yields `/bin`, `/opt/bin`, `/bin`. An empty
component denotes `Path(".")`; a missing `PATH` yields an empty sequence.

## `os/platform`

- [x] enum `OperatingSystem`
  - [x] `linux`
  - [x] `macos`
  - [x] `windows`
  - [x] `freebsd`
  - [x] `other`
- [x] object `Platform`
  - [x] `Platform()`
  - [x] `operating_system() is OperatingSystem`
  - [x] `architecture() is string`
  - [x] `byte_order() is ByteOrder`
- [x] object `Host`
  - [x] `Host()`
  - [x] `name() is string`
  - [x] `cpu_count() is int`
- [x] object `UnavailableHostInformation` does `Error`
  - [x] `UnavailableHostInformation(message is string)`
  - [x] `text() is string`
- [x] Snapshot platform and host properties during construction.
- [x] Keep `cpu_count()` positive on every supported runtime.
- [x] Remove broad `System` information bag.

## `os/account`

- [x] object `User`
  - [x] `User()`
  - [x] `User(name is string)`
  - [x] `name() is string`
  - [x] `identity() is string`
  - [x] `home() is Directory`
- [x] object `Group`
  - [x] `Group(name is string)`
  - [x] `name() is string`
  - [x] `identity() is string`
  - [x] `users() is Sequencing<User>`
- [x] object `GroupUsers` does `Sequencing<User>`
  - [x] `GroupUsers(name is string)`
  - [x] `cursor() is Cursor<User>`
  - [x] `size() is int`
  - [x] `item(index is int) is User`
  - [x] `first() is User`
  - [x] `last() is User`
- [x] object `MissingUser` does `Error`
  - [x] `MissingUser(name is string)`
  - [x] `name() is string`
  - [x] `text() is string`
- [x] object `MissingGroup` does `Error`
  - [x] `MissingGroup(name is string)`
  - [x] `name() is string`
  - [x] `text() is string`
- [x] object `MissingUserHome` does `Error`
  - [x] `MissingUserHome(name is string)`
  - [x] `name() is string`
  - [x] `text() is string`
- [x] object `InvalidAccountName` does `Error`
  - [x] `InvalidAccountName(name is string)`
  - [x] `text() is string`
- [x] object `UnavailableAccountInformation` does `Error`
  - [x] `UnavailableAccountInformation(message is string)`
  - [x] `text() is string`
- [x] Make every account constructor assign attributes only.
- [x] Resolve users and groups lazily in observing actions.
- [x] Make each group-user cursor retain one stable membership snapshot.
- [x] Include direct and primary-group users without recursively expanding groups.
- [x] Use platform-neutral string identities instead of universal integer UID/GID.
- [x] Remove `CurrentUser`, `UserNamed` and `GroupNamed` projection wrappers.

The zero-argument `User` selects the process effective identity. A named user
or group is equally direct; no `value` projection is involved:

```okkama
let current = User();
let deploy = User(name: "deploy");
let operators = Group(name: "operators");

output.writeln(current.name(), " ", current.identity());
output.writeln(deploy.home().path().string());

for operators.users() -> user {
    output.writeln(user.name());
}
```

Construction performs no account-database access. `name()`, `identity()` and
`home()` observe the selected user or group when called. A `GroupUsers` action
observes membership when called, and `cursor()` keeps that particular traversal
stable even if the host account database changes later.

## `os/directory`

- [x] object `WorkingDirectory` does `DirectoryLocating`
  - [x] `WorkingDirectory()`
  - [x] `path() is Path`
- [x] Make construction perform no runtime work.
- [x] Resolve the process working directory lazily in `path()`.
- [x] Do not expose process-global `chdir`.
- [x] Accept `DirectoryLocating` wherever only a directory path is required.
- [x] Pass working directories explicitly to commands.

`WorkingDirectory` is used directly without a projected `value()` or an
adapter through the concrete `Directory` object:

```okkama
let working = WorkingDirectory();

Stdout().writeln(working.path().string());

for DirectoryEntries(origin: working) -> entry {
    Stdout().writeln(entry.path().string());
}
```

The same role is the directory boundary for commands and filesystem objects:

```okkama
let command = Command(
    program: program,
    arguments: arguments,
    environment: environment,
    directory: WorkingDirectory(),
);
```

## `os/command`

- [x] role `Commanding`
  - [x] `program() is Path`
  - [x] `arguments() is @string`
  - [x] `environment() is Environment`
  - [x] `directory() is DirectoryLocating`
  - [x] `string() is string`
- [x] object `Command` does `Commanding`
  - [x] `Command(program is Path, arguments is @string, environment is Environment, directory is DirectoryLocating)`
  - [x] `program() is Path`
  - [x] `arguments() is @string`
  - [x] `environment() is Environment`
  - [x] `directory() is DirectoryLocating`
  - [x] `string() is string`
- [x] object `InheritedCommand` does `Commanding`
  - [x] `InheritedCommand(program is Path, arguments is @string)`
  - [x] `program() is Path`
  - [x] `arguments() is @string`
  - [x] `environment() is Environment`
  - [x] `directory() is DirectoryLocating`
  - [x] `string() is string`
- [x] Make every constructor assign attributes only.
- [x] Make `InheritedCommand.environment()` observe `Environment()` lazily.
- [x] Make `InheritedCommand.directory()` return `WorkingDirectory()`.
- [x] Keep shell parsing and shell execution out of `Commanding`.
- [x] Treat a bare program path such as `Path("git")` as a name searched through the command environment's `PATH`.
- [x] Treat a program path containing a platform separator as an explicit path.
- [x] Keep Windows executable-extension policy inside the runtime provider.
- [x] Report a missing searched program as `ProcessFailure` with code `not_found`.
- [x] Do not add a projected `Executable` lookup wrapper.
- [x] Make `string()` diagnostic, not shell-round-trippable.
- [x] Format `string()` as `Command(program: "...", arguments: ["..."], directory: "...")`.
- [x] Escape diagnostic fields with Okkama string-literal escaping.
- [x] Never include environment variables in `string()`.
- [x] Document that arguments may contain secrets and diagnostic strings must not be logged blindly.
- [x] Remove `CommandString` and projected `LocalCommand` builders.

Use `Command` when reproducibility matters. Every piece of execution context is
supplied explicitly, so nothing depends on the process that happens to launch
it:

```okkama
let environment = Environment(values: [
    "PATH": "/usr/bin:/bin",
    "LANG": "C.UTF-8",
]);
let repository = Directory(path: Path("/srv/application"));

let status is Commanding = Command(
    program: Path("/usr/bin/git"),
    arguments: ["status", "--short"],
    environment: environment,
    directory: repository,
);
```

Use `InheritedCommand` when the command should deliberately inherit a snapshot
of the current process environment and observe its working directory:

```okkama
let version is Commanding = InheritedCommand(
    program: Path("/usr/bin/git"),
    arguments: ["--version"],
);
```

A bare `Path` searches `PATH` from that command's environment. This is direct
process execution, not shell parsing:

```okkama
let version is Commanding = InheritedCommand(
    program: Path("git"),
    arguments: ["--version"],
);
```

Include a directory component when the exact executable matters:

```okkama
let version is Commanding = InheritedCommand(
    program: Path("/usr/bin/git"),
    arguments: ["--version"],
);
```

The same rule applies to `Command`, except that lookup uses its explicitly
supplied `Environment`. Windows executable extensions are a runtime-provider
policy; portable callers never append `.exe` themselves.

Both objects expose the same API and pass directly to process consumers. There
is no `value()` projection:

```okkama
Stdout().writeln("running: ", status.string());

let completed = await CommandCapture(
    origin: status,
    limit: 16 * 1024 * 1024,
).completion();

Stdout().writeln("exit code: ", completed.exit().code().string());
Stdout() <- completed.output();
```

`string()` is intended for diagnostics and resembles this:

```text
Command(program: "/usr/bin/git", arguments: ["status", "--short"], directory: "/srv/application")
```

It is not input for a shell, cannot be executed, and intentionally omits the
environment. Even so, arguments themselves may contain passwords or tokens;
callers must not treat the diagnostic string as automatically safe to log.

## `os/process`

- [x] enum `Signal`
  - [x] `interrupt`
  - [x] `terminate`
  - [x] `kill`
- [x] enum `ExitKind`
  - [x] `exited`
  - [x] `interrupted`
  - [x] `terminated`
  - [x] `killed`
  - [x] `other`
- [x] object `Exit`
  - [x] `Exit(kind is ExitKind, code is int)`
  - [x] `kind() is ExitKind`
  - [x] `code() is int`
  - [x] `success() is bool`
- [x] object `Process`
  - [x] `Process(command is Commanding)`
  - [x] `id() is int`
  - [x] `running() is bool`
  - [x] `async exit() is Exit`
  - [x] `signal(value is Signal) is void`
  - [x] `close() is void`
- [x] object `ProcessInput` does `Writing, Closing`
  - [x] `ProcessInput(origin is Process)`
  - [x] `write(source is ByteSource) is void`
  - [x] `flush() is void`
  - [x] `closed() is bool`
  - [x] `close() is void`
- [x] object `ProcessOutput` does `Reading`
  - [x] `ProcessOutput(origin is Process)`
  - [x] `bytes(limit is int) is @byte`
- [x] object `ProcessErrors` does `Reading`
  - [x] `ProcessErrors(origin is Process)`
  - [x] `bytes(limit is int) is @byte`
- [x] object `MissingExitCode` does `Error`
  - [x] `text() is string`
- [x] object `UnsupportedSignal` does `Error`
  - [x] `UnsupportedSignal(signal is Signal)`
  - [x] `signal() is Signal`
  - [x] `text() is string`
- [x] object `ProcessFailure` does `Error`
  - [x] `ProcessFailure(code is string, message is string)`
  - [x] `code() is string`
  - [x] `text() is string`
- [x] Add one shared opaque lazy `ProcessState` to the runtime ABI.
- [x] Allocate an empty `ProcessState` without spawning a child.
- [x] Resolve inherited environment and working directory at the first start action.
- [x] Spawn exactly once at the first lifecycle or stream action.
- [x] Make `Process.close()` before spawning close without starting the command.
- [x] Make `Process.close()` forcefully stop and reap a running child.
- [x] Make `Process.close()` idempotent and close every pipe.
- [x] Make `ProcessInput.close()` idempotent and deliver EOF without terminating the child.
- [x] Make `ProcessInput.flush()` a valid no-op for the unbuffered pipe.
- [x] Make `Exit.code()` throw `MissingExitCode` unless `kind()` is `exited`.
- [x] Define `success()` as an `exited` status with code zero.
- [x] Return empty bytes only at stable stdout or stderr EOF.
- [x] Reject non-positive read limits before spawning.
- [x] Make stdin writes all-or-`IncompleteWrite`.
- [x] Keep `hangup` and `quit` out of the portable module.
- [x] Keep process lifecycle on `Process`, not its stream views.
- [x] Document that waiting before draining both output streams can deadlock.

Write bytes to stdin and close only that stream to deliver EOF. The child keeps
running and its output remains readable:

```okkama
let process = Process(command: command);
let input = ProcessInput(origin: process);
let output = ProcessOutput(origin: process);

input <- request;
input.close();

let response = output.bytes(64 * 1024);
let exit = await process.exit();
process.close();
```

`Exit.kind()` is total. `code()` is intentionally available only for an
ordinary exit:

```okkama
let exit = await process.exit();

match exit.kind() {
    case ExitKind.exited {
        Stdout().writeln("exit code: ", exit.code().string());
    }
    case ExitKind.killed {
        Stdout().writeln("process was killed");
    }
    else {
        Stdout().writeln("abnormal termination");
    }
}
```

Direct stream users must drain stdout and stderr concurrently whenever either
stream can exceed the operating-system pipe capacity. Waiting first can
deadlock. `CommandCapture` is the higher-level API for bounded capture without
manual stream orchestration.

## `os/command-capture`

- [x] enum `CapturedStream`
  - [x] `output`
  - [x] `errors`
- [x] object `CommandCapture`
  - [x] `CommandCapture(origin is Commanding, limit is int)`
  - [x] `CommandCapture(origin is Commanding, input is @byte, limit is int)`
  - [x] `async completion() is CompletedCommand`
- [x] object `CompletedCommand`
  - [x] `CompletedCommand(exit is Exit, output is @byte, errors is @byte)`
  - [x] `exit() is Exit`
  - [x] `output() is @byte`
  - [x] `errors() is @byte`
- [x] object `InvalidCaptureLimit` does `Error`
  - [x] `InvalidCaptureLimit(limit is int)`
  - [x] `limit() is int`
  - [x] `text() is string`
- [x] object `CaptureLimitExceeded` does `Error`
  - [x] `CaptureLimitExceeded(stream is CapturedStream, limit is int)`
  - [x] `stream() is CapturedStream`
  - [x] `limit() is int`
  - [x] `text() is string`
- [x] Reject non-positive limits before observing the command.
- [x] Apply the limit independently to stdout and stderr.
- [x] Close stdin immediately when input is omitted.
- [x] Write all explicit input and then close stdin.
- [x] Drain stdout and stderr concurrently while waiting for process termination.
- [x] Complete only after both output streams reach EOF and the process terminates.
- [x] Terminate and reap the child when either capture limit is exceeded.
- [x] Return no partial `CompletedCommand` after a capture failure.
- [x] Decode captured output explicitly through an encoding module.
- [x] Remove separate `OutputProcess` and `TextProcess`.

Capture a command that does not need stdin. The limit bounds each output
stream separately, so this permits at most 16 MiB of stdout and 16 MiB of
stderr:

```okkama
let completed = await CommandCapture(
    origin: InheritedCommand(
        program: Path("/usr/bin/git"),
        arguments: ["status", "--short"],
    ),
    limit: 16 * 1024 * 1024,
).completion();

if completed.exit().success() {
    let output = Utf8Decoded(origin: completed.output()).string();
    Stdout().write(output);
}
else {
    let errors = Utf8Decoded(origin: completed.errors()).string();
    Stderr().write(errors);
}
```

Pass stdin as bytes and choose its encoding explicitly. `completion()` writes
the full request, closes stdin to deliver EOF, and drains both outputs while
the child is running:

```okkama
let request = Utf8Encoded(origin: "hello\n").bytes();
let completed = await CommandCapture(
    origin: InheritedCommand(
        program: Path("/usr/bin/tr"),
        arguments: ["a-z", "A-Z"],
    ),
    input: request,
    limit: 1024 * 1024,
).completion();

let response = Utf8Decoded(origin: completed.output()).string();
Stdout().write(response);
```

Captured output is always bytes. No locale or ambient encoding is consulted.
If either stream exceeds its limit, `completion()` throws
`CaptureLimitExceeded`, identifies the overflowing `CapturedStream`, and
leaves no child process running.

## `os/pipeline-capture`

- [x] enum `PipelineStream`
  - [x] `output`
  - [x] `errors`
- [x] object `PipelineCapture`
  - [x] `PipelineCapture(commands is @Commanding, limit is int)`
  - [x] `PipelineCapture(commands is @Commanding, input is @byte, limit is int)`
  - [x] `async completion() is CompletedPipeline`
- [x] object `CompletedPipeline`
  - [x] `CompletedPipeline(exits is @Exit, output is @byte, errors is @@byte)`
  - [x] `exits() is @Exit`
  - [x] `output() is @byte`
  - [x] `errors() is @@byte`
- [x] object `EmptyPipeline` does `Error`
  - [x] `text() is string`
- [x] object `InvalidPipelineCaptureLimit` does `Error`
  - [x] `InvalidPipelineCaptureLimit(limit is int)`
  - [x] `limit() is int`
  - [x] `text() is string`
- [x] object `PipelineCaptureLimitExceeded` does `Error`
  - [x] `PipelineCaptureLimitExceeded(command_index is int, stream is PipelineStream, limit is int)`
  - [x] `command_index() is int`
  - [x] `stream() is PipelineStream`
  - [x] `limit() is int`
  - [x] `text() is string`
- [x] Reject an empty command array before observing any command.
- [x] Reject a non-positive limit before observing any command.
- [x] Connect processes directly without an implicit shell.
- [x] Connect each stdout to the following stdin.
- [x] Capture only the final stdout.
- [x] Preserve one stderr byte array per command in command order.
- [x] Preserve one `Exit` per command in command order.
- [x] Close the first stdin immediately when input is omitted.
- [x] Write all explicit input and then close the first stdin.
- [x] Drain final stdout and every stderr concurrently.
- [x] Wait for every process and every captured-stream EOF before completing.
- [x] Apply the limit independently to final stdout and every stderr stream.
- [x] Terminate and reap every started pipeline process after overflow or partial spawn failure.
- [x] Return no partial `CompletedPipeline` after failure.
- [x] Remove `Pipeline`, `PipelineResult` and `PipelineErrors` wrappers.

Connect commands directly. No shell parses these arguments, and the stdout of
each command becomes the stdin of the next command:

```okkama
let commands is @Commanding = [
    InheritedCommand(
        program: Path("/usr/bin/find"),
        arguments: [".", "-name", "*.ok"],
    ),
    InheritedCommand(
        program: Path("/usr/bin/sort"),
        arguments: [],
    ),
];

let completed = await PipelineCapture(
    commands: commands,
    limit: 16 * 1024 * 1024,
).completion();

let output = Utf8Decoded(origin: completed.output()).string();
Stdout().write(output);
```

Supply binary stdin explicitly when the first command consumes input:

```okkama
let request = Utf8Encoded(origin: "beta\nalpha\n").bytes();
let completed = await PipelineCapture(
    commands: [
        InheritedCommand(
            program: Path("/usr/bin/tr"),
            arguments: ["a-z", "A-Z"],
        ),
        InheritedCommand(
            program: Path("/usr/bin/sort"),
            arguments: [],
        ),
    ],
    input: request,
    limit: 1024 * 1024,
).completion();
```

Results use command indices consistently. `exits()[index]` and
`errors()[index]` describe the same command; `output()` is stdout from only the
last command:

```okkama
for 0..completed.exits().size() -> index {
    let exit = completed.exits()[index];
    let errors = Utf8Decoded(origin: completed.errors()[index]).string();
    Stdout().writeln(
        "command ", index.string(),
        ": success=", exit.success().string(),
        ", stderr=", errors,
    );
}
```

Every captured value remains bytes until explicitly decoded. The limit is
independent for final stdout and each stderr. `PipelineCaptureLimitExceeded`
reports both the stream and command index, and leaves no pipeline process
running.

## `os/storage`

- [x] object `Storage`
  - [x] `Storage(path is Path)`
  - [x] `path() is Path`
  - [x] `usage() is StorageUsage`
- [x] object `StorageUsage`
  - [x] `StorageUsage(capacity is BigInt, used is BigInt, available is BigInt)`
  - [x] `capacity() is BigInt`
  - [x] `used() is BigInt`
  - [x] `available() is BigInt`
- [x] object `InvalidStorageUsage` does `Error`
  - [x] `text() is string`
- [x] object `StorageFailure` does `Error`
  - [x] `StorageFailure(code is string, message is string)`
  - [x] `code() is string`
  - [x] `text() is string`
- [x] Make `Storage` denote the mounted filesystem or volume containing its path.
- [x] Do not imply that the path belongs to a physical disk.
- [x] Make every `usage()` call perform one fresh coherent provider observation.
- [x] Express every quantity as an arbitrary-precision byte count.
- [x] Define `available()` as capacity available to the current process after reservations and quotas.
- [x] Define `used()` as `capacity() - available()`.
- [x] Guarantee `capacity() == used() + available()` for every snapshot.
- [x] Reject nonexistent paths rather than returning zero capacity.
- [x] Follow symbolic links when locating the containing storage.
- [x] Keep percentages and human-readable size formatting out of this module.

Observe the storage containing a path. Construction retains only the path;
the operating system is not consulted until `usage()`:

```okkama
let storage = Storage(path: Path("/srv/application"));
let usage = storage.usage();

Stdout().writeln(
    "capacity: ", usage.capacity().string(),
    ", used: ", usage.used().string(),
    ", available: ", usage.available().string(),
);
```

`StorageUsage` is an immutable snapshot. Retain it when several values must
describe the same observation; call `usage()` again when current information
is required:

```okkama
let before = storage.usage();
write_large_result();
let after = storage.usage();

Stdout().writeln("before: ", before.available().string());
Stdout().writeln("after: ", after.available().string());
```

The three quantities are bytes and always satisfy:

```text
capacity == used + available
```

`available()` is process-relative: filesystem reservations and applicable
quotas may make it smaller than the filesystem's raw unallocated block count.

---

# Stage 13 — structured serialization

## `json/value`

- [x] enum `JsonKind`
  - [x] `null_value`
  - [x] `boolean_value`
  - [x] `number_value`
  - [x] `string_value`
  - [x] `array_value`
  - [x] `object_value`
- [x] role `JsonValue`
  - [x] `kind() is JsonKind`
  - [x] `dispatch(to is HandlingJsonValue) is void`
- [x] role `HandlingJsonValue`
  - [x] `handle() is void` for null
  - [x] `handle(value is bool) is void`
  - [x] `handle(value is Decimal) is void`
  - [x] `handle(value is string) is void`
  - [x] `handle(values is Sequencing<JsonValue>) is void`
  - [x] `handle(members is Sequencing<JsonMember>) is void`
  - [x] Count the six overloads as one public action name: `handle`.
- [x] object `JsonMember`
  - [x] `JsonMember(name is string, value is JsonValue)`
  - [x] `name() is string`
  - [x] `value() is JsonValue`
- [x] object `JsonNull` does `JsonValue`
  - [x] `JsonNull()`
  - [x] `kind() is JsonKind`
  - [x] `dispatch(to is HandlingJsonValue) is void`
- [x] object `JsonBoolean` does `JsonValue`
  - [x] `JsonBoolean(value is bool)`
  - [x] `kind() is JsonKind`
  - [x] `value() is bool`
  - [x] `dispatch(to is HandlingJsonValue) is void`
- [x] object `JsonNumber` does `JsonValue`
  - [x] `JsonNumber(value is Decimal)`
  - [x] `kind() is JsonKind`
  - [x] `value() is Decimal`
  - [x] `dispatch(to is HandlingJsonValue) is void`
- [x] object `JsonText` does `JsonValue`
  - [x] `JsonText(value is string)`
  - [x] `kind() is JsonKind`
  - [x] `value() is string`
  - [x] `dispatch(to is HandlingJsonValue) is void`
- [x] object `JsonArray` does `JsonValue`
  - [x] `JsonArray(values is Sequencing<JsonValue>)`
  - [x] `kind() is JsonKind`
  - [x] `values() is Sequencing<JsonValue>`
  - [x] `dispatch(to is HandlingJsonValue) is void`
- [x] object `JsonObject` does `JsonValue`
  - [x] `JsonObject(members is Sequencing<JsonMember>)`
  - [x] `kind() is JsonKind`
  - [x] `membership(name is string) is bool`
  - [x] `value(name is string) is JsonValue`
  - [x] `members() is Sequencing<JsonMember>`
  - [x] `dispatch(to is HandlingJsonValue) is void`
- [x] object `MissingJsonMember` does `Error`
  - [x] `MissingJsonMember(name is string)`
  - [x] `name() is string`
  - [x] `text() is string`
- [x] object `DuplicateJsonMember` does `Error`
  - [x] `DuplicateJsonMember(name is string)`
  - [x] `DuplicateJsonMember(name is string, offset is int, line is int, column is int)`
  - [x] `name() is string`
  - [x] `offset() is int`
  - [x] `line() is int`
  - [x] `column() is int`
  - [x] `text() is string`
- [x] Keep every JSON value immutable.
- [x] Preserve JSON-object insertion order through `Sequencing<JsonMember>`.
- [x] Reject duplicate member names instead of choosing first-wins or last-wins.
- [x] Do not make `JsonArray` implement `Sequencing<JsonValue>`.
- [x] Do not add a universal structured-value role above `JsonValue`.
- [x] Do not use builtin maps as ordered JSON-object storage.

### `json/value` API examples

Build heterogeneous arrays through the nominal `JsonValue` boundary:

```okkama
let values is @JsonValue = [
    JsonText("Ada"),
    JsonNumber(Decimal("37")),
    JsonBoolean(true),
    JsonNull(),
];
let document = JsonArray(values: values);
```

Objects take an ordered sequence of members rather than a builtin map:

```okkama
let profile = JsonObject(members: [
    JsonMember(name: "name", value: JsonText("Ada")),
    JsonMember(name: "active", value: JsonBoolean(true)),
]);

if profile.membership("name") {
    let name is JsonValue = profile.value("name");
}

for profile.members() -> member {
    Stdout().writeln(member.name(), ": ", member.value().kind().string());
}
```

`value()` throws `MissingJsonMember` when the name is absent. Construction
throws `DuplicateJsonMember` for repeated names, so lookup and serialization
never need an implicit first-wins or last-wins policy.

Broad values use typed double dispatch instead of a downcast:

```okkama
object DescribingJson does HandlingJsonValue {
    has output is WritingText;
    DescribingJson(:output);

    handle() is void { output.writeln("null"); }
    handle(value is bool) is void { output.writeln("boolean: ", value.string()); }
    handle(value is Decimal) is void { output.writeln("number: ", value.string()); }
    handle(value is string) is void { output.writeln("string: ", value); }
    handle(values is Sequencing<JsonValue>) is void {
        output.writeln("array of ", values.size().string(), " values");
    }
    handle(members is Sequencing<JsonMember>) is void {
        output.writeln("object with ", members.size().string(), " members");
    }
}

let describing = DescribingJson(output: Stdout());
for document.values() -> value {
    value.dispatch(describing);
}
```

## `json/parse`

- [x] object `ParsedJson` does `JsonValue`
  - [x] `ParsedJson(origin is string)`
  - [x] `ParsedJson(origin is string, maximum_depth is int)`
  - [x] `kind() is JsonKind`
  - [x] `dispatch(to is HandlingJsonValue) is void`
- [x] object `InvalidJson` does `Error`
  - [x] `InvalidJson(offset is int, line is int, column is int, message is string)`
  - [x] `offset() is int`
  - [x] `line() is int`
  - [x] `column() is int`
  - [x] `text() is string`
- [x] Reuse `DuplicateJsonMember` from `json/value` for programmatic and parsed duplicates.
- [x] Preserve parser offset, line and column on both parse-error types.
- [x] Parse JSON numbers into `Decimal` without a `float` round trip.
- [x] Reject `NaN`, positive infinity and negative infinity.
- [x] Reject leading zeroes outside the value zero.
- [x] Reject duplicate object member names.
- [x] Reject trailing non-whitespace input.
- [x] Reject unpaired UTF-16 surrogate escapes.
- [x] Count line and column from one and scalar offset from zero.
- [x] Reject a non-positive maximum depth.
- [x] Make the default maximum depth `128` and count the root as depth `1`.
- [x] Parse lazily when `kind()` or `dispatch()` first observes `ParsedJson`.
- [x] Memoize the parsed tree or typed failure.
- [x] Make `ParsedJson` a `JsonValue`; do not hide it behind projected `value()`.
- [x] Keep parsing independent from host JSON and floating-point libraries.

### `json/parse` API examples

Parse a complete document directly as a JSON value:

```okkama
let document is JsonValue = ParsedJson(
    origin: "{\"name\":\"Ada\",\"active\":true}",
);
```

Use an explicit depth policy for untrusted or deeply nested input:

```okkama
let document is JsonValue = ParsedJson(
    origin: request_body,
    maximum_depth: 32,
);
```

The root counts as depth `1`. A non-positive limit is rejected before parsing.
The default constructor uses a finite maximum depth of `128`.

Syntax failures carry a zero-based Unicode-scalar offset and one-based line
and column:

```okkama
let document is JsonValue = ParsedJson(origin: source);
try {
    document.dispatch(handler);
}
catch invalid is InvalidJson {
    Stderr().writeln(
        invalid.text(),
        " (offset ",
        invalid.offset().string(),
        ")",
    );
    throw invalid;
}
```

Construction is intentionally cheap. The error appears at the first
observation through `kind()` or `dispatch()`, where the document is actually
needed; later observations reuse the same parsed tree or typed failure.

Duplicate names use the same error for constructed and parsed objects. Parsed
duplicates additionally expose their source position:

```okkama
try {
    ParsedJson(origin: "{\"name\":1,\"name\":2}").kind();
}
catch duplicate is DuplicateJsonMember {
    Stderr().writeln(
        "duplicate ",
        duplicate.name(),
        " at ",
        duplicate.line().string(),
        ":",
        duplicate.column().string(),
    );
}
```

## `json/write`

- [ ] object `CompactJson`
  - [ ] `CompactJson(origin is JsonValue)`
  - [ ] `string() is string`
- [ ] object `PrettyJson`
  - [ ] `PrettyJson(origin is JsonValue, indentation is int)`
  - [ ] `string() is string`
- [ ] Reject negative indentation.
- [ ] Escape every JSON control character.
- [ ] Preserve Unicode characters unless JSON escaping is required.
- [ ] Preserve object member insertion order.
- [ ] Emit only standard JSON numbers.
- [ ] Do not add `JsonData`; compose JSON text with an explicit Unicode encoding.
- [ ] Do not add magical `json()` actions to unrelated objects.

## `csv`

- [ ] role `CsvDialect`
  - [ ] `delimiter() is char`
  - [ ] `quote() is char`
  - [ ] `line_ending() is string`
  - [ ] `doubled_quotes() is bool`
- [ ] object `CustomCsvDialect` does `CsvDialect`
  - [ ] `CustomCsvDialect(delimiter is char, quote is char, line_ending is string, doubled_quotes is bool)`
  - [ ] `delimiter() is char`
  - [ ] `quote() is char`
  - [ ] `line_ending() is string`
  - [ ] `doubled_quotes() is bool`
- [ ] object `ExcelCsvDialect` does `CsvDialect`
  - [ ] `ExcelCsvDialect()`
  - [ ] `delimiter() is char`
  - [ ] `quote() is char`
  - [ ] `line_ending() is string`
  - [ ] `doubled_quotes() is bool`
- [ ] object `UnixCsvDialect` does `CsvDialect`
  - [ ] `UnixCsvDialect()`
  - [ ] `delimiter() is char`
  - [ ] `quote() is char`
  - [ ] `line_ending() is string`
  - [ ] `doubled_quotes() is bool`
- [ ] object `TabCsvDialect` does `CsvDialect`
  - [ ] `TabCsvDialect()`
  - [ ] `delimiter() is char`
  - [ ] `quote() is char`
  - [ ] `line_ending() is string`
  - [ ] `doubled_quotes() is bool`
- [ ] object `CsvRows` does `Iterating<@string>`
  - [ ] `CsvRows(origin is string, dialect is CsvDialect)`
  - [ ] `cursor() is Cursor<@string>`
- [ ] object `CsvString`
  - [ ] `CsvString(rows is Iterating<@string>, dialect is CsvDialect)`
  - [ ] `string() is string`
- [ ] object `InvalidCsv` does `Error`
  - [ ] `InvalidCsv(line is int, column is int, message is string)`
  - [ ] `line() is int`
  - [ ] `column() is int`
  - [ ] `text() is string`
- [ ] Reject equal delimiter and quote characters.
- [ ] Reject an empty line ending.
- [ ] Preserve empty fields and empty final fields.
- [ ] Preserve embedded line endings inside quoted fields.
- [ ] Quote fields only when their contents require it.
- [ ] Make malformed quoting fail instead of guessing.
- [ ] Keep blocking CSV input out of `Cursor` traversal.
- [ ] Add a resource-based CSV reader only with a non-cursor row-reading contract.

## `toml/value`

- [ ] enum `TomlKind`
  - [ ] `string`
  - [ ] `integer`
  - [ ] `float`
  - [ ] `boolean`
  - [ ] `offset_datetime`
  - [ ] `local_datetime`
  - [ ] `local_date`
  - [ ] `local_time`
  - [ ] `array`
  - [ ] `table`
- [ ] role `TomlValue`
  - [ ] `kind() is TomlKind`
- [ ] object `TomlString` does `TomlValue`
  - [ ] `TomlString(value is string)`
  - [ ] `kind() is TomlKind`
  - [ ] `value() is string`
- [ ] object `TomlInteger` does `TomlValue`
  - [ ] `TomlInteger(value is int)`
  - [ ] `kind() is TomlKind`
  - [ ] `value() is int`
- [ ] object `TomlFloat` does `TomlValue`
  - [ ] `TomlFloat(value is float)`
  - [ ] `kind() is TomlKind`
  - [ ] `value() is float`
- [ ] object `TomlBoolean` does `TomlValue`
  - [ ] `TomlBoolean(value is bool)`
  - [ ] `kind() is TomlKind`
  - [ ] `value() is bool`
- [ ] object `TomlOffsetDateTime` does `TomlValue`
  - [ ] `TomlOffsetDateTime(value is ZonedDateTime)`
  - [ ] `kind() is TomlKind`
  - [ ] `value() is ZonedDateTime`
- [ ] object `TomlLocalDateTime` does `TomlValue`
  - [ ] `TomlLocalDateTime(value is LocalDateTime)`
  - [ ] `kind() is TomlKind`
  - [ ] `value() is LocalDateTime`
- [ ] object `TomlLocalDate` does `TomlValue`
  - [ ] `TomlLocalDate(value is Date)`
  - [ ] `kind() is TomlKind`
  - [ ] `value() is Date`
- [ ] object `TomlLocalTime` does `TomlValue`
  - [ ] `TomlLocalTime(value is TimeOfDay)`
  - [ ] `kind() is TomlKind`
  - [ ] `value() is TimeOfDay`
- [ ] object `TomlArray` does `TomlValue`
  - [ ] `TomlArray(values is Sequencing<TomlValue>)`
  - [ ] `kind() is TomlKind`
  - [ ] `values() is Sequencing<TomlValue>`
- [ ] object `TomlTable` does `TomlValue`
  - [ ] `TomlTable(members is %string:TomlValue)`
  - [ ] `kind() is TomlKind`
  - [ ] `membership(name is string) is bool`
  - [ ] `value(name is string) is TomlValue`
  - [ ] `members() is %string:TomlValue`
- [ ] object `MissingTomlMember` does `Error`
  - [ ] `MissingTomlMember(name is string)`
  - [ ] `name() is string`
  - [ ] `text() is string`
- [ ] Keep the TOML model independent from `JsonValue`.
- [ ] Preserve source key order in every table.
- [ ] Do not make arrays or tables implement sequence/map contracts directly.

## `toml/parse`

- [ ] object `ParsedToml`
  - [ ] `ParsedToml(origin is string)`
  - [ ] `#projected value() is TomlTable`
- [ ] object `InvalidToml` does `Error`
  - [ ] `InvalidToml(offset is int, line is int, column is int, message is string)`
  - [ ] `offset() is int`
  - [ ] `line() is int`
  - [ ] `column() is int`
  - [ ] `text() is string`
- [ ] Implement TOML 1.0 grammar.
- [ ] Reject duplicate keys.
- [ ] Reject conflicting dotted keys and tables.
- [ ] Reject redefining a table.
- [ ] Preserve arrays of tables as arrays containing `TomlTable` values.
- [ ] Preserve TOML integer precision within the runtime `int` range.
- [ ] Reject integers outside the runtime `int` range.
- [ ] Parse offset date-times with an explicit fixed offset.
- [ ] Do not add TOML writing in the first implementation.

## `config/ini`

- [ ] object `IniDocument`
  - [ ] `IniDocument(sections is Sequencing<IniSection>)`
  - [ ] `sections() is Sequencing<IniSection>`
  - [ ] `section(name is string) is IniSection`
- [ ] object `IniSection`
  - [ ] `IniSection(name is string, entries is %string:string)`
  - [ ] `name() is string`
  - [ ] `membership(name is string) is bool`
  - [ ] `value(name is string) is string`
  - [ ] `entries() is %string:string`
- [ ] object `ParsedIni`
  - [ ] `ParsedIni(origin is string)`
  - [ ] `#projected value() is IniDocument`
- [ ] object `IniString`
  - [ ] `IniString(origin is IniDocument)`
  - [ ] `string() is string`
- [ ] object `MissingIniSection` does `Error`
  - [ ] `MissingIniSection(name is string)`
  - [ ] `name() is string`
  - [ ] `text() is string`
- [ ] object `MissingIniEntry` does `Error`
  - [ ] `MissingIniEntry(section is string, name is string)`
  - [ ] `section() is string`
  - [ ] `name() is string`
  - [ ] `text() is string`
- [ ] object `InvalidIni` does `Error`
  - [ ] `InvalidIni(line is int, column is int, message is string)`
  - [ ] `line() is int`
  - [ ] `column() is int`
  - [ ] `text() is string`
- [ ] Preserve section and entry insertion order.
- [ ] Treat section and entry names as case-sensitive.
- [ ] Reject duplicate sections and duplicate entries.
- [ ] Do not add implicit interpolation.
- [ ] Do not add an ambient `DEFAULT` section.
- [ ] Document the accepted INI grammar as Okkama's dialect.

## `xml/name`

- [ ] object `XmlName`
  - [ ] `XmlName(namespace is string, local is string, prefix is string)`
  - [ ] `namespace() is string`
  - [ ] `local_name() is string`
  - [ ] `prefix() is string`
- [ ] Reject an empty local name.
- [ ] Treat prefix as serialization metadata, not namespace identity.

## `xml/node`

- [ ] role `XmlNode`
  - [ ] `xml() is string`
- [ ] object `XmlText` does `XmlNode`
  - [ ] `XmlText(value is string)`
  - [ ] `value() is string`
  - [ ] `xml() is string`
- [ ] object `XmlComment` does `XmlNode`
  - [ ] `XmlComment(value is string)`
  - [ ] `value() is string`
  - [ ] `xml() is string`
- [ ] object `XmlProcessingInstruction` does `XmlNode`
  - [ ] `XmlProcessingInstruction(target is string, data is string)`
  - [ ] `target() is string`
  - [ ] `data() is string`
  - [ ] `xml() is string`
- [ ] object `XmlElement` does `XmlNode`
  - [ ] `XmlElement(name is XmlName, attributes is %XmlName:string, children is Sequencing<XmlNode>)`
  - [ ] `name() is XmlName`
  - [ ] `attributes() is %XmlName:string`
  - [ ] `children() is Sequencing<XmlNode>`
  - [ ] `xml() is string`
- [ ] object `XmlDocument`
  - [ ] `XmlDocument(root is XmlElement)`
  - [ ] `root() is XmlElement`
  - [ ] `xml() is string`
- [ ] Preserve text as child nodes in mixed content.
- [ ] Preserve comment and processing-instruction positions.
- [ ] Keep namespace URI separate from lexical prefix.
- [ ] Reject duplicate expanded attribute names.
- [ ] Do not add lossy `inner_text()`.

## `xml/parse`

- [ ] object `ParsedXml`
  - [ ] `ParsedXml(origin is string)`
  - [ ] `#projected value() is XmlDocument`
- [ ] object `InvalidXml` does `Error`
  - [ ] `InvalidXml(offset is int, line is int, column is int, message is string)`
  - [ ] `offset() is int`
  - [ ] `line() is int`
  - [ ] `column() is int`
  - [ ] `text() is string`
- [ ] Parse XML 1.0 documents.
- [ ] Reject multiple root elements.
- [ ] Reject undeclared namespace prefixes.
- [ ] Reject malformed entity references.
- [ ] Reject external entities.
- [ ] Reject external DTD loading.
- [ ] Never read files or network resources while parsing.
- [ ] Add entity-expansion limits for internal entities.

## `xml/write`

- [ ] Serialize every `XmlNode` through `xml()`.
- [ ] Escape text content independently from attribute content.
- [ ] Reject `--` inside XML comments.
- [ ] Reject invalid processing-instruction targets and data.
- [ ] Emit namespace declarations deterministically.
- [ ] Preserve child order exactly.
- [ ] Do not add automatic pretty-printing that changes mixed content.

## `html/entities`

- [ ] object `EscapedHtml`
  - [ ] `EscapedHtml(origin is string)`
  - [ ] `EscapedHtml(origin is string, quotes is bool)`
  - [ ] `string() is string`
- [ ] object `UnescapedHtml`
  - [ ] `UnescapedHtml(origin is string)`
  - [ ] `string() is string`
- [ ] Escape ampersand, less-than and greater-than characters.
- [ ] Escape quotation marks only when requested.
- [ ] Decode the standard HTML named-character reference set.
- [ ] Follow HTML recovery rules for malformed character references.
- [ ] Move event-based HTML parsing to Stage 14.
- [ ] Do not model HTML as lenient XML.
- [ ] Do not add a browser-style HTML DOM.

## `plist/value`

- [ ] enum `PlistKind`
  - [ ] `boolean`
  - [ ] `integer`
  - [ ] `real`
  - [ ] `string`
  - [ ] `data`
  - [ ] `date`
  - [ ] `array`
  - [ ] `dictionary`
  - [ ] `uid`
- [ ] role `PlistValue`
  - [ ] `kind() is PlistKind`
- [ ] object `PlistBoolean` does `PlistValue`
  - [ ] `PlistBoolean(value is bool)`
  - [ ] `kind() is PlistKind`
  - [ ] `value() is bool`
- [ ] object `PlistInteger` does `PlistValue`
  - [ ] `PlistInteger(value is BigInt)`
  - [ ] `kind() is PlistKind`
  - [ ] `value() is BigInt`
- [ ] object `PlistReal` does `PlistValue`
  - [ ] `PlistReal(value is float)`
  - [ ] `kind() is PlistKind`
  - [ ] `value() is float`
- [ ] object `PlistString` does `PlistValue`
  - [ ] `PlistString(value is string)`
  - [ ] `kind() is PlistKind`
  - [ ] `value() is string`
- [ ] object `PlistData` does `PlistValue`
  - [ ] `PlistData(value is @byte)`
  - [ ] `kind() is PlistKind`
  - [ ] `value() is @byte`
- [ ] object `PlistDate` does `PlistValue`
  - [ ] `PlistDate(value is Instant)`
  - [ ] `kind() is PlistKind`
  - [ ] `value() is Instant`
- [ ] object `PlistArray` does `PlistValue`
  - [ ] `PlistArray(values is Sequencing<PlistValue>)`
  - [ ] `kind() is PlistKind`
  - [ ] `values() is Sequencing<PlistValue>`
- [ ] object `PlistDictionary` does `PlistValue`
  - [ ] `PlistDictionary(members is %string:PlistValue)`
  - [ ] `kind() is PlistKind`
  - [ ] `membership(name is string) is bool`
  - [ ] `value(name is string) is PlistValue`
  - [ ] `members() is %string:PlistValue`
- [ ] object `PlistUid` does `PlistValue`
  - [ ] `PlistUid(value is BigInt)`
  - [ ] `kind() is PlistKind`
  - [ ] `value() is BigInt`
- [ ] Keep the plist model independent from `JsonValue`.
- [ ] Preserve dictionary insertion order.

## `plist/parse`

- [ ] object `ParsedPlist`
  - [ ] `ParsedPlist(origin is @byte)`
  - [ ] `#projected value() is PlistValue`
- [ ] object `InvalidPlist` does `Error`
  - [ ] `InvalidPlist(offset is int, message is string)`
  - [ ] `offset() is int`
  - [ ] `text() is string`
- [ ] Detect XML and binary plist formats from the input.
- [ ] Parse XML plist without enabling external XML entities.
- [ ] Validate binary plist offsets before reading data.
- [ ] Reject cyclic or self-referential object graphs.
- [ ] Implement plist after the other Stage 13 formats.

## `plist/write`

- [ ] enum `PlistFormat`
  - [ ] `xml`
  - [ ] `binary`
- [ ] object `EncodedPlist`
  - [ ] `EncodedPlist(origin is PlistValue, format is PlistFormat)`
  - [ ] `bytes() is @byte`
- [ ] Write deterministic dictionary ordering.
- [ ] Normalize plist dates to UTC instants.
- [ ] Reject values unsupported by the selected plist format.

## Stage 13 exclusions

- [ ] Do not add YAML to the standard library.
- [ ] Do not add executable object deserialization such as Pickle.
- [ ] Do not expose a runtime-internal Marshal format as a stable API.
- [ ] Do not automatically serialize arbitrary objects.
- [ ] Do not add universal `Serializing<T>` or `Deserializing<T>` roles.

---

# Stage 14 — regex and lexical text

## `regex/pattern`

- [x] enum `RegexFlag`
  - [x] `ascii`
  - [x] `ignore_case`
  - [x] `multiline`
  - [x] `dot_all`
  - [x] `verbose`
- [x] enum `RegexLimit`
  - [x] `expression_size`
  - [x] `nesting`
  - [x] `groups`
  - [x] `states`
- [x] object `Regex`
  - [x] `Regex(expression is string)`
  - [x] `Regex(expression is string, flags is @RegexFlag)`
  - [x] `expression() is string`
  - [x] `flags() is @RegexFlag`
  - [x] `group_count() is int`
  - [x] `group_names() is %string:int`
  - [x] `state() is RegexState` as an opaque inter-module capability
- [x] object `InvalidRegex` does `Error`
  - [x] `InvalidRegex(offset is int, message is string)`
  - [x] `offset() is int`
  - [x] `text() is string`
- [x] object `RegexLimitExceeded` does `Error`
  - [x] `RegexLimitExceeded(kind is RegexLimit, limit is int)`
  - [x] `kind() is RegexLimit`
  - [x] `limit() is int`
  - [x] `text() is string`
- [x] Bind expression text and flags during construction without compiling.
- [x] Compile lazily on the first metadata or matching action.
- [x] Memoize the compiled expression or its typed failure in one opaque state.
- [x] Treat duplicate flags as idempotent while preserving the supplied flag array.
- [x] Count captures from one and exclude the complete-match group zero.
- [x] Exclude non-capturing groups from capture numbering.
- [x] Preserve named-group declaration numbers in `group_names()`.
- [x] Reject duplicate named groups.
- [x] Reject backreferences, lookaround, conditionals and atomic host extensions.
- [x] Reject executable regex constructs.
- [x] Reject recursive and subroutine-call constructs.
- [x] Count error offsets in Unicode scalar values from zero.
- [x] Keep compilation and matching independent from the process locale.
- [x] Pin the public syntax as **Okkama Regex Dialect 1**.
- [x] Pin Unicode-property and case-fold behavior to Unicode **16.0.0**.
- [x] Make `RegexFlag.ascii` restrict `\d`, `\w`, `\s` and case folding to ASCII.
- [x] Limit expression length to 4,096 Unicode scalar values.
- [x] Limit syntactic group nesting to 256.
- [x] Limit capturing groups to 1,024.
- [x] Limit compiled Thompson-NFA states to 16,384.
- [x] Limit a bounded repetition endpoint to 4,096.
- [x] Limit matching input to 1,048,576 Unicode scalar values.

Construction is state-only from the caller's perspective. Reading retained
syntax does not compile; metadata actions trigger the shared lazy state:

```okkama
let pattern = Regex(
    expression: "^(?<name>[a-z]+): (?<amount>\\d+)$",
    flags: [RegexFlag.ignore_case, RegexFlag.multiline],
);

Stdout().writeln(pattern.expression());
Stdout().writeln("groups: ", pattern.group_count().string());
Stdout().writeln(
    "amount group: ",
    pattern.group_names()["amount"].string(),
);
```

`group_count()` does not include the implicit complete match. Named and
unnamed captures share the same one-based declaration numbering;
non-capturing `(?:...)` groups consume no number. `group_names()` contains only
named captures.

Dialect 1 is a regular-language dialect implemented through a bounded
Thompson NFA. It never forwards expression text to Perl, PCRE, JavaScript or
another host regex compiler. Executable constructs, recursion, subroutine
calls, backreferences and lookaround are syntax errors rather than
provider-dependent extensions.

The pattern layer owns syntax, flags, lazy compilation and capture metadata.
The following match/search modules define and test how each flag affects
matching. All of them consume the same memoized compile state.

## `regex/range`

- [x] object `RegexRange`
  - [x] `RegexRange(start is int, end is int)`
  - [x] `start() is int`
  - [x] `end() is int`
  - [x] `length() is int`
- [x] object `InvalidRegexRange` does `Error`
  - [x] `InvalidRegexRange(start is int, end is int)`
  - [x] `start() is int`
  - [x] `end() is int`
  - [x] `text() is string`
- [x] Use half-open intervals `[start, end)`.
- [x] Allow empty ranges with equal boundaries.
- [x] Reject a negative start.
- [x] Reject an end smaller than start.
- [x] Count positions in Unicode scalar values rather than encoded bytes.
- [x] Keep `RegexRange` local to regex instead of adding a global `Span`.

`RegexRange` is the coordinate value returned by regex matches and captures:

```okkama
import "regex/range" take RegexRange;

let word = RegexRange(start: 7, end: 12);
word.start();  // 7
word.end();    // 12
word.length(); // 5
```

The end is excluded. Empty matches therefore have a natural representation:

```okkama
let boundary = RegexRange(start: 4, end: 4);
boundary.length(); // 0
```

Offsets count Unicode scalar values. In `"a😀я"`, `[1, 3)` covers `"😀я"`
and has length `2`, regardless of how many bytes encode those characters.
An unmatched optional capture does not manufacture a range; its future
`RegexCapture.range()` action throws `UnmatchedRegexCapture` instead.

## `regex/match`

- [x] role `RegexMatching`
  - [x] `text() is string`
  - [x] `range() is RegexRange`
  - [x] `capture(index is int) is RegexCapture`
  - [x] `group(name is string) is RegexCapture`
  - [x] `captures() is @RegexCapture`
- [x] object `RegexCapture`
  - [x] Keep construction unusable without an opaque runtime match state.
  - [x] `index() is int`
  - [x] `name() is string`
  - [x] `matched() is bool`
  - [x] `text() is string`
  - [x] `range() is RegexRange`
- [x] object `RegexMatch` does `RegexMatching`
  - [x] Keep construction unusable without an opaque runtime match state.
  - [x] Implement all five `RegexMatching` actions.
- [x] object `MissingRegexCapture` does `Error`
  - [x] `MissingRegexCapture(index is int)`
  - [x] `index() is int`
  - [x] `text() is string`
- [x] object `MissingRegexGroup` does `Error`
  - [x] `MissingRegexGroup(name is string)`
  - [x] `name() is string`
  - [x] `text() is string`
- [x] object `UnmatchedRegexCapture` does `Error`
  - [x] `UnmatchedRegexCapture(index is int)`
  - [x] `index() is int`
  - [x] `text() is string`
- [x] Treat capture zero as the complete match.
- [x] Exclude capture zero from `captures()`.
- [x] Return an empty name for an unnamed capture.
- [x] Make `matched()` false when an existing group did not participate.
- [x] Make `text()` and `range()` throw `UnmatchedRegexCapture`.
- [x] Preserve capture declaration order.
- [x] Retain the final participating capture for a quantified group.

```okkama
let found = FirstRegexMatch(
    "https://example.org",
    Regex("(?<scheme>https?)://([^/]+)"),
);

found.text();                   // "https://example.org"
found.capture(0).text();        // complete match
found.group("scheme").text();  // "https"
found.capture(2).text();        // "example.org"
found.captures().size();        // 2; capture zero is excluded
```

An optional group remains addressable when it did not participate. Its
`matched()` fact is false; asking it for text or a range is a typed error.

## `regex/search`

- [x] object `FirstRegexMatch` does `RegexMatching`
  - [x] `FirstRegexMatch(origin is string, regex is Regex)`
  - [x] `FirstRegexMatch(origin is string, regex is Regex, range is RegexRange)`
  - [x] Implement all five `RegexMatching` actions directly; no `value()`.
- [x] object `PrefixRegexMatch` does `RegexMatching`
  - [x] `PrefixRegexMatch(origin is string, regex is Regex)`
  - [x] `PrefixRegexMatch(origin is string, regex is Regex, range is RegexRange)`
  - [x] Implement all five `RegexMatching` actions directly; no `value()`.
- [x] object `EntireRegexMatch` does `RegexMatching`
  - [x] `EntireRegexMatch(origin is string, regex is Regex)`
  - [x] `EntireRegexMatch(origin is string, regex is Regex, range is RegexRange)`
  - [x] Implement all five `RegexMatching` actions directly; no `value()`.
- [x] object `RegexMatches` does `Iterating<RegexMatching>`
  - [x] `RegexMatches(origin is string, regex is Regex)`
  - [x] `RegexMatches(origin is string, regex is Regex, range is RegexRange)`
  - [x] `cursor() is Cursor<RegexMatching>`
  - [x] `found() is bool`
- [x] object `RegexRangeOutsideInput` does `Error`
  - [x] `RegexRangeOutsideInput(range is RegexRange, length is int)`
  - [x] `range() is RegexRange`
  - [x] `length() is int`
  - [x] `text() is string`
- [x] object `NoRegexMatch` does `Error`
  - [x] `text() is string`
- [x] Remove the projected boolean bag `RegexFound`.
- [x] Use `RegexMatches(...).found()` for non-throwing presence checks.
- [x] Use strict half-open `RegexRange` selections.
- [x] Make search offsets absolute Unicode scalar positions in the input.
- [x] Make `FirstRegexMatch` search anywhere in the selected range.
- [x] Make `PrefixRegexMatch` require a match at the selected start.
- [x] Make `EntireRegexMatch` consume the complete selected range.
- [x] Make `RegexMatches` produce non-overlapping matches from left to right.
- [x] Advance by one Unicode scalar value after an empty match.
- [x] Memoize each mandatory query's match or absence lazily.
- [x] Throw `NoRegexMatch` only when a mandatory result is observed.

```okkama
let pattern = Regex("\\d+");

FirstRegexMatch("one 22 three", pattern).text(); // "22"
PrefixRegexMatch("22 three", pattern).text();   // "22"
EntireRegexMatch("22", pattern).text();         // "22"

if RegexMatches("one 22", pattern).found() {
    for RegexMatches("one 22 three 333", pattern) -> found {
        Stdout().writeln(found.text());
    }
}
```

Selected searches reuse `RegexRange` rather than repeating loose `start/end`
integers:

```okkama
let found = FirstRegexMatch(
    document,
    pattern,
    RegexRange(start: 100, end: 500),
);
```

## `regex/replace`

- [x] role `ReplacingRegexMatch`
  - [x] `replacement(found is RegexMatching) is string`
- [x] object `LiteralRegexReplacement` does `ReplacingRegexMatch`
  - [x] `LiteralRegexReplacement(value is string)`
  - [x] `replacement(found is RegexMatching) is string`
- [x] object `ExpandedRegexReplacement` does `ReplacingRegexMatch`
  - [x] `ExpandedRegexReplacement(template is string)`
  - [x] `replacement(found is RegexMatching) is string`
- [x] object `RegexSubstitution`
  - [x] `RegexSubstitution(origin is string, regex is Regex, replacing is ReplacingRegexMatch)`
  - [x] `RegexSubstitution(origin is string, regex is Regex, replacing is ReplacingRegexMatch, limit is int)`
  - [x] `string() is string`
  - [x] `amount() is int`
- [x] object `InvalidRegexReplacement` does `Error`
  - [x] `InvalidRegexReplacement(offset is int, message is string)`
  - [x] `offset() is int`
  - [x] `text() is string`
- [x] object `InvalidRegexReplacementLimit` does `Error`
  - [x] `InvalidRegexReplacementLimit(limit is int)`
  - [x] `limit() is int`
  - [x] `text() is string`
- [x] Make literal replacement ignore capture syntax.
- [x] Support `$0`, numeric `$1`, named `${name}` and literal `$$`.
- [x] Expand an unmatched existing capture to an empty string.
- [x] Validate and memoize expanded syntax lazily on first use.
- [x] Make the constructor without a limit replace every match.
- [x] Reject a non-positive explicit replacement limit.
- [x] Count only replacements actually performed.
- [x] Advance safely after zero-width substitutions.
- [x] Memoize the shared substitution result for `string()` and `amount()`.

```okkama
let changed = RegexSubstitution(
    "Ada Lovelace; Alan Turing",
    Regex("(?<first>\\w+) (?<last>\\w+)"),
    ExpandedRegexReplacement("${last}, ${first}"),
);

changed.string(); // "Lovelace, Ada; Turing, Alan"
changed.amount(); // 2
```

`LiteralRegexReplacement("$1")` writes the two literal characters `$1`.
Application-specific policy implements the single-action `ReplacingRegexMatch`
role and receives the ordinary `RegexMatching` interface.

## `regex/split`

- [x] object `RegexParts` does `Iterating<string>`
  - [x] `RegexParts(origin is string, regex is Regex)`
  - [x] `RegexParts(origin is string, regex is Regex, limit is int)`
  - [x] `cursor() is Cursor<string>`
- [x] object `InvalidRegexSplitLimit` does `Error`
  - [x] `InvalidRegexSplitLimit(limit is int)`
  - [x] `limit() is int`
  - [x] `text() is string`
- [x] Remove the `RegexSplit.values()` value bag.
- [x] Traverse fields lazily through `RegexParts` itself.
- [x] Preserve empty leading, adjacent and trailing fields.
- [x] Do not inject captured groups into the result.
- [x] Make the constructor without a limit split at every match.
- [x] Define an explicit limit as the maximum number of separators consumed.
- [x] Reject a non-positive explicit split limit.
- [x] Advance by one Unicode scalar value after an empty match.

```okkama
let fields = RegexParts(
    "one, two;three",
    Regex("[,;]\\s*"),
);

for fields -> field {
    Stdout().writeln(field);
}
```

An explicit limit of `1` produces at most two fields. Splitting `",a,"` on
`,` produces `""`, `"a"`, `""`; no edge field is silently discarded.

## `regex/escape`

- [x] object `EscapedRegex`
  - [x] `EscapedRegex(origin is string)`
  - [x] `#projected string() is string`
- [x] Escape every character with syntactic meaning in the pinned dialect.
- [x] Keep escaped whitespace and `#` literal under `RegexFlag.verbose`.
- [x] Verify the escaped expression matches the original string literally.
- [x] Preserve ordinary Unicode characters.

```okkama
let literal = "price: $5.00";
let pattern = Regex(string EscapedRegex(literal));
EntireRegexMatch(literal, pattern).text(); // "price: $5.00"
```

## `html/events`

- [x] enum `HtmlEventKind`
  - [x] `start_tag`
  - [x] `end_tag`
  - [x] `text`
  - [x] `comment`
  - [x] `declaration`
  - [x] `processing_instruction`
- [x] role `HtmlEvent`
  - [x] `kind() is HtmlEventKind`
  - [x] `dispatch(to is HandlingHtmlEvent) is void`
- [x] role `StartingHtmlTag`
  - [x] `name() is string`
  - [x] `attributes() is Sequencing<HtmlAttribute>`
  - [x] `self_closing() is bool`
- [x] role `EndingHtmlTag`
  - [x] `name() is string`
- [x] role `ContainingHtmlText`
  - [x] `value() is string`
- [x] role `ContainingHtmlComment`
  - [x] `value() is string`
- [x] role `DeclaringHtml`
  - [x] `value() is string`
- [x] role `ProcessingHtmlInstruction`
  - [x] `target() is string`
  - [x] `data() is string`
- [x] role `HandlingHtmlEvent`
  - [x] `handle(event is StartingHtmlTag) is void`
  - [x] `handle(event is EndingHtmlTag) is void`
  - [x] `handle(event is ContainingHtmlText) is void`
  - [x] `handle(event is ContainingHtmlComment) is void`
  - [x] `handle(event is DeclaringHtml) is void`
  - [x] `handle(event is ProcessingHtmlInstruction) is void`
  - [x] Count the six overloads as one public action name: `handle`.
- [x] object `HtmlAttribute`
  - [x] `HtmlAttribute(name is string, valued is bool, value is string)`
  - [x] `name() is string`
  - [x] `valued() is bool`
  - [x] `value() is string`
- [x] object `HtmlStartTag` does `HtmlEvent, StartingHtmlTag`
  - [x] `HtmlStartTag(name is string, attributes is Sequencing<HtmlAttribute>, self_closing is bool)`
  - [x] `kind() is HtmlEventKind`
  - [x] `name() is string`
  - [x] `attributes() is Sequencing<HtmlAttribute>`
  - [x] `self_closing() is bool`
  - [x] `dispatch(to is HandlingHtmlEvent) is void`
- [x] object `HtmlEndTag` does `HtmlEvent, EndingHtmlTag`
  - [x] `HtmlEndTag(name is string)`
  - [x] `kind() is HtmlEventKind`
  - [x] `name() is string`
  - [x] `dispatch(to is HandlingHtmlEvent) is void`
- [x] object `HtmlText` does `HtmlEvent, ContainingHtmlText`
  - [x] `HtmlText(value is string)`
  - [x] `kind() is HtmlEventKind`
  - [x] `value() is string`
  - [x] `dispatch(to is HandlingHtmlEvent) is void`
- [x] object `HtmlComment` does `HtmlEvent, ContainingHtmlComment`
  - [x] `HtmlComment(value is string)`
  - [x] `kind() is HtmlEventKind`
  - [x] `value() is string`
  - [x] `dispatch(to is HandlingHtmlEvent) is void`
- [x] object `HtmlDeclaration` does `HtmlEvent, DeclaringHtml`
  - [x] `HtmlDeclaration(value is string)`
  - [x] `kind() is HtmlEventKind`
  - [x] `value() is string`
  - [x] `dispatch(to is HandlingHtmlEvent) is void`
- [x] object `HtmlProcessingInstruction` does `HtmlEvent, ProcessingHtmlInstruction`
  - [x] `HtmlProcessingInstruction(target is string, data is string)`
  - [x] `kind() is HtmlEventKind`
  - [x] `target() is string`
  - [x] `data() is string`
  - [x] `dispatch(to is HandlingHtmlEvent) is void`
- [x] object `HtmlEvents` does `Iterating<HtmlEvent>`
  - [x] `HtmlEvents(origin is string)`
  - [x] `cursor() is Cursor<HtmlEvent>`
- [x] Preserve event and attribute order.
- [x] Preserve boolean attributes through `valued() == false`.
- [x] Require an empty value when `valued() == false`.
- [x] Decode named and numeric character references in text and attributes.
- [x] Apply raw-text rules inside `script` and `style` elements.
- [x] Recover from malformed markup without inventing a DOM tree.
- [x] Never read external resources.
- [x] Do not expose blocking input through `HtmlEvents` traversal.
- [x] Do not promise browser-equivalent tree construction.
- [x] Use typed double dispatch because Okkama deliberately has no general
  downcast from `HtmlEvent` to a concrete event object.

### `html/events` API examples

`HtmlEvents` tokenizes one already available string. Construction and traversal
never read a file, socket, URL, DTD or other external resource:

```okkama
let document = "<!doctype html><p class=lead>Hello &amp; welcome</p>";

for HtmlEvents(origin: document) -> event {
    Stdout().writeln(event.kind().string());
}
```

That loop can inspect the broad event kind, but `HtmlEvent` intentionally does
not expose a union of every possible payload action. Okkama has no general
downcast, so payload processing uses `dispatch()` and one overloaded handler
action.

The handler parameters are small nominal payload roles rather than concrete
storage objects. This keeps the declaration graph acyclic, gives each overload
only the actions that make sense for that event shape, and lets implementations
of events change without breaking handlers.

The handler below prints every event through its precise payload role:

```okkama
object PrintingHtmlEvents does HandlingHtmlEvent {
    has output is WritingText;

    PrintingHtmlEvents(:output);

    handle(event is StartingHtmlTag) is void {
        output.write("start <", event.name(), ">");
        for event.attributes() -> attribute {
            output.write(" ", attribute.name());
            if attribute.valued() {
                output.write("=\"", attribute.value(), "\"");
            }
        }
        if event.self_closing() {
            output.write(" /");
        }
        output.writeln();
    }

    handle(event is EndingHtmlTag) is void {
        output.writeln("end </", event.name(), ">");
    }

    handle(event is ContainingHtmlText) is void {
        output.writeln("text: ", event.value());
    }

    handle(event is ContainingHtmlComment) is void {
        output.writeln("comment: ", event.value());
    }

    handle(event is DeclaringHtml) is void {
        output.writeln("declaration: ", event.value());
    }

    handle(event is ProcessingHtmlInstruction) is void {
        output.writeln(
            "instruction: ",
            event.target(),
            " ",
            event.data(),
        );
    }
}

let printing = PrintingHtmlEvents(output: Stdout());
for HtmlEvents(origin: document) -> event {
    event.dispatch(printing);
}
```

The six overloads are one public action name, `handle`. Every concrete event
calls the overload accepting the one payload role it implements:

```okkama
dispatch(to is HandlingHtmlEvent) is void {
    to.handle(self);
}
```

This keeps `HtmlStartTag` within the five-name limit: `kind`, `name`,
`attributes`, `self_closing` and `dispatch`.

Boolean and explicitly empty attributes remain distinguishable:

```okkama
let source = "<input disabled value=\"\">";

// The start-tag handler observes:
// disabled: valued() == false, value() == ""
// value:    valued() == true,  value() == ""
```

`value()` is always safe to read. An unvalued attribute must contain the empty
string; `valued()` preserves the semantic distinction.

A handler does not need special logic for irrelevant events. It still implements
the complete role, but all overloads share one small and searchable name. This
example emits only links:

```okkama
object WritingHtmlLinks does HandlingHtmlEvent {
    has output is WritingText;

    WritingHtmlLinks(:output);

    handle(event is StartingHtmlTag) is void {
        if event.name() == "a" {
            for event.attributes() -> attribute {
                if attribute.name() == "href" && attribute.valued() {
                    output.writeln(attribute.value());
                }
            }
        }
    }

    handle(event is EndingHtmlTag) is void {}
    handle(event is ContainingHtmlText) is void {}
    handle(event is ContainingHtmlComment) is void {}
    handle(event is DeclaringHtml) is void {}
    handle(event is ProcessingHtmlInstruction) is void {}
}

let links = WritingHtmlLinks(output: Stdout());
for HtmlEvents(origin: page) -> event {
    event.dispatch(links);
}
```

Character references are decoded before an event reaches its handler:

```okkama
let source = "<p title=\"Tom &amp; Jerry\">A&#x20;B&#32;C</p>";

// title attribute value: "Tom & Jerry"
// HtmlText value:        "A B C"
```

Raw-text elements are different. Their contents remain text until the matching
end tag and character references are not decoded there:

```okkama
let source = "<script>if (a < b) x = '&amp;';</script><p>&amp;</p>";

// HtmlText inside script: "if (a < b) x = '&amp;'"
// HtmlText inside p:      "&"
```

Malformed markup produces a deterministic best-effort event stream rather than
an `InvalidHtml` tree error:

```okkama
let source = "before < broken <!-- unfinished";

for HtmlEvents(origin: source) -> event {
    event.dispatch(printing);
}
```

Recovery never implies browser DOM construction. In particular, the tokenizer
does not insert missing `html`, `head`, `body`, table or formatting elements,
does not repair nesting, and does not execute scripts.

Because the complete origin is already a `string`, traversal never blocks:

```okkama
let events = HtmlEvents(origin: already_loaded_text);
let cursor = events.cursor();
```

Reading a large file or network response remains a separate explicit I/O step.
Do not pass a blocking `ReadingText` object to `HtmlEvents`; no such constructor
exists.

## Stage 14 exclusions

- [ ] Do not add a generic `TextTokens` object.
- [ ] Keep shell word parsing in Stage 24.
- [ ] Keep Okkama source tokens in Stage 32.
- [x] Do not add a global `Span` type for regex ranges.
- [x] Do not expose runtime-specific regex extensions.
- [x] Do not support regex code execution.
- [ ] Do not model HTML as XML.
- [ ] Do not add a browser-style HTML DOM.

---

# Stage 15 — checksums, digests and HMAC

## `checksum/state`

- [ ] role `Checksumming`
  - [ ] `state(origin is @byte) is Checksumming`
  - [ ] `value() is int`
- [ ] Make every checksum state immutable.
- [ ] Make `state()` leave its origin unchanged.
- [ ] Represent every checksum result as a non-negative `int`.
- [ ] Do not describe checksums as cryptographic digests.

## `checksum/adler32`

- [ ] object `Adler32` does `Checksumming`
  - [ ] `Adler32()`
  - [ ] `Adler32(origin is @byte)`
  - [ ] `Adler32(origin is @byte, initial is int)`
  - [ ] `state(origin is @byte) is Checksumming`
  - [ ] `value() is int`
- [ ] Use initial value `1` in constructors without an explicit initial value.
- [ ] Reject initial values outside the unsigned 32-bit range.
- [ ] Match the Adler-32 result defined by zlib.
- [ ] Add the empty-input reference vector.
- [ ] Add the `Wikipedia` reference vector.
- [ ] Verify chunked and one-shot results are identical.

## `checksum/crc`

- [ ] object `Crc32` does `Checksumming`
  - [ ] `Crc32()`
  - [ ] `Crc32(origin is @byte)`
  - [ ] `Crc32(origin is @byte, initial is int)`
  - [ ] `state(origin is @byte) is Checksumming`
  - [ ] `value() is int`
- [ ] object `Crc16Ccitt` does `Checksumming`
  - [ ] `Crc16Ccitt(initial is int)`
  - [ ] `Crc16Ccitt(origin is @byte, initial is int)`
  - [ ] `state(origin is @byte) is Checksumming`
  - [ ] `value() is int`
- [ ] Define `Crc32` as CRC-32/ISO-HDLC.
- [ ] Use initial value `0` in `Crc32` constructors without an explicit value.
- [ ] Reject `Crc32` initial values outside the unsigned 32-bit range.
- [ ] Define `Crc16Ccitt` with polynomial `0x1021`.
- [ ] Reject `Crc16Ccitt` initial values outside the unsigned 16-bit range.
- [ ] Add the `123456789` reference vector for each CRC variant.
- [ ] Verify chunked and one-shot results are identical.
- [ ] Do not add an ambiguous `Crc64` object.

## `digest/state`

- [ ] role `Digesting`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] Make every digest state immutable.
- [ ] Make `state()` leave its origin unchanged.
- [ ] Let the empty constructor bind an empty byte source and compute lazily from `bytes()`.
- [ ] Return raw digest bytes from `bytes()`.
- [ ] Compose hexadecimal output through `HexEncoded`.
- [ ] Require explicit Unicode encoding before digesting text.

## `digest/legacy`

- [ ] object `Md5` does `Digesting`
  - [ ] `Md5()`
  - [ ] `Md5(origin is @byte)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] object `Sha1` does `Digesting`
  - [ ] `Sha1()`
  - [ ] `Sha1(origin is @byte)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] Add official empty-input vectors for MD5 and SHA-1.
- [ ] Add official `abc` vectors for MD5 and SHA-1.
- [ ] Document MD5 as legacy interoperability only.
- [ ] Document SHA-1 as legacy interoperability only.
- [ ] Do not use MD5 or SHA-1 for new security decisions.

## `digest/sha2`

- [ ] object `Sha224` does `Digesting`
  - [ ] `Sha224()`
  - [ ] `Sha224(origin is @byte)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] object `Sha256` does `Digesting`
  - [ ] `Sha256()`
  - [ ] `Sha256(origin is @byte)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] object `Sha384` does `Digesting`
  - [ ] `Sha384()`
  - [ ] `Sha384(origin is @byte)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] object `Sha512` does `Digesting`
  - [ ] `Sha512()`
  - [ ] `Sha512(origin is @byte)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] object `Sha512_224` does `Digesting`
  - [ ] `Sha512_224()`
  - [ ] `Sha512_224(origin is @byte)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] object `Sha512_256` does `Digesting`
  - [ ] `Sha512_256()`
  - [ ] `Sha512_256(origin is @byte)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] Implement SHA-512/224 as its standardized variant.
- [ ] Implement SHA-512/256 as its standardized variant.
- [ ] Do not implement SHA-512/224 by truncating a SHA-512 result.
- [ ] Do not implement SHA-512/256 by truncating a SHA-512 result.
- [ ] Add official empty-input vectors for every SHA-2 variant.
- [ ] Add official `abc` vectors for every SHA-2 variant.
- [ ] Verify chunked and one-shot results for every SHA-2 variant.

## `digest/sha3`

- [ ] object `Sha3_224` does `Digesting`
  - [ ] `Sha3_224()`
  - [ ] `Sha3_224(origin is @byte)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] object `Sha3_256` does `Digesting`
  - [ ] `Sha3_256()`
  - [ ] `Sha3_256(origin is @byte)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] object `Sha3_384` does `Digesting`
  - [ ] `Sha3_384()`
  - [ ] `Sha3_384(origin is @byte)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] object `Sha3_512` does `Digesting`
  - [ ] `Sha3_512()`
  - [ ] `Sha3_512(origin is @byte)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] Use official SHA-3 names with an underscore before output size.
- [ ] Add official empty-input vectors for every SHA-3 variant.
- [ ] Add official `abc` vectors for every SHA-3 variant.
- [ ] Verify chunked and one-shot results for every SHA-3 variant.

## `digest/shake`

- [ ] object `Shake128` does `Digesting`
  - [ ] `Shake128(size is int)`
  - [ ] `Shake128(origin is @byte, size is int)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] object `Shake256` does `Digesting`
  - [ ] `Shake256(size is int)`
  - [ ] `Shake256(origin is @byte, size is int)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] Interpret size as output bytes rather than bits.
- [ ] Reject a non-positive output size.
- [ ] Preserve output size through `state()`.
- [ ] Add official vectors at multiple output sizes.
- [ ] Verify chunked and one-shot results.
- [ ] Do not allow SHAKE as an HMAC algorithm.

## `digest/blake2`

- [ ] object `Blake2b` does `Digesting`
  - [ ] `Blake2b()`
  - [ ] `Blake2b(origin is @byte)`
  - [ ] `Blake2b(origin is @byte, size is int)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] object `Blake2s` does `Digesting`
  - [ ] `Blake2s()`
  - [ ] `Blake2s(origin is @byte)`
  - [ ] `Blake2s(origin is @byte, size is int)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] object `KeyedBlake2b` does `Digesting`
  - [ ] `KeyedBlake2b(key is @byte, size is int)`
  - [ ] `KeyedBlake2b(origin is @byte, key is @byte, size is int)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] object `KeyedBlake2s` does `Digesting`
  - [ ] `KeyedBlake2s(key is @byte, size is int)`
  - [ ] `KeyedBlake2s(origin is @byte, key is @byte, size is int)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] Default `Blake2b` output to 64 bytes.
- [ ] Default `Blake2s` output to 32 bytes.
- [ ] Accept `Blake2b` output sizes from 1 through 64 bytes.
- [ ] Accept `Blake2s` output sizes from 1 through 32 bytes.
- [ ] Accept `KeyedBlake2b` keys from 1 through 64 bytes.
- [ ] Accept `KeyedBlake2s` keys from 1 through 32 bytes.
- [ ] Preserve output size and key state through `state()`.
- [ ] Add official keyed and unkeyed vectors.
- [ ] Verify chunked and one-shot results.
- [ ] Keep BLAKE2 tree mode, salt and personalization out of the first API.

## `digest/read`

- [ ] object `ReadingDigest`
  - [ ] `ReadingDigest(origin is Reading, digest is Digesting)`
  - [ ] `ReadingDigest(origin is Reading, digest is Digesting, chunk_size is int)`
  - [ ] `result() is @byte`
- [ ] Choose and document the default chunk size.
- [ ] Reject a non-positive explicit chunk size.
- [ ] Read until `Reading.bytes()` returns an empty array.
- [ ] Continue the supplied digest state instead of replacing it.
- [ ] Propagate reading errors unchanged.
- [ ] Leave the source open after success.
- [ ] Leave the source lifecycle under caller control after failure.
- [ ] Test with reads shorter than the requested chunk size.
- [ ] Test empty input.
- [ ] Test a source larger than several chunks.

## `crypto/constant-time`

- [ ] object `ConstantTimeEqual`
  - [ ] `ConstantTimeEqual(left is @byte, right is @byte)`
  - [ ] `#projected value() is bool`
- [ ] Compare every byte when input lengths are equal.
- [ ] Do not return at the first differing byte.
- [ ] Document that input length is not hidden.
- [ ] Accept only raw bytes.
- [ ] Add equal, unequal and unequal-length tests.
- [ ] Add a regression test preventing replacement with ordinary array equality.

## `crypto/hmac`

- [ ] enum `HmacAlgorithm`
  - [ ] `md5`
  - [ ] `sha1`
  - [ ] `sha224`
  - [ ] `sha256`
  - [ ] `sha384`
  - [ ] `sha512`
  - [ ] `sha3_224`
  - [ ] `sha3_256`
  - [ ] `sha3_384`
  - [ ] `sha3_512`
- [ ] object `Hmac` does `Digesting`
  - [ ] `Hmac(key is @byte, algorithm is HmacAlgorithm)`
  - [ ] `Hmac(origin is @byte, key is @byte, algorithm is HmacAlgorithm)`
  - [ ] `state(origin is @byte) is Digesting`
  - [ ] `bytes() is @byte`
- [ ] Accept keys of any byte length according to HMAC key normalization.
- [ ] Preserve key and algorithm through `state()`.
- [ ] Add RFC 4231 vectors for HMAC-SHA224, HMAC-SHA256, HMAC-SHA384 and HMAC-SHA512.
- [ ] Add NIST vectors for every HMAC-SHA3 variant.
- [ ] Add RFC 2202 interoperability vectors for HMAC-MD5 and HMAC-SHA1.
- [ ] Document HMAC-MD5 and HMAC-SHA1 as legacy interoperability only.
- [ ] Verify HMAC tags through `ConstantTimeEqual`.
- [ ] Do not add a separate `ValidHmac` object.
- [ ] Do not allow SHAKE as an HMAC algorithm.
- [ ] Do not wrap keyed BLAKE2 in HMAC.

## Stage 15 exclusions

- [ ] Do not add a universal `Hash` object.
- [ ] Do not add a checksum-algorithm enum.
- [ ] Do not add CRC variants without their exact standardized names.
- [ ] Do not add FNV without a concrete public consumer.
- [ ] Do not add xxHash or MurmurHash without a concrete public consumer.
- [ ] Do not digest `string` without an explicit Unicode encoding.
- [ ] Do not add digest-specific hexadecimal actions.
- [ ] Keep password hashing and KDFs in Stage 18.
- [ ] Keep digital signatures in Stage 18.
- [ ] Do not use a raw digest as a message authentication code.

---

# Stage 16 — compression

## `compression/state`

- [ ] role `Compressing`
  - [ ] `write(origin is @byte) is void`
  - [ ] `output() is Reading`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
- [ ] role `Decompressing`
  - [ ] `write(origin is @byte) is void`
  - [ ] `output() is Reading`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
  - [ ] `remainder() is @byte`
- [ ] object `CompressionLimitExceeded` does `Error`
  - [ ] `CompressionLimitExceeded(maximum is int)`
  - [ ] `maximum() is int`
  - [ ] `text() is string`
- [ ] Treat compressors and decompressors as changing resources.
- [ ] Allow `write()` to make no output immediately.
- [ ] Make `finish()` make every buffered output byte available from `output()`.
- [ ] Make successful `finish()` set `finished() == true`.
- [ ] Throw `InvalidState` when writing to a finished codec.
- [ ] Throw `InvalidState` when finishing a finished codec again.
- [ ] Return trailing input from `remainder()` only after a stream completes.
- [ ] Apply decompression limits to cumulative emitted output.
- [ ] Reject non-positive decompression limits.
- [ ] Keep native codec error messages out of stable public errors.

## `compression/read`

- [ ] object `CompressedReading` does `Reading`
  - [ ] `CompressedReading(origin is Reading, compressor is Compressing)`
  - [ ] `bytes(limit is int) is @byte`
- [ ] object `DecompressedReading` does `Reading`
  - [ ] `DecompressedReading(origin is Reading, decompressor is Decompressing)`
  - [ ] `bytes(limit is int) is @byte`
- [ ] Never return more than the requested limit.
- [ ] Reject a non-positive read limit.
- [ ] Buffer codec output that does not fit the requested limit.
- [ ] Continue reading when a codec emits an empty intermediate chunk.
- [ ] Finalize the codec after the origin reaches EOF.
- [ ] Return EOF only after final codec output is consumed.
- [ ] Propagate origin and codec errors unchanged.
- [ ] Leave the origin open after success.
- [ ] Leave origin lifecycle under caller control after failure.
- [ ] Do not make compressed readings iterable cursors.

## `compression/gzip`

- [ ] object `GzipCompressed`
  - [ ] `GzipCompressed(origin is @byte)`
  - [ ] `GzipCompressed(origin is @byte, level is int)`
  - [ ] `bytes() is @byte`
- [ ] object `GzipDecompressed`
  - [ ] `GzipDecompressed(origin is @byte, maximum_size is int)`
  - [ ] `bytes() is @byte`
- [ ] object `GzipCompression` does `Compressing`
  - [ ] `GzipCompression()`
  - [ ] `GzipCompression(level is int)`
  - [ ] `write(origin is @byte) is void`
  - [ ] `output() is Reading`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
- [ ] object `GzipDecompression` does `Decompressing`
  - [ ] `GzipDecompression(maximum_size is int)`
  - [ ] `write(origin is @byte) is void`
  - [ ] `output() is Reading`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
  - [ ] `remainder() is @byte`
- [ ] object `InvalidGzip` does `Error`
  - [ ] `text() is string`
- [ ] Implement the RFC 1952 wrapper.
- [ ] Use compression level `6` by default.
- [ ] Accept compression levels from `0` through `9`.
- [ ] Set default modification time to the Unix epoch.
- [ ] Omit filename and comment from the default header.
- [ ] Emit a deterministic OS marker.
- [ ] Validate the CRC-32 trailer.
- [ ] Validate the uncompressed-size trailer.
- [ ] Decode every concatenated gzip member.
- [ ] Apply maximum size across all concatenated members.
- [ ] Add empty-input and multi-member vectors.
- [ ] Test damaged header, payload and trailer separately.
- [ ] Verify one-shot and streaming interoperability.

## `compression/zlib`

- [ ] object `ZlibCompressed`
  - [ ] `ZlibCompressed(origin is @byte)`
  - [ ] `ZlibCompressed(origin is @byte, level is int)`
  - [ ] `ZlibCompressed(origin is @byte, level is int, dictionary is @byte)`
  - [ ] `bytes() is @byte`
- [ ] object `ZlibDecompressed`
  - [ ] `ZlibDecompressed(origin is @byte, maximum_size is int)`
  - [ ] `ZlibDecompressed(origin is @byte, maximum_size is int, dictionary is @byte)`
  - [ ] `bytes() is @byte`
- [ ] object `ZlibCompression` does `Compressing`
  - [ ] `ZlibCompression()`
  - [ ] `ZlibCompression(level is int)`
  - [ ] `ZlibCompression(level is int, dictionary is @byte)`
  - [ ] `write(origin is @byte) is void`
  - [ ] `output() is Reading`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
- [ ] object `ZlibDecompression` does `Decompressing`
  - [ ] `ZlibDecompression(maximum_size is int)`
  - [ ] `ZlibDecompression(maximum_size is int, dictionary is @byte)`
  - [ ] `write(origin is @byte) is void`
  - [ ] `output() is Reading`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
  - [ ] `remainder() is @byte`
- [ ] object `InvalidZlib` does `Error`
  - [ ] `text() is string`
- [ ] object `MissingZlibDictionary` does `Error`
  - [ ] `identifier() is int`
  - [ ] `text() is string`
- [ ] Implement the RFC 1950 wrapper around DEFLATE.
- [ ] Use compression level `6` by default.
- [ ] Accept compression levels from `0` through `9`.
- [ ] Validate the Adler-32 trailer.
- [ ] Validate a supplied preset dictionary identifier.
- [ ] Throw `MissingZlibDictionary` when required dictionary data is absent.
- [ ] Reject trailing bytes in one-shot decompression.
- [ ] Add preset-dictionary interoperability vectors.
- [ ] Test damaged header, payload and trailer separately.
- [ ] Verify one-shot and streaming interoperability.

## `compression/deflate`

- [ ] object `RawDeflated`
  - [ ] `RawDeflated(origin is @byte)`
  - [ ] `RawDeflated(origin is @byte, level is int)`
  - [ ] `RawDeflated(origin is @byte, level is int, dictionary is @byte)`
  - [ ] `bytes() is @byte`
- [ ] object `RawInflated`
  - [ ] `RawInflated(origin is @byte, maximum_size is int)`
  - [ ] `RawInflated(origin is @byte, maximum_size is int, dictionary is @byte)`
  - [ ] `bytes() is @byte`
- [ ] object `RawDeflateCompression` does `Compressing`
  - [ ] `RawDeflateCompression()`
  - [ ] `RawDeflateCompression(level is int)`
  - [ ] `RawDeflateCompression(level is int, dictionary is @byte)`
  - [ ] `write(origin is @byte) is void`
  - [ ] `output() is Reading`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
- [ ] object `RawDeflateDecompression` does `Decompressing`
  - [ ] `RawDeflateDecompression(maximum_size is int)`
  - [ ] `RawDeflateDecompression(maximum_size is int, dictionary is @byte)`
  - [ ] `write(origin is @byte) is void`
  - [ ] `output() is Reading`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
  - [ ] `remainder() is @byte`
- [ ] object `InvalidDeflate` does `Error`
  - [ ] `text() is string`
- [ ] Implement raw RFC 1951 DEFLATE without gzip or zlib wrappers.
- [ ] Use compression level `6` by default.
- [ ] Accept compression levels from `0` through `9`.
- [ ] Reject trailing bytes in one-shot decompression.
- [ ] Add preset-dictionary interoperability vectors.
- [ ] Test truncated and malformed block streams.
- [ ] Verify one-shot and streaming interoperability.
- [ ] Do not add a boolean raw-mode flag to zlib objects.

## `compression/bzip2`

- [ ] object `Bzip2Compressed`
  - [ ] `Bzip2Compressed(origin is @byte)`
  - [ ] `Bzip2Compressed(origin is @byte, level is int)`
  - [ ] `bytes() is @byte`
- [ ] object `Bzip2Decompressed`
  - [ ] `Bzip2Decompressed(origin is @byte, maximum_size is int)`
  - [ ] `bytes() is @byte`
- [ ] object `Bzip2Compression` does `Compressing`
  - [ ] `Bzip2Compression()`
  - [ ] `Bzip2Compression(level is int)`
  - [ ] `write(origin is @byte) is void`
  - [ ] `output() is Reading`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
- [ ] object `Bzip2Decompression` does `Decompressing`
  - [ ] `Bzip2Decompression(maximum_size is int)`
  - [ ] `write(origin is @byte) is void`
  - [ ] `output() is Reading`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
  - [ ] `remainder() is @byte`
- [ ] object `InvalidBzip2` does `Error`
  - [ ] `text() is string`
- [ ] Use compression level `9` by default.
- [ ] Accept compression levels from `1` through `9`.
- [ ] Decode every concatenated bzip2 stream in one-shot mode.
- [ ] Apply maximum size across all concatenated streams.
- [ ] Test damaged and truncated streams.
- [ ] Verify one-shot and streaming interoperability.

## `compression/xz`

- [ ] object `XzCompressed`
  - [ ] `XzCompressed(origin is @byte)`
  - [ ] `XzCompressed(origin is @byte, level is int)`
  - [ ] `bytes() is @byte`
- [ ] object `XzDecompressed`
  - [ ] `XzDecompressed(origin is @byte, maximum_size is int)`
  - [ ] `bytes() is @byte`
- [ ] object `XzCompression` does `Compressing`
  - [ ] `XzCompression()`
  - [ ] `XzCompression(level is int)`
  - [ ] `write(origin is @byte) is void`
  - [ ] `output() is Reading`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
- [ ] object `XzDecompression` does `Decompressing`
  - [ ] `XzDecompression(maximum_size is int)`
  - [ ] `write(origin is @byte) is void`
  - [ ] `output() is Reading`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
  - [ ] `remainder() is @byte`
- [ ] object `InvalidXz` does `Error`
  - [ ] `text() is string`
- [ ] Use compression level `6` by default.
- [ ] Accept compression levels from `0` through `9`.
- [ ] Validate XZ integrity checks.
- [ ] Decode concatenated XZ streams and permitted stream padding.
- [ ] Apply maximum size across all concatenated streams.
- [ ] Test damaged index, footer and integrity checks.
- [ ] Verify one-shot and streaming interoperability.

## `compression/lzma`

- [ ] object `LzmaCompressed`
  - [ ] `LzmaCompressed(origin is @byte)`
  - [ ] `LzmaCompressed(origin is @byte, level is int)`
  - [ ] `bytes() is @byte`
- [ ] object `LzmaDecompressed`
  - [ ] `LzmaDecompressed(origin is @byte, maximum_size is int)`
  - [ ] `bytes() is @byte`
- [ ] object `LzmaCompression` does `Compressing`
  - [ ] `LzmaCompression()`
  - [ ] `LzmaCompression(level is int)`
  - [ ] `write(origin is @byte) is void`
  - [ ] `output() is Reading`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
- [ ] object `LzmaDecompression` does `Decompressing`
  - [ ] `LzmaDecompression(maximum_size is int)`
  - [ ] `write(origin is @byte) is void`
  - [ ] `output() is Reading`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
  - [ ] `remainder() is @byte`
- [ ] object `InvalidLzma` does `Error`
  - [ ] `text() is string`
- [ ] Implement the legacy LZMA-Alone format.
- [ ] Use compression level `6` by default.
- [ ] Accept compression levels from `0` through `9`.
- [ ] Reject trailing bytes in one-shot decompression.
- [ ] Test damaged and truncated streams.
- [ ] Verify one-shot and streaming interoperability.
- [ ] Do not add raw LZMA filter chains without a concrete consumer.

## `compression/zstd`

- [ ] object `ZstdOptions`
  - [ ] `ZstdOptions(level is int, checksum is bool, content_size is bool)`
  - [ ] `level() is int`
  - [ ] `checksum() is bool`
  - [ ] `content_size() is bool`
- [ ] object `ZstdDictionary`
  - [ ] `ZstdDictionary(data is @byte)`
  - [ ] `data() is @byte`
  - [ ] `identifier() is int`
- [ ] object `TrainedZstdDictionary`
  - [ ] `TrainedZstdDictionary(samples is Iterating<@byte>, size is int)`
  - [ ] `result() is ZstdDictionary`
- [ ] object `ZstdCompressed`
  - [ ] `ZstdCompressed(origin is @byte)`
  - [ ] `ZstdCompressed(origin is @byte, options is ZstdOptions)`
  - [ ] `ZstdCompressed(origin is @byte, options is ZstdOptions, dictionary is ZstdDictionary)`
  - [ ] `bytes() is @byte`
- [ ] object `ZstdDecompressed`
  - [ ] `ZstdDecompressed(origin is @byte, maximum_size is int)`
  - [ ] `ZstdDecompressed(origin is @byte, maximum_size is int, dictionary is ZstdDictionary)`
  - [ ] `bytes() is @byte`
- [ ] object `ZstdCompression` does `Compressing`
  - [ ] `ZstdCompression()`
  - [ ] `ZstdCompression(options is ZstdOptions)`
  - [ ] `ZstdCompression(options is ZstdOptions, dictionary is ZstdDictionary)`
  - [ ] `write(origin is @byte) is void`
  - [ ] `output() is Reading`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
- [ ] object `ZstdDecompression` does `Decompressing`
  - [ ] `ZstdDecompression(maximum_size is int)`
  - [ ] `ZstdDecompression(maximum_size is int, dictionary is ZstdDictionary)`
  - [ ] `write(origin is @byte) is void`
  - [ ] `output() is Reading`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
  - [ ] `remainder() is @byte`
- [ ] object `InvalidZstd` does `Error`
  - [ ] `text() is string`
- [ ] object `MissingZstdDictionary` does `Error`
  - [ ] `MissingZstdDictionary(identifier is int)`
  - [ ] `identifier() is int`
  - [ ] `text() is string`
- [ ] Use compression level `3` by default.
- [ ] Define stable supported compression-level bounds.
- [ ] Bind dictionary bytes during construction and validate them lazily on first use or `identifier()`.
- [ ] Reject an empty dictionary-training sample set.
- [ ] Reject a non-positive dictionary size.
- [ ] Perform dictionary training only through `result()`.
- [ ] Decode every concatenated zstd frame in one-shot mode.
- [ ] Apply maximum size across all concatenated frames.
- [ ] Validate frame checksums when present.
- [ ] Throw `MissingZstdDictionary` with the required dictionary identifier.
- [ ] Add official dictionary and multi-frame vectors.
- [ ] Test damaged headers, blocks and checksums.
- [ ] Verify one-shot and streaming interoperability.
- [ ] Keep advanced native compression parameters out of the first API.

## Stage 16 exclusions

- [ ] Do not combine gzip, zlib and raw DEFLATE behind a boolean mode.
- [ ] Do not add a universal compression-level value.
- [ ] Do not decompress without an explicit maximum output size.
- [ ] Do not hide blocking compression work inside cursor traversal.
- [ ] Do not add format-specific compressed-file objects.
- [ ] Do not let reading adapters close their origins.
- [ ] Do not expose native codec error strings as stable API text.
- [ ] Do not expose raw LZMA filter chains without a concrete consumer.
- [ ] Keep archive entries and extraction in Stage 17.

---

# Stage 17 — archives

## `archive/path`

- [ ] object `ArchivePath`
  - [ ] `ArchivePath(origin is string)`
  - [ ] `string() is string`
  - [ ] `segments() is Sequencing<string>`
  - [ ] `parent() is ArchivePath`
  - [ ] `name() is string`
- [ ] object `UnsafeArchivePath` does `Error`
  - [ ] `UnsafeArchivePath(origin is string, message is string)`
  - [ ] `path() is string`
  - [ ] `text() is string`
- [ ] object `MissingArchiveParent` does `Error`
  - [ ] `MissingArchiveParent(path is ArchivePath)`
  - [ ] `path() is ArchivePath`
  - [ ] `text() is string`
- [ ] Reject an empty archive path.
- [ ] Reject absolute paths.
- [ ] Reject `.` path segments.
- [ ] Reject `..` path segments.
- [ ] Reject empty path segments.
- [ ] Reject backslash as a path separator.
- [ ] Reject NUL characters.
- [ ] Reject Windows drive prefixes.
- [ ] Reject UNC prefixes.
- [ ] Use `/` as the only archive path separator.
- [ ] Preserve valid Unicode path characters exactly.
- [ ] Make `parent()` throw `MissingArchiveParent` for a one-segment path.
- [ ] Keep archive paths independent from filesystem `Path`.

## `archive/metadata`

- [ ] object `ArchiveOwnership`
  - [ ] `ArchiveOwnership(user is string, group is string, user_identity is BigInt, group_identity is BigInt)`
  - [ ] `user() is string`
  - [ ] `group() is string`
  - [ ] `user_identity() is BigInt`
  - [ ] `group_identity() is BigInt`
- [ ] object `ArchiveMetadata`
  - [ ] `ArchiveMetadata(modified is LocalDateTime, comment is string)`
  - [ ] `ArchiveMetadata(mode is PermissionMode, modified is LocalDateTime, comment is string)`
  - [ ] `ArchiveMetadata(mode is PermissionMode, modified is LocalDateTime, ownership is ArchiveOwnership, comment is string)`
  - [ ] `mode() is PermissionMode`
  - [ ] `modification() is LocalDateTime`
  - [ ] `ownership() is ArchiveOwnership`
  - [ ] `comment() is string`
- [ ] object `MissingArchiveMode` does `Error`
  - [ ] `text() is string`
- [ ] object `MissingArchiveOwnership` does `Error`
  - [ ] `text() is string`
- [ ] Make `mode()` throw `MissingArchiveMode` when mode is absent.
- [ ] Make `ownership()` throw `MissingArchiveOwnership` when ownership is absent.
- [ ] Preserve timezone-naive ZIP calendar fields as `LocalDateTime`.
- [ ] Represent TAR epoch timestamps as UTC `LocalDateTime` values.
- [ ] Never interpret ZIP timestamps through the process timezone.
- [ ] Do not use magic identities such as `-1` for missing ownership.

## `archive/entry`

- [ ] enum `ArchiveEntryKind`
  - [ ] `file`
  - [ ] `directory`
  - [ ] `symbolic_link`
  - [ ] `hard_link`
  - [ ] `fifo`
  - [ ] `character_device`
  - [ ] `block_device`
- [ ] role `ArchiveEntry`
  - [ ] `path() is ArchivePath`
  - [ ] `kind() is ArchiveEntryKind`
  - [ ] `metadata() is ArchiveMetadata`
- [ ] object `ArchiveFile` does `ArchiveEntry`
  - [ ] `ArchiveFile(path is ArchivePath, metadata is ArchiveMetadata, data is @byte)`
  - [ ] `path() is ArchivePath`
  - [ ] `kind() is ArchiveEntryKind`
  - [ ] `metadata() is ArchiveMetadata`
  - [ ] `data() is @byte`
- [ ] object `ArchiveDirectory` does `ArchiveEntry`
  - [ ] `ArchiveDirectory(path is ArchivePath, metadata is ArchiveMetadata)`
  - [ ] `path() is ArchivePath`
  - [ ] `kind() is ArchiveEntryKind`
  - [ ] `metadata() is ArchiveMetadata`
- [ ] object `ArchiveSymbolicLink` does `ArchiveEntry`
  - [ ] `ArchiveSymbolicLink(path is ArchivePath, metadata is ArchiveMetadata, target is ArchivePath)`
  - [ ] `path() is ArchivePath`
  - [ ] `kind() is ArchiveEntryKind`
  - [ ] `metadata() is ArchiveMetadata`
  - [ ] `target() is ArchivePath`
- [ ] object `ArchiveHardLink` does `ArchiveEntry`
  - [ ] `ArchiveHardLink(path is ArchivePath, metadata is ArchiveMetadata, target is ArchivePath)`
  - [ ] `path() is ArchivePath`
  - [ ] `kind() is ArchiveEntryKind`
  - [ ] `metadata() is ArchiveMetadata`
  - [ ] `target() is ArchivePath`
- [ ] object `ArchiveFifo` does `ArchiveEntry`
  - [ ] `ArchiveFifo(path is ArchivePath, metadata is ArchiveMetadata)`
  - [ ] `path() is ArchivePath`
  - [ ] `kind() is ArchiveEntryKind`
  - [ ] `metadata() is ArchiveMetadata`
- [ ] object `ArchiveDevice` does `ArchiveEntry`
  - [ ] `ArchiveDevice(path is ArchivePath, kind is ArchiveEntryKind, metadata is ArchiveMetadata, major is int, minor is int)`
  - [ ] `path() is ArchivePath`
  - [ ] `kind() is ArchiveEntryKind`
  - [ ] `metadata() is ArchiveMetadata`
  - [ ] `major() is int`
  - [ ] `minor() is int`
- [ ] Reject a device kind other than `character_device` or `block_device`.
- [ ] Reject negative device numbers.
- [ ] Keep every archive entry immutable.
- [ ] Keep file data unavailable on non-file entries.
- [ ] Keep symbolic-link and hard-link targets distinct.
- [ ] Keep the noun role name because an entry is a domain value.

## `archive/limits`

- [ ] object `ArchiveLimits`
  - [ ] `ArchiveLimits(entries is int, entry_size is int, total_size is int, path_length is int)`
  - [ ] `entries() is int`
  - [ ] `entry_size() is int`
  - [ ] `total_size() is int`
  - [ ] `path_length() is int`
- [ ] object `ArchiveLimitExceeded` does `Error`
  - [ ] `ArchiveLimitExceeded(limit is string, maximum is int)`
  - [ ] `limit() is string`
  - [ ] `maximum() is int`
  - [ ] `text() is string`
- [ ] Reject every non-positive limit.
- [ ] Apply entry count before appending a parsed entry.
- [ ] Apply entry size before allocating entry data.
- [ ] Apply total size cumulatively across every entry.
- [ ] Apply path length before constructing `ArchivePath`.
- [ ] Verify actual expanded size instead of trusting archive headers.

## `archive/file`

- [ ] object `ArchivedFile`
  - [ ] `ArchivedFile(origin is FileEntry, path is ArchivePath, maximum_size is int)`
  - [ ] `result() is ArchiveFile`
- [ ] Reject a non-positive maximum size.
- [ ] Read filesystem data only through `result()`.
- [ ] Reject a filesystem entry that is not a regular file.
- [ ] Reject a file larger than maximum size before reading when size is known.
- [ ] Enforce maximum size while reading when size changes concurrently.
- [ ] Read the file without following a symbolic link.
- [ ] Capture available mode and modification metadata.
- [ ] Do not hide filesystem I/O behind `ArchiveFile.data()`.
- [ ] Remove the legacy archive-local `FileEntry` and `DataEntry` objects.

## `archive/stream`

- [ ] role `ReadingArchive`
  - [ ] `entry() is ArchiveEntry`
  - [ ] `finished() is bool`
- [ ] role `WritingArchive`
  - [ ] `write(entry is ArchiveEntry) is void`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
- [ ] object `EndOfArchive` does `Error`
  - [ ] `text() is string`
- [ ] Make archive readings and writings changing resources.
- [ ] Make `entry()` throw `EndOfArchive` after normal completion.
- [ ] Make successful `finish()` set `finished() == true`.
- [ ] Throw `InvalidState` when writing a finished archive.
- [ ] Throw `InvalidState` when finishing an archive twice.
- [ ] Materialize at most one bounded entry at a time while reading.
- [ ] Keep blocking archive work out of cursor traversal.
- [ ] Leave underlying reading and writing lifecycle under caller control.

## `archive/tar`

- [ ] enum `TarFormat`
  - [ ] `ustar`
  - [ ] `pax`
- [ ] object `TarArchive`
  - [ ] `TarArchive(entries is Iterating<ArchiveEntry>)`
  - [ ] `TarArchive(entries is Iterating<ArchiveEntry>, format is TarFormat)`
  - [ ] `bytes() is @byte`
- [ ] object `ParsedTar`
  - [ ] `ParsedTar(origin is @byte, limits is ArchiveLimits)`
  - [ ] `entries() is Sequencing<ArchiveEntry>`
- [ ] object `TarReading` does `ReadingArchive`
  - [ ] `TarReading(origin is Reading, limits is ArchiveLimits)`
  - [ ] `entry() is ArchiveEntry`
  - [ ] `finished() is bool`
- [ ] object `TarWriting` does `WritingArchive`
  - [ ] `TarWriting(origin is Writing)`
  - [ ] `TarWriting(origin is Writing, format is TarFormat)`
  - [ ] `write(entry is ArchiveEntry) is void`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
- [ ] object `InvalidTar` does `Error`
  - [ ] `text() is string`
- [ ] object `DuplicateArchivePath` does `Error`
  - [ ] `DuplicateArchivePath(path is ArchivePath)`
  - [ ] `path() is ArchivePath`
  - [ ] `text() is string`
- [ ] Use PAX as the default writing format.
- [ ] Validate every TAR header checksum.
- [ ] Parse POSIX.1-2001 PAX headers.
- [ ] Parse GNU long-name and long-link extensions.
- [ ] Do not write GNU format by default.
- [ ] Reject duplicate archive paths.
- [ ] Reject truncated entry data and missing padding.
- [ ] Preserve declared entry order.
- [ ] Support regular files and directories.
- [ ] Support symbolic and hard links.
- [ ] Support FIFOs and character/block devices as values.
- [ ] Test USTAR field-size boundaries.
- [ ] Test PAX long paths and large numeric fields.
- [ ] Verify value and streaming APIs interoperate.
- [ ] Do not add compressed TAR variants.
- [ ] Compose compressed TAR through Stage 16 codecs.

## `archive/zip`

- [ ] enum `ZipMethod`
  - [ ] `stored`
  - [ ] `deflated`
  - [ ] `bzip2`
  - [ ] `lzma`
  - [ ] `zstandard`
- [ ] object `ZipCompression`
  - [ ] `ZipCompression(method is ZipMethod)`
  - [ ] `ZipCompression(method is ZipMethod, level is int)`
  - [ ] `method() is ZipMethod`
  - [ ] `level() is int`
- [ ] object `ZipArchiveEntry`
  - [ ] `ZipArchiveEntry(entry is ArchiveEntry, compression is ZipCompression)`
  - [ ] `entry() is ArchiveEntry`
  - [ ] `compression() is ZipCompression`
- [ ] object `ZipArchive`
  - [ ] `ZipArchive(entries is Iterating<ZipArchiveEntry>)`
  - [ ] `bytes() is @byte`
- [ ] object `ParsedZip`
  - [ ] `ParsedZip(origin is @byte, limits is ArchiveLimits)`
  - [ ] `entries() is Sequencing<ZipArchiveEntry>`
- [ ] object `ZipReading` does `ReadingArchive`
  - [ ] `ZipReading(origin is Reading, limits is ArchiveLimits)`
  - [ ] `entry() is ArchiveEntry`
  - [ ] `finished() is bool`
- [ ] object `ZipWriting` does `WritingArchive`
  - [ ] `ZipWriting(origin is Writing)`
  - [ ] `ZipWriting(origin is Writing, compression is ZipCompression)`
  - [ ] `write(entry is ArchiveEntry) is void`
  - [ ] `write(entry is ZipArchiveEntry) is void`
  - [ ] `finish() is void`
  - [ ] `finished() is bool`
- [ ] object `InvalidZip` does `Error`
  - [ ] `text() is string`
- [ ] object `UnsupportedZipCompression` does `Error`
  - [ ] `UnsupportedZipCompression(identifier is int)`
  - [ ] `identifier() is int`
  - [ ] `text() is string`
- [ ] object `EncryptedZipEntry` does `Error`
  - [ ] `EncryptedZipEntry(path is ArchivePath)`
  - [ ] `path() is ArchivePath`
  - [ ] `text() is string`
- [ ] Use DEFLATE as the default writing method.
- [ ] Resolve a default level for every compression method.
- [ ] Reject explicit levels for stored and LZMA entries.
- [ ] Return level `0` for stored and LZMA entries.
- [ ] Validate levels according to the selected compression method.
- [ ] Enable ZIP64 automatically when required.
- [ ] Write entry names as UTF-8.
- [ ] Honor the UTF-8 filename flag while reading.
- [ ] Decode unflagged legacy names as CP437.
- [ ] Never decode names through the process locale.
- [ ] Validate local and central-directory consistency.
- [ ] Validate every entry CRC-32.
- [ ] Reject duplicate archive paths.
- [ ] Reject encrypted entries in the first API.
- [ ] Reject multipart ZIP archives.
- [ ] Read Zstandard method identifiers `20` and `93`.
- [ ] Write only Zstandard method identifier `93`.
- [ ] Preserve declared entry order.
- [ ] Test stored, DEFLATE, bzip2, LZMA and Zstandard entries.
- [ ] Test mixed compression methods in one archive.
- [ ] Test data descriptors and non-seekable writing.
- [ ] Test ZIP64 size and entry-count boundaries.
- [ ] Verify value and streaming APIs interoperate.

## `archive/extract`

- [ ] object `ExtractedArchiveEntry`
  - [ ] `ExtractedArchiveEntry(origin is ArchiveEntry, destination is Directory)`
  - [ ] `result() is Path`
- [ ] object `UnsupportedArchiveEntry` does `Error`
  - [ ] `UnsupportedArchiveEntry(kind is ArchiveEntryKind)`
  - [ ] `kind() is ArchiveEntryKind`
  - [ ] `text() is string`
- [ ] object `ExistingArchiveDestination` does `Error`
  - [ ] `ExistingArchiveDestination(path is Path)`
  - [ ] `path() is Path`
  - [ ] `text() is string`
- [ ] Extract regular files.
- [ ] Extract directories.
- [ ] Reject symbolic links.
- [ ] Reject hard links.
- [ ] Reject FIFOs.
- [ ] Reject character devices.
- [ ] Reject block devices.
- [ ] Never overwrite an existing destination.
- [ ] Resolve every entry relative to the supplied directory.
- [ ] Use directory-relative no-follow filesystem operations.
- [ ] Reject a symlink in any existing destination path component.
- [ ] Reject platform-specific device and reserved names.
- [ ] Reject case-folding path collisions on case-insensitive filesystems.
- [ ] Create each file with a safe initial mode.
- [ ] Do not restore archive ownership.
- [ ] Do not restore special permission bits.
- [ ] Remove a partially created file after a failed write.
- [ ] Limit failure effects to the current entry.
- [ ] Leave the destination directory lifecycle under caller control.
- [ ] Do not add batch extraction.

## Stage 17 exclusions

- [ ] Do not add RAR support.
- [ ] Do not add 7z support.
- [ ] Do not add CPIO without a concrete consumer.
- [ ] Do not add multipart ZIP support in the first API.
- [ ] Do not create encrypted ZIP archives.
- [ ] Do not detect archive format from a filename extension.
- [ ] Do not restore archive ownership by default.
- [ ] Do not create archive links through safe extraction.
- [ ] Do not create FIFOs or devices through safe extraction.
- [ ] Do not add `extract_all()` or an equivalent batch effect.
- [ ] Do not use filesystem `Path` inside archive values.
- [ ] Do not hide TAR compression inside archive objects.
- [ ] Do not add archive-as-filesystem abstractions in the first API.

---

# Stage 18 — cryptography

## `crypto/secret`

- [ ] object `SecretBytes`
  - [ ] `SecretBytes(origin is @byte)`
  - [ ] `size() is int`
- [ ] object `ExportedSecret`
  - [ ] `ExportedSecret(origin is SecretBytes)`
  - [ ] `bytes() is @byte`
- [ ] Keep secret contents unavailable through ordinary observations.
- [ ] Do not add `bytes()`, `string()` or `text()` to `SecretBytes`.
- [ ] Do not make `SecretBytes` a `ByteSource`.
- [ ] Do not stream secrets through `<-`.
- [ ] Do not add ordinary equality for secrets.
- [ ] Require `ConstantTimeEqual` for exported-secret comparison.
- [ ] Make export explicit through `ExportedSecret`.
- [ ] Document that a garbage-collected runtime cannot guarantee complete zeroization.
- [ ] Do not duplicate `SecureRandom` from Stage 09.
- [ ] Require the concrete `SecureRandom` object for cryptographic generation.

## `crypto/kdf`

- [ ] enum `KdfAlgorithm`
  - [ ] `sha256`
  - [ ] `sha384`
  - [ ] `sha512`
- [ ] object `Hkdf`
  - [ ] `Hkdf(secret is SecretBytes, salt is @byte, info is @byte, size is int, algorithm is KdfAlgorithm)`
  - [ ] `secret() is SecretBytes`
- [ ] object `Pbkdf2`
  - [ ] `Pbkdf2(password is SecretBytes, salt is @byte, iterations is int, size is int, algorithm is KdfAlgorithm)`
  - [ ] `secret() is SecretBytes`
- [ ] object `ScryptParameters`
  - [ ] `ScryptParameters(cost is int, block_size is int, parallelism is int, size is int)`
  - [ ] `cost() is int`
  - [ ] `block_size() is int`
  - [ ] `parallelism() is int`
  - [ ] `size() is int`
- [ ] object `Scrypt`
  - [ ] `Scrypt(password is SecretBytes, salt is @byte, parameters is ScryptParameters)`
  - [ ] `secret() is SecretBytes`
- [ ] object `Argon2idParameters`
  - [ ] `Argon2idParameters(memory is int, iterations is int, parallelism is int, size is int)`
  - [ ] `memory() is int`
  - [ ] `iterations() is int`
  - [ ] `parallelism() is int`
  - [ ] `size() is int`
- [ ] object `RecommendedArgon2id`
  - [ ] `#projected value() is Argon2idParameters`
- [ ] object `Argon2id`
  - [ ] `Argon2id(password is SecretBytes, salt is @byte, parameters is Argon2idParameters)`
  - [ ] `secret() is SecretBytes`
- [ ] object `InvalidKdfParameters` does `Error`
  - [ ] `text() is string`
- [ ] Reject MD5 and SHA-1 as KDF algorithms.
- [ ] Reject a non-positive derived size.
- [ ] Enforce the RFC 5869 HKDF output-size bound.
- [ ] Permit an empty HKDF salt and info value.
- [ ] Reject a non-positive PBKDF2 iteration count.
- [ ] Document PBKDF2 as legacy and protocol interoperability.
- [ ] Require scrypt cost greater than one and a power of two.
- [ ] Reject non-positive scrypt block size and parallelism.
- [ ] Reject overflowing scrypt memory calculations before allocation.
- [ ] Reject non-positive Argon2id memory, iterations and parallelism.
- [ ] Require an Argon2id memory value compatible with parallelism.
- [ ] Define `RecommendedArgon2id` as 64 MiB, three iterations and four lanes.
- [ ] Use a 32-byte output in `RecommendedArgon2id`.
- [ ] Add RFC vectors for HKDF, PBKDF2, scrypt and Argon2id.

## `crypto/password`

- [ ] object `Argon2idPassword`
  - [ ] `Argon2idPassword(password is SecretBytes, random is SecureRandom)`
  - [ ] `Argon2idPassword(password is SecretBytes, random is SecureRandom, parameters is Argon2idParameters)`
  - [ ] `string() is string`
- [ ] object `VerifiedPassword`
  - [ ] `VerifiedPassword(password is SecretBytes, encoded is string)`
  - [ ] `#projected value() is bool`
- [ ] object `InvalidPasswordHash` does `Error`
  - [ ] `text() is string`
- [ ] object `PasswordHashLimitExceeded` does `Error`
  - [ ] `text() is string`
- [ ] Encode Argon2id password hashes in PHC string format.
- [ ] Generate a fresh 16-byte salt through `SecureRandom`.
- [ ] Use `RecommendedArgon2id` in the constructor without explicit parameters.
- [ ] Store algorithm version and all parameters in the encoded value.
- [ ] Parse and bound memory before allocating it during verification.
- [ ] Parse and bound iterations and parallelism before computing.
- [ ] Compare the computed tag in constant time.
- [ ] Return false for an incorrect password.
- [ ] Throw `InvalidPasswordHash` for malformed encoding.
- [ ] Throw `PasswordHashLimitExceeded` for valid but unacceptable resource parameters.
- [ ] Do not expose whether salt or tag caused verification failure.
- [ ] Do not make PBKDF2 the default password-storage format.

## `crypto/nonce`

- [ ] object `AeadNonce`
  - [ ] `AeadNonce(origin is @byte)`
  - [ ] `bytes() is @byte`
- [ ] object `NonceSequence`
  - [ ] `NonceSequence(prefix is @byte, initial is BigInt)`
  - [ ] `nonce() is AeadNonce`
  - [ ] `exhausted() is bool`
- [ ] object `NonceSequenceExhausted` does `Error`
  - [ ] `text() is string`
- [ ] Require every AEAD nonce to contain exactly 12 bytes.
- [ ] Require a four-byte sequence prefix.
- [ ] Require the initial counter to fit unsigned 64-bit range.
- [ ] Construct nonce bytes from prefix followed by a big-endian counter.
- [ ] Advance the sequence exactly once per successful `nonce()`.
- [ ] Throw `NonceSequenceExhausted` after the maximum counter.
- [ ] Never wrap a nonce counter.
- [ ] Document that a prefix must be unique for every key and sequence.
- [ ] Do not provide a random-nonce default that pretends to guarantee uniqueness.

## `crypto/aead`

- [ ] object `AuthenticationFailed` does `Error`
  - [ ] `text() is string`
- [ ] Use the same authentication failure for wrong key, nonce, AAD, ciphertext and tag.
- [ ] Do not reveal which authenticated input was incorrect.
- [ ] Authenticate before exposing plaintext.
- [ ] Keep AEAD operations bounded to in-memory byte arrays.
- [ ] Do not add unauthenticated streaming plaintext output.

## `crypto/aes-gcm`

- [ ] object `AesGcmKey`
  - [ ] `AesGcmKey(origin is SecretBytes)`
- [ ] object `GeneratedAesGcmKey`
  - [ ] `GeneratedAesGcmKey(random is SecureRandom, size is int)`
  - [ ] `result() is AesGcmKey`
- [ ] object `AesGcmSealed`
  - [ ] `AesGcmSealed(nonce is AeadNonce, ciphertext is @byte, tag is @byte)`
  - [ ] `nonce() is AeadNonce`
  - [ ] `ciphertext() is @byte`
  - [ ] `tag() is @byte`
- [ ] object `AesGcmEncryption`
  - [ ] `AesGcmEncryption(key is AesGcmKey, nonce is AeadNonce, plaintext is @byte, associated is @byte)`
  - [ ] `result() is AesGcmSealed`
- [ ] object `AesGcmDecryption`
  - [ ] `AesGcmDecryption(key is AesGcmKey, sealed is AesGcmSealed, associated is @byte)`
  - [ ] `result() is @byte`
- [ ] Accept AES-GCM keys of 16, 24 or 32 bytes only.
- [ ] Generate AES-GCM keys of 16, 24 or 32 bytes only.
- [ ] Require a full 16-byte authentication tag.
- [ ] Reject truncated GCM tags.
- [ ] Permit empty plaintext and associated data.
- [ ] Add NIST AES-GCM vectors for every supported key size.
- [ ] Test ciphertext, nonce, AAD and tag corruption independently.
- [ ] Document catastrophic nonce reuse under the same key.

## `crypto/chacha20-poly1305`

- [ ] object `ChaCha20Poly1305Key`
  - [ ] `ChaCha20Poly1305Key(origin is SecretBytes)`
- [ ] object `GeneratedChaCha20Poly1305Key`
  - [ ] `GeneratedChaCha20Poly1305Key(random is SecureRandom)`
  - [ ] `result() is ChaCha20Poly1305Key`
- [ ] object `ChaCha20Poly1305Sealed`
  - [ ] `ChaCha20Poly1305Sealed(nonce is AeadNonce, ciphertext is @byte, tag is @byte)`
  - [ ] `nonce() is AeadNonce`
  - [ ] `ciphertext() is @byte`
  - [ ] `tag() is @byte`
- [ ] object `ChaCha20Poly1305Encryption`
  - [ ] `ChaCha20Poly1305Encryption(key is ChaCha20Poly1305Key, nonce is AeadNonce, plaintext is @byte, associated is @byte)`
  - [ ] `result() is ChaCha20Poly1305Sealed`
- [ ] object `ChaCha20Poly1305Decryption`
  - [ ] `ChaCha20Poly1305Decryption(key is ChaCha20Poly1305Key, sealed is ChaCha20Poly1305Sealed, associated is @byte)`
  - [ ] `result() is @byte`
- [ ] Require exactly 32 key bytes.
- [ ] Require a full 16-byte authentication tag.
- [ ] Permit empty plaintext and associated data.
- [ ] Implement the RFC 8439 construction.
- [ ] Add every RFC 8439 AEAD vector.
- [ ] Test ciphertext, nonce, AAD and tag corruption independently.
- [ ] Document catastrophic nonce reuse under the same key.
- [ ] Do not expose unauthenticated raw ChaCha20.

## `crypto/key`

- [ ] role `PrivateKey`
  - [ ] Keep this role action-free as a deliberate marker.
- [ ] role `PublicKey`
  - [ ] Keep this role action-free as a deliberate marker.
- [ ] Keep private key material unavailable through ordinary observations.
- [ ] Permit public keys to expose standardized public encodings.
- [ ] Keep noun role names because these are domain categories.

## `crypto/ed25519`

- [ ] object `Ed25519PrivateKey` does `PrivateKey`
  - [ ] `Ed25519PrivateKey(seed is SecretBytes)`
  - [ ] `public_key() is Ed25519PublicKey`
- [ ] object `Ed25519PublicKey` does `PublicKey`
  - [ ] `Ed25519PublicKey(origin is @byte)`
  - [ ] `bytes() is @byte`
- [ ] object `GeneratedEd25519Key`
  - [ ] `GeneratedEd25519Key(random is SecureRandom)`
  - [ ] `result() is Ed25519PrivateKey`
- [ ] object `Ed25519Signature`
  - [ ] `Ed25519Signature(origin is @byte)`
  - [ ] `bytes() is @byte`
- [ ] object `Ed25519Signed`
  - [ ] `Ed25519Signed(key is Ed25519PrivateKey, message is @byte)`
  - [ ] `result() is Ed25519Signature`
- [ ] object `VerifiedEd25519`
  - [ ] `VerifiedEd25519(key is Ed25519PublicKey, message is @byte, signature is Ed25519Signature)`
  - [ ] `#projected value() is bool`
- [ ] object `InvalidEd25519Key` does `Error`
  - [ ] `text() is string`
- [ ] object `InvalidEd25519Signature` does `Error`
  - [ ] `text() is string`
- [ ] Require a 32-byte Ed25519 seed.
- [ ] Require a 32-byte encoded public key.
- [ ] Require a 64-byte encoded signature.
- [ ] Validate imported public points.
- [ ] Reject non-canonical signatures.
- [ ] Add RFC 8032 generation, signing and verification vectors.
- [ ] Return false for a well-formed but incorrect signature.
- [ ] Keep signature parsing distinct from signing.

## `crypto/x25519`

- [ ] object `X25519PrivateKey` does `PrivateKey`
  - [ ] `X25519PrivateKey(secret is SecretBytes)`
  - [ ] `public_key() is X25519PublicKey`
- [ ] object `X25519PublicKey` does `PublicKey`
  - [ ] `X25519PublicKey(origin is @byte)`
  - [ ] `bytes() is @byte`
- [ ] object `GeneratedX25519Key`
  - [ ] `GeneratedX25519Key(random is SecureRandom)`
  - [ ] `result() is X25519PrivateKey`
- [ ] object `X25519SharedSecret`
  - [ ] `X25519SharedSecret(private is X25519PrivateKey, public is X25519PublicKey)`
  - [ ] `secret() is SecretBytes`
- [ ] object `InvalidX25519Key` does `Error`
  - [ ] `text() is string`
- [ ] object `InvalidX25519SharedSecret` does `Error`
  - [ ] `text() is string`
- [ ] Require 32-byte private and public inputs.
- [ ] Validate imported public keys.
- [ ] Reject an all-zero shared secret.
- [ ] Add RFC 7748 key agreement vectors.
- [ ] Require HKDF before using a shared secret as an application key.
- [ ] Do not expose a generic unspecified ECDH key.

## `crypto/ecdsa`

- [ ] enum `EcdsaCurve`
  - [ ] `p256`
  - [ ] `p384`
  - [ ] `p521`
- [ ] object `EcdsaPrivateKey` does `PrivateKey`
  - [ ] `EcdsaPrivateKey(secret is SecretBytes, curve is EcdsaCurve)`
  - [ ] `curve() is EcdsaCurve`
  - [ ] `public_key() is EcdsaPublicKey`
- [ ] object `EcdsaPublicKey` does `PublicKey`
  - [ ] `EcdsaPublicKey(origin is @byte, curve is EcdsaCurve)`
  - [ ] `curve() is EcdsaCurve`
  - [ ] `bytes() is @byte`
- [ ] object `GeneratedEcdsaKey`
  - [ ] `GeneratedEcdsaKey(random is SecureRandom, curve is EcdsaCurve)`
  - [ ] `result() is EcdsaPrivateKey`
- [ ] object `EcdsaSignature`
  - [ ] `EcdsaSignature(r is BigInt, s is BigInt)`
  - [ ] `r() is BigInt`
  - [ ] `s() is BigInt`
- [ ] object `EcdsaSigned`
  - [ ] `EcdsaSigned(key is EcdsaPrivateKey, message is @byte)`
  - [ ] `result() is EcdsaSignature`
- [ ] object `VerifiedEcdsa`
  - [ ] `VerifiedEcdsa(key is EcdsaPublicKey, message is @byte, signature is EcdsaSignature)`
  - [ ] `#projected value() is bool`
- [ ] object `DerEcdsaSignature`
  - [ ] `DerEcdsaSignature(origin is EcdsaSignature)`
  - [ ] `bytes() is @byte`
- [ ] object `EcdsaSignatureFromDer`
  - [ ] `EcdsaSignatureFromDer(origin is @byte)`
  - [ ] `#projected value() is EcdsaSignature`
- [ ] object `InvalidEcdsaKey` does `Error`
  - [ ] `text() is string`
- [ ] object `InvalidEcdsaSignature` does `Error`
  - [ ] `text() is string`
- [ ] Map P-256 to SHA-256.
- [ ] Map P-384 to SHA-384.
- [ ] Map P-521 to SHA-512.
- [ ] Sign messages rather than caller-supplied digests.
- [ ] Generate signing nonces deterministically according to RFC 6979.
- [ ] Normalize generated signatures to low-S form.
- [ ] Reject out-of-range `r` and `s` values.
- [ ] Validate imported curve points.
- [ ] Require canonical DER integers and lengths.
- [ ] Add NIST and RFC 6979 vectors for every curve.
- [ ] Return false for a well-formed but incorrect signature.
- [ ] Do not add custom curves in the first API.

## `crypto/rsa`

- [ ] enum `RsaHash`
  - [ ] `sha256`
  - [ ] `sha384`
  - [ ] `sha512`
- [ ] object `RsaPrivateKey` does `PrivateKey`
  - [ ] Do not expose a raw component constructor.
  - [ ] `public_key() is RsaPublicKey`
  - [ ] `bits() is int`
- [ ] object `RsaPublicKey` does `PublicKey`
  - [ ] `RsaPublicKey(modulus is BigInt, exponent is int)`
  - [ ] `modulus() is BigInt`
  - [ ] `exponent() is int`
  - [ ] `bits() is int`
- [ ] object `GeneratedRsaKey`
  - [ ] `GeneratedRsaKey(random is SecureRandom)`
  - [ ] `GeneratedRsaKey(random is SecureRandom, bits is int)`
  - [ ] `result() is RsaPrivateKey`
- [ ] object `RsaPssSignature`
  - [ ] `RsaPssSignature(origin is @byte)`
  - [ ] `bytes() is @byte`
- [ ] object `RsaPssSigned`
  - [ ] `RsaPssSigned(key is RsaPrivateKey, message is @byte)`
  - [ ] `RsaPssSigned(key is RsaPrivateKey, message is @byte, hash is RsaHash)`
  - [ ] `result() is RsaPssSignature`
- [ ] object `VerifiedRsaPss`
  - [ ] `VerifiedRsaPss(key is RsaPublicKey, message is @byte, signature is RsaPssSignature)`
  - [ ] `VerifiedRsaPss(key is RsaPublicKey, message is @byte, signature is RsaPssSignature, hash is RsaHash)`
  - [ ] `#projected value() is bool`
- [ ] object `RsaOaepCiphertext`
  - [ ] `RsaOaepCiphertext(origin is @byte)`
  - [ ] `bytes() is @byte`
- [ ] object `RsaOaepEncrypted`
  - [ ] `RsaOaepEncrypted(key is RsaPublicKey, plaintext is SecretBytes, label is @byte, random is SecureRandom)`
  - [ ] `RsaOaepEncrypted(key is RsaPublicKey, plaintext is SecretBytes, label is @byte, random is SecureRandom, hash is RsaHash)`
  - [ ] `result() is RsaOaepCiphertext`
- [ ] object `RsaOaepDecrypted`
  - [ ] `RsaOaepDecrypted(key is RsaPrivateKey, ciphertext is RsaOaepCiphertext, label is @byte)`
  - [ ] `RsaOaepDecrypted(key is RsaPrivateKey, ciphertext is RsaOaepCiphertext, label is @byte, hash is RsaHash)`
  - [ ] `secret() is SecretBytes`
- [ ] object `InvalidRsaKey` does `Error`
  - [ ] `text() is string`
- [ ] object `InvalidRsaSignature` does `Error`
  - [ ] `text() is string`
- [ ] object `RsaDecryptionFailed` does `Error`
  - [ ] `text() is string`
- [ ] Generate 3072-bit RSA keys by default.
- [ ] Reject RSA keys smaller than 2048 bits.
- [ ] Require public exponent 65537 for generated keys.
- [ ] Validate imported modulus and exponent values.
- [ ] Use SHA-256 by default for PSS and OAEP.
- [ ] Use MGF1 with the selected hash.
- [ ] Use PSS salt length equal to digest length.
- [ ] Enforce the OAEP maximum plaintext size before encryption.
- [ ] Return one indistinguishable error for OAEP decryption failures.
- [ ] Document OAEP as small key-material transport only.
- [ ] Add RFC 8017 interoperability vectors.
- [ ] Do not expose raw RSA operations.
- [ ] Do not expose PKCS#1 v1.5 encryption.

## `crypto/key-format`

- [ ] object `UnencryptedPkcs8`
  - [ ] `UnencryptedPkcs8(key is PrivateKey)`
  - [ ] `bytes() is @byte`
- [ ] object `EncryptedPkcs8`
  - [ ] `EncryptedPkcs8(key is PrivateKey, password is SecretBytes, random is SecureRandom)`
  - [ ] `result() is @byte`
- [ ] object `ParsedPrivateKey`
  - [ ] `ParsedPrivateKey(origin is @byte)`
  - [ ] `#projected value() is PrivateKey`
- [ ] object `DecryptedPrivateKey`
  - [ ] `DecryptedPrivateKey(origin is @byte, password is SecretBytes)`
  - [ ] `#projected value() is PrivateKey`
- [ ] object `SubjectPublicKeyInfo`
  - [ ] `SubjectPublicKeyInfo(key is PublicKey)`
  - [ ] `bytes() is @byte`
- [ ] object `ParsedPublicKey`
  - [ ] `ParsedPublicKey(origin is @byte)`
  - [ ] `#projected value() is PublicKey`
- [ ] object `InvalidKeyEncoding` does `Error`
  - [ ] `text() is string`
- [ ] Export private keys only through PKCS#8.
- [ ] Make unencrypted private-key export explicit in the object name.
- [ ] Encrypt PKCS#8 through PBES2 with PBKDF2-HMAC-SHA256 and AES-256-GCM.
- [ ] Generate fresh encryption salt and nonce through `SecureRandom`.
- [ ] Reject unsupported key algorithms.
- [ ] Reject trailing DER data.
- [ ] Require canonical DER encoding.
- [ ] Compose PEM through `PemEncoded` and `PemBlocks`.
- [ ] Keep the DER codec internal and schema-specific.
- [ ] Do not expose a generic public ASN.1 value model.

## `crypto/x509/name`

- [ ] object `NameAttribute`
  - [ ] `NameAttribute(identifier is string, value is string)`
  - [ ] `identifier() is string`
  - [ ] `value() is string`
- [ ] object `DistinguishedName`
  - [ ] `DistinguishedName(attributes is Sequencing<NameAttribute>)`
  - [ ] `attributes() is Sequencing<NameAttribute>`
- [ ] Preserve repeated name attributes.
- [ ] Preserve relative-distinguished-name ordering.
- [ ] Represent attribute identifiers as dotted object identifiers.
- [ ] Do not flatten names into a lossy display string.

## `crypto/x509/certificate`

- [ ] enum `CertificateKeyUsage`
  - [ ] `digital_signature`
  - [ ] `content_commitment`
  - [ ] `key_encipherment`
  - [ ] `data_encipherment`
  - [ ] `key_agreement`
  - [ ] `certificate_signing`
  - [ ] `crl_signing`
  - [ ] `encipher_only`
  - [ ] `decipher_only`
- [ ] enum `CertificatePurpose`
  - [ ] `server_authentication`
  - [ ] `client_authentication`
  - [ ] `code_signing`
  - [ ] `email_protection`
  - [ ] `time_stamping`
  - [ ] `ocsp_signing`
  - [ ] `any`
- [ ] object `CertificateValidity`
  - [ ] `CertificateValidity(starts is Instant, ends is Instant)`
  - [ ] `starts() is Instant`
  - [ ] `ends() is Instant`
- [ ] object `Certificate`
  - [ ] Do not expose a public constructor.
  - [ ] `der() is @byte`
  - [ ] `subject() is DistinguishedName`
  - [ ] `issuer() is DistinguishedName`
  - [ ] `validity() is CertificateValidity`
  - [ ] `public_key() is PublicKey`
- [ ] object `ParsedCertificate`
  - [ ] `ParsedCertificate(origin is @byte)`
  - [ ] `#projected value() is Certificate`
- [ ] object `CertificateNames`
  - [ ] `CertificateNames(origin is Certificate)`
  - [ ] `dns() is Sequencing<string>`
  - [ ] `ips() is Sequencing<string>`
- [ ] object `CertificateConstraints`
  - [ ] `CertificateConstraints(authority is bool, limited is bool, maximum_depth is int)`
  - [ ] `authority() is bool`
  - [ ] `limited() is bool`
  - [ ] `maximum_depth() is int`
- [ ] object `MissingCertificateDepth` does `Error`
  - [ ] `text() is string`
- [ ] object `CertificateExtensions`
  - [ ] `CertificateExtensions(origin is Certificate)`
  - [ ] `constraints() is CertificateConstraints`
  - [ ] `key_usages() is Set<CertificateKeyUsage>`
  - [ ] `purposes() is Set<CertificatePurpose>`
  - [ ] `critical() is Sequencing<string>`
- [ ] object `CertificateChain`
  - [ ] `CertificateChain(certificates is Sequencing<Certificate>)`
  - [ ] `certificates() is Sequencing<Certificate>`
- [ ] object `InvalidCertificate` does `Error`
  - [ ] `text() is string`
- [ ] Make `maximum_depth()` throw `MissingCertificateDepth` when `limited() == false`.
- [ ] Reject an empty certificate chain.
- [ ] Reject trailing DER data.
- [ ] Require canonical DER encoding.
- [ ] Preserve unknown extensions by identifier.
- [ ] Reject duplicate extensions forbidden by X.509.
- [ ] Keep DNS names separate from IP address names.
- [ ] Do not fall back to common name when subjectAltName is present.
- [ ] Do not add `Certificate.valid()` or `Certificate.matches()`.

## `crypto/x509/trust`

- [ ] role `TrustStore`
  - [ ] `certificates() is Sequencing<Certificate>`
- [ ] object `SystemTrustStore`
  - [ ] `result() is TrustStore`
- [ ] object `PemTrustStore` does `TrustStore`
  - [ ] `PemTrustStore(origin is string)`
  - [ ] `certificates() is Sequencing<Certificate>`
- [ ] enum `CertificateFailure`
  - [ ] `not_yet_valid`
  - [ ] `expired`
  - [ ] `unknown_issuer`
  - [ ] `invalid_signature`
  - [ ] `name_mismatch`
  - [ ] `invalid_usage`
  - [ ] `invalid_constraints`
  - [ ] `unsupported_critical_extension`
- [ ] object `VerifiedCertificateChain`
  - [ ] `VerifiedCertificateChain(chain is CertificateChain, trust is TrustStore, host is string, at is Instant, purpose is CertificatePurpose)`
  - [ ] `result() is Certificate`
- [ ] object `CertificateVerificationFailed` does `Error`
  - [ ] `CertificateVerificationFailed(reason is CertificateFailure)`
  - [ ] `reason() is CertificateFailure`
  - [ ] `text() is string`
- [ ] Read system roots only through `SystemTrustStore.result()`.
- [ ] Snapshot system roots into an immutable trust store.
- [ ] Build and verify a chain to a trusted root.
- [ ] Verify every certificate signature.
- [ ] Verify validity at the explicit `Instant`.
- [ ] Enforce basic constraints and maximum path depth.
- [ ] Enforce key usage and extended key usage.
- [ ] Reject unsupported critical extensions.
- [ ] Verify DNS names according to IDNA-aware hostname rules.
- [ ] Verify IP addresses only against IP subjectAltName entries.
- [ ] Keep hostname verification inside chain verification.
- [ ] Document that revocation checking is outside the first API.

## Stage 18 exclusions

- [ ] Do not duplicate Stage 09 secure randomness.
- [ ] Do not expose private key bytes through ordinary key actions.
- [ ] Do not promise complete secret zeroization in the managed runtime.
- [ ] Do not add AES-ECB, AES-CBC, DES, 3DES or RC4 encryption APIs.
- [ ] Do not expose raw ChaCha20 without authentication.
- [ ] Do not support truncated AES-GCM tags.
- [ ] Do not expose unauthenticated streaming AEAD plaintext.
- [ ] Do not add a generic unspecified ECDH API.
- [ ] Do not add custom elliptic curves in the first API.
- [ ] Do not expose raw RSA operations.
- [ ] Do not expose PKCS#1 v1.5 RSA encryption.
- [ ] Do not add a generic public ASN.1 tree.
- [ ] Do not add `Certificate.valid()` or `Certificate.matches()`.
- [ ] Move TLS after TCP into Stage 20.

---

# Stage 19 — async and concurrency

## `builtin/async`

- [x] builtin object `Future<T>`
  - [x] Do not expose a public constructor.
  - [x] Calling an async action returns `Future<T>`.
  - [x] Starting a future is idempotent.
  - [x] Cache one successful result or typed failure.
  - [x] `cancel() is void`
  - [x] `done() is bool`
  - [x] `cancelled() is bool`
- [x] builtin object `Cancelled` does `Error`
  - [x] `text() is string`
- [x] `await Future<T>` produces `T` inside an async action.
- [x] A pending `await` suspends only the current async action.
- [x] Resume the continuation at the exact suspended expression.
- [x] Preserve locals, rebinding, typed catches, bare rethrow and `defer` across suspension.
- [x] Preserve left-to-right and lazy expression evaluation across suspension.
- [x] Support first-class bound action types written as `async (...) is T`.
- [x] Make `spawn` of an async action eagerly start and return the exact `Future<T>` produced by that action.
- [x] Do not wrap an async action's future in another language `Future`.
- [x] Make `spawn` of a synchronous action execute through an isolated worker process.
- [x] Keep cooperative async work and CPU-parallel process work as distinct execution modes.
- [x] Validate values crossing the process boundary.
- [x] Allow the callee of `spawn` to be any type-correct bound action value, not only direct `Worker(...).action(...)` syntax.
- [x] Require the receiver and arguments of a synchronously spawned bound action to be transferable through the worker protocol.
- [x] Reject an untransferable synchronous spawn before starting a worker.
- [x] Make `Future.cancel()` idempotently request cancellation.
- [x] Make cancelling an unstarted future settle it without invoking its starter.
- [x] Make cancelling an already settled future leave its result unchanged.
- [x] Keep `done()` false while cancellation cleanup is still running.
- [x] Make `done()` true after success, failure or completed cancellation.
- [x] Make `cancelled()` true only after cancellation has finished unwinding.
- [x] Make completed cancellation cause `await` to throw builtin `Cancelled`.
- [x] Make every suspending runtime operation a cancellation point.
- [x] Make cancelling an awaiting action stop that await without implicitly cancelling the independently owned future being awaited.
- [x] Remove cancelled channel senders, receivers and runtime waiters from their waiting queues exactly once.
- [x] Run every registered `defer` exactly once during cancellation unwinding.
- [x] Do not inject the same cancellation repeatedly while its `defer` cleanup is running.
- [x] Settle the future only after its cancellation cleanup completes.
- [x] Define one implicit async root for each top-level execution.
- [x] Register every eagerly spawned future with the current async root.
- [x] Do not register merely constructed lazy futures with an async root.
- [x] Mark a failed future observed when an `await` observes its completion.
- [x] On root shutdown, request cancellation of every unfinished owned future.
- [x] Wait for owned cancellation cleanup before the async root exits.
- [x] Report every unobserved non-cancellation failure before the root exits.
- [x] Do not report expected `Cancelled` completion as an unobserved failure.
- [x] Do not add a second `Task<T>` abstraction over `Future<T>`.
- [x] Do not add `Future.result()`; `await` is the only result extraction operation.
- [x] Do not add a public `FutureState` inspection object.
- [x] Do not imply OS-thread parallelism for ordinary async actions.

## `async/cancellation`

- [x] role `Cancelling`
  - [x] `requested() is bool`
  - [x] `check() is void`
  - [x] `async wait() is Cancelling`
- [x] object `Cancellation`
  - [x] `Cancellation()`
  - [x] `cancelling() is Cancelling`
  - [x] `cancel() is void`
- [x] object `CancellationToken` does `Cancelling`
  - [x] Do not expose a useful public constructor.
  - [x] `requested() is bool`
  - [x] `check() is void`
  - [x] `async wait() is Cancelling`
- [x] Make `Cancellation` the request-owning capability and
  `CancellationToken` the restricted observing capability.
- [x] Share one runtime-local state between an owner and every token obtained
  from it.
- [x] Use the builtin `Cancelled` error from `builtin/async`.
- [x] Make `Cancellation.cancel()` idempotent.
- [x] Make `Cancelling.check()` throw `Cancelled` after a request.
- [x] Make `Cancelling.wait()` finish only after a request.
- [x] Keep `wait()` lazy until its returned future is started by `await` or
  `spawn`.
- [x] Make a wait started after cancellation complete immediately.
- [x] Resume every waiter exactly once when cancellation is requested.
- [x] Remove a cancelled wait future from the live waiter set exactly once.
- [x] Make cancelling a wait future abandon only that wait, without requesting
  cancellation or affecting other waiters.
- [x] Let CPU-bound cooperative code call `check()` explicitly.
- [x] Preserve the original failure when cleanup also observes cancellation.
- [x] Keep explicit token cancellation independent from `Future.cancel()`;
  cooperating code must call `check()` or await `wait()`.
- [x] Do not use a process-global cancellation flag.
- [x] Document capability ownership, CPU-bound checks and asynchronous waiting
  with practical examples in `std/async/cancellation.ok`.

## `async/channel`

- [x] object `Channel<T>`
  - [x] `Channel(capacity is int)`
  - [x] `sending() is Sending<T>`
  - [x] `receiving() is Receiving<T>`
- [x] object `Sending<T>`
  - [x] Do not expose a useful public constructor.
  - [x] `async send(value is T) is Sending<T>`
  - [x] `close() is void`
- [x] object `Receiving<T>`
  - [x] Do not expose a useful public constructor.
  - [x] `async item() is T`
  - [x] `close() is void`
- [x] object `ChannelClosed` does `Error`
  - [x] `text() is string`
- [x] object `InvalidChannelCapacity` does `Error`
  - [x] `InvalidChannelCapacity(capacity is int)`
  - [x] `capacity() is int`
  - [x] `text() is string`
- [x] Keep `Channel<T>`, `Sending<T>` and `Receiving<T>` invariant.
- [x] Bound every channel and reject only negative capacity.
- [x] Preserve FIFO buffered delivery.
- [x] Preserve FIFO order among blocked senders.
- [x] Preserve FIFO order among waiting receivers.
- [x] Deliver every message to exactly one receiver.
- [x] Hand a message directly to an already waiting receiver.
- [x] Suspend a sender while the buffer is full.
- [x] Make enqueue, dequeue and waiter management amortized O(1).
- [x] Allow either endpoint to close the whole channel idempotently.
- [x] Fail new and blocked sends with `ChannelClosed` after close.
- [x] Drain buffered messages before receives start failing with `ChannelClosed`.
- [x] Permit multiple sender and receiver capabilities for one channel.
- [x] Accept capacity zero as an unbuffered rendezvous channel.
- [x] Remove a blocked sender from its waiter queue when its future is cancelled.
- [x] Remove a blocked receiver from its waiter queue when its future is cancelled.
- [x] Prove that cancellation cannot deliver or consume a message twice.
- [x] Define channel-local FIFO fairness: once an operation enters a waiter
  queue, a later operation of the same kind cannot bypass it.
- [x] Leave scheduling fairness between runnable continuations to the async
  runtime; do not claim a bounded scheduler guarantee from the channel.
- [x] Add cancellation races for send, receive, close and direct handoff.
- [x] Do not add polling `try_send()` or `try_receive()` in the first API.
- [x] Do not add race-prone `closed()` observations to endpoints.
- [x] Do not automatically close a channel when sending capabilities disappear.
- [x] Do not couple a task failure to unrelated channels implicitly.
- [x] Keep `send()` and `item()` lazy until their returned futures are started
  by `await` or `spawn`.
- [x] Document buffered use, rendezvous, explicit closing, fairness and
  cancellation in `std/async/channel.ok`.

## `async/scope`

- [x] role `Supervising<T>`
  - [x] `start(origin is Future<T>) is void`
  - [x] `active() is int`
  - [x] `cancel() is void`
- [x] object `FutureGroup<T>` does `Supervising<T>`
  - [x] `FutureGroup()`
  - [x] `start(origin is Future<T>) is void`
  - [x] `active() is int`
  - [x] `cancel() is void`
  - [x] `async wait() is FutureGroup<T>`
- [x] object `FutureScope<T,R>`
  - [x] `FutureScope(action is async (Supervising<T>) is R)`
  - [x] `async result() is R`
- [x] object `FinishedFutureGroup` does `Error`
  - [x] `FinishedFutureGroup()`
  - [x] `text() is string`
- [x] Make `start()` a void manipulator that registers and eagerly starts its supplied future exactly once.
- [x] Let callers retain the future they supplied instead of returning it from `start()`.
- [x] Make repeated registration of the same `Future` identity idempotent.
- [x] Count a registered future once even when it was already started before registration.
- [x] Observe an already settled registered future immediately and include its failure normally.
- [x] Reject registration after the group begins finishing.
- [x] Make either `wait()` or `cancel()` seal the group against later registration.
- [x] Throw `FinishedFutureGroup` from `start()` after sealing.
- [x] Make repeated `cancel()` idempotent.
- [x] Make repeated `wait()` observe one memoized completion rather than starting another wait lifecycle.
- [x] Make `FutureGroup.wait()` observe every child completion.
- [x] Make an empty `FutureGroup.wait()` complete immediately.
- [x] Make `FutureGroup.cancel()` request cancellation for every unfinished child.
- [x] Keep cancelled children active until their cancellation cleanup settles.
- [x] Define `active()` as the number of registered children which have not settled.
- [x] Keep every registered child owned by the current async root.
- [x] On normal scope return, wait for all children before returning the body result.
- [x] On body failure, cancel unfinished children and wait for their cleanup.
- [x] On parent cancellation, cancel unfinished children and wait for their cleanup.
- [x] Preserve the body failure as primary when child cleanup also fails.
- [x] If the body succeeds, make the first child failure in registration order primary.
- [x] Attach every remaining child failure as a secondary runtime failure in registration order.
- [x] Extend the runtime failure envelope with ordered secondary failures instead of introducing a public failure value bag.
- [x] Let typed `catch` observe the unchanged primary error value.
- [x] Include every secondary failure in an uncaught diagnostic without replacing the primary failure.
- [x] Make `wait()` observe child failures so the async root does not report them again independently.
- [x] Invoke the action encapsulated by `FutureScope` exactly once.
- [x] Make repeated `result()` calls return the same memoized `Future<R>` identity.
- [x] Permit nested scopes.
- [x] Use nested homogeneous groups when child result types differ.
- [x] Do not let scope-owned children outlive `FutureScope.result()`.
- [x] Do not silently detach work from its async root.
- [x] Document direct group supervision, structured scopes, sealing,
  active-count semantics and deterministic failure ordering in
  `std/async/scope.ok`.

## `async/collect`

- [x] object `AllFutures<T>`
  - [x] `AllFutures(origins is Sequencing<Future<T>>)`
  - [x] `async results() is Sequencing<T>`
- [x] object `Raced<T>`
  - [x] `Raced(origins is Sequencing<Future<T>>)`
  - [x] `async result() is T`
- [x] object `FirstSuccessful<T>`
  - [x] `FirstSuccessful(origins is Sequencing<Future<T>>)`
  - [x] `async result() is T`
- [x] object `FutureCompletions<T>`
  - [x] `FutureCompletions(origins is Sequencing<Future<T>>)`
  - [x] `remaining_count() is int`
  - [x] `async completion() is Future<T>`
- [x] object `NoFutures` does `Error`
  - [x] `NoFutures()`
  - [x] `text() is string`
- [x] Start every input future exactly once before observing completions.
- [x] Count duplicate input identities as separate input positions.
- [x] Preserve input order in `AllFutures.results()`.
- [x] Return an empty sequence immediately from empty `AllFutures`.
- [x] On the first observed failure, cancel unfinished siblings and await cleanup.
- [x] Preserve the triggering failure as primary.
- [x] Attach sibling cleanup failures as ordered secondary failures.
- [x] Make `Raced` select the first settled future, whether successful or failed.
- [x] Cancel and await every race loser before settling `Raced.result()`.
- [x] Let `FirstSuccessful` ignore failures while a possible success remains.
- [x] When every input fails, preserve the first completed failure as primary.
- [x] Attach later failures in completion order as secondary failures.
- [x] Cancel and await unfinished inputs after the first success.
- [x] Do not hide loser cleanup failures behind a successful result.
- [x] Preserve completion order in `FutureCompletions`.
- [x] Make each input position appear exactly once in `FutureCompletions`.
- [x] Throw `NoFutures` for empty `Raced` and `FirstSuccessful`.
- [x] Throw `NoFutures` from `completion()` after exhaustion.
- [x] Memoize one Future identity for `AllFutures.results()`, `Raced.result()`
  and `FirstSuccessful.result()`.
- [x] Make each started `FutureCompletions.completion()` reserve exactly one
  completion position.
- [x] Propagate parent cancellation to unfinished inputs and await cleanup.
- [x] Do not make `FutureCompletions` a `Cursor`; `completion()` may suspend.
- [x] Do not add a public aggregate-failure value bag.
- [x] Document ordered results, selection semantics, completion streaming,
  empty input, cancellation, cleanup and memoized identity in
  `std/async/collect.ok`.

## `async/timeout`

- [x] object `Within<T>`
  - [x] `Within(origin is Future<T>, limit is Duration, clock is Timing)`
  - [x] `async result() is T`
- [x] object `Before<T>`
  - [x] `Before(origin is Future<T>, deadline is MonotonicInstant, clock is Timing)`
  - [x] `async result() is T`
- [x] object `TimedOut` does `Error`
  - [x] `TimedOut()`
  - [x] `text() is string`
- [x] Reject negative timeout durations with `InvalidArgument`.
- [x] Treat an already reached `Before` deadline as an immediate timeout.
- [x] Let an already completed future win without invoking `Timing.wait()`.
- [x] When both sides are already settled, prefer the origin deterministically.
- [x] Race the future only against monotonic time.
- [x] Derive `Before` waits from `Timing.monotonic_instant()` and
  `MonotonicInstant.minus()`.
- [x] Cancel the future when the deadline wins.
- [x] Await cancellation cleanup before throwing `TimedOut`.
- [x] Preserve an origin result or typed failure that wins the race.
- [x] Preserve a clock-provider failure instead of relabelling it as `TimedOut`.
- [x] Propagate cancellation of the timeout result to the origin and timer, then
  await their cleanup.
- [x] Memoize one Future identity for repeated `result()` calls.
- [x] Keep cleanup failures in the ordered runtime failure envelope.
- [x] Make timeout tests deterministic through an injected `Timing` object.
- [x] Do not derive deadlines from civil `Instant`.
- [x] Document relative limits, shared deadlines, cleanup, memoization and the
  monotonic-only contract in `std/async/timeout.ok`.

## `async/lock`

- [x] object `Lock`
  - [x] `Lock()`
  - [x] `async guard() is LockGuard`
  - [x] `locked() is bool`
- [x] object `LockGuard` does `Closing`
  - [x] Do not expose a public constructor.
  - [x] `closed() is bool`
  - [x] `close() is void`
- [x] Make each successful acquisition produce one unique ownership guard.
- [x] Keep a lock owned continuously while handing it to the next waiter.
- [x] Grant the lock to live waiters in FIFO order.
- [x] Make guard close idempotent.
- [x] Make a stale or already closed guard unable to release a later owner.
- [x] Remove a cancelled acquirer without consuming ownership.
- [x] Compact cancelled waiter storage while preserving amortized O(1) queue
  operations.
- [x] Make a guard usable with `defer guard.close()`.
- [x] Define `locked()` as a diagnostic observation rather than a synchronization
  precondition.
- [x] Document that the lock is not reentrant.
- [x] Do not expose ownerless `unlock()`.
- [x] Prefer state-owner tasks and channels when they express the domain directly.
- [x] Document lexical guarding, FIFO ordering, cancellation and the
  state-owner alternative in `std/async/lock.ok`.

## `async/semaphore`

- [x] object `Semaphore`
  - [x] `Semaphore(permits is int)`
  - [x] `async permit() is Permit`
  - [x] `permits() is int`
- [x] object `Permit` does `Closing`
  - [x] Do not expose a public constructor.
  - [x] `closed() is bool`
  - [x] `close() is void`
- [x] object `InvalidPermitCount` does `Error`
  - [x] `InvalidPermitCount(permits is int)`
  - [x] `permits() is int`
  - [x] `text() is string`
- [x] Reject non-positive initial permit counts.
- [x] Define `permits()` as the currently available count.
- [x] Treat `permits()` as a diagnostic observation rather than a
  synchronization precondition.
- [x] Grant permits to live waiters in FIFO order.
- [x] Hand a returned permit directly to the oldest waiter without exposing
  transient spare capacity.
- [x] Return a permit exactly once when its guard closes.
- [x] Make `Permit.close()` idempotent and prevent capacity inflation.
- [x] Remove a cancelled acquirer without consuming a permit.
- [x] Compact cancelled waiter storage while preserving amortized O(1) queue
  operations.
- [x] Do not expose an unbalanced public `release()`.
- [x] Recommend an unbuffered channel rather than a zero-permit semaphore.
- [x] Document bounded ownership, lexical cleanup, cancellation and available
  count semantics in `std/async/semaphore.ok`.

## `async/event`

- [x] object `Event`
  - [x] `Event()`
  - [x] `Event(ready is bool)`
  - [x] `async wait() is Event`
  - [x] `set() is void`
  - [x] `ready() is bool`
- [x] Default `Event()` to not ready.
- [x] Release all current waiters when the event is set.
- [x] Let future waiters pass while the event remains set.
- [x] Make repeated `set()` idempotent.
- [x] Construct a fresh event instead of resetting a signalled event.
- [x] Remove cancelled waiters promptly without signalling the event or
  disturbing other waiters.
- [x] Compact cancelled waiter storage while preserving amortized O(1) queue
  operations.
- [x] Keep `wait()` lazy until its Future is started.
- [x] Keep event signalling level-triggered rather than message-counting.
- [x] Recommend `Channel` when each signal must be consumed exactly once.
- [x] Document broadcast, initial readiness, cancellation and the absence of
  reset semantics in `std/async/event.ok`.

## `async/barrier`

- [x] object `Barrier`
  - [x] `Barrier(parties is int)`
  - [x] `async wait() is int`
  - [x] `state() is BarrierState`
  - [x] `abort() is void`
  - [x] `reset() is void`
- [x] object `BarrierState`
  - [x] Do not expose a public constructor.
  - [x] `parties() is int`
  - [x] `waiters() is int`
  - [x] `generation() is int`
  - [x] `broken() is bool`
- [x] object `InvalidPartyCount` does `Error`
  - [x] `InvalidPartyCount(parties is int)`
  - [x] `parties() is int`
  - [x] `text() is string`
- [x] object `BrokenBarrier` does `Error`
  - [x] `BrokenBarrier()`
  - [x] `text() is string`
- [x] Reject party counts below one.
- [x] Define `BarrierState` as an immutable point-in-time observation.
- [x] Release exactly one generation when all parties arrive.
- [x] Automatically begin the next generation after a successful release.
- [x] Return distinct arrival indexes `0..parties-1` in arrival order from
  `wait()`.
- [x] Break the current generation when a waiter is cancelled.
- [x] Keep the cancelled wait as `Cancelled` while its peers receive
  `BrokenBarrier`.
- [x] Wake every current waiter with `BrokenBarrier` after abort or break.
- [x] Make future waiters fail with `BrokenBarrier` while the barrier is broken.
- [x] Make repeated `abort()` idempotent.
- [x] Make `reset()` wake old waiters before opening and advancing to a fresh
  unbroken generation.
- [x] Never carry an old waiter into a reset generation.
- [x] Document arrival indexes, cyclic generations, snapshots, cancellation,
  abort and reset in `std/async/barrier.ok`.

## `parallel/map`

- [ ] object `ParallelMapped<T, R>`
  - [ ] `ParallelMapped(origins is Sequencing<T>, action is (T) is R)`
  - [ ] `ParallelMapped(origins is Sequencing<T>, action is (T) is R, workers is int)`
  - [ ] `async results() is Sequencing<R>`
- [ ] object `InvalidWorkerCount` does `Error`
  - [ ] `InvalidWorkerCount(workers is int)`
  - [ ] `workers() is int`
  - [ ] `text() is string`
- [ ] Execute actions through the existing isolated process-worker path.
- [ ] Preserve input order in results.
- [ ] Bound active workers.
- [ ] Stop assigning new inputs after the first failure.
- [ ] Terminate or cancel active workers after failure or parent cancellation.
- [ ] Await worker cleanup before returning or failing.
- [ ] Validate every argument, result and failure crossing the process boundary.
- [ ] Reject non-positive worker counts.
- [ ] Choose a runtime-informed default worker count.
- [ ] Do not add shared-memory threads in the first API.
- [ ] Do not call process work `async`; it is parallel work awaited asynchronously.

## Stage 19 verification already present

- [x] Document the complete language and standard-library async model in the
  beginner-oriented Russian [async guide](../async-guide.md).
- [x] Cover async typing and await context in `t/interpreter/157-async-await-context.t`.
- [x] Cover suspension and continuation behavior in `t/interpreter/158-async-await-suspension.t`.
- [x] Cover spawn analysis in `t/interpreter/159-spawn-analysis.t`.
- [x] Cover public channel behavior in `t/interpreter/160-stdlib-async-channel.t`.
- [x] Cover state-owner composition in `t/interpreter/161-channel-state-owner.t`.
- [x] Cover async block adaptation in `t/interpreter/162-async-block-adapter.t`.
- [x] Run channel examples in `t/interpreter/163-channel-examples.t`.
- [x] Cover runtime channels, lifecycle, continuation and growth in `t/runtime/`.
- [x] Run the existing focused async suite: 12 files and 116 tests pass on Perl 5.42.2.

## Stage 19 exclusions

- [x] Do not introduce `Task<T>` beside builtin `Future<T>`.
- [x] Keep the runtime scheduler and event loop private to the VM; expose only
  suspending actions and `Future<T>`.
- [x] Do not expose thread identity to ordinary async code.
- [x] Do not add mutable shared collections as a concurrency primitive.
- [x] Express shared mutable state through one owning task and typed channels,
  or guard an unavoidable shared resource explicitly.
- [x] Do not infer structured ownership from garbage collection.
- [x] Make async roots, scopes, groups, cancellation and endpoint closing
  explicit program actions.
- [x] Do not make channel endpoint destruction observable.
- [x] Do not automatically close a channel when endpoint values become
  unreachable.
- [x] Do not promise strict global scheduling order.
- [x] Promise FIFO only inside the documented waiter queue of an individual
  channel, lock, semaphore or barrier generation.
- [x] Defer heterogeneous channel `select` until the language has a sound typed
  result shape.
- [x] Keep networking transports in Stage 20 and expose their suspension through
  the same Stage 19 Future model.

---

# Stage 20 — networking and TLS

## `runtime/network` substrate

- [x] Parse and format numeric IPv4 addresses natively.
- [x] Parse and format numeric IPv6 addresses natively.
- [x] Encode portable IP endpoint envelopes across the native boundary.
- [x] Resolve forward host names through the system resolver.
- [x] Resolve reverse host names through the system resolver.
- [x] Create, bind, listen, accept, connect and close native IP sockets.
- [x] Send and receive TCP byte streams.
- [x] Send and receive addressed UDP datagrams.
- [x] Read local and peer socket addresses.
- [x] Shut down TCP input and output independently.
- [x] Read and change the currently supported boolean socket options.
- [x] Register file, pipe and socket readiness in one native poller.
- [x] Wait on monotonic absolute poll deadlines.
- [x] Cancel one pending native poll wait.
- [x] Create client and server TLS sessions from connected TCP handles.
- [x] Perform native TLS handshake, read, write and close operations.
- [x] Project the peer certificate as DER.
- [ ] Put every async-managed socket into non-blocking mode.
- [ ] Integrate one native poller with the cooperative async scheduler.
- [ ] Suspend `accept`, `connect`, `read`, `write`, `send` and `receive` on readiness.
- [ ] Resume only the continuation registered for the ready operation.
- [ ] Remove cancelled I/O registrations before resuming another waiter.
- [ ] Make close wake every operation pending on the same resource.
- [ ] Run system DNS resolution without blocking the cooperative scheduler.
- [ ] Drive TLS `want_read` and `want_write` through scheduler readiness.
- [ ] Preserve one socket handle while wrapping TCP in TLS.
- [ ] Add native Unix-domain stream sockets on supported platforms.
- [ ] Add IPv6 scope identifiers to native endpoint envelopes.
- [ ] Add UDP truncation reporting instead of silently losing that fact.
- [ ] Add portable multicast membership operations.
- [ ] Keep raw handles, numeric families and poll registration tags internal.

## `net/failure`

- [x] enum `NetworkOperation`
  - [x] `resolving`
  - [x] `binding`
  - [x] `listening`
  - [x] `accepting`
  - [x] `connecting`
  - [x] `reading`
  - [x] `writing`
  - [x] `shutting_down`
  - [x] `closing`
- [x] enum `NetworkFailureKind`
  - [x] `refused`
  - [x] `reset`
  - [x] `aborted`
  - [x] `unreachable`
  - [x] `timed_out`
  - [x] `address_in_use`
  - [x] `permission_denied`
  - [x] `resource_exhausted`
  - [x] `unsupported`
  - [x] `other`
- [x] object `NetworkFailure` does `Error`
  - [x] `NetworkFailure(operation is NetworkOperation, kind is NetworkFailureKind, message is string)`
  - [ ] `operation() is NetworkOperation`
  - [ ] `kind() is NetworkFailureKind`
  - [x] `text() is string`
- [x] object `NetworkClosed` does `Error`
  - [x] `text() is string`
- [ ] Translate stable native failure categories into `NetworkFailureKind`.
- [ ] Preserve the attempted operation separately from the failure kind.
- [ ] Keep human-readable native detail in `text()`.
- [ ] Do not expose platform `errno`, Winsock codes or TLS-library codes.
- [ ] Let Stage 19 cancellation remain `Cancelled`, not `NetworkFailure`.
- [ ] Let Stage 19 timeout remain `TimedOut`, not a socket-specific timeout object.

## `net/ip`

- [x] enum `IpVersion`
  - [x] `v4`
  - [x] `v6`
- [x] enum `IpProperty`
  - [x] `unspecified`
  - [x] `loopback`
  - [x] `link_local`
  - [x] `private_network`
  - [x] `global`
  - [x] `multicast`
  - [x] `reserved`
  - [x] `documentation`
- [ ] object `IpAddress` does `ByteSource, Comparing<IpAddress>`
  - [ ] `IpAddress(origin is string)`
  - [ ] `version() is IpVersion`
  - [ ] `bytes() is @byte`
  - [ ] `properties() is Set<IpProperty>`
  - [ ] `string() is string`
  - [ ] `order(other is IpAddress) is Order`
- [ ] object `IpNetwork`
  - [ ] `IpNetwork(address is IpAddress, prefix is int)`
  - [ ] `network() is IpAddress`
  - [ ] `broadcast() is IpAddress`
  - [ ] `prefix() is int`
  - [ ] `membership(address is IpAddress) is bool`
  - [ ] `size() is BigInt`
- [ ] object `IpAddresses` does `Iterating<IpAddress>`
  - [ ] `IpAddresses(origin is IpNetwork)`
  - [ ] `cursor() is Cursor<IpAddress>`
- [ ] object `IpInterface`
  - [ ] `IpInterface(address is IpAddress, prefix is int)`
  - [ ] `address() is IpAddress`
  - [ ] `network() is IpNetwork`
  - [ ] `string() is string`
- [ ] object `InvalidIpAddress` does `Error`
  - [ ] `InvalidIpAddress(origin is string)`
  - [ ] `text() is string`
- [ ] object `InvalidIpPrefix` does `Error`
  - [ ] `InvalidIpPrefix(prefix is int, version is IpVersion)`
  - [ ] `prefix() is int`
  - [ ] `text() is string`
- [ ] Parse only numeric addresses; never perform DNS from `IpAddress`.
- [ ] Emit canonical compressed lowercase IPv6 text.
- [ ] Compare addresses first by version and then by unsigned packed bytes.
- [ ] Validate prefixes in `0..32` for IPv4 and `0..128` for IPv6.
- [ ] Mask host bits when constructing `IpNetwork`.
- [ ] Define `broadcast()` as the final address for both IP versions.
- [ ] Iterate network addresses lazily.
- [ ] Keep IPv6 cardinality in `BigInt`.
- [ ] Classify special-use ranges from versioned IANA data.
- [ ] Do not attach a port or IPv6 zone to `IpAddress`.

## `net/endpoint`

- [ ] object `HostName`
  - [ ] `HostName(origin is string)`
  - [ ] `ascii() is string`
  - [ ] `unicode() is string`
- [ ] object `HostEndpoint`
  - [ ] `HostEndpoint(host is HostName, port is int)`
  - [ ] `host() is HostName`
  - [ ] `port() is int`
  - [ ] `string() is string`
- [ ] object `IpEndpoint`
  - [ ] `IpEndpoint(address is IpAddress, port is int)`
  - [ ] `IpEndpoint(address is IpAddress, port is int, zone is string)`
  - [ ] `address() is IpAddress`
  - [ ] `port() is int`
  - [ ] `zone() is string`
  - [ ] `string() is string`
- [ ] object `InvalidHostName` does `Error`
  - [ ] `InvalidHostName(origin is string)`
  - [ ] `text() is string`
- [ ] object `InvalidPort` does `Error`
  - [ ] `InvalidPort(port is int)`
  - [ ] `port() is int`
  - [ ] `text() is string`
- [ ] object `InvalidIpv6Zone` does `Error`
  - [ ] `InvalidIpv6Zone(zone is string)`
  - [ ] `text() is string`
- [ ] Validate ports in `0..65535`.
- [ ] Permit port zero only where the consuming operation allows OS assignment.
- [ ] Normalize host names through IDNA before resolution.
- [ ] Reject empty labels, embedded NUL and overlong DNS names.
- [ ] Permit a zone only for IPv6 endpoints.
- [ ] Bracket IPv6 addresses when formatting an endpoint with a port.
- [ ] Keep service-name lookup outside the first API; ports remain integers.
- [ ] Do not create a one-method `Port` wrapper.

## `net/dns`

- [ ] role `Resolving`
  - [ ] `async addresses(origin is HostEndpoint) is Sequencing<IpEndpoint>`
  - [ ] `async name(origin is IpAddress) is HostName`
- [ ] object `SystemResolution` does `Resolving`
  - [ ] `SystemResolution()`
  - [ ] `async addresses(origin is HostEndpoint) is Sequencing<IpEndpoint>`
  - [ ] `async name(origin is IpAddress) is HostName`
- [ ] object `FixedResolution` does `Resolving`
  - [ ] `FixedResolution(addresses is %HostName:Sequencing<IpAddress>, names is %IpAddress:HostName)`
  - [ ] `FixedResolution(addresses is %HostName:Sequencing<IpAddress>)`
  - [ ] `async addresses(origin is HostEndpoint) is Sequencing<IpEndpoint>`
  - [ ] `async name(origin is IpAddress) is HostName`
- [ ] object `UnknownHost` does `Error`
  - [ ] `UnknownHost(origin is HostName)`
  - [ ] `host() is HostName`
  - [ ] `text() is string`
- [ ] object `UnknownAddress` does `Error`
  - [ ] `UnknownAddress(origin is IpAddress)`
  - [ ] `address() is IpAddress`
  - [ ] `text() is string`
- [ ] object `DnsFailure` does `Error`
  - [ ] `DnsFailure(message is string)`
  - [ ] `text() is string`
- [ ] Preserve resolver result order while removing exact duplicates.
- [ ] Attach the requested port to every forward result.
- [ ] Return both IPv6 and IPv4 results when available.
- [ ] Make resolution cancellable without blocking unrelated tasks.
- [ ] Compose DNS deadlines through `Within`, not resolver-specific timeout settings.
- [ ] Keep `FixedResolution` deterministic and free of native calls.
- [ ] Do not expose raw `getaddrinfo` flags or DNS wire records.
- [ ] Defer configurable recursive DNS clients to a separate future stage.

## `net/interface`

- [ ] object `NetworkInterface`
  - [ ] Do not expose a public constructor.
  - [ ] `name() is string`
  - [ ] `index() is int`
  - [ ] `addresses() is Sequencing<IpInterface>`
  - [ ] `up() is bool`
  - [ ] `loopback() is bool`
- [ ] object `SystemInterfaces`
  - [ ] `SystemInterfaces()`
  - [ ] `interfaces() is Sequencing<NetworkInterface>`
  - [ ] `interface(name is string) is NetworkInterface`
  - [ ] `interface(index is int) is NetworkInterface`
- [ ] object `MissingNetworkInterface` does `Error`
  - [ ] `text() is string`
- [ ] Snapshot interface observations for each `SystemInterfaces` call.
- [ ] Preserve stable system interface indexes.
- [ ] Include every assigned IPv4 and IPv6 prefix available portably.
- [ ] Do not expose mutable interface configuration in the portable stdlib.
- [ ] Keep routes, firewall state and platform flags in Stage 34.

## `net/stream`

- [ ] role `AsyncReading`
  - [ ] `async bytes(limit is int) is @byte`
- [ ] role `AsyncWriting`
  - [ ] `async write(source is ByteSource) is AsyncWriting`
- [ ] role `StreamReading` does `AsyncReading, Closing`
  - [ ] `async bytes(limit is int) is @byte`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] role `StreamWriting` does `AsyncWriting, Closing`
  - [ ] `async write(source is ByteSource) is StreamWriting`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] role `StreamConnection` does `Closing`
  - [ ] `input() is StreamReading`
  - [ ] `output() is StreamWriting`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] role `StreamAcceptance` does `Closing`
  - [ ] `async connection() is StreamConnection`
  - [ ] `local_endpoint() is IpEndpoint`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `ConnectionEndpoints<T>`
  - [ ] `ConnectionEndpoints(local is T, remote is T)`
  - [ ] `local_endpoint() is T`
  - [ ] `remote_endpoint() is T`
- [ ] Make empty `bytes()` output mean orderly end of stream.
- [ ] Reject non-positive read limits.
- [ ] Make `write()` all-or-failure even when native writes are partial.
- [ ] Preserve bytes already written when reporting a later failure only in internal diagnostics.
- [ ] Make closing a reading half perform input shutdown.
- [ ] Make closing a writing half perform output shutdown after queued writes finish.
- [ ] Make connection close close both halves idempotently.
- [ ] Serialize overlapping writes on one writing half.
- [ ] Reject overlapping reads on one reading half with `InvalidState`.
- [ ] Let cancellation stop waiting without corrupting the byte stream.
- [ ] Do not add `flush()` to unbuffered network streams.
- [ ] Do not make async streams satisfy synchronous Stage 07 `Reading` or `Writing`.

## `net/tcp`

- [ ] object `TcpOptions`
  - [ ] `TcpOptions()`
  - [ ] `TcpOptions(keep_alive is bool, no_delay is bool)`
  - [ ] `keep_alive() is bool`
  - [ ] `no_delay() is bool`
- [ ] object `TcpListeningOptions`
  - [ ] `TcpListeningOptions()`
  - [ ] `TcpListeningOptions(address_reuse is bool, dual_stack is bool, backlog is int)`
  - [ ] `address_reuse() is bool`
  - [ ] `dual_stack() is bool`
  - [ ] `backlog() is int`
- [ ] object `TcpConnecting`
  - [ ] `TcpConnecting(remote is HostEndpoint, resolver is Resolving, options is TcpOptions, clock is Timing)`
  - [ ] `TcpConnecting(remote is IpEndpoint, options is TcpOptions)`
  - [ ] `async connection() is TcpConnection`
- [ ] object `TcpListening`
  - [ ] `TcpListening(local is IpEndpoint, options is TcpListeningOptions)`
  - [ ] `async acceptance() is TcpAcceptance`
- [ ] object `TcpAcceptance` does `StreamAcceptance`
  - [ ] Do not expose a public constructor.
  - [ ] `async connection() is TcpConnection`
  - [ ] `local_endpoint() is IpEndpoint`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `TcpConnection` does `StreamConnection`
  - [ ] Do not expose a public constructor.
  - [ ] `input() is StreamReading`
  - [ ] `output() is StreamWriting`
  - [ ] `endpoints() is ConnectionEndpoints<IpEndpoint>`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] Resolve and attempt multiple addresses through a Happy Eyeballs strategy.
- [ ] Prefer neither IPv4 nor IPv6 unconditionally.
- [ ] Delay fallback attempts through injected monotonic `Timing`.
- [ ] Close every losing connection attempt.
- [ ] Make parent cancellation close every active attempt.
- [ ] Apply keepalive and packet-delay settings after connection establishment.
- [ ] Apply listener options before bind.
- [ ] Reject negative backlog and choose a conservative default.
- [ ] Report the OS-selected address after binding port zero.
- [ ] Keep accepted connections independent from listener lifecycle.
- [ ] Compose connection, read and write deadlines through `Within`.
- [ ] Do not preserve the old empty `Socket` role containing only `local()`.
- [ ] Do not expose raw `connect()`, `bind()` or `listen()` mutation on a socket object.
- [ ] Do not let one `TcpConnection` accumulate stream, endpoint and shutdown actions directly.

## `net/udp`

- [ ] object `Datagram` does `ByteSource`
  - [ ] `Datagram(source is IpEndpoint, bytes is @byte, truncated is bool)`
  - [ ] `source() is IpEndpoint`
  - [ ] `bytes() is @byte`
  - [ ] `truncated() is bool`
- [ ] object `UdpOptions`
  - [ ] `UdpOptions()`
  - [ ] `UdpOptions(address_reuse is bool, broadcast is bool, dual_stack is bool)`
  - [ ] `address_reuse() is bool`
  - [ ] `broadcast() is bool`
  - [ ] `dual_stack() is bool`
- [ ] object `UdpBinding`
  - [ ] `UdpBinding(local is IpEndpoint, options is UdpOptions)`
  - [ ] `async socket() is UdpSocket`
- [ ] object `UdpSocket` does `Closing`
  - [ ] Do not expose a public constructor.
  - [ ] `async send(target is IpEndpoint, source is ByteSource) is UdpSocket`
  - [ ] `async datagram(limit is int) is Datagram`
  - [ ] `local_endpoint() is IpEndpoint`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `UdpConnecting`
  - [ ] `UdpConnecting(origin is UdpSocket, remote is IpEndpoint)`
  - [ ] `async connection() is UdpConnection`
- [ ] object `UdpConnection` does `Closing`
  - [ ] Do not expose a public constructor.
  - [ ] `async send(source is ByteSource) is UdpConnection`
  - [ ] `async bytes(limit is int) is @byte`
  - [ ] `endpoints() is ConnectionEndpoints<IpEndpoint>`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `DatagramTooLarge` does `Error`
  - [ ] `DatagramTooLarge(size is int)`
  - [ ] `size() is int`
  - [ ] `text() is string`
- [ ] Preserve one datagram boundary per send and receive.
- [ ] Report the actual sender endpoint.
- [ ] Report payload truncation explicitly.
- [ ] Reject non-positive receive limits.
- [ ] Reject payloads larger than the platform and IP-version limit before sending.
- [ ] Make cancellation remove readiness without consuming a later datagram.
- [ ] Apply reuse, broadcast and dual-stack options before bind.
- [ ] Make connected UDP accept packets only from its fixed peer.
- [ ] Surface asynchronous ICMP failures when the platform reports them.
- [ ] Keep unconnected and connected UDP APIs separate.
- [ ] Do not model UDP as a byte stream.

## `net/multicast`

- [ ] object `MulticastOptions`
  - [ ] `MulticastOptions()`
  - [ ] `MulticastOptions(hop_limit is int, loopback is bool)`
  - [ ] `hop_limit() is int`
  - [ ] `loopback() is bool`
- [ ] object `MulticastJoining`
  - [ ] `MulticastJoining(socket is UdpSocket, group is IpAddress, interface is NetworkInterface, options is MulticastOptions)`
  - [ ] `membership() is MulticastMembership`
- [ ] object `MulticastMembership` does `Closing`
  - [ ] Do not expose a public constructor.
  - [ ] `group() is IpAddress`
  - [ ] `interface() is NetworkInterface`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `InvalidMulticastGroup` does `Error`
  - [ ] `InvalidMulticastGroup(origin is IpAddress)`
  - [ ] `text() is string`
- [ ] Accept only addresses classified as multicast.
- [ ] Validate hop limits in `0..255`.
- [ ] Support IPv4 and IPv6 memberships through one public shape.
- [ ] Leave the group exactly once when membership closes.
- [ ] Let closing the socket implicitly end all of its memberships.
- [ ] Keep source-specific multicast outside the first API.

## `net/unix`

- [ ] object `UnixAddress`
  - [ ] `UnixAddress(origin is Path)`
  - [ ] `path() is Path`
  - [ ] `string() is string`
- [ ] object `UnixConnecting`
  - [ ] `UnixConnecting(remote is UnixAddress)`
  - [ ] `async connection() is UnixConnection`
- [ ] object `UnixListening`
  - [ ] `UnixListening(local is UnixAddress, backlog is int)`
  - [ ] `async acceptance() is UnixAcceptance`
- [ ] object `UnixAcceptance` does `Closing`
  - [ ] Do not expose a public constructor.
  - [ ] `async connection() is UnixConnection`
  - [ ] `local_address() is UnixAddress`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `UnixConnection` does `Closing`
  - [ ] Do not expose a public constructor.
  - [ ] `input() is StreamReading`
  - [ ] `output() is StreamWriting`
  - [ ] `endpoints() is ConnectionEndpoints<UnixAddress>`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `UnixPaired`
  - [ ] `UnixPaired()`
  - [ ] `async result() is UnixPair`
- [ ] object `UnixPair`
  - [ ] Do not expose a public constructor.
  - [ ] `first() is UnixConnection`
  - [ ] `second() is UnixConnection`
- [ ] object `UnixSocketsUnsupported` does `Error`
  - [ ] `text() is string`
- [ ] Reject embedded NUL in filesystem socket paths.
- [ ] Reject paths longer than the platform limit before bind or connect.
- [ ] Remove a filesystem socket entry after listener close only when it still identifies that listener.
- [ ] Keep peer credentials in a separate platform-capability object.
- [ ] Put Linux abstract addresses and descriptor passing in Stage 34.
- [ ] Do not pretend the current always-unsupported wrapper is Unix socket support.

## `net/tls`

- [ ] enum `TlsVersion`
  - [ ] `tls12`
  - [ ] `tls13`
- [ ] enum `TlsPhase`
  - [ ] `configuration`
  - [ ] `handshake`
  - [ ] `reading`
  - [ ] `writing`
  - [ ] `closing`
- [ ] object `TlsIdentity`
  - [ ] `TlsIdentity(certificates is CertificateChain, key is PrivateKey)`
  - [ ] `certificates() is CertificateChain`
  - [ ] `key() is PrivateKey`
- [ ] object `TlsClientPolicy`
  - [ ] `TlsClientPolicy(trust is TrustStore, versions is Set<TlsVersion>, protocols is Sequencing<string>)`
  - [ ] `trust() is TrustStore`
  - [ ] `versions() is Set<TlsVersion>`
  - [ ] `protocols() is Sequencing<string>`
- [ ] object `TlsServerPolicy`
  - [ ] `TlsServerPolicy(identity is TlsIdentity, versions is Set<TlsVersion>, protocols is Sequencing<string>)`
  - [ ] `identity() is TlsIdentity`
  - [ ] `versions() is Set<TlsVersion>`
  - [ ] `protocols() is Sequencing<string>`
- [ ] object `TlsClientHandshake`
  - [ ] `TlsClientHandshake(origin is TcpConnection, server is HostName, policy is TlsClientPolicy)`
  - [ ] `TlsClientHandshake(origin is TcpConnection, server is HostName, policy is TlsClientPolicy, identity is TlsIdentity)`
  - [ ] `async connection() is TlsConnection`
- [ ] object `TlsServerHandshake`
  - [ ] `TlsServerHandshake(origin is TcpConnection, policy is TlsServerPolicy)`
  - [ ] `async connection() is TlsConnection`
- [ ] object `TlsAcceptance` does `StreamAcceptance`
  - [ ] `TlsAcceptance(origin is TcpAcceptance, policy is TlsServerPolicy)`
  - [ ] `async connection() is TlsConnection`
  - [ ] `local_endpoint() is IpEndpoint`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `TlsConnection` does `StreamConnection`
  - [ ] Do not expose a public constructor.
  - [ ] `input() is StreamReading`
  - [ ] `output() is StreamWriting`
  - [ ] `details() is TlsDetails`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `TlsDetails`
  - [ ] `endpoints() is ConnectionEndpoints<IpEndpoint>`
  - [ ] `peer() is CertificateChain`
  - [ ] `version() is TlsVersion`
  - [ ] `cipher() is string`
  - [ ] `protocol() is string`
- [ ] object `TlsFailure` does `Error`
  - [ ] `TlsFailure(phase is TlsPhase, message is string)`
  - [ ] `phase() is TlsPhase`
  - [ ] `text() is string`
- [ ] Require peer verification and hostname verification for every client handshake.
- [ ] Verify the `HostName` supplied to each client handshake independently from reusable policy.
- [ ] Do not provide a `verification: disabled` production policy.
- [ ] Permit explicit private trust stores for tests and private deployments.
- [ ] Support optional client identity without weakening server verification.
- [ ] Support only TLS 1.2 and TLS 1.3.
- [ ] Reject an empty enabled-version set.
- [ ] Validate certificate and private-key correspondence before listening.
- [ ] Negotiate application protocols in caller preference order.
- [ ] Return an empty negotiated protocol when neither side advertises ALPN.
- [ ] Verify certificates through Stage 18 trust and X.509 policy.
- [ ] Reuse Stage 18 `Certificate`, `CertificateChain`, `PrivateKey` and `TrustStore`.
- [ ] Make handshake, read, write and orderly shutdown cancellable.
- [ ] Close the consumed TCP connection after any failed handshake.
- [ ] Keep accepted TLS sessions independent from listener lifecycle.
- [ ] Add deterministic loopback tests with generated test certificates.
- [ ] Add interoperability tests against an external TLS implementation.
- [ ] Add server-name identity selection after the single-identity API is stable.
- [ ] Add required and optional mutual-TLS client verification after the single-trust API is stable.
- [ ] Do not duplicate Stage 18 certificate value objects.
- [ ] Do not expose TLS engine handles, cipher numeric identifiers or raw alert codes.
- [ ] Do not place TLS back into `crypto/tls`; it is a network transport using cryptography.

## Stage 20 verification already present

- [x] Cover native IP, DNS and loopback stream operations in `t/interpreter/48-native-network-and-dns.t`.
- [x] Cover poll registration, deadlines, cancellation and readiness in `t/interpreter/native/49-native-poll-eventing.t`.
- [x] Cover current DNS wrappers in `t/interpreter/105-stdlib-net-dns.t`.
- [x] Cover current socket values and failures in `t/interpreter/118-stdlib-socket-socket.t`.
- [x] Cover current TCP loopback behavior in `t/interpreter/127-stdlib-socket-tcp.t`.
- [x] Cover current UDP loopback behavior in `t/interpreter/128-stdlib-socket-udp.t`.
- [x] Cover the current explicitly unsupported Unix wrapper in `t/interpreter/129-stdlib-socket-unix.t`.
- [x] Cover current TLS policy and portable provider failure in `t/interpreter/130-stdlib-crypto-tls.t`.
- [x] Cover default-provider native networking in `t/suite/34-native-network-and-dns.t`.
- [x] Cover default-provider polling in `t/suite/35-native-poll-eventing.t`.
- [x] Run the available focused networking suite: 10 files and 42 tests pass on Perl 5.42.2.
- [ ] Run `t/interpreter/native/53-native-tls.t` with `IO::Socket::SSL` installed.
- [ ] Add async scheduler integration tests for every network suspension point.
- [ ] Add cancellation races for every pending network operation.
- [ ] Add IPv4, IPv6 and dual-stack loopback tests.
- [ ] Add resource-leak tests after success, failure, cancellation and close.

## Stage 20 exclusions

- [ ] Do not expose raw socket constructors or numeric address families.
- [ ] Do not expose the native poller as a public event loop.
- [ ] Do not duplicate timeouts already composed by Stage 19.
- [ ] Do not duplicate certificate and trust types from Stage 18.
- [ ] Do not make blocking network calls on the cooperative scheduler thread.
- [ ] Do not add shared-memory networking callbacks.
- [ ] Do not add SCTP, raw IP sockets or packet capture to the portable first API.
- [ ] Keep URL parsing, MIME and mail in Stage 21.
- [ ] Keep HTTP and related application protocols in Stage 22.
- [ ] Keep platform-specific socket capabilities in Stage 34.

---

# Stage 21 — URL, MIME and mail

## `runtime/message` substrate

- [x] Encode `string` as UTF-8 bytes natively.
- [x] Decode strict UTF-8 bytes natively.
- [x] Report the valid UTF-8 prefix length natively.
- [ ] Do not add native URL, MIME or mail value objects.
- [ ] Implement URL grammar and resolution in ordinary stdlib code.
- [ ] Implement MIME and Internet Message Format grammar in ordinary stdlib code.
- [ ] Build mail transports on Stage 20 async streams and TLS.
- [ ] Reuse Stage 06 Base64 and quoted-printable codecs.
- [ ] Reuse Stage 09 secure randomness for MIME boundaries and message identifiers.
- [ ] Reuse Stage 10 time values for message dates.
- [ ] Add a native bulk path only after profiling proves a specific parser bottleneck.

## `url/percent`

- [ ] enum `UrlComponent`
  - [ ] `userinfo`
  - [ ] `registered_name`
  - [ ] `path_segment`
  - [ ] `path`
  - [ ] `query`
  - [ ] `fragment`
- [ ] object `PercentEncoded`
  - [ ] `PercentEncoded(origin is string, component is UrlComponent)`
  - [ ] `string() is string`
- [ ] object `PercentDecoded`
  - [ ] `PercentDecoded(origin is string)`
  - [ ] `bytes() is @byte`
  - [ ] `string() is string`
- [ ] object `InvalidPercentEncoding` does `Error`
  - [ ] `InvalidPercentEncoding(offset is int)`
  - [ ] `offset() is int`
  - [ ] `text() is string`
- [ ] Encode the UTF-8 bytes of non-ASCII characters.
- [ ] Preserve only the unreserved and component-safe ASCII set.
- [ ] Emit uppercase hexadecimal digits.
- [ ] Decode percent triplets into bytes before decoding UTF-8.
- [ ] Make `PercentDecoded.string()` throw `InvalidUtf8` for non-text bytes.
- [ ] Preserve source offsets for malformed triplets.
- [ ] Do not treat `+` as space outside form encoding.
- [ ] Do not add primitive string actions for percent conversion.

## `url/value`

- [ ] enum `UrlHostKind`
  - [ ] `registered_name`
  - [ ] `ipv4`
  - [ ] `ipv6`
  - [ ] `ipv_future`
- [ ] object `Url`
  - [ ] `Url(origin is UrlParts)`
  - [ ] `parts() is UrlParts`
  - [ ] `url(reference is UrlReference) is Url`
  - [ ] `string() is string`
- [ ] object `UrlReference`
  - [ ] `UrlReference(origin is UrlParts)`
  - [ ] `absolute() is bool`
  - [ ] `parts() is UrlParts`
  - [ ] `string() is string`
- [ ] object `ParsedUrl`
  - [ ] `ParsedUrl(origin is string)`
  - [ ] `#projected value() is Url`
- [ ] object `ParsedUrlReference`
  - [ ] `ParsedUrlReference(origin is string)`
  - [ ] `#projected value() is UrlReference`
- [ ] object `UrlParts`
  - [ ] `UrlParts(scheme is string, authority is UrlAuthority, path is string, query is UrlQuery, fragment is UrlFragment)`
  - [ ] `scheme() is string`
  - [ ] `authority() is UrlAuthority`
  - [ ] `path() is string`
  - [ ] `query() is UrlQuery`
  - [ ] `fragment() is UrlFragment`
- [ ] object `UrlAuthority`
  - [ ] `UrlAuthority()`
  - [ ] `UrlAuthority(host is UrlHost)`
  - [ ] `UrlAuthority(host is UrlHost, port is int)`
  - [ ] `UrlAuthority(userinfo is string, host is UrlHost, port is int)`
  - [ ] `present() is bool`
  - [ ] `userinfo() is string`
  - [ ] `host() is UrlHost`
  - [ ] `port() is int`
  - [ ] `string() is string`
- [ ] role `UrlHost`
  - [ ] `kind() is UrlHostKind`
  - [ ] `name() is HostName`
  - [ ] `address() is IpAddress`
  - [ ] `string() is string`
- [ ] object `NamedUrlHost` does `UrlHost`
  - [ ] `NamedUrlHost(origin is HostName)`
  - [ ] `kind() is UrlHostKind`
  - [ ] `name() is HostName`
  - [ ] `address() is IpAddress`
  - [ ] `string() is string`
- [ ] object `AddressUrlHost` does `UrlHost`
  - [ ] `AddressUrlHost(origin is IpAddress)`
  - [ ] `kind() is UrlHostKind`
  - [ ] `name() is HostName`
  - [ ] `address() is IpAddress`
  - [ ] `string() is string`
- [ ] object `FutureUrlHost` does `UrlHost`
  - [ ] `FutureUrlHost(origin is string)`
  - [ ] `kind() is UrlHostKind`
  - [ ] `name() is HostName`
  - [ ] `address() is IpAddress`
  - [ ] `string() is string`
- [ ] object `UrlQuery`
  - [ ] `UrlQuery()`
  - [ ] `UrlQuery(origin is string)`
  - [ ] `present() is bool`
  - [ ] `string() is string`
- [ ] object `UrlFragment`
  - [ ] `UrlFragment()`
  - [ ] `UrlFragment(origin is string)`
  - [ ] `present() is bool`
  - [ ] `string() is string`
- [ ] object `InvalidUrl` does `Error`
  - [ ] `InvalidUrl(offset is int, reason is string)`
  - [ ] `offset() is int`
  - [ ] `text() is string`
- [ ] object `MissingUrlAuthority` does `Error`
  - [ ] `text() is string`
- [ ] object `MissingUrlPort` does `Error`
  - [ ] `text() is string`
- [ ] object `UrlHostIsNotName` does `Error`
  - [ ] `text() is string`
- [ ] object `UrlHostIsNotAddress` does `Error`
  - [ ] `text() is string`
- [ ] Require a scheme for `Url` and permit its absence in `UrlReference`.
- [ ] Support hierarchical and opaque absolute URLs.
- [ ] Permit authority only where introduced by `//`.
- [ ] Make absent and explicitly empty query values distinguishable.
- [ ] Make absent and explicitly empty fragments distinguishable.
- [ ] Throw `MissingUrlAuthority` from authority-dependent observations when absent.
- [ ] Throw `MissingUrlPort` instead of inventing a scheme-default port.
- [ ] Parse bracketed IPv6 and IPvFuture without confusing their colons with a port.
- [ ] Convert registered names through Stage 20 `HostName` and IDNA rules.
- [ ] Accept Unicode IRI characters and serialize them as canonical ASCII URL text.
- [ ] Lowercase schemes and registered host names.
- [ ] Canonicalize numeric IP address text through `IpAddress`.
- [ ] Preserve escaped reserved delimiters as escaped data.
- [ ] Resolve references according to RFC 3986 including dot-segment removal.
- [ ] Preserve a trailing slash during reference resolution.
- [ ] Define equality from canonical serialized URL components.
- [ ] Make zero-argument authority, query and fragment constructors represent absence.
- [ ] Validate composed parts by the same rules as parsed parts.
- [ ] Return an empty scheme only for a relative `UrlReference` without one.
- [ ] Do not remove default ports without scheme-specific policy.
- [ ] Do not sort query parameters during generic URL serialization.
- [ ] Do not expose internal parser-part objects.

## `url/query`

- [ ] object `QueryParameter`
  - [ ] `QueryParameter(name is string, value is string)`
  - [ ] `QueryParameter(name is string)`
  - [ ] `name() is string`
  - [ ] `value() is string`
  - [ ] `assigned() is bool`
- [ ] object `QueryParameters`
  - [ ] `QueryParameters(entries is Sequencing<QueryParameter>)`
  - [ ] `entries() is Sequencing<QueryParameter>`
  - [ ] `membership(name is string) is bool`
  - [ ] `first(name is string) is string`
  - [ ] `values(name is string) is Sequencing<string>`
  - [ ] `string() is string`
- [ ] object `ParsedQueryParameters`
  - [ ] `ParsedQueryParameters(origin is UrlQuery)`
  - [ ] `#projected value() is QueryParameters`
- [ ] object `MissingQueryParameter` does `Error`
  - [ ] `MissingQueryParameter(name is string)`
  - [ ] `name() is string`
  - [ ] `text() is string`
- [ ] Preserve parameter order and duplicate names.
- [ ] Preserve the distinction between `flag` and `flag=`.
- [ ] Split only on `&` in the first API.
- [ ] Decode names and values independently after structural splitting.
- [ ] Treat `+` as a literal plus.
- [ ] Percent-encode spaces as `%20`.
- [ ] Make `all()` return an empty sequence for a missing name.
- [ ] Make `first()` throw `MissingQueryParameter` for a missing name.
- [ ] Do not assume that every `UrlQuery` is a parameter list.

## `url/form`

- [ ] object `FormUrlEncoded` does `ByteSource`
  - [ ] `FormUrlEncoded(origin is QueryParameters)`
  - [ ] `bytes() is @byte`
  - [ ] `string() is string`
- [ ] object `ParsedForm`
  - [ ] `ParsedForm(origin is @byte)`
  - [ ] `parameters() is QueryParameters`
- [ ] object `InvalidFormEncoding` does `Error`
  - [ ] `InvalidFormEncoding(offset is int)`
  - [ ] `offset() is int`
  - [ ] `text() is string`
- [ ] Encode form data as UTF-8.
- [ ] Encode spaces as `+` and literal plus as `%2B`.
- [ ] Preserve ordering, duplicates and empty values.
- [ ] Require `name=value` when serializing form parameters.
- [ ] Decode percent triplets before UTF-8.
- [ ] Reject malformed UTF-8 with an input offset.
- [ ] Keep form encoding separate from generic URL query encoding.

## `url/file`

- [ ] object `FileUrl`
  - [ ] `FileUrl(origin is Path)`
  - [ ] `url() is Url`
- [ ] object `UrlFile`
  - [ ] `UrlFile(origin is Url, style is PathStyle)`
  - [ ] `path() is Path`
- [ ] object `InvalidFileUrl` does `Error`
  - [ ] `text() is string`
- [ ] Require the `file` scheme when projecting a path.
- [ ] Encode path segments without encoding path separators.
- [ ] Handle POSIX roots and Windows drive letters explicitly.
- [ ] Accept an empty authority and canonical `localhost` as local.
- [ ] Reject non-local authorities in the portable projection.
- [ ] Keep UNC and device paths behind explicit Windows behavior.
- [ ] Do not consult the filesystem while converting paths and URLs.

## `message/header`

- [ ] object `MessageHeaderName`
  - [ ] `MessageHeaderName(origin is string)`
  - [ ] `string() is string`
- [ ] object `MessageHeader`
  - [ ] `MessageHeader(name is MessageHeaderName, value is string)`
  - [ ] `name() is MessageHeaderName`
  - [ ] `value() is string`
- [ ] object `MessageHeaders`
  - [ ] `MessageHeaders(entries is Sequencing<MessageHeader>)`
  - [ ] `entries() is Sequencing<MessageHeader>`
  - [ ] `membership(name is MessageHeaderName) is bool`
  - [ ] `first(name is MessageHeaderName) is string`
  - [ ] `values(name is MessageHeaderName) is Sequencing<string>`
  - [ ] `addition(entry is MessageHeader) is MessageHeaders`
- [ ] object `RenderedMessageHeaders` does `ByteSource`
  - [ ] `RenderedMessageHeaders(origin is MessageHeaders)`
  - [ ] `bytes() is @byte`
- [ ] object `InvalidMessageHeader` does `Error`
  - [ ] `InvalidMessageHeader(offset is int, reason is string)`
  - [ ] `offset() is int`
  - [ ] `text() is string`
- [ ] object `MissingMessageHeader` does `Error`
  - [ ] `MissingMessageHeader(name is MessageHeaderName)`
  - [ ] `name() is MessageHeaderName`
  - [ ] `text() is string`
- [ ] Compare header names case-insensitively while preserving original spelling.
- [ ] Preserve field order and duplicate fields.
- [ ] Reject colon, controls, CR and LF in a header name.
- [ ] Reject bare CR or LF in a logical field value.
- [ ] Fold rendered fields only at legal whitespace boundaries.
- [ ] Use CRLF for physical lines.
- [ ] Enforce the hard 998-octet line limit.
- [ ] Target a soft 78-character line length.
- [ ] Unfold continuation lines before exposing values.
- [ ] Make `first()` throw and `all()` return empty on absence.
- [ ] Do not reuse HTTP header values or parsing rules.

## `mime/media-type`

- [ ] object `MimeParameter`
  - [ ] `MimeParameter(name is string, value is string)`
  - [ ] `name() is string`
  - [ ] `value() is string`
- [ ] object `MimeParameters`
  - [ ] `MimeParameters(entries is Sequencing<MimeParameter>)`
  - [ ] `entries() is Sequencing<MimeParameter>`
  - [ ] `membership(name is string) is bool`
  - [ ] `first(name is string) is string`
  - [ ] `values(name is string) is Sequencing<string>`
  - [ ] `string() is string`
- [ ] object `MediaType`
  - [ ] `MediaType(origin is string)`
  - [ ] `MediaType(type is string, subtype is string, parameters is MimeParameters)`
  - [ ] `type() is string`
  - [ ] `subtype() is string`
  - [ ] `parameters() is MimeParameters`
  - [ ] `string() is string`
- [ ] object `InvalidMediaType` does `Error`
  - [ ] `InvalidMediaType(offset is int, reason is string)`
  - [ ] `offset() is int`
  - [ ] `text() is string`
- [ ] object `MissingMimeParameter` does `Error`
  - [ ] `MissingMimeParameter(name is string)`
  - [ ] `name() is string`
  - [ ] `text() is string`
- [ ] Lowercase type, subtype and parameter names.
- [ ] Preserve parameter ordering and unknown parameters.
- [ ] Parse quoted strings and backslash escapes strictly.
- [ ] Support extended UTF-8 parameters and continuations from RFC 2231.
- [ ] Prefer the extended value when plain and extended forms coexist validly.
- [ ] Make `first()` throw and `all()` return empty on absence.
- [ ] Reject duplicate continuation indexes and missing continuation gaps.
- [ ] Serialize non-ASCII parameter values through the extended form.
- [ ] Do not maintain a closed enum of media types.

## `url/data`

- [ ] object `DataUrl`
  - [ ] `DataUrl(type is MediaType, data is @byte)`
  - [ ] `url() is Url`
- [ ] object `ParsedDataUrl`
  - [ ] `ParsedDataUrl(origin is Url)`
  - [ ] `ParsedDataUrl(origin is Url, maximum is int)`
  - [ ] `type() is MediaType`
  - [ ] `data() is @byte`
- [ ] object `InvalidDataUrl` does `Error`
  - [ ] `text() is string`
- [ ] Use percent encoding for compact safe text and Base64 otherwise.
- [ ] Recognize the `;base64` marker case-insensitively.
- [ ] Default an omitted media type to `text/plain;charset=US-ASCII`.
- [ ] Enforce the decoded-size limit while decoding.
- [ ] Reject fragments on data URLs.
- [ ] Never fetch external data while parsing a data URL.

## `mime/disposition`

- [ ] object `ContentDisposition`
  - [ ] `ContentDisposition(origin is string)`
  - [ ] `ContentDisposition(kind is string, parameters is MimeParameters)`
  - [ ] `kind() is string`
  - [ ] `parameters() is MimeParameters`
  - [ ] `string() is string`
- [ ] object `InvalidContentDisposition` does `Error`
  - [ ] `InvalidContentDisposition(offset is int, reason is string)`
  - [ ] `offset() is int`
  - [ ] `text() is string`
- [ ] object `MissingContentDisposition` does `Error`
  - [ ] `text() is string`
- [ ] Lowercase disposition kinds and parameter names.
- [ ] Preserve extension disposition kinds.
- [ ] Parse filenames through RFC 2231 extended parameters.
- [ ] Keep a received filename as an untrusted name, never a path.
- [ ] Do not join a received filename to a filesystem directory implicitly.

## `mime/transfer`

- [ ] enum `TransferEncoding`
  - [ ] `seven_bit`
  - [ ] `eight_bit`
  - [ ] `binary`
  - [ ] `quoted_printable`
  - [ ] `base64`
- [ ] object `TransferEncodedBody` does `ByteSource`
  - [ ] `TransferEncodedBody(origin is ByteSource, encoding is TransferEncoding)`
  - [ ] `bytes() is @byte`
- [ ] object `TransferDecodedBody` does `ByteSource`
  - [ ] `TransferDecodedBody(origin is ByteSource, encoding is TransferEncoding)`
  - [ ] `bytes() is @byte`
- [ ] object `InvalidTransferEncoding` does `Error`
  - [ ] `text() is string`
- [ ] Enforce CRLF-safe line wrapping for MIME Base64.
- [ ] Enforce MIME quoted-printable line limits and trailing-whitespace rules.
- [ ] Reject bytes outside the declared seven-bit profile.
- [ ] Preserve bytes exactly for binary transfer.
- [ ] Reject unknown transfer-encoding tokens while parsing a MIME entity.
- [ ] Reuse Stage 06 codecs rather than duplicate their alphabets.

## `mime/word`

- [ ] object `EncodedWord`
  - [ ] `EncodedWord(origin is string, charset is string)`
  - [ ] `string() is string`
- [ ] object `DecodedWords`
  - [ ] `DecodedWords(origin is string)`
  - [ ] `string() is string`
- [ ] object `InvalidEncodedWord` does `Error`
  - [ ] `InvalidEncodedWord(offset is int, reason is string)`
  - [ ] `offset() is int`
  - [ ] `text() is string`
- [ ] Support Base64 and Q encoded-word forms.
- [ ] Prefer UTF-8 for newly encoded words.
- [ ] Split long encoded words without splitting a Unicode scalar value.
- [ ] Decode adjacent encoded words according to RFC 2047 whitespace rules.
- [ ] Never decode encoded-word syntax in forbidden structured positions.
- [ ] Reject unsupported declared character sets explicitly.

## `mime/part`

- [ ] role `MimeBody` does `ByteSource`
  - [ ] `bytes() is @byte`
- [ ] object `BinaryBody` does `MimeBody`
  - [ ] `BinaryBody(origin is @byte)`
  - [ ] `bytes() is @byte`
- [ ] object `TextBody` does `MimeBody`
  - [ ] `TextBody(origin is string, charset is string)`
  - [ ] `string() is string`
  - [ ] `charset() is string`
  - [ ] `bytes() is @byte`
- [ ] object `UnsupportedMimeCharset` does `Error`
  - [ ] `UnsupportedMimeCharset(charset is string)`
  - [ ] `charset() is string`
  - [ ] `text() is string`
- [ ] object `MultipartBody` does `MimeBody, Iterating<MimePart>`
  - [ ] `MultipartBody(subtype is string, boundary is string, parts is Sequencing<MimePart>)`
  - [ ] `subtype() is string`
  - [ ] `boundary() is string`
  - [ ] `cursor() is Cursor<MimePart>`
  - [ ] `bytes() is @byte`
- [ ] object `GeneratedBoundary`
  - [ ] `GeneratedBoundary(random is SecureRandom)`
  - [ ] `string() is string`
- [ ] object `MimePart` does `ByteSource`
  - [ ] `MimePart(headers is MessageHeaders, body is MimeBody)`
  - [ ] `headers() is MessageHeaders`
  - [ ] `body() is MimeBody`
  - [ ] `bytes() is @byte`
- [ ] object `MimeContent`
  - [ ] `MimeContent(origin is MimePart)`
  - [ ] `type() is MediaType`
  - [ ] `disposition() is ContentDisposition`
  - [ ] `encoding() is TransferEncoding`
- [ ] object `MimeLimits`
  - [ ] `MimeLimits()`
  - [ ] `MimeLimits(header_bytes is int, line_bytes is int, parts is int, depth is int, body_bytes is int)`
  - [ ] `header_bytes() is int`
  - [ ] `line_bytes() is int`
  - [ ] `parts() is int`
  - [ ] `depth() is int`
  - [ ] `body_bytes() is int`
- [ ] object `ParsedMime`
  - [ ] `ParsedMime(origin is @byte)`
  - [ ] `ParsedMime(origin is @byte, limits is MimeLimits)`
  - [ ] `part() is MimePart`
- [ ] object `InvalidMime` does `Error`
  - [ ] `InvalidMime(offset is int, reason is string)`
  - [ ] `offset() is int`
  - [ ] `text() is string`
- [ ] Match multipart boundaries only at valid line boundaries.
- [ ] Exclude delimiter CRLF from child body bytes.
- [ ] Preserve preamble and epilogue internally without treating them as parts.
- [ ] Require a closing boundary in strict parsing.
- [ ] Validate that multipart media type and body boundary agree.
- [ ] Apply content-transfer decoding only after boundary separation.
- [ ] Enforce header, line, part, nesting and decoded-body limits incrementally.
- [ ] Generate boundaries absent from every serialized child body.
- [ ] Use CRLF in rendered MIME entities.
- [ ] Preserve unknown MIME headers.
- [ ] Make an absent content type default to `text/plain;charset=US-ASCII`.
- [ ] Make an absent transfer encoding default to `seven_bit`.
- [ ] Make an absent disposition throw `MissingContentDisposition`.
- [ ] Decode only character sets supported by Stage 06 and report the rest explicitly.
- [ ] Do not expose multipart traversal as a blocking cursor.

## `mime/types`

- [ ] role `MediaIdentifying`
  - [ ] `type(path is Path) is MediaType`
  - [ ] `extensions(type is MediaType) is Sequencing<string>`
- [ ] object `StandardMediaTypes` does `MediaIdentifying`
  - [ ] `StandardMediaTypes()`
  - [ ] `type(path is Path) is MediaType`
  - [ ] `extensions(type is MediaType) is Sequencing<string>`
- [ ] object `SystemMediaTypes` does `MediaIdentifying`
  - [ ] `SystemMediaTypes(fallback is MediaIdentifying)`
  - [ ] `type(path is Path) is MediaType`
  - [ ] `extensions(type is MediaType) is Sequencing<string>`
- [ ] object `UnknownMediaType` does `Error`
  - [ ] `UnknownMediaType(path is Path)`
  - [ ] `path() is Path`
  - [ ] `text() is string`
- [ ] Ship a deterministic IANA-derived common type table.
- [ ] Match extensions case-insensitively.
- [ ] Prefer the longest compound extension.
- [ ] Snapshot optional system media-type files.
- [ ] Keep caller overrides outside the immutable standard table.
- [ ] Do not inspect file contents in extension-based identification.

## `mail/address`

- [ ] enum `MailAddressKind`
  - [ ] `mailbox`
  - [ ] `group`
- [ ] object `Mailbox`
  - [ ] `Mailbox(local is string, domain is HostName)`
  - [ ] `Mailbox(name is string, local is string, domain is HostName)`
  - [ ] `name() is string`
  - [ ] `local_part() is string`
  - [ ] `domain() is HostName`
  - [ ] `string() is string`
- [ ] object `AddressGroup`
  - [ ] `AddressGroup(name is string, members is Sequencing<Mailbox>)`
  - [ ] `name() is string`
  - [ ] `members() is Sequencing<Mailbox>`
  - [ ] `string() is string`
- [ ] role `MailAddress`
  - [ ] `kind() is MailAddressKind`
  - [ ] `mailbox() is Mailbox`
  - [ ] `group() is AddressGroup`
  - [ ] `string() is string`
- [ ] object `MailboxAddress` does `MailAddress`
  - [ ] `MailboxAddress(origin is Mailbox)`
  - [ ] `kind() is MailAddressKind`
  - [ ] `mailbox() is Mailbox`
  - [ ] `group() is AddressGroup`
  - [ ] `string() is string`
- [ ] object `GroupAddress` does `MailAddress`
  - [ ] `GroupAddress(origin is AddressGroup)`
  - [ ] `kind() is MailAddressKind`
  - [ ] `mailbox() is Mailbox`
  - [ ] `group() is AddressGroup`
  - [ ] `string() is string`
- [ ] object `MailAddresses`
  - [ ] `MailAddresses(entries is Sequencing<MailAddress>)`
  - [ ] `entries() is Sequencing<MailAddress>`
  - [ ] `mailboxes() is Sequencing<Mailbox>`
  - [ ] `string() is string`
- [ ] object `ParsedMailAddresses`
  - [ ] `ParsedMailAddresses(origin is string)`
  - [ ] `#projected value() is MailAddresses`
- [ ] object `InvalidMailAddress` does `Error`
  - [ ] `InvalidMailAddress(offset is int, reason is string)`
  - [ ] `offset() is int`
  - [ ] `text() is string`
- [ ] object `MailAddressIsNotMailbox` does `Error`
  - [ ] `text() is string`
- [ ] object `MailAddressIsNotGroup` does `Error`
  - [ ] `text() is string`
- [ ] Parse quoted display names, comments, groups and angle addresses.
- [ ] Preserve display names as Unicode logical text.
- [ ] Serialize non-ASCII display names through encoded words.
- [ ] Normalize domains through `HostName`.
- [ ] Preserve local-part case.
- [ ] Throw a typed variant error from the wrong `MailAddress` projection.
- [ ] Support UTF-8 local parts only when SMTPUTF8 is explicitly available.
- [ ] Bind address parts without validation and reject obsolete route-address syntax lazily on the first projection.
- [ ] Do not expose a misleading undivided `email()` string action.

## `mail/auth`

- [ ] enum `MailAuthenticationKind`
  - [ ] `password`
  - [ ] `bearer`
- [ ] role `MailAuthentication`
  - [ ] `kind() is MailAuthenticationKind`
  - [ ] `identity() is string`
- [ ] object `MailCredentials` does `MailAuthentication`
  - [ ] `MailCredentials(username is string, password is SecretBytes)`
  - [ ] `kind() is MailAuthenticationKind`
  - [ ] `identity() is string`
- [ ] object `BearerCredentials` does `MailAuthentication`
  - [ ] `BearerCredentials(identity is string, token is SecretBytes)`
  - [ ] `kind() is MailAuthenticationKind`
  - [ ] `identity() is string`
- [ ] Reject embedded NUL, CR and LF in authentication identities.
- [ ] Never expose password or bearer-token bytes through public actions.
- [ ] Redact all credential objects in diagnostics.
- [ ] Do not authenticate over plaintext transport.

## `mail/message`

- [ ] object `MailMessage` does `ByteSource`
  - [ ] `MailMessage(headers is MessageHeaders, body is MimeBody)`
  - [ ] `headers() is MessageHeaders`
  - [ ] `body() is MimeBody`
  - [ ] `bytes() is @byte`
- [ ] object `ParsedMail`
  - [ ] `ParsedMail(origin is @byte)`
  - [ ] `ParsedMail(origin is @byte, limits is MimeLimits)`
  - [ ] `message() is MailMessage`
- [ ] object `ReversePath`
  - [ ] `ReversePath()`
  - [ ] `ReversePath(origin is Mailbox)`
  - [ ] `present() is bool`
  - [ ] `mailbox() is Mailbox`
- [ ] object `MissingReversePath` does `Error`
  - [ ] `text() is string`
- [ ] object `MailEnvelope`
  - [ ] `MailEnvelope(reverse_path is ReversePath, recipients is Sequencing<Mailbox>)`
  - [ ] `reverse_path() is ReversePath`
  - [ ] `recipients() is Sequencing<Mailbox>`
- [ ] object `OutgoingMail`
  - [ ] `OutgoingMail(envelope is MailEnvelope, message is MailMessage)`
  - [ ] `envelope() is MailEnvelope`
  - [ ] `message() is MailMessage`
- [ ] object `MessageId`
  - [ ] `MessageId(origin is string)`
  - [ ] `string() is string`
- [ ] object `GeneratedMessageId`
  - [ ] `GeneratedMessageId(domain is HostName, clock is Timing, random is SecureRandom)`
  - [ ] `result() is MessageId`
- [ ] object `MailDate`
  - [ ] `MailDate(origin is ZonedDateTime)`
  - [ ] `string() is string`
- [ ] object `ParsedMailDate`
  - [ ] `ParsedMailDate(origin is string)`
  - [ ] `value() is ZonedDateTime`
- [ ] object `InvalidMail` does `Error`
  - [ ] `InvalidMail(offset is int, reason is string)`
  - [ ] `offset() is int`
  - [ ] `text() is string`
- [ ] Reject an envelope without recipients.
- [ ] Keep transport envelope sender and recipients separate from message headers.
- [ ] Permit a null envelope sender for delivery-status messages.
- [ ] Make `EnvelopeSender.mailbox()` throw `MissingEnvelopeSender` for the null sender.
- [ ] Preserve unknown headers and their order.
- [ ] Parse MIME recursively through `ParsedMime` and shared limits.
- [ ] Render one blank CRLF line between header block and body.
- [ ] Generate globally unpredictable message identifiers without embedding private host data.
- [ ] Parse obsolete date zones only where RFC compatibility requires them.
- [ ] Render new dates with a numeric offset.
- [ ] Do not infer an SMTP envelope from `From`, `Sender`, `To`, `Cc` or `Bcc`.
- [ ] Remove `Bcc` fields from a delivered wire message without changing the envelope.

## `mail/mailbox`

- [ ] enum `SystemMailFlag`
  - [ ] `seen`
  - [ ] `answered`
  - [ ] `flagged`
  - [ ] `deleted`
  - [ ] `draft`
  - [ ] `recent`
- [ ] enum `MailFlagKind`
  - [ ] `system`
  - [ ] `keyword`
- [ ] role `MailFlag`
  - [ ] `kind() is MailFlagKind`
  - [ ] `string() is string`
- [ ] object `SystemFlag` does `MailFlag`
  - [ ] `SystemFlag(origin is SystemMailFlag)`
  - [ ] `kind() is MailFlagKind`
  - [ ] `system() is SystemMailFlag`
  - [ ] `string() is string`
- [ ] object `KeywordFlag` does `MailFlag`
  - [ ] `KeywordFlag(origin is string)`
  - [ ] `kind() is MailFlagKind`
  - [ ] `keyword() is string`
  - [ ] `string() is string`
- [ ] object `StoredMail`
  - [ ] Do not expose a public constructor.
  - [ ] `identifier() is string`
  - [ ] `message() is MailMessage`
  - [ ] `flags() is Set<MailFlag>`
- [ ] object `Maildir`
  - [ ] `Maildir(path is Path)`
  - [ ] `path() is Path`
  - [ ] `messages() is Sequencing<StoredMail>`
  - [ ] `add(message is MailMessage) is void`
  - [ ] `remove(identifier is string) is void`
- [ ] object `ChangedMaildirFlags`
  - [ ] `ChangedMaildirFlags(origin is Maildir, identifier is string, flags is Set<MailFlag>)`
  - [ ] `result() is StoredMail`
- [ ] object `Mbox`
  - [ ] `Mbox(path is Path)`
  - [ ] `session() is MboxSession`
- [ ] object `MboxSession` does `Closing`
  - [ ] `messages() is Sequencing<StoredMail>`
  - [ ] `add(message is MailMessage) is void`
  - [ ] `remove(identifier is string) is void`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `InvalidMailbox` does `Error`
  - [ ] `InvalidMailbox(offset is int, reason is string)`
  - [ ] `offset() is int`
  - [ ] `text() is string`
- [ ] Commit Maildir messages through same-filesystem atomic rename.
- [ ] Generate collision-resistant Maildir identifiers.
- [ ] Preserve Maildir flags in filenames without trusting arbitrary path text.
- [ ] Preserve unknown IMAP keyword flags.
- [ ] Throw a typed variant error from the wrong `MailFlag` projection.
- [ ] Lock mbox before reading or changing it.
- [ ] Escape and unescape mbox `From ` separator lines reversibly.
- [ ] Rewrite mbox changes atomically before releasing the lock.
- [ ] Enforce `MimeLimits` while loading stored messages.
- [ ] Do not hold an mbox lock after `MboxSession.close()`.
- [ ] Add MMDF, MH and Babyl only as low-priority format adapters.

## `mail/smtp`

- [ ] enum `SmtpSecurity`
  - [ ] `tls`
  - [ ] `start_tls`
  - [ ] `plain`
- [ ] enum `SmtpFeature`
  - [ ] `eight_bit_mime`
  - [ ] `smtp_utf8`
  - [ ] `pipelining`
  - [ ] `size`
  - [ ] `delivery_status`
  - [ ] `start_tls`
  - [ ] `authentication`
- [ ] enum `SmtpProtocol`
  - [ ] `smtp`
  - [ ] `lmtp`
- [ ] object `SmtpServer`
  - [ ] `SmtpServer(endpoint is HostEndpoint, security is SmtpSecurity)`
  - [ ] `SmtpServer(endpoint is HostEndpoint, security is SmtpSecurity, protocol is SmtpProtocol)`
  - [ ] `endpoint() is HostEndpoint`
  - [ ] `security() is SmtpSecurity`
  - [ ] `protocol() is SmtpProtocol`
- [ ] object `SmtpCapabilities`
  - [ ] Do not expose a public constructor.
  - [ ] `features() is Set<SmtpFeature>`
  - [ ] `maximum_size() is BigInt`
  - [ ] `mechanisms() is Sequencing<string>`
- [ ] object `SmtpConnecting`
  - [ ] `SmtpConnecting(server is SmtpServer, resolver is Resolving, clock is Timing)`
  - [ ] `SmtpConnecting(server is SmtpServer, resolver is Resolving, tls is TlsClientPolicy, clock is Timing)`
  - [ ] `async session() is SmtpSession`
- [ ] object `LmtpUnixConnecting`
  - [ ] `LmtpUnixConnecting(endpoint is UnixAddress, clock is Timing)`
  - [ ] `async session() is SmtpSession`
- [ ] object `SmtpSession` does `Closing`
  - [ ] Do not expose a public constructor.
  - [ ] `capabilities() is SmtpCapabilities`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `SmtpAuthenticated`
  - [ ] `SmtpAuthenticated(origin is SmtpSession, authentication is MailAuthentication)`
  - [ ] `async session() is SmtpSession`
- [ ] object `SmtpDelivery`
  - [ ] `SmtpDelivery(origin is SmtpSession, mail is OutgoingMail)`
  - [ ] `async receipt() is SmtpReceipt`
- [ ] object `RecipientDelivery`
  - [ ] Do not expose a public constructor.
  - [ ] `recipient() is Mailbox`
  - [ ] `accepted() is bool`
  - [ ] `code() is int`
  - [ ] `text() is string`
- [ ] object `SmtpReceipt`
  - [ ] Do not expose a public constructor.
  - [ ] `recipients() is Sequencing<RecipientDelivery>`
  - [ ] `accepted() is bool`
  - [ ] `code() is int`
  - [ ] `text() is string`
- [ ] object `SmtpFailure` does `Error`
  - [ ] `SmtpFailure(code is int, message is string)`
  - [ ] `code() is int`
  - [ ] `text() is string`
- [ ] Read multiline replies with strict line and aggregate limits.
- [ ] Send EHLO and retain unknown extension tokens internally.
- [ ] Require a successful second EHLO after STARTTLS.
- [ ] Verify implicit TLS and STARTTLS with `TlsClientPolicy`.
- [ ] Permit the constructor without `TlsClientPolicy` only for explicitly plain unauthenticated transport.
- [ ] Make `SmtpServer` default to the SMTP protocol.
- [ ] Support local LMTP over a Unix-domain connection without pretending it is SMTP over TCP.
- [ ] Refuse credentials before verified TLS is active.
- [ ] Support PLAIN and bearer authentication only over verified TLS.
- [ ] Dot-stuff message lines and terminate DATA with CRLF dot CRLF.
- [ ] Negotiate SMTPUTF8 before sending UTF-8 envelope local parts.
- [ ] Negotiate 8BITMIME or choose a seven-bit transfer encoding.
- [ ] Enforce the advertised message-size limit before DATA.
- [ ] Preserve per-recipient partial acceptance in `SmtpReceipt`.
- [ ] Preserve LMTP's final response for every accepted recipient.
- [ ] Reuse an open session for sequential deliveries.
- [ ] Serialize commands within one session.
- [ ] Make every network wait cancellable.
- [ ] Compose connection and command deadlines through `Within`.
- [ ] Send QUIT during orderly close but close immediately after cancellation or protocol failure.
- [ ] Do not expose a raw SMTP command method.
- [ ] Do not retry delivery implicitly.

## `mail/imap`

- [ ] enum `ImapSecurity`
  - [ ] `tls`
  - [ ] `start_tls`
- [ ] enum `ImapFeature`
  - [ ] `idle`
  - [ ] `move`
  - [ ] `uid_plus`
  - [ ] `literal_plus`
  - [ ] `utf8_accept`
- [ ] role `MailMatching`
  - [ ] `criterion() is string`
- [ ] object `AllMail` does `MailMatching`
  - [ ] `AllMail()`
  - [ ] `criterion() is string`
- [ ] object `UnseenMail` does `MailMatching`
  - [ ] `UnseenMail()`
  - [ ] `criterion() is string`
- [ ] object `FromMail` does `MailMatching`
  - [ ] `FromMail(origin is Mailbox)`
  - [ ] `criterion() is string`
- [ ] object `SinceMail` does `MailMatching`
  - [ ] `SinceMail(origin is Date)`
  - [ ] `criterion() is string`
- [ ] object `AndMail` does `MailMatching`
  - [ ] `AndMail(left is MailMatching, right is MailMatching)`
  - [ ] `criterion() is string`
- [ ] object `OrMail` does `MailMatching`
  - [ ] `OrMail(left is MailMatching, right is MailMatching)`
  - [ ] `criterion() is string`
- [ ] object `NotMail` does `MailMatching`
  - [ ] `NotMail(origin is MailMatching)`
  - [ ] `criterion() is string`
- [ ] object `ImapServer`
  - [ ] `ImapServer(endpoint is HostEndpoint, security is ImapSecurity)`
  - [ ] `endpoint() is HostEndpoint`
  - [ ] `security() is ImapSecurity`
- [ ] object `ImapConnecting`
  - [ ] `ImapConnecting(server is ImapServer, resolver is Resolving, tls is TlsClientPolicy, authentication is MailAuthentication, clock is Timing)`
  - [ ] `async session() is ImapSession`
- [ ] object `ImapCapabilities`
  - [ ] Do not expose a public constructor.
  - [ ] `features() is Set<ImapFeature>`
  - [ ] `authentication() is Sequencing<string>`
- [ ] object `ImapMailbox`
  - [ ] Do not expose a public constructor.
  - [ ] `name() is string`
  - [ ] `delimiter() is string`
  - [ ] `flags() is Set<MailFlag>`
- [ ] object `SelectedMailbox` does `Closing`
  - [ ] Do not expose a public constructor.
  - [ ] `name() is string`
  - [ ] `state() is ImapMailboxState`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `ImapMailboxState`
  - [ ] Do not expose a public constructor.
  - [ ] `messages() is int`
  - [ ] `recent() is int`
  - [ ] `unseen() is int`
  - [ ] `uid_validity() is BigInt`
  - [ ] `uid_next() is BigInt`
- [ ] object `ImapSession` does `Closing`
  - [ ] Do not expose a public constructor.
  - [ ] `capabilities() is ImapCapabilities`
  - [ ] `async mailboxes() is Sequencing<ImapMailbox>`
  - [ ] `async mailbox(name is string) is SelectedMailbox`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] enum `ImapDatum`
  - [ ] `headers`
  - [ ] `body`
  - [ ] `flags`
  - [ ] `internal_date`
  - [ ] `size`
- [ ] object `ImapMessage`
  - [ ] Do not expose a public constructor.
  - [ ] `sequence() is int`
  - [ ] `uid() is BigInt`
  - [ ] `flags() is Set<MailFlag>`
  - [ ] `headers() is MessageHeaders`
  - [ ] `body() is @byte`
- [ ] object `SearchedMail`
  - [ ] `SearchedMail(origin is SelectedMailbox, matching is MailMatching)`
  - [ ] `async uids() is Sequencing<BigInt>`
- [ ] object `FetchedMail`
  - [ ] `FetchedMail(origin is SelectedMailbox, uids is Sequencing<BigInt>, data is Set<ImapDatum>)`
  - [ ] `async messages() is Sequencing<ImapMessage>`
- [ ] enum `FlagChange`
  - [ ] `adding`
  - [ ] `removing`
  - [ ] `replacing`
- [ ] object `ChangedMailFlags`
  - [ ] `ChangedMailFlags(origin is SelectedMailbox, uids is Sequencing<BigInt>, flags is Set<MailFlag>, change is FlagChange)`
  - [ ] `async messages() is Sequencing<ImapMessage>`
- [ ] object `ExpungedMail`
  - [ ] `ExpungedMail(origin is SelectedMailbox)`
  - [ ] `async identifiers() is Sequencing<int>`
- [ ] object `AppendedMail`
  - [ ] `AppendedMail(origin is ImapSession, mailbox is string, message is MailMessage, flags is Set<MailFlag>)`
  - [ ] `async uid() is BigInt`
- [ ] object `CopiedMail`
  - [ ] `CopiedMail(origin is SelectedMailbox, uids is Sequencing<BigInt>, destination is string)`
  - [ ] `async uids() is Sequencing<BigInt>`
- [ ] object `MovedMail`
  - [ ] `MovedMail(origin is SelectedMailbox, uids is Sequencing<BigInt>, destination is string)`
  - [ ] `async uids() is Sequencing<BigInt>`
- [ ] object `CreatedImapMailbox`
  - [ ] `CreatedImapMailbox(origin is ImapSession, name is string)`
  - [ ] `async result() is ImapMailbox`
- [ ] object `ImapMailboxDeletion`
  - [ ] `ImapMailboxDeletion(origin is ImapSession, name is string)`
  - [ ] `async delete() is ImapMailboxDeletion`
- [ ] object `RenamedImapMailbox`
  - [ ] `RenamedImapMailbox(origin is ImapSession, source is string, destination is string)`
  - [ ] `async result() is ImapMailbox`
- [ ] object `ImapSubscription`
  - [ ] `ImapSubscription(origin is ImapSession, name is string, subscribed is bool)`
  - [ ] `async apply() is ImapSubscription`
- [ ] enum `MailboxEventKind`
  - [ ] `exists`
  - [ ] `expunged`
  - [ ] `flags_changed`
  - [ ] `recent`
- [ ] object `MailboxEvent`
  - [ ] Do not expose a public constructor.
  - [ ] `kind() is MailboxEventKind`
  - [ ] `sequence() is int`
  - [ ] `uid() is BigInt`
  - [ ] `flags() is Set<MailFlag>`
- [ ] object `MailboxEvents` does `Closing`
  - [ ] `MailboxEvents(origin is SelectedMailbox)`
  - [ ] `async event() is MailboxEvent`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `ImapFailure` does `Error`
  - [ ] `ImapFailure(message is string)`
  - [ ] `text() is string`
- [ ] Require verified TLS before authentication.
- [ ] Support SASL PLAIN and bearer authentication over verified TLS.
- [ ] Generate unique command tags internally.
- [ ] Parse tagged, untagged and continuation responses incrementally.
- [ ] Bound response lines, literals and aggregate command output.
- [ ] Quote or literal-encode every mailbox name and search value.
- [ ] Support modified UTF-7 until UTF8=ACCEPT is active.
- [ ] Keep sequence numbers and stable UIDs distinct.
- [ ] Use UID SEARCH, UID FETCH and UID STORE by default.
- [ ] Use UID COPY, UID MOVE and UID EXPUNGE when the server supports them.
- [ ] Fall back from MOVE to copy, deleted flag and expunge only when equivalent behavior is possible.
- [ ] Return assigned UIDs only when UIDPLUS supplies them.
- [ ] Invalidate cached UIDs when UIDVALIDITY changes.
- [ ] Surface unsolicited mailbox state changes through the next operation.
- [ ] Serialize commands within one session.
- [ ] Enter IDLE lazily on the first `MailboxEvents.next()` call.
- [ ] Leave IDLE before executing another command on the session.
- [ ] Make closing `MailboxEvents` send DONE and await tagged completion.
- [ ] Compose all deadlines through `Within`.
- [ ] Do not expose raw IMAP commands or unescaped criteria strings.

## `mail/pop3`

- [ ] enum `PopSecurity`
  - [ ] `tls`
  - [ ] `start_tls`
- [ ] enum `PopFeature`
  - [ ] `unique_ids`
  - [ ] `top`
  - [ ] `pipelining`
  - [ ] `start_tls`
- [ ] object `PopServer`
  - [ ] `PopServer(endpoint is HostEndpoint, security is PopSecurity)`
  - [ ] `endpoint() is HostEndpoint`
  - [ ] `security() is PopSecurity`
- [ ] object `PopConnecting`
  - [ ] `PopConnecting(server is PopServer, resolver is Resolving, tls is TlsClientPolicy, credentials is MailCredentials, clock is Timing)`
  - [ ] `async session() is PopSession`
- [ ] object `PopMessage`
  - [ ] Do not expose a public constructor.
  - [ ] `number() is int`
  - [ ] `identifier() is string`
  - [ ] `size() is int`
- [ ] object `PopMailbox`
  - [ ] Do not expose a public constructor.
  - [ ] `count() is int`
  - [ ] `size() is BigInt`
  - [ ] `async messages() is Sequencing<PopMessage>`
  - [ ] `async message(number is int) is MailMessage`
  - [ ] `async delete(number is int) is PopMailbox`
- [ ] object `PopSession` does `Closing`
  - [ ] Do not expose a public constructor.
  - [ ] `mailbox() is PopMailbox`
  - [ ] `capabilities() is Set<PopFeature>`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `PreviewedPopMail`
  - [ ] `PreviewedPopMail(origin is PopMailbox, number is int, lines is int)`
  - [ ] `async bytes() is @byte`
- [ ] object `PopDeleteReset`
  - [ ] `PopDeleteReset(origin is PopSession)`
  - [ ] `async reset() is PopDeleteReset`
- [ ] object `PopFailure` does `Error`
  - [ ] `PopFailure(message is string)`
  - [ ] `text() is string`
- [ ] Require verified TLS before authentication.
- [ ] Support USER/PASS only over verified TLS.
- [ ] Parse multiline dot-stuffed responses incrementally.
- [ ] Bound line, message and aggregate listing sizes.
- [ ] Keep session message numbers distinct from unique identifiers.
- [ ] Use UIDL for stable identifiers and report its absence explicitly.
- [ ] Use TOP only when advertised and reject negative line counts.
- [ ] Apply deletes only after successful orderly QUIT.
- [ ] Reset pending deletes explicitly through `PopDeleteReset`.
- [ ] Make every network wait cancellable.
- [ ] Compose all deadlines through `Within`.
- [ ] Do not expose raw POP3 commands.

## Stage 21 verification

- [ ] Add RFC 3986 parsing and relative-resolution vectors.
- [ ] Add Unicode IRI, IDNA, IPv4, IPv6 and IPvFuture URL cases.
- [ ] Add percent-decoding source-offset and malformed UTF-8 cases.
- [ ] Prove generic query and form encoding disagree on space and plus exactly as specified.
- [ ] Add file-URL vectors for POSIX and Windows path styles.
- [ ] Add bounded data-URL decoding tests.
- [ ] Add RFC 2045, 2046, 2047, 2183 and 2231 MIME vectors.
- [ ] Add nested multipart, preamble, epilogue and boundary-collision cases.
- [ ] Add adversarial MIME depth, part-count, line and decoded-size limit cases.
- [ ] Add RFC 5322 mailbox, group, comment, quoting and folding vectors.
- [ ] Add SMTPUTF8 and ASCII-only envelope cases.
- [ ] Add Maildir atomicity and identifier-collision tests.
- [ ] Add mbox locking, escaping and interrupted-rewrite tests.
- [ ] Test SMTP and LMTP against deterministic scripted loopback servers.
- [ ] Test SMTP partial recipient acceptance and post-STARTTLS EHLO behavior.
- [ ] Test IMAP literals, untagged responses, UIDVALIDITY changes and IDLE cancellation.
- [ ] Test POP3 dot-stuffing, UIDL, pending deletion reset and orderly commit.
- [ ] Add cancellation and timeout races at every mail-protocol network wait.
- [ ] Add one interoperability test for each mail protocol against an external implementation.
- [ ] Verify credential and token values are redacted from every failure path.
- [ ] Add one Perl-level regression file per public module group.
- [ ] Add runnable URL, MIME construction, SMTP delivery and IMAP retrieval examples.
- [ ] Run `git diff --check`.

## Stage 21 exclusions

- [ ] Do not derive the v3 public API from the existing stdlib URL module.
- [ ] Do not add URL, MIME or mail methods to builtin `string` or `@byte`.
- [ ] Do not merge generic URL query parsing with form encoding.
- [ ] Do not treat an arbitrary URL query as key-value data implicitly.
- [ ] Do not make received filenames into filesystem paths implicitly.
- [ ] Do not reuse HTTP header parsing for Internet Message Format headers.
- [ ] Do not duplicate Stage 06 Base64 or quoted-printable codecs.
- [ ] Do not expose credentials, bearer tokens or private keys in diagnostics.
- [ ] Do not send credentials through plaintext SMTP, IMAP or POP3.
- [ ] Do not infer transport envelopes from visible message headers.
- [ ] Do not retry mail delivery or deletion implicitly.
- [ ] Keep HTTP and WebSocket protocols in Stage 22.
- [ ] Keep DKIM, ARC, SPF and DMARC verification for the final coverage audit after DNS record support exists.

---

# Stage 22 — HTTP and related protocols

## `runtime/http` substrate

- [x] Keep the runtime free of HTTP-specific value objects and protocol intrinsics.
- [ ] Build HTTP/1.1 and HTTP/2 on Stage 20 async byte streams.
- [ ] Drive protocol timeouts through Stage 19 cancellation and `Within`.
- [ ] Reuse Stage 15 digests for WebSocket handshake validation.
- [ ] Reuse Stage 16 compression for HTTP content codings.
- [ ] Reuse Stage 18 certificate verification and Stage 20 TLS.
- [ ] Reuse Stage 21 URL, form, MIME media-type and cookie-adjacent grammar.
- [ ] Add a native bulk path only after profiling identifies a concrete framing or HPACK bottleneck.
- [ ] Do not add a second event loop, socket layer or TLS abstraction for HTTP.

## `http/method`

- [ ] object `Method`
  - [ ] `Method(origin is string)`
  - [ ] `string() is string`
- [ ] object `MethodSemantics`
  - [ ] `MethodSemantics(origin is Method)`
  - [ ] `safe() is bool`
  - [ ] `idempotent() is bool`
- [ ] object `InvalidMethod` does `Error`
  - [ ] `InvalidMethod(origin is string)`
  - [ ] `text() is string`
- [ ] Normalize method tokens to uppercase.
- [ ] Preserve valid extension methods.
- [ ] Mark GET, HEAD, OPTIONS and TRACE as safe.
- [ ] Mark safe methods plus PUT and DELETE as idempotent.
- [ ] Do not use a closed enum for HTTP methods.
- [ ] Do not infer that GET or DELETE can never carry a body.

## `http/status`

- [ ] enum `StatusClass`
  - [ ] `informational`
  - [ ] `successful`
  - [ ] `redirection`
  - [ ] `client_error`
  - [ ] `server_error`
- [ ] object `Status`
  - [ ] `Status(code is int)`
  - [ ] `code() is int`
  - [ ] `class() is StatusClass`
  - [ ] `phrase() is string`
- [ ] object `InvalidStatus` does `Error`
  - [ ] `InvalidStatus(code is int)`
  - [ ] `code() is int`
  - [ ] `text() is string`
- [ ] Accept extension status codes in `100..599`.
- [ ] Return an empty phrase for an unknown valid code.
- [ ] Treat reason phrases as display text, never protocol semantics.
- [ ] Make status-class checks operate on the numeric hundred digit.
- [ ] Do not generate a reason phrase in HTTP/2 framing.

## `http/version`

- [ ] enum `HttpVersion`
  - [ ] `http10`
  - [ ] `http11`
  - [ ] `http2`
- [ ] object `UnsupportedHttpVersion` does `Error`
  - [ ] `text() is string`
- [ ] Negotiate HTTP/2 over TLS through ALPN `h2`.
- [ ] Negotiate HTTP/1.1 through ALPN `http/1.1` or fallback.
- [ ] Support cleartext HTTP/1.1 directly.
- [ ] Support prior-knowledge cleartext HTTP/2 only through explicit policy.
- [ ] Do not silently attempt the ambiguous h2c upgrade path in the first API.

## `http/header`

- [ ] object `HeaderName`
  - [ ] `HeaderName(origin is string)`
  - [ ] `string() is string`
- [ ] object `Header`
  - [ ] `Header(name is HeaderName, value is string)`
  - [ ] `name() is HeaderName`
  - [ ] `value() is string`
- [ ] object `Headers`
  - [ ] `Headers(entries is Sequencing<Header>)`
  - [ ] `entries() is Sequencing<Header>`
  - [ ] `membership(name is HeaderName) is bool`
  - [ ] `first(name is HeaderName) is string`
  - [ ] `values(name is HeaderName) is Sequencing<string>`
  - [ ] `addition(entry is Header) is Headers`
- [ ] object `HeadersWithout`
  - [ ] `HeadersWithout(origin is Headers, names is Set<HeaderName>)`
  - [ ] `result() is Headers`
- [ ] object `ReplacedHeader`
  - [ ] `ReplacedHeader(origin is Headers, entry is Header)`
  - [ ] `result() is Headers`
- [ ] object `MissingHeader` does `Error`
  - [ ] `MissingHeader(name is HeaderName)`
  - [ ] `name() is HeaderName`
  - [ ] `text() is string`
- [ ] object `InvalidHeader` does `Error`
  - [ ] `InvalidHeader(name is string, reason is string)`
  - [ ] `text() is string`
- [ ] Compare names case-insensitively while preserving HTTP/1 spelling.
- [ ] Preserve field order and duplicates.
- [ ] Reject controls, separators, CR and LF in names.
- [ ] Reject CR, LF and NUL in field values.
- [ ] Trim only optional whitespace permitted by the field grammar.
- [ ] Make `first()` throw and `all()` return empty on absence.
- [ ] Keep `Set-Cookie` as separate field entries.
- [ ] Do not blindly comma-join fields whose grammar forbids it.
- [ ] Lowercase field names when encoding HTTP/2.
- [ ] Reject connection-specific fields in HTTP/2.
- [ ] Do not reuse Stage 21 mail/MIME header parsing.

## `http/body`

- [ ] role `BodyReading` does `Closing`
  - [ ] `async bytes(limit is int) is @byte`
  - [ ] `trailers() is Headers`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] role `BodyOpening`
  - [ ] `length() is BigInt`
  - [ ] `replayable() is bool`
  - [ ] `reading() is BodyReading`
- [ ] object `EmptyBody` does `BodyOpening`
  - [ ] `EmptyBody()`
  - [ ] `length() is BigInt`
  - [ ] `replayable() is bool`
  - [ ] `reading() is BodyReading`
- [ ] object `ByteBody` does `BodyOpening`
  - [ ] `ByteBody(origin is @byte)`
  - [ ] `length() is BigInt`
  - [ ] `replayable() is bool`
  - [ ] `reading() is BodyReading`
- [ ] object `FileBody` does `BodyOpening`
  - [ ] `FileBody(origin is FileEntry)`
  - [ ] `length() is BigInt`
  - [ ] `replayable() is bool`
  - [ ] `reading() is BodyReading`
- [ ] object `OneShotBody` does `BodyOpening`
  - [ ] `OneShotBody(origin is BodyReading)`
  - [ ] `length() is BigInt`
  - [ ] `replayable() is bool`
  - [ ] `reading() is BodyReading`
- [ ] object `UnknownBodyLength` does `Error`
  - [ ] `text() is string`
- [ ] object `BodyAlreadyOpened` does `Error`
  - [ ] `text() is string`
- [ ] object `TrailersUnavailable` does `Error`
  - [ ] `text() is string`
- [ ] Make empty `bytes()` output mean end of body.
- [ ] Reject non-positive read limits.
- [ ] Make byte and file bodies replayable.
- [ ] Make a one-shot body reject its second `open()`.
- [ ] Make `length()` throw `UnknownBodyLength` when framing length is unknown.
- [ ] Expose trailers only after the body reaches its clean end.
- [ ] Make `trailers()` throw `TrailersUnavailable` before clean completion.
- [ ] Make close cancel pending body reads.
- [ ] Decide connection reuse from framing completion, not object destruction.
- [ ] Drain only a bounded unread HTTP/1.1 body before reusing its connection.
- [ ] Reset only the affected HTTP/2 stream when its body closes early.
- [ ] Never buffer an entire unknown body implicitly.

## `http/content-coding`

- [ ] object `ContentCoding`
  - [ ] `ContentCoding(origin is string)`
  - [ ] `string() is string`
- [ ] object `ContentEncodedBody` does `BodyOpening`
  - [ ] `ContentEncodedBody(origin is BodyOpening, coding is ContentCoding)`
  - [ ] `length() is BigInt`
  - [ ] `replayable() is bool`
  - [ ] `reading() is BodyReading`
- [ ] object `ContentDecodedBody` does `BodyOpening`
  - [ ] `ContentDecodedBody(origin is BodyOpening, coding is ContentCoding, maximum is BigInt)`
  - [ ] `length() is BigInt`
  - [ ] `replayable() is bool`
  - [ ] `reading() is BodyReading`
- [ ] object `UnsupportedContentCoding` does `Error`
  - [ ] `UnsupportedContentCoding(origin is ContentCoding)`
  - [ ] `coding() is ContentCoding`
  - [ ] `text() is string`
- [ ] Support identity, gzip, deflate and zstd through Stage 16.
- [ ] Apply stacked content codings in header order and decode in reverse order.
- [ ] Remove `Content-Length` when encoding changes the unknown wire length.
- [ ] Enforce decoded-size and expansion-ratio limits incrementally.
- [ ] Do not advertise Brotli until Stage 16 contains a Brotli codec.
- [ ] Do not silently decode a coding the client did not request.

## `http/message`

- [ ] enum `RequestTargetKind`
  - [ ] `origin`
  - [ ] `absolute`
  - [ ] `authority`
  - [ ] `asterisk`
- [ ] role `RequestTarget`
  - [ ] `kind() is RequestTargetKind`
  - [ ] `url() is Url`
  - [ ] `reference() is UrlReference`
  - [ ] `authority() is UrlAuthority`
  - [ ] `string() is string`
- [ ] object `OriginTarget` does `RequestTarget`
  - [ ] `OriginTarget(origin is UrlReference)`
  - [ ] `kind() is RequestTargetKind`
  - [ ] `url() is Url`
  - [ ] `reference() is UrlReference`
  - [ ] `authority() is UrlAuthority`
  - [ ] `string() is string`
- [ ] object `AbsoluteTarget` does `RequestTarget`
  - [ ] `AbsoluteTarget(origin is Url)`
  - [ ] `kind() is RequestTargetKind`
  - [ ] `url() is Url`
  - [ ] `reference() is UrlReference`
  - [ ] `authority() is UrlAuthority`
  - [ ] `string() is string`
- [ ] object `AuthorityTarget` does `RequestTarget`
  - [ ] `AuthorityTarget(origin is UrlAuthority)`
  - [ ] `kind() is RequestTargetKind`
  - [ ] `url() is Url`
  - [ ] `reference() is UrlReference`
  - [ ] `authority() is UrlAuthority`
  - [ ] `string() is string`
- [ ] object `AsteriskTarget` does `RequestTarget`
  - [ ] `AsteriskTarget()`
  - [ ] `kind() is RequestTargetKind`
  - [ ] `url() is Url`
  - [ ] `reference() is UrlReference`
  - [ ] `authority() is UrlAuthority`
  - [ ] `string() is string`
- [ ] object `Request`
  - [ ] `Request(method is Method, target is RequestTarget, headers is Headers, body is BodyOpening, version is HttpVersion)`
  - [ ] `method() is Method`
  - [ ] `target() is RequestTarget`
  - [ ] `headers() is Headers`
  - [ ] `body() is BodyOpening`
  - [ ] `version() is HttpVersion`
- [ ] object `Response`
  - [ ] `Response(status is Status, headers is Headers, body is BodyOpening, version is HttpVersion)`
  - [ ] `status() is Status`
  - [ ] `headers() is Headers`
  - [ ] `body() is BodyOpening`
  - [ ] `version() is HttpVersion`
- [ ] object `ExchangeDetails`
  - [ ] Do not expose a public constructor.
  - [ ] `endpoints() is ConnectionEndpoints<IpEndpoint>`
  - [ ] `secure() is bool`
  - [ ] `tls() is TlsDetails`
  - [ ] `version() is HttpVersion`
- [ ] object `ExchangeHop`
  - [ ] Do not expose a public constructor.
  - [ ] `request() is Request`
  - [ ] `response() is Response`
- [ ] object `ClientResponse`
  - [ ] Do not expose a public constructor.
  - [ ] `request() is Request`
  - [ ] `response() is Response`
  - [ ] `history() is Sequencing<ExchangeHop>`
  - [ ] `details() is ExchangeDetails`
- [ ] object `RequestContext`
  - [ ] Do not expose a public constructor.
  - [ ] `endpoints() is ConnectionEndpoints<IpEndpoint>`
  - [ ] `secure() is bool`
  - [ ] `tls() is TlsDetails`
  - [ ] `cancelling() is Cancelling`
  - [ ] `parameters() is %string:string`
- [ ] object `WrongRequestTargetKind` does `Error`
  - [ ] `text() is string`
- [ ] object `PlainHttpHasNoTls` does `Error`
  - [ ] `text() is string`
- [ ] Throw `WrongRequestTargetKind` from an invalid target projection.
- [ ] Throw `PlainHttpHasNoTls` from TLS observations on plaintext exchanges.
- [ ] Require origin-form for normal direct server requests.
- [ ] Require absolute-form for forward-proxy requests.
- [ ] Require authority-form for CONNECT.
- [ ] Require asterisk-form only where the method permits it.
- [ ] Keep message objects immutable while body resources remain stateful.
- [ ] Do not expose raw serialized request or status lines as public values.

## `http/date`

- [ ] object `HttpDate`
  - [ ] `HttpDate(origin is Instant)`
  - [ ] `string() is string`
- [ ] object `ParsedHttpDate`
  - [ ] `ParsedHttpDate(origin is string)`
  - [ ] `value() is Instant`
- [ ] object `InvalidHttpDate` does `Error`
  - [ ] `text() is string`
- [ ] Render new dates only in IMF-fixdate form and GMT.
- [ ] Parse the two obsolete HTTP-date forms for compatibility.
- [ ] Reject leap seconds unsupported by Stage 10.
- [ ] Compare HTTP dates at whole-second precision.

## `http/conditional`

- [ ] object `EntityTag`
  - [ ] `EntityTag(value is string, weak is bool)`
  - [ ] `value() is string`
  - [ ] `weak() is bool`
  - [ ] `string() is string`
- [ ] object `EntityTags`
  - [ ] `EntityTags(values is Sequencing<EntityTag>)`
  - [ ] `wildcard() is bool`
  - [ ] `values() is Sequencing<EntityTag>`
  - [ ] `string() is string`
- [ ] object `ParsedEntityTags`
  - [ ] `ParsedEntityTags(origin is string)`
  - [ ] `#projected value() is EntityTags`
- [ ] object `ByteRange`
  - [ ] `ByteRange(start is BigInt, end is BigInt)`
  - [ ] `start() is BigInt`
  - [ ] `end() is BigInt`
  - [ ] `size() is BigInt`
- [ ] object `ByteRanges`
  - [ ] `ByteRanges(origin is string)`
  - [ ] `ranges(size is BigInt) is Sequencing<ByteRange>`
  - [ ] `string() is string`
- [ ] object `InvalidRange` does `Error`
  - [ ] `text() is string`
- [ ] Use strong comparison for `If-Match` and weak comparison for `If-None-Match`.
- [ ] Give entity-tag conditions precedence over date conditions.
- [ ] Normalize satisfiable byte ranges against the selected representation length.
- [ ] Coalesce overlapping ranges before serving them.
- [ ] Reject an excessive number of ranges through server limits.
- [ ] Render multiple ranges through `multipart/byteranges`.
- [ ] Do not apply ranges to an unknown-length representation.

## `http/negotiation`

- [ ] object `WeightedValue`
  - [ ] `WeightedValue(value is string, quality is rat)`
  - [ ] `value() is string`
  - [ ] `quality() is rat`
- [ ] object `AcceptedValues`
  - [ ] `AcceptedValues(origin is string)`
  - [ ] `values() is Sequencing<WeightedValue>`
  - [ ] `preference(available is Sequencing<string>) is string`
- [ ] object `NoAcceptableValue` does `Error`
  - [ ] `text() is string`
- [ ] Validate quality values in `0..1` with at most three decimal places.
- [ ] Preserve client order between equal quality values.
- [ ] Apply specificity before original order where the field defines specificity.
- [ ] Support media type, content coding, charset and language negotiation profiles.
- [ ] Treat an explicit quality zero as unacceptable.
- [ ] Keep locale matching policy in Stage 26.

## `http/cookie`

- [ ] enum `SameSite`
  - [ ] `unspecified`
  - [ ] `strict`
  - [ ] `lax`
  - [ ] `none`
- [ ] enum `CookieFlag`
  - [ ] `secure`
  - [ ] `http_only`
  - [ ] `partitioned`
- [ ] object `CookieExpiry`
  - [ ] `CookieExpiry()`
  - [ ] `CookieExpiry(origin is Instant)`
  - [ ] `persistent() is bool`
  - [ ] `instant() is Instant`
- [ ] object `MissingCookieExpiry` does `Error`
  - [ ] `text() is string`
- [ ] object `CookieAttributes`
  - [ ] `CookieAttributes(domain is string, path is string, expiry is CookieExpiry, same_site is SameSite, flags is Set<CookieFlag>)`
  - [ ] `domain() is string`
  - [ ] `path() is string`
  - [ ] `expiry() is CookieExpiry`
  - [ ] `same_site() is SameSite`
  - [ ] `flags() is Set<CookieFlag>`
- [ ] object `Cookie`
  - [ ] `Cookie(name is string, value is string, attributes is CookieAttributes)`
  - [ ] `name() is string`
  - [ ] `value() is string`
  - [ ] `attributes() is CookieAttributes`
  - [ ] `string() is string`
- [ ] object `CookieContext`
  - [ ] `CookieContext(site is Url, top_level is bool, method is Method)`
  - [ ] `site() is Url`
  - [ ] `top_level() is bool`
  - [ ] `method() is Method`
- [ ] role `CookieStoring`
  - [ ] `cookies(origin is Url, context is CookieContext) is Sequencing<Cookie>`
  - [ ] `store(origin is Url, headers is Headers, context is CookieContext) is void`
  - [ ] `clear() is void`
- [ ] object `MemoryCookieJar` does `CookieStoring`
  - [ ] `MemoryCookieJar(clock is Timing)`
  - [ ] `cookies(origin is Url, context is CookieContext) is Sequencing<Cookie>`
  - [ ] `store(origin is Url, headers is Headers, context is CookieContext) is void`
  - [ ] `clear() is void`
- [ ] object `SavedCookieJar`
  - [ ] `SavedCookieJar(origin is MemoryCookieJar, destination is FileEntry)`
  - [ ] `result() is FileEntry`
- [ ] object `LoadedCookieJar`
  - [ ] `LoadedCookieJar(origin is FileEntry, clock is Timing)`
  - [ ] `result() is MemoryCookieJar`
- [ ] object `InvalidCookie` does `Error`
  - [ ] `InvalidCookie(reason is string)`
  - [ ] `text() is string`
- [ ] Implement RFC 6265bis domain, path, expiry and prefix rules.
- [ ] Make zero-argument `CookieExpiry` represent a session cookie.
- [ ] Make `CookieExpiry.instant()` throw `MissingCookieExpiry` for a session cookie.
- [ ] Give Max-Age precedence over Expires and store one absolute expiry instant.
- [ ] Use a pinned public-suffix list for domain-cookie rejection.
- [ ] Require Secure for SameSite=None, Partitioned and `__Secure-` names.
- [ ] Enforce the `__Host-` prefix rules.
- [ ] Reject control characters and separator injection.
- [ ] Apply per-domain and total cookie limits with deterministic eviction.
- [ ] Order outbound cookies by descending path length then creation time.
- [ ] Never merge multiple Set-Cookie fields.
- [ ] Redact cookie values from default diagnostics.
- [ ] Persist `SavedCookieJar` atomically through an explicit effect.
- [ ] Never persist session cookies.
- [ ] Do not let a cookie jar decide browser-site context implicitly.

## `http/protocol`

- [ ] object `Http2Settings`
  - [ ] `Http2Settings(header_table is int, initial_window is int, frame_size is int, streams is int)`
  - [ ] `header_table() is int`
  - [ ] `initial_window() is int`
  - [ ] `frame_size() is int`
  - [ ] `streams() is int`
- [ ] Implement strict HTTP/1.1 request-line, status-line, field and chunk framing.
- [ ] Require exactly one valid Host field on HTTP/1.1 requests.
- [ ] Require HTTP/2 authority and Host, when both present, to agree.
- [ ] Treat close-delimited response bodies as non-reusable HTTP/1 connections.
- [ ] Reject conflicting Content-Length values unless byte-for-byte identical.
- [ ] Reject Transfer-Encoding plus Content-Length on requests.
- [ ] Require chunked as the final supported transfer coding.
- [ ] Parse chunk extensions within limits and ignore unknown safe extensions.
- [ ] Validate trailer fields against forbidden framing and routing names.
- [ ] Implement the HTTP/2 connection preface and SETTINGS handshake.
- [ ] Implement DATA, HEADERS, CONTINUATION, SETTINGS, WINDOW_UPDATE, RST_STREAM, PING and GOAWAY.
- [ ] Reject PUSH_PROMISE; server push is outside the first API.
- [ ] Implement HPACK static and dynamic tables with bounded memory.
- [ ] Mark sensitive fields never-indexed.
- [ ] Enforce header-list size after decompression.
- [ ] Enforce connection and stream flow-control windows without deadlock.
- [ ] Schedule outbound DATA fairly across writable streams.
- [ ] Distinguish connection errors from stream errors.
- [ ] Stop creating streams after GOAWAY and retry only eligible unprocessed requests.
- [ ] Send RST_STREAM on stream cancellation without cancelling sibling streams.
- [ ] Keep HTTP/2 frame and HPACK types internal.
- [ ] Do not expose pseudo-fields through ordinary `Headers`.

## `http/client`

- [ ] role `Exchanging`
  - [ ] `async response(request is Request) is ClientResponse`
- [ ] role `EndpointAllowing`
  - [ ] `allowed(url is Url, endpoint is IpEndpoint) is bool`
- [ ] object `AllEndpoints` does `EndpointAllowing`
  - [ ] `AllEndpoints()`
  - [ ] `allowed(url is Url, endpoint is IpEndpoint) is bool`
- [ ] object `PublicEndpoints` does `EndpointAllowing`
  - [ ] `PublicEndpoints()`
  - [ ] `allowed(url is Url, endpoint is IpEndpoint) is bool`
- [ ] object `ListedNetworks` does `EndpointAllowing`
  - [ ] `ListedNetworks(origins is Sequencing<IpNetwork>)`
  - [ ] `allowed(url is Url, endpoint is IpEndpoint) is bool`
- [ ] object `HttpLimits`
  - [ ] `HttpLimits()`
  - [ ] `HttpLimits(header_bytes is int, line_bytes is int, body_bytes is BigInt)`
  - [ ] `header_bytes() is int`
  - [ ] `line_bytes() is int`
  - [ ] `body_bytes() is BigInt`
- [ ] object `ClientTimeouts`
  - [ ] `ClientTimeouts(connection is Duration, headers is Duration, body_idle is Duration)`
  - [ ] `connection_timeout() is Duration`
  - [ ] `headers_timeout() is Duration`
  - [ ] `body_idle_timeout() is Duration`
- [ ] Reject negative client timeouts and make zero explicitly disable that deadline.
- [ ] object `ClientPolicy`
  - [ ] `ClientPolicy(protocols is Set<HttpVersion>, limits is HttpLimits, timeouts is ClientTimeouts, connections is int, access is EndpointAllowing)`
  - [ ] `protocols() is Set<HttpVersion>`
  - [ ] `limits() is HttpLimits`
  - [ ] `timeouts() is ClientTimeouts`
  - [ ] `connections() is int`
  - [ ] `access() is EndpointAllowing`
- [ ] object `HttpClient` does `Exchanging, Closing`
  - [ ] `HttpClient(resolver is Resolving, tls is TlsClientPolicy, policy is ClientPolicy, clock is Timing)`
  - [ ] `async response(request is Request) is ClientResponse`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `Decoding` does `Exchanging`
  - [ ] `Decoding(origin is Exchanging, codings is Sequencing<ContentCoding>, maximum is BigInt)`
  - [ ] `async response(request is Request) is ClientResponse`
- [ ] object `HttpFailure` does `Error`
  - [ ] `HttpFailure(message is string)`
  - [ ] `text() is string`
- [ ] Resolve only HTTP and HTTPS absolute targets in the network client.
- [ ] Reject an empty enabled-protocol set and non-positive limits or connection counts.
- [ ] Validate that HTTP protocol preference and TLS ALPN policy do not contradict each other.
- [ ] Check every resolved candidate through `EndpointAllowing` before connecting.
- [ ] Connect only to the checked address, without resolving the name a second time.
- [ ] Make `PublicEndpoints` reject loopback, private, link-local, multicast, unspecified and reserved ranges.
- [ ] Apply endpoint policy again after every redirect and proxy decision.
- [ ] Derive the transport port only from an explicit port or scheme policy.
- [ ] Verify HTTPS through the request URL host, never a redirected stale name.
- [ ] Pool connections by scheme, host, port, proxy and TLS policy.
- [ ] Reuse HTTP/1.1 connections only after clean body completion or bounded drain.
- [ ] Multiplex compatible HTTP/2 exchanges on one connection.
- [ ] Bound idle and total connections per pool key.
- [ ] Evict idle connections deterministically.
- [ ] Retry a stale pooled connection only for replayable idempotent requests.
- [ ] Never retry after observing response bytes.
- [ ] Handle `100 Continue` without sending a large body prematurely.
- [ ] Ignore unsupported informational responses except `101` protocol switching.
- [ ] Expose an empty body for HEAD, 1xx, 204 and 304 responses regardless of framing fields.
- [ ] Treat successful CONNECT as a tunnel transition only inside typed proxy handling.
- [ ] Apply header and body limits incrementally.
- [ ] Apply connect, response-header and body-idle deadlines independently.
- [ ] Return once response headers arrive; stream the body afterward.
- [ ] Make closing the client cancel pending exchanges and close pooled connections.
- [ ] Do not enable redirects, cookies, authentication or cache implicitly.
- [ ] Make `Decoding` advertise only its configured supported codings.
- [ ] Make `Decoding` update Content-Encoding and Content-Length after transformation.
- [ ] Do not add a default User-Agent field; callers and higher-level clients choose one explicitly.

## `http/redirect`

- [ ] object `RedirectPolicy`
  - [ ] `RedirectPolicy(maximum is int, https_downgrade is bool)`
  - [ ] `maximum() is int`
  - [ ] `https_downgrade() is bool`
- [ ] object `Redirecting` does `Exchanging`
  - [ ] `Redirecting(origin is Exchanging, policy is RedirectPolicy)`
  - [ ] `async response(request is Request) is ClientResponse`
- [ ] object `TooManyRedirects` does `Error`
  - [ ] `text() is string`
- [ ] object `RedirectLoop` does `Error`
  - [ ] `text() is string`
- [ ] Resolve relative Location values against the effective request URL.
- [ ] Detect loops from canonical effective URLs.
- [ ] Implement 301, 302 and 303 method rewriting explicitly.
- [ ] Preserve method and body for 307 and 308.
- [ ] Reject replay of a non-replayable body.
- [ ] Strip authorization and cookie fields across origins.
- [ ] Forbid HTTPS-to-HTTP downgrade by default.
- [ ] Preserve every intermediate response in `ClientResponse.history()`.
- [ ] Do not interpret a missing Location as a redirect.

## `http/auth`

- [ ] object `BasicCredentials`
  - [ ] `BasicCredentials(username is string, password is SecretBytes)`
  - [ ] `username() is string`
- [ ] object `DigestCredentials`
  - [ ] `DigestCredentials(username is string, password is SecretBytes)`
  - [ ] `username() is string`
- [ ] object `BearerAuthorization`
  - [ ] `BearerAuthorization(token is SecretBytes)`
- [ ] object `BasicExchanging` does `Exchanging`
  - [ ] `BasicExchanging(origin is Exchanging, scope is Url, credentials is BasicCredentials)`
  - [ ] `async response(request is Request) is ClientResponse`
- [ ] object `DigestExchanging` does `Exchanging`
  - [ ] `DigestExchanging(origin is Exchanging, scope is Url, credentials is DigestCredentials, random is SecureRandom)`
  - [ ] `async response(request is Request) is ClientResponse`
- [ ] object `BearerExchanging` does `Exchanging`
  - [ ] `BearerExchanging(origin is Exchanging, scope is Url, authorization is BearerAuthorization)`
  - [ ] `async response(request is Request) is ClientResponse`
- [ ] Limit preemptive credentials to the configured origin and path scope.
- [ ] Strip credentials on every cross-origin redirect.
- [ ] Support Basic only over verified TLS by default.
- [ ] Support Digest SHA-256 and SHA-512-256 with fresh client nonces.
- [ ] Do not add new MD5 Digest credentials; parse legacy challenges only under explicit compatibility policy.
- [ ] Prevent bearer and password values from entering diagnostics.
- [ ] Do not implement NTLM or negotiate platform credentials in the portable stdlib.

## `http/cookie-client`

- [ ] role `CookieContextualizing`
  - [ ] `context(request is Request) is CookieContext`
- [ ] object `FirstPartyCookies` does `CookieContextualizing`
  - [ ] `FirstPartyCookies()`
  - [ ] `context(request is Request) is CookieContext`
- [ ] object `CookieExchanging` does `Exchanging`
  - [ ] `CookieExchanging(origin is Exchanging, jar is CookieStoring, contexts is CookieContextualizing)`
  - [ ] `async response(request is Request) is ClientResponse`
- [ ] Make `FirstPartyCookies` explicitly treat every request as a top-level first-party request.
- [ ] Select matching cookies immediately before each exchange.
- [ ] Store every Set-Cookie field from the effective response URL.
- [ ] Re-evaluate cookies independently for every redirect hop.
- [ ] Preserve an explicit caller Cookie field only under an explicit conflict rule.
- [ ] Serialize concurrent cookie-jar changes safely.

## `http/proxy`

- [ ] object `ProxyPolicy`
  - [ ] `ProxyPolicy(origin is Url)`
  - [ ] `ProxyPolicy(origin is Url, credentials is BasicCredentials)`
  - [ ] `url() is Url`
  - [ ] `authenticated() is bool`
  - [ ] `credentials() is BasicCredentials`
- [ ] object `MissingProxyCredentials` does `Error`
  - [ ] `text() is string`
- [ ] object `Proxied` does `Exchanging, Closing`
  - [ ] `Proxied(proxy is ProxyPolicy, resolver is Resolving, tls is TlsClientPolicy, policy is ClientPolicy, clock is Timing)`
  - [ ] `async response(request is Request) is ClientResponse`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `EnvironmentProxy`
  - [ ] `EnvironmentProxy(environment is Environment)`
  - [ ] `proxy(origin is Url) is ProxyPolicy`
- [ ] object `MissingProxy` does `Error`
  - [ ] `text() is string`
- [ ] Send absolute-form targets to a plaintext HTTP proxy.
- [ ] Establish HTTPS through CONNECT before the origin TLS handshake.
- [ ] Verify the origin certificate after tunneling.
- [ ] Keep proxy authorization separate from origin authorization.
- [ ] Make `ProxyPolicy.credentials()` throw `MissingProxyCredentials` when absent.
- [ ] Match no-proxy host suffixes, exact IP addresses and CIDR networks explicitly.
- [ ] Apply `ClientPolicy.access()` to the actual proxy endpoint.
- [ ] Document that a remotely resolving proxy owns origin-address filtering after CONNECT.
- [ ] Parse environment proxy variables only through `EnvironmentProxy`.
- [ ] Never trust ambient proxy variables unless the caller constructs `EnvironmentProxy`.
- [ ] Do not support SOCKS until a separate typed transport module exists.

## `http/cache`

- [ ] role `ResponseCaching`
  - [ ] `response(request is Request) is ClientResponse`
  - [ ] `store(request is Request, response is ClientResponse) is void`
  - [ ] `invalidate(request is Request) is void`
- [ ] object `MemoryHttpCache` does `ResponseCaching`
  - [ ] `MemoryHttpCache(clock is Timing, maximum is BigInt)`
  - [ ] `response(request is Request) is ClientResponse`
  - [ ] `store(request is Request, response is ClientResponse) is void`
  - [ ] `invalidate(request is Request) is void`
- [ ] object `Caching` does `Exchanging`
  - [ ] `Caching(origin is Exchanging, cache is ResponseCaching, clock is Timing)`
  - [ ] `async response(request is Request) is ClientResponse`
- [ ] object `MissingCachedResponse` does `Error`
  - [ ] `text() is string`
- [ ] Implement RFC 9111 freshness, age and validation calculations.
- [ ] Respect Cache-Control, Expires, Date, Age, Vary and validators.
- [ ] Never store `no-store` responses.
- [ ] Require explicit policy before storing authenticated responses.
- [ ] Cache only a response body that reaches clean completion within limits.
- [ ] Revalidate stale entries through conditional requests.
- [ ] Merge a valid 304 response without replacing the stored body.
- [ ] Invalidate unsafe-method targets after successful responses.
- [ ] Collapse concurrent revalidation for the same cache key.
- [ ] Keep shared-cache behavior out of the private client cache.
- [ ] Do not cache partial or protocol-failed responses.

## `http/websocket`

- [ ] enum `WebSocketMessageKind`
  - [ ] `text`
  - [ ] `binary`
- [ ] role `WebSocketMessage`
  - [ ] `kind() is WebSocketMessageKind`
  - [ ] `text() is string`
  - [ ] `bytes() is @byte`
- [ ] object `TextWebSocketMessage` does `WebSocketMessage`
  - [ ] `TextWebSocketMessage(origin is string)`
  - [ ] `kind() is WebSocketMessageKind`
  - [ ] `text() is string`
  - [ ] `bytes() is @byte`
- [ ] object `BinaryWebSocketMessage` does `WebSocketMessage`
  - [ ] `BinaryWebSocketMessage(origin is @byte)`
  - [ ] `kind() is WebSocketMessageKind`
  - [ ] `text() is string`
  - [ ] `bytes() is @byte`
- [ ] object `WebSocketLimits`
  - [ ] `WebSocketLimits(message_bytes is BigInt, frame_bytes is int, queue is int)`
  - [ ] `message_bytes() is BigInt`
  - [ ] `frame_bytes() is int`
  - [ ] `queue() is int`
- [ ] object `WebSocketConnection`
  - [ ] Do not expose a public constructor.
  - [ ] `async message() is WebSocketMessage`
  - [ ] `async send(origin is WebSocketMessage) is WebSocketConnection`
  - [ ] `async ping(origin is @byte) is WebSocketConnection`
  - [ ] `async close(code is int, reason is string) is WebSocketConnection`
  - [ ] `closed() is bool`
- [ ] object `WebSocketMessageIsNotText` does `Error`
  - [ ] `text() is string`
- [ ] object `WebSocketMessageIsNotBinary` does `Error`
  - [ ] `text() is string`
- [ ] role `WebSocketHandling`
  - [ ] `async handle(connection is WebSocketConnection, request is Request, context is RequestContext) is WebSocketHandling`
- [ ] role `WebSocketAllowing`
  - [ ] `allowed(request is Request) is bool`
- [ ] object `SameOriginWebSockets` does `WebSocketAllowing`
  - [ ] `SameOriginWebSockets()`
  - [ ] `allowed(request is Request) is bool`
- [ ] object `WebSocketUpgrade`
  - [ ] `WebSocketUpgrade(protocols is Sequencing<string>, handling is WebSocketHandling, limits is WebSocketLimits, access is WebSocketAllowing)`
  - [ ] `protocols() is Sequencing<string>`
  - [ ] `handling() is WebSocketHandling`
  - [ ] `limits() is WebSocketLimits`
  - [ ] `access() is WebSocketAllowing`
- [ ] object `WebSocketConnecting`
  - [ ] `WebSocketConnecting(origin is Exchanging, url is Url, protocols is Sequencing<string>, headers is Headers, random is SecureRandom, limits is WebSocketLimits)`
  - [ ] `async connection() is WebSocketConnection`
- [ ] object `WebSocketClosed` does `Error`
  - [ ] Do not expose a public constructor.
  - [ ] `code() is int`
  - [ ] `reason() is string`
  - [ ] `text() is string`
- [ ] object `WebSocketFailure` does `Error`
  - [ ] `WebSocketFailure(reason is string)`
  - [ ] `text() is string`
- [ ] Validate HTTP Upgrade, Connection, version, nonce and accept fields.
- [ ] Check server-side Origin policy before sending the switching response.
- [ ] Make `SameOriginWebSockets` reject absent, malformed and cross-origin browser origins.
- [ ] Generate every client nonce from `SecureRandom`.
- [ ] Mask every client-to-server frame and reject masked server frames.
- [ ] Require minimal payload-length encoding.
- [ ] Reassemble fragmented data messages within limits.
- [ ] Validate text messages and close reasons as UTF-8.
- [ ] Throw a typed variant error from the wrong `WebSocketMessage` projection.
- [ ] Handle ping, pong and close frames internally while fragments are open.
- [ ] Enforce control-frame finality and 125-byte payload limit.
- [ ] Validate registered close codes and reason length.
- [ ] Accept at most one server-selected subprotocol and require that the client offered it.
- [ ] Complete the close handshake once and then close the transport.
- [ ] Apply bounded queues and backpressure to outgoing messages.
- [ ] Cancel pending send and receive operations when closing.
- [ ] Negotiate per-message deflate only with explicit policy and expansion limits.
- [ ] Support HTTP/1.1 Upgrade first and RFC 8441 extended CONNECT after HTTP/2 is stable.
- [ ] Exempt an accepted WebSocket handler from ordinary HTTP handler deadlines while retaining server cancellation.
- [ ] Do not expose raw frames in the first public API.

## `http/server`

- [ ] enum `HandlingKind`
  - [ ] `response`
  - [ ] `websocket`
- [ ] role `HandlingResult`
  - [ ] `kind() is HandlingKind`
  - [ ] `response() is Response`
  - [ ] `websocket() is WebSocketUpgrade`
- [ ] object `ResponseHandling` does `HandlingResult`
  - [ ] `ResponseHandling(origin is Response)`
  - [ ] `kind() is HandlingKind`
  - [ ] `response() is Response`
  - [ ] `websocket() is WebSocketUpgrade`
- [ ] object `WebSocketHandlingResult` does `HandlingResult`
  - [ ] `WebSocketHandlingResult(origin is WebSocketUpgrade)`
  - [ ] `kind() is HandlingKind`
  - [ ] `response() is Response`
  - [ ] `websocket() is WebSocketUpgrade`
- [ ] object `WrongHandlingKind` does `Error`
  - [ ] `text() is string`
- [ ] role `Handling`
  - [ ] `async handling(request is Request, context is RequestContext) is HandlingResult`
- [ ] object `ServerTimeouts`
  - [ ] `ServerTimeouts(headers is Duration, body_idle is Duration, keep_alive is Duration, handling is Duration)`
  - [ ] `headers_timeout() is Duration`
  - [ ] `body_idle_timeout() is Duration`
  - [ ] `keep_alive_timeout() is Duration`
  - [ ] `handling_timeout() is Duration`
- [ ] Reject negative server timeouts and make zero explicitly disable that deadline.
- [ ] object `ServerPolicy`
  - [ ] `ServerPolicy(protocols is Set<HttpVersion>, limits is HttpLimits, timeouts is ServerTimeouts, connections is int, ranges is int)`
  - [ ] `protocols() is Set<HttpVersion>`
  - [ ] `limits() is HttpLimits`
  - [ ] `timeouts() is ServerTimeouts`
  - [ ] `connections() is int`
  - [ ] `ranges() is int`
- [ ] object `HttpServer` does `Closing`
  - [ ] `HttpServer(origin is StreamAcceptance, handling is Handling, policy is ServerPolicy, clock is Timing)`
  - [ ] `async serve() is HttpServer`
  - [ ] `local_endpoint() is IpEndpoint`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `ServerDrain`
  - [ ] `ServerDrain(origin is HttpServer, limit is Duration, clock is Timing)`
  - [ ] `async drain() is ServerDrain`
- [ ] object `BadRequest` does `Error`
  - [ ] `BadRequest(message is string)`
  - [ ] `text() is string`
- [ ] Parse request framing before invoking the handler.
- [ ] Reject an empty enabled-protocol set, negative timeouts and non-positive resource limits.
- [ ] Throw `WrongHandlingKind` from an invalid handling-result projection.
- [ ] Reject request smuggling ambiguities around Content-Length and Transfer-Encoding.
- [ ] Reject obsolete line folding and whitespace before a colon.
- [ ] Enforce request-target, header, line and body limits incrementally.
- [ ] Apply request-header, body-idle, keep-alive and handler deadlines independently.
- [ ] Send `100 Continue` only after validating headers and handler willingness.
- [ ] Drain or close rejected request bodies deterministically.
- [ ] Serialize HTTP/1.1 pipelined responses in request order.
- [ ] Run independent HTTP/2 streams in a structured future scope.
- [ ] Cancel a handler when its peer resets the stream or disconnects.
- [ ] Convert uncaught handler failures to a minimal 500 only before headers are sent.
- [ ] Close the stream or connection after a failure occurring mid-response.
- [ ] Add Date and Server fields only through explicit server policy.
- [ ] Make `HttpServer.close()` stop accepting and cancel active work immediately.
- [ ] Make `ServerDrain` stop accepting, wait for active work, then cancel at its deadline.
- [ ] Do not expose connection hijacking except the typed WebSocket upgrade.

## `http/routing`

- [ ] object `RoutePattern`
  - [ ] `RoutePattern(origin is string)`
  - [ ] `string() is string`
- [ ] object `Route`
  - [ ] `Route(method is Method, pattern is RoutePattern, handling is Handling)`
  - [ ] `method() is Method`
  - [ ] `pattern() is RoutePattern`
  - [ ] `handling() is Handling`
- [ ] object `Routes` does `Handling`
  - [ ] `Routes(routes is Sequencing<Route>, fallback is Handling)`
  - [ ] `async handling(request is Request, context is RequestContext) is HandlingResult`
- [ ] object `InvalidRoute` does `Error`
  - [ ] `InvalidRoute(reason is string)`
  - [ ] `text() is string`
- [ ] Match decoded path segments without decoding `%2F` into a separator.
- [ ] Support literal, named and final wildcard segments.
- [ ] Reject duplicate parameter names in one pattern.
- [ ] Match routes in declaration order only after validating ambiguity.
- [ ] Return 405 with an Allow field when the path matches but the method does not.
- [ ] Let HEAD use GET routing while suppressing the response body on the wire.
- [ ] Put matched parameters into the successor `RequestContext`.
- [ ] Keep host routing as an explicit outer `Handling` decorator.
- [ ] Do not add mutable route registration.

## `http/static`

- [ ] object `StaticPolicy`
  - [ ] `StaticPolicy(indexes is Sequencing<string>, listing is bool, precompressed is bool)`
  - [ ] `indexes() is Sequencing<string>`
  - [ ] `listing() is bool`
  - [ ] `precompressed() is bool`
- [ ] object `StaticFiles` does `Handling`
  - [ ] `StaticFiles(root is Directory, media is MediaIdentifying, policy is StaticPolicy)`
  - [ ] `async handling(request is Request, context is RequestContext) is HandlingResult`
- [ ] Resolve request paths beneath one already resolved root.
- [ ] Reject traversal, encoded separators and symlink escape.
- [ ] Serve only regular files.
- [ ] Generate ETag, Last-Modified and Content-Length deterministically.
- [ ] Evaluate conditional requests before opening the response body.
- [ ] Support single and multipart byte ranges.
- [ ] Negotiate precompressed siblings only when their metadata matches the source policy.
- [ ] Escape every directory-listing name and disable listing by default.
- [ ] Do not execute CGI, templates or files based on extension.

## `http/form`

- [ ] object `FormLimits`
  - [ ] `FormLimits(fields is int, files is int, field_bytes is BigInt, file_bytes is BigInt, total_bytes is BigInt)`
  - [ ] `fields() is int`
  - [ ] `files() is int`
  - [ ] `field_bytes() is BigInt`
  - [ ] `file_bytes() is BigInt`
  - [ ] `total_bytes() is BigInt`
- [ ] object `FormPart`
  - [ ] Do not expose a public constructor.
  - [ ] `name() is string`
  - [ ] `file() is bool`
  - [ ] `filename() is string`
  - [ ] `headers() is MessageHeaders`
  - [ ] `body() is BodyReading`
- [ ] object `MultipartParts` does `Closing`
  - [ ] Do not expose a public constructor.
  - [ ] `async part() is FormPart`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `ParsedMultipartForm`
  - [ ] `ParsedMultipartForm(origin is BodyReading, boundary is string, limits is FormLimits)`
  - [ ] `parts() is MultipartParts`
- [ ] object `MultipartFormEnded` does `Error`
  - [ ] `text() is string`
- [ ] object `InvalidMultipartForm` does `Error`
  - [ ] `InvalidMultipartForm(reason is string)`
  - [ ] `text() is string`
- [ ] Read each part body before advancing to the next part.
- [ ] Make `part()` throw `MultipartFormEnded` after the closing boundary.
- [ ] Enforce field, file, individual and aggregate limits while streaming.
- [ ] Treat filename as an untrusted display name.
- [ ] Spool files only through an explicit Stage 11 temporary-file adapter.
- [ ] Decode text fields only through their declared supported charset.
- [ ] Reuse Stage 21 form URL decoding for `application/x-www-form-urlencoded`.
- [ ] Do not materialize all uploaded files in memory.

## `http/sse`

- [ ] object `ServerEvent`
  - [ ] `ServerEvent(data is string)`
  - [ ] `ServerEvent(data is string, event is string, identifier is string, retry is Duration)`
  - [ ] `data() is string`
  - [ ] `event() is string`
  - [ ] `identifier() is string`
  - [ ] `retry() is Duration`
- [ ] object `EventBody` does `BodyOpening`
  - [ ] `EventBody(origin is Receiving<ServerEvent>)`
  - [ ] `length() is BigInt`
  - [ ] `replayable() is bool`
  - [ ] `reading() is BodyReading`
- [ ] object `ServerEvents` does `Closing`
  - [ ] `ServerEvents(origin is BodyReading)`
  - [ ] `async event() is ServerEvent`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `EventStreamEnded` does `Error`
  - [ ] `text() is string`
- [ ] object `MissingEventRetry` does `Error`
  - [ ] `text() is string`
- [ ] Encode every data line with its own `data:` field.
- [ ] Reject CR, LF and NUL in event names and identifiers.
- [ ] Parse CRLF, CR and LF input line endings.
- [ ] Ignore comment lines while allowing them as keepalives.
- [ ] Update the last event identifier only after dispatch.
- [ ] Clamp invalid or negative retry values.
- [ ] Make `ServerEvent.retry()` throw `MissingEventRetry` when no retry was supplied.
- [ ] Disable intermediary buffering through explicit response headers.
- [ ] Keep automatic reconnection as a separate client policy.

## `http/robots`

- [ ] object `RobotsPolicy`
  - [ ] `RobotsPolicy(origin is string)`
  - [ ] `allowed(agent is string, url is Url) is bool`
  - [ ] `delay(agent is string) is Duration`
  - [ ] `sitemaps() is Sequencing<Url>`
- [ ] object `FetchedRobots`
  - [ ] `FetchedRobots(origin is Exchanging, site is Url, agent is string)`
  - [ ] `async policy() is RobotsPolicy`
- [ ] object `MissingCrawlDelay` does `Error`
  - [ ] `text() is string`
- [ ] Match the most specific user-agent group.
- [ ] Resolve equal-specificity allow/disallow rules in favor of allow.
- [ ] Match percent-encoded URL paths without unsafe double decoding.
- [ ] Make `delay()` throw `MissingCrawlDelay` when absent.
- [ ] Accept absolute HTTP and HTTPS sitemap URLs only.
- [ ] Bound robots file size before parsing.
- [ ] Leave fetching, caching and redirect policy explicit through `Exchanging`.

## `http/xmlrpc` — low priority

- [ ] enum `XmlRpcKind`
  - [ ] `integer`
  - [ ] `boolean`
  - [ ] `text`
  - [ ] `floating`
  - [ ] `datetime`
  - [ ] `binary`
  - [ ] `array`
  - [ ] `structure`
  - [ ] `nil`
- [ ] role `XmlRpcValue`
  - [ ] `kind() is XmlRpcKind`
- [ ] object `XmlRpcInteger` does `XmlRpcValue`
  - [ ] `XmlRpcInteger(value is int)`
  - [ ] `kind() is XmlRpcKind`
  - [ ] `value() is int`
- [ ] object `XmlRpcBoolean` does `XmlRpcValue`
  - [ ] `XmlRpcBoolean(value is bool)`
  - [ ] `kind() is XmlRpcKind`
  - [ ] `value() is bool`
- [ ] object `XmlRpcText` does `XmlRpcValue`
  - [ ] `XmlRpcText(value is string)`
  - [ ] `kind() is XmlRpcKind`
  - [ ] `value() is string`
- [ ] object `XmlRpcFloat` does `XmlRpcValue`
  - [ ] `XmlRpcFloat(value is float)`
  - [ ] `kind() is XmlRpcKind`
  - [ ] `value() is float`
- [ ] object `XmlRpcDateTime` does `XmlRpcValue`
  - [ ] `XmlRpcDateTime(value is LocalDateTime)`
  - [ ] `kind() is XmlRpcKind`
  - [ ] `value() is LocalDateTime`
- [ ] object `XmlRpcBinary` does `XmlRpcValue`
  - [ ] `XmlRpcBinary(value is @byte)`
  - [ ] `kind() is XmlRpcKind`
  - [ ] `value() is @byte`
- [ ] object `XmlRpcArray` does `XmlRpcValue`
  - [ ] `XmlRpcArray(values is Sequencing<XmlRpcValue>)`
  - [ ] `kind() is XmlRpcKind`
  - [ ] `values() is Sequencing<XmlRpcValue>`
- [ ] object `XmlRpcStruct` does `XmlRpcValue`
  - [ ] `XmlRpcStruct(values is %string:XmlRpcValue)`
  - [ ] `kind() is XmlRpcKind`
  - [ ] `values() is %string:XmlRpcValue`
- [ ] object `XmlRpcNil` does `XmlRpcValue`
  - [ ] `XmlRpcNil()`
  - [ ] `kind() is XmlRpcKind`
- [ ] object `XmlRpcCall`
  - [ ] `XmlRpcCall(method is string, parameters is Sequencing<XmlRpcValue>)`
  - [ ] `method() is string`
  - [ ] `parameters() is Sequencing<XmlRpcValue>`
- [ ] object `XmlRpcFault`
  - [ ] `XmlRpcFault(code is int, message is string)`
  - [ ] `code() is int`
  - [ ] `text() is string`
- [ ] enum `XmlRpcResultKind`
  - [ ] `success`
  - [ ] `fault`
- [ ] role `XmlRpcResult`
  - [ ] `kind() is XmlRpcResultKind`
  - [ ] `value() is XmlRpcValue`
  - [ ] `fault() is XmlRpcFault`
- [ ] object `XmlRpcSuccess` does `XmlRpcResult`
  - [ ] `XmlRpcSuccess(origin is XmlRpcValue)`
  - [ ] `kind() is XmlRpcResultKind`
  - [ ] `value() is XmlRpcValue`
  - [ ] `fault() is XmlRpcFault`
- [ ] object `XmlRpcFailure` does `XmlRpcResult`
  - [ ] `XmlRpcFailure(origin is XmlRpcFault)`
  - [ ] `kind() is XmlRpcResultKind`
  - [ ] `value() is XmlRpcValue`
  - [ ] `fault() is XmlRpcFault`
- [ ] object `XmlRpcLimits`
  - [ ] `XmlRpcLimits(depth is int, values is int, text_bytes is BigInt, binary_bytes is BigInt)`
  - [ ] `depth() is int`
  - [ ] `values() is int`
  - [ ] `text_bytes() is BigInt`
  - [ ] `binary_bytes() is BigInt`
- [ ] object `XmlRpcClient`
  - [ ] `XmlRpcClient(origin is Exchanging, endpoint is Url, limits is XmlRpcLimits)`
  - [ ] `async result(origin is XmlRpcCall) is XmlRpcResult`
- [ ] role `XmlRpcCalling`
  - [ ] `async result(origin is XmlRpcCall) is XmlRpcResult`
- [ ] object `XmlRpcHandling` does `Handling`
  - [ ] `XmlRpcHandling(path is string, calling is XmlRpcCalling, limits is XmlRpcLimits)`
  - [ ] `async handling(request is Request, context is RequestContext) is HandlingResult`
- [ ] object `InvalidXmlRpc` does `Error`
  - [ ] `InvalidXmlRpc(reason is string)`
  - [ ] `text() is string`
- [ ] Restrict standard integer values to the XML-RPC 32-bit signed range.
- [ ] Keep XML-RPC date-time values timezone-free.
- [ ] Enable nil only as an explicitly negotiated extension.
- [ ] Encode binary values through Stage 06 Base64.
- [ ] Parse XML with DTDs, external entities and entity expansion disabled.
- [ ] Enforce depth, value-count, text and decoded-binary limits incrementally.
- [ ] Accept only POST with the configured path on the server adapter.
- [ ] Require a successful HTTP response before parsing an XML-RPC result.
- [ ] Return typed XML-RPC faults separately from HTTP and XML failures.
- [ ] Throw a typed variant error from the wrong `XmlRpcResult` projection.
- [ ] Do not expose arbitrary XML-RPC extension tags in the first API.

## Stage 22 verification

- [ ] Add RFC 9110, 9111 and 9112 message and caching vectors.
- [ ] Add request-smuggling cases for Content-Length, Transfer-Encoding and whitespace variants.
- [ ] Add chunked framing, trailers, early close and bounded-drain cases.
- [ ] Add redirect method, replayability, downgrade and credential-stripping cases.
- [ ] Add cookie domain, path, expiry, prefix, public-suffix and eviction cases.
- [ ] Add conditional, range and multipart-range response cases.
- [ ] Add deterministic proxy and CONNECT loopback servers.
- [ ] Add HTTP/1.0 and HTTP/1.1 connection-reuse tests.
- [ ] Add RFC 7541 HPACK vectors.
- [ ] Add HTTP/2 flow-control, cancellation, GOAWAY and multiplexing tests.
- [ ] Add handler failure tests before and after response headers.
- [ ] Add graceful and immediate server shutdown tests.
- [ ] Add route ambiguity, encoded-separator and method-mismatch tests.
- [ ] Add static-file traversal, symlink escape, conditional and range tests.
- [ ] Add streaming multipart upload limit tests.
- [ ] Add RFC 6455 WebSocket framing and handshake vectors.
- [ ] Add fragmented-message, control-frame, UTF-8 and close-race cases.
- [ ] Add SSE parsing, keepalive and cancellation tests.
- [ ] Add robots matching and size-limit tests.
- [ ] Add cancellation and timeout races at every client, server and body suspension point.
- [ ] Test HTTP/1.1 and HTTP/2 interoperability against external implementations.
- [ ] Add one Perl-level regression file per public module group.
- [ ] Add runnable client, server, upload, WebSocket and SSE examples.
- [ ] Run `git diff --check`.

## Stage 22 exclusions

- [ ] Do not derive the v3 API from existing `stdlib/http/*` modules.
- [ ] Do not add HTTP methods to builtin strings or byte arrays.
- [ ] Do not buffer arbitrary request or response bodies implicitly.
- [ ] Do not enable redirects, cookies, authentication, proxies or caching implicitly.
- [ ] Do not forward credentials or cookies across origins by default.
- [ ] Do not expose raw socket, TLS, HTTP/2 frame or HPACK state.
- [ ] Do not implement CGI or extension-based code execution.
- [ ] Do not add HTTP/3 until a reviewed QUIC and TLS 1.3 transport exists.
- [ ] Do not add WebTransport before HTTP/3.
- [ ] Keep generic RPC frameworks outside the standard HTTP layer.
- [ ] Keep templates and locale in Stage 26.
- [ ] Keep tracing and metrics integration in Stage 28.

---

# Stage 23 — databases

## `runtime/database` substrate

- [x] Reuse canonical native-symbol registration and signature validation for optional database providers.
- [x] Reuse provider-owned opaque handles with nominal-type, authority and generation validation.
- [x] Reuse exactly-once native-handle closing and runtime-shutdown finalization.
- [x] Reuse normalized native failures with stable provider domains such as `optional.sqlite`.
- [x] Reuse native poll-source registration for scheduler-visible readiness.
- [x] Keep the runtime free of SQL, row and transaction value objects.
- [ ] Add an optional `optional.sqlite` provider; no database provider exists yet.
- [ ] Run every potentially blocking SQLite operation in an isolated worker owned by the connection.
- [ ] Communicate with that worker through bounded request and response messages.
- [ ] Make worker completion visible to the Stage 19 scheduler through a pollable handle.
- [ ] Add cancellation that invokes `sqlite3_interrupt()` in the owning worker.
- [ ] Keep SQLite handles, callbacks and allocator-owned memory inside their owning worker.
- [ ] Transfer only validated scalar values and owned byte copies across the worker boundary.
- [ ] Reject stale request identifiers and replies from an earlier handle generation.
- [ ] Translate SQLite result codes into stable native failure codes without leaking SQL parameters.
- [ ] Shut down each worker and finalize every statement, blob and connection exactly once.
- [ ] Prove that a slow query never blocks unrelated tasks in the cooperative scheduler.

## `db/value`

- [ ] enum `DbValueKind`
  - [ ] `null`
  - [ ] `boolean`
  - [ ] `integer`
  - [ ] `floating`
  - [ ] `decimal`
  - [ ] `text`
  - [ ] `bytes`
- [ ] role `DbValue`
  - [ ] Keep the noun role name because this is the closed portable database-value category.
  - [ ] `kind() is DbValueKind`
- [ ] object `DbNull` does `DbValue`
  - [ ] `DbNull()`
  - [ ] `kind() is DbValueKind`
- [ ] object `DbBoolean` does `DbValue`
  - [ ] `DbBoolean(value is bool)`
  - [ ] `kind() is DbValueKind`
  - [ ] `value() is bool`
- [ ] object `DbInteger` does `DbValue`
  - [ ] `DbInteger(value is BigInt)`
  - [ ] `kind() is DbValueKind`
  - [ ] `value() is BigInt`
- [ ] object `DbFloating` does `DbValue`
  - [ ] `DbFloating(value is float)`
  - [ ] `kind() is DbValueKind`
  - [ ] `value() is float`
- [ ] object `DbDecimal` does `DbValue`
  - [ ] `DbDecimal(value is Decimal)`
  - [ ] `kind() is DbValueKind`
  - [ ] `value() is Decimal`
- [ ] object `DbText` does `DbValue`
  - [ ] `DbText(value is string)`
  - [ ] `kind() is DbValueKind`
  - [ ] `value() is string`
- [ ] object `DbBytes` does `DbValue`
  - [ ] `DbBytes(value is @byte)`
  - [ ] `kind() is DbValueKind`
  - [ ] `value() is @byte`
- [ ] object `WrongDbValue` does `Error`
  - [ ] `WrongDbValue(expected is DbValueKind, actual is DbValueKind)`
  - [ ] `expected_kind() is DbValueKind`
  - [ ] `actual_kind() is DbValueKind`
  - [ ] `text() is string`
- [ ] Keep every database value immutable.
- [ ] Preserve SQL `NULL` as `DbNull`; never substitute an empty string, zero or a missing collection item.
- [ ] Reject a projection whose `DbValueKind` does not match.
- [ ] Keep dates, times, UUIDs, JSON and vendor types out of the portable value sum.
- [ ] Encode those domain values explicitly before binding instead of installing ambient adapters.

## `db/statement`

- [ ] object `Statement`
  - [ ] `Statement(text is string)`
  - [ ] `Statement(text is string, values is Sequencing<DbValue>)`
  - [ ] `text() is string`
  - [ ] `values() is Sequencing<DbValue>`
- [ ] object `Execution`
  - [ ] Do not expose a public constructor.
  - [ ] `affected_known() is bool`
  - [ ] `affected_rows() is BigInt`
- [ ] Require `Statement.text()` to contain exactly one statement.
- [ ] Keep placeholder syntax driver-specific; SQLite uses its native positional `?` placeholders.
- [ ] Match placeholders only through the database parser, never through string replacement.
- [ ] Bind every `Statement.values()` item as data.
- [ ] Make `Execution.affected_rows()` throw `AffectedRowsUnknown` when the provider cannot report it.
- [ ] Use a row-producing statement through `rows()`, including `RETURNING` and generated-key queries.
- [ ] Do not expose a generic last-insert identifier with backend-dependent meaning.

## `db/row`

- [ ] object `Column`
  - [ ] Do not expose a public constructor.
  - [ ] `index() is int`
  - [ ] `name() is string`
  - [ ] `declaration() is string`
- [ ] object `Row` does `Sizing`
  - [ ] Do not expose a public constructor.
  - [ ] `size() is int`
  - [ ] `item(index is int) is DbValue`
  - [ ] `value(name is string) is DbValue`
  - [ ] `columns() is Sequencing<Column>`
- [ ] role `RowReading` does `Closing`
  - [ ] `async row() is Row`
  - [ ] `columns() is Sequencing<Column>`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `RowsEnded` does `Error`
  - [ ] `text() is string`
- [ ] object `MissingColumn` does `Error`
  - [ ] `MissingColumn(name is string)`
  - [ ] `name() is string`
  - [ ] `text() is string`
- [ ] object `AmbiguousColumn` does `Error`
  - [ ] `AmbiguousColumn(name is string)`
  - [ ] `name() is string`
  - [ ] `text() is string`
- [ ] Preserve column order and duplicate column names.
- [ ] Make `Row.named()` fail when the name is missing or ambiguous.
- [ ] Make exhaustion a typed `RowsEnded` result of `row()`.
- [ ] Do not model database rows with immutable `Cursor<Row>`: fetching can suspend and fail.
- [ ] Fetch rows in bounded batches while exposing one row at a time.
- [ ] Make `close()` discard unread rows and release the underlying statement.
- [ ] Make cancellation close the reader and leave its connection in a defined state or mark it broken.

## `db/error`

- [ ] enum `DbFailureKind`
  - [ ] `connection`
  - [ ] `constraint`
  - [ ] `syntax`
  - [ ] `data`
  - [ ] `permission`
  - [ ] `busy`
  - [ ] `deadlock`
  - [ ] `serialization`
  - [ ] `timeout`
  - [ ] `cancelled`
  - [ ] `resource`
  - [ ] `unsupported`
  - [ ] `unexpected`
- [ ] enum `DbOperation`
  - [ ] `connecting`
  - [ ] `preparing`
  - [ ] `executing`
  - [ ] `fetching`
  - [ ] `committing`
  - [ ] `rolling_back`
  - [ ] `closing`
- [ ] object `DbFailure` does `Error`
  - [ ] `DbFailure(kind is DbFailureKind, operation is DbOperation, code is string, message is string)`
  - [ ] `kind() is DbFailureKind`
  - [ ] `operation() is DbOperation`
  - [ ] `code() is string`
  - [ ] `text() is string`
- [ ] object `AffectedRowsUnknown` does `Error`
  - [ ] `text() is string`
- [ ] Keep `code()` stable within a driver and use SQLSTATE when the backend supplies it.
- [ ] Keep vendor diagnostics as the cause without making them part of portable control flow.
- [ ] Never include bound values, credentials or complete connection strings in failure text.
- [ ] Do not expose a universal `retryable()` flag; retry safety depends on the transaction and operation.

## `db/connection`

- [ ] role `DbExecuting`
  - [ ] `async execution(origin is Statement) is Execution`
  - [ ] `async rows(origin is Statement) is RowReading`
- [ ] role `DbConnecting`
  - [ ] `async connection() is DbConnection`
- [ ] object `DbConnection` does `DbExecuting, Closing`
  - [ ] Do not expose a public constructor.
  - [ ] `async execution(origin is Statement) is Execution`
  - [ ] `async rows(origin is Statement) is RowReading`
  - [ ] `async transaction(options is TransactionOptions) is Transaction`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] Serialize operations issued through one connection.
- [ ] Permit at most one active `RowReading` on a connection unless the driver proves independent streams.
- [ ] Reject a new operation when unread rows make it unsafe instead of buffering them implicitly.
- [ ] Make connection closing roll back an active transaction before releasing the native resource.
- [ ] Mark a connection broken after protocol loss or cancellation with an unknown server-side outcome.
- [ ] Keep authentication and transport configuration inside concrete connectors.

## `db/transaction`

- [ ] enum `Isolation`
  - [ ] `read_uncommitted`
  - [ ] `read_committed`
  - [ ] `repeatable_read`
  - [ ] `serializable`
- [ ] object `TransactionOptions`
  - [ ] `TransactionOptions(isolation is Isolation, read_only is bool, deferrable is bool)`
  - [ ] `isolation() is Isolation`
  - [ ] `read_only() is bool`
  - [ ] `deferrable() is bool`
- [ ] object `Transaction` does `DbExecuting`
  - [ ] Do not expose a public constructor.
  - [ ] `async execution(origin is Statement) is Execution`
  - [ ] `async rows(origin is Statement) is RowReading`
  - [ ] `async commit() is Transaction`
  - [ ] `async rollback() is Transaction`
  - [ ] `active() is bool`
- [ ] Reject isolation options a backend cannot honor; never silently weaken them.
- [ ] Make `commit()` and `rollback()` terminal and idempotent only after confirmed success.
- [ ] Make lexical cleanup roll back an active transaction.
- [ ] Preserve an indeterminate commit outcome as a distinct failure cause.
- [ ] Do not retry a transaction body implicitly.
- [ ] Keep savepoints driver-specific until a portable nested-transaction contract is justified.

## `db/prepared`

- [ ] object `Prepared`
  - [ ] `Prepared(origin is DbConnection, text is string)`
  - [ ] `async statement() is PreparedStatement`
- [ ] object `PreparedStatement` does `Closing`
  - [ ] Do not expose a public constructor.
  - [ ] `async execution(values is Sequencing<DbValue>) is Execution`
  - [ ] `async rows(values is Sequencing<DbValue>) is RowReading`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] Prepare exactly one statement and reject trailing executable input.
- [ ] Keep a prepared statement bound to its originating connection.
- [ ] Serialize executions of the same prepared statement.
- [ ] Reset and clear bindings after every execution.
- [ ] Invalidate prepared statements when their connection closes.
- [ ] Do not add `prepare()` to `DbConnection`; the separate object keeps it below five public actions.

## `db/pool`

- [ ] object `PoolOptions`
  - [ ] `PoolOptions(minimum is int, maximum is int, idle_timeout is Duration, lifetime is Duration, acquisition_timeout is Duration)`
  - [ ] `minimum() is int`
  - [ ] `maximum() is int`
  - [ ] `idle_timeout() is Duration`
  - [ ] `lifetime() is Duration`
  - [ ] `acquisition_timeout() is Duration`
- [ ] object `PoolSnapshot`
  - [ ] Do not expose a public constructor.
  - [ ] `acquisitions() is int`
  - [ ] `idle_connections() is int`
  - [ ] `waiting_requests() is int`
  - [ ] `capacity() is int`
- [ ] object `DbPool` does `Closing`
  - [ ] `DbPool(origin is DbConnecting, options is PoolOptions, clock is Timing)`
  - [ ] `async connection() is PooledConnection`
  - [ ] `snapshot() is PoolSnapshot`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `PooledConnection` does `DbExecuting, Closing`
  - [ ] Do not expose a public constructor.
  - [ ] `async execution(origin is Statement) is Execution`
  - [ ] `async rows(origin is Statement) is RowReading`
  - [ ] `async transaction(options is TransactionOptions) is Transaction`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] Reject negative durations, a non-positive maximum and `minimum > maximum`.
- [ ] Serve acquisition waiters in FIFO order.
- [ ] Make a cancelled or timed-out acquisition relinquish its queue position exactly once.
- [ ] Make `PooledConnection.close()` return a healthy physical connection to the pool.
- [ ] Roll back active transactions and close active row readers before returning a connection.
- [ ] Destroy broken, expired or reset-failing connections instead of pooling them.
- [ ] Let `DbPool.close()` reject new acquisitions, cancel waiters and close returned connections.
- [ ] Do not let pool shutdown wait forever for leaked leases.
- [ ] Keep health checks opt-in and driver-supplied; do not inject a magic SQL query.

## `db/sqlite`

- [ ] enum `SqliteJournal`
  - [ ] `native`
  - [ ] `delete`
  - [ ] `truncate`
  - [ ] `persist`
  - [ ] `memory`
  - [ ] `wal`
  - [ ] `off`
- [ ] object `SqliteOptions`
  - [ ] `SqliteOptions()`
  - [ ] `SqliteOptions(read_only is bool, create is bool, foreign_keys is bool, busy_timeout is Duration, journal is SqliteJournal)`
  - [ ] `read_only() is bool`
  - [ ] `creation() is bool`
  - [ ] `foreign_keys() is bool`
  - [ ] `busy_timeout() is Duration`
  - [ ] `journal() is SqliteJournal`
- [ ] object `SqliteDatabase` does `DbConnecting`
  - [ ] `SqliteDatabase(path is Path, options is SqliteOptions)`
  - [ ] `path() is Path`
  - [ ] `options() is SqliteOptions`
  - [ ] `async connection() is DbConnection`
- [ ] object `SqliteMemory` does `DbConnecting`
  - [ ] `SqliteMemory(options is SqliteOptions)`
  - [ ] `options() is SqliteOptions`
  - [ ] `async connection() is DbConnection`
- [ ] enum `SqliteTransactionMode`
  - [ ] `deferred`
  - [ ] `immediate`
  - [ ] `exclusive`
- [ ] object `SqliteTransactions`
  - [ ] `SqliteTransactions(origin is DbConnection, mode is SqliteTransactionMode)`
  - [ ] `async transaction(options is TransactionOptions) is Transaction`
- [ ] Open files through validated native paths without accepting SQLite URI options implicitly.
- [ ] Reject contradictory read-only and create options.
- [ ] Enable foreign-key enforcement by default on every connection.
- [ ] Apply the requested journal mode explicitly and verify the resulting mode.
- [ ] Treat every `SqliteMemory.connection()` call as a new independent database.
- [ ] Reject pooling `SqliteMemory`; multiple pooled connections would not share one database.
- [ ] Map SQLite NULL, INTEGER, REAL, TEXT and BLOB to the corresponding `DbValue` objects.
- [ ] Reject `DbInteger` values outside SQLite's signed 64-bit integer range.
- [ ] Bind `DbBoolean` as integer zero or one and `DbDecimal` as canonical text.
- [ ] Return stored SQLite values by storage class; do not guess dates, booleans or decimals from declarations.
- [ ] Preserve embedded NUL characters in text and bytes.
- [ ] Expose extended SQLite result codes through `DbFailure.code()`.
- [ ] Map busy and locked outcomes to `DbFailureKind.busy` after the configured timeout.
- [ ] Reject isolation and read-only transaction promises SQLite cannot enforce.
- [ ] Keep PRAGMA names out of parameter interpolation; expose reviewed settings as typed options.
- [ ] Keep loadable extensions disabled by default.

## `db/sqlite/blob`

- [ ] object `OpenedSqliteBlob`
  - [ ] `OpenedSqliteBlob(origin is DbConnection, database is string, table is string, column is string, row is BigInt, writable is bool)`
  - [ ] `async blob() is SqliteBlob`
- [ ] object `SqliteBlob` does `Closing, Sizing`
  - [ ] Do not expose a public constructor.
  - [ ] `async bytes(offset is BigInt, limit is int) is @byte`
  - [ ] `async write(offset is BigInt, source is @byte) is SqliteBlob`
  - [ ] `size() is int`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] Validate database, table and column identifiers through SQLite's API rather than SQL concatenation.
- [ ] Reject negative offsets, non-positive limits and reads or writes beyond the fixed blob size.
- [ ] Reject writes when the blob was opened read-only.
- [ ] Invalidate the blob when its connection or transaction ends.

## `db/sqlite/backup`

- [ ] object `SqliteBackup`
  - [ ] `SqliteBackup(source is DbConnection, target is DbConnection, pages is int)`
  - [ ] `async result() is BackupReport`
- [ ] object `BackupReport`
  - [ ] Do not expose a public constructor.
  - [ ] `copies() is BigInt`
  - [ ] `total() is BigInt`
- [ ] Require source and target to be distinct SQLite connections.
- [ ] Copy a bounded page batch before yielding to the scheduler.
- [ ] Retry busy source snapshots only within an explicit deadline.
- [ ] Leave an interrupted target as an ordinary database whose transaction was rolled back.

## `db/sqlite/script`

- [ ] object `SqliteScript`
  - [ ] `SqliteScript(origin is DbConnection, source is string)`
  - [ ] `async executions() is Sequencing<Execution>`
- [ ] Split scripts only with SQLite's prepare-tail API, not with a handwritten semicolon splitter.
- [ ] Reject statements that return rows.
- [ ] Execute inside the caller's current transaction state without an implicit commit.
- [ ] Stop at the first failure and preserve prior effects according to the explicit transaction.
- [ ] Bound the script byte size and statement count.

## Stage 23 verification

- [ ] Add value-kind and wrong-projection cases for every `DbValue` object.
- [ ] Add placeholder-count, trailing-statement and injection cases.
- [ ] Add duplicate, empty and unusual column-name cases.
- [ ] Add row exhaustion, early close and bounded-fetch cases.
- [ ] Add commit, rollback, lexical cleanup and indeterminate-outcome cases.
- [ ] Add prepared-statement reset, reuse and connection-invalidation cases.
- [ ] Add pool fairness, cancellation, timeout, expiry, reset and shutdown races.
- [ ] Add SQLite NULL, integer-boundary, floating, embedded-NUL, text and blob cases.
- [ ] Add SQLite foreign-key, journal, busy-timeout and transaction-mode cases.
- [ ] Add in-memory database isolation cases.
- [ ] Add incremental blob read, write, bounds and invalidation cases.
- [ ] Add online-backup progress, busy and cancellation cases.
- [ ] Add script quoting, comments, triggers, row-returning rejection and rollback cases.
- [ ] Prove slow SQLite queries do not block unrelated timers, I/O or database connections.
- [ ] Prove cancellation interrupts SQLite and leaves the connection reusable or explicitly broken.
- [ ] Run the SQLite conformance suite against the minimum and newest supported SQLite versions.
- [ ] Add one Perl-level regression file per public module group.
- [ ] Add runnable parameter binding, transaction, prepared statement, pool and backup examples.
- [ ] Run `git diff --check`.

## Stage 23 exclusions

- [ ] Do not derive the v3 public API from existing `stdlib/db/*` modules.
- [ ] Do not add database conversions or duration helpers to builtin runtime types.
- [ ] Do not interpolate values or identifiers into SQL strings.
- [ ] Do not add an ORM, active-record layer, schema DSL or universal query builder.
- [ ] Do not add a migration framework to the standard library; migrations are application policy.
- [ ] Do not bundle PostgreSQL, MySQL, MariaDB or ODBC drivers with the standard library.
- [ ] Let external drivers implement `DbConnecting` and the same value, row and failure contracts.
- [ ] Do not expose a universal connection-string grammar across unrelated drivers.
- [ ] Do not enable SQLite loadable extensions implicitly.
- [ ] Do not expose SQLite language callbacks until Stage 31 defines a safe reentrant callback ABI.
- [ ] Do not retry statements, transactions or commits implicitly.
- [ ] Keep database tracing and metrics integration in Stage 28.

---

# Stage 24 — config, CLI and terminal

## `runtime/application` substrate

- [x] Snapshot application arguments as immutable raw byte arrays.
- [x] Exclude the source-file path from application arguments.
- [x] Preserve non-UTF-8 POSIX argument bytes without loss.
- [x] Snapshot, read, set and remove raw environment values.
- [x] Keep an empty environment value distinct from a missing variable.
- [x] Provide byte-oriented standard input, output and error operations.
- [x] Provide platform and architecture identities.
- [ ] Add native standard-stream TTY detection.
- [ ] Add native controlling-terminal discovery and opening.
- [ ] Add terminal width and height queries.
- [ ] Add guarded cooked, cbreak and raw terminal modes.
- [ ] Add a pollable terminal-resize source.
- [ ] Add scheduler-aware standard-input readiness without a blocking host read.
- [ ] Add Windows console Unicode input and output support.
- [ ] Add guarded Windows virtual-terminal processing.
- [ ] Restore terminal modes and console flags after normal return, error, cancellation and panic.
- [ ] Keep config grammars, CLI policy and ANSI rendering out of the runtime.

## `cli/arguments`

- [ ] object `ProcessArguments` does `Sequencing<@byte>`
  - [ ] Do not expose a public constructor.
  - [ ] `cursor() is Cursor<@byte>`
  - [ ] `size() is int`
  - [ ] `item(index is int) is @byte`
  - [ ] `first() is @byte`
  - [ ] `last() is @byte`
- [ ] object `Utf8Arguments` does `Sequencing<string>`
  - [ ] `Utf8Arguments(origin is ProcessArguments)`
  - [ ] `cursor() is Cursor<string>`
  - [ ] `size() is int`
  - [ ] `item(index is int) is string`
  - [ ] `first() is string`
  - [ ] `last() is string`
- [ ] Decode each argument independently with Stage 06 strict UTF-8.
- [ ] Preserve argument boundaries exactly.
- [ ] Report the failing argument index and byte offset on invalid UTF-8.
- [ ] Do not reinterpret quotes, escapes, globs or environment references after the host has split argv.

## `config/key`

- [ ] object `SettingKey` does `Comparing<SettingKey>`
  - [ ] `SettingKey(segments is Sequencing<string>)`
  - [ ] `segments() is Sequencing<string>`
  - [ ] `order(other is SettingKey) is Order`
  - [ ] `string() is string`
- [ ] object `SettingKeyFromString`
  - [ ] `SettingKeyFromString(origin is string, separator is string)`
  - [ ] `#projected value() is SettingKey`
- [ ] object `InvalidSettingKey` does `Error`
  - [ ] `InvalidSettingKey(reason is string)`
  - [ ] `text() is string`
- [ ] Reject an empty key, empty segment or empty separator.
- [ ] Keep segments as the identity; make `string()` diagnostic rather than round-trippable.
- [ ] Do not assign universal meaning to dots, underscores or case folding.

## `config/source`

- [ ] enum `SettingSourceKind`
  - [ ] `defaults`
  - [ ] `file`
  - [ ] `environment`
  - [ ] `arguments`
  - [ ] `explicit`
- [ ] object `SettingSource`
  - [ ] `SettingSource(kind is SettingSourceKind, name is string)`
  - [ ] `kind() is SettingSourceKind`
  - [ ] `name() is string`
- [ ] object `SettingEntry`
  - [ ] `SettingEntry(key is SettingKey, values is Sequencing<string>, source is SettingSource)`
  - [ ] `key() is SettingKey`
  - [ ] `values() is Sequencing<string>`
  - [ ] `source() is SettingSource`
- [ ] role `SettingReading`
  - [ ] `membership(key is SettingKey) is bool`
  - [ ] `entry(key is SettingKey) is SettingEntry`
  - [ ] `keys() is Sequencing<SettingKey>`
- [ ] object `Settings` does `SettingReading`
  - [ ] `Settings(entries is Sequencing<SettingEntry>)`
  - [ ] `membership(key is SettingKey) is bool`
  - [ ] `entry(key is SettingKey) is SettingEntry`
  - [ ] `keys() is Sequencing<SettingKey>`
- [ ] object `LayeredSettings` does `SettingReading`
  - [ ] `LayeredSettings(layers is Sequencing<SettingReading>)`
  - [ ] `membership(key is SettingKey) is bool`
  - [ ] `entry(key is SettingKey) is SettingEntry`
  - [ ] `keys() is Sequencing<SettingKey>`
- [ ] object `MissingSetting` does `Error`
  - [ ] `MissingSetting(key is SettingKey)`
  - [ ] `key() is SettingKey`
  - [ ] `text() is string`
- [ ] Reject duplicate keys inside one `Settings` object.
- [ ] Let a later layer replace the complete entry from an earlier layer.
- [ ] Never append repeated values across precedence layers implicitly.
- [ ] Preserve the winning entry's source for diagnostics.
- [ ] Preserve deterministic key order by first appearance, replacing values in place.
- [ ] Keep `Settings` immutable.

## `config/value`

- [ ] object `Setting`
  - [ ] `Setting(origin is SettingReading, key is SettingKey)`
  - [ ] `values() is Sequencing<string>`
  - [ ] `source() is SettingSource`
- [ ] object `ScalarSetting`
  - [ ] `ScalarSetting(origin is SettingReading, key is SettingKey)`
  - [ ] `value() is string`
  - [ ] `source() is SettingSource`
- [ ] object `ScalarSettingOr`
  - [ ] `ScalarSettingOr(origin is SettingReading, key is SettingKey, fallback is string)`
  - [ ] `value() is string`
  - [ ] `source() is SettingSource`
- [ ] object `NonScalarSetting` does `Error`
  - [ ] `NonScalarSetting(key is SettingKey, count is int)`
  - [ ] `key() is SettingKey`
  - [ ] `count() is int`
  - [ ] `text() is string`
- [ ] Make `Setting` throw `MissingSetting` when the key is absent.
- [ ] Make `ScalarSetting` require exactly one value.
- [ ] Make `ScalarSettingOr` use its fallback only when the key is absent, not when the value is empty.
- [ ] Give a fallback the synthetic source `SettingSourceKind.defaults`.
- [ ] Parse scalar text explicitly through Stage 05 `strings/parse` objects.
- [ ] Do not create a parallel typed configuration-value hierarchy.
- [ ] Do not infer secrets from key names; callers decide which values must be redacted.

## `config/environment`

- [ ] object `EnvironmentSettings` does `SettingReading`
  - [ ] `EnvironmentSettings(origin is Environment, names is %string:SettingKey, source is SettingSource)`
  - [ ] `membership(key is SettingKey) is bool`
  - [ ] `entry(key is SettingKey) is SettingEntry`
  - [ ] `keys() is Sequencing<SettingKey>`
- [ ] Decode mapped names and values as strict UTF-8.
- [ ] Keep missing variables absent and empty variables present with one empty value.
- [ ] Reject two environment names mapped to the same setting key.
- [ ] Require an explicit environment-name mapping.
- [ ] Do not invent implicit prefixes, underscore splitting or case normalization.
- [ ] Do not mutate the process environment while reading configuration.

## `config/document`

- [ ] object `TomlSettings` does `SettingReading`
  - [ ] `TomlSettings(origin is TomlTable, source is SettingSource)`
  - [ ] `membership(key is SettingKey) is bool`
  - [ ] `entry(key is SettingKey) is SettingEntry`
  - [ ] `keys() is Sequencing<SettingKey>`
- [ ] object `JsonSettings` does `SettingReading`
  - [ ] `JsonSettings(origin is JsonObject, source is SettingSource)`
  - [ ] `membership(key is SettingKey) is bool`
  - [ ] `entry(key is SettingKey) is SettingEntry`
  - [ ] `keys() is Sequencing<SettingKey>`
- [ ] object `IniSettings` does `SettingReading`
  - [ ] `IniSettings(origin is IniDocument, source is SettingSource)`
  - [ ] `membership(key is SettingKey) is bool`
  - [ ] `entry(key is SettingKey) is SettingEntry`
  - [ ] `keys() is Sequencing<SettingKey>`
- [ ] Flatten nested TOML tables and JSON objects into key segments without joining segment text.
- [ ] Convert scalar values to their canonical source-format text.
- [ ] Convert an array of scalars into repeated values from one source.
- [ ] Reject nested arrays, arrays containing objects and TOML arrays of tables.
- [ ] Map INI sections and entry names to exactly two key segments.
- [ ] Preserve document insertion order.
- [ ] Keep file opening, charset decoding and size limits explicit through Stage 07 and Stage 11 objects.
- [ ] Do not select a configuration grammar from a filename extension implicitly.

## `cli/spec`

- [ ] object `OptionValue`
  - [ ] `OptionValue(name is string, repeated is bool)`
  - [ ] `name() is string`
  - [ ] `repeated() is bool`
- [ ] object `Option`
  - [ ] `Option(key is SettingKey, names is Sequencing<string>, value is OptionValue, help is string)`
  - [ ] `key() is SettingKey`
  - [ ] `names() is Sequencing<string>`
  - [ ] `value() is OptionValue`
  - [ ] `help() is string`
- [ ] object `Flag`
  - [ ] `Flag(key is SettingKey, names is Sequencing<string>, present is string, help is string)`
  - [ ] `key() is SettingKey`
  - [ ] `names() is Sequencing<string>`
  - [ ] `present() is string`
  - [ ] `help() is string`
- [ ] enum `OperandCardinality`
  - [ ] `required`
  - [ ] `optional`
  - [ ] `repeated`
  - [ ] `repeated_nonempty`
- [ ] object `Operand`
  - [ ] `Operand(key is SettingKey, name is string, cardinality is OperandCardinality, help is string)`
  - [ ] `key() is SettingKey`
  - [ ] `name() is string`
  - [ ] `cardinality() is OperandCardinality`
  - [ ] `help() is string`
- [ ] object `CommandSyntax`
  - [ ] `CommandSyntax(options is Sequencing<Option>, flags is Sequencing<Flag>, operands is Sequencing<Operand>)`
  - [ ] `options() is Sequencing<Option>`
  - [ ] `flags() is Sequencing<Flag>`
  - [ ] `operands() is Sequencing<Operand>`
- [ ] object `CliCommand`
  - [ ] `CliCommand(name is string, summary is string, syntax is CommandSyntax, children is Sequencing<CliCommand>)`
  - [ ] `name() is string`
  - [ ] `summary() is string`
  - [ ] `syntax() is CommandSyntax`
  - [ ] `children() is Sequencing<CliCommand>`
- [ ] Validate the complete command tree before parsing arguments.
- [ ] Require long names to match `--[a-z0-9][a-z0-9-]*`.
- [ ] Require short names to contain one visible ASCII character after `-`.
- [ ] Reject duplicate option names and child command names in one command.
- [ ] Permit multiple flags for one setting key only to express explicit alternatives such as `true` and `false`.
- [ ] Reject a setting key shared by value options, operands or incompatible declarations.
- [ ] Require optional operands to follow required operands.
- [ ] Require repeated operands to be last.
- [ ] Keep help and version as ordinary explicitly declared flags.
- [ ] Make every `Option` consume exactly one value per occurrence.
- [ ] Check whether a setting is required only after all configuration layers are composed.
- [ ] Do not attach callbacks or process exits to command specifications.

## `cli/parse`

- [ ] object `ParsedCommand`
  - [ ] `ParsedCommand(origin is CliCommand, arguments is Sequencing<string>)`
  - [ ] `#projected value() is Invocation`
- [ ] object `Invocation`
  - [ ] Do not expose a public constructor.
  - [ ] `command() is CliCommand`
  - [ ] `path() is Sequencing<string>`
  - [ ] `settings() is Settings`
- [ ] object `CliFailure` does `Error`
  - [ ] `CliFailure(index is int, token is string, message is string)`
  - [ ] `index() is int`
  - [ ] `token() is string`
  - [ ] `text() is string`
- [ ] Accept exact long options as `--name value` and `--name=value`.
- [ ] Accept short flags in clusters such as `-abc`.
- [ ] Let only the final short name in a cluster consume a value.
- [ ] Make `--` terminate option recognition.
- [ ] Do not abbreviate long names or correct misspellings automatically.
- [ ] Reject a repeated scalar option unless its `OptionValue.repeated()` is true.
- [ ] Store a present flag as its declared `Flag.present()` value.
- [ ] Reject a flag value such as `--verbose=false`; model negation as a separate explicit flag.
- [ ] Select a child command only at the command position defined by the validated syntax.
- [ ] Preserve repeated option and operand order.
- [ ] Treat a negative numeric token as an operand when no exact option name matches it.
- [ ] Reject unknown options, missing values, extra operands and incomplete command paths.
- [ ] Return argument-derived `Settings` without reading files or environment variables.
- [ ] Never print help, write diagnostics or terminate the process from the parser.

## `cli/help`

- [ ] object `Usage`
  - [ ] `Usage(origin is Command, program is string, width is int)`
  - [ ] `string() is string`
- [ ] enum `CompletionShell`
  - [ ] `bash`
  - [ ] `zsh`
  - [ ] `fish`
  - [ ] `powershell`
- [ ] object `ShellCompletion`
  - [ ] `ShellCompletion(origin is Command, program is string, shell is CompletionShell)`
  - [ ] `string() is string`
- [ ] Render deterministic plain text with no ANSI sequences.
- [ ] Preserve declaration order for options, operands and child commands.
- [ ] Display every accepted name, value placeholder and repetition marker.
- [ ] Wrap descriptions on Unicode display width rather than byte or code-point count.
- [ ] Reject unusably narrow and non-positive widths.
- [ ] Quote shell completion literals for the selected shell.
- [ ] Generate static completion only; keep dynamic completion callbacks outside the first API.

## `terminal/capability`

- [ ] enum `TerminalColorDepth`
  - [ ] `none`
  - [ ] `ansi16`
  - [ ] `ansi256`
  - [ ] `true_color`
- [ ] enum `ColorPolicy`
  - [ ] `automatic`
  - [ ] `always`
  - [ ] `never`
- [ ] enum `TerminalOutputKind`
  - [ ] `output`
  - [ ] `error`
- [ ] object `TerminalSize`
  - [ ] `TerminalSize(columns is int, rows is int)`
  - [ ] `columns() is int`
  - [ ] `rows() is int`
- [ ] object `TerminalCapabilities`
  - [ ] Do not expose a public constructor.
  - [ ] `input_interactive() is bool`
  - [ ] `output_interactive() is bool`
  - [ ] `colors() is TerminalColorDepth`
  - [ ] `size() is TerminalSize`
- [ ] Respect `ColorPolicy.never` unconditionally.
- [ ] Respect `ColorPolicy.always` even for redirected output, but only for styling sequences.
- [ ] In automatic mode require a TTY, honor `NO_COLOR`, reject `TERM=dumb` and inspect platform capabilities.
- [ ] Use `COLUMNS` and `LINES` only as validated fallbacks when a native size query is unavailable.
- [ ] Keep terminal capability detection immutable for one opened session except for size changes.

## `terminal/width`

- [ ] object `DisplayWidth`
  - [ ] `DisplayWidth(origin is string)`
  - [ ] `#projected value() is int`
- [ ] object `TruncatedDisplay`
  - [ ] `TruncatedDisplay(origin is string, width is int, ellipsis is string)`
  - [ ] `string() is string`
- [ ] object `InvalidDisplayText` does `Error`
  - [ ] `InvalidDisplayText(index is int, reason is string)`
  - [ ] `index() is int`
  - [ ] `text() is string`
- [ ] Use the Stage 06 pinned Unicode character database.
- [ ] Give combining marks, variation selectors and zero-width joiners zero standalone width.
- [ ] Account for East Asian wide characters and joined emoji sequences.
- [ ] Treat control characters as `InvalidDisplayText` rather than assigning them a printable width.
- [ ] Truncate only at extended grapheme-cluster boundaries.
- [ ] Reject negative widths and an ellipsis wider than the target width.

## `terminal/style`

- [ ] role `TerminalColor`
  - [ ] Keep this noun role action-free as a closed styling category.
- [ ] enum `BasicColor`
  - [ ] `black`
  - [ ] `red`
  - [ ] `green`
  - [ ] `yellow`
  - [ ] `blue`
  - [ ] `magenta`
  - [ ] `cyan`
  - [ ] `white`
  - [ ] `bright_black`
  - [ ] `bright_red`
  - [ ] `bright_green`
  - [ ] `bright_yellow`
  - [ ] `bright_blue`
  - [ ] `bright_magenta`
  - [ ] `bright_cyan`
  - [ ] `bright_white`
- [ ] object `DefaultColor` does `TerminalColor`
  - [ ] `DefaultColor()`
- [ ] object `NamedColor` does `TerminalColor`
  - [ ] `NamedColor(value is BasicColor)`
  - [ ] `value() is BasicColor`
- [ ] object `IndexedColor` does `TerminalColor`
  - [ ] `IndexedColor(value is int)`
  - [ ] `value() is int`
- [ ] object `RgbColor` does `TerminalColor`
  - [ ] `RgbColor(red is byte, green is byte, blue is byte)`
  - [ ] `red() is byte`
  - [ ] `green() is byte`
  - [ ] `blue() is byte`
- [ ] enum `TextAttribute`
  - [ ] `bold`
  - [ ] `dim`
  - [ ] `italic`
  - [ ] `underline`
  - [ ] `blink`
  - [ ] `reverse`
  - [ ] `hidden`
  - [ ] `strikethrough`
- [ ] object `TextStyle`
  - [ ] `TextStyle(foreground is TerminalColor, background is TerminalColor, attributes is Set<TextAttribute>)`
  - [ ] `foreground() is TerminalColor`
  - [ ] `background() is TerminalColor`
  - [ ] `attributes() is Set<TextAttribute>`
- [ ] object `StyledText`
  - [ ] `StyledText(origin is string, style is TextStyle)`
  - [ ] `text() is string`
  - [ ] `style() is TextStyle`
- [ ] Reject indexed colors outside 0..255.
- [ ] Downgrade colors deterministically to the detected depth.
- [ ] Emit a complete style reset after each `StyledText` value.
- [ ] Never count ANSI control bytes toward display width.
- [ ] Keep styling separate from semantic log levels.

## `terminal/session`

- [ ] object `OpenedStandardTerminal`
  - [ ] `OpenedStandardTerminal(output is TerminalOutputKind, policy is ColorPolicy)`
  - [ ] `session() is TerminalSession`
- [ ] object `OpenedControllingTerminal`
  - [ ] `OpenedControllingTerminal(policy is ColorPolicy)`
  - [ ] `session() is TerminalSession`
- [ ] object `TerminalSession` does `Closing`
  - [ ] Do not expose a public constructor.
  - [ ] `input() is TerminalInput`
  - [ ] `output() is TerminalOutput`
  - [ ] `capabilities() is TerminalCapabilities`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] object `TerminalInput` does `Reading, ReadingText`
  - [ ] Do not expose a public constructor.
  - [ ] `bytes(limit is int) is @byte`
  - [ ] `line() is string`
  - [ ] `text() is string`
  - [ ] `capabilities() is TerminalCapabilities`
- [ ] object `TerminalOutput` does `Writing, WritingText`
  - [ ] Do not expose a public constructor.
  - [ ] `write(source is ByteSource) is void`
  - [ ] `write(values is ...string) is void`
  - [ ] `write(value is StyledText) is void`
  - [ ] `writeln(values is ...string) is void`
  - [ ] `writeln(value is StyledText) is void`
  - [ ] `flush() is void`
  - [ ] `capabilities() is TerminalCapabilities`
- [ ] Make standard-terminal interactivity depend on the selected standard output and standard input independently.
- [ ] Open the controlling terminal independently of redirected stdin, stdout and stderr.
- [ ] Make a missing controlling terminal a typed `TerminalUnavailable` failure.
- [ ] Make session closing restore every mode and screen change made through that session.
- [ ] Render `StyledText` as plain text when styling is disabled.
- [ ] Preserve the Stage 07 `write`, `writeln`, `flush` and `<-` behavior.

## `terminal/mode`

- [ ] enum `TerminalMode`
  - [ ] `cooked`
  - [ ] `cbreak`
  - [ ] `raw`
- [ ] object `AppliedTerminalMode`
  - [ ] `AppliedTerminalMode(origin is TerminalSession, mode is TerminalMode)`
  - [ ] `lease() is TerminalModeLease`
- [ ] object `TerminalModeLease` does `Closing`
  - [ ] Do not expose a public constructor.
  - [ ] `mode() is TerminalMode`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] Apply mode changes only to an interactive terminal input.
- [ ] Keep mode leases properly nested.
- [ ] Make closing a lease restore the immediately previous mode.
- [ ] Restore all outstanding leases when their session closes.
- [ ] Do not expose host `termios` bit masks or Windows console flags.

## `terminal/screen`

- [ ] enum `ClearRegion`
  - [ ] `screen`
  - [ ] `screen_before`
  - [ ] `screen_after`
  - [ ] `line`
  - [ ] `line_before`
  - [ ] `line_after`
- [ ] object `TerminalScreen`
  - [ ] `TerminalScreen(origin is TerminalSession)`
  - [ ] `clear(region is ClearRegion) is void`
  - [ ] `move(row is int, column is int) is void`
  - [ ] `display_cursor(visible is bool) is void`
  - [ ] `use_alternate_screen(enabled is bool) is void`
- [ ] object `TerminalResizes` does `Closing`
  - [ ] `TerminalResizes(origin is TerminalSession)`
  - [ ] `async size() is TerminalSize`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] Reject screen control on non-interactive output even under `ColorPolicy.always`.
- [ ] Treat row and column as zero-based logical coordinates.
- [ ] Coalesce resize notifications and return the newest observed size.
- [ ] Restore a visible cursor and leave the alternate screen when the session closes.
- [ ] Do not expose arbitrary escape-sequence construction.

## `terminal/prompt`

- [ ] object `PromptedLine`
  - [ ] `PromptedLine(origin is TerminalSession, prompt is string, limit is int)`
  - [ ] `string() is string`
- [ ] object `PromptedSecret`
  - [ ] `PromptedSecret(origin is TerminalSession, prompt is string, limit is int)`
  - [ ] `string() is string`
- [ ] object `TerminalUnavailable` does `Error`
  - [ ] `TerminalUnavailable(reason is string)`
  - [ ] `text() is string`
- [ ] Flush the prompt before waiting for input.
- [ ] Strip one trailing line ending without trimming other whitespace.
- [ ] Enforce a positive byte limit incrementally.
- [ ] Require a controlling terminal for secret input.
- [ ] Disable echo only for the secret read and restore it on every exit path.
- [ ] Do not return partial secret input after interruption or overflow.
- [ ] Do not add confirmation, menu or form widgets to the primitive prompt layer.

## Stage 24 verification

- [ ] Add raw argv boundary, empty and invalid UTF-8 cases.
- [ ] Add setting-key segment and diagnostic-string cases.
- [ ] Add missing, empty, scalar, repeated and provenance cases.
- [ ] Add deterministic precedence tests for every source ordering.
- [ ] Add explicit environment mapping and collision cases.
- [ ] Add TOML, JSON and INI flattening and unsupported-structure cases.
- [ ] Add command-spec ambiguity and invalid-name cases.
- [ ] Add long option, equals form, short cluster, terminator and negative-number cases.
- [ ] Add subcommand, repeated option and operand-cardinality cases.
- [ ] Add deterministic help snapshots at narrow, normal and wide terminal widths.
- [ ] Parse every generated shell-completion file with its target shell where available.
- [ ] Test TTY detection with pseudoterminals and redirected standard streams.
- [ ] Test 16-color, 256-color, true-color, `NO_COLOR` and `TERM=dumb` policy.
- [ ] Add Unicode combining-mark, emoji and wide-character display-width cases.
- [ ] Test cooked, cbreak and raw restoration after success, error, cancellation and panic.
- [ ] Add cursor, alternate-screen and resize-event pseudoterminal cases.
- [ ] Verify secret prompts never echo or leak partial values into failures.
- [ ] Test Windows console behavior on a native Windows runner.
- [ ] Add one Perl-level regression file per public module group.
- [ ] Add runnable layered-config, subcommand, styled-output and secret-prompt examples.
- [ ] Run `git diff --check`.

## Stage 24 exclusions

- [ ] Do not derive the v3 API from existing configuration, CLI or terminal modules.
- [ ] Do not add configuration or CLI parsing methods to builtin strings and collections.
- [ ] Do not create a second JSON-, TOML- or YAML-like universal config value tree.
- [ ] Do not infer configuration precedence; layer order is constructor input.
- [ ] Do not infer environment mappings, filename formats or secret keys.
- [ ] Do not read environment variables or files inside the CLI parser.
- [ ] Do not let CLI parsing print, prompt, execute callbacks or terminate the process.
- [ ] Do not support shell evaluation, response files or long-option abbreviation.
- [ ] Do not emit styling or screen controls to redirected output implicitly.
- [ ] Do not expose raw ANSI, termios, ioctl or Windows console constants.
- [ ] Do not build a curses clone, full-screen widget toolkit or line editor into this stage.
- [ ] Keep locale-aware messages in Stage 26 and diagnostics integration in Stage 28.

---

# Stage 25 — logging

## `runtime/logging` substrate

- [x] Reuse the Stage 10 wall clock for event timestamps.
- [x] Reuse the native current-process identifier.
- [x] Reuse all-or-error Stage 07 writes to stderr, files and streams.
- [x] Reuse Stage 19 scoped futures and event signalling for optional queued delivery.
- [x] Keep runtime tasks cooperatively serialized between synchronous stdlib actions.
- [x] Keep the runtime free of logger names, levels, fields, formatting and sink policy.
- [ ] Add a stable runtime task identity only if Stage 28 demonstrates a tracing requirement for it.
- [ ] Do not add ambient task-local logging context as a runtime intrinsic.
- [ ] Do not add a native logging provider; compose logging from existing clocks and I/O capabilities.

## `logging/level`

- [ ] enum `LogLevel`
  - [ ] `debug`
  - [ ] `info`
  - [ ] `warning`
  - [ ] `error`
  - [ ] `critical`
- [ ] object `LogLevelName`
  - [ ] `LogLevelName(origin is LogLevel)`
  - [ ] `string() is string`
- [ ] object `LogLevelFromName`
  - [ ] `LogLevelFromName(origin is string)`
  - [ ] `#projected value() is LogLevel`
- [ ] Order levels exactly as declared from least to most severe.
- [ ] Parse canonical lowercase names case-insensitively.
- [ ] Render canonical uppercase names for text output.
- [ ] Keep tracing spans separate instead of adding an ambiguous `trace` log level.
- [ ] Do not add custom numeric levels to the portable API.

## `logging/name`

- [ ] object `LogName` does `Comparing<LogName>`
  - [ ] `LogName(segments is Sequencing<string>)`
  - [ ] `segments() is Sequencing<string>`
  - [ ] `order(other is LogName) is Order`
  - [ ] `string() is string`
- [ ] object `ChildLogName`
  - [ ] `ChildLogName(origin is LogName, segment is string)`
  - [ ] `#projected value() is LogName`
- [ ] object `InvalidLogName` does `Error`
  - [ ] `InvalidLogName(reason is string)`
  - [ ] `text() is string`
- [ ] Reject empty names and empty segments.
- [ ] Preserve segments as identity and use a dot only for diagnostic rendering.
- [ ] Compare names lexicographically by Unicode scalar value.
- [ ] Do not make name ancestry imply filtering or sink inheritance automatically.

## `logging/field`

- [ ] object `LogFields`
  - [ ] `LogFields(values is %string:JsonValue)`
  - [ ] `membership(name is string) is bool`
  - [ ] `value(name is string) is JsonValue`
  - [ ] `values() is %string:JsonValue`
- [ ] object `EmptyLogFields`
  - [ ] `#projected value() is LogFields`
- [ ] object `CombinedLogFields`
  - [ ] `CombinedLogFields(base is LogFields, added is LogFields)`
  - [ ] `#projected value() is LogFields`
- [ ] object `MissingLogField` does `Error`
  - [ ] `MissingLogField(name is string)`
  - [ ] `name() is string`
  - [ ] `text() is string`
- [ ] Reject empty field names.
- [ ] Preserve field insertion order.
- [ ] Let an explicitly added field replace a base field without moving its position.
- [ ] Reuse Stage 13 `JsonValue` for structured scalar, array and object fields.
- [ ] Encode bytes, timestamps and application domain objects explicitly before logging them.
- [ ] Do not stringify arbitrary `any` values through the runtime debug dumper.

## `logging/record`

- [ ] object `LogContext`
  - [ ] `LogContext(name is LogName, fields is LogFields, process is int)`
  - [ ] `name() is LogName`
  - [ ] `fields() is LogFields`
  - [ ] `process() is int`
- [ ] object `LogRecord`
  - [ ] `LogRecord(occurred is Instant, level is LogLevel, message is string, context is LogContext)`
  - [ ] `LogRecord(occurred is Instant, level is LogLevel, message is string, context is LogContext, cause is Error)`
  - [ ] `time() is Instant`
  - [ ] `level() is LogLevel`
  - [ ] `message() is string`
  - [ ] `context() is LogContext`
  - [ ] `cause() is Error`
- [ ] object `MissingLogCause` does `Error`
  - [ ] `text() is string`
- [ ] Keep records and contexts immutable.
- [ ] Capture the wall-clock timestamp when the logging action executes.
- [ ] Make `cause()` throw `MissingLogCause` when no cause was supplied.
- [ ] Preserve a cause as an `Error`; format only its public `text()` in Stage 25.
- [ ] Add stack and source diagnostics only through Stage 28 capabilities.
- [ ] Do not infer fields by reflecting over the message or cause.

## `logging/write`

- [ ] role `LogWriting`
  - [ ] `write(origin is LogRecord) is void`
- [ ] object `DiscardingLogs` does `LogWriting`
  - [ ] `DiscardingLogs()`
  - [ ] `write(origin is LogRecord) is void`
- [ ] object `CollectedLogs` does `LogWriting, Sizing`
  - [ ] `CollectedLogs()`
  - [ ] `write(origin is LogRecord) is void`
  - [ ] `size() is int`
  - [ ] `records() is Sequencing<LogRecord>`
- [ ] object `BroadcastLogs` does `LogWriting`
  - [ ] `BroadcastLogs(destinations is Sequencing<LogWriting>)`
  - [ ] `write(origin is LogRecord) is void`
- [ ] object `FallbackLogs` does `LogWriting`
  - [ ] `FallbackLogs(primary is LogWriting, fallback is LogWriting)`
  - [ ] `write(origin is LogRecord) is void`
- [ ] object `BestEffortLogs` does `LogWriting`
  - [ ] `BestEffortLogs(origin is LogWriting)`
  - [ ] `write(origin is LogRecord) is void`
  - [ ] `failures() is BigInt`
- [ ] object `LogDeliveryFailure` does `Error`
  - [ ] `LogDeliveryFailure(causes is Sequencing<Error>)`
  - [ ] `causes() is Sequencing<Error>`
  - [ ] `text() is string`
- [ ] Keep one `write()` call equal to one complete immutable record.
- [ ] Make `CollectedLogs.records()` return a snapshot.
- [ ] Attempt broadcast destinations in declaration order and report every failure together.
- [ ] Invoke fallback only when primary delivery fails.
- [ ] Document that fallback delivery can duplicate a record after an indeterminate partial primary write.
- [ ] Let ordinary sink failures propagate unless wrapped in `FallbackLogs` or `BestEffortLogs`.
- [ ] Never log a logging failure through the failing destination recursively.

## `logging/named`

- [ ] role `Logging`
  - [ ] `log(level is LogLevel, message is string) is void`
  - [ ] `log(level is LogLevel, message is string, fields is LogFields) is void`
  - [ ] `log(level is LogLevel, message is string, fields is LogFields, cause is Error) is void`
- [ ] object `NamedLogging` does `Logging`
  - [ ] `NamedLogging(name is LogName, destination is LogWriting, clock is Timing, fields is LogFields)`
  - [ ] `log(level is LogLevel, message is string) is void`
  - [ ] `log(level is LogLevel, message is string, fields is LogFields) is void`
  - [ ] `log(level is LogLevel, message is string, fields is LogFields, cause is Error) is void`
- [ ] Merge logger fields with call fields, giving call fields precedence.
- [ ] Capture one timestamp and process identifier per delivered record.
- [ ] Construct the record before invoking its destination exactly once.
- [ ] Keep logging actions synchronous and deterministic.
- [ ] Do not hide sink failures by default.
- [ ] Do not install a process-global default logger.
- [ ] Do not derive logger names from source packages or object names implicitly.

## `logging/enrichment`

- [ ] object `FieldLogging` does `Logging`
  - [ ] `FieldLogging(origin is Logging, fields is LogFields)`
  - [ ] `log(level is LogLevel, message is string) is void`
  - [ ] `log(level is LogLevel, message is string, fields is LogFields) is void`
  - [ ] `log(level is LogLevel, message is string, fields is LogFields, cause is Error) is void`
- [ ] object `ChildLogging`
  - [ ] `ChildLogging(origin is NamedLogging, segment is string)`
  - [ ] `#projected value() is NamedLogging`
- [ ] Merge contextual fields before call fields so the nearest explicit value wins.
- [ ] Preserve the destination, clock and base fields in child loggers.
- [ ] Keep contextual logging as an explicit object passed to collaborators.
- [ ] Do not use thread-local, task-local or dynamically scoped field maps.
- [ ] Reserve trace and span correlation-field construction for Stage 28.

## `logging/filter`

- [ ] role `LogAllowing`
  - [ ] `allowed(name is LogName, level is LogLevel) is bool`
- [ ] object `LevelThresholds` does `LogAllowing`
  - [ ] `LevelThresholds(fallback is LogLevel, overrides is %LogName:LogLevel)`
  - [ ] `allowed(name is LogName, level is LogLevel) is bool`
- [ ] object `AllowedLogs` does `LogWriting`
  - [ ] `AllowedLogs(origin is LogWriting, allowing is LogAllowing)`
  - [ ] `write(origin is LogRecord) is void`
- [ ] object `EnabledLog`
  - [ ] `EnabledLog(origin is LogAllowing, name is LogName, level is LogLevel)`
  - [ ] `#projected value() is bool`
- [ ] Match the longest exact name-prefix override by `LogName` segments.
- [ ] Let a threshold admit its own level and every more severe level.
- [ ] Filter before formatting and I/O.
- [ ] Use `EnabledLog` only when constructing a record would be materially expensive.
- [ ] Keep field- and message-dependent policy in explicit application `LogWriting` decorators.
- [ ] Do not make filtering mutate records.

## `logging/redact`

- [ ] object `RedactedLogs` does `LogWriting`
  - [ ] `RedactedLogs(origin is LogWriting, names is Set<string>, replacement is JsonValue)`
  - [ ] `write(origin is LogRecord) is void`
- [ ] object `RedactedCauseLogs` does `LogWriting`
  - [ ] `RedactedCauseLogs(origin is LogWriting, replacement is string)`
  - [ ] `write(origin is LogRecord) is void`
- [ ] Replace matching top-level field values without removing their names or positions.
- [ ] Apply redaction before broadcasting, queuing, formatting or persistence.
- [ ] Treat field-name matching as exact and case-sensitive.
- [ ] Replace a cause with public replacement text without inspecting private state.
- [ ] Do not pretend key-based redaction can discover secrets embedded in arbitrary messages.

## `logging/line`

- [ ] object `TextLogLine`
  - [ ] `TextLogLine(origin is LogRecord, zone is TimeZone)`
  - [ ] `string() is string`
- [ ] object `JsonLogLine`
  - [ ] `JsonLogLine(origin is LogRecord)`
  - [ ] `string() is string`
- [ ] Render text timestamps as RFC 3339 with an explicit offset.
- [ ] Render level, logger name, process, message, fields and cause in a stable order.
- [ ] Escape newlines and control characters in text records so one record occupies one physical line.
- [ ] Encode JSON records through Stage 13 without hand-built quoting.
- [ ] Put application fields under a dedicated `fields` JSON member to prevent reserved-key collisions.
- [ ] Represent an absent cause by omitting the JSON member.
- [ ] Return no trailing line ending from a log-line object.
- [ ] Do not support an unrestricted interpolation template language for log formats.

## `logging/stream`

- [ ] object `TextStreamLogs` does `LogWriting`
  - [ ] `TextStreamLogs(origin is Writing, zone is TimeZone)`
  - [ ] `write(origin is LogRecord) is void`
- [ ] object `JsonStreamLogs` does `LogWriting`
  - [ ] `JsonStreamLogs(origin is Writing)`
  - [ ] `write(origin is LogRecord) is void`
- [ ] object `TextTerminalLogs` does `LogWriting`
  - [ ] `TextTerminalLogs(origin is TerminalOutput, zone is TimeZone, styles is %LogLevel:TextStyle)`
  - [ ] `write(origin is LogRecord) is void`
- [ ] object `JsonTerminalLogs` does `LogWriting`
  - [ ] `JsonTerminalLogs(origin is TerminalOutput, styles is %LogLevel:TextStyle)`
  - [ ] `write(origin is LogRecord) is void`
- [ ] Encode formatted records as UTF-8 and append exactly one newline.
- [ ] Submit the complete encoded record through one all-or-error `Writing.write()` action.
- [ ] Make terminal log sinks style the complete rendered record according to its level.
- [ ] Let terminal capability and `ColorPolicy` decide whether style sequences are emitted.
- [ ] Keep flushing under the caller's control through the underlying `Writing` capability.

## `logging/file`

- [ ] object `FileLogOptions`
  - [ ] `FileLogOptions(permissions is PermissionMode, flush_each is bool, sync_each is bool)`
  - [ ] `permissions() is PermissionMode`
  - [ ] `flush_each() is bool`
  - [ ] `sync_each() is bool`
- [ ] object `LogFile` does `Writing, Closing`
  - [ ] `LogFile(path is Path, options is FileLogOptions)`
  - [ ] `write(source is ByteSource) is void`
  - [ ] `flush() is void`
  - [ ] `reopen() is void`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] Open the file lazily in append mode with close-on-exec.
- [ ] Create a missing file with the requested permissions without changing an existing file's permissions.
- [ ] Encode and append each complete record under the sink's serialization boundary.
- [ ] Make `flush_each` flush userspace buffers and `sync_each` request durable storage.
- [ ] Make `reopen()` support external rotation without losing a record between handles.
- [ ] Keep multi-process record atomicity outside the portable guarantee.
- [ ] Surface disk-full, permission and short-write failures.

## `logging/rotation`

- [ ] role `Rotating`
  - [ ] `needed(size is BigInt, opened is Instant, now is Instant) is bool`
  - [ ] `suffix(now is Instant, index is int) is string`
- [ ] object `SizeRotating` does `Rotating`
  - [ ] `SizeRotating(limit is BigInt)`
  - [ ] `needed(size is BigInt, opened is Instant, now is Instant) is bool`
  - [ ] `suffix(now is Instant, index is int) is string`
- [ ] object `IntervalRotating` does `Rotating`
  - [ ] `IntervalRotating(interval is Duration, zone is TimeZone)`
  - [ ] `needed(size is BigInt, opened is Instant, now is Instant) is bool`
  - [ ] `suffix(now is Instant, index is int) is string`
- [ ] object `RotatingLogFile` does `Writing, Closing`
  - [ ] `RotatingLogFile(path is Path, options is FileLogOptions, rotation is Rotating, backups is int, clock is Timing)`
  - [ ] `write(source is ByteSource) is void`
  - [ ] `flush() is void`
  - [ ] `reopen() is void`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] Reject non-positive size and time limits and negative backup counts.
- [ ] Check rotation before appending the next complete record.
- [ ] Rotate through atomic renames when the filesystem supports them.
- [ ] Keep at most the configured number of completed files.
- [ ] Never split one record across files.
- [ ] Keep rotation serialized within one sink.
- [ ] Document that several processes must not rotate the same path independently.
- [ ] Compose Stage 16 compression explicitly after a file is no longer active.

## `logging/queue`

- [ ] enum `LogOverflow`
  - [ ] `drop_newest`
  - [ ] `drop_oldest`
- [ ] role `LogDraining`
  - [ ] `async drain() is LogDraining`
- [ ] object `QueuedLogs` does `LogWriting, LogDraining, Closing`
  - [ ] `QueuedLogs(origin is LogWriting, capacity is int, overflow is LogOverflow, scope is Supervising<int>)`
  - [ ] `write(origin is LogRecord) is void`
  - [ ] `async drain() is QueuedLogs`
  - [ ] `dropped_records() is BigInt`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] Reject non-positive capacity.
- [ ] Keep accepted immutable record references in a private bounded deque.
- [ ] Mutate the deque and overflow counter only inside one non-suspending object action.
- [ ] Wake the worker through a Stage 19 `Event` when the queue changes.
- [ ] Never block a logging caller waiting for queue capacity.
- [ ] Apply the configured overflow decision atomically and increment `dropped_records()`.
- [ ] Preserve FIFO order among records that remain queued.
- [ ] Register one lazily started delivery worker in the supplied structured-concurrency scope.
- [ ] Let that one worker deliver records serially to the destination.
- [ ] Make `drain()` wait until every record accepted before the call has been delivered or failed.
- [ ] Propagate the first downstream failure from `drain()` and reject further records.
- [ ] Make `close()` reject new records and cancel undrained work; call `drain()` first for graceful shutdown.
- [ ] Do not spawn hidden unmanaged tasks.

## `logging/syslog`

- [ ] enum `SyslogFacility`
  - [ ] `kernel`
  - [ ] `user`
  - [ ] `mail`
  - [ ] `daemon`
  - [ ] `authentication`
  - [ ] `syslog`
  - [ ] `printer`
  - [ ] `news`
  - [ ] `uucp`
  - [ ] `cron`
  - [ ] `authentication_private`
  - [ ] `ftp`
  - [ ] `local0`
  - [ ] `local1`
  - [ ] `local2`
  - [ ] `local3`
  - [ ] `local4`
  - [ ] `local5`
  - [ ] `local6`
  - [ ] `local7`
- [ ] object `SyslogLogs` does `LogWriting, LogDraining, Closing`
  - [ ] `SyslogLogs(origin is UdpConnection, facility is SyslogFacility, host is string, application is string, capacity is int, overflow is LogOverflow, scope is Supervising<int>)`
  - [ ] `write(origin is LogRecord) is void`
  - [ ] `async drain() is SyslogLogs`
  - [ ] `dropped_records() is BigInt`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] Format RFC 5424 version 1 messages with valid PRI and severity mapping.
- [ ] Use `-` for unavailable hostname, application, process, message ID and structured data.
- [ ] Encode message text as UTF-8 with the BOM required by RFC 5424 for unambiguous Unicode.
- [ ] Reject a formatted datagram above the configured UDP payload limit; never truncate it silently.
- [ ] Reuse queued overflow, drain and failure semantics.
- [ ] Keep TCP, TLS and local-system journal transports out of the first portable syslog sink.

## Stage 25 verification

- [ ] Add level parsing, rendering and ordering cases.
- [ ] Add segmented logger-name comparison and prefix-threshold cases.
- [ ] Add field ordering, replacement, nesting and missing-field cases.
- [ ] Add record timestamp, process, cause and immutability cases.
- [ ] Verify logger and call-field precedence at every level.
- [ ] Verify filtering happens before formatting and I/O.
- [ ] Add exact-name redaction and cause-redaction cases.
- [ ] Add multiline, control-character and malformed-text formatting cases.
- [ ] Add JSON Lines round-trip tests through the Stage 13 parser.
- [ ] Add broadcast partial-failure, fallback and best-effort cases.
- [ ] Add file append, permission, flush, sync, reopen and disk-full cases.
- [ ] Add size and interval rotation boundary, retention and rename-failure cases.
- [ ] Add queued FIFO, overflow, drain, close and downstream-failure races.
- [ ] Add RFC 5424 priority, timestamp, Unicode and size-limit vectors.
- [ ] Test syslog against a deterministic loopback UDP receiver.
- [ ] Prove no sink failure recursively logs through itself.
- [ ] Prove redacted values do not appear in formatted, queued or fallback copies.
- [ ] Add one Perl-level regression file per public module group.
- [ ] Add runnable structured, filtered, rotating, queued and syslog examples.
- [ ] Run `git diff --check`.

## Stage 25 exclusions

- [ ] Do not derive the v3 API from existing logging modules.
- [ ] Do not add logging methods to builtin strings, errors or collections.
- [ ] Do not add a process-global logger, mutable registry or implicit logger hierarchy.
- [ ] Do not infer logger names from packages, files or object names.
- [ ] Do not inspect arbitrary objects or `any` values to manufacture fields.
- [ ] Do not infer secrets or promise message-body redaction.
- [ ] Do not swallow sink failures unless an explicit decorator requests it.
- [ ] Do not block callers for queue capacity or perform hidden unbounded buffering.
- [ ] Do not rotate one file concurrently from several processes.
- [ ] Do not add arbitrary formatting templates or execute code from logging configuration.
- [ ] Do not conflate log levels with traces, spans, metrics or audit-event semantics.
- [ ] Keep trace correlation, diagnostic stacks and telemetry export in Stage 28.
- [ ] Keep systemd journal, Windows Event Log and Apple unified logging in Stage 34.

---

# Stage 26 — templates and locale

## `runtime/locale` substrate

- [x] Reuse the Stage 06 pinned Unicode character database.
- [x] Reuse immutable Stage 12 environment snapshots.
- [x] Reuse the Stage 10 IANA timezone database and civil-time objects.
- [x] Keep the runtime free of process-global locale mutation.
- [ ] Add a native immutable snapshot of platform-preferred language tags.
- [ ] Return Windows and Apple preferred languages through Unicode APIs rather than legacy code pages.
- [ ] Keep POSIX `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` and `LANG` interpretation in stdlib policy.
- [ ] Do not call host `setlocale()` for formatting, parsing or collation.
- [ ] Do not delegate portable output to host-dependent `strftime()`, `strcoll()` or locale databases.
- [ ] Keep templates, CLDR data, message catalogs and plural rules out of the runtime.

## `template/path`

- [ ] object `TemplatePath` does `Comparing<TemplatePath>`
  - [ ] `TemplatePath(segments is Sequencing<string>)`
  - [ ] `segments() is Sequencing<string>`
  - [ ] `order(other is TemplatePath) is Order`
  - [ ] `string() is string`
- [ ] object `TemplatePathFromString`
  - [ ] `TemplatePathFromString(origin is string)`
  - [ ] `#projected value() is TemplatePath`
- [ ] object `InvalidTemplatePath` does `Error`
  - [ ] `InvalidTemplatePath(reason is string)`
  - [ ] `text() is string`
- [ ] Reject empty paths and empty segments.
- [ ] Split source paths on literal dots without trimming or case folding.
- [ ] Keep path segments as identity and make `string()` diagnostic.
- [ ] Reserve the single dot path for the current section item.

## `template/model`

- [ ] object `TemplateLimits`
  - [ ] `TemplateLimits(depth is int, partial_depth is int, iterations is BigInt, output_bytes is BigInt)`
  - [ ] `depth() is int`
  - [ ] `partial_depth() is int`
  - [ ] `iterations() is BigInt`
  - [ ] `output_bytes() is BigInt`
- [ ] role `TemplateRendering`
  - [ ] `string(data is JsonValue, partials is TemplateLibrary, limits is TemplateLimits) is string`
- [ ] object `TemplateLibrary`
  - [ ] `TemplateLibrary(values is %string:TemplateRendering)`
  - [ ] `membership(name is string) is bool`
  - [ ] `value(name is string) is TemplateRendering`
  - [ ] `names() is Sequencing<string>`
- [ ] object `EmptyTemplateLibrary`
  - [ ] `#projected value() is TemplateLibrary`
- [ ] object `MissingTemplate` does `Error`
  - [ ] `MissingTemplate(name is string)`
  - [ ] `name() is string`
  - [ ] `text() is string`
- [ ] Reject non-positive limits and empty partial names.
- [ ] Keep compiled templates and libraries immutable.
- [ ] Preserve partial declaration order.
- [ ] Resolve partials only through the explicitly supplied library.
- [ ] Do not let a template open files, import modules or access the environment.

## `template/compile`

- [ ] object `TextTemplate` does `TemplateRendering`
  - [ ] `TextTemplate(origin is string)`
  - [ ] `source() is string`
  - [ ] `string(data is JsonValue, partials is TemplateLibrary, limits is TemplateLimits) is string`
- [ ] object `TemplateFailure` does `Error`
  - [ ] `TemplateFailure(offset is int, line is int, column is int, message is string)`
  - [ ] `offset() is int`
  - [ ] `line() is int`
  - [ ] `column() is int`
  - [ ] `text() is string`
- [ ] Parse variables as `{{ path }}`.
- [ ] Parse sections as `{{# path }}...{{/ path }}`.
- [ ] Parse inverted sections as `{{^ path }}...{{/ path }}`.
- [ ] Parse static partials as `{{> name }}`.
- [ ] Parse comments as `{{! text }}`.
- [ ] Escape a literal opening delimiter as `{{{{` and closing delimiter as `}}}}`.
- [ ] Reject mismatched, crossing and unclosed sections with source positions.
- [ ] Reject dynamic partial names and delimiter-changing directives.
- [ ] Preserve ordinary whitespace exactly.
- [ ] Apply standalone-line removal only to section, partial and comment tags occupying the whole logical line.
- [ ] Compile to a bounded immutable instruction tree before rendering.

## `template/render`

- [ ] object `TemplateOutput`
  - [ ] `TemplateOutput(origin is TemplateRendering, data is JsonValue, partials is TemplateLibrary, limits is TemplateLimits, destination is WritingText)`
  - [ ] `write() is void`
- [ ] object `MissingTemplateValue` does `Error`
  - [ ] `MissingTemplateValue(path is TemplatePath)`
  - [ ] `path() is TemplatePath`
  - [ ] `text() is string`
- [ ] object `NonScalarTemplateValue` does `Error`
  - [ ] `NonScalarTemplateValue(path is TemplatePath, kind is JsonKind)`
  - [ ] `path() is TemplatePath`
  - [ ] `kind() is JsonKind`
  - [ ] `text() is string`
- [ ] Resolve a path only through `JsonObject` members and the current section item.
- [ ] Render JSON strings as their unquoted text and numbers in canonical decimal form.
- [ ] Render booleans as canonical `true` or `false` and null as an empty string.
- [ ] Reject array and object interpolation outside a section.
- [ ] Iterate array sections in order and render object sections once with that object as context.
- [ ] Render scalar sections once when truthy and inverted sections once when false or absent.
- [ ] Define false, null, an empty string, an empty array and absence as false; define zero as true.
- [ ] Make ordinary missing interpolation a typed failure rather than silently losing text.
- [ ] Enforce nesting, partial, iteration and UTF-8 output-byte limits incrementally.
- [ ] Detect direct and indirect partial recursion through the partial-depth limit.
- [ ] Make `TemplateOutput` stream bounded chunks without constructing the complete output string.
- [ ] Make string rendering and streamed rendering byte-for-byte identical.

## `template/html`

- [ ] object `HtmlTemplate` does `TemplateRendering`
  - [ ] `HtmlTemplate(origin is string)`
  - [ ] `source() is string`
  - [ ] `string(data is JsonValue, partials is TemplateLibrary, limits is TemplateLimits) is string`
- [ ] Bind template source during construction and validate it lazily on the first `string()` builder.
- [ ] Permit interpolation only in ordinary HTML text and fully quoted ordinary attribute values.
- [ ] Escape text through Stage 13 `EscapedHtml` without quote escaping.
- [ ] Escape quoted attribute values through `EscapedHtml` with quote escaping.
- [ ] Reject interpolation in tag names, attribute names, comments and declarations.
- [ ] Reject interpolation inside `script`, `style`, raw-text and escapable-raw-text elements.
- [ ] Reject interpolation in event-handler, `srcdoc`, URL-bearing and CSS-bearing attributes.
- [ ] Require every partial used by an `HtmlTemplate` to be an `HtmlTemplate`.
- [ ] Validate the composed contextual state at each partial boundary.
- [ ] Do not provide an unescaped or trusted-markup interpolation form.
- [ ] Do not claim browser sanitization for literal markup authored in the template itself.

## `template/syntax exclusions`

- [ ] Do not add arithmetic, boolean expressions or comparisons.
- [ ] Do not add function calls, filters, pipes, macros or user-defined directives.
- [ ] Do not add assignment, mutation, includes by path or template inheritance.
- [ ] Do not expose application objects, reflection or arbitrary `any` values.
- [ ] Do not add shell, SQL, JavaScript, CSS or JSON source templating helpers.
- [ ] Build SQL through Stage 23 parameter binding and commands through Stage 12 argument sequences.

## `locale/tag`

- [ ] object `Locale` does `Comparing<Locale>`
  - [ ] `Locale(language is string)`
  - [ ] `Locale(language is string, region is string)`
  - [ ] `Locale(language is string, script is string, region is string, variants is Sequencing<string>)`
  - [ ] `language() is string`
  - [ ] `script() is string`
  - [ ] `region() is string`
  - [ ] `string() is string`
  - [ ] `order(other is Locale) is Order`
- [ ] object `LocaleFromLanguageTag`
  - [ ] `LocaleFromLanguageTag(origin is string)`
  - [ ] `#projected value() is Locale`
- [ ] object `LocaleExtensions`
  - [ ] `LocaleExtensions(origin is Locale)`
  - [ ] `unicode() is %string:Sequencing<string>`
  - [ ] `transformations() is Sequencing<string>`
  - [ ] `private_use() is Sequencing<string>`
- [ ] object `LocaleVariants`
  - [ ] `LocaleVariants(origin is Locale)`
  - [ ] `values() is Sequencing<string>`
- [ ] object `MissingLocaleSubtag` does `Error`
  - [ ] `MissingLocaleSubtag(kind is string)`
  - [ ] `text() is string`
- [ ] object `InvalidLocale` does `Error`
  - [ ] `InvalidLocale(offset is int, reason is string)`
  - [ ] `offset() is int`
  - [ ] `text() is string`
- [ ] Implement current BCP 47 syntax and IANA language-subtag aliases.
- [ ] Canonicalize language to lowercase, script to title case and region to uppercase.
- [ ] Preserve registered variants and Unicode, transformed and private-use extensions.
- [ ] Reject duplicate variants, extension singletons and Unicode extension keys.
- [ ] Make absent `script()` and `region()` projections typed failures.
- [ ] Compare canonical complete tags, not localized display names.
- [ ] Do not accept POSIX locale suffixes such as `.UTF-8` or `@euro` as BCP 47 syntax.

## `locale/data`

- [ ] object `LocaleDataVersion`
  - [ ] `cldr() is string`
  - [ ] `unicode() is string`
  - [ ] `currencies() is string`
- [ ] object `LocaleFallbacks` does `Sequencing<Locale>`
  - [ ] `LocaleFallbacks(origin is Locale)`
  - [ ] `cursor() is Cursor<Locale>`
  - [ ] `size() is int`
  - [ ] `item(index is int) is Locale`
  - [ ] `first() is Locale`
  - [ ] `last() is Locale`
- [ ] Bundle one reviewed and pinned CLDR release.
- [ ] Generate compact immutable locale tables at build time.
- [ ] Bundle CLDR parent-locale, likely-subtag, numbering, plural, date, currency and collation data used by public APIs.
- [ ] Expose the exact CLDR, Unicode and ISO 4217 data versions.
- [ ] Apply CLDR parent-locale rules before structural subtag removal.
- [ ] End every data fallback chain at the CLDR root locale.
- [ ] Keep language-tag negotiation distinct from CLDR data fallback.
- [ ] Never depend on the host locale database for deterministic output.

## `locale/system`

- [ ] object `PreferredLocales` does `Sequencing<Locale>`
  - [ ] `PreferredLocales(origin is Environment)`
  - [ ] `cursor() is Cursor<Locale>`
  - [ ] `size() is int`
  - [ ] `item(index is int) is Locale`
  - [ ] `first() is Locale`
  - [ ] `last() is Locale`
- [ ] Read POSIX preference in `LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, then `LANG` order.
- [ ] Treat `C` and `POSIX` as the invariant root locale.
- [ ] Strip only validated POSIX charset and modifier suffixes before BCP 47 conversion.
- [ ] Prefer the native platform language snapshot when environment convention is inapplicable.
- [ ] Deduplicate canonical locales while preserving preference order.
- [ ] Return the invariant root locale when no valid preference exists.
- [ ] Do not mutate environment variables or process locale state.

## `locale/match`

- [ ] object `LookedUpLocale`
  - [ ] `LookedUpLocale(requested is Sequencing<Locale>, available is Sequencing<Locale>, fallback is Locale)`
  - [ ] `#projected value() is Locale`
- [ ] object `FilteredLocales` does `Sequencing<Locale>`
  - [ ] `FilteredLocales(requested is Sequencing<Locale>, available is Sequencing<Locale>)`
  - [ ] `cursor() is Cursor<Locale>`
  - [ ] `size() is int`
  - [ ] `item(index is int) is Locale`
  - [ ] `first() is Locale`
  - [ ] `last() is Locale`
- [ ] object `LikelyLocale`
  - [ ] `LikelyLocale(origin is Locale)`
  - [ ] `#projected value() is Locale`
- [ ] Implement RFC 4647 lookup and basic filtering over canonical language ranges.
- [ ] Preserve caller preference order and available-locale order deterministically.
- [ ] Return only locales from the supplied available set.
- [ ] Use CLDR likely subtags only through the explicit `LikelyLocale` object.
- [ ] Do not use likely-subtag expansion implicitly during RFC 4647 matching.

## `locale/number`

- [ ] enum `NumberNotation`
  - [ ] `standard`
  - [ ] `scientific`
  - [ ] `engineering`
  - [ ] `compact_short`
  - [ ] `compact_long`
- [ ] enum `NumberGrouping`
  - [ ] `never`
  - [ ] `automatic`
  - [ ] `always`
  - [ ] `minimum_two`
- [ ] enum `NumberSign`
  - [ ] `automatic`
  - [ ] `always`
  - [ ] `except_zero`
  - [ ] `negative_only`
  - [ ] `never`
- [ ] object `NumberPrecision`
  - [ ] `NumberPrecision(minimum_integer is int, minimum_fraction is int, maximum_fraction is int, rounding is Rounding)`
  - [ ] `minimum_integer() is int`
  - [ ] `minimum_fraction() is int`
  - [ ] `maximum_fraction() is int`
  - [ ] `rounding() is Rounding`
- [ ] object `NumberFormat`
  - [ ] `NumberFormat(notation is NumberNotation, grouping is NumberGrouping, sign is NumberSign, precision is NumberPrecision)`
  - [ ] `notation() is NumberNotation`
  - [ ] `grouping() is NumberGrouping`
  - [ ] `sign() is NumberSign`
  - [ ] `precision() is NumberPrecision`
- [ ] object `LocalizedInteger`
  - [ ] `LocalizedInteger(origin is int, locale is Locale, format is NumberFormat)`
  - [ ] `string() is string`
- [ ] object `LocalizedBigInteger`
  - [ ] `LocalizedBigInteger(origin is BigInt, locale is Locale, format is NumberFormat)`
  - [ ] `string() is string`
- [ ] object `LocalizedDecimal`
  - [ ] `LocalizedDecimal(origin is Decimal, locale is Locale, format is NumberFormat)`
  - [ ] `string() is string`
- [ ] object `LocalizedFloat`
  - [ ] `LocalizedFloat(origin is float, locale is Locale, format is NumberFormat)`
  - [ ] `string() is string`
- [ ] object `LocalizedDecimalFromString`
  - [ ] `LocalizedDecimalFromString(origin is string, locale is Locale, format is NumberFormat)`
  - [ ] `#projected value() is Decimal`
- [ ] object `LocalizedIntegerFromString`
  - [ ] `LocalizedIntegerFromString(origin is string, locale is Locale, grouping is NumberGrouping)`
  - [ ] `#projected value() is BigInt`
- [ ] enum `PercentScale`
  - [ ] `percent`
  - [ ] `per_mille`
- [ ] object `LocalizedPercent`
  - [ ] `LocalizedPercent(origin is Decimal, locale is Locale, format is NumberFormat, scale is PercentScale)`
  - [ ] `string() is string`
- [ ] Validate precision ranges and reject `minimum_fraction > maximum_fraction`.
- [ ] Use locale numbering-system extensions only when bundled data supports them.
- [ ] Apply decimal rounding before digit substitution and grouping.
- [ ] Localize finite float values from their exact shortest decimal representation.
- [ ] Render localized NaN and infinity symbols explicitly.
- [ ] Parse strictly with the requested locale's digits, signs, decimal and grouping rules.
- [ ] Reject mixed digit systems and misplaced grouping separators.
- [ ] Scale percent and per-mille values before rounding and append the locale pattern's sign.
- [ ] Do not parse compact notation because its inverse is ambiguous.

## `locale/currency`

- [ ] object `Currency` does `Comparing<Currency>`
  - [ ] `Currency(code is string)`
  - [ ] `code() is string`
  - [ ] `digits() is int`
  - [ ] `order(other is Currency) is Order`
- [ ] enum `CurrencyDisplay`
  - [ ] `symbol`
  - [ ] `narrow_symbol`
  - [ ] `code`
  - [ ] `name`
- [ ] object `CurrencyFormat`
  - [ ] `CurrencyFormat(numbers is NumberFormat, display is CurrencyDisplay, accounting is bool, currency_digits is bool)`
  - [ ] `numbers() is NumberFormat`
  - [ ] `display() is CurrencyDisplay`
  - [ ] `accounting() is bool`
  - [ ] `currency_digits() is bool`
- [ ] object `LocalizedCurrency`
  - [ ] `LocalizedCurrency(amount is Decimal, currency is Currency, locale is Locale, format is CurrencyFormat)`
  - [ ] `string() is string`
- [ ] Validate uppercase ISO 4217 alphabetic codes against the pinned currency dataset.
- [ ] Override number precision with pinned currency fraction digits only when `currency_digits()` is true.
- [ ] Select localized currency names through CLDR plural rules.
- [ ] Keep currency formatting separate from exchange rates and monetary arithmetic.
- [ ] Do not infer a currency from locale or region.

## `locale/date-time`

- [ ] enum `DateStyle`
  - [ ] `full`
  - [ ] `long`
  - [ ] `medium`
  - [ ] `short`
- [ ] enum `TimeStyle`
  - [ ] `full`
  - [ ] `long`
  - [ ] `medium`
  - [ ] `short`
- [ ] object `DateTimeStyle`
  - [ ] `DateTimeStyle(date is DateStyle, time is TimeStyle)`
  - [ ] `date() is DateStyle`
  - [ ] `time() is TimeStyle`
- [ ] object `LocalizedDate`
  - [ ] `LocalizedDate(origin is Date, locale is Locale, style is DateStyle)`
  - [ ] `string() is string`
- [ ] object `LocalizedTime`
  - [ ] `LocalizedTime(origin is TimeOfDay, locale is Locale, style is TimeStyle)`
  - [ ] `string() is string`
- [ ] object `LocalizedLocalDateTime`
  - [ ] `LocalizedLocalDateTime(origin is LocalDateTime, locale is Locale, style is DateTimeStyle)`
  - [ ] `string() is string`
- [ ] object `LocalizedInstant`
  - [ ] `LocalizedInstant(origin is Instant, zone is TimeZone, locale is Locale, style is DateTimeStyle)`
  - [ ] `string() is string`
- [ ] Apply locale calendar and numbering extensions only when explicitly supported.
- [ ] Default to the Gregorian calendar without changing the instant or time zone.
- [ ] Use CLDR month, weekday, day-period, era and timezone-name data.
- [ ] Keep timezone selection explicit for every absolute instant.
- [ ] Preserve `LocalDateTime` as timezone-free.
- [ ] Do not add locale-dependent date parsing without an explicit unambiguous pattern.

## `locale/list`

- [ ] enum `ListKind`
  - [ ] `and`
  - [ ] `or`
  - [ ] `unit`
- [ ] enum `ListStyle`
  - [ ] `wide`
  - [ ] `short`
  - [ ] `narrow`
- [ ] object `LocalizedList`
  - [ ] `LocalizedList(values is Sequencing<string>, locale is Locale, kind is ListKind, style is ListStyle)`
  - [ ] `string() is string`
- [ ] Use distinct zero-, one-, two- and many-item CLDR patterns.
- [ ] Preserve item text exactly and never parse it as markup.
- [ ] Do not infer conjunction style from surrounding prose.

## `locale/collation`

- [ ] enum `CollationStrength`
  - [ ] `primary`
  - [ ] `secondary`
  - [ ] `tertiary`
  - [ ] `quaternary`
  - [ ] `identical`
- [ ] enum `CollationCaseFirst`
  - [ ] `locale_default`
  - [ ] `upper`
  - [ ] `lower`
  - [ ] `off`
- [ ] object `CollationOptions`
  - [ ] `CollationOptions(strength is CollationStrength, case_first is CollationCaseFirst, numeric is bool, normalization is bool)`
  - [ ] `strength() is CollationStrength`
  - [ ] `case_first() is CollationCaseFirst`
  - [ ] `numeric() is bool`
  - [ ] `normalization() is bool`
- [ ] object `CollatedString` does `Comparing<CollatedString>`
  - [ ] `CollatedString(origin is string, locale is Locale, options is CollationOptions)`
  - [ ] `string() is string`
  - [ ] `key() is @byte`
  - [ ] `order(other is CollatedString) is Order`
- [ ] Implement the Unicode Collation Algorithm with pinned CLDR tailoring.
- [ ] Make `order()` and byte-key ordering identical.
- [ ] Treat collation keys as opaque and valid only for the same data version, locale and options.
- [ ] Do not persist collation keys or use them as text identities.
- [ ] Reject comparison between values with different locale, options or pinned data versions.
- [ ] Sort localized text by decorating each value with `CollatedString` rather than passing a comparator service.
- [ ] Keep builtin string equality and ordering locale-independent.

## `i18n/plural`

- [ ] enum `PluralCategory`
  - [ ] `zero`
  - [ ] `one`
  - [ ] `two`
  - [ ] `few`
  - [ ] `many`
  - [ ] `other`
- [ ] enum `PluralKind`
  - [ ] `cardinal`
  - [ ] `ordinal`
- [ ] object `PluralCategoryOf`
  - [ ] `PluralCategoryOf(origin is Decimal, locale is Locale, kind is PluralKind)`
  - [ ] `#projected value() is PluralCategory`
- [ ] Evaluate pinned CLDR plural operands without converting through float.
- [ ] Distinguish visible fractional digits as required by CLDR rules.
- [ ] Require every plural message set to contain `other`.
- [ ] Do not assume `one` means the numeric value one in every locale.

## `i18n/catalog`

- [ ] object `MessageKey` does `Comparing<MessageKey>`
  - [ ] `MessageKey(context is string, identity is string)`
  - [ ] `context() is string`
  - [ ] `identity() is string`
  - [ ] `order(other is MessageKey) is Order`
- [ ] role `MessageEntry`
  - [ ] `key() is MessageKey`
  - [ ] `plural() is bool`
- [ ] object `SingularMessage` does `MessageEntry`
  - [ ] `SingularMessage(key is MessageKey, text is string)`
  - [ ] `key() is MessageKey`
  - [ ] `plural() is bool`
  - [ ] `text() is string`
- [ ] object `PluralMessage` does `MessageEntry`
  - [ ] `PluralMessage(key is MessageKey, forms is %PluralCategory:string)`
  - [ ] `key() is MessageKey`
  - [ ] `plural() is bool`
  - [ ] `forms() is %PluralCategory:string`
- [ ] role `Translating`
  - [ ] `membership(key is MessageKey) is bool`
  - [ ] `text(key is MessageKey) is string`
  - [ ] `plural(key is MessageKey, count is Decimal) is string`
- [ ] object `MessageCatalog` does `Translating`
  - [ ] `MessageCatalog(locale is Locale, entries is Sequencing<MessageEntry>)`
  - [ ] `membership(key is MessageKey) is bool`
  - [ ] `text(key is MessageKey) is string`
  - [ ] `plural(key is MessageKey, count is Decimal) is string`
- [ ] object `FallbackTranslation` does `Translating`
  - [ ] `FallbackTranslation(primary is Translating, fallback is Translating)`
  - [ ] `membership(key is MessageKey) is bool`
  - [ ] `text(key is MessageKey) is string`
  - [ ] `plural(key is MessageKey, count is Decimal) is string`
- [ ] object `MissingMessage` does `Error`
  - [ ] `MissingMessage(key is MessageKey)`
  - [ ] `key() is MessageKey`
  - [ ] `text() is string`
- [ ] object `WrongMessageEntry` does `Error`
  - [ ] `WrongMessageEntry(expected_plural is bool, actual_plural is bool)`
  - [ ] `expected_plural() is bool`
  - [ ] `actual_plural() is bool`
  - [ ] `text() is string`
- [ ] Reject duplicate keys, empty identities and plural sets without `other`.
- [ ] Make the wrong singular or plural projection a typed error.
- [ ] Select plural forms through the catalog locale's CLDR rule.
- [ ] Keep fallback order explicit and deterministic.
- [ ] Do not return source identifiers silently when a translation is missing.
- [ ] Format translated text through `TextTemplate` only when the caller explicitly composes them.

## `i18n/gettext`

- [ ] object `GettextCatalog` does `Translating`
  - [ ] `GettextCatalog(origin is @byte, locale is Locale)`
  - [ ] `membership(key is MessageKey) is bool`
  - [ ] `text(key is MessageKey) is string`
  - [ ] `plural(key is MessageKey, count is Decimal) is string`
- [ ] object `PoComments`
  - [ ] `PoComments(translator is Sequencing<string>, extracted is Sequencing<string>, references is Sequencing<string>, flags is Set<string>, previous is Sequencing<string>)`
  - [ ] `translator() is Sequencing<string>`
  - [ ] `extraction() is Sequencing<string>`
  - [ ] `references() is Sequencing<string>`
  - [ ] `flags() is Set<string>`
  - [ ] `previous_comments() is Sequencing<string>`
- [ ] object `PoEntry`
  - [ ] `PoEntry(context is string, identity is string, plural is string, translations is Sequencing<string>, comments is PoComments)`
  - [ ] `context() is string`
  - [ ] `identity() is string`
  - [ ] `plural() is string`
  - [ ] `translations() is Sequencing<string>`
  - [ ] `comments() is PoComments`
- [ ] object `PoDocument`
  - [ ] `PoDocument(metadata is %string:string, entries is Sequencing<PoEntry>)`
  - [ ] `metadata() is %string:string`
  - [ ] `entries() is Sequencing<PoEntry>`
- [ ] object `ParsedPo`
  - [ ] `ParsedPo(origin is string)`
  - [ ] `#projected value() is PoDocument`
- [ ] object `PoString`
  - [ ] `PoString(origin is PoDocument)`
  - [ ] `string() is string`
- [ ] object `InvalidCatalog` does `Error`
  - [ ] `InvalidCatalog(offset is int, reason is string)`
  - [ ] `offset() is int`
  - [ ] `text() is string`
- [ ] Parse GNU MO little- and big-endian revisions supported by the documented format.
- [ ] Bound string count, table offsets, individual strings and aggregate decoded text.
- [ ] Decode catalog text through its declared charset and reject unknown charsets.
- [ ] Support message context and plural translations.
- [ ] Parse `Plural-Forms` through a bounded expression parser; never host-evaluate catalog text.
- [ ] Require plural counts accepted by gettext catalogs to be integral `Decimal` values.
- [ ] Parse and write active PO comments, references, flags, contexts, plurals and multiline strings without semantic loss.
- [ ] Preserve obsolete PO entries as source text only in a separate future translation-tooling layer.
- [ ] Preserve entry and comment order.
- [ ] Do not install global `_()`, text domains or process locale state.

## Stage 26 verification

- [ ] Add template path, delimiter, nesting, whitespace and source-position cases.
- [ ] Add scalar, section, inverted, current-item and missing-value cases.
- [ ] Add partial recursion and every rendering-limit boundary.
- [ ] Prove string and streamed template rendering are byte-identical.
- [ ] Add HTML text, quoted-attribute and every rejected dangerous-context case.
- [ ] Run rendered HTML through independent HTML parsers and browser-oriented XSS fixtures.
- [ ] Add BCP 47 canonicalization, alias, extension and malformed-tag vectors.
- [ ] Add RFC 4647 lookup and filtering vectors.
- [ ] Add CLDR fallback and likely-subtag conformance data.
- [ ] Add number formatting and strict parsing cases across representative numbering systems.
- [ ] Add rounding, grouping, compact, percent, per-mille, NaN and infinity boundaries.
- [ ] Add currency symbols, names, accounting and fraction-digit cases.
- [ ] Add date and time styles across calendars, time zones, DST gaps and folds.
- [ ] Add zero-, one-, two- and many-item list cases.
- [ ] Run Unicode Collation Algorithm and CLDR tailoring conformance suites.
- [ ] Run CLDR cardinal and ordinal plural-rule fixtures without float conversion.
- [ ] Add catalog missing, fallback, context and plural cases.
- [ ] Add GNU MO endian, charset, context, plural and corrupt-offset cases.
- [ ] Add PO parsing and writing round trips preserving comments and order.
- [ ] Verify results are unchanged by the host process locale and timezone.
- [ ] Add one Perl-level regression file per public module group.
- [ ] Add runnable plain template, safe HTML, localized number, date and gettext examples.
- [ ] Run `git diff --check`.

## Stage 26 exclusions

- [ ] Do not derive the v3 API from existing template or locale modules.
- [ ] Do not add template rendering, translation or locale conversions to builtin strings.
- [ ] Do not create a general-purpose programming language inside templates.
- [ ] Do not expose application objects, reflection, filesystem or network access to templates.
- [ ] Do not call HTML escaping sufficient for JavaScript, CSS, URL or arbitrary markup contexts.
- [ ] Do not add shell, SQL or source-code templating helpers.
- [ ] Do not read or mutate ambient process locale state while formatting.
- [ ] Do not infer locale, currency, calendar or timezone from unrelated values.
- [ ] Do not use locale-sensitive collation for builtin string identity or map keys.
- [ ] Do not persist opaque collation keys.
- [ ] Do not silently return source message identifiers on missing translation.
- [ ] Do not execute gettext plural formulas through host-language evaluation.
- [ ] Do not add global gettext domains or `_()` bindings.
- [ ] Keep advanced ICU MessageFormat-style select expressions outside the first API.

---

# Stage 27 — testing

## `runtime/testing` substrate

- [x] Discover root-module, object and role `test` blocks in source order.
- [x] Qualify object and role test names and reject duplicates before execution.
- [x] Execute root tests instead of `main()` and require no `main()` in test mode.
- [x] Keep dependency tests parsed but outside the root test run.
- [x] Give every test an isolated lexical environment and assertion state.
- [x] Continue with later tests after an assertion, language failure, panic or cleanup failure.
- [x] Finish `defer` and object destruction before starting the next test.
- [x] Provide contextual `test.ok`, `test.is`, `test.isnt` and `test.fail` assertions.
- [x] Evaluate assertion arguments left-to-right exactly once.
- [x] Compare nested builtin collections structurally in `test.is` and `test.isnt`.
- [x] Accumulate several assertion failures without aborting the test body.
- [x] Keep the contextual test capability unavailable as a first-class value.
- [x] Restrict role fakes and fake state to test mode.
- [x] Validate role-fake action signatures, generics and module visibility statically.
- [x] Keep independently constructed role fakes and their state isolated.
- [ ] Support `async test "name" { ... }` and await its complete future before reporting.
- [ ] Make cancellation and timeout run async cleanup exactly once.
- [ ] Add `test.skip(reason is string)` as the fifth contextual test action.
- [ ] Attach assertion call-site module, line and column to every failure.
- [ ] Preserve a stdlib assertion's external test call site instead of reporting its implementation line.
- [ ] Record test start, duration, captured output and terminal status structurally.
- [ ] Bound captured stdout and stderr per test and report truncation.
- [ ] Add deterministic name filtering, shuffling seed and timeout inputs to the runner.
- [ ] Add process-isolated parallel execution without sharing interpreter or native-provider state.
- [ ] Add a test-only virtual-clock state owner that can mint internal monotonic instants safely.
- [ ] Add test-only recorder and stub state owners without exposing them in production mode.
- [ ] Replace the six-action `fake.storage` surface before v3.
- [ ] Expose contextual `fake.reading.get(key is K) is V`.
- [ ] Expose contextual `fake.reading.has(key is K) is bool`.
- [ ] Expose contextual `fake.reading.size() is int`.
- [ ] Expose contextual `fake.writing.set(key is K, value is V)`.
- [ ] Expose contextual `fake.writing.remove(key is K) is bool`.
- [ ] Expose contextual `fake.writing.clear()`.
- [ ] Keep both fake-state capabilities unavailable as first-class values.

## `testing/check`

- [ ] role `Checking`
  - [ ] `check() is void`
- [ ] role `AsyncChecking`
  - [ ] `async check() is AsyncChecking`
- [ ] object `CloseTo` does `Checking`
  - [ ] `CloseTo(actual is float, expected is float, relative is float, absolute is float)`
  - [ ] `check() is void`
- [ ] object `MatchingRegex` does `Checking`
  - [ ] `MatchingRegex(origin is string, pattern is Regex)`
  - [ ] `check() is void`
- [ ] object `TextEquals` does `Checking`
  - [ ] `TextEquals(actual is string, expected is string, context is int)`
  - [ ] `check() is void`
- [ ] object `BytesEqual` does `Checking`
  - [ ] `BytesEqual(actual is @byte, expected is @byte, context is int)`
  - [ ] `check() is void`
- [ ] object `SequenceEquals<T>` does `Checking`
  - [ ] `SequenceEquals(actual is Sequencing<T>, expected is Sequencing<T>, context is int)`
  - [ ] `check() is void`
- [ ] object `Eventually` does `AsyncChecking`
  - [ ] `Eventually(condition is () is bool, clock is Timing, timeout is Duration, interval is Duration)`
  - [ ] `async check() is Eventually`
- [ ] object `AsyncEventually` does `AsyncChecking`
  - [ ] `AsyncEventually(condition is async () is bool, clock is Timing, timeout is Duration, interval is Duration)`
  - [ ] `async check() is AsyncEventually`
- [ ] Make every checker report through the current contextual `test.fail()`.
- [ ] Reject checker execution outside a running test with a typed `MissingTestContext` error.
- [ ] Reject negative tolerances and non-positive diff context.
- [ ] Treat NaN as unequal to every value, including another NaN, in `CloseTo`.
- [ ] Compare infinity only by exact sign.
- [ ] Use `max(absolute, relative * max(abs(actual), abs(expected)))` for finite closeness.
- [ ] Report the first byte or item mismatch and a bounded surrounding diff.
- [ ] Use Stage 05 unified text diffs without reading files.
- [ ] Make `Eventually` evaluate immediately, then wait through the injected clock.
- [ ] Make `AsyncEventually` await each condition evaluation serially.
- [ ] Stop `Eventually` at the first successful exact `bool` result.
- [ ] Propagate a condition failure instead of retrying it as false.

## `testing/error`

- [ ] role `ErrorMatching`
  - [ ] `match(origin is Error) is bool`
  - [ ] `description() is string`
- [ ] object `AnyErrorMatching` does `ErrorMatching`
  - [ ] `AnyErrorMatching()`
  - [ ] `match(origin is Error) is bool`
  - [ ] `description() is string`
- [ ] object `ErrorTextMatching` does `ErrorMatching`
  - [ ] `ErrorTextMatching(expected is string)`
  - [ ] `match(origin is Error) is bool`
  - [ ] `description() is string`
- [ ] object `ErrorRegexMatching` does `ErrorMatching`
  - [ ] `ErrorRegexMatching(pattern is Regex)`
  - [ ] `match(origin is Error) is bool`
  - [ ] `description() is string`
- [ ] object `Throwing<R>` does `Checking`
  - [ ] `Throwing(action is () is R, matching is ErrorMatching)`
  - [ ] `check() is void`
- [ ] object `AsyncThrowing<R>` does `AsyncChecking`
  - [ ] `AsyncThrowing(action is async () is R, matching is ErrorMatching)`
  - [ ] `async check() is AsyncThrowing<R>`
- [ ] object `MissingTestContext` does `Error`
  - [ ] `text() is string`
- [ ] Fail when the action returns normally or throws a nonmatching typed `Error`.
- [ ] Preserve the original error as diagnostic cause when matching fails.
- [ ] Let panic and programming errors remain outside typed-error matching.
- [ ] Do not add runtime type reflection solely for exception assertions.
- [ ] Let applications implement precise domain matchers through `ErrorMatching`.

## `testing/time`

- [ ] object `ManualClock` does `Timing`
  - [ ] `ManualClock(start is Instant)`
  - [ ] `instant() is Instant`
  - [ ] `monotonic_instant() is MonotonicInstant`
  - [ ] `async wait(duration is Duration) is Timing`
  - [ ] `advance(duration is Duration) is void`
  - [ ] `set_wall(value is Instant) is void`
- [ ] Reject negative advances and waits.
- [ ] Advance wall and monotonic time together through `advance()`.
- [ ] Let `set_wall()` jump wall time without changing monotonic time.
- [ ] Complete every due waiter in deadline and registration order.
- [ ] Remove cancelled waiters promptly.
- [ ] Never sleep or read a host clock.
- [ ] Use ordinary dependency injection; do not replace `SystemClock` globally.

## `testing/generation`

- [ ] role `Generation<T>`
  - [ ] `sample(random is Randomizing, size is int) is Generated<T>`
- [ ] object `Generated<T>`
  - [ ] Do not expose a public constructor.
  - [ ] `value() is T`
  - [ ] `reductions() is Iterating<T>`
  - [ ] `string() is string`
- [ ] object `GeneratedElement<T>`
  - [ ] `GeneratedElement(value is T, string is string)`
  - [ ] `value() is T`
  - [ ] `string() is string`
- [ ] object `BooleanGeneration` does `Generation<bool>`
  - [ ] `BooleanGeneration()`
  - [ ] `sample(random is Randomizing, size is int) is Generated<bool>`
- [ ] object `IntegerGeneration` does `Generation<int>`
  - [ ] `IntegerGeneration(minimum is int, maximum is int)`
  - [ ] `sample(random is Randomizing, size is int) is Generated<int>`
- [ ] object `FiniteFloatGeneration` does `Generation<float>`
  - [ ] `FiniteFloatGeneration(minimum is float, maximum is float)`
  - [ ] `sample(random is Randomizing, size is int) is Generated<float>`
- [ ] object `ElementGeneration<T>` does `Generation<T>`
  - [ ] `ElementGeneration(values is Sequencing<GeneratedElement<T>>)`
  - [ ] `sample(random is Randomizing, size is int) is Generated<T>`
- [ ] object `SequenceGeneration<T>` does `Generation<Sequencing<T>>`
  - [ ] `SequenceGeneration(origin is Generation<T>, minimum is int, maximum is int)`
  - [ ] `sample(random is Randomizing, size is int) is Generated<Sequencing<T>>`
- [ ] object `FilteredGeneration<T>` does `Generation<T>`
  - [ ] `FilteredGeneration(origin is Generation<T>, condition is (T) is bool, attempts is int)`
  - [ ] `sample(random is Randomizing, size is int) is Generated<T>`
- [ ] object `GenerationExhausted` does `Error`
  - [ ] `GenerationExhausted(attempts is int)`
  - [ ] `attempts() is int`
  - [ ] `text() is string`
- [ ] Reject empty element sources, inverted ranges and invalid size bounds.
- [ ] Let size control complexity without changing configured hard bounds.
- [ ] Generate finite floats from documented bit patterns without NaN or infinity.
- [ ] Shrink toward simple canonical values and preserve generator invariants.
- [ ] Make every `Generated.reductions()` sequence finite, deterministic and duplicate-free.
- [ ] Filter shrink candidates through the same predicate.
- [ ] Fail after the explicit rejection-attempt bound.
- [ ] Carry stable reproduction text inside every `Generated` object instead of invoking a static representation service.

## `testing/property`

- [ ] object `Property<T>` does `Checking`
  - [ ] `Property(origin is Generation<T>, seed is int, cases is int, maximum_size is int, shrink_limit is int, condition is (T) is bool)`
  - [ ] `check() is void`
- [ ] object `AsyncProperty<T>` does `AsyncChecking`
  - [ ] `AsyncProperty(origin is Generation<T>, seed is int, cases is int, maximum_size is int, shrink_limit is int, condition is async (T) is bool)`
  - [ ] `async check() is AsyncProperty<T>`
- [ ] object `PropertyReproduction`
  - [ ] `PropertyReproduction(seed is int, case_index is int, size is int, value is string)`
  - [ ] `seed() is int`
  - [ ] `case_index() is int`
  - [ ] `size() is int`
  - [ ] `value() is string`
- [ ] Reject non-positive case counts and negative size or shrink limits.
- [ ] Create a private Stage 09 `PcgRandom` from the explicit seed.
- [ ] Increase size deterministically from zero through the configured maximum.
- [ ] Treat false, typed errors and cancellation as distinct property failures.
- [ ] Shrink only the first failing generated case.
- [ ] Explore shrink candidates deterministically within the explicit limit.
- [ ] Report seed, case index, size and smallest observed counterexample.
- [ ] Make the async property await one case at a time in generation order.
- [ ] Do not parallelize cases because that would change reproducibility and shrinking order.

## `testing/snapshot`

- [ ] object `SnapshotFile`
  - [ ] `SnapshotFile(root is Directory, name is string)`
  - [ ] `path() is Path`
- [ ] object `TextSnapshot` does `ByteSource`
  - [ ] `TextSnapshot(origin is string)`
  - [ ] `bytes() is @byte`
- [ ] object `JsonSnapshot` does `ByteSource`
  - [ ] `JsonSnapshot(origin is JsonValue)`
  - [ ] `bytes() is @byte`
- [ ] object `RawSnapshot` does `ByteSource`
  - [ ] `RawSnapshot(origin is @byte)`
  - [ ] `bytes() is @byte`
- [ ] object `Snapshot` does `Checking`
  - [ ] `Snapshot(origin is ByteSource, file is SnapshotFile, limit is BigInt)`
  - [ ] `check() is void`
- [ ] object `SnapshotUpdate`
  - [ ] `SnapshotUpdate(origin is ByteSource, file is SnapshotFile, limit is BigInt)`
  - [ ] `update() is void`
- [ ] Reject absolute names, parent traversal, separators and non-positive limits.
- [ ] Resolve every snapshot beneath its explicit root without following an escaping symlink.
- [ ] Normalize text snapshots to UTF-8 and LF only through `TextSnapshot`.
- [ ] Render JSON snapshots deterministically through Stage 13.
- [ ] Compare snapshot bytes exactly and report a bounded text or byte diff.
- [ ] Make a missing snapshot a failure during checking.
- [ ] Update snapshots only through the explicitly constructed `SnapshotUpdate` object.
- [ ] Write updates atomically through a sibling temporary file.
- [ ] Never let a normal test run rewrite expected data.

## `testing/double`

- [ ] object `Recorded<T>`
  - [ ] Do not expose a public constructor.
  - [ ] `index() is BigInt`
  - [ ] `value() is T`
- [ ] object `RecordedCalls<T>` does `Sizing`
  - [ ] `RecordedCalls()`
  - [ ] `record(value is T) is void`
  - [ ] `records() is Sequencing<Recorded<T>>`
  - [ ] `size() is int`
- [ ] object `Stub<T>` does `Sizing`
  - [ ] `Stub(values is Sequencing<T>)`
  - [ ] `value() is T`
  - [ ] `size() is int`
- [ ] object `StubExhausted` does `Error`
  - [ ] `text() is string`
- [ ] Make recorder and stub state test-mode-only runtime state owners.
- [ ] Preserve call and stub order exactly.
- [ ] Make `records()` return an immutable snapshot.
- [ ] Make `Stub.next()` throw after the configured sequence is exhausted.
- [ ] Construct fresh `RecordedCalls` and `Stub` objects instead of resetting test state.
- [ ] Compose recorders and stubs explicitly inside role fake actions.
- [ ] Do not generate mocks through reflection or string action names.
- [ ] Do not add ordered-expectation DSLs, implicit verification or call-count magic.

## `testing/result`

- [ ] enum `TestStatus`
  - [ ] `passed`
  - [ ] `failed`
  - [ ] `skipped`
  - [ ] `timed_out`
  - [ ] `crashed`
- [ ] object `TestPosition`
  - [ ] `TestPosition(module is string, line is int, column is int)`
  - [ ] `module() is string`
  - [ ] `line() is int`
  - [ ] `column() is int`
- [ ] object `TestFailure`
  - [ ] `TestFailure(message is string)`
  - [ ] `TestFailure(message is string, position is TestPosition)`
  - [ ] `message() is string`
  - [ ] `located() is bool`
  - [ ] `position() is TestPosition`
- [ ] object `TestOutput`
  - [ ] `TestOutput(output is @byte, errors is @byte, truncated is bool)`
  - [ ] `output() is @byte`
  - [ ] `errors() is @byte`
  - [ ] `truncated() is bool`
- [ ] object `TestResult`
  - [ ] Do not expose a public constructor.
  - [ ] `name() is string`
  - [ ] `status() is TestStatus`
  - [ ] `duration() is Duration`
  - [ ] `failures() is Sequencing<TestFailure>`
  - [ ] `output() is TestOutput`
- [ ] object `TestCounts`
  - [ ] `passes() is int`
  - [ ] `failures() is int`
  - [ ] `skips() is int`
  - [ ] `total() is int`
- [ ] object `TestReport`
  - [ ] Do not expose a public constructor.
  - [ ] `start() is Instant`
  - [ ] `duration() is Duration`
  - [ ] `seed() is int`
  - [ ] `counts() is TestCounts`
  - [ ] `results() is Sequencing<TestResult>`
- [ ] Preserve discovery order in reports even when process workers finish out of order.
- [ ] Make a missing `TestFailure.position()` a typed projection error.
- [ ] Keep captured output as bytes until an explicit report value decodes it.
- [ ] Count timeout and crash results as failed in the process exit status.
- [ ] Reserve `crashed` for runner or worker termination; ordinary assertions, typed errors and panics are `failed`.

## `testing/report`

- [ ] object `HumanTestReport`
  - [ ] `HumanTestReport(origin is TestReport, width is int)`
  - [ ] `string() is string`
- [ ] object `TapTestReport`
  - [ ] `TapTestReport(origin is TestReport)`
  - [ ] `string() is string`
- [ ] object `JUnitTestReport`
  - [ ] `JUnitTestReport(origin is TestReport, suite is string)`
  - [ ] `string() is string`
- [ ] object `JsonTestReport`
  - [ ] `JsonTestReport(origin is TestReport)`
  - [ ] `string() is string`
- [ ] Keep human output deterministic and free of ANSI styling.
- [ ] Emit valid TAP version 13 with escaped diagnostics.
- [ ] Emit JUnit XML through Stage 13 XML objects rather than string concatenation.
- [ ] Emit JSON through Stage 13 with stable member order.
- [ ] Include reproduction seeds and captured output without assuming UTF-8 validity.
- [ ] Escape invalid output bytes in human and TAP reports and Base64-encode them in XML and JSON reports.
- [ ] Apply Stage 24 terminal styling only through an explicit output decorator.

## Test runner behavior

- [ ] Run tests serially in discovery order by default.
- [ ] Select tests by exact qualified name or explicit Stage 14 regex.
- [ ] Report an unmatched selection as a runner error, not a successful empty run.
- [ ] Shuffle only with an explicit seed and print that seed in every report.
- [ ] Apply timeouts through Stage 19 cancellation and wait for cooperative cleanup.
- [ ] Escalate a stuck process-isolated test only after a documented grace period.
- [ ] Parallelize only process-isolated tests with an explicit worker bound.
- [ ] Give every process worker an explicit environment, directory and random seed.
- [ ] Keep role fakes, temporary resources and native providers local to one test process.
- [ ] Capture output independently for each test and replay failed-test output in report order.
- [ ] Return exit code zero only when every selected test passed or was skipped.
- [ ] Let Stage 32 discover project test entry modules and invoke this runner contract.
- [ ] Keep source coverage and profiling integration in Stage 28.

## Stage 27 verification

- [ ] Preserve every currently working test-block, assertion, isolation and cleanup behavior.
- [ ] Add async success, failure, cancellation, timeout and cleanup cases.
- [ ] Add skip, source-position, output-capture and truncation cases.
- [ ] Add exact selection, regex selection, shuffle and unmatched-selection cases.
- [ ] Add test-result ordering under deliberately reversed worker completion.
- [ ] Add close, regex, text, byte, sequence and eventually assertion cases.
- [ ] Add typed error matching for success, mismatch, panic and async failure.
- [ ] Add manual-clock waiter ordering, wall jump and cancellation cases.
- [ ] Add deterministic generator and shrink vectors for every bundled generator.
- [ ] Reproduce each property failure from its reported seed and case index.
- [ ] Add snapshot path escape, symlink, missing, mismatch, limit and atomic-update cases.
- [ ] Add role-fake state separation and recorder/stub ordering cases.
- [ ] Prove production mode cannot construct fakes, recorders or stubs.
- [ ] Parse generated TAP, JUnit XML and JSON reports independently.
- [ ] Test process-isolated timeouts and crashes without leaking workers.
- [ ] Add one Perl-level regression file per public module group.
- [ ] Add runnable assertion, role-fake, manual-clock, property and snapshot examples.
- [ ] Run `git diff --check`.

## Stage 27 exclusions

- [ ] Do not derive the v3 API from existing test stdlib modules.
- [ ] Do not add assertion methods to builtin values or `Error`.
- [ ] Do not add an inherited `TestCase`, fixture base class or lifecycle annotations.
- [ ] Construct fixtures as ordinary objects and clean resources explicitly through `defer` and `Closing`.
- [ ] Reuse Stage 11 temporary files and directories instead of adding test-specific copies.
- [ ] Do not inject fixtures by parameter name or hidden global registry.
- [ ] Do not generate mocks through reflection or accept stringly typed action expectations.
- [ ] Prefer real value objects, role fakes, recorders and deterministic collaborators.
- [ ] Do not make ordinary test order random or parallel by default.
- [ ] Do not add a special expected-failure status; assert the expected error explicitly or skip with a reason.
- [ ] Do not share mutable interpreter, environment or native-provider state between parallel tests.
- [ ] Do not rewrite snapshots without an explicit update object or tooling command.
- [ ] Do not make property tests depend on secure or ambient randomness.
- [ ] Do not hide reproduction seeds, swallowed failures or captured output.
- [ ] Keep doctest source extraction and project test discovery in Stage 32.
- [ ] Keep coverage, profiling and diagnostics enrichment in Stage 28.

---

# Stage 28 — diagnostics and observability

## Decision

- [ ] Keep diagnostics, tracing, metrics, profiling and coverage as separate module families.
- [ ] Share only small value objects such as attributes and source positions.
- [ ] Pass every collector, tracing, metrics and exporter explicitly.
- [ ] Keep logging usable without tracing and tracing usable without logging.
- [ ] Instrument HTTP, databases and logging through decorators instead of enlarging their core objects.
- [ ] Treat observability as bounded application work with ordinary failures and cleanup.
- [ ] Keep every public object at five action names or fewer, including inherited roles.

## Runtime substrate audit

- [x] Preserve the exact thrown `Error` value and its origin token in an immutable unwind carrier.
- [x] Keep thrown errors, panics, programming errors and normalized native failures distinct.
- [x] Preserve the primary failure while draining deferred and destruction cleanup.
- [x] Expose native failure domain, code and public message without leaking host internals.
- [x] Provide wall and monotonic clocks for timestamps and elapsed measurements.
- [x] Provide the process identifier needed by logs and telemetry resource fields.
- [ ] Give every parsed token a module, line, column and UTF-8 byte offset.
- [ ] Give every executable AST node a complete source range rather than only a line number.
- [ ] Record immutable action, constructor, native and async-continuation frames during execution.
- [ ] Attach the captured call stack to a failure carrier when it first crosses a language boundary.
- [ ] Preserve causal and suppressed cleanup failures without replacing the primary failure.
- [ ] Let explicit task and process-invocation payloads carry ordinary trace-context values unchanged.
- [ ] Add stable runtime task identifiers only for diagnostic correlation, never as ambient application identity.
- [ ] Report failed futures that become unreachable without their failure being observed.
- [ ] Expose bounded stack sampling points for the explicit profiler.
- [ ] Expose statement and branch hit points for the explicit coverage collector.
- [ ] Expose allocation size and allocation-site events only while an explicit allocation profiler is active.
- [ ] Replace the current line-only and debug-enter/debug-leave output with structured internal events.
- [ ] Keep the existing textual debug mode as a projected value over those internal events.
- [ ] Do not expose interpreter objects, Perl exceptions, host stack frames or mutable runtime state.

## `diagnostics/source`

- [ ] object `SourcePosition`
  - [ ] `SourcePosition(module is string, line is int, column is int, offset is int)`
  - [ ] `module() is string`
  - [ ] `line() is int`
  - [ ] `column() is int`
  - [ ] `offset() is int`
- [ ] role `SourceRange`
  - [ ] `known() is bool`
  - [ ] `start() is SourcePosition`
  - [ ] `end() is SourcePosition`
- [ ] object `KnownSourceRange` does `SourceRange`
  - [ ] `KnownSourceRange(start is SourcePosition, end is SourcePosition)`
  - [ ] `known() is bool`
  - [ ] `start() is SourcePosition`
  - [ ] `end() is SourcePosition`
- [ ] object `UnknownSourceRange` does `SourceRange`
  - [ ] `UnknownSourceRange()`
  - [ ] `known() is bool`
  - [ ] `start() is SourcePosition`
  - [ ] `end() is SourcePosition`
- [ ] role `SourceReading`
  - [ ] `string(module is string) is string`
- [ ] object `SourceExcerpt`
  - [ ] `SourceExcerpt(range is SourceRange, sources is SourceReading, context is int)`
  - [ ] `#projected value() is string`
- [ ] Number lines and columns from one and UTF-8 byte offsets from zero.
- [ ] Make ranges half-open and require both endpoints to belong to the same module.
- [ ] Make `start()` and `end()` fail explicitly for an unknown range.
- [ ] Keep module identifiers logical and stable; do not require a filesystem path.
- [ ] Make source access explicit so formatting a diagnostic cannot read files implicitly.
- [ ] Bound excerpt context, line length and total rendered size.

## `diagnostics/model`

- [ ] enum `DiagnosticSeverity`
  - [ ] `note`
  - [ ] `warning`
  - [ ] `error`
  - [ ] `fatal`
- [ ] enum `CallKind`
  - [ ] `action`
  - [ ] `constructor`
  - [ ] `native`
  - [ ] `async_continuation`
- [ ] object `DiagnosticCode`
  - [ ] `DiagnosticCode(domain is string, value is string)`
  - [ ] `domain() is string`
  - [ ] `value() is string`
  - [ ] `string() is string`
- [ ] object `CallFrame`
  - [ ] `CallFrame(name is string, kind is CallKind, range is SourceRange)`
  - [ ] `name() is string`
  - [ ] `kind() is CallKind`
  - [ ] `range() is SourceRange`
- [ ] object `DiagnosticNote`
  - [ ] `DiagnosticNote(message is string, range is SourceRange)`
  - [ ] `message() is string`
  - [ ] `range() is SourceRange`
- [ ] object `DiagnosticContext`
  - [ ] `DiagnosticContext(range is SourceRange, frames is @CallFrame, notes is @DiagnosticNote, causes is @Diagnostic, suppressed is @Diagnostic)`
  - [ ] `range() is SourceRange`
  - [ ] `frames() is @CallFrame`
  - [ ] `notes() is @DiagnosticNote`
  - [ ] `causes() is @Diagnostic`
  - [ ] `suppressed_diagnostics() is @Diagnostic`
- [ ] object `Diagnostic`
  - [ ] `Diagnostic(severity is DiagnosticSeverity, code is DiagnosticCode, message is string, context is DiagnosticContext)`
  - [ ] `severity() is DiagnosticSeverity`
  - [ ] `code() is DiagnosticCode`
  - [ ] `message() is string`
  - [ ] `context() is DiagnosticContext`
- [ ] object `DiagnosticReport`
  - [ ] `DiagnosticReport(diagnostics is @Diagnostic)`
  - [ ] `diagnostics() is @Diagnostic`
  - [ ] `failed() is bool`
- [ ] object `ErrorDiagnostic`
  - [ ] `ErrorDiagnostic(origin is Error, code is DiagnosticCode)`
  - [ ] `#projected value() is Diagnostic`
- [ ] Validate diagnostic domains and codes as portable lower-case identifiers.
- [ ] Use an unknown `SourceRange` and empty sequences instead of nullable members.
- [ ] Keep causal failures distinct from failures suppressed during cleanup.
- [ ] Detect cycles and enforce depth, frame, note and cause limits while building a bounded report.
- [ ] Convert an arbitrary `Error` to an unlocated diagnostic without inventing a stack.
- [ ] Let the runtime construct the rich diagnostic only from the actual failure carrier.
- [ ] Keep `Error` small; do not add diagnostic, source or stack actions to it.

## `diagnostics/format`

- [ ] object `HumanDiagnosticReport`
  - [ ] `HumanDiagnosticReport(origin is DiagnosticReport, sources is SourceReading)`
  - [ ] `string() is string`
- [ ] object `JsonDiagnosticReport`
  - [ ] `JsonDiagnosticReport(origin is DiagnosticReport)`
  - [ ] `string() is string`
- [ ] Render causes and suppressed failures as an explicit tree with a configured depth limit.
- [ ] Render frames from newest to oldest and collapse only consecutive identical frames.
- [ ] Keep human output deterministic and free of ANSI control sequences.
- [ ] Apply Stage 24 styling only in a separate terminal-output decorator.
- [ ] Encode machine output through Stage 13 JSON with stable field names and ordering.
- [ ] Preserve unknown locations as explicit JSON nulls while keeping public objects non-null.
- [ ] Escape control characters and bound messages, excerpts and complete reports.
- [ ] Do not expose host stack traces or native exception text in public diagnostics.

## `observability/attributes`

- [ ] object `AttributeValue`
  - [ ] `AttributeValue(value is JsonValue)`
  - [ ] `#projected value() is JsonValue`
- [ ] object `Attributes`
  - [ ] `Attributes()`
  - [ ] `Attributes(values is %string:AttributeValue)`
  - [ ] `Attributes(values is %string:AttributeValue, limits is TelemetryLimits)`
  - [ ] `membership(name is string) is bool`
  - [ ] `value(name is string) is AttributeValue`
  - [ ] `values() is %string:AttributeValue`
- [ ] object `CombinedAttributes`
  - [ ] `CombinedAttributes(base is Attributes, overriding is Attributes)`
  - [ ] `#projected value() is Attributes`
- [ ] object `TelemetryLimits`
  - [ ] `TelemetryLimits(attributes is int, events is int, batch_items is int, batch_bytes is int, queue_items is int)`
  - [ ] `attributes() is int`
  - [ ] `events() is int`
  - [ ] `batch_items() is int`
  - [ ] `batch_bytes() is int`
  - [ ] `queue_items() is int`
- [ ] Accept only JSON booleans, numbers, strings and homogeneous arrays of those scalar kinds.
- [ ] Reject nested objects, nulls, nested arrays and heterogeneous arrays as telemetry attributes.
- [ ] Validate names and enforce configured count, name-length and value-length limits.
- [ ] Give the overriding set precedence without mutating either input.
- [ ] Document that secrets and unbounded identifiers do not belong in attributes.

## `tracing/model`

- [ ] enum `SpanKind`
  - [ ] `internal`
  - [ ] `server`
  - [ ] `client`
  - [ ] `producer`
  - [ ] `consumer`
- [ ] enum `SpanStatus`
  - [ ] `unset`
  - [ ] `ok`
  - [ ] `error`
- [ ] object `TraceId`
  - [ ] `TraceId(bytes is @byte)`
  - [ ] `bytes() is @byte`
  - [ ] `string() is string`
- [ ] object `TraceIdFromString`
  - [ ] `TraceIdFromString(origin is string)`
  - [ ] `#projected value() is TraceId`
- [ ] object `SpanId`
  - [ ] `SpanId(bytes is @byte)`
  - [ ] `bytes() is @byte`
  - [ ] `string() is string`
- [ ] object `SpanIdFromString`
  - [ ] `SpanIdFromString(origin is string)`
  - [ ] `#projected value() is SpanId`
- [ ] object `TraceFlags`
  - [ ] `TraceFlags(sampled is bool)`
  - [ ] `sampled() is bool`
- [ ] object `TraceContext`
  - [ ] `TraceContext(trace is TraceId, span is SpanId, flags is TraceFlags, state is string)`
  - [ ] `trace() is TraceId`
  - [ ] `span() is SpanId`
  - [ ] `flags() is TraceFlags`
  - [ ] `state() is string`
- [ ] object `SpanEvent`
  - [ ] `SpanEvent(name is string, occurred is Instant, attributes is Attributes)`
  - [ ] `name() is string`
  - [ ] `time() is Instant`
  - [ ] `attributes() is Attributes`
- [ ] object `SpanLink`
  - [ ] `SpanLink(context is TraceContext, attributes is Attributes)`
  - [ ] `context() is TraceContext`
  - [ ] `attributes() is Attributes`
- [ ] role `SpanParent`
  - [ ] `root() is bool`
  - [ ] `context() is TraceContext`
- [ ] object `RootSpanParent` does `SpanParent`
  - [ ] `RootSpanParent()`
  - [ ] `root() is bool`
  - [ ] `context() is TraceContext`
- [ ] object `ChildSpanParent` does `SpanParent`
  - [ ] `ChildSpanParent(context is TraceContext)`
  - [ ] `root() is bool`
  - [ ] `context() is TraceContext`
- [ ] object `SpanStart`
  - [ ] `SpanStart(name is string, kind is SpanKind, attributes is Attributes, parent is SpanParent, links is @SpanLink)`
  - [ ] `name() is string`
  - [ ] `kind() is SpanKind`
  - [ ] `attributes() is Attributes`
  - [ ] `parent() is SpanParent`
  - [ ] `links() is @SpanLink`
- [ ] Make trace IDs exactly 16 bytes, span IDs exactly 8 bytes and reject all-zero IDs.
- [ ] Render IDs as fixed-width lower-case hexadecimal.
- [ ] Keep trace state opaque but validate its grammar and size at the boundary.
- [ ] Make asking a root `SpanParent` for `context()` fail explicitly.

## `tracing/span`

- [ ] role `SpanRecording`
  - [ ] `context() is TraceContext`
  - [ ] `annotate(name is string, value is AttributeValue) is void`
  - [ ] `record(origin is SpanEvent) is void`
  - [ ] `mark(value is SpanStatus, message is string) is void`
  - [ ] `end() is void`
- [ ] role `Tracing`
  - [ ] `span(origin is SpanStart) is SpanRecording`
- [ ] object `ConfiguredTracing` does `Tracing`
  - [ ] `ConfiguredTracing(ids is TraceIdentifying, sampling is TraceSamplingPolicy, clock is Timing, destination is SpanWriting)`
  - [ ] `span(origin is SpanStart) is SpanRecording`
- [ ] object `NoTracing` does `Tracing`
  - [ ] `NoTracing()`
  - [ ] `span(origin is SpanStart) is SpanRecording`
- [ ] object `SpanOperation`
  - [ ] `SpanOperation(name is string, kind is SpanKind, status is SpanStatus, message is string)`
  - [ ] `name() is string`
  - [ ] `kind() is SpanKind`
  - [ ] `status() is SpanStatus`
  - [ ] `message() is string`
- [ ] object `SpanTiming`
  - [ ] `SpanTiming(started is Instant, ended is Instant, duration is Duration)`
  - [ ] `start() is Instant`
  - [ ] `end() is Instant`
  - [ ] `duration() is Duration`
- [ ] object `FinishedSpan`
  - [ ] `FinishedSpan(context is TraceContext, parent is SpanParent, operation is SpanOperation, timing is SpanTiming, data is SpanData)`
  - [ ] `context() is TraceContext`
  - [ ] `parent() is SpanParent`
  - [ ] `operation() is SpanOperation`
  - [ ] `timing() is SpanTiming`
  - [ ] `data() is SpanData`
- [ ] object `SpanData`
  - [ ] `SpanData(attributes is Attributes, events is @SpanEvent, links is @SpanLink)`
  - [ ] `attributes() is Attributes`
  - [ ] `events() is @SpanEvent`
  - [ ] `links() is @SpanLink`
- [ ] Capture both wall start time and monotonic elapsed time.
- [ ] Make `end()` idempotent and export exactly one immutable finished span.
- [ ] Ignore recording after `end()` and count it as an internal dropped operation.
- [ ] Require callers to set error status explicitly; do not infer it from message text.
- [ ] Return a real no-op span from `NoTracing` so callers need no branches.
- [ ] Do not install an ambient current span or a process-global tracer.

## `tracing/sampling`

- [ ] role `TraceIdentifying`
  - [ ] `trace() is TraceId`
  - [ ] `span() is SpanId`
- [ ] object `SecureTraceIds` does `TraceIdentifying`
  - [ ] `SecureTraceIds(random is SecureRandom)`
  - [ ] `trace() is TraceId`
  - [ ] `span() is SpanId`
- [ ] role `TraceSamplingPolicy`
  - [ ] `sampled(origin is SpanStart, trace is TraceId) is bool`
- [ ] object `AlwaysSampling` does `TraceSamplingPolicy`
  - [ ] `AlwaysSampling()`
  - [ ] `sampled(origin is SpanStart, trace is TraceId) is bool`
- [ ] object `NeverSampling` does `TraceSamplingPolicy`
  - [ ] `NeverSampling()`
  - [ ] `sampled(origin is SpanStart, trace is TraceId) is bool`
- [ ] object `ProbabilitySampling` does `TraceSamplingPolicy`
  - [ ] `ProbabilitySampling(probability is float)`
  - [ ] `sampled(origin is SpanStart, trace is TraceId) is bool`
- [ ] object `ParentSampling` does `TraceSamplingPolicy`
  - [ ] `ParentSampling(root is TraceSamplingPolicy)`
  - [ ] `sampled(origin is SpanStart, trace is TraceId) is bool`
- [ ] Generate IDs from Stage 15 secure randomness, never from the ordinary seeded generator.
- [ ] Make probability decisions deterministic from the trace ID so all services agree.
- [ ] Preserve a valid parent's sampling decision in `ParentSampling`.
- [ ] Record no events or attributes for unsampled spans while preserving propagation context.

## `tracing/propagation`

- [ ] object `PropagationLimits`
  - [ ] `PropagationLimits(fields is int, bytes is int, member_bytes is int)`
  - [ ] `fields() is int`
  - [ ] `bytes() is int`
  - [ ] `member_bytes() is int`

- [ ] object `W3cTraceContext`
  - [ ] `W3cTraceContext(headers is Headers, limits is PropagationLimits)`
  - [ ] `#projected value() is TraceContext`
- [ ] object `W3cTraceHeaders`
  - [ ] `W3cTraceHeaders(origin is TraceContext)`
  - [ ] `#projected value() is Headers`
- [ ] object `Baggage`
  - [ ] `Baggage(values is %string:BaggageMember)`
  - [ ] `membership(name is string) is bool`
  - [ ] `member(name is string) is BaggageMember`
  - [ ] `members() is %string:BaggageMember`
- [ ] object `BaggageMember`
  - [ ] `BaggageMember(value is string, properties is %string:string)`
  - [ ] `value() is string`
  - [ ] `properties() is %string:string`
- [ ] object `W3cBaggage`
  - [ ] `W3cBaggage(header is string, limits is PropagationLimits)`
  - [ ] `#projected value() is Baggage`
- [ ] object `BaggageHeader`
  - [ ] `BaggageHeader(origin is Baggage)`
  - [ ] `#projected value() is string`
- [ ] Follow W3C `traceparent`, `tracestate` and `baggage` grammar exactly.
- [ ] Apply version-specific parsing and reject unsupported extensions conservatively.
- [ ] Return a documented invalid-context error instead of silently creating a new parent.
- [ ] Keep baggage separate from span attributes and propagate it only when explicitly requested.
- [ ] Apply strict member, byte and value limits before decoding baggage.
- [ ] Never place authorization, cookies or other request headers into baggage automatically.

## `tracing/export`

- [ ] role `SpanWriting`
  - [ ] `write(origin is FinishedSpan) is void`
- [ ] role `SpanExporting`
  - [ ] `async export(origin is @FinishedSpan) is SpanExporting`
- [ ] object `DiscardingSpans` does `SpanWriting`
  - [ ] `DiscardingSpans()`
  - [ ] `write(origin is FinishedSpan) is void`
- [ ] object `CollectingSpans` does `SpanWriting`
  - [ ] `CollectingSpans(limit is int)`
  - [ ] `write(origin is FinishedSpan) is void`
  - [ ] `spans() is @FinishedSpan`
  - [ ] `dropped_spans() is int`
- [ ] object `OtlpJsonSpans`
  - [ ] `OtlpJsonSpans(origin is @FinishedSpan, resource is Attributes)`
  - [ ] `#projected value() is JsonValue`
- [ ] object `OtlpHttpSpans` does `SpanExporting`
  - [ ] `OtlpHttpSpans(endpoint is Uri, headers is Headers, client is Exchanging, resource is Attributes, limits is TelemetryLimits)`
  - [ ] `async export(origin is @FinishedSpan) is OtlpHttpSpans`
- [ ] object `QueuedSpans` does `SpanWriting, Closing`
  - [ ] `QueuedSpans(destination is SpanExporting, limits is TelemetryLimits)`
  - [ ] `write(origin is FinishedSpan) is void`
  - [ ] `async run() is QueuedSpans`
  - [ ] `dropped_spans() is int`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] Batch by explicit item and byte limits before serialization.
- [ ] Use OTLP/HTTP JSON so Stage 28 does not require a new protobuf subsystem.
- [ ] Apply Stage 22 timeout, TLS and retry policy explicitly at construction.
- [ ] Retry only retryable transport failures and honor cancellation and server backoff.
- [ ] Make `SpanWriting.write()` synchronous, bounded and free of network I/O.
- [ ] Run `QueuedSpans.run()` inside a Stage 19 structured-concurrency scope.
- [ ] Bound queues and expose dropped-span counts instead of blocking application work forever.
- [ ] Make `close()` stop acceptance and wake the worker; let `run()` drain accepted batches before returning.
- [ ] Put the surrounding scope's explicit deadline around waiting for `run()`.
- [ ] Never recursively instrument the exporter HTTP client.

## `tracing/instrument`

- [ ] object `TraceLogging` does `Logging`
  - [ ] `TraceLogging(origin is Logging, context is TraceContext)`
  - [ ] `log(level is LogLevel, message is string) is void`
  - [ ] `log(level is LogLevel, message is string, fields is LogFields) is void`
  - [ ] `log(level is LogLevel, message is string, fields is LogFields, cause is Error) is void`
- [ ] object `TracedExchanging` does `Exchanging`
  - [ ] `TracedExchanging(origin is Exchanging, tracing is Tracing)`
  - [ ] `async response(request is Request) is ClientResponse`
- [ ] object `TracedHandling` does `Handling`
  - [ ] `TracedHandling(origin is Handling, tracing is Tracing)`
  - [ ] `async handling(request is Request, context is RequestContext) is HandlingResult`
- [ ] object `TracedDbConnecting` does `DbConnecting`
  - [ ] `TracedDbConnecting(origin is DbConnecting, tracing is Tracing)`
  - [ ] `async connection() is DbConnection`
- [ ] Add only trace and span IDs to logs; never copy all baggage or span attributes.
- [ ] Inject outbound context and extract inbound context only at the HTTP decorators.
- [ ] Record HTTP method, route template and status without raw query strings or credentials.
- [ ] Record database system and normalized operation without statement values.
- [ ] End spans on success, typed error, panic and cancellation.
- [ ] Keep instrumentation decorators out of exporter transports to prevent recursion.

## `metrics/model`

- [ ] enum `MetricKind`
  - [ ] `counter`
  - [ ] `up_down_counter`
  - [ ] `gauge`
  - [ ] `histogram`
- [ ] object `MetricDescriptor`
  - [ ] `MetricDescriptor(name is string, kind is MetricKind, unit is string, description is string)`
  - [ ] `name() is string`
  - [ ] `kind() is MetricKind`
  - [ ] `unit() is string`
  - [ ] `description() is string`
- [ ] role `MetricCounting`
  - [ ] `add(value is float, attributes is Attributes) is void`
- [ ] role `MetricGauging`
  - [ ] `set(value is float, attributes is Attributes) is void`
- [ ] role `MetricRecording`
  - [ ] `record(value is float, attributes is Attributes) is void`
- [ ] object `HistogramBoundaries`
  - [ ] `HistogramBoundaries(values is @float)`
  - [ ] `values() is @float`
- [ ] object `HistogramBucket`
  - [ ] `HistogramBucket(upper is float, count is int)`
  - [ ] `upper_bound() is float`
  - [ ] `count() is int`
- [ ] object `HistogramValue`
  - [ ] `HistogramValue(count is int, sum is float, buckets is @HistogramBucket)`
  - [ ] `count() is int`
  - [ ] `sum() is float`
  - [ ] `buckets() is @HistogramBucket`
- [ ] role `MetricValue`
  - [ ] `kind() is MetricKind`
- [ ] object `ScalarMetricValue` does `MetricValue`
  - [ ] `ScalarMetricValue(kind is MetricKind, value is float)`
  - [ ] `kind() is MetricKind`
  - [ ] `float() is float`
- [ ] object `HistogramMetricValue` does `MetricValue`
  - [ ] `HistogramMetricValue(value is HistogramValue)`
  - [ ] `kind() is MetricKind`
  - [ ] `histogram() is HistogramValue`
- [ ] object `MetricSample`
  - [ ] `MetricSample(descriptor is MetricDescriptor, attributes is Attributes, occurred is Instant, value is MetricValue)`
  - [ ] `descriptor() is MetricDescriptor`
  - [ ] `attributes() is Attributes`
  - [ ] `time() is Instant`
  - [ ] `value() is MetricValue`
- [ ] object `MetricSnapshot`
  - [ ] `MetricSnapshot(collected is Instant, samples is @MetricSample)`
  - [ ] `time() is Instant`
  - [ ] `samples() is @MetricSample`
- [ ] Require counters to receive finite non-negative deltas.
- [ ] Let up/down counters accept finite signed deltas and gauges accept finite values.
- [ ] Require strictly increasing finite histogram boundaries.
- [ ] Keep histogram counts cumulative and include the positive-infinity bucket.
- [ ] Do not implement client-side quantile summaries because they cannot be merged correctly.

## `metrics/state`

- [ ] role `MetricCollecting`
  - [ ] `snapshot() is MetricSnapshot`
- [ ] object `Metrics` does `MetricCollecting`
  - [ ] `Metrics(clock is Timing, limits is MetricLimits)`
  - [ ] `snapshot() is MetricSnapshot`
- [ ] object `Counter` does `MetricCounting`
  - [ ] `Counter(metrics is Metrics, descriptor is MetricDescriptor)`
  - [ ] `add(value is float, attributes is Attributes) is void`
- [ ] object `UpDownCounter` does `MetricCounting`
  - [ ] `UpDownCounter(metrics is Metrics, descriptor is MetricDescriptor)`
  - [ ] `add(value is float, attributes is Attributes) is void`
- [ ] object `Gauge` does `MetricGauging`
  - [ ] `Gauge(metrics is Metrics, descriptor is MetricDescriptor)`
  - [ ] `set(value is float, attributes is Attributes) is void`
- [ ] object `Histogram` does `MetricRecording`
  - [ ] `Histogram(metrics is Metrics, descriptor is MetricDescriptor, boundaries is HistogramBoundaries)`
  - [ ] `record(value is float, attributes is Attributes) is void`
- [ ] object `MetricLimits`
  - [ ] `MetricLimits(instruments is int, series is int, attributes is int)`
  - [ ] `instruments() is int`
  - [ ] `series() is int`
  - [ ] `attributes() is int`
- [ ] Reject duplicate instrument names with incompatible kind, unit or boundaries.
- [ ] Own all instrument state inside one explicitly passed `Metrics` object.
- [ ] Register an instrument lazily on its first manipulator call, never during construction.
- [ ] Bound instruments and distinct attribute series and count rejected updates.
- [ ] Never coalesce excess labels into a misleading synthetic series.
- [ ] Make snapshot collection deterministic and safe between cooperative task turns.
- [ ] Provide no factory methods, process-global metrics, default labels or callback-based observable gauges.

## `metrics/export`

- [ ] role `MetricExporting`
  - [ ] `async export(origin is MetricSnapshot) is MetricExporting`
- [ ] object `PrometheusMetrics`
  - [ ] `PrometheusMetrics(origin is MetricSnapshot)`
  - [ ] `string() is string`
- [ ] object `OtlpJsonMetrics`
  - [ ] `OtlpJsonMetrics(origin is MetricSnapshot, resource is Attributes)`
  - [ ] `#projected value() is JsonValue`
- [ ] object `OtlpHttpMetrics` does `MetricExporting`
  - [ ] `OtlpHttpMetrics(endpoint is Uri, headers is Headers, client is Exchanging, resource is Attributes, limits is TelemetryLimits)`
  - [ ] `async export(origin is MetricSnapshot) is OtlpHttpMetrics`
- [ ] object `PeriodicMetrics` does `Closing`
  - [ ] `PeriodicMetrics(source is MetricCollecting, destination is MetricExporting, interval is Duration, clock is Timing)`
  - [ ] `async run() is PeriodicMetrics`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] Render Prometheus names and labels deterministically and escape them exactly once.
- [ ] Reject descriptors that cannot be represented without a naming collision.
- [ ] Export cumulative counters and histograms and current gauges.
- [ ] Bound export payloads and split only between complete metric series.
- [ ] Make `close()` request a cooperative stop and let `run()` export the last complete snapshot before returning.
- [ ] Never recursively measure the exporter HTTP client.

## `metrics/instrument`

- [ ] object `MeasuredExchanging` does `Exchanging`
  - [ ] `MeasuredExchanging(origin is Exchanging, metrics is Metrics)`
  - [ ] `async response(request is Request) is ClientResponse`
- [ ] object `MeasuredHandling` does `Handling`
  - [ ] `MeasuredHandling(origin is Handling, metrics is Metrics)`
  - [ ] `async handling(request is Request, context is RequestContext) is HandlingResult`
- [ ] object `MeasuredDbConnecting` does `DbConnecting`
  - [ ] `MeasuredDbConnecting(origin is DbConnecting, metrics is Metrics)`
  - [ ] `async connection() is DbConnection`
- [ ] Record request counts, active requests and elapsed histograms with bounded attributes.
- [ ] Use route templates rather than raw paths for server metric labels.
- [ ] Keep status classes bounded and do not label by error message or database statement.
- [ ] Expose runtime and process metrics only through separately constructed collectors.
- [ ] Do not make metric instrumentation a side effect of tracing.

## `profiling/cpu`

- [ ] object `ProfileFrame`
  - [ ] `ProfileFrame(name is string, range is SourceRange)`
  - [ ] `name() is string`
  - [ ] `range() is SourceRange`
- [ ] object `ProfileSample`
  - [ ] `ProfileSample(occurred is MonotonicInstant, frames is @ProfileFrame)`
  - [ ] `time() is MonotonicInstant`
  - [ ] `frames() is @ProfileFrame`
- [ ] object `CpuProfile`
  - [ ] `CpuProfile(started is Instant, duration is Duration, samples is @ProfileSample, dropped is int)`
  - [ ] `start() is Instant`
  - [ ] `duration() is Duration`
  - [ ] `samples() is @ProfileSample`
  - [ ] `dropped_samples() is int`
- [ ] object `CpuProfiling<R>`
  - [ ] `CpuProfiling(action is () is R, interval is Duration, limit is int, clock is Timing)`
  - [ ] `result() is Profiled<R>`
- [ ] object `AsyncCpuProfiling<R>`
  - [ ] `AsyncCpuProfiling(action is async () is R, interval is Duration, limit is int, clock is Timing)`
  - [ ] `async result() is Profiled<R>`
- [ ] object `Profiled<R>`
  - [ ] `Profiled(value is R, profile is CpuProfile)`
  - [ ] `value() is R`
  - [ ] `profile() is CpuProfile`
- [ ] object `FoldedProfile`
  - [ ] `FoldedProfile(origin is CpuProfile)`
  - [ ] `string() is string`
- [ ] Sample only at runtime safe points and document that native blocking time is attributed to its native frame.
- [ ] Bound samples and stack depth and count dropped samples.
- [ ] Preserve the action result or failure exactly and always stop the sampler during cleanup.
- [ ] Keep profiling explicit; do not add a global signal-driven profiler.
- [ ] Render folded stacks deterministically for flame-graph tools.

## `profiling/allocation`

- [ ] object `AllocationSite`
  - [ ] `AllocationSite(type is string, range is SourceRange)`
  - [ ] `type() is string`
  - [ ] `range() is SourceRange`
- [ ] object `AllocationStat`
  - [ ] `AllocationStat(site is AllocationSite, count is int, bytes is int)`
  - [ ] `site() is AllocationSite`
  - [ ] `count() is int`
  - [ ] `bytes() is int`
- [ ] object `AllocationProfile`
  - [ ] `AllocationProfile(stats is @AllocationStat, dropped is int)`
  - [ ] `stats() is @AllocationStat`
  - [ ] `dropped_sites() is int`
- [ ] object `AllocationProfiling<R>`
  - [ ] `AllocationProfiling(action is () is R, limit is int)`
  - [ ] `result() is AllocationProfiled<R>`
- [ ] object `AllocationProfiled<R>`
  - [ ] `AllocationProfiled(value is R, profile is AllocationProfile)`
  - [ ] `value() is R`
  - [ ] `profile() is AllocationProfile`
- [ ] Count language-visible allocation size rather than host allocator overhead.
- [ ] Aggregate by language type and source site with a strict site limit.
- [ ] Disable allocation hooks completely outside an explicit profiling scope.
- [ ] Preserve the action result or failure and drain cleanup before finalizing the profile.

## `coverage`

- [ ] enum `CoverageKind`
  - [ ] `statement`
  - [ ] `branch`
- [ ] object `CoveragePoint`
  - [ ] `CoveragePoint(kind is CoverageKind, range is SourceRange, hits is int)`
  - [ ] `kind() is CoverageKind`
  - [ ] `range() is SourceRange`
  - [ ] `hits() is int`
- [ ] object `ModuleCoverage`
  - [ ] `ModuleCoverage(module is string, points is @CoveragePoint)`
  - [ ] `module() is string`
  - [ ] `points() is @CoveragePoint`
  - [ ] `covered_points() is int`
  - [ ] `total() is int`
- [ ] object `CoverageReport`
  - [ ] `CoverageReport(modules is @ModuleCoverage)`
  - [ ] `modules() is @ModuleCoverage`
  - [ ] `covered_points() is int`
  - [ ] `total() is int`
  - [ ] `ratio() is float`
- [ ] object `CoverageCollecting<R>`
  - [ ] `CoverageCollecting(action is () is R, modules is @string)`
  - [ ] `result() is Covered<R>`
- [ ] object `AsyncCoverageCollecting<R>`
  - [ ] `AsyncCoverageCollecting(action is async () is R, modules is @string)`
  - [ ] `async result() is Covered<R>`
- [ ] object `Covered<R>`
  - [ ] `Covered(value is R, report is CoverageReport)`
  - [ ] `value() is R`
  - [ ] `report() is CoverageReport`
- [ ] object `LcovCoverage`
  - [ ] `LcovCoverage(origin is CoverageReport)`
  - [ ] `string() is string`
- [ ] Count executable statements and every reachable decision branch separately.
- [ ] Keep source identities stable and sort modules and positions deterministically.
- [ ] Include uncovered points known from the compiled module, not only observed points.
- [ ] Restrict collection to an explicit module allow-list.
- [ ] Preserve the action result or failure and finalize coverage after cleanup.
- [ ] Integrate Stage 27 tests by wrapping the runner explicitly, never through hidden test mode.

## Stage 28 verification

- [ ] Preserve every currently working failure, panic, cleanup, clock and process-identity behavior.
- [ ] Add source range vectors for ASCII, Unicode, multiline and generated modules.
- [ ] Add sync, async, native, constructor, cause and suppressed-cleanup stack cases.
- [ ] Prove arbitrary caught `Error` values do not acquire invented call stacks.
- [ ] Parse human and JSON diagnostics independently and enforce every output limit.
- [ ] Add W3C propagation conformance vectors and malformed-header rejection cases.
- [ ] Verify deterministic probability sampling and parent sampling.
- [ ] Verify exactly-once span completion under success, error, panic and cancellation.
- [ ] Add bounded queue, retry, timeout, close and recursive-export protection cases.
- [ ] Verify metric kind conflicts, cardinality limits and histogram boundary behavior.
- [ ] Parse Prometheus and OTLP JSON output with independent implementations.
- [ ] Verify HTTP and database decorators never expose queries, credentials or statement values.
- [ ] Add profiler sample-limit, cleanup, async suspension and native-frame cases.
- [ ] Add statement and branch coverage cases, including never-executed source points.
- [ ] Prove all observability state is isolated between interpreter and process instances.
- [ ] Add one Perl-level regression file per runtime hook and public module group.
- [ ] Add runnable diagnostics, tracing, metrics, profiling and coverage examples.
- [ ] Run the public-action-count audit with inherited roles included.
- [ ] Run `git diff --check`.

## Stage 28 exclusions

- [ ] Do not derive this API from the existing stdlib or its module names.
- [ ] Do not add source, stack, trace, metric or profiling methods to builtin runtime values.
- [ ] Do not add stdlib conversions such as `int.duration()` or `string.trace_id()`.
- [ ] Do not create one `Observability` object that owns logging, tracing and metrics.
- [ ] Do not install global logging, tracing, metrics, exporters, baggage or current spans.
- [ ] Do not hide trace context in thread-local, task-local or dynamically scoped state.
- [ ] Do not turn diagnostics into catchable error values or make every error a diagnostic tree.
- [ ] Do not expose raw host exceptions, host stack frames, absolute paths or interpreter internals.
- [ ] Do not put unrestricted JSON objects, secrets or high-cardinality values into telemetry attributes.
- [ ] Do not infer span errors from log levels or metric values.
- [ ] Do not make tracing automatically enable logs or metrics.
- [ ] Do not block application work indefinitely when an exporter is slow or unavailable.
- [ ] Do not retry permanent export failures or recursively instrument exporter traffic.
- [ ] Do not promise exact CPU time from a cooperative safe-point sampler.
- [ ] Do not keep profiler or coverage hooks active outside their explicit scope.
- [ ] Do not include image, audio, package metadata, FFI, language tooling, project packaging or platform-specific APIs from starred Stages 29–34.

---

# Stage 35 — explicit exclusions

These are deliberate boundaries of stdlib v3. They are not forgotten modules
and do not block completion of Stages 00–28.

## Public API shape

- [x] Do not add an object with more than five public action names, including inherited roles.
- [x] Do not evade the limit with synonyms, numbered actions or a generic `perform(name, arguments)` action.
- [x] Do not create utility, manager, service, context or facade objects that merely collect unrelated operations.
- [x] Do not create a stateless algorithm role whose operands arrive entirely through one action; put behavior on a represented value or explicit decorator.
- [x] Do not create zero-state procedure objects; a zero-argument object must be a meaningful domain atom, policy or explicit external capability.
- [x] Do not split modules mechanically by primitive type or grammatical role.
- [x] Do not create an interface for every object; introduce a role only when independent implementations compose through it.
- [x] Do not introduce getter prefixes, Java-style builders or mutable property bags.
- [x] Name pure value-returning builders with nouns and effectful void manipulators with verbs.
- [x] Do not let one action both change observable resource state and return a value.
- [x] Do not add aliases solely to imitate names from Python, JavaScript, Java or a previous Okkama stdlib.
- [x] Do not use inheritance as a namespace or as a substitute for composition.
- [x] Do not perform filesystem, network, process or other unbounded work in constructors.
- [x] Do not hide effects behind innocent value queries.
- [x] Do not return stdlib objects from methods added to builtin primitive receivers.
- [x] Do not add universal conversion, serialization, validation, cloning or reflection roles.
- [x] Do not use `any` where a closed value model or a generic type expresses the contract.
- [x] Do not use null as an ordinary absence protocol; model a meaningful empty, missing or variant value.
- [x] Do not add mutable reset actions when a fresh small object expresses the lifecycle more clearly.

## Dynamic runtime facilities

- [x] Exclude runtime monkey-patching and mutation of object or role definitions.
- [x] Exclude unrestricted runtime reflection over fields, private state and action bodies.
- [x] Exclude stack-frame mutation, caller-local access and executable frame objects.
- [x] Exclude source-string `eval`, dynamic compilation and loading code from arbitrary bytes.
- [x] Exclude implicit module loading by a computed string.
- [x] Exclude public garbage-collector tuning, forced collection and heap-object enumeration.
- [x] Exclude weak references and observable finalization order from the portable API.
- [x] Exclude global exit hooks; use lexical `defer`, `Closing` and structured async scopes.
- [x] Exclude mutable recursion limits and other interpreter-global tuning knobs.
- [x] Exclude host-language exception objects, host stack frames and interpreter implementation objects.
- [x] Use Stage 28 diagnostics and profiling for safe observations instead of a general `runtime` or `sys` facade.

## Unsafe persistence and copying

- [x] Exclude `pickle`-style executable object deserialization.
- [x] Exclude `marshal`-style persistence of runtime-internal representations.
- [x] Exclude universal object serialization based on reflection.
- [x] Exclude `shelve` and `dbm` facades; compose Stage 13 values with Stage 23 SQLite when durable keyed storage is needed.
- [x] Exclude generic shallow-copy and deep-copy operations.
- [x] Reuse immutable values directly and give resources explicit domain-specific duplication operations where meaningful.
- [x] Exclude automatic graph cloning that guesses identity, ownership or resource semantics.
- [x] Exclude general pretty-printing through reflection; use explicit projected values, JSON or trusted diagnostics.

## Hidden ambient state

- [x] Exclude process-global locale, timezone, current directory and mutable environment APIs.
- [x] Exclude global random generators, loggers, tracers, meters and test registries.
- [x] Exclude thread-local, task-local and dynamically scoped application contexts.
- [x] Exclude ambient credentials, proxy discovery and trust-policy weakening.
- [x] Exclude default HTTP cookies, redirects, caches, retries and authentication.
- [x] Exclude implicit database connections and process-global connection pools.
- [x] Exclude background workers that are not owned by a Stage 19 structured-concurrency scope.
- [x] Require applications to pass capabilities and policy objects explicitly.

## Security boundaries

- [x] Exclude shell evaluation from process execution and CLI parsing.
- [x] Exclude SQL construction from interpolated strings and templates.
- [x] Exclude executable regular expressions, recursive regex constructs and replacement-code evaluation.
- [x] Exclude unsafe XML entities, network resource resolution and unbounded expansion.
- [x] Exclude archive extraction that can escape its destination or follow archive-provided links implicitly.
- [x] Exclude production TLS verification-disable switches.
- [x] Exclude unauthenticated encryption and obsolete password hashing from the modern crypto API.
- [x] Exclude cryptographic algorithm selection by arbitrary unvalidated strings.
- [x] Exclude silent decoding repair, character replacement and malformed-input acceptance from strict codecs.
- [x] Exclude silent truncation, partial success and best-effort behavior unless an explicitly named decorator requests it.
- [x] Exclude secrets, authorization data and unbounded identifiers from default logs, diagnostics and telemetry.

## Python capabilities covered by language or composition

- [x] Use language objects and roles instead of `dataclasses`, `namedtuple`, `abc` and runtime `typing` helpers.
- [x] Use language enums instead of a stdlib enum-construction framework.
- [x] Use closures instead of `partial` and operator-wrapper modules.
- [x] Use `defer` and `Closing` instead of `contextlib` adapters.
- [x] Use immutable collection decorators instead of iterator utility functions with ambient state.
- [x] Use Stage 02 sequences, Stage 04 bounds and heaps instead of separate `bisect`, stack and queue facades.
- [x] Use Stage 07 streams instead of `fileinput` and a second text-I/O hierarchy.
- [x] Use Stage 11 path objects instead of stringly `os.path` operations.
- [x] Use Stage 12 commands and processes instead of an implicit-shell `subprocess` facade.
- [x] Use Stage 19 futures, channels and scopes instead of shared-memory threads and callback event loops.
- [x] Use Stage 20 async networking instead of public `select` and selector objects.
- [x] Use Stage 22 HTTP server objects instead of WSGI/CGI compatibility layers.
- [x] Use Stage 23 SQLite instead of a separate convenience persistence database.
- [x] Use Stage 25 logging and Stage 28 diagnostics instead of a global warnings registry.
- [x] Use Stage 27 tests and Stage 28 profiling instead of separate `timeit`, tracing and test-runner globals.
- [x] Permit observationally pure memoization inside immutable values; exclude a generic effectful memoization cache from v3.

## Legacy and niche protocols

- [x] Exclude FTP because it provides no secure modern transport contract; use HTTPS or an application-specific adapter.
- [x] Exclude Telnet because its plaintext interactive protocol conflicts with the security baseline.
- [x] Exclude NNTP, Gopher and platform mailcap databases from the portable core.
- [x] Exclude browser launching and desktop URL handlers from the portable core.
- [x] Keep XML-RPC explicitly low priority because Stage 22 already defines its bounded safe shape.
- [x] Keep POP3 because Stage 21 already defines its bounded role despite its legacy status.
- [x] Require a new reviewed stage for any omitted wire protocol rather than adding it to a generic network facade.

## Starred families not taken in v3

- [x] Exclude Stage 29 image and audio codecs, metadata, playback and capture.
- [x] Exclude Stage 30 module discovery, import metadata, semantic versions and distribution metadata.
- [x] Exclude Stage 31 FFI, dynamic libraries, foreign memory and foreign callbacks.
- [x] Exclude Stage 32 lexer/parser/AST APIs, formatter, linter, debugger, REPL and documentation tooling.
- [x] Exclude Stage 33 package installation, virtual environments, build systems and embedded-resource lookup.
- [x] Exclude Stage 34 direct POSIX, Windows, registry, service-manager, GUI and device APIs.
- [x] Allow portable implementations to use platform facilities internally without exposing their native handles or constants.
- [x] Require each starred family to receive a separate greenfield workbook before it can enter the stdlib.
- [x] Do not reserve public names or placeholder modules for starred families.

---

# Stage 36 — final coverage audit

## Coverage gaps closed by the audit

### `collections/topology`

- [ ] object `Dependency<T>`
  - [ ] `Dependency(item is T, requirements is Sequencing<T>)`
  - [ ] `item() is T`
  - [ ] `requirements() is Sequencing<T>`
- [ ] object `TopologicalOrder<T>` does `Sequencing<T>`
  - [ ] `TopologicalOrder(origin is Iterating<Dependency<T>>)`
  - [ ] `cursor() is Cursor<T>`
  - [ ] `size() is int`
  - [ ] `item(index is int) is T`
  - [ ] `first() is T`
  - [ ] `last() is T`
- [ ] object `DependencyCycle<T>` does `Error`
  - [ ] `DependencyCycle(items is Sequencing<T>)`
  - [ ] `items() is Sequencing<T>`
  - [ ] `text() is string`
- [ ] Treat a requirement absent from the input as a node with no requirements.
- [ ] Reject duplicate declarations for one item.
- [ ] Build and memoize the ordering lazily on the first sequence action.
- [ ] Return independent ready nodes in first-declaration order.
- [ ] Report one deterministic closed cycle when ordering is impossible.
- [ ] Implement indegrees and reverse edges through native bulk map/set operations.

### `os/signals`

- [ ] role `SignalReceiving`
  - [ ] `async signal() is Signal`
- [ ] object `ProcessSignals` does `SignalReceiving, Closing`
  - [ ] `ProcessSignals(values is Set<Signal>)`
  - [ ] `async signal() is Signal`
  - [ ] `closed() is bool`
  - [ ] `close() is void`
- [ ] Add a runtime self-pipe, signalfd, kqueue or Windows console-control adapter behind one portable event source.
- [ ] Install handlers lazily on the first `signal()` so construction only binds policy.
- [ ] Install native handlers only while the explicit `ProcessSignals` resource is open.
- [ ] Coalesce repeated signals only when the host cannot preserve their count.
- [ ] Wake a pending receiver on close with the ordinary closed-resource error.
- [ ] Restore previous host handlers during cleanup after normal return, error, cancellation and panic.
- [ ] Keep fatal machine exceptions and `kill` outside the receivable set.

### `cli/shell-word`

- [ ] object `PosixShellWords` does `Sequencing<string>`
  - [ ] `PosixShellWords(origin is string)`
  - [ ] `cursor() is Cursor<string>`
  - [ ] `size() is int`
  - [ ] `item(index is int) is string`
  - [ ] `first() is string`
  - [ ] `last() is string`
- [ ] object `PosixShellQuoted`
  - [ ] `PosixShellQuoted(origin is string)`
  - [ ] `#projected string() is string`
- [ ] object `InvalidShellWords` does `Error`
  - [ ] `InvalidShellWords(offset is int, reason is string)`
  - [ ] `offset() is int`
  - [ ] `text() is string`
- [ ] Parse POSIX quotes, backslash escapes and comments without expansion or evaluation.
- [ ] Parse and memoize lazily on the first sequence action.
- [ ] Reject unfinished quotes and escapes with a byte offset.
- [ ] Quote one literal word so parsing the result produces exactly that word.
- [ ] Do not expand variables, commands, arithmetic, globs, tildes or response files.
- [ ] Do not pass parsed words to a shell implicitly; construct Stage 12 `Command` explicitly.
- [ ] Keep PowerShell, CMD, fish and shell-script grammars outside this portable lexical module.

## Python-level capability matrix

- [x] Core values and contracts — Stages 00–01.
- [x] Iteration, sequences, collection views, sets, deques, heaps, counting, ordering, graph ordering and combinatorics — Stages 02–04 and this audit.
- [x] Strings, formatting, parsing, diffs, Unicode and codecs — Stages 05–06.
- [x] Binary and text I/O, buffering and stream composition — Stage 07.
- [x] Integer, rational, decimal, complex, aggregate and statistical mathematics — Stage 08.
- [x] Deterministic and secure randomness, distributions and UUIDs — Stage 09.
- [x] Durations, clocks, dates, time zones, formats and calendars — Stage 10.
- [x] Paths, files, directories, metadata, globbing, links, temporary resources, mapping and watching — Stage 11.
- [x] Environment snapshots, platform identity, accounts, commands, processes, signals, pipelines and disks — Stage 12 and this audit.
- [x] JSON, CSV, TOML, INI, XML, HTML entities and plist — Stage 13.
- [x] Regex, lexical matching and streaming HTML events — Stage 14.
- [x] Checksums, hashes, HMAC and constant-time comparison — Stage 15.
- [x] Gzip, zlib, deflate, bzip2, xz, LZMA and Zstandard — Stage 16.
- [x] TAR, ZIP, archive streaming, metadata and safe extraction — Stage 17.
- [x] Secrets, KDFs, passwords, AEAD, modern keys, signatures, key formats and X.509 — Stage 18.
- [x] Futures, cancellation, channels, scopes, synchronization and process-parallel mapping — Stage 19.
- [x] IP, DNS, interfaces, TCP, UDP, multicast, Unix sockets and TLS — Stage 20.
- [x] URL, headers, MIME and SMTP/IMAP/POP3 mail — Stage 21.
- [x] HTTP/1.1, HTTP/2, clients, servers, routing, auth, cookies, cache, WebSocket, SSE, forms and XML-RPC — Stage 22.
- [x] Typed database values, statements, rows, transactions, pools and full SQLite lifecycle APIs — Stage 23.
- [x] Layered configuration, CLI parsing/help/completion, POSIX shell words and terminal sessions — Stage 24 and this audit.
- [x] Structured synchronous and queued logging, rotation, redaction and syslog — Stage 25.
- [x] Plain and HTML templates, locale data, collation, numbers, currency, dates and gettext — Stage 26.
- [x] Assertions, fakes, deterministic time, property tests, snapshots, isolation and reports — Stage 27.
- [x] Diagnostics, tracing, metrics, CPU/allocation profiling and source coverage — Stage 28.
- [x] Every materially omitted Python family has an explicit Stage 35 reason and a preferred composition path.

## Naming and shape audit

- [x] Use `string`, `@byte` and existing builtin names consistently.
- [x] Use `.string()` for an object's explicit textual projection and never `.text()` as a generic conversion.
- [x] Reserve `.text()` for the required public message of an `Error`.
- [x] Keep primitive conversions limited to primitive runtime result types.
- [x] Name roles and objects as entities or capabilities, not as procedures, utilities or stateless algorithms.
- [x] Use a natural `-ing` name only when it denotes a genuine capability such as `Reading`, `Writing` or `Timing`; never force a grammatical suffix.
- [x] Keep noun roles for value categories and domain capabilities such as `Error`, `Cursor`, `ByteSource`, `TimeZone` and closed value unions.
- [x] Keep comparison on the compared value or an explicit keyed decorator; do not restore `Ordering<T>` or another binary-function role.
- [x] Give every value-returning action a noun name; allow established adjectives only for boolean predicates.
- [x] Give every effectful action a verb name and no result value.
- [x] Keep exactly one code-free primary constructor per object and make all secondary constructors delegate to it.
- [x] Reject static methods, utility namespaces, zero-state procedure objects and stateless algorithm roles.
- [x] Keep constructors out of the public-action count.
- [x] Keep every declared role at five public action names or fewer.
- [x] Keep every declared object at five local public action names or fewer.
- [x] Keep every currently declared multi-role composition at five distinct inherited and local action names or fewer.
- [x] Keep top-level parsing in the domain of the source string, including `strings/parse` and `cli/shell-word`.
- [x] Keep stream operations named `write`, `writeln`, `bytes`, `line`, `text` and `flush`; do not replace them with a generic print facade.
- [x] Keep formatting separate from writing, including printf-style `Formatted` values from Stage 05.
- [x] Keep large resources decomposed into capability views instead of adding convenience methods to the owner.

## Unresolved signature audit

The following earlier entries are design blockers, not implementation tasks.
Resolve each into full object signatures before approving its module.

- [ ] Replace the Stage 05 `MatchingBlocks` and `MatchingRatio` placeholders with exact result values and one named diff algorithm.
- [ ] Replace the Stage 06 Base32/Base64/Base85 policy placeholders with explicit alphabet and decoding-policy values.
- [ ] Replace the Stage 06 `BinaryLayout`, `Packed` and `Unpacked` placeholders with a complete bounded binary-layout object model.
- [ ] Replace the Stage 07 `BufferedReading` placeholder with its exact constructor and inherited actions.
- [ ] Replace the Stage 08 big-rational arithmetic placeholders with projected result objects.
- [ ] Replace the Stage 08 complex transcendental placeholders with separately named projected result objects.
- [ ] Replace the Stage 08 linear-regression placeholder with an immutable slope/intercept result.
- [x] Resolve Stage 08 finite constants as zero-state projected objects until immutable module values exist; exclude non-finite constants under the current float contract.
- [x] Replace every Stage 09 distribution placeholder with exact constructor parameters and validation rules.
- [x] Remove UUID 1 and UUID 6 placeholders from v3 unless a concrete privacy-preserving node-identity capability is approved.
- [ ] Replace the Stage 10 date/time formatting placeholders with explicit pattern value signatures.
- [ ] Replace the Stage 11 permission, hard-link, preserved-temporary-file and mapped-position placeholders with exact signatures.
- [ ] Replace every remaining “specify”, “choose later” and conditional-public-API line with a signature, an explicit exclusion or an internal implementation note.
- [ ] Reject roadmap approval for a module while any public object beneath it lacks a complete constructor and action checklist.

## Greenfield runtime audit

- [ ] Collect every unchecked `runtime/* substrate` item into one dependency graph without consulting the existing stdlib.
- [ ] Assign each runtime dependency to the first public module that needs it.
- [ ] Implement semantic primitives before their first stdlib consumer.
- [ ] Keep protocol parsing, policy and domain objects in stdlib even when native transport primitives live in runtime.
- [ ] Give every native boundary typed error normalization, cancellation and cleanup semantics.
- [ ] Verify every native bulk path preserves the public scalar fallback result exactly.
- [ ] Verify process-isolated work serializes only explicitly supported immutable values.
- [ ] Verify no stdlib object relies on Perl package identity, host exception text or existing v2 module behavior.
- [ ] Remove a runtime hook from the plan if its only consumer belongs to excluded Stages 29–34.

## Implementation completion gate

- [ ] Resolve every item in the unresolved-signature audit before freezing v3 signatures.
- [ ] Approve every public signature in Stages 00–28 and the three audit gap modules.
- [ ] Complete the definition-of-done checklist for every included module.
- [ ] Complete all declared runtime substrate tasks before marking their dependent public modules done.
- [ ] Add an automated public-action-count check that expands inherited roles.
- [ ] Add an automated check for duplicate signatures, unresolved type names and dangling stage references.
- [ ] Add an automated check rejecting public stdlib-returning actions on primitive receivers.
- [ ] Add an automated check rejecting verb-named builders and value-returning manipulators.
- [ ] Add an automated check rejecting noun-named void actions outside an explicit predicate exception list.
- [ ] Add an automated check rejecting duplicate public type names across modules.
- [ ] Add an automated check rejecting placeholder words in approved module sections.
- [ ] Build every runnable example in a clean interpreter process.
- [ ] Run focused tests, all Perl interpreter regressions and the complete stdlib suite.
- [ ] Run coverage and require every public action, typed error and cleanup path to be exercised.
- [ ] Generate a module index and verify every included public type appears exactly once.
- [ ] Generate the final API reference from approved signatures rather than maintaining a second handwritten specification.
- [ ] Confirm excluded Stages 29–34 contribute no runtime hook, dependency, placeholder module or reserved public name.
- [ ] Run `git diff --check`.

## Final acceptance

- [ ] No included module contains a vague public API placeholder.
- [ ] No public object or role exceeds five action names after inherited roles are expanded.
- [ ] No builder has effects or a verb name, and no manipulator returns a value or has a noun name.
- [ ] No static algorithm role, utility namespace or zero-state procedure object remains.
- [ ] Every object has one code-free primary constructor and delegation-only secondary constructors.
- [ ] No builtin primitive method returns a stdlib type.
- [ ] No constructor performs I/O or unbounded work.
- [ ] No resource depends on hidden global or task-local application state.
- [ ] Every effect has explicit ownership, failure, cancellation and cleanup behavior.
- [ ] Every parser, decoder and protocol boundary has explicit limits.
- [ ] Every security-sensitive default fails closed.
- [ ] Every Python-level capability is implemented, covered by language composition or explicitly excluded with an alternative.
- [ ] The implementation workbook is sufficient to complete tasks one checkbox at a time without consulting v2 stdlib APIs.
- [ ] Freeze stdlib v3 only after every item above is true.
