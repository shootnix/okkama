# Current std API — v4

Inventory of production declarations, updated with completed v4 work. The [audit](stdlib-api-audit-v4.md) records design decisions, including justified compromises. The [completion guide](stdlib-completion-todo-v4.md) lists missing work. This inventory describes implemented declarations, not a proof of all behavior.

Shorthand constructors marked **named only** accept named arguments. Public infrastructure declarations are importable because the language has no module-private declarations; their presence is not an endorsement of direct application use. Builtin/native objects may obtain actions from the interpreter rather than an Okkama body.

## `async/atomic`

[Implementation and concurrency contract](../../std/async/atomic.ok) · [Design decision](stdlib-api-audit-v4.md#stdasyncatomicok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Atomic<T>` does `Closing` | `Atomic(value is T)` | `value() is T`<br>`replace(value is T) is void`<br>`compare_and_replace(expected is T, revised is T) is bool`<br>`closed() is bool`<br>`close() is void` |
| `AtomicClosed` does `Error` | `AtomicClosed()` | `text() is string` |

## `async/barrier`

[Implementation](../../std/async/barrier.ok) · [Design decision](stdlib-api-audit-v4.md#stdasyncbarrierok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Barrier` | `Barrier(parties is int)` | `async wait() is int`<br>`state() is BarrierState`<br>`abort() is void`<br>`reset() is void` |
| `BarrierState` | — | `parties() is int`<br>`waiters() is int`<br>`generation() is int`<br>`broken() is bool` |
| `InvalidPartyCount` does `Error` | `InvalidPartyCount(parties is int)` | `parties() is int`<br>`text() is string` |
| `BrokenBarrier` does `Error` | `BrokenBarrier()` | `text() is string` |

## `async/cancellation`

[Implementation](../../std/async/cancellation.ok) · [Design decision](stdlib-api-audit-v4.md#stdasynccancellationok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Cancelling` | — | `requested() is bool`<br>`check() is void`<br>`async wait() is Cancelling` |
| `Cancellation` | `Cancellation()` | `cancelling() is Cancelling`<br>`cancel() is void` |
| `CancellationToken` does `Cancelling` | `CancellationToken()` | `requested() is bool`<br>`check() is void`<br>`async wait() is Cancelling` |

## `async/channel`

[Implementation](../../std/async/channel.ok) · [Design decision](stdlib-api-audit-v4.md#stdasyncchannelok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Channel<T>` | `Channel(capacity is int)` | `sending() is Sending<T>`<br>`receiving() is Receiving<T>` |
| `Sending<T>` | `Sending()` | `async send(value is T) is Sending<T>`<br>`close() is void` |
| `Receiving<T>` | `Receiving()` | `async item() is T`<br>`close() is void` |
| `ChannelClosed` does `Error` | `ChannelClosed()` | `text() is string` |
| `InvalidChannelCapacity` does `Error` | `InvalidChannelCapacity(capacity is int)` **named only** | `capacity() is int`<br>`text() is string` |

## `async/collect`

[Implementation](../../std/async/collect.ok) · [Design decision](stdlib-api-audit-v4.md#stdasynccollectok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `AllFutures<T>` | `AllFutures(origins is Sequencing<Future<T>>)` **named only** | `async results() is Sequencing<T>` |
| `Raced<T>` | `Raced(origins is Sequencing<Future<T>>)` **named only** | `async result() is T` |
| `FirstSuccessful<T>` | `FirstSuccessful(origins is Sequencing<Future<T>>)` **named only** | `async result() is T` |
| `FutureCompletions<T>` | `FutureCompletions(origins is Sequencing<Future<T>>)` **named only** | `remaining_count() is int`<br>`async completion() is Future<T>` |
| `NoFutures` does `Error` | `NoFutures()` | `text() is string` |

## `async/event`

[Implementation](../../std/async/event.ok) · [Design decision](stdlib-api-audit-v4.md#stdasynceventok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Event` | `Event()`<br>`Event(ready is bool)` | `async wait() is Event`<br>`set() is void`<br>`ready() is bool` |

## `async/lock`

[Implementation](../../std/async/lock.ok) · [Design decision](stdlib-api-audit-v4.md#stdasynclockok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Lock` | `Lock()` | `async guard() is LockGuard`<br>`locked() is bool` |
| `LockGuard` does `Closing` | — | `closed() is bool`<br>`close() is void` |

## `async/scope`

[Implementation](../../std/async/scope.ok) · [Design decision](stdlib-api-audit-v4.md#stdasyncscopeok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Supervising<T>` | — | `start(origin is Future<T>) is void`<br>`active() is int`<br>`cancel() is void` |
| `FutureGroup<T>` does `Supervising<T>` | `FutureGroup()` | `start(origin is Future<T>) is void`<br>`active() is int`<br>`cancel() is void`<br>`async wait() is FutureGroup<T>` |
| `FutureScope<T,R>` | `FutureScope(action is async (Supervising<T>) is R)` **named only** | `async result() is R` |
| `FinishedFutureGroup` does `Error` | `FinishedFutureGroup()` | `text() is string` |

## `async/semaphore`

[Implementation](../../std/async/semaphore.ok) · [Design decision](stdlib-api-audit-v4.md#stdasyncsemaphoreok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Semaphore` | `Semaphore(permits is int)` | `async permit() is Permit`<br>`permits() is int` |
| `Permit` does `Closing` | — | `closed() is bool`<br>`close() is void` |
| `InvalidPermitCount` does `Error` | `InvalidPermitCount(permits is int)` | `permits() is int`<br>`text() is string` |

## `async/timeout`

[Implementation](../../std/async/timeout.ok) · [Design decision](stdlib-api-audit-v4.md#stdasynctimeoutok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Within<T>` | `Within(origin is Future<T>, limit is Duration, clock is Timing)` | `async result() is T` |
| `Before<T>` | `Before(origin is Future<T>, deadline is MonotonicInstant, clock is Timing)` **named only** | `async result() is T` |
| `TimedOut` does `Error` | `TimedOut()` | `text() is string` |

## `calendar`

[Implementation](../../std/calendar.ok) · [Design decision](stdlib-api-audit-v4.md#stdcalendarok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `YearMonth` does `Comparing<YearMonth>` | `YearMonth(year is int, month is int)` | `year() is int`<br>`month() is int`<br>`order(other is YearMonth) is Order` |
| `MonthDates` does `Sequencing<Date>`, `Iterating<Date>`, `Sizing` | `MonthDates(origin is YearMonth)` **named only** | `cursor() is Cursor<Date>`<br>`size() is int`<br>`item(index is int) is Date`<br>`first() is Date`<br>`last() is Date` |
| `MonthDateCursor` does `Cursor<Date>` | `MonthDateCursor(origin is MonthDates, position is int)` | `available() is bool`<br>`item() is Date`<br>`successor() is Cursor<Date>` |
| `CalendarWeek` does `Sequencing<Date>`, `Iterating<Date>`, `Sizing` | `CalendarWeek(start is Date)` | `cursor() is Cursor<Date>`<br>`size() is int`<br>`item(index is int) is Date`<br>`first() is Date`<br>`last() is Date` |
| `CalendarWeekCursor` does `Cursor<Date>` | `CalendarWeekCursor(origin is CalendarWeek, position is int)` | `available() is bool`<br>`item() is Date`<br>`successor() is Cursor<Date>` |
| `CalendarMonth` does `Sequencing<Sequencing<Date>>`, `Iterating<Sequencing<Date>>`, `Sizing` | `CalendarMonth(origin is YearMonth, first_weekday is Weekday)` | `cursor() is Cursor<Sequencing<Date>>`<br>`size() is int`<br>`item(index is int) is Sequencing<Date>`<br>`first() is Sequencing<Date>`<br>`last() is Sequencing<Date>` |
| `CalendarMonthCursor` does `Cursor<Sequencing<Date>>` | `CalendarMonthCursor(origin is CalendarMonth, position is int)` | `available() is bool`<br>`item() is Sequencing<Date>`<br>`successor() is Cursor<Sequencing<Date>>` |
| `CalendarMonthLayout` | `CalendarMonthLayout(origin is YearMonth, first_weekday is Weekday)` **named only** | `first_ordinal() is int`<br>`size() is int` |
| `IsoWeek` does `Comparing<IsoWeek>` | `IsoWeek(year is int, week is int)` | `year() is int`<br>`week() is int`<br>`first() is Date`<br>`last() is Date`<br>`order(other is IsoWeek) is Order` |
| `LeapYear` | `LeapYear(year is int)` | `#projected value() is bool` |
| `WeekdayIndex` | `WeekdayIndex(origin is Weekday)` **named only** | `#projected value() is int` |
| `InvalidYearMonth` does `Error` | `InvalidYearMonth(message is string)` | `#projected text() is string` |
| `InvalidCalendarMonth` does `Error` | `InvalidCalendarMonth(message is string)` | `#projected text() is string` |
| `InvalidIsoWeek` does `Error` | `InvalidIsoWeek(message is string)` | `#projected text() is string` |

## `checksum/adler32`

[Implementation](../../std/checksum/adler32.ok) · [Contract](checksum-guide.md#checksumadler32-contract)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Adler32` does `Checksumming` | `Adler32()`<br>`Adler32(origin is @byte)`<br>`Adler32(origin is @byte, initial is int)` | `value() is int`<br>`state(origin is @byte) is ^Adler32` |

Default seed 1, explicit unsigned 32-bit seed; invalid seeds raise
`InvalidChecksumInitial`. Construction binds bytes, observation computes them,
append evaluates the receiver and binds a new chunk without retaining its history.

## `checksum/crc`

[Implementation](../../std/checksum/crc.ok) · [Contract](checksum-guide.md#checksumcrc-contract)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Crc32` does `Checksumming` | `Crc32()`<br>`Crc32(origin is @byte)`<br>`Crc32(origin is @byte, initial is int)` | `value() is int`<br>`state(origin is @byte) is ^Crc32` |
| `Crc16Ccitt` does `Checksumming` | `Crc16Ccitt(initial is int)`<br>`Crc16Ccitt(origin is @byte, initial is int)` | `value() is int`<br>`state(origin is @byte) is ^Crc16Ccitt` |

CRC-32/ISO-HDLC defaults to public seed 0 (a finalized CRC); CRC-16 uses
polynomial 0x1021 without reflection/final XOR and requires a seed. Invalid
unsigned 32-/16-bit seeds raise `InvalidChecksumInitial` at construction.
Both compute lazily and append immutably; empty input preserves the seed.

## `checksum/state`

[Implementation](../../std/checksum/state.ok) · [Contract](checksum-guide.md)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Checksumming` | — | `state(origin is @byte) is ^Checksumming`<br>`value() is int` |
| `InvalidChecksumInitial` does `Error` | `InvalidChecksumInitial(initial is int, maximum is int)` | `initial() is int`<br>`maximum() is int`<br>`text() is string` |

Successors append bytes without mutating the origin; results are non-negative
and non-cryptographic. The test-only role fake counts bytes.

## `collections/combinatorics`

[Implementation](../../std/collections/combinatorics.ok) · [Design decision](stdlib-api-audit-v4.md#stdcollectionscombinatoricsok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Combinations<T>` does `Iterating<@T>` | `Combinations(origin is Sequencing<T>, size is int)` | `cursor() is Cursor<@T>` |
| `CombinationCursor<T>` does `Cursor<@T>` | `CombinationCursor(origin is Sequencing<T>, positions is @int, size is int, active is bool)` | `available() is bool`<br>`item() is @T`<br>`successor() is Cursor<@T>` |
| `CombinationsWithReplacement<T>` does `Iterating<@T>` | `CombinationsWithReplacement(origin is Sequencing<T>, size is int)` | `cursor() is Cursor<@T>` |
| `ReplacementCombinationCursor<T>` does `Cursor<@T>` | `ReplacementCombinationCursor(origin is Sequencing<T>, positions is @int, size is int, active is bool)` | `available() is bool`<br>`item() is @T`<br>`successor() is Cursor<@T>` |
| `Permutations<T>` does `Iterating<@T>` | `Permutations(origin is Sequencing<T>)`<br>`Permutations(origin is Sequencing<T>, size is int)` | `cursor() is Cursor<@T>` |
| `PermutationCursor<T>` does `Cursor<@T>` | `PermutationCursor(origin is Sequencing<T>, positions is @int, size is int, active is bool)` | `available() is bool`<br>`item() is @T`<br>`successor() is Cursor<@T>` |
| `CartesianProduct<T>` does `Iterating<@T>` | `CartesianProduct(origins is Sequencing<Sequencing<T>>)` **named only** | `cursor() is Cursor<@T>` |
| `CartesianCursor<T>` does `Cursor<@T>` | `CartesianCursor(origins is Sequencing<Sequencing<T>>, positions is @int, active is bool)` | `available() is bool`<br>`item() is @T`<br>`successor() is Cursor<@T>` |
| `CombinationPositions` | `CombinationPositions(positions is @int, origin_size is int)` | `valid() is bool` |
| `CombinationPivot` | `CombinationPivot(positions is @int, origin_size is int)` | `value() is int` |
| `ReplacementPositions` | `ReplacementPositions(positions is @int, origin_size is int)` | `valid() is bool` |
| `ReplacementPivot` | `ReplacementPivot(positions is @int, origin_size is int)` | `value() is int` |
| `PermutationPositions` | `PermutationPositions(positions is @int, origin_size is int)` | `valid() is bool` |
| `PrefixMembership` | `PrefixMembership(positions is @int, stop is int, value is int)` | `result() is bool` |
| `PermutationCandidate` | `PermutationCandidate(positions is @int, prefix is int, origin_size is int, start is int)` | `#projected value() is int` |
| `PermutationPivot` | `PermutationPivot(positions is @int, origin_size is int)` | `value() is int` |
| `PrefixBefore` | `PrefixBefore(positions is @int, stop is int, value is int)` | `amount() is int` |
| `PermutationAvailableIndex` | `PermutationAvailableIndex(positions is @int, prefix is int, selected is int, origin_size is int, offset is int)` | `#projected value() is int` |
| `CartesianAvailability<T>` | `CartesianAvailability(origins is Sequencing<Sequencing<T>>)` **named only** | `result() is bool` |
| `CartesianPivot<T>` | `CartesianPivot(origins is Sequencing<Sequencing<T>>, positions is @int)` | `value() is int` |
| `CartesianPositions<T>` | `CartesianPositions(origins is Sequencing<Sequencing<T>>, positions is @int)` | `valid() is bool` |

## `collections/combining`

[Implementation](../../std/collections/combining.ok) · [Design decision](stdlib-api-audit-v4.md#stdcollectionscombiningok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Pair<A,B>` | `Pair(first is A, second is B)` | `first() is A`<br>`second() is B` |
| `Zipped<A,B>` does `Iterating<Pair<A,B>>` | `Zipped(left is Iterating<A>, right is Iterating<B>)` **named only** | `cursor() is Cursor<Pair<A,B>>` |
| `ZippedCursor<A,B>` does `Cursor<Pair<A,B>>` | `ZippedCursor(left is Cursor<A>, right is Cursor<B>)` **named only** | `available() is bool`<br>`item() is Pair<A,B>`<br>`successor() is Cursor<Pair<A,B>>` |
| `StrictlyZipped<A,B>` does `Iterating<Pair<A,B>>` | `StrictlyZipped(left is Iterating<A>, right is Iterating<B>)` **named only** | `cursor() is Cursor<Pair<A,B>>` |
| `StrictlyZippedCursor<A,B>` does `Cursor<Pair<A,B>>` | `StrictlyZippedCursor(left is Cursor<A>, right is Cursor<B>)` **named only** | `available() is bool`<br>`item() is Pair<A,B>`<br>`successor() is Cursor<Pair<A,B>>` |
| `UnequalSizes` does `Error` | `UnequalSizes()` | `text() is string` |
| `Chained<T>` does `Iterating<T>` | `Chained(origins is Iterating<Iterating<T>>)` **named only** | `cursor() is Cursor<T>` |
| `ChainedCursorSearch<T>` | `ChainedCursorSearch(origins is Cursor<Iterating<T>>)` **named only** | `result() is Cursor<T>` |
| `ChainedCursorCandidates<T>` does `Iterating<ChainedCursorCandidateValue<T>>` | `ChainedCursorCandidates(origins is Cursor<Iterating<T>>)` **named only** | `cursor() is Cursor<ChainedCursorCandidateValue<T>>` |
| `ChainedCursorCandidate<T>` does `Cursor<ChainedCursorCandidateValue<T>>` | `ChainedCursorCandidate(origins is Cursor<Iterating<T>>)` **named only** | `available() is bool`<br>`item() is ChainedCursorCandidateValue<T>`<br>`successor() is Cursor<ChainedCursorCandidateValue<T>>` |
| `ChainedCursorCandidateValue<T>` | `ChainedCursorCandidateValue(origin is Iterating<T>, remaining is Cursor<Iterating<T>>)` | `origin() is Iterating<T>`<br>`remaining() is Cursor<Iterating<T>>` |
| `ExhaustedChainedCursor<T>` does `Cursor<T>` | `ExhaustedChainedCursor(origins is Cursor<Iterating<T>>)` **named only** | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `ChainedItemCursor<T>` does `Cursor<T>` | `ChainedItemCursor(current is Cursor<T>, remaining is Cursor<Iterating<T>>)` **named only** | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |

## `collections/deque`

[Implementation](../../std/collections/deque.ok) · [Design decision](stdlib-api-audit-v4.md#stdcollectionsdequeok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Deque<T>` does `Iterating<T>`, `Sizing` | `Deque(values is Iterating<T>)`<br>`Deque(front is List<T>, back is List<T>, size is int)` | `cursor() is Cursor<T>`<br>`size() is int`<br>`item(index is int) is T`<br>`front() is DequeFront<T>`<br>`back() is DequeBack<T>` |
| `DequeFront<T>` | `DequeFront(front is List<T>, back is List<T>, size is int)` | `value() is T`<br>`addition(value is T) is Deque<T>`<br>`remainder() is Deque<T>` |
| `DequeBack<T>` | `DequeBack(front is List<T>, back is List<T>, size is int)` | `value() is T`<br>`addition(value is T) is Deque<T>`<br>`remainder() is Deque<T>` |
| `DequeItems<T>` does `Iterating<T>` | `DequeItems(front is List<T>, back is List<T>)` | `cursor() is Cursor<T>` |
| `ConcatenatedDequeCursor<T>` does `Cursor<T>` | `ConcatenatedDequeCursor(front is Cursor<T>, back is Cursor<T>)` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `ArrayItems<T>` does `Iterating<T>` | `ArrayItems(values is @T, witness is Iterating<T>)` | `cursor() is Cursor<T>` |
| `ArrayCursor<T>` does `Cursor<T>` | `ArrayCursor(values is @T, position is int, witness is Iterating<T>)` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `ForwardList<T>` | `ForwardList(values is Iterating<T>)` | `list() is List<T>` |
| `ReversedList<T>` | `ReversedList(values is Iterating<T>)` | `list() is List<T>` |
| `DequeParts<T>` | `DequeParts(front is List<T>, back is List<T>, size is int)` | `front() is List<T>`<br>`back() is List<T>`<br>`size() is int` |
| `BalancedDeque<T>` | `BalancedDeque(values is Iterating<T>)` | `parts() is DequeParts<T>` |
| `NormalizedDeque<T>` | `NormalizedDeque(front is List<T>, back is List<T>, size is int)` | `parts() is DequeParts<T>` |
| `EmptyDequeAccess` does `Error` | `EmptyDequeAccess(operation is string)` | `operation() is string`<br>`text() is string` |

## `collections/enumeration`

[Implementation](../../std/collections/enumeration.ok) · [Design decision](stdlib-api-audit-v4.md#stdcollectionsenumerationok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Indexed<T>` | `Indexed(index is int, item is T)` | `index() is int`<br>`item() is T` |
| `Enumerated<T>` does `Iterating<Indexed<T>>` | `Enumerated(origin is Iterating<T>)` **named only**<br>`Enumerated(origin is Iterating<T>, start is int)` **named only** | `cursor() is Cursor<Indexed<T>>` |
| `EnumerationCursor<T>` does `Cursor<Indexed<T>>` | `EnumerationCursor(origin is Cursor<T>, position is int)` **named only** | `available() is bool`<br>`item() is Indexed<T>`<br>`successor() is Cursor<Indexed<T>>` |
| `EnumerationOverflow` does `Error` | `EnumerationOverflow(index is int)` | `index() is int`<br>`text() is string` |

## `collections/frequencies`

[Implementation](../../std/collections/frequencies.ok) · [Design decision](stdlib-api-audit-v4.md#stdcollectionsfrequenciesok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Frequency<T>` | `Frequency(item is T, amount is int)` | `item() is T`<br>`amount() is int` |
| `Frequencies<T>` does `Iterating<Frequency<T>>`, `Sizing` | `Frequencies(values is Iterating<T>)` | `cursor() is Cursor<Frequency<T>>`<br>`size() is int`<br>`amount(value is T) is int`<br>`total() is int` |
| `FrequencyBuilding<T>` | `FrequencyBuilding(frequencies is List<Frequency<T>>, total is int)` | `frequencies() is List<Frequency<T>>`<br>`total() is int`<br>`addition(value is T) is FrequencyBuilding<T>` |
| `FrequencyRevision<T>` | `FrequencyRevision(frequencies is List<Frequency<T>>, value is T, present is bool)` | `frequencies() is List<Frequency<T>>`<br>`membership() is bool`<br>`addition(frequency is Frequency<T>) is FrequencyRevision<T>` |

## `collections/heap`

[Implementation](../../std/collections/heap.ok) · [Design decision](stdlib-api-audit-v4.md#stdcollectionsheapok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Heap<T>` does `Sizing` | `Heap(values is Iterating<T>)`<br>`Heap(root is HeapRepresenting<T>, size is int)` | `size() is int`<br>`top() is T`<br>`remainder() is Heap<T>`<br>`addition(value is T) is Heap<T>` |
| `HeapBuilding<T>` | `HeapBuilding(root is HeapRepresenting<T>)` | `root() is HeapRepresenting<T>`<br>`addition(value is T) is HeapBuilding<T>` |
| `HeapRepresenting<T>` | — | `size() is int`<br>`rank() is int`<br>`top() is T`<br>`remainder() is HeapRepresenting<T>`<br>`combination(other is HeapRepresenting<T>) is HeapRepresenting<T>` |
| `EmptyHeapTree<T>` does `HeapRepresenting<T>` | `EmptyHeapTree()` | `size() is int`<br>`rank() is int`<br>`top() is T`<br>`remainder() is HeapRepresenting<T>`<br>`combination(other is HeapRepresenting<T>) is HeapRepresenting<T>` |
| `HeapTreeNode<T>` does `HeapRepresenting<T>` | `HeapTreeNode(value is T, left is HeapRepresenting<T>, right is HeapRepresenting<T>)` | `size() is int`<br>`rank() is int`<br>`top() is T`<br>`remainder() is HeapRepresenting<T>`<br>`combination(other is HeapRepresenting<T>) is HeapRepresenting<T>` |
| `HeapChildren<T>` | `HeapChildren(left is HeapRepresenting<T>, right is HeapRepresenting<T>)` | `left() is HeapRepresenting<T>`<br>`right() is HeapRepresenting<T>` |

## `collections/iteration`

[Implementation](../../std/collections/iteration.ok) · [Design decision](stdlib-api-audit-v4.md#stdcollectionsiterationok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Cursor<T>` | — | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `Iterating<T>` | — | `cursor() is Cursor<T>` |
| `IterationEnded` does `Error` | `IterationEnded()` | `text() is string` |
| `CursorItems<T>` does `Iterating<T>` | `CursorItems(cursor is Cursor<T>)` | `cursor() is Cursor<T>` |

## `collections/list`

[Implementation](../../std/collections/list.ok) · [Design decision](stdlib-api-audit-v4.md#stdcollectionslistok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `List<T>` | — | `cursor() is Cursor<T>`<br>`size() is int`<br>`first() is T`<br>`tail() is List<T>`<br>`prefix(value is T) is List<T>` |
| `EmptyList<T>` does `List<T>`, `Iterating<T>`, `Sizing` | `EmptyList()` | `cursor() is Cursor<T>`<br>`size() is int`<br>`first() is T`<br>`tail() is List<T>`<br>`prefix(value is T) is List<T>` |
| `LinkedList<T>` does `List<T>`, `Iterating<T>`, `Sizing` | `LinkedList(value is T, following is List<T>)` | `cursor() is Cursor<T>`<br>`size() is int`<br>`first() is T`<br>`tail() is List<T>`<br>`prefix(value is T) is List<T>` |
| `EmptyListCursor<T>` does `Cursor<T>` | `EmptyListCursor(origin is List<T>)` **named only** | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `LinkedListCursor<T>` does `Cursor<T>` | `LinkedListCursor(value is T, following is List<T>)` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `EmptyListAccess` does `Error` | `EmptyListAccess(operation is string)` | `operation() is string`<br>`text() is string` |

## `collections/ordering`

[Implementation](../../std/collections/ordering.ok) · [Design decision](stdlib-api-audit-v4.md#stdcollectionsorderingok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Sorted<T>` does `Sequencing<T>`, `Iterating<T>`, `Sizing` | `Sorted(origin is Iterating<T>)` | `cursor() is Cursor<T>`<br>`size() is int`<br>`item(index is int) is T`<br>`first() is T`<br>`last() is T` |
| `SortedBy<T,K>` does `Sequencing<T>`, `Iterating<T>`, `Sizing` | `SortedBy(origin is Iterating<T>, key is (T) is K)` | `cursor() is Cursor<T>`<br>`size() is int`<br>`item(index is int) is T`<br>`first() is T`<br>`last() is T` |
| `Ranked<T,K>` does `Comparing<Ranked<T,K>>` | `Ranked(item is T, key is K)` | `item() is T`<br>`key() is K`<br>`order(other is Ranked<T,K>) is Order` |
| `PositionOf<T>` | `PositionOf(origin is Sequencing<T>, item is T)` | `#projected value() is int` |
| `LowerBound<T>` | `LowerBound(origin is Sequencing<T>, item is T)` | `#projected value() is int` |
| `UpperBound<T>` | `UpperBound(origin is Sequencing<T>, item is T)` | `#projected value() is int` |
| `LowerSearching<T>` | `LowerSearching(origin is Sequencing<T>, item is T, start is int, stop is int)` | `#projected value() is int` |
| `UpperSearching<T>` | `UpperSearching(origin is Sequencing<T>, item is T, start is int, stop is int)` | `#projected value() is int` |
| `OrderedCursor<T>` does `Cursor<T>` | `OrderedCursor(values is @T, position is int)` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `RankedCursor<T,K>` does `Cursor<T>` | `RankedCursor(values is @Ranked<T,K>, position is int)` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `MissingItem` does `Error` | `MissingItem()` | `text() is string` |

## `collections/repetition`

[Implementation](../../std/collections/repetition.ok) · [Design decision](stdlib-api-audit-v4.md#stdcollectionsrepetitionok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Repeating<T>` does `Iterating<T>` | `Repeating(value is T)` | `cursor() is Cursor<T>` |
| `RepeatingCursor<T>` does `Cursor<T>` | `RepeatingCursor(value is T)` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `Repeated<T>` does `Sequencing<T>`, `Iterating<T>`, `Sizing` | `Repeated(value is T, count is int)` | `cursor() is Cursor<T>`<br>`size() is int`<br>`item(index is int) is T`<br>`first() is T`<br>`last() is T` |
| `RepeatedCursor<T>` does `Cursor<T>` | `RepeatedCursor(value is T, position is int, count is int)` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `Cycled<T>` does `Iterating<T>` | `Cycled(origin is Sequencing<T>)` **named only** | `cursor() is Cursor<T>` |
| `CycledCursor<T>` does `Cursor<T>` | `CycledCursor(origin is Sequencing<T>, position is int)` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `EmptyCycleCursor<T>` does `Cursor<T>` | `EmptyCycleCursor(origin is Sequencing<T>)` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |

## `collections/sequence`

[Implementation](../../std/collections/sequence.ok) · [Design decision](stdlib-api-audit-v4.md#stdcollectionssequenceok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Sequencing<T>` | — | `cursor() is Cursor<T>`<br>`size() is int`<br>`item(index is int) is T`<br>`first() is T`<br>`last() is T` |
| `EmptySequence` does `Error` | `EmptySequence()` | `text() is string` |

## `collections/set`

[Implementation](../../std/collections/set.ok) · [Design decision](stdlib-api-audit-v4.md#stdcollectionssetok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Set<T>` | — | `cursor() is Cursor<T>`<br>`size() is int`<br>`membership(value is T) is bool`<br>`addition(value is T) is Set<T>`<br>`difference(value is T) is Set<T>` |
| `OrderedSet<T>` does `Set<T>`, `Iterating<T>`, `Sizing` | `OrderedSet(values is Iterating<T>)` | `cursor() is Cursor<T>`<br>`size() is int`<br>`membership(value is T) is bool`<br>`addition(value is T) is Set<T>`<br>`difference(value is T) is Set<T>` |
| `Union<T>` does `Set<T>`, `Iterating<T>`, `Sizing` | `Union(left is Set<T>, right is Set<T>)` **named only** | `cursor() is Cursor<T>`<br>`size() is int`<br>`membership(value is T) is bool`<br>`addition(value is T) is Set<T>`<br>`difference(value is T) is Set<T>` |
| `Intersection<T>` does `Set<T>`, `Iterating<T>`, `Sizing` | `Intersection(left is Set<T>, right is Set<T>)` **named only** | `cursor() is Cursor<T>`<br>`size() is int`<br>`membership(value is T) is bool`<br>`addition(value is T) is Set<T>`<br>`difference(value is T) is Set<T>` |
| `Difference<T>` does `Set<T>`, `Iterating<T>`, `Sizing` | `Difference(left is Set<T>, right is Set<T>)` **named only** | `cursor() is Cursor<T>`<br>`size() is int`<br>`membership(value is T) is bool`<br>`addition(value is T) is Set<T>`<br>`difference(value is T) is Set<T>` |
| `SymmetricDifference<T>` does `Set<T>`, `Iterating<T>`, `Sizing` | `SymmetricDifference(left is Set<T>, right is Set<T>)` **named only** | `cursor() is Cursor<T>`<br>`size() is int`<br>`membership(value is T) is bool`<br>`addition(value is T) is Set<T>`<br>`difference(value is T) is Set<T>` |
| `SetWith<T>` does `Set<T>`, `Iterating<T>`, `Sizing` | `SetWith(origin is Set<T>, value is T)` | `cursor() is Cursor<T>`<br>`size() is int`<br>`membership(value is T) is bool`<br>`addition(value is T) is Set<T>`<br>`difference(value is T) is Set<T>` |
| `SetWithout<T>` does `Set<T>`, `Iterating<T>`, `Sizing` | `SetWithout(origin is Set<T>, value is T)` | `cursor() is Cursor<T>`<br>`size() is int`<br>`membership(value is T) is bool`<br>`addition(value is T) is Set<T>`<br>`difference(value is T) is Set<T>` |
| `SetWithCursor<T>` does `Cursor<T>` | `SetWithCursor(origin is Cursor<T>, value is T, emitted is bool)` **named only** | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `ConcatenatedSetCursor<T>` does `Cursor<T>` | `ConcatenatedSetCursor(left is Cursor<T>, right is Cursor<T>)` **named only** | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `SetMembership<T>` | `SetMembership(origin is Iterating<T>, value is T)` **named only** | `result() is bool` |
| `SetSize<T>` | `SetSize(origin is Iterating<T>)` **named only** | `result() is int` |
| `AbsentFrom<T>` | `AbsentFrom(origin is Set<T>)` **named only** | `membership(value is T) is bool` |
| `DifferentFrom<T>` | `DifferentFrom(origin is T)` **named only** | `comparison(value is T) is bool` |
| `DistinctList<T>` | `DistinctList(values is Iterating<T>)` | `list() is List<T>` |

## `collections/views`

[Implementation](../../std/collections/views.ok) · [Design decision](stdlib-api-audit-v4.md#stdcollectionsviewsok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `CursorStep<T>` | `CursorStep(current is Cursor<T>, following is Cursor<T>)` | `current() is Cursor<T>`<br>`following() is Cursor<T>`<br>`item() is T` |
| `CursorSteps<T>` does `Iterating<CursorStep<T>>` | `CursorSteps(cursor is Cursor<T>)` | `cursor() is Cursor<CursorStep<T>>` |
| `CursorStepCursor<T>` does `Cursor<CursorStep<T>>` | `CursorStepCursor(origin is Cursor<T>)` **named only** | `available() is bool`<br>`item() is CursorStep<T>`<br>`successor() is Cursor<CursorStep<T>>` |
| `Mapped<A,B>` does `Iterating<B>` | `Mapped(origin is Iterating<A>, mapping is (A) is B)` **named only** | `cursor() is Cursor<B>` |
| `MappedCursorSearch<A,B>` | `MappedCursorSearch(origin is Cursor<A>, mapping is (A) is B)` **named only** | `result() is Cursor<B>` |
| `MappedCursor<A,B>` does `Cursor<B>` | `MappedCursor(value is B, remaining is Cursor<A>, mapping is (A) is B)` | `available() is bool`<br>`item() is B`<br>`successor() is Cursor<B>` |
| `EmptyMappedCursor<A,B>` does `Cursor<B>` | `EmptyMappedCursor(origin is Cursor<A>, mapping is (A) is B)` **named only** | `available() is bool`<br>`item() is B`<br>`successor() is Cursor<B>` |
| `Filtered<T>` does `Iterating<T>` | `Filtered(origin is Iterating<T>, condition is (T) is bool)` **named only** | `cursor() is Cursor<T>` |
| `FilteredCursorSearch<T>` | `FilteredCursorSearch(origin is Cursor<T>, condition is (T) is bool)` **named only** | `result() is Cursor<T>` |
| `FilteredCursor<T>` does `Cursor<T>` | `FilteredCursor(origin is Cursor<T>, condition is (T) is bool)` **named only** | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `EmptyFilteredCursor<T>` does `Cursor<T>` | `EmptyFilteredCursor(origin is Cursor<T>, condition is (T) is bool)` **named only** | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `Taken<T>` does `Iterating<T>` | `Taken(origin is Iterating<T>, count is int)` | `cursor() is Cursor<T>` |
| `TakenCursor<T>` does `Cursor<T>` | `TakenCursor(origin is Cursor<T>, remaining is int)` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `Dropped<T>` does `Iterating<T>` | `Dropped(origin is Iterating<T>, count is int)` | `cursor() is Cursor<T>` |
| `DroppedCursorSearch<T>` | `DroppedCursorSearch(origin is Cursor<T>, count is int)` | `result() is Cursor<T>` |
| `TakenWhile<T>` does `Iterating<T>` | `TakenWhile(origin is Iterating<T>, condition is (T) is bool)` **named only** | `cursor() is Cursor<T>` |
| `TakenWhileCursorSearch<T>` | `TakenWhileCursorSearch(origin is Cursor<T>, condition is (T) is bool)` **named only** | `result() is Cursor<T>` |
| `TakenWhileCursor<T>` does `Cursor<T>` | `TakenWhileCursor(origin is Cursor<T>, condition is (T) is bool)` **named only** | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `EmptyTakenWhileCursor<T>` does `Cursor<T>` | `EmptyTakenWhileCursor(origin is Cursor<T>, condition is (T) is bool)` **named only** | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `DroppedWhile<T>` does `Iterating<T>` | `DroppedWhile(origin is Iterating<T>, condition is (T) is bool)` **named only** | `cursor() is Cursor<T>` |
| `DroppedWhileCursorSearch<T>` | `DroppedWhileCursorSearch(origin is Cursor<T>, condition is (T) is bool)` **named only** | `result() is Cursor<T>` |
| `Chunked<T>` does `Iterating<@T>` | `Chunked(origin is Iterating<T>, size is int)` | `cursor() is Cursor<@T>` |
| `ChunkedCursor<T>` does `Cursor<@T>` | `ChunkedCursor(origin is Cursor<T>, size is int)` | `available() is bool`<br>`item() is @T`<br>`successor() is Cursor<@T>` |
| `Flattened<T>` does `Iterating<T>` | `Flattened(origin is Iterating<Iterating<T>>)` **named only** | `cursor() is Cursor<T>` |
| `Pairwise<T>` does `Iterating<Pair<T,T>>` | `Pairwise(origin is Iterating<T>)` **named only** | `cursor() is Cursor<Pair<T,T>>` |
| `PairwiseCursor<T>` does `Cursor<Pair<T,T>>` | `PairwiseCursor(origin is Cursor<T>)` **named only** | `available() is bool`<br>`item() is Pair<T,T>`<br>`successor() is Cursor<Pair<T,T>>` |
| `Accumulated<T,R>` does `Iterating<R>` | `Accumulated(origin is Iterating<T>, initial is R, action is (R,T) is R)` **named only** | `cursor() is Cursor<R>` |
| `AccumulatedCursor<T,R>` does `Cursor<R>` | `AccumulatedCursor(origin is Cursor<T>, current is R, action is (R,T) is R)` **named only** | `available() is bool`<br>`item() is R`<br>`successor() is Cursor<R>` |
| `ExhaustedAccumulatedCursor<R>` does `Cursor<R>` | `ExhaustedAccumulatedCursor(value is R)` | `available() is bool`<br>`item() is R`<br>`successor() is Cursor<R>` |
| `Reduced<T,R>` | `Reduced(origin is Iterating<T>, initial is R, action is (R,T) is R)` **named only** | `#projected value() is R` |
| `Reversed<T>` does `Sequencing<T>`, `Iterating<T>`, `Sizing` | `Reversed(origin is Sequencing<T>)` **named only** | `cursor() is Cursor<T>`<br>`size() is int`<br>`item(index is int) is T`<br>`first() is T`<br>`last() is T` |
| `ReversedCursor<T>` does `Cursor<T>` | `ReversedCursor(origin is Sequencing<T>, position is int)` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `Sliced<T>` does `Iterating<T>` | `Sliced(origin is Iterating<T>, start is int, stop is int, step is int)` | `cursor() is Cursor<T>` |
| `SlicedCursor<T>` does `Cursor<T>` | `SlicedCursor(origin is Cursor<T>, position is int, stop is int, step is int)` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |

## `compare`

[Implementation](../../std/compare.ok) · [Design decision](stdlib-api-audit-v4.md#stdcompareok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Order` | — | Language/native contract; no declared actions |
| `Comparing<T>` | — | `order(other is T) is Order` |

## `csv`

[Implementation](../../std/csv.ok) / [Contract and remaining work](csv-guide.md)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `CsvString` | `CsvString(rows is Iterating<@string>)`<br>`CsvString(rows is Iterating<@string>, maximum_output is int, maximum_field is int, maximum_rows is int, maximum_columns is int)`<br>`CsvString(rows is Iterating<@string>, dialect is CsvDialect)`<br>`CsvString(rows is Iterating<@string>, maximum_output is int, maximum_field is int, maximum_rows is int, maximum_columns is int, dialect is CsvDialect)` | `#projected string() is string` |
| `CsvRecord` | `CsvRecord(fields is @string)` **named only** | `#projected string() is string` |
| `CsvField` | `CsvField(origin is string)` **named only** | `size() is int`<br>`#projected string() is string` |
| `CsvDialect` | `CsvDialect(delimiter is string, quote is string, line_ending is string)` | `symbol(value is string) is CsvSymbol`<br>`encoded(value is string) is string`<br>`decoded(value is string) is string`<br>`size(value is string) is int`<br>`record(fields is @string, allowance is int) is string` |
| `CsvSymbol` | — | ordinary, separator, quote, pending, end, invalid |
| `InvalidCsvDialect` does `Error` | `InvalidCsvDialect()` | `text() is string` |
| `EmptyCsvRecord` does `Error` | `EmptyCsvRecord()` | `text() is string` |
| `CsvRows` does `Iterating<@string>` | `CsvRows(origin is string)`<br>`CsvRows(origin is string, maximum_input is int, maximum_field is int, maximum_rows is int, maximum_columns is int)`<br>`CsvRows(origin is string, dialect is CsvDialect)`<br>`CsvRows(origin is string, maximum_input is int, maximum_field is int, maximum_rows is int, maximum_columns is int, dialect is CsvDialect)` | `cursor() is Cursor<@string>` |
| `InvalidCsv` does `Error` | `InvalidCsv(offset is int)` | `offset() is int`<br>`text() is string` |
| `CsvLimit` | — | input, field, rows, columns, output |
| `InvalidCsvLimits` does `Error` | `InvalidCsvLimits()` | `text() is string` |
| `CsvLimitExceeded` does `Error` | `CsvLimitExceeded(limit is CsvLimit)` | `kind() is CsvLimit`<br>`text() is string` |
| `CsvMode` | — | Implementation enum: start, plain, quoted, closed, cr |
| `CsvPrefix` | `CsvPrefix(origin is string, maximum_field is int, maximum_rows is int, maximum_columns is int)`<br>`CsvPrefix(origin is string, maximum_field is int, maximum_rows is int, maximum_columns is int, dialect is CsvDialect)` | `following(symbol is string, offset is int) is ^CsvPrefix`<br>`records(end is int) is List<@string>` |
| `CsvSnapshot` | `CsvSnapshot(maximum_field is int, maximum_rows is int, maximum_columns is int)` | `following(row is @string) is ^CsvSnapshot`<br>`rows() is @@string` |
| `CsvPrintPrefix` | `CsvPrintPrefix(maximum_output is int)`<br>`CsvPrintPrefix(maximum_output is int, dialect is CsvDialect)` | `following(row is @string) is ^CsvPrintPrefix`<br>`string() is string` |
| `CsvPrintRecord` | `CsvPrintRecord(fields is @string, allowance is int)` **named only** | `string() is string` |

CsvPrefix, CsvMode, CsvSnapshot, CsvPrintPrefix and CsvPrintRecord are importable implementation collaborators, not a new
public parsing protocol: the language currently has no module-private declarations.
CsvPrefix represents a consumed prefix; `following` accepts one scalar and its
nonnegative source offset, and `records` validates completion at the EOF offset.
Neither exposes private parse state. See the CSV guide for limits and timing.


## `csv/input`

[Implementation](../../std/csv/input.ok) · [Contract](csv-input-guide.md)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `CsvInput` | `CsvInput(origin is Reading)`<br>`CsvInput(origin is Reading, dialect is CsvDialect)`<br>`CsvInput(origin is Reading, maximum_bytes is int, maximum_input is int, maximum_field is int, maximum_rows is int, maximum_columns is int, dialect is CsvDialect)` | `read() is @@string` |
| `CsvByteLimitExceeded` does `Error` | `CsvByteLimitExceeded()` | `text() is string` |

Explicit synchronous bounded document consumption from a borrowed source. No
cursor I/O or implicit close; repeated calls consume remaining input. Failure
returns no rows and does not roll back consumed bytes. In-module tests cover
configuration, limits, decoding, errors, repeat EOF and borrowing.

## `archive/path`

[Implementation](../../std/archive/path.ok) · [Contract](archive-guide.md#archivepath)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `ArchivePath` | `ArchivePath(origin is string)` | `string() is string`<br>`segments() is Sequencing<string>`<br>`parent() is ArchivePath`<br>`name() is string` |
| `UnsafeArchivePath` does `Error` | `UnsafeArchivePath(origin is string, message is string)` | `path() is string`<br>`text() is string` |
| `MissingArchiveParent` does `Error` | `MissingArchiveParent(path is ArchivePath)` | `path() is ArchivePath`<br>`text() is string` |

Immutable relative archive names with exact Unicode spelling and slash segments.
Rejects empty/absolute/UNC/drive-prefixed names, backslashes, NUL and empty/dot/
parent segments. No host path conversion or I/O. A one-segment path has no parent;
lexical validation alone does not guarantee destination containment on extraction.

## `archive/metadata`

[Implementation](../../std/archive/metadata.ok) · [Contract](archive-guide.md#archivemetadata)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `ArchiveOwnership` | `ArchiveOwnership(user is string, group is string, user_identity is BigInt, group_identity is BigInt)` | `user() is string`<br>`group() is string`<br>`user_identity() is BigInt`<br>`group_identity() is BigInt` |
| `ArchiveMetadata` | `ArchiveMetadata(modified is LocalDateTime, comment is string)`<br>`ArchiveMetadata(mode is PermissionMode, modified is LocalDateTime, comment is string)`<br>`ArchiveMetadata(mode is PermissionMode, modified is LocalDateTime, ownership is ArchiveOwnership, comment is string)` | `mode() is PermissionMode`<br>`modification() is LocalDateTime`<br>`ownership() is ArchiveOwnership`<br>`comment() is string` |
| `ArchiveModificationFromEpoch` | `ArchiveModificationFromEpoch(seconds is rat)` | `#projected value() is LocalDateTime` |
| `MissingArchiveMode` does `Error` | `MissingArchiveMode()` | `text() is string` |
| `MissingArchiveOwnership` does `Error` | `MissingArchiveOwnership()` | `text() is string` |
| `InvalidArchiveOwnership` does `Error` | `InvalidArchiveOwnership()` | `text() is string` |

Immutable format-boundary attributes, with typed absence and nonnegative BigInt
identities. Exact names/comments and calendar fields are retained without host
lookups or effects. Epoch conversion explicitly yields UTC calendar fields with
nanosecond precision, rejecting unsupported precision/range through the existing
time errors. Ownership without mode is outside the three initial constructors.

## `archive/entry`

[Implementation](../../std/archive/entry.ok) · [Contract](archive-guide.md#archiveentry)

`ArchiveEntryKind` contains `file`, `directory`, `symbolic_link`, `hard_link`,
`fifo`, `character_device`, `block_device`.

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `ArchiveEntry` | — | `path() is ArchivePath`<br>`kind() is ArchiveEntryKind`<br>`metadata() is ArchiveMetadata`<br>`accept(to is ArchiveEntryVisitor) is void` |
| `ArchiveEntryVisitor` | — | `file(entry is ArchiveFile) is void`<br>`directory(entry is ArchiveDirectory) is void`<br>`symbolic_link(entry is ArchiveSymbolicLink) is void`<br>`hard_link(entry is ArchiveHardLink) is void`<br>`fifo(entry is ArchiveFifo) is void`<br>`device(entry is ArchiveDevice) is void` |
| `ArchiveFile` does `ArchiveEntry` | `ArchiveFile(path is ArchivePath, metadata is ArchiveMetadata, data is @byte)` | Common entry actions; `data() is @byte` |
| `ArchiveDirectory` does `ArchiveEntry` | `ArchiveDirectory(path is ArchivePath, metadata is ArchiveMetadata)` | Common entry actions |
| `ArchiveSymbolicLink` does `ArchiveEntry` | `ArchiveSymbolicLink(path is ArchivePath, metadata is ArchiveMetadata, target is ArchivePath)` | Common entry actions; `target() is ArchivePath` |
| `ArchiveHardLink` does `ArchiveEntry` | `ArchiveHardLink(path is ArchivePath, metadata is ArchiveMetadata, target is ArchivePath)` | Common entry actions; `target() is ArchivePath` |
| `ArchiveFifo` does `ArchiveEntry` | `ArchiveFifo(path is ArchivePath, metadata is ArchiveMetadata)` | Common entry actions |
| `ArchiveDevice` does `ArchiveEntry` | `ArchiveDevice(path is ArchivePath, kind is ArchiveEntryKind, metadata is ArchiveMetadata, major is int, minor is int)` | Common entry actions; `major() is int`<br>`minor() is int` |
| `InvalidArchiveDevice` does `Error` | `InvalidArchiveDevice(message is string)` | `text() is string` |

Immutable members with exactly one nominal visitor event per accept. File bytes
are already materialized; non-files have no data action. Links remain distinct
and unresolved; devices validate kind and nonnegative numbers. No filesystem or
extraction effects. Role fake represents a coherent empty file named `entry`.

## `archive/limits`

[Implementation](../../std/archive/limits.ok) · [Contract](archive-guide.md#archivelimits)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `ArchiveLimits` | `ArchiveLimits(entries is int, entry_size is int, total_size is int, path_length is int)` | `entries() is int`<br>`entry_size() is int`<br>`total_size() is int`<br>`path_length() is int`<br>`path(origin is string) is ArchivePath`<br>`budget() is ArchiveBudget` |
| `ArchiveBudget` | `ArchiveBudget(limits is ArchiveLimits)` | `entries_remaining() is int`<br>`total_remaining() is int`<br>`entry_remaining() is int`<br>`path() is ArchivePath`<br>`entered(origin is string, declared_size is int) is ^ArchiveBudget`<br>`expanded(amount is int) is ^ArchiveBudget`<br>`completed() is ^ArchiveBudget` |
| `ArchiveLimitExceeded` does `Error` | `ArchiveLimitExceeded(limit is string, maximum is int)` | `limit() is string`<br>`maximum() is int`<br>`text() is string` |
| `InvalidArchiveLimit` does `Error` | `InvalidArchiveLimit(limit is string, maximum is int)` | `limit() is string`<br>`maximum() is int`<br>`text() is string` |

Positive policy maxima, expanded-byte sizes and Unicode code-point path lengths.
Immutable budgets admit one entry at a time, precheck declarations, and spend
actual chunks cumulatively before payload retention. Completing an entry restores
neither slots nor total bytes. Callers retain successors; this is accounting, not
a parser or a mutable shared guard. Real format readers must integrate it.

## `archive/stream`

[Implementation](../../std/archive/stream.ok) · [Contract](archive-guide.md#archivestream)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `ReadingArchive` | — | `entry() is ArchiveEntry`<br>`finished() is bool`<br>`close() is void`<br>`closed() is bool` |
| `WritingArchive` | — | `write(entry is ArchiveEntry) is void`<br>`finish() is void`<br>`finished() is bool`<br>`close() is void`<br>`closed() is bool` |
| `ArchiveReading` does `ReadingArchive`, `Closing` | `ArchiveReading(next_entry is () is ArchiveEntry)` | ReadingArchive actions above |
| `ArchiveWriting` does `WritingArchive`, `Closing` | `ArchiveWriting(write_entry is (ArchiveEntry) is void, finish_stream is () is void)` | WritingArchive actions above |
| `EndOfArchive` does `Error` | `EndOfArchive()` | `text() is string` |

Consuming lifecycle adapters bind explicit synchronous operations lazily. Reading
invokes one operation without prefetch and remembers normal EOF; writing forwards
entries once and marks finished only after successful finish. Other Errors retain
identity and disable subsequent processing. Close/destroy release private state
without finishing or closing borrowed byte streams. No cursor protocol; no payload
allocation bound is claimed until a concrete format operation enforces it.

## `archive/tar`

[Implementation](../../std/archive/tar.ok) · [Contract](archive-tar.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `TarFormat` | `pax`, `ustar` | PAX default; explicit strict USTAR |
| `TarArchive` | `TarArchive(entries is Iterating<ArchiveEntry> [, format is TarFormat])` | `bytes() is @byte` |
| `ParsedTar` | `ParsedTar(origin is @byte, limits is ArchiveLimits)`<br>`ParsedTar(origin is @byte, limits is ArchiveLimits, maximum_metadata is int, maximum_extensions is int)` | `entries() is Sequencing<ArchiveEntry>` |
| `TarReading` does `ReadingArchive`, `Closing` | `TarReading(origin is Reading, limits is ArchiveLimits)`<br>`TarReading(origin is Reading, limits is ArchiveLimits, maximum_metadata is int, maximum_extensions is int)` | `entry() is ArchiveEntry`; `finished()`, `closed()` return bool; `close() is void` |
| `TarWriting` does `WritingArchive`, `Closing` | `TarWriting(origin is Writing [, format is TarFormat])` | `write(entry is ArchiveEntry)`, `finish()`, `close()` return void; `finished()`, `closed()` return bool |
| `InvalidTar` does `Error` | `InvalidTar(message is string)` | `text() is string` |
| `DuplicateArchivePath` does `Error` | `DuplicateArchivePath(path is ArchivePath)` | `path() is ArchivePath`, `text() is string` |

Both writers preserve entry order and all seven variants, reject duplicate paths,
and produce deterministic checksummed headers, padding and EOF blocks. PAX extends
USTAR only when needed; unrepresentable strict USTAR fields fail before that
member is emitted. Streaming finish writes EOF and flushes once; close/destroy
never finish or close the borrowed sink. Partial errors poison the writer.
Missing-mode/ownership defaults, field bounds and exact UTC timestamp policy are
documented in the contract. ParsedTar returns an independent immutable snapshot,
checking checksums, spans, duplicates and effective paths before payload copies.
It reads USTAR, PAX local/global records and GNU names/links/base-256 numbers.
Payload admission uses ArchiveLimits; separate cumulative metadata/header bounds
default to 1 MiB/4096. The input byte array is already caller-owned.

TarReading borrows a Reading and materializes one admitted entry per call without
prefetch. It shares ParsedTar's field validation and applies cumulative budgets
before body I/O; read requests never exceed 8192 bytes or the admitted span.
Normal EndOfArchive is cached after validating the trailer and source EOF;
finished is passive. Source/format errors poison the reader, preserving the
original error. Close/destroy neither drain nor close the source; returned values
survive closure. Other `Tar*` helpers are implementation details.

## `archive/zip`

[Implementation](../../std/archive/zip.ok) · [Contract](archive-zip.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `ZipMethod` | `stored`, `deflated`, `bzip2`, `lzma`, `zstandard` | Method selection, not codec availability |
| `ZipCompression` | `ZipCompression(method is ZipMethod)`<br>`ZipCompression(method is ZipMethod, level is int)` | `method() is ZipMethod`, `level() is int` |
| `ZipArchiveEntry` | `ZipArchiveEntry(entry is ArchiveEntry, compression is ZipCompression)` | `entry() is ArchiveEntry`, `compression() is ZipCompression` |

Immutable validated writing choice with defaults 0/6/9/0/3. Explicit stored/LZMA
levels are rejected; DEFLATE accepts 0..9, bzip2 1..9, Zstandard -131072..22 with
zero normalized to 3. Invalid choices raise InvalidArgument during construction.
ZipArchiveEntry binds an archive member to an explicit choice without invoking
entry methods or visitors. It is a writing request, not itself an ArchiveEntry or
a wire representation. All nominal entry implementations can be bound; the future
writer must separately check representability before output. No byte processing,
provider discovery or resources; ZIP codecs remain pending.

## `compression/state`

[Implementation](../../std/compression/state.ok) · [Contract](compression-guide.md#compressionstate)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Compressing` | — | `write(origin is @byte) is void`<br>`output() is Reading`<br>`finish() is void`<br>`finished() is bool`<br>`close() is void`<br>`closed() is bool` |
| `Decompressing` | — | `write(origin is @byte) is void`<br>`output() is Reading`<br>`finish() is void`<br>`finished() is bool`<br>`remainder() is @byte`<br>`close() is void`<br>`closed() is bool` |
| `CompressionOutput` does `Reading`, `Closing` | `CompressionOutput(origin is @byte)` | `bytes(limit is int) is @byte`<br>`close() is void`<br>`closed() is bool` |
| `DecompressionBudget` | `DecompressionBudget(maximum is int)` | `remaining() is int`<br>`spent(amount is int) is ^DecompressionBudget` |
| `CompressionLimitExceeded` does `Error` | `CompressionLimitExceeded(maximum is int)` | `maximum() is int`<br>`text() is string` |
| `InvalidCompressionLimit` does `Error` | `InvalidCompressionLimit(maximum is int)` | `maximum() is int`<br>`text() is string` |

`output()` transfers finite batches with permanent EOF independently of codec
completion. Batches own their buffers and survive codec close. Decompression
budget is cumulative; draining output does not restore it. Role fakes echo bytes;
no concrete compression format is implemented by this module.

## `compression/read`

[Implementation](../../std/compression/read.ok) · [Contract](compression-guide.md#compressionread)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `CompressedReading` does `Reading`, `Closing` | `CompressedReading(origin is Reading, compressor is Compressing)`<br>`CompressedReading(origin is Reading, compressor is Compressing, chunk_size is int)` | `bytes(limit is int) is @byte`<br>`close() is void`<br>`closed() is bool` |
| `DecompressedReading` does `Reading`, `Closing` | `DecompressedReading(origin is Reading, decompressor is Decompressing)`<br>`DecompressedReading(origin is Reading, decompressor is Decompressing, chunk_size is int)` | `bytes(limit is int) is @byte`<br>`close() is void`<br>`closed() is bool` |
| `CompressionFlow` (internal role) | — | `input(limit is int) is @byte`<br>`write(origin is @byte) is void`<br>`output() is Reading`<br>`finish() is void`<br>`finished() is bool` |
| `CompressingFlow` (internal) does `CompressionFlow` | `CompressingFlow(origin is Reading, compressor is Compressing)` | Implements the flow actions above |
| `DecompressingFlow` (internal) does `CompressionFlow` | `DecompressingFlow(origin is Reading, decompressor is Decompressing)` | Implements the flow actions above |
| `CompressionReading` (internal) does `Reading`, `Closing` | `CompressionReading(flow is CompressionFlow, chunk_size is int)` | `bytes(limit is int) is @byte`<br>`close() is void`<br>`closed() is bool` |

Positive chunk size defaults to 8192; read limits must be positive. Drains finite
codec batches, preserves excess, skips intermediate empty output, finishes at
source EOF and remembers final EOF. Owns only cache/status buffers; source and
codec remain caller-owned on success, close and failure. Errors retain their
nominal identity, and failed readers reject further reads. Internal declarations
are importable because the language has no module-private declarations.

## `digest/blocks` (internal)

[Implementation](../../std/digest/blocks.ok) · [Contract](digest-guide.md#digestblocks-internal-contract)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `DigestCompression64` | — | `block(origin is @byte) is ^DigestCompression64`<br>`bytes() is @byte` |
| `DigestBlocks64` | `DigestBlocks64(compression is DigestCompression64, origin is @byte, little_endian is bool)` | `state(origin is @byte) is ^DigestBlocks64`<br>`bytes() is @byte` |
| `DigestPadding64` | `DigestPadding64(tail is @byte, length is int, little_endian is bool)` | `bytes() is @byte` |
| `DigestInputTooLong` does `Error` | `DigestInputTooLong()` | `text() is string` |
| `InvalidDigestPadding` does `Error` | `InvalidDigestPadding()` | `text() is string` |
| `InvalidDigestBlock` does `Error` | `InvalidDigestBlock()` | `text() is string` |

64-byte compression and 64-bit length padding infrastructure; no algorithm enum.
Constructors bind input lazily, append compacts previous blocks, and observation
finalizes a temporary state. Maximum message length is 2^61-1 bytes.

## `digest/legacy`

[Implementation](../../std/digest/legacy.ok) · [Contract](digest-guide.md#digestlegacy-contract)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Md5` does `Digesting` | `Md5()`<br>`Md5(origin is @byte)` | `state(origin is @byte) is ^Md5`<br>`bytes() is @byte` |
| `Sha1` does `Digesting` | `Sha1()`<br>`Sha1(origin is @byte)` | `state(origin is @byte) is ^Sha1`<br>`bytes() is @byte` |
| `Md5Compression` does `DigestCompression64` (internal) | `Md5Compression()` | `block(origin is @byte) is ^Md5Compression`<br>`bytes() is @byte` |
| `Sha1Compression` does `DigestCompression64` (internal) | `Sha1Compression()` | `block(origin is @byte) is ^Sha1Compression`<br>`bytes() is @byte` |

Legacy interoperability only; not for new security decisions. Results are 16/20
raw bytes, composed with `HexEncoded` for display. Empty constructors compute
lazily; querying never prevents later append. Internal compression bytes are
unfinalized chaining values, not application digests.

## `digest/state`

[Implementation](../../std/digest/state.ok) · [Contract](digest-guide.md#digeststate-contract)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Digesting` | — | `state(origin is @byte) is ^Digesting`<br>`bytes() is @byte` |

Immutable append preserves previous observations; raw output composes with
`HexEncoded`, and text input requires explicit encoding. The test-only fake
is a byte echo, not a cryptographic implementation.

## `encoding/base32`

[Implementation](../../std/encoding/base32.ok) · [Design decision](stdlib-api-audit-v4.md#stdencodingbase32ok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Base32Encoded` | `Base32Encoded(origin is @byte)` | `string() is string` |
| `Base32Decoded` | `Base32Decoded(origin is string)` | `bytes() is @byte` |
| `InvalidBase32` does `Error` | `InvalidBase32(offset is int)` | `offset() is int`<br>`#projected text() is string` |

## `encoding/base64`

[Implementation](../../std/encoding/base64.ok) · [Design decision](stdlib-api-audit-v4.md#stdencodingbase64ok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Base64Alphabet` | — | Language/native contract; no declared actions |
| `Base64Encoded` | `Base64Encoded(origin is @byte)`<br>`Base64Encoded(origin is @byte, alphabet is Base64Alphabet)` | `string() is string` |
| `Base64Decoded` | `Base64Decoded(origin is string)`<br>`Base64Decoded(origin is string, alphabet is Base64Alphabet)` | `bytes() is @byte` |
| `InvalidBase64` does `Error` | `InvalidBase64(offset is int)` | `offset() is int`<br>`#projected text() is string` |

## `encoding/base85`

[Implementation](../../std/encoding/base85.ok) · [Design decision](stdlib-api-audit-v4.md#stdencodingbase85ok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Base85Alphabet` | — | Language/native contract; no declared actions |
| `Base85Encoded` | `Base85Encoded(origin is @byte, alphabet is Base85Alphabet)` | `string() is string` |
| `Base85Decoded` | `Base85Decoded(origin is string, alphabet is Base85Alphabet)` | `bytes() is @byte` |
| `InvalidBase85` does `Error` | `InvalidBase85(offset is int)` | `offset() is int`<br>`#projected text() is string` |

## `encoding/binary`

[Implementation](../../std/encoding/binary.ok) · [Design decision](stdlib-api-audit-v4.md#stdencodingbinaryok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Signedness` | — | Language/native contract; no declared actions |
| `FloatWidth` | — | Language/native contract; no declared actions |
| `IntegerFromBytes` | `IntegerFromBytes(origin is @byte, signedness is Signedness, order is ByteOrder)` | `#projected value() is int` |
| `IntegerBytes` | `IntegerBytes(value is int, size is int, signedness is Signedness, order is ByteOrder)` | `bytes() is @byte` |
| `FloatFromBytes` | `FloatFromBytes(origin is @byte, width is FloatWidth, order is ByteOrder)` | `#projected value() is float` |
| `FloatBytes` | `FloatBytes(value is float, width is FloatWidth, order is ByteOrder)` | `bytes() is @byte` |

## `encoding/hex`

[Implementation](../../std/encoding/hex.ok) · [Design decision](stdlib-api-audit-v4.md#stdencodinghexok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `HexEncoded` | `HexEncoded(origin is @byte)` | `string() is string` |
| `HexDecoded` | `HexDecoded(origin is string)` | `bytes() is @byte` |
| `InvalidHex` does `Error` | `InvalidHex(offset is int)` | `offset() is int`<br>`#projected text() is string` |

## `encoding/pem`

[Implementation](../../std/encoding/pem.ok) · [Design decision](stdlib-api-audit-v4.md#stdencodingpemok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `PemEncoded` | `PemEncoded(kind is string, data is @byte)` | `string() is string` |
| `PemBlock` does `ByteSource` | `PemBlock(origin is string)` | `kind() is string`<br>`bytes() is @byte` |
| `PemBlocks` does `Iterating<PemBlock>` | `PemBlocks(origin is string)` | `cursor() is Cursor<PemBlock>` |
| `PemBlockCursor` does `Cursor<PemBlock>` | `PemBlockCursor(origin is string, position is int, size is int)` | `available() is bool`<br>`item() is PemBlock`<br>`successor() is Cursor<PemBlock>` |
| `InvalidPem` does `Error` | `InvalidPem(offset is int)` | `offset() is int`<br>`#projected text() is string` |

## `encoding/quoted_printable`

[Implementation](../../std/encoding/quoted_printable.ok) · [Design decision](stdlib-api-audit-v4.md#stdencodingquoted_printableok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `QuotedPrintableEncoded` | `QuotedPrintableEncoded(origin is @byte)` | `bytes() is @byte` |
| `QuotedPrintableDecoded` | `QuotedPrintableDecoded(origin is @byte)` | `bytes() is @byte` |
| `InvalidQuotedPrintable` does `Error` | `InvalidQuotedPrintable(offset is int)` | `offset() is int`<br>`#projected text() is string` |

## `encoding/single_byte`

[Implementation](../../std/encoding/single_byte.ok) · [Design decision](stdlib-api-audit-v4.md#stdencodingsingle_byteok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `AsciiEncoded` | `AsciiEncoded(origin is string)` | `bytes() is @byte` |
| `AsciiDecoded` | `AsciiDecoded(origin is @byte)` | `string() is string` |
| `Latin1Encoded` | `Latin1Encoded(origin is string)` | `bytes() is @byte` |
| `Latin1Decoded` | `Latin1Decoded(origin is @byte)` | `string() is string` |
| `UnrepresentableAsciiCharacter` does `Error` | `UnrepresentableAsciiCharacter(offset is int)` | `offset() is int`<br>`#projected text() is string` |
| `UnrepresentableLatin1Character` does `Error` | `UnrepresentableLatin1Character(offset is int)` | `offset() is int`<br>`#projected text() is string` |
| `InvalidAsciiByte` does `Error` | `InvalidAsciiByte(offset is int)` | `offset() is int`<br>`#projected text() is string` |

## `encoding/utf`

[Implementation](../../std/encoding/utf.ok) · [Design decision](stdlib-api-audit-v4.md#stdencodingutfok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `ByteOrder` | — | Language/native contract; no declared actions |
| `Utf8Encoded` | `Utf8Encoded(origin is string)` | `bytes() is @byte` |
| `Utf8Decoded` | `Utf8Decoded(origin is @byte)` | `string() is string` |
| `Utf16Encoded` | `Utf16Encoded(origin is string, order is ByteOrder)` | `bytes() is @byte` |
| `Utf16Decoded` | `Utf16Decoded(origin is @byte, order is ByteOrder)` | `string() is string` |
| `Utf32Encoded` | `Utf32Encoded(origin is string, order is ByteOrder)` | `bytes() is @byte` |
| `Utf32Decoded` | `Utf32Decoded(origin is @byte, order is ByteOrder)` | `string() is string` |
| `InvalidUtf8` does `Error` | `InvalidUtf8(offset is int)` | `offset() is int`<br>`#projected text() is string` |
| `InvalidUtf16` does `Error` | `InvalidUtf16(offset is int)` | `offset() is int`<br>`#projected text() is string` |
| `InvalidUtf32` does `Error` | `InvalidUtf32(offset is int)` | `offset() is int`<br>`#projected text() is string` |

## `error`

[Implementation](../../std/error.ok) · [Design decision](stdlib-api-audit-v4.md#stderrorok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `InvalidArgument` does `Error` | `InvalidArgument()`<br>`InvalidArgument(msg is string)` | `#projected text() is string` |
| `InvalidState` does `Error` | `InvalidState()`<br>`InvalidState(msg is string)` | `#projected text() is string` |
| `OutOfRange` does `Error` | `OutOfRange()`<br>`OutOfRange(msg is string)` | `#projected text() is string` |
| `UnsupportedOperation` does `Error` | `UnsupportedOperation()`<br>`UnsupportedOperation(msg is string)` | `#projected text() is string` |

## `fs/directory`

[Implementation](../../std/fs/directory.ok) · [Design decision](stdlib-api-audit-v4.md#stdfsdirectoryok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `DirectoryLocating` | — | `path() is Path` |
| `Directory` does `DirectoryLocating` | `Directory(path is Path)` | `path() is Path`<br>`existence() is bool`<br>`create() is void`<br>`remove() is void`<br>`move(destination is Path) is void` |
| `DirectoryEntry` | `DirectoryEntry(path is Path)` | `path() is Path`<br>`kind() is FileKind`<br>`stat() is FileStat` |
| `DirectoryEntries` does `Iterating<DirectoryEntry>` | `DirectoryEntries(origin is DirectoryLocating)` | `cursor() is Cursor<DirectoryEntry>` |
| `DirectoryEntryCursor` does `Cursor<DirectoryEntry>` | `DirectoryEntryCursor(parent is Path, names is @string, position is int)` **named only** | `available() is bool`<br>`item() is DirectoryEntry`<br>`successor() is Cursor<DirectoryEntry>` |
| `Descendants` does `Iterating<DirectoryEntry>` | `Descendants(origin is DirectoryLocating)` | `cursor() is Cursor<DirectoryEntry>` |
| `DescendantCursor` does `Cursor<DirectoryEntry>` | `DescendantCursor(current is Cursor<DirectoryEntry>, following is List<Cursor<DirectoryEntry>>)` **named only** | `available() is bool`<br>`item() is DirectoryEntry`<br>`successor() is Cursor<DirectoryEntry>` |
| `DescendantCursorFrom` | `DescendantCursorFrom(current is Cursor<DirectoryEntry>, following is List<Cursor<DirectoryEntry>>)` **named only** | `#projected value() is Cursor<DirectoryEntry>` |
| `ExhaustedDescendantCursor` does `Cursor<DirectoryEntry>` | `ExhaustedDescendantCursor()` | `available() is bool`<br>`item() is DirectoryEntry`<br>`successor() is Cursor<DirectoryEntry>` |
| `DirectoryTree` | `DirectoryTree(origin is DirectoryLocating)` | `remove() is void` |
| `DirectoryNotEmpty` does `Error` | `DirectoryNotEmpty(path is Path)` | `path() is Path`<br>`#projected text() is string` |

## `fs/file`

[Implementation](../../std/fs/file.ok) · [Design decision](stdlib-api-audit-v4.md#stdfsfileok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `FileMode` | — | Language/native contract; no declared actions |
| `FileLocking` | — | `closed() is bool`<br>`close() is void` |
| `File` does `Closing` | `File(mode is FileMode, path is Path)`<br>`File(mode is FileMode, path is Path, encoding is string)`<br>`File(mode is FileMode, path is Path, binary is bool)`<br>`File(mode is FileMode, path is Path, encoding is string, binary is bool)` | `binary() is FileBytes`<br>`text() is FileText`<br>`closed() is bool`<br>`close() is void` |
| `FileBytes` does `Reading`, `Writing` | `FileBytes(origin is File)` | `bytes(limit is int) is @byte`<br>`write(source is ByteSource) is void`<br>`flush() is void` |
| `FileText` does `ReadingText`, `WritingText` | `FileText(origin is File)` | `line() is string`<br>`text() is string`<br>`write(values is ...string) is void`<br>`writeln(values is ...string) is void`<br>`flush() is void` |
| `SeekOrigin` | — | Language/native contract; no declared actions |
| `FilePosition` | `FilePosition(origin is File)` | `offset() is int`<br>`seek(offset is int, from is SeekOrigin) is void`<br>`truncate(size is int) is void` |
| `FileEntry` | `FileEntry(path is Path)` | `path() is Path`<br>`existence() is bool`<br>`stat() is FileStat`<br>`remove() is void`<br>`move(destination is Path) is void` |
| `FileContent` | `FileContent(origin is FileEntry)` | `bytes() is @byte` |
| `ConfiguredFileState` | `ConfiguredFileState(mode is FileMode, path is Path, encoding is string, binary is bool)` **named only** | `#projected value() is FileState` |
| `SupportedFileEncoding` | `SupportedFileEncoding(origin is string)` **named only** | `#projected value() is bool` |
| `FileModeCode` | `FileModeCode(origin is FileMode)` **named only** | `#projected value() is int` |
| `SeekOriginCode` | `SeekOriginCode(origin is SeekOrigin)` **named only** | `#projected value() is int` |
| `FileError` | `FileError(failure is NativeFailure)` **named only** | `#projected value() is Error` |
| `InvalidFileOperation` does `Error` | `InvalidFileOperation(message is string)` | `#projected text() is string` |
| `UnsupportedFileEncoding` does `Error` | `UnsupportedFileEncoding(encoding is string)` | `#projected text() is string` |
| `FileSystemFailure` does `Error` | `FileSystemFailure(code is string, message is string)` | `code() is string`<br>`#projected text() is string` |

## `fs/glob`

[Implementation](../../std/fs/glob.ok) · [Design decision](stdlib-api-audit-v4.md#stdfsglobok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `MatchingPaths` does `Iterating<Path>` | `MatchingPaths(root is DirectoryLocating, pattern is string)` | `cursor() is Cursor<Path>` |
| `MatchingPathCursor` does `Cursor<Path>` | `MatchingPathCursor(paths is @string, position is int)` **named only** | `available() is bool`<br>`item() is Path`<br>`successor() is Cursor<Path>` |
| `InvalidGlobPattern` does `Error` | `InvalidGlobPattern(pattern is string, message is string)` | `pattern() is string`<br>`#projected text() is string` |

## `fs/link`

[Implementation](../../std/fs/link.ok) · [Design decision](stdlib-api-audit-v4.md#stdfslinkok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `SymbolicLink` | `SymbolicLink(path is Path)` | `path() is Path`<br>`existence() is bool`<br>`target() is Path`<br>`create(target is Path) is void`<br>`remove() is void` |
| `HardLink` | `HardLink(source is FileEntry, destination is Path)` **named only** | `create() is void` |
| `NotSymbolicLink` does `Error` | `NotSymbolicLink(path is Path)` | `path() is Path`<br>`#projected text() is string` |
| `InvalidHardLinkSource` does `Error` | `InvalidHardLinkSource(path is Path)` | `path() is Path`<br>`#projected text() is string` |

## `fs/lock`

[Implementation](../../std/fs/lock.ok) · [Design decision](stdlib-api-audit-v4.md#stdfslockok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `FileLockMode` | — | Language/native contract; no declared actions |
| `FileLockWaiting` | — | Language/native contract; no declared actions |
| `FileLock` does `Closing`, `FileLocking` | `FileLock(origin is File, mode is FileLockMode)`<br>`FileLock(origin is File, mode is FileLockMode, waiting is FileLockWaiting)` | `closed() is bool`<br>`close() is void` |
| `FileLockModeCode` | `FileLockModeCode(origin is FileLockMode)` **named only** | `#projected value() is int` |
| `FileLockWaitingCode` | `FileLockWaitingCode(origin is FileLockWaiting)` **named only** | `#projected value() is int` |
| `UnavailableFileLock` does `Error` | `UnavailableFileLock()` | `#projected text() is string` |

## `fs/path`

[Implementation](../../std/fs/path.ok) · [Design decision](stdlib-api-audit-v4.md#stdfspathok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `PathStyle` | — | Language/native contract; no declared actions |
| `Path` does `Comparing<Path>` | `Path(origin is string)`<br>`Path(origin is string, style is PathStyle)`<br>`Path(parts is ...string)` | `parent() is Path`<br>`child(part is string) is Path`<br>`absolute() is bool`<br>`order(other is Path) is Order`<br>`string() is string`<br>`style() is PathStyle` |
| `NativePathStyle` | `NativePathStyle()` | `#projected value() is PathStyle` |
| `PathStyleCode` | `PathStyleCode(origin is PathStyle)` **named only** | `#projected value() is int` |
| `InvalidPath` does `Error` | `InvalidPath(message is string)` | `#projected text() is string` |

## `fs/stat`

[Implementation](../../std/fs/stat.ok) · [Design decision](stdlib-api-audit-v4.md#stdfsstatok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `FileKind` | — | Language/native contract; no declared actions |
| `FileStat` | `FileStat(kind is FileKind, size is int, mode is PermissionMode, times is FileTimes, identity is FileIdentity)` | `kind() is FileKind`<br>`size() is int`<br>`mode() is PermissionMode`<br>`times() is FileTimes`<br>`identity() is FileIdentity` |
| `FileTimes` | `FileTimes(access is int, modification is int, change is int, creation is int)` | `access() is Instant`<br>`modification() is Instant`<br>`change() is Instant`<br>`creation() is Instant` |
| `FileIdentity` | `FileIdentity(device is int, inode is BigInt)` | `same(other is FileIdentity) is bool` |
| `Permissions` | `Permissions(readable is bool, writable is bool, executable is bool)` | `readable() is bool`<br>`writable() is bool`<br>`executable() is bool` |
| `PermissionMode` | `PermissionMode(bits is int)` | `owner() is Permissions`<br>`group() is Permissions`<br>`others() is Permissions`<br>`special() is int` |
| `FileStatFromMap` | `FileStatFromMap(origin is %string:int)` **named only** | `#projected value() is FileStat` |
| `EpochInstant` | `EpochInstant(seconds is int)` **named only** | `#projected value() is Instant` |
| `PermissionBits` | `PermissionBits(bits is int)` **named only** | `#projected value() is Permissions` |
| `FileKindFromCode` | `FileKindFromCode(code is int)` **named only** | `#projected value() is FileKind` |
| `MissingCreationTime` does `Error` | `MissingCreationTime()` | `#projected text() is string` |
| `InvalidPermissionMode` does `Error` | `InvalidPermissionMode(bits is int)` | `#projected text() is string` |

## `fs/temp`

[Implementation](../../std/fs/temp.ok) · [Design decision](stdlib-api-audit-v4.md#stdfstempok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `TemporaryFile` does `Reading`, `Writing`, `Closing` | `TemporaryFile(directory is DirectoryLocating, prefix is string)` | `bytes(limit is int) is @byte`<br>`write(source is ByteSource) is void`<br>`flush() is void`<br>`closed() is bool`<br>`close() is void` |
| `TemporaryFilePath` | `TemporaryFilePath(origin is TemporaryFile)` | `#projected value() is Path` |
| `TemporaryFilePreservation` | `TemporaryFilePreservation(origin is TemporaryFile, destination is Path)` | `destination() is FileEntry`<br>`preserve() is void` |
| `TemporaryDirectory` does `Closing` | `TemporaryDirectory(parent is DirectoryLocating, prefix is string)` | `path() is Path`<br>`closed() is bool`<br>`close() is void` |

## `fs/watch`

[Implementation](../../std/fs/watch.ok) · [Design decision](stdlib-api-audit-v4.md#stdfswatchok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `FileEventKind` | — | Language/native contract; no declared actions |
| `FileEvent` | `FileEvent(path is Path, kind is FileEventKind)`<br>`FileEvent(path is Path, kind is FileEventKind, previous_path is Path)` | `path() is Path`<br>`kind() is FileEventKind`<br>`previous_path() is Path` |
| `FileWatch` does `Closing` | `FileWatch(path is Path)` | `async event() is FileEvent`<br>`closed() is bool`<br>`close() is void` |
| `MissingPreviousPath` does `Error` | `MissingPreviousPath(path is Path)` | `path() is Path`<br>`#projected text() is string` |

## `html/entities`

[Implementation](../../std/html/entities.ok) · [Contract](html-entities-guide.md)

Incremental addition to the generated inventory above: one domain module,
two objects, three constructors and two public actions.

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `EscapedHtml` | `EscapedHtml(origin is string)`<br>`EscapedHtml(origin is string, quotes is bool)` | `string() is string` |
| `UnescapedHtml` | `UnescapedHtml(origin is string)` | `string() is string` |

EscapedHtml defaults to quotes=false; always escapes ampersand and angle brackets,
optionally both quotation marks. UnescapedHtml decodes references once using HTML
text-context recovery and preserves literal markup. Neither sanitizes HTML.

## `html/events`

[Implementation](../../std/html/events.ok) · [Design decision](stdlib-api-audit-v4.md#stdhtmleventsok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `HtmlEventKind` | — | Language/native contract; no declared actions |
| `HtmlAttribute` | `HtmlAttribute(name is string, valued is bool, value is string)` | `name() is string`<br>`valued() is bool`<br>`value() is string` |
| `StartingHtmlTag` | — | `name() is string`<br>`attributes() is Sequencing<HtmlAttribute>`<br>`self_closing() is bool` |
| `EndingHtmlTag` | — | `name() is string` |
| `ContainingHtmlText` | — | `value() is string` |
| `ContainingHtmlComment` | — | `value() is string` |
| `DeclaringHtml` | — | `value() is string` |
| `ProcessingHtmlInstruction` | — | `target() is string`<br>`data() is string` |
| `HandlingHtmlEvent` | — | `handle(event is StartingHtmlTag) is void`<br>`handle(event is EndingHtmlTag) is void`<br>`handle(event is ContainingHtmlText) is void`<br>`handle(event is ContainingHtmlComment) is void`<br>`handle(event is DeclaringHtml) is void`<br>`handle(event is ProcessingHtmlInstruction) is void` |
| `HtmlEvent` | — | `kind() is HtmlEventKind`<br>`dispatch(to is HandlingHtmlEvent) is void` |
| `HtmlStartTag` does `HtmlEvent`, `StartingHtmlTag` | `HtmlStartTag(name is string, attributes is Sequencing<HtmlAttribute>, self_closing is bool)` | `kind() is HtmlEventKind`<br>`name() is string`<br>`attributes() is Sequencing<HtmlAttribute>`<br>`self_closing() is bool`<br>`dispatch(to is HandlingHtmlEvent) is void` |
| `HtmlEndTag` does `HtmlEvent`, `EndingHtmlTag` | `HtmlEndTag(name is string)` | `kind() is HtmlEventKind`<br>`name() is string`<br>`dispatch(to is HandlingHtmlEvent) is void` |
| `HtmlText` does `HtmlEvent`, `ContainingHtmlText` | `HtmlText(value is string)` | `kind() is HtmlEventKind`<br>`value() is string`<br>`dispatch(to is HandlingHtmlEvent) is void` |
| `HtmlComment` does `HtmlEvent`, `ContainingHtmlComment` | `HtmlComment(value is string)` | `kind() is HtmlEventKind`<br>`value() is string`<br>`dispatch(to is HandlingHtmlEvent) is void` |
| `HtmlDeclaration` does `HtmlEvent`, `DeclaringHtml` | `HtmlDeclaration(value is string)` | `kind() is HtmlEventKind`<br>`value() is string`<br>`dispatch(to is HandlingHtmlEvent) is void` |
| `HtmlProcessingInstruction` does `HtmlEvent`, `ProcessingHtmlInstruction` | `HtmlProcessingInstruction(target is string, data is string)` | `kind() is HtmlEventKind`<br>`target() is string`<br>`data() is string`<br>`dispatch(to is HandlingHtmlEvent) is void` |
| `HtmlEvents` does `Iterating<HtmlEvent>` | `HtmlEvents(origin is string)` | `cursor() is Cursor<HtmlEvent>` |
| `HtmlEventCursor` does `Cursor<HtmlEvent>` | `HtmlEventCursor(origin is HtmlEventsState, position is int, amount is int)` | `available() is bool`<br>`item() is HtmlEvent`<br>`successor() is Cursor<HtmlEvent>` |

## `io/buffer`

[Implementation](../../std/io/buffer.ok) · [Design decision](stdlib-api-audit-v4.md#stdiobufferok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `ByteBuffer` | `ByteBuffer()`<br>`ByteBuffer(initial is @byte)` | `reading() is Reading`<br>`writing() is Writing`<br>`content() is BufferContent`<br>`closing() is Closing` |
| `BufferReading` does `Reading` | `BufferReading(state is BufferState)` | `bytes(limit is int) is @byte` |
| `BufferWriting` does `Writing` | `BufferWriting(state is BufferState)` | `write(source is ByteSource) is void`<br>`flush() is void` |
| `BufferClosing` does `Closing` | `BufferClosing(state is BufferState)` | `closed() is bool`<br>`close() is void` |
| `BufferContent` does `Sizing` | `BufferContent(state is BufferState)` | `bytes() is @byte`<br>`size() is int` |
| `ClosedBuffer` does `Error` | `ClosedBuffer()` | `#projected text() is string` |

## `io/decorators`

[Implementation](../../std/io/decorators.ok) · [Design decision](stdlib-api-audit-v4.md#stdiodecoratorsok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `BufferedReading` does `Reading`, `Closing` | `BufferedReading(origin is Reading)`<br>`BufferedReading(origin is Reading, capacity is int)` | `bytes(limit is int) is @byte`<br>`closed() is bool`<br>`close() is void` |
| `LimitedReading` does `Reading` | `LimitedReading(origin is Reading, limit is int)` | `bytes(size is int) is @byte` |
| `ChainedReading` does `Reading` | `ChainedReading(origins is Sequencing<Reading>)` | `bytes(size is int) is @byte` |
| `ByteSnapshot` does `ByteSource` | `ByteSnapshot(origin is @byte)` | `bytes() is @byte` |
| `TeeWriting` does `Writing` | `TeeWriting(destinations is Sequencing<Writing>)` | `write(source is ByteSource) is void`<br>`flush() is void` |
| `Discarding` does `Writing` | `Discarding()` | `write(source is ByteSource) is void`<br>`flush() is void` |
| `ClosedReading` does `Error` | `ClosedReading()` | `#projected text() is string` |
| `InvalidReading` does `Error` | `InvalidReading(message is string)` | `#projected text() is string` |

## `io/stdin`

[Implementation](../../std/io/stdin.ok) · [Design decision](stdlib-api-audit-v4.md#stdiostdinok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Stdin` does `Reading` | `Stdin()` | `bytes(limit is int) is @byte` |
| `Utf8Input` does `ReadingText`, `Closing` | `Utf8Input(origin is Reading)` | `line() is string`<br>`text() is string`<br>`closed() is bool`<br>`close() is void` |
| `EndOfInput` does `Error` | `EndOfInput()` | `#projected text() is string` |
| `ClosedInput` does `Error` | `ClosedInput()` | `#projected text() is string` |

## `io/stdout`

[Implementation](../../std/io/stdout.ok) · [Design decision](stdlib-api-audit-v4.md#stdiostdoutok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Stdout` does `Writing`, `WritingText` | `Stdout()` | `write(source is ByteSource) is void`<br>`flush() is void`<br>`write(values is ...string) is void`<br>`writeln(values is ...string) is void` |
| `Stderr` does `Writing`, `WritingText` | `Stderr()` | `write(source is ByteSource) is void`<br>`flush() is void`<br>`write(values is ...string) is void`<br>`writeln(values is ...string) is void` |

## `io/stream`

[Implementation](../../std/io/stream.ok) · [Design decision](stdlib-api-audit-v4.md#stdiostreamok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `ByteSource` | — | `bytes() is @byte` |
| `Writing` | — | `write(source is ByteSource) is void`<br>`flush() is void` |
| `Reading` | — | `bytes(limit is int) is @byte` |
| `WritingText` | — | `write(values is ...string) is void`<br>`writeln(values is ...string) is void` |
| `ReadingText` | — | `line() is string`<br>`text() is string` |
| `Closing` | — | `closed() is bool`<br>`close() is void` |
| `IncompleteWrite` does `Error` | `IncompleteWrite(written is int)` | `written() is int`<br>`#projected text() is string` |
| `InvalidReadLimit` does `Error` | `InvalidReadLimit(limit is int)` | `limit() is int`<br>`#projected text() is string` |

## `io/text`

[Implementation](../../std/io/text.ok) · [Design decision](stdlib-api-audit-v4.md#stdiotextok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Utf8Output` does `WritingText` | `Utf8Output(origin is Writing)` **named only** | `write(values is ...string) is void`<br>`writeln(values is ...string) is void` |

## `json`

[Implementation](../../std/json.ok) · [Contract](json.md)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `JsonKind` | enum | `null_value`, `boolean_value`, `number_value`, `string_value`, `array_value`, `object_value` |
| `Json` | `Json()`<br>`Json(value is string)`, `int`, `Decimal`, `BigInt`, `float`, `bool`<br>`Json(fields is @JsonField)`<br>`Json(items is @Json)` | `kind() is JsonKind`<br>`pointer() is string`<br>`text() is string`<br>`number() is Decimal`<br>`integer() is BigInt`<br>`int() is int`<br>`boolean() is bool`<br>`size() is int`<br>`at(index is int) is Json`<br>`items() is @Json`<br>`names() is @string`<br>`values() is @Json`<br>`fields() is @JsonField`<br>`has(name is string) is bool`<br>`get(name is string) is Json`<br>`text/number/integer/int/boolean(name is string)`<br>`list(name is string) is @Json`<br>`optional(name is string) is JsonOptional`<br>`string() is string`<br>`pretty(indentation is int) is string` |
| `JsonField` | `JsonField(name is string, value is Json)`; the value may also be `string`, `int`, `Decimal`, `BigInt`, `float` or `bool` | `name() is string`<br>`value() is Json` |
| `ParsedJson` | `ParsedJson(origin is string)`<br>`ParsedJson(origin is string, maximum_depth is int)` | `#projected value() is Json` |
| `JsonOptional` | made by `Json.optional(name)` | `present() is bool`<br>`pointer() is string`<br>`value() is Json`<br>`text/number/integer/int/boolean(fallback)`<br>`items() is @Json`<br>`optional(name is string) is JsonOptional` |
| `JsonPointerToken` | `JsonPointerToken(origin is string)` | `string() is string` (RFC 6901 escaping) |
| `WrongJsonKind` does `Error` | `WrongJsonKind(pointer is string, expected is JsonKind, actual is JsonKind)` | `pointer()`, `expected()`, `actual()`, `text()` |
| `MissingJsonValue` does `Error` | `MissingJsonValue(pointer is string)` | `pointer()`, `text()` |
| `InvalidJsonNumber` does `Error` | `InvalidJsonNumber(pointer is string, number is string, reason is string)` | `pointer()`, `number()`, `reason()`, `text()` |
| `InvalidJson` does `Error` | `InvalidJson(offset is int, line is int, column is int, message is string)` | `offset() is int`<br>`line() is int`<br>`column() is int`<br>`text() is string` |
| `DuplicateJsonMember` does `Error` | `DuplicateJsonMember(name is string)`<br>`DuplicateJsonMember(name, offset, line, column)` | `name()`, `located()`, `offset()`, `line()`, `column()`, `text()` |
| `MissingJsonLocation` does `Error` | `MissingJsonLocation(name is string)` | `name()`, `text()` |
| `InvalidJsonIndentation` does `Error` | `InvalidJsonIndentation(indentation is int)` | `indentation()`, `text()` |

## `math/aggregate`

[Implementation](../../std/math/aggregate.ok) · [Design decision](stdlib-api-audit-v4.md#stdmathaggregateok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Minimum<T>` | `Minimum(values is Iterating<T>)` | `#projected value() is T` |
| `Maximum<T>` | `Maximum(values is Iterating<T>)` | `#projected value() is T` |
| `Sum` | `Sum(values is Iterating<float>)` | `#projected value() is float` |
| `AccurateSum` | `AccurateSum(values is Iterating<float>)` | `#projected value() is float` |
| `AccurateTotal` | `AccurateTotal(sum is float, correction is float)` | `addition(value is float) is AccurateTotal`<br>`value() is float` |
| `Product` | `Product(values is Iterating<float>)` | `#projected value() is float` |
| `SumOfProducts` | `SumOfProducts(left is Iterating<float>, right is Iterating<float>)` | `#projected value() is float` |
| `Hypotenuse` | `Hypotenuse(values is Iterating<float>)` | `#projected value() is float` |
| `Distance` | `Distance(left is Sequencing<float>, right is Sequencing<float>)` | `#projected value() is float` |
| `ScaledSquares` | `ScaledSquares(scale is float, squares is float)` | `addition(value is float) is ScaledSquares`<br>`value() is float` |

## `math/constants`

[Implementation](../../std/math/constants.ok) · [Design decision](stdlib-api-audit-v4.md#stdmathconstantsok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Pi` | `Pi()` | `#projected value() is float` |
| `Tau` | `Tau()` | `#projected value() is float` |
| `Euler` | `Euler()` | `#projected value() is float` |

## `math/decimal`

[Implementation](../../std/math/decimal.ok) · [Design decision](stdlib-api-audit-v4.md#stdmathdecimalok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Decimal` does `Comparing<Decimal>` | `Decimal(origin is string)` | `coefficient() is BigInt`<br>`exponent() is int`<br>`order(other is Decimal) is Order`<br>`string() is string` |
| `InvalidDecimal` does `Error` | `InvalidDecimal(origin is string)` | `text() is string` |

## `math/rounding`

[Implementation](../../std/math/rounding.ok) · [Design decision](stdlib-api-audit-v4.md#stdmathroundingok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Rounding` | — | Language/native contract; no declared actions |

## `net/failure`

[Implementation](../../std/net/failure.ok) · [Design decision](stdlib-api-audit-v4.md#stdnetfailureok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `NetworkOperation` | — | Language/native contract; no declared actions |
| `NetworkFailureKind` | — | Language/native contract; no declared actions |
| `NetworkFailure` does `Error` | `NetworkFailure(operation is NetworkOperation, kind is NetworkFailureKind, message is string)` | `operation() is NetworkOperation`, `kind() is NetworkFailureKind`, `text() is string` |
| `NetworkClosed` does `Error` | `NetworkClosed()` | `text() is string` |
| `NetworkError` | `NetworkError(operation is NetworkOperation, failure is NativeFailure)` | `applicable() is bool`, `value() is Error` |

[Mapping contract](network-failure.md): terminal core.net categories become
NetworkFailure with the explicit attempted operation and exact message. Native
timed_out becomes the existing TimedOut. Foreign domains/would_block/interrupted
are inapplicable; callers preserve them with bare throw in their original catch.
value() on an inapplicable failure throws InvalidArgument. No native symbol/errno
inspection or I/O; catching only NativeFailure leaves cancellation and existing
domain errors unchanged.

## `net/endpoint`

[Implementation](../../std/net/endpoint.ok) · [Contract](ip-endpoint.md)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `IpEndpoint` | `IpEndpoint(address is IpAddress, port is int)`, plus `zone is string` overload | `address() is IpAddress`, `port() is int`, `zone() is string`, `string() is string` |
| `InvalidPort` does `Error` | `InvalidPort(port is int)` | `port() is int`, `text() is string` |
| `InvalidIpv6Zone` does `Error` | `InvalidIpv6Zone(zone is string)` | `text() is string` |

HostName/HostEndpoint and IDNA are verified below; no DNS or interface lookup occurs.

## `net/interface`

[Implementation](../../std/net/interface.ok) · [Contract](network-interface.md)

`NetworkInterface(name is string, index is int, addresses is @IpInterface, up is bool, loopback is bool)`
retains immutable observations: `name() is string`, `index() is int`,
`addresses() is Sequencing<IpInterface>`, `up() is bool`, `loopback() is bool`.
SystemInterfaces remains open; construction receives an explicit snapshot and performs no OS query.

## `net/ip`

[Implementation](../../std/net/ip.ok) · [Design decision](stdlib-api-audit-v4.md#stdnetipok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `IpVersion` | — | Language/native contract; no declared actions |
| `IpProperty` | — | Language/native contract; no declared actions |
| `IpAddress` does `Comparing<IpAddress>` | `IpAddress(origin is string)` | `version() is IpVersion`, `bytes() is @byte`, `string() is string`, `order(other is IpAddress) is Order`, `properties() is Set<IpProperty>` |
| `InvalidIpAddress` does `Error` | `InvalidIpAddress(origin is string)` | `text() is string` |
| `IpNetwork` | `IpNetwork(address is IpAddress, prefix is int)` | `network() is IpAddress`, `broadcast() is IpAddress`, `prefix() is int`, `membership(address is IpAddress) is bool`, `size() is BigInt` |
| `InvalidIpPrefix` does `Error` | `InvalidIpPrefix(prefix is int, version is IpVersion)` | `prefix() is int`, `text() is string` |
| `IpInterface` | `IpInterface(address is IpAddress, prefix is int)` | `address() is IpAddress`, `network() is IpNetwork`, `string() is string` |
| `IpAddresses` does `Iterating<IpAddress>` | `IpAddresses(origin is IpNetwork)` | `cursor() is Cursor<IpAddress>` |
| `IpAddressCursor` (implementation) | `IpAddressCursor(current is IpAddress, limit is IpAddress)` | `available()`, `item()`, `successor()` |
| `ExhaustedIpAddressCursor` (implementation) | `ExhaustedIpAddressCursor()` | typed exhaustion |
| `IpNetworkBytes` (implementation) | `IpNetworkBytes(origin is @byte, prefix is int, upper is bool)` | `bytes() is @byte` |
| `PackedIpText` (implementation) | `PackedIpText(origin is @byte)` | `string() is string` |

[Numeric contract](ip-address.md), [pinned IANA properties](ip-properties.md),
[CIDR contract](ip-network.md), [lazy traversal](ip-addresses.md), [interface value](ip-interface.md).

## `crypto/key` and `crypto/x509/certificate`

[Certificate contract](x509-certificate.md). PublicKey is a noun role exposing
`bytes() is @byte` as complete DER SubjectPublicKeyInfo. Certificate(origin is
@byte) and ParsedCertificate(origin is @byte).value() validate a bounded envelope
and decode with the host provider. Certificate exposes der(), subject(), issuer(),
validity() and public_key(). Exact DER/deterministic facts define identity; no
live host pointer escapes. InvalidCertificate, CertificateLimitExceeded,
CertificateBackendUnavailable and UnsupportedCertificateName distinguish failures.
CertificateName/CertificatePublicKey are implementation adapters, not independent
certificate/key verification APIs. Extension observers, trust and TLS verification
are subsequent work; PrivateKey and independent key parsing remain deferred.

## `crypto/x509` observers and explicit trust roots

[Contract](certificate-observers.md). CertificateNames(Certificate) exposes dns()
as Sequencing<string> and ips() as Sequencing<IpAddress>, preserving SAN spelling
without CN fallback or a trust/matching claim. CertificateChain(Sequencing<Certificate>)
snapshots a nonempty leaf-first untrusted chain and exposes certificates().
TrustStore is a noun role with certificates(); TrustedCertificates(Sequencing<Certificate>)
and PemTrustStore(source is string) implement explicit root snapshots. Empty roots
never select defaults. InvalidTrustStore and TrustStoreLimitExceeded are typed
construction failures. System acquisition and extension metadata are implemented;
explicit TLS-purpose path verification is implemented below.
PemBlocks now returns an immutable source-snapshot cursor, retaining explicit
legacy PemBlockCursor constructor compatibility. There is no global PEM cache.

## `crypto/x509/trust` system roots

[Contract](system-trust-store.md). SystemTrustStore() creates an effect-free
request; result() is TrustStore reads linked OpenSSL CA sources explicitly,
returning a bounded immutable snapshot. Each call reacquires actual roots.
SystemTrustUnavailable(reason) exposes a portable code; limits use
TrustStoreLimitExceeded. No implicit root fallback, auxiliary trust flattening,
peer verification or native OS-store enumeration is claimed.

## `crypto/x509/trust` explicit verification

[Contract](certificate-verification.md). VerifiedCertificateChain(chain,trust,
host is CertificateIdentity,at is Instant,purpose) is an immutable request:
result() returns Certificate, path() returns the selected CertificateChain.
DnsCertificateIdentity(HostName)/IpCertificateIdentity(IpAddress) implement the
identity role with CertificateIdentityKind (dns/ip) and string(). Roots are
snapshotted explicitly; no clock/DNS/system-default acquisition occurs.
CertificateVerificationFailed exposes CertificateFailure; unknown requested
purpose raises UnsupportedCertificatePurpose. First profile supports complete
self-signed-anchor paths and TLS server/client purposes with exact time and
SAN-only matching. Revocation, partial anchors and transport remain separate.

## `crypto/x509` extension metadata

[Contract](certificate-extensions.md). CertificateExtensions(Certificate) exposes
items(), extension(identifier), critical(), constraints(), key_usages() and purposes().
CertificateExtension retains identifier/critical/exact bytes; MissingCertificateExtension
is typed absence. CertificateKeyUsage has the nine standard named bits.
CertificatePurpose(identifier is string) retains exact canonical EKU OIDs,
including unknown purposes. These are bounded metadata observers; signatures,
path/time/purpose/name policy and critical-extension acceptance remain separate.

## `crypto/x509/name`

[Contract](x509-name.md). NameAttribute(identifier is string, value is string)
exposes identifier()/value(). RelativeDistinguishedName(Sequencing<NameAttribute>)
exposes attributes(); DistinguishedName(Sequencing<RelativeDistinguishedName>)
exposes relative_names(). Group membership, repeated OIDs and observed order are
preserved. InvalidDistinguishedName exposes reason()/text(). Wire parsing and
PKIX name comparison remain provider work. [Certificate parsing](x509-certificate.md)
now integrates the original DER groups.

## `crypto/x509/value`

[Contract](x509-values.md). CertificateValidity(starts is Instant, ends is Instant)
exposes starts()/ends(); reversed wall-clock intervals throw InvalidCertificate.
CertificateConstraints(authority is bool[, maximum_depth is int]) exposes
authority()/limited()/maximum_depth(); present depth requires a CA and nonnegative
value. MissingCertificateDepth distinguishes absence from zero. InvalidCertificate
exposes reason()/text(). These metadata values establish no trust or TLS identity.

## `net/dns`

[Implementation](../../std/net/dns.ok), [fixed contract](fixed-resolution.md),
[system worker contract](system-resolution.md).

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `Resolving` role | — | `async addresses(HostEndpoint) is Sequencing<IpEndpoint>`, `async name(IpAddress) is HostName` |
| `HostAddresses` | `HostAddresses(HostName, Sequencing<IpAddress>)` | `host() is HostName`, `addresses() is Sequencing<IpAddress>` |
| `AddressName` | `AddressName(IpAddress, HostName)` | `address() is IpAddress`, `name() is HostName` |
| `FixedResolution` does `Resolving` | `FixedResolution(Sequencing<HostAddresses>[, Sequencing<AddressName>])` | role actions; immutable snapshots, bounded first-occurrence deduplication |
| `SystemResolution` does `Resolving` | `SystemResolution()` | role actions; lazy worker ownership, system order/scopes and typed failures |
| `UnknownHost` does `Error` | `UnknownHost(HostName)` | `host() is HostName`, `text() is string` |
| `UnknownAddress` does `Error` | `UnknownAddress(IpAddress)` | `address() is IpAddress`, `text() is string` |
| `DnsFailure` does `Error` | `DnsFailure(string)` | `text() is string` |
| `ResolutionLimitExceeded` does `Error` | `ResolutionLimitExceeded(string)` | `kind() is string`, `text() is string` |

## `net/stream`

[Implementation](../../std/net/stream.ok) · [Contract](async-stream-contracts.md)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `AsyncReading` role | — | `async bytes(limit is int) is @byte` |
| `AsyncWriting` role | — | `async write(source is ByteSource) is AsyncWriting` |
| `ConnectionEndpoints<T>` | `ConnectionEndpoints(local is T, remote is T)` | `local_endpoint() is T`, `remote_endpoint() is T` |

`StreamReading` and `StreamWriting` implement the async roles and `Closing`;
`StreamConnection` owns both halves and observes `ConnectionEndpoints<IpEndpoint>`.
Their native-state constructors are infrastructure; applications acquire them
through TCP. [Ownership contract](tcp-stream.md). These roles do not satisfy
synchronous Reading/Writing.

## `net/tls`

[TlsClientPolicy](tls-client-policy.md) snapshots explicit roots, modern versions
and opaque ordered ALPN. [Owned client transport](tls-client-transport.md)
consumes managed TCP only on started handshake and publishes only verified peers.

| Declaration | Inputs / observations | Behavior |
| --- | --- | --- |
| TlsClientHandshake | TcpConnection, CertificateIdentity, TlsClientPolicy[, Timing] | async connection() is TlsConnection; DNS/IP verification at post-handshake time |
| TlsConnection does Closing | input() is TlsReading, output() is TlsWriting, details() is TlsDetails | close() immediate abort; async shutdown() orderly close-notify/disposal |
| TlsReading does AsyncReading, Closing | async bytes(limit is int) is @byte | positive limit, one pending read, authenticated EOF; cancellation preserves owner |
| TlsWriting does AsyncWriting, Closing | async write(source is ByteSource) is AsyncWriting | FIFO all-or-failure; active write cancellation aborts owner |
| TlsDetails | endpoints(), peer(), version(), cipher(), protocol() | immutable actual metadata; no ALPN is an empty string |
| TlsFailure does Error | phase() is TlsPhase, reason() is string | fixed portable diagnostic; certificate rejection retains CertificateVerificationFailed |

TlsPhase is handshaking/verifying/reading/writing/closing. Both halves close the
whole TLS owner. There is no disabled verification, implicit roots, server TLS,
client identity, early data, session cache or exposed engine handle. The private
runtime/tls_transport ABI uses TlsState and remains infrastructure. Native-state
constructors do not acquire connections for applications.

## `net/tcp_racing`

[Contract](tcp-racing.md). TcpRacing(Sequencing<IpEndpoint>[, TcpOptions,Duration,
Timing]).connection() returns the first successful owned TcpConnection. Bounds
1..256, stable family interleaving, relative stagger (default 250 ms), failure
acceleration and disposal of pending/ready losers; no DNS lookup or handle equality.
InvalidTcpRace rejects source counts, destination zero and negative delay.

## `net/tcp`

[Implementation](../../std/net/tcp.ok) · [Option contract](tcp-options.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `TcpOptions` | `TcpOptions()`, `TcpOptions(keep_alive is bool, no_delay is bool)` | `keep_alive() is bool`, `no_delay() is bool` |
| `TcpListeningOptions` | `TcpListeningOptions()`, `TcpListeningOptions(address_reuse is bool, dual_stack is bool, backlog is int)` | `address_reuse() is bool`, `dual_stack() is bool`, `backlog() is int` |

| `TcpConnecting` | `TcpConnecting(remote is IpEndpoint[, options is TcpOptions])` | `async connection() is TcpConnection` |
| `TcpListening` | `TcpListening(local is IpEndpoint[, options is TcpListeningOptions[, accepted is TcpOptions]])` | `async acceptance() is TcpAcceptance` |
| `TcpAcceptance` does `Closing` | Acquired through `TcpListening` | `async connection() is TcpConnection`, `local_endpoint() is IpEndpoint`, `closed() is bool`, `close() is void` |
| `TcpConnection` does `Closing` | Acquired through connect/accept | `input() is StreamReading`, `output() is StreamWriting`, `endpoints() is ConnectionEndpoints<IpEndpoint>`, `closed() is bool`, `close() is void` |

[Numeric provider and cancellation contract](tcp-stream.md). Hostname connection
and streamed DNS/strict resolver profile remain open; supplied numeric racing
is implemented separately in net/tcp_racing.

## `net/udp`

[Implementation](../../std/net/udp.ok) · [Datagram contract](udp-datagram.md)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Datagram` does `ByteSource` | `Datagram(source is IpEndpoint, bytes is @byte, truncated is bool)` | `source() is IpEndpoint`, `bytes() is @byte`, `truncated() is bool` |
| `UdpOptions` | `UdpOptions()`, `UdpOptions(address_reuse is bool, broadcast is bool, dual_stack is bool)` | `address_reuse() is bool`, `broadcast() is bool`, `dual_stack() is bool` |
| `UdpBinding` | `UdpBinding(local is IpEndpoint, options is UdpOptions)` | `async socket() is UdpSocket` |
| `UdpSocket` | Acquired through `UdpBinding`; native-state constructor is infrastructure | `async send(target is IpEndpoint, source is ByteSource) is int`, `async datagram(limit is int) is Datagram`, `local_endpoint() is IpEndpoint`, `closed() is bool`, `close() is void` |
| `UdpMembership` role (native composition infrastructure) | — | `closed() is bool`, `close() is void`; allowed lifecycle role for membership access to the owning socket, following FileLocking |
| `UdpConnecting` | `UdpConnecting(local is IpEndpoint, remote is IpEndpoint, options is UdpOptions)` | `async connection() is UdpConnection` |
| `UdpConnection` | Acquired through `UdpConnecting`; state/owner constructor is infrastructure | `async send(source is ByteSource) is int`, `async datagram(limit is int) is Datagram`, `endpoints() is ConnectionEndpoints<IpEndpoint>`, `closed() is bool`, `close() is void` |
| `DatagramTooLarge` does `Error` | `DatagramTooLarge(size is int)` | `size() is int`, `text() is string` |

[Binding contract](udp-binding.md): lazy numeric IPv4/IPv6 acquisition, options
applied before bind, actual OS endpoint and shared idempotent closure. The owner
closes on scope exit. [Receive contract](udp-receive.md): one packet per lazy
Future, bounded prefix, explicit truncation and actual numeric sender, cancellation
removes readiness, close wakes the pending receive. Captured empty payload is a
packet, not EOF. [Send contract](udp-send.md): one complete packet, sampled once,
accepted-byte result (zero for empty), cached Future, protocol size preflight and
typed host size rejection, independent read/write waits and cancellation.
[Connected contract](udp-connection.md): fresh owned socket with a fixed numeric
peer, actual endpoint snapshots, packet-preserving receive and OS-reported errors.
Cancellation during acquisition releases the unpublished socket. Connected UDP
has no byte-stream role; send counts mean OS acceptance, without a handshake.
[Option requests](udp-options.md) remain pure values.
[Payload-size failure](udp-size-failure.md) retains a non-negative byte count;
send enforces the IP ceiling and translates smaller host/path size rejections.

## `net/multicast`

[Implementation](../../std/net/multicast.ok) · [Option contract](multicast-options.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `MulticastOptions` | `MulticastOptions()`, `MulticastOptions(hop_limit is int, loopback is bool)` | `hop_limit() is int`, `loopback() is bool` |
| `MulticastJoining` | `MulticastJoining(socket is UdpSocket, group is IpAddress, interface is NetworkInterface, options is MulticastOptions)` | `membership() is MulticastMembership` |
| `MulticastMembership` does `UdpMembership` | Acquired through `MulticastJoining`; direct acquisition constructor is infrastructure | `group() is IpAddress`, `interface() is NetworkInterface`, `closed() is bool`, `close() is void` |
| `InvalidMulticastGroup` does `Error` | `InvalidMulticastGroup(origin is IpAddress)` | `text() is string` |

Options default to one hop and loopback enabled, with an inclusive hop range of
`0..255`. Invalid limits raise `InvalidArgument`. [Membership contract](multicast-membership.md):
validated pure request, synchronous IPv4/IPv6 any-source acquisition, retained
socket owner, deterministic leave and shared closure. The first successful join
pins the socket's outbound interface/options; conflicts fail before mutation.
Interface name/index is checked against the host; IPv4 primary-address lookup
currently supports Darwin/Linux. A failed rollback or leave closes the socket
to guarantee cleanup. Source-specific multicast is outside this API.

## `net/unix`

[Implementation](../../std/net/unix.ok) · [Address contract](unix-address.md) · [Streams contract](unix-streams.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `UnixAddress` | `UnixAddress(origin is Path)` | `path() is Path`, `string() is string` |
| `UnixEndpoint` | `UnixEndpoint()`, `UnixEndpoint(address is UnixAddress)` | `named() is bool`, `address() is UnixAddress` (unnamed throws InvalidArgument) |
| `UnixPaired` | `UnixPaired()` | `async result() is UnixPair` |
| `UnixPair` | acquisition result | `first() is UnixConnection`, `second() is UnixConnection` |
| `UnixConnecting` | `UnixConnecting(remote is UnixAddress)` | `async connection() is UnixConnection` |
| `UnixListening` | `UnixListening(local is UnixAddress, backlog is int)` | `async acceptance() is UnixAcceptance` |
| `UnixAcceptance` | acquisition result | `async connection() is UnixConnection`, `local_address() is UnixAddress`, `closed() is bool`, `close() is void` |
| `UnixConnection` | acquisition result | `input() is AsyncReading`, `output() is AsyncWriting`, `endpoints() is ConnectionEndpoints<UnixEndpoint>`, `closed() is bool`, `close() is void` |
| `UnixSocketsUnsupported` does `Error` | `UnixSocketsUnsupported()` | `text() is string` |

Named filesystem addresses only: original lexical Path retained, NUL rejected,
UTF-8 byte length checked against host sockaddr_un capacity with a terminating
NUL reserved. Validation performs no filesystem or socket I/O. Invalid paths
raise InvalidArgument; unavailable address representation raises
UnixSocketsUnsupported. Acquisitions resolve relative addresses at startup and
recheck expanded byte limits. Connections own nonblocking streams; retained
halves support independent pending read/write and cached Future outcomes.
Read cancellation retains the owner; native write cancellation closes it.
Listener cleanup checks captured device/inode, with a protected parent directory
required during acquisition/close. Accepted connections outlive the listener.
UnixReading/UnixWriting, UnixEndpointBytes and UnixError are importable
implementation collaborators; native-state constructors are acquisition infrastructure.

## `url/percent`

[Implementation](../../std/url/percent.ok) · [Contract](url-percent.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `UrlComponent` | enum: data, userinfo, path_segment, path, query, fragment | Selects preserved ASCII delimiters; data preserves only unreserved characters |
| `PercentEncoded` | `PercentEncoded(origin is string, component is UrlComponent)` | `string() is string`: UTF-8 bytes, uppercase escapes |
| `PercentDecoded` | `PercentDecoded(origin is string)` | `bytes() is @byte`, `string() is string`: one decoding pass, plus stays literal |
| `InvalidPercentEncoding` does `Error` | `InvalidPercentEncoding(offset is int)` | `offset() is int` (source scalar offset), `text() is string` |

Construction retains inputs; observation converts and validates. Decode bytes
before strict UTF-8 conversion; InvalidUtf8 keeps decoded-byte offsets. The
whole-query mode preserves separators; use data for individual parameter values.
No URL structure parsing, IDNA, form conversion or recursive decoding.

## `url/value` — query/fragment slice

[Contract](url-parameters.md) · [Implementation](../../std/url/value.ok).
Full URL/authority/host parsing and reference resolution remain open; named hosts
depend on HostName/IDNA. This slice does not provide a partial unchecked Url.

| Declaration | Constructors | Behavior |
| --- | --- | --- |
| `UrlQuery` | `UrlQuery()`, `UrlQuery(origin is string)` | `present() is bool`, `string() is string` |
| `UrlFragment` | `UrlFragment()`, `UrlFragment(origin is string)` | `present() is bool`, `string() is string` |
| `InvalidUrl` does `Error` | `InvalidUrl(offset is int, reason is string)` | `offset() is int`, `text() is string` |

Absent and present-empty values differ. Construction validates URI/IRI component
syntax and captures canonical ASCII; existing reserved escapes remain escaped,
unreserved escapes normalize to literals, Unicode becomes UTF-8 escapes. Query
permits IRI private-use characters; fragment does not. No raw controls, spaces or
hash delimiters. Diagnostic offsets refer to the original component's scalars.
UrlComponentText is an importable implementation collaborator.

## `url/query`

[Contract](url-parameters.md) · [Implementation](../../std/url/query.ok).

| Declaration | Constructors | Behavior |
| --- | --- | --- |
| `QueryParameter` | `QueryParameter(name is string)`, `QueryParameter(name is string, value is string)` | `name() is string`, `value() is string`, `assigned() is bool` |
| `QueryParameters` | `QueryParameters(entries is Sequencing<QueryParameter>)`, same plus `maximum_parameters is int` | `entries() is Sequencing<QueryParameter>`, `membership(name is string) is bool`, `first(name is string) is string`, `values(name is string) is Sequencing<string>`, `string() is string` |
| `ParsedQueryParameters` | `ParsedQueryParameters(origin is UrlQuery)`, same plus `maximum_parameters is int, maximum_bytes is int` | `#projected value() is QueryParameters` |
| `MissingQueryParameter` does `Error` | `MissingQueryParameter(name is string)` | `name() is string`, `text() is string` |
| `InvalidQueryEncoding` does `Error` | `InvalidQueryEncoding(offset is int, reason is string)` | `offset() is int`, `text() is string` |
| `QueryLimitExceeded` does `Error` | `QueryLimitExceeded(kind is string)` | `kind() is string`, `text() is string` |

Snapshots observe size once and each item once. Default 10000 parameters, parser
input 1048576 bytes. Retain order, duplicates and assignment; split literal & and
first = before strict UTF-8 percent decoding. Plus remains literal. Errors point
into the canonical ASCII query. Empty input has no entries; nonempty input keeps
empty segments. A singleton empty unassigned parameter has the same wire spelling
as no entries, while their value identities differ. ParameterSnapshot, Parsing,
Parts, Position and Text are importable collaborators (all with Parameter prefix).

## `url/form`

[Contract](url-parameters.md) · [Implementation](../../std/url/form.ok).

| Declaration | Constructors | Behavior |
| --- | --- | --- |
| `FormUrlEncoded` | `FormUrlEncoded(origin is QueryParameters)` | `bytes() is @byte`, `string() is string` |
| `ParsedForm` | `ParsedForm(origin is @byte)`, same plus `maximum_parameters is int, maximum_bytes is int` | `parameters() is QueryParameters` |
| `InvalidFormEncoding` does `Error` | `InvalidFormEncoding(offset is int)` | `offset() is int`, `text() is string` |

UTF-8, space as +, literal plus as %2B, safe ASCII alphanumeric and *-._.
Serialization always assigns values. Parsing skips empty segments and assigns
empty values to bare names. Strict escape/UTF-8 errors report original byte
offsets; bounds and QueryLimitExceeded match query parsing. This is an explicit
form codec, separate from generic URL query decoding.

## `message/header`

[Contract](message-headers.md) · [Implementation](../../std/message/header.ok).

| Declaration | Constructors | Behavior |
| --- | --- | --- |
| `MessageHeaderName` does `Comparing<MessageHeaderName>` | `MessageHeaderName(origin is string)` | `string() is string`, ASCII case-insensitive `order(other is MessageHeaderName) is Order` |
| `MessageHeader` | `MessageHeader(name is MessageHeaderName, value is string)` | `name() is MessageHeaderName`, unfolded `value() is string` |
| `MessageHeaders` | `MessageHeaders(entries is Sequencing<MessageHeader>)`, same plus `maximum_entries is int` | `entries() is Sequencing<MessageHeader>`, `membership(name is MessageHeaderName) is bool`, `first(name is MessageHeaderName) is string`, `values(name is MessageHeaderName) is Sequencing<string>`, `addition(entry is MessageHeader) is MessageHeaders` |
| `RenderedMessageHeaders` | `RenderedMessageHeaders(origin is MessageHeaders)`, same plus `maximum_bytes is int` | `bytes() is @byte` |
| `InvalidMessageHeader` does `Error` | `InvalidMessageHeader(offset is int, reason is string)` | `offset() is int`, `text() is string` |
| `MissingMessageHeader` does `Error` | `MissingMessageHeader(name is MessageHeaderName)` | `name() is MessageHeaderName`, `text() is string` |
| `MessageHeaderLimitExceeded` does `Error` | `MessageHeaderLimitExceeded(kind is string)` | `kind() is string`, `text() is string` |

Names preserve spelling; use order or collection lookup for case-insensitive
matching. ASCII values reject bare newlines and unfold only CRLF followed by WSP.
Snapshots retain duplicates/order; addition preserves the retained limit (1024
by default). Rendering preserves logical whitespace, targets 78 octets and caps
physical lines at 998, with CRLF and no header/body separator. Default output
budget 1048576 bytes. UnfoldedMessageHeader, MessageHeaderSnapshot,
MessageHeaderOutput, MessageHeaderLine and FoldedMessageHeader are implementation
collaborators; field-specific grammar and Unicode mail encoding remain separate.

## `mime/media_type`

[Contract](mime-media-type.md) · [Implementation](../../std/mime/media_type.ok).
The historical roadmap spelling `mime/media-type` is not a valid module identifier.

| Declaration | Constructors | Behavior |
| --- | --- | --- |
| `MimeParameter` | `(name is string, value is string)` | `name()`, `value()` return strings |
| `MimeParameters` | `(entries is Sequencing<MimeParameter>)`, same plus `maximum_parameters is int` | `entries()`, case-insensitive `membership(name)`, `first(name)`, `values(name)`, `string()` |
| `MediaType` | `(origin is string)`, `(origin is string, limits is MediaTypeLimits)`, `(type is string, subtype is string, parameters is MimeParameters)` | `type()`, `subtype()`, `parameters()`, `string()` |
| `MediaTypeLimits` | `(parameters is int, bytes is int)` | `parameters()`, `bytes()` return nonnegative int limits |
| `InvalidMediaType` does `Error` | `(offset is int, reason is string)` | `offset()`, `reason()`, `text()` |
| `MissingMimeParameter` does `Error` | `(name is string)` | `name()`, `text()` |
| `MimeLimitExceeded` does `Error` | `(kind is string)` | `kind()`, `text()` |

Immutable snapshots preserve ordering and plain duplicates. Parsing is eager,
with strict quoted strings and RFC 2231 UTF-8/US-ASCII continuation assembly.
Defaults: 256 parameters and 1 MiB wire input. Extensions supersede fallbacks;
non-ASCII values serialize as extended UTF-8. Long continuation generation,
comments and obsolete wire syntax are outside this initial contract.
MimeScanning and its Mime-prefixed lexer/assembly collaborators are importable
implementation details shared with disposition.

## `mime/disposition`

[Contract](mime-disposition.md) · [Implementation](../../std/mime/disposition.ok).
`ContentDisposition(origin is string)`, `(kind is string, parameters is MimeParameters)`
and `(origin is string, maximum_parameters is int, maximum_bytes is int)` expose
`kind() is string`, `parameters() is MimeParameters`, `string() is string`.
The shared MIME grammar retains extension kinds and decoded filenames as data.
`InvalidContentDisposition(offset is int, reason is string)` exposes `offset()`
and `text()`; `MissingContentDisposition()` exposes `text()`. Both do `Error`.

## `mime/transfer`

[Contract](mime-transfer.md) · [Implementation](../../std/mime/transfer.ok).
`TransferEncoding = (seven_bit, eight_bit, binary, base64, quoted_printable)`.
`TransferEncodedBody(origin is ByteSource, encoding is TransferEncoding)` and
`TransferDecodedBody(origin is ByteSource, encoding is TransferEncoding)` do
`ByteSource` and expose `bytes() is @byte`. Both accept a third nonnegative
`maximum_bytes is int` (default 16 MiB, independently caps input/output).
Each call observes the borrowed source once; construction is lazy and no source
is closed. Codecs preserve bytes without charset or newline conversion.
`InvalidTransferEncoding(offset is int, reason is string)` exposes `offset()`
and `text()`. `TransferLimitExceeded(kind is string)` exposes `kind()` and `text()`.
Both do `Error`. Unknown header-token handling remains a MIME entity concern.

## `mime/word`

[Contract](mime-word.md) · [Implementation](../../std/mime/word.ok).
`EncodedWord(origin is string)`, `(origin is string, charset is string)` and
the same with `maximum_bytes is int` expose `string() is string`. UTF-8 is the
default; US-ASCII is the other supported charset. B output uses complete
Unicode scalars, words at most 75 characters and CRLF SP separators.
`DecodedWords(origin is string)`, the same with `context is EncodedWordContext`,
and the same with a third `maximum_bytes is int` expose `string() is string`.
`EncodedWordContext = (unstructured, forbidden)`: default unstructured input
must already be unfolded; forbidden context preserves input literally.
B/Q decoding suppresses whitespace only between adjacent encoded words.
Both input/output budgets default to 1 MiB. `InvalidEncodedWord(offset,reason)`
does `Error` with `offset()` and `text()`; `EncodedWordLimitExceeded(kind)` does
`Error` with `kind()` and `text()`. Structured phrase/comment recognition is
outside this explicit codec contract.

## `mime/part`

[Contract](mime-part.md) · [Implementation](../../std/mime/part.ok).

| Declaration | Constructors | Behavior |
| --- | --- | --- |
| `MimeBody` role | — | decoded `bytes() is @byte`, `validate(type is MediaType) is void` |
| `BinaryBody` does `MimeBody`, `ByteSource` | `(origin is @byte)` | immutable bytes; rejects multipart metadata |
| `TextBody` does `MimeBody`, `ByteSource` | `(origin is string, charset is string)` | `string()`, `charset()`, exact `bytes()`; validates declared text charset |
| `MultipartBody` does `MimeBody`, `ByteSource` | `(subtype is string, boundary is string, parts is Sequencing<MimePart>)`, same plus `limits is MimeLimits`; `(subtype,boundary,parts,preamble is @byte,epilogue is @byte,limits)` | `subtype()`, `boundary()`, snapshot `parts()`/`cursor()`, `preamble()`/`epilogue()`, `validate(type)`, `bytes()`, explicit `part(headers[,limits[,default_type]])` |
| `MimePart` does `ByteSource` | `(headers is MessageHeaders, body is MimeBody)`, same plus `limits is MimeLimits`, same plus enclosing-context `default_type is MediaType` | `headers()`, decoded `body()`, `default_type()`, `limits()`, serialized `bytes()`; validated immutable `headers(headers,bounds)` and `multipart(body is MultipartBody)` |
| `MimeContent` | `(origin is MimePart)` | `type() is MediaType`, `disposition() is ContentDisposition`, `encoding() is TransferEncoding` |
| `MimeLimits` | `()`, `(header_bytes is int,line_bytes is int,parts is int,depth is int,body_bytes is int)` | corresponding int getters; nonnegative, line_bytes at most 998 |
| `ParsedMime` | `(origin is @byte)`, same plus `limits is MimeLimits` | deferred `part() is MimePart`, `result() is MimeParsedNode` |
| `MimeParsedNode` (parse result) | produced by `ParsedMime.result()` | `part()`, `nodes()`, `decoded()`, ordered `children() is @MimeParsedNode`; leaves have no children |
| `GeneratedBoundary` | `(random is Randomizing)`, same plus `parts is Sequencing<MimePart>` | eagerly selected, stable `string()`; second form checks child snapshots with bounded retries |
| `UnsupportedMimeCharset` does `Error` | `(charset is string)` | `charset()`, `text()` |
| `InvalidMime` does `Error` | `(offset is int,reason is string)` | original-wire `offset()`, `reason()`, `text()` |
| `MimeLimitFailure` does `Error` | `(kind is string)` | `kind()`, `text()` |

Root limits govern rendering/parsing across the full tree. Defaults: 64 KiB
headers per entity, 998-byte header lines, 1000 nodes including root, depth 16,
16 MiB independently for input/rendered wire and cumulative decoded leaves.
Only identity transfer encodings are valid for multipart; child decoding follows
boundary separation. Missing type/encoding use text/plain;charset=US-ASCII and
seven_bit; multipart/digest changes child type to message/rfc822 without inventing
a header. Encapsulated message bodies remain opaque. Unknown headers/order,
preamble/epilogue survive parsing; explicit blank header/body separators and
closing multipart delimiters are required. Supported text codecs: UTF-8,
US-ASCII, ISO-8859-1, UTF-16BE/LE, UTF-32BE/LE. Parser/rendering helper objects are
importable implementation collaborators, not a general-purpose parser namespace.

## `mime/types`

[Contract](mime-types.md) · [Implementation](../../std/mime/types.ok).
`MediaIdentifying` exposes lexical `type(path is Path) is MediaType` and
`extensions(type is MediaType) is Sequencing<string>`. `StandardMediaTypes()`
implements the role with a deterministic common table. `MediaTypeExtension(
extension is string,type is MediaType)` exposes `extension()` and `type()`;
`MappedMediaTypes(entries is Sequencing<MediaTypeExtension>,fallback is MediaIdentifying)`
implements separate caller overrides, optionally with `maximum_entries is int`.
Longest suffix wins inside each layer; earlier identical mappings win; reverse
lookup ignores parameters and omits shadowed/duplicate aliases.

`SystemMediaTypes(fallback is MediaIdentifying)`, the same plus `files is
Sequencing<Path>`, and the same plus `limits is MediaTypeFileLimits` eagerly
snapshot optional configured files and then implement `MediaIdentifying` without
further I/O. Default POSIX file is /etc/mime.types; Windows uses none. Only missing
files are skipped. `MediaTypeFileLimits()` or `(files is int,bytes is int,entries
is int)` expose corresponding getters; defaults are 16/1048576/10000.
`UnknownMediaType(path)` exposes `path()`/`text()`; `InvalidMediaExtension(extension)`
exposes `extension()`/`text()`; `InvalidMediaTypeDatabase(line,reason)` exposes
`line()`/`text()`; `MediaTypeLimitExceeded(kind)` exposes `kind()`/`text()`. All do
`Error`. Identification never opens the target path; its lexical grammar comes
from the new immutable `Path.style()` projection.

## `mail/message`

[Contract](mail-message.md) · [Implementation](../../std/mail/message.ok).

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `MailMessage` does `ByteSource` | `(headers is MessageHeaders, body is MimeBody[, limits is MimeLimits])`, `(part is MimePart)` | `headers()`, `body()`, `part()`, `limits()`, `bytes()`; preserves all fields including Bcc and explicit part structure |
| `ParsedMail` | `(origin is @byte[, limits is MimeLimits])` | `message() is MailMessage`; recursive MIME parse, InvalidMime becomes InvalidMail |
| `DeliveredMail` does `ByteSource` | `(origin is MailMessage)` | `bytes()` removes root Bcc/Resent-Bcc, preserves source/body/bounds |
| `MessageId` | `(origin is string)` | `string()` retains validated modern ASCII identifier spelling; no DNS lookup |
| `MailDate` | `(origin is ZonedDateTime[, offset_known is bool])` | `string()`; eager snapshot, numeric zone, whole seconds/minutes |
| `ParsedMailDate` | `(origin is string[, maximum_bytes is int])` | `value() is ZonedDateTime`, `offset_known() is bool`; strict bounded documented compatibility profile |
| `InvalidMail` does `Error` | `(offset is int, reason is string)` | `offset()`, `reason()`, `text()` |

Default date input limit is 4096; unknown zero-offset provenance is explicit.
MailMessage validates MIME, not complete RFC address/origination semantics.
ReversePath, MailEnvelope, OutgoingMail and GeneratedMessageId remain blocked
on Mailbox/HostName. No SMTP, implicit envelope or credential behavior is added.

## `mail/mailbox`

[Flag contract](mail-flags.md) · [Implementation](../../std/mail/mailbox.ok).
`MailFlagKind = (system,keyword)` and `SystemMailFlag = (answered,flagged,
deleted,seen,draft,recent)`. `MailFlag` exposes kind(), string(), system(),
keyword(), order(other is MailFlag). SystemFlag(SystemMailFlag) and
KeywordFlag(string) implement MailFlag and Comparing<MailFlag>.
Keyword spelling is preserved; protocol comparison ignores ASCII case.
`WrongMailFlagKind(expected,actual)` exposes expected()/actual()/text();
`InvalidMailFlag(offset,reason)` exposes offset()/reason()/text(). Both do Error.
Keywords are bounded to 255 ASCII atom bytes. Wrong variant projections throw;
Recent is a legacy observation, not an assertion of IMAP4rev2 support.
Generic structural equality is not redefined. MailFlags supplies the protocol set.

[Stored snapshot contract](stored-mail.md) · [Maildir contract](maildir.md).

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `MailFlags` does `Set<MailFlag>`, `Sequencing<MailFlag>`, `Iterating<MailFlag>` | `(entries is Sequencing<MailFlag>[,maximum_entries is int])` | Bounded eager snapshot; size/item/first/last/cursor; protocol membership/addition/difference; default cap 256 |
| `StoredMail` | `(identifier is string,message is MailMessage,flags is Set<MailFlag>[,limits is MimeLimits])` | Eager semantic message/flag snapshot; identifier()/message()/flags(); no filesystem effects |
| `InvalidMailbox` does `Error` | `(offset is int,reason is string)` | offset()/reason()/text() |
| `MailboxLimitExceeded` does `Error` | `(kind is string)` | kind()/text() |
| `MaildirLimits` | `()` or `(messages is int,wire_bytes is int,mime is MimeLimits)` | Getters; defaults 1000/67108864/MimeLimits() |
| `MaildirName` | `(origin is string)` | Validated ASCII component; string()/identifier()/current()/flags(); DFRST profile |
| `Maildir` | `(path is Path[,limits is MaildirLimits])` | path()/limits(); messages() eager snapshots; add(message)/remove(identifier) explicit commands |
| `MaildirDelivery` | `(origin is Maildir,message is MailMessage[,random is Randomizing])` | deliver(); default SecureRandom, one body observation, bounded retries, atomic no-replace hard-link publication |
| `ChangedMaildirFlags` | `(origin is Maildir,identifier is string,flags is Set<MailFlag>)` | Eager flag validation; result() locks, prepares StoredMail, atomically renames with canonical DFRST flags |
| `MissingStoredMail` does `Error` | `(identifier is string)` | identifier()/text() |
| `CommittedMaildirFailure` does `Error` | `(identifier is string,code is string,reason is string)` | identifier()/code()/reason()/text(); cleanup failed after a committed mutation |

Maildir requires a trusted local path namespace. All operations use a permanent
`.okkama-maildir.lock`: shared readers, exclusive writers, immediate contention
failure. External non-cooperating writers remain outside this protocol. Listing
is a bulk provider snapshot; message materialization is bounded. Publication
has no EXDEV copy fallback or crash-durability claim. Mbox/MboxSession are provided by `mail/mbox_store` below.

## `mail/mbox`

[mboxrd contract](mbox.md) · [Implementation](../../std/mail/mbox.ok).

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `MboxSeparator` | `(origin is string)` | Eager printable ASCII envelope-line validation, string(); metadata retained without inference |
| `MboxRecord` | `(separator is MboxSeparator,message is MailMessage)` | separator()/message(); borrowed message, no constructor body observation |
| `MboxLimits` | `()` or `(messages is int,wire_bytes is int,mime is MimeLimits)` | Getters; defaults 1000/67108864/MimeLimits() |
| `MboxArchive` does `ByteSource` | `(records is Sequencing<MboxRecord>[,limits is MboxLimits])` | Bounded eager record-reference snapshot; bytes() observes each body once and emits LF mboxrd |
| `ParsedMbox` | `(origin is @byte[,limits is MboxLimits])` | records() returns bounded eager Sequencing<MboxRecord>; CRLF MIME snapshots |
| `InvalidMbox` does `Error` | `(offset is int,reason is string)` | offset()/reason()/text(); framing error coordinates |
| `MboxLimitExceeded` does `Error` | `(kind is string)` | kind()/text(); messages or wire_bytes |

Pure content codec with reversible `^>*From ` quoting and explicit extra empty
record delimiters. Rejects NUL, bare MIME newlines and missing terminators.
Does not guess mbox variants or provide a filesystem session.

## `mail/mbox_store`

[Local session contract](mbox-session.md) · [Implementation](../../std/mail/mbox_store.ok).

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `Mbox` | `(path is Path[,limits is MboxLimits])` | Pure path()/limits(); session() opens an existing regular mboxrd archive under an exclusive sidecar lock |
| `MboxSession` does `Closing` | `(origin is Mbox)` | Eager messages()/records(); add(record is MboxRecord)/remove(identifier is string) return ^MboxSession; closed()/close() |
| `MboxConflict` does `Error` | `()` | text(); current file differs from the version's exact original bytes |
| `CommittedMboxFailure` does `Error` | `(code is string,reason is string)` | code()/reason()/text(); replacement committed but cleanup failed; shared lease closed |

Commands commit immediately through same-directory atomic replacement and return
successors with stable session-local IDs. Explicit envelope metadata is required
for addition; IDs are neither persistent UIDs nor paths. Closing any version
closes their shared lease. Saved snapshots remain usable. Replacement is 0600;
filesystem metadata and crash durability are outside this trusted-local profile.
No Status/X-UID header interpretation or interoperability with other locking
protocols. MboxLease/MboxSnapshot/MboxReplacement are implementation helpers.

## `mail/smtp_reply`

[SMTP wire contract](smtp-wire.md) · [Implementation](../../std/mail/smtp_reply.ok).

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `SmtpReplyLimits` | `()` or `(line_bytes is int,lines is int,wire_bytes is int)` | Nonnegative bounds; getters; defaults 512/100/65536 |
| `SmtpReply` does `ByteSource` | `(code is int,lines is Sequencing<string>[,limits is SmtpReplyLimits])` | Eager bounded snapshot; code()/lines()/text()/bytes(); canonical ASCII CRLF framing |
| `ParsedSmtpReply` | `(origin is @byte[,limits is SmtpReplyLimits])` | reply() requires exactly one complete frame, without trailing bytes |
| `SmtpReplyDecoder` | `([limits is SmtpReplyLimits])` | fed(chunk is @byte) returns ^SmtpReplyDecoder; complete()/consumed()/reply()/finish()/remainder(); exact following-frame remainder |
| `InvalidSmtpReply` does `Error` | `(offset is int,reason is string)` | offset()/reason()/text() |
| `SmtpLimitExceeded` does `Error` | `(kind is string)` | kind()/text(); layer-specific reply/DATA bounds |

No I/O or automatic EOF inference. Empty feed is allowed before completion;
finish() diagnoses truncation. Old successor versions remain independent.

## `mail/smtp_reading`

[Asynchronous reading contract](smtp-reading.md) · [Implementation](../../std/mail/smtp_reading.ok).

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `SmtpReplyReading` | `(origin is AsyncReading)`; `(origin,prefix is @byte)`; `(origin,prefix,limits is SmtpReplyLimits,chunk_bytes is int)` | result() returns one cached Future<SmtpReplyRead>; lazy bounded reads; explicit prefix; default chunk size 4096 |
| `SmtpReplyRead` | `(decoder is SmtpReplyDecoder)` | Requires completed decoder; reply()/remainder()/consumed() are immutable observations |

The stream is borrowed. Empty reads mean EOF/truncation. Source failures retain
their identity; oversized results throw InvalidReading. Within/Before may bound
the operation; started cancellation or failure requires discarding the owning
protocol session. Separate operations on one stream must be sequenced by the
caller. SmtpReplyInput is an implementation helper, not a session API.

## `mail/smtp`

[SMTP wire contract](smtp-wire.md) · [Implementation](../../std/mail/smtp.ok).

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `SmtpFeature` | enum | size/eight_bit_mime/smtp_utf8/pipelining/start_tls/authentication/dsn/enhanced_status_codes/chunking/binary_mime |
| `SmtpExtension` | `(origin is string)` | Validated bounded EHLO keyword/parameters; name() uppercase, parameters()/string() preserve values/spelling |
| `SmtpCapabilities` | `(reply is SmtpReply)` | Requires 250; skips greeting; extensions()/features()/mechanisms()/has_maximum_size()/maximum_size(); unknown extensions retained |
| `InvalidSmtpCapabilities` does `Error` | `(reason is string)` | reason()/text() |
| `MissingSmtpSize` does `Error` | `()` | text(); no explicit numeric SIZE parameter |

Keywords are ASCII-insensitive; duplicate keywords fail. SIZE numbers are BigInt;
explicit zero and absent numbers are distinct. AUTH mechanisms use the SASL name
grammar. These are advertisements, not verified security or active negotiation.

## `mail/smtp_data`

[DATA contract](smtp-wire.md) · [Implementation](../../std/mail/smtp_data.ok).

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `SmtpDataLimits` | `()` or `(message_bytes is int,wire_bytes is int,line_bytes is int)` | Getters; defaults 67108864/134217731/1000 |
| `SmtpBodyMode` | enum | seven_bit/eight_bit_mime |
| `PreparedSmtpData` does `ByteSource` | `(origin is MailMessage,capabilities is SmtpCapabilities[,limits is SmtpDataLimits])` | Eager body-byte snapshot; bytes()/message_size()/body_mode(); Bcc stripping, SIZE/8BITMIME checks, dot transparency and terminator |
| `InvalidSmtpData` does `Error` | `(offset is int,reason is string)` | offset()/reason()/text() |
| `MissingSmtpExtension` does `Error` | `(name is string)` | name()/text() |

No network commands, authentication or envelope inference. Literal content needs
terminal CRLF; top-level Base64 may acquire ignored final CRLF whitespace.
Binary MIME requires a future BDAT path. Message size excludes transparency dots
and the DATA terminator; output bounds include both. Server/session/transport,
SMTPUTF8 envelope negotiation and command deadlines remain DESIGN. Bounded
asynchronous reply reading is available separately in `mail/smtp_reading`.

## `mail/imap_name`

[IMAP4rev1 name contract](imap-names.md) · [Implementation](../../std/mail/imap_name.ok).

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `ImapNameLimits` | `()` or `(scalars is int,wire_bytes is int)` | Nonnegative limits, getters; defaults 4096/32768 |
| `ImapUtf7Encoded` | `(origin is string[,limits is ImapNameLimits])` | Eager bounded Unicode-to-modified-UTF-7 value; string() |
| `ImapUtf7Decoded` | `(origin is string[,limits is ImapNameLimits])` | Strict canonical decoding; string(); no normalization or replacement |
| `ImapMailboxName` | `(origin is string[,limits is ImapNameLimits])` | Rejects ASCII controls; string()/wire_name()/quoted(); always escapes and quotes the legacy wire name |
| `InvalidImapUtf7` does `Error` | `(offset is int,reason is string)` | offset()/reason()/text() |
| `InvalidImapMailboxName` does `Error` | `(offset is int)` | offset()/text() |
| `ImapNameLimitExceeded` does `Error` | `(kind is string)` | kind()/text(): scalars or wire_bytes |

Empty references are representable. The wire bound includes quotes and escapes
for ImapMailboxName. LIST wildcards retain their meaning; no hierarchy or INBOX
normalization is inferred. UTF8=ACCEPT/rev2 negotiation and transport remain
unimplemented. Encoder/parser/run/shift/value/output objects are internal helpers.

## `mail/imap`

[Capability contract](imap-capabilities.md) · [Implementation](../../std/mail/imap.ok).

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `ImapCapabilityLimits` | `()` or `(tokens is int,token_bytes is int,wire_bytes is int)` | Nonnegative bounds/getters; defaults 256/256/65536 |
| `ImapCapability` | `(origin is string[,limits is ImapCapabilityLimits])` | Validated atom/AUTH profile; string() preserves spelling; name() uppercase |
| `ImapCapabilities` does `ByteSource` | `(entries is Sequencing<string>[,limits is ImapCapabilityLimits])` | Eager bounded snapshot; tokens()/names()/features()/authentication()/bytes(); requires rev1 or rev2 |
| `ParsedImapCapabilities` | `(origin is @byte[,limits is ImapCapabilityLimits])` | Eager exact untagged CAPABILITY line parser; capabilities() |
| `ImapFeature` | enum | Explicitly advertised recognized tokens; full list in contract; no inferred negotiation state |
| `InvalidImapCapabilities` does `Error` | `(reason is string)` | reason()/text() |
| `ImapCapabilityLimitExceeded` does `Error` | `(kind is string)` | kind()/text(): tokens, token_bytes or wire_bytes |

Tokens retain unknown names, original spelling and duplicates; sets and AUTH
observations deduplicate. Bounds count every source token and complete line
framing. Fresh snapshots replace old advertisements after TLS/authentication.
No greeting response-code parser, ENABLED state, command completion or session
is implied. Token/upper/snapshot/line/known-feature classes are internal helpers.

## `mail/imap_response`

[Response framing contract](imap-responses.md) · [Implementation](../../std/mail/imap_response.ok).

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `ImapResponseLimits` | `()` or `(line_bytes,literal_bytes,wire_bytes,literals is int)` | Nonnegative bounds/getters; defaults 8192/16777216/33554432/100 |
| `ImapResponseDecoder` | `([limits is ImapResponseLimits])` | fed(@byte) successor; complete()/consumed()/response()/finish()/remainder() |
| `ParsedImapResponse` | `(origin is @byte[,limits is ImapResponseLimits])` | response() requires exactly one full frame |
| `ImapResponse` does `ByteSource` | `(state is ImapResponseProgress)` | Completed immutable snapshot; kind()/name()/tag()/parts()/bytes() |
| `ImapResponsePart` does `ByteSource` | `(kind is ImapResponsePartKind,value is @byte)` | kind()/bytes(); text bytes include CRLF and literal markers |
| `ImapResponseKind` | enum | tagged/untagged/continuation |
| `ImapResponsePartKind` | enum | text/literal/literal8 |
| `InvalidImapResponse` does `Error` | `(offset is int,reason is string)` | offset()/reason()/text() |
| `ImapResponseLimitExceeded` does `Error` | `(kind is string)` | kind()/text(): line_bytes, literal_bytes, wire_bytes, literals, chunk_bytes |
| `MissingImapTag` does `Error` | `()` | text(); non-tagged response has no tag |

ASCII syntax and a documented set of response names only. Literals remain opaque
bytes; ordinary literals reject NUL, literal8 preserves it. Status text braces
are not markers. Multiline response codes, unknown response names, negotiated
UTF-8 syntax, semantic payload validation and command-wide accumulation remain
outside the profile. Progress/header/text/word/line/quote/marker are parser helpers.

## `mail/imap_reading`

[Asynchronous reading contract](imap-reading.md) · [Implementation](../../std/mail/imap_reading.ok).

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| `ImapResponseReading` | `(origin is AsyncReading)`, `(origin,prefix is @byte)`, `(origin,prefix,limits is ImapResponseLimits,chunk_bytes is int)` | result() returns the same lazy Future<ImapResponseRead>; one frame, cached outcome, explicit remainder |
| `ImapResponseRead` | `(decoder is ImapResponseDecoder)` | response()/consumed()/remainder() completed snapshot |

Default chunk size 4096. Requests are capped at the remaining response budget;
literal framing and limits follow `mail/imap_response`. Empty input means EOF,
oversized source results throw InvalidReading, and source/parser errors propagate.
Within/Before cancellation awaits source cleanup. The caller owns the stream and
must discard a session after a started failure/cancellation. No retry, close,
serialization or tagged-command collection is implied. `ImapResponseInput` is
the internal operation implementation.

## IMAP authenticated protocol modules

[Contract, ownership, limits and supported wire profile](imap-protocol.md) ·
[Example](../../examples/std-v4/imap-session.ok).
This is an already-authenticated **IMAP4rev1** layer, not ImapConnecting or TLS/auth.

| Module | Main declarations and results |
| --- | --- |
| `mail/imap_search` | MailMatching.key → ImapSearchKey; AllMail, UnseenMail, SinceMail, AndMail, OrMail, NotMail; ImapSearchLimits |
| `mail/imap_uid` | ImapUid(BigInt), ImapUidValidity(BigInt), ImapSequence(int), ImapUidSet, MailboxUids(name,validity,uids,account); strict scoped checked() |
| `mail/imap_data` | ParsedImapData, ImapValue(atom/quoted/literal/literal8/list), ImapDataLimits; typed scalar/list access |
| `mail/imap_metadata` | ImapMailbox, ImapStatusCode, ImapSearchResult, ImapUidRanges, ImapFlagsValue |
| `mail/imap_command` | Closed named ImapCommand overloads; bytes(tag), literal_bytes; ImapDatum, FlagChange |
| `mail/imap_command_reading` | ImapCommandReading.result → cached Future<ImapCommandRead>; responses/completion/remainder/consumed/require_success; ImapCommandLimits |
| `mail/imap_exchange` | ImapCommandChannel owns transport; request(command).result, aborting close; serialization and internal tags |
| `mail/imap_state` | ImapMailboxState(result,read_only), successor observed; optional recent/first_unseen/uid_next, required uid_validity and message count |
| `mail/imap_message` | ImapFetch partial observation; ImapMessage requires UID; explicit content/flag presence, missing-data errors |
| `mail/imap_session` | ImapSession(channel,capabilities,account); mailboxes/mailbox/examine; SelectedMailbox async state; SearchedMail.uids, FetchedMail.messages, ChangedMailFlags.messages; MailboxEvents next/event and async close |
| `mail/imap_changes` | CopiedMail/MovedMail.uids, AppendedMail.uids, ExpungedMail.identifiers; optional assigned UIDs; create/delete/rename/subscription actions |

Operation objects cache one future and do not retry. UID sets are scoped to account,
mailbox name and UIDVALIDITY. SelectedMailbox.close aborts the owner without
expunging. MailboxEvents.close sends DONE and awaits completion. Unsupported modes,
missing assignments and observations, stale UIDs and server rejections stay explicit.
The linked contract records constructors, finite budgets, failure types, cancellation
and the remaining blocked connector/UTF-8/address scope.

## `mail/pop3`, `mail/pop3_session`

[Protocol/ownership contract](pop3-session.md) · [Example](../../examples/std-v4/pop3-session.ok).

| Declaration | Constructors / behavior |
| --- | --- |
| `PopNumber` / `PopIdentifier` | validated transaction position / opaque UIDL identity; `value()` / `string()` |
| `PopListingLimits` | default 4096 entries / 40 decimal digits, or explicit bounds |
| `PopMessage` | `(number,size,identifiers)`; typed number, BigInt size, `identified()` and throwing `identifier()` |
| `PopCapabilities` | parsed CAPA reply; availability, names, TOP/UIDL feature set |
| `PopMailboxState` | `(reply,limits)`; checked STAT count and exact total size |
| `PopListing` | `(reply,optional UIDL reply,limits)`; checked message snapshots |
| `PopSession` does `Closing` | `(input,output,owner,capabilities,prefix,reply_limits,listing_limits)`; serialized authenticated transport, mailbox/state/messages, async void reset/quit, aborting close |
| `PopMailbox` | `(session)`; async state/messages/bytes/message/delete/preview; retains the session |
| `PreviewedPopMail` | `(mailbox,number,lines)`; one cached lazy `Future<@byte>` from `bytes()` |
| `PopDeleteReset` | `(session)`; async void `reset()` |
| `PopCommand` | closed enum/typed-number infrastructure; two-argument constructors use named arguments; no raw text/credentials |
| `PopWire`, `PopSessionCleanup`, `PopNumbers`, `PopDecimal`, `PopDataLines` | protocol infrastructure, not application connection/authentication APIs |
| typed errors | `InvalidPopData`, `PopListingLimitExceeded`, `MissingPopIdentifier`, `PopCommandRejected`, `PopSessionClosed` |

`quit()` confirms UPDATE only after +OK; a failed/lost completion is not a
rollback guarantee. `close()`/destroy abort without QUIT. Optional UIDL/TOP are
advertisement-gated. Malformed protocol data and started cancellation close the
owned transport; aligned command rejection preserves it except for QUIT.
No automatic connect, plaintext login, TLS assertion or retry is provided.

## `mail/pop3_reply`, `mail/pop3_reading`

[Contract and limits](pop3-responses.md) · [Example](../../examples/std-v4/pop3-replies.ok).

| Declaration | Constructors / behavior |
| --- | --- |
| `PopReplyMode` | single or multiline, selected from the command's expected response |
| `PopReplyLimits` | defaults or (status_bytes,line_bytes,body_bytes,wire_bytes,lines), all nonnegative |
| `PopReplyDecoder` | (mode[,limits]); successor fed(chunk), complete/consumed/reply/finish/remainder |
| `ParsedPopReply` | (bytes,mode[,limits]); reply() requires exactly one complete reply |
| `PopReply` | immutable successful/text/body/lines/multiline observation; no Unicode interpretation of body |
| `PopReplyReading` | (input,mode), (input,mode,prefix), (input,mode,prefix,limits,chunk_bytes); cached lazy result() |
| `PopReplyRead` | (completed decoder); reply/remainder/consumed |

Strict CRLF, uppercase status markers, dot unstuffing, exact remainder, finite
independent bounds and typed EOF/errors. Negative replies end at the status even
in multiline mode. No commands, session, TLS/authentication or deletion effects.

## `net/ip_registry`

Private classification implementation; source data and the numeric flag encoding
are described in [the contract](ip-properties.md). Declarations are importable
infrastructure, not an alternative application address API.

| Declaration | Constructors | Behavior |
| --- | --- | --- |
| `IpClassification` | `IpClassification(origin is @byte)` | `flags() is int` |
| `PackedIpPrefix` | `PackedIpPrefix(origin is @byte, prefix is string, bits is int)` | `matches() is bool` |

## `numbers/bigint`

[Implementation](../../std/numbers/bigint.ok) · [Contract and examples](bigint-guide.md)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `BigInt` does `Comparing<BigInt>` | `BigInt(origin is string)` | `sign() is int`<br>`bits() is int`<br>`order(other is BigInt) is Order`<br>`string() is string` |
| `InvalidBigInt` does `Error` | `InvalidBigInt()` | `#projected text() is string` |
| `BigDifference` | `BigDifference(left is BigInt, right is BigInt)` **named only** | `#projected value() is BigInt` |
| `BigSum` | `BigSum(left is BigInt, right is BigInt)` **named only** | `#projected value() is BigInt` |
| `BigProduct` | `BigProduct(left is BigInt, right is BigInt)` **named only** | `#projected value() is BigInt` |
| `BigQuotient` | `BigQuotient(left is BigInt, right is BigInt)` **named only** | `#projected value() is BigInt` |
| `BigRemainder` | `BigRemainder(left is BigInt, right is BigInt)` **named only** | `#projected value() is BigInt` |
| `BigDivisionByZero` does `Error` | `BigDivisionByZero()` | `#projected text() is string` |
| `BigGcd` | `BigGcd(left is BigInt, right is BigInt)` **named only** | `#projected value() is BigInt` |
| `BigLcm` | `BigLcm(left is BigInt, right is BigInt)` **named only** | `#projected value() is BigInt` |
| `BigPower` | `BigPower(base is BigInt, exponent is int)`<br>`BigPower(base is BigInt, exponent is int, maximum_bits is int)` | `#projected value() is BigInt` |
| `InvalidBigExponent` does `Error` | `InvalidBigExponent(exponent is int)` | `exponent() is int`<br>`#projected text() is string` |
| `InvalidBigPowerLimit` does `Error` | `InvalidBigPowerLimit(maximum_bits is int)` | `maximum_bits() is int`<br>`#projected text() is string` |
| `BigPowerLimitExceeded` does `Error` | `BigPowerLimitExceeded(maximum_bits is int)` | `maximum_bits() is int`<br>`#projected text() is string` |

## `numbers/bigrational`

[Implementation](../../std/numbers/bigrational.ok) · [Contract](bigrational-guide.md)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `BigRational` does `Comparing<BigRational>` | `BigRational(numerator is BigInt, denominator is BigInt)` | `numerator() is BigInt`<br>`denominator() is BigInt`<br>`order(other is BigRational) is Order`<br>`string() is string` |
| `BigRationalSum` | `BigRationalSum(left is BigRational, right is BigRational)` **named only** | `#projected value() is BigRational` |
| `BigRationalDifference` | `BigRationalDifference(left is BigRational, right is BigRational)` **named only** | `#projected value() is BigRational` |
| `BigRationalProduct` | `BigRationalProduct(left is BigRational, right is BigRational)` **named only** | `#projected value() is BigRational` |
| `BigRationalQuotient` | `BigRationalQuotient(left is BigRational, right is BigRational)` **named only** | `#projected value() is BigRational` |
| `ZeroBigRationalDenominator` does `Error` | `ZeroBigRationalDenominator()` | `text() is string` |
| `BigRationalDivisionByZero` does `Error` | `BigRationalDivisionByZero()` | `text() is string` |


## `os/account`

[Implementation](../../std/os/account.ok) · [Design decision](stdlib-api-audit-v4.md#stdosaccountok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `User` | `User()`<br>`User(name is string)` | `name() is string`<br>`identity() is string`<br>`home() is Directory` |
| `Group` | `Group(name is string)` | `name() is string`<br>`identity() is string`<br>`users() is Sequencing<User>` |
| `GroupUsers` does `Sequencing<User>`, `Iterating<User>`, `Sizing` | `GroupUsers(name is string)` | `cursor() is Cursor<User>`<br>`size() is int`<br>`item(index is int) is User`<br>`first() is User`<br>`last() is User` |
| `GroupUserCursor` does `Cursor<User>` | `GroupUserCursor(names is @string, position is int)` | `available() is bool`<br>`item() is User`<br>`successor() is Cursor<User>` |
| `MissingUser` does `Error` | `MissingUser(name is string)` | `name() is string`<br>`#projected text() is string` |
| `MissingGroup` does `Error` | `MissingGroup(name is string)` | `name() is string`<br>`#projected text() is string` |
| `MissingUserHome` does `Error` | `MissingUserHome(name is string)` | `name() is string`<br>`#projected text() is string` |
| `InvalidAccountName` does `Error` | `InvalidAccountName(name is string)` | `#projected text() is string` |
| `UnavailableAccountInformation` does `Error` | `UnavailableAccountInformation(message is string)` | `#projected text() is string` |

## `os/command`

[Implementation](../../std/os/command.ok) · [Design decision](stdlib-api-audit-v4.md#stdoscommandok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Commanding` | — | `program() is Path`<br>`arguments() is @string`<br>`environment() is Environment`<br>`directory() is DirectoryLocating`<br>`string() is string` |
| `Command` does `Commanding` | `Command(program is Path, arguments is @string, environment is Environment, directory is DirectoryLocating)` | `program() is Path`<br>`arguments() is @string`<br>`environment() is Environment`<br>`directory() is DirectoryLocating`<br>`string() is string` |
| `InheritedCommand` does `Commanding` | `InheritedCommand(program is Path, arguments is @string)` | `program() is Path`<br>`arguments() is @string`<br>`environment() is Environment`<br>`directory() is DirectoryLocating`<br>`string() is string` |

## `os/command_capture`

[Implementation](../../std/os/command_capture.ok) · [Design decision](stdlib-api-audit-v4.md#stdoscommand_captureok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `CapturedStream` | — | Language/native contract; no declared actions |
| `CommandCapture` | `CommandCapture(origin is Commanding, limit is int)`<br>`CommandCapture(origin is Commanding, input is @byte, limit is int)` | `async completion() is CompletedCommand` |
| `CompletedCommand` | `CompletedCommand(exit is Exit, output is @byte, errors is @byte)` | `exit() is Exit`<br>`output() is @byte`<br>`errors() is @byte` |
| `InvalidCaptureLimit` does `Error` | `InvalidCaptureLimit(limit is int)` | `limit() is int`<br>`#projected text() is string` |
| `CaptureLimitExceeded` does `Error` | `CaptureLimitExceeded(stream is CapturedStream, limit is int)` | `stream() is CapturedStream`<br>`limit() is int`<br>`#projected text() is string` |

## `os/directory`

[Implementation](../../std/os/directory.ok) · [Design decision](stdlib-api-audit-v4.md#stdosdirectoryok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `WorkingDirectory` does `DirectoryLocating` | `WorkingDirectory()` | `path() is Path` |

## `os/environment`

[Implementation](../../std/os/environment.ok) · [Design decision](stdlib-api-audit-v4.md#stdosenvironmentok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Environment` | `Environment()`<br>`Environment(values is %string:string)` | `membership(name is string) is bool`<br>`variable(name is string) is string`<br>`variables() is %string:string`<br>`addition(name is string, value is string) is Environment`<br>`difference(name is string) is Environment` |
| `HomeDirectory` | `HomeDirectory(environment is Environment)` **named only** | `#projected value() is Directory` |
| `SearchPath` does `Sequencing<Path>`, `Iterating<Path>`, `Sizing` | `SearchPath(environment is Environment)` | `cursor() is Cursor<Path>`<br>`size() is int`<br>`item(index is int) is Path`<br>`first() is Path`<br>`last() is Path` |
| `SearchPathCursor` does `Cursor<Path>` | `SearchPathCursor(paths is @Path, position is int)` **named only** | `available() is bool`<br>`item() is Path`<br>`successor() is Cursor<Path>` |
| `MissingEnvironmentVariable` does `Error` | `MissingEnvironmentVariable(name is string)` | `name() is string`<br>`#projected text() is string` |
| `InvalidEnvironmentVariable` does `Error` | `InvalidEnvironmentVariable(message is string)` | `#projected text() is string` |
| `MissingHomeDirectory` does `Error` | `MissingHomeDirectory()` | `#projected text() is string` |

## `os/pipeline_capture`

[Implementation](../../std/os/pipeline_capture.ok) · [Design decision](stdlib-api-audit-v4.md#stdospipeline_captureok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `PipelineStream` | — | Language/native contract; no declared actions |
| `PipelineCapture` | `PipelineCapture(commands is @Commanding, limit is int)`<br>`PipelineCapture(commands is @Commanding, input is @byte, limit is int)` | `async completion() is CompletedPipeline` |
| `CompletedPipeline` | `CompletedPipeline(exits is @Exit, output is @byte, errors is @@byte)` | `exits() is @Exit`<br>`output() is @byte`<br>`errors() is @@byte` |
| `EmptyPipeline` does `Error` | `EmptyPipeline()` | `#projected text() is string` |
| `InvalidPipelineCaptureLimit` does `Error` | `InvalidPipelineCaptureLimit(limit is int)` | `limit() is int`<br>`#projected text() is string` |
| `PipelineCaptureLimitExceeded` does `Error` | `PipelineCaptureLimitExceeded(command_index is int, stream is PipelineStream, limit is int)` | `command_index() is int`<br>`stream() is PipelineStream`<br>`limit() is int`<br>`#projected text() is string` |

## `os/platform`

[Implementation](../../std/os/platform.ok) · [Design decision](stdlib-api-audit-v4.md#stdosplatformok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `OperatingSystem` | — | Language/native contract; no declared actions |
| `Platform` | `Platform()` | `operating_system() is OperatingSystem`<br>`architecture() is string`<br>`byte_order() is ByteOrder` |
| `Host` | `Host()` | `name() is string`<br>`cpu_count() is int` |
| `UnavailableHostInformation` does `Error` | `UnavailableHostInformation(message is string)` | `#projected text() is string` |

## `os/process`

[Implementation](../../std/os/process.ok) · [Design decision](stdlib-api-audit-v4.md#stdosprocessok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Signal` | — | Language/native contract; no declared actions |
| `ExitKind` | — | Language/native contract; no declared actions |
| `Exit` | `Exit(kind is ExitKind, code is int)` | `kind() is ExitKind`<br>`code() is int`<br>`success() is bool` |
| `Process` | `Process(command is Commanding)` | `id() is int`<br>`running() is bool`<br>`async exit() is Exit`<br>`signal(value is Signal) is void`<br>`close() is void` |
| `ProcessInput` does `Writing`, `Closing` | `ProcessInput(origin is Process)` | `write(source is ByteSource) is void`<br>`flush() is void`<br>`closed() is bool`<br>`close() is void` |
| `ProcessOutput` does `Reading` | `ProcessOutput(origin is Process)` | `bytes(limit is int) is @byte` |
| `ProcessErrors` does `Reading` | `ProcessErrors(origin is Process)` | `bytes(limit is int) is @byte` |
| `MissingExitCode` does `Error` | `MissingExitCode()` | `#projected text() is string` |
| `UnsupportedSignal` does `Error` | `UnsupportedSignal(signal is Signal)` | `signal() is Signal`<br>`#projected text() is string` |
| `ProcessFailure` does `Error` | `ProcessFailure(code is string, message is string)` | `code() is string`<br>`#projected text() is string` |

## `os/storage`

[Implementation](../../std/os/storage.ok) · [Design decision](stdlib-api-audit-v4.md#stdosstorageok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Storage` | `Storage(path is Path)` | `path() is Path`<br>`usage() is StorageUsage` |
| `StorageUsage` | `StorageUsage(capacity is BigInt, available is BigInt)` | `capacity() is BigInt`<br>`used() is BigInt`<br>`available() is BigInt`<br>`fits(amount is BigInt) is bool` |
| `InvalidStorageUsage` does `Error` | `InvalidStorageUsage()` | `#projected text() is string` |
| `StorageFailure` does `Error` | `StorageFailure(code is string, message is string)` | `code() is string`<br>`#projected text() is string` |

## `parallel/map`

[Implementation](../../std/parallel/map.ok) · [Contract](parallel-map.md)

| Declaration | Constructors | Behavior |
| --- | --- | --- |
| `ParallelMapped<T,R>` | `ParallelMapped(origins is Sequencing<T>, action is (T) is R)`; same with `workers is int` | `workers() is int`, `async results() is Sequencing<R>` |
| `InvalidWorkerCount` does `Error` | `InvalidWorkerCount(workers is int)` | `workers() is int`, `text() is string` |
| `ParallelMapRun<T,R>` (implementation) | `ParallelMapRun(origins is Sequencing<T>, action is (T) is R, workers is int)` **named only** | `async run(group is Supervising<R>) is Sequencing<R>`, `pending(group is Supervising<R>, position is int, count is int) is @Future<R>` |
| `ParallelMapProgress<R>` (implementation) | `ParallelMapProgress()` | `position() is int`, `added(value is R) is ^ParallelMapProgress<R>`, `values() is Sequencing<R>` |

Bounded ordered process batches use transferable synchronous typed bound actions.
Failure/cancellation settles after child cleanup; each results() call owns a fresh
run. Embedded execution retains its configured executor.

## `random/distribution`

[Implementation](../../std/random/distribution.ok) · [Design decision](stdlib-api-audit-v4.md#stdrandomdistributionok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Distribution<T>` | — | `sample() is T` |
| `UniformIntegers` does `Distribution<int>` | `UniformIntegers(random is Randomizing, start is int, stop is int)` | `sample() is int` |
| `UniformFloats` does `Distribution<float>` | `UniformFloats(random is Randomizing, start is float, stop is float)` | `sample() is float` |
| `NormalDistribution` does `Distribution<float>` | `NormalDistribution(random is Randomizing, mean is float, deviation is float)` | `sample() is float` |
| `StandardNormal` | `StandardNormal(random is Randomizing)` | `#projected value() is float` |
| `ExponentialDistribution` does `Distribution<float>` | `ExponentialDistribution(random is Randomizing, rate is float)` | `sample() is float` |
| `GammaDistribution` does `Distribution<float>` | `GammaDistribution(random is Randomizing, shape is float, scale is float)` | `sample() is float` |
| `GammaUnit` | `GammaUnit(random is Randomizing, shape is float)` | `#projected value() is float` |
| `GammaCandidate` | `GammaCandidate(random is Randomizing, shape is float)` | `#projected value() is float` |
| `BetaDistribution` does `Distribution<float>` | `BetaDistribution(random is Randomizing, alpha is float, beta is float)` | `sample() is float` |
| `BetaSample` | `BetaSample(random is Randomizing, alpha is float, beta is float)` | `#projected value() is float` |
| `LogNormalDistribution` does `Distribution<float>` | `LogNormalDistribution(random is Randomizing, mean is float, deviation is float)` | `sample() is float` |
| `VonMisesDistribution` does `Distribution<float>` | `VonMisesDistribution(random is Randomizing, mean is float, concentration is float)` | `sample() is float` |
| `VonMisesSample` | `VonMisesSample(random is Randomizing, mean is float, concentration is float)` | `#projected value() is float` |
| `VonMisesCandidate` | `VonMisesCandidate(random is Randomizing, mean is float, ratio is float)` | `#projected value() is float` |
| `ParetoDistribution` does `Distribution<float>` | `ParetoDistribution(random is Randomizing, shape is float, scale is float)` | `sample() is float` |
| `WeibullDistribution` does `Distribution<float>` | `WeibullDistribution(random is Randomizing, shape is float, scale is float)` | `sample() is float` |
| `Choices<T>` does `Distribution<T>` | `Choices(random is Randomizing, values is Sequencing<T>)` | `sample() is T` |
| `WeightedChoices<T>` does `Distribution<T>` | `WeightedChoices(random is Randomizing, values is Sequencing<T>, weights is Sequencing<float>)` | `sample() is T` |
| `WeightedSelection<T>` | `WeightedSelection(values is Sequencing<T>, weights is Sequencing<float>, scaling is float, target is float, position is int, cumulative is float)` | `#projected value() is T` |
| `Samples<T>` does `Distribution<@T>` | `Samples(random is Randomizing, values is Sequencing<T>, count is int)` | `sample() is @T` |
| `IndexSamples` | `IndexSamples(random is Randomizing, positions is List<int>, count is int, selected is List<int>)` | `#projected value() is List<int>` |
| `ListRemoval<T>` | `ListRemoval(origin is List<T>, index is int)` | `item() is T`<br>`remainder() is List<T>` |
| `PermutationSamples<T>` does `Distribution<@T>` | `PermutationSamples(random is Randomizing, values is Sequencing<T>)` | `sample() is @T` |
| `InvalidDistribution` does `Error` | `InvalidDistribution(message is string)` | `#projected text() is string` |

## `random/source`

[Implementation](../../std/random/source.ok) · [Design decision](stdlib-api-audit-v4.md#stdrandomsourceok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Randomizing` | — | `integer(start is int, stop is int) is int`<br>`float() is float`<br>`bytes(size is int) is @byte` |
| `PcgRandom` does `Randomizing` | `PcgRandom(seed is int)` | `integer(start is int, stop is int) is int`<br>`float() is float`<br>`bytes(size is int) is @byte` |
| `SecureRandom` does `Randomizing` | `SecureRandom()` | `integer(start is int, stop is int) is int`<br>`float() is float`<br>`bytes(size is int) is @byte` |
| `SecureInteger` | `SecureInteger(start is int, stop is int)` | `#projected value() is int` |
| `SecureBytes` | `SecureBytes(size is int)` | `bytes() is @byte` |
| `InvalidRandomRange` does `Error` | `InvalidRandomRange(start is int, stop is int)` | `start() is int`<br>`stop() is int`<br>`#projected text() is string` |
| `InvalidRandomSize` does `Error` | `InvalidRandomSize(requested is int, maximum is int)` | `requested() is int`<br>`maximum() is int`<br>`#projected text() is string` |
| `UnavailableRandom` does `Error` | `UnavailableRandom()` | `#projected text() is string` |

## `regex/escape`

[Implementation](../../std/regex/escape.ok) · [Design decision](stdlib-api-audit-v4.md#stdregexescapeok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `EscapedRegex` | `EscapedRegex(origin is string)` | `#projected string() is string` |

## `regex/match`

[Implementation](../../std/regex/match.ok) · [Design decision](stdlib-api-audit-v4.md#stdregexmatchok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `RegexMatching` | — | `text() is string`<br>`range() is RegexRange`<br>`capture(index is int) is RegexCapture`<br>`group(name is string) is RegexCapture`<br>`captures() is @RegexCapture` |
| `RegexMatch` does `RegexMatching` | `RegexMatch(origin is string, state is RegexMatchState)` | `text() is string`<br>`range() is RegexRange`<br>`capture(index is int) is RegexCapture`<br>`group(name is string) is RegexCapture`<br>`captures() is @RegexCapture` |
| `RegexCapture` | `RegexCapture(origin is string, state is RegexMatchState, index is int, name is string)` | `index() is int`<br>`name() is string`<br>`matched() is bool`<br>`text() is string`<br>`range() is RegexRange` |
| `MissingRegexCapture` does `Error` | `MissingRegexCapture(index is int)` | `index() is int`<br>`#projected text() is string` |
| `MissingRegexGroup` does `Error` | `MissingRegexGroup(name is string)` | `name() is string`<br>`#projected text() is string` |
| `UnmatchedRegexCapture` does `Error` | `UnmatchedRegexCapture(index is int)` | `index() is int`<br>`#projected text() is string` |

## `regex/pattern`

[Implementation](../../std/regex/pattern.ok) · [Design decision](stdlib-api-audit-v4.md#stdregexpatternok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `RegexFlag` | — | Language/native contract; no declared actions |
| `RegexLimit` | — | Language/native contract; no declared actions |
| `Regex` | `Regex(expression is string)`<br>`Regex(expression is string, flags is @RegexFlag)` | `expression() is string`<br>`flags() is @RegexFlag`<br>`group_count() is int`<br>`group_names() is %string:int` |
| `InvalidRegex` does `Error` | `InvalidRegex(offset is int, message is string)` | `offset() is int`<br>`#projected text() is string` |
| `RegexLimitExceeded` does `Error` | `RegexLimitExceeded(kind is RegexLimit, limit is int)` | `kind() is RegexLimit`<br>`limit() is int`<br>`#projected text() is string` |
| `RegexQuery` | — | Language/native contract; no declared actions |
| `RegexMatchObservation` | `RegexMatchObservation(origin is string, regex is Regex, range is RegexRange, mode is RegexQuery)` | `found() is bool`<br>`match() is RegexMatching` |
| `RegexRangeOutsideInput` does `Error` | `RegexRangeOutsideInput(range is RegexRange, length is int)` | `range() is RegexRange`<br>`length() is int`<br>`#projected text() is string` |
| `NoRegexMatch` does `Error` | `NoRegexMatch()` | `#projected text() is string` |

## `regex/range`

[Implementation](../../std/regex/range.ok) · [Design decision](stdlib-api-audit-v4.md#stdregexrangeok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `RegexRange` | `RegexRange(start is int, end is int)` | `start() is int`<br>`end() is int`<br>`length() is int` |
| `InvalidRegexRange` does `Error` | `InvalidRegexRange(start is int, end is int)` | `start() is int`<br>`end() is int`<br>`#projected text() is string` |

## `regex/replace`

[Implementation](../../std/regex/replace.ok) · [Design decision](stdlib-api-audit-v4.md#stdregexreplaceok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `ReplacingRegexMatch` | — | `replacement(found is RegexMatching) is string` |
| `LiteralRegexReplacement` does `ReplacingRegexMatch` | `LiteralRegexReplacement(value is string)` | `replacement(found is RegexMatching) is string` |
| `ExpandedRegexReplacement` does `ReplacingRegexMatch` | `ExpandedRegexReplacement(template is string)` | `replacement(found is RegexMatching) is string` |
| `ReplacementToken` | `ReplacementToken(kind is string, text is string)` | `kind() is string`<br>`text() is string` |
| `ReplacementTokens` does `Iterating<ReplacementToken>` | `ReplacementTokens(tokens is @string)` | `cursor() is Cursor<ReplacementToken>` |
| `ReplacementTokenCursor` does `Cursor<ReplacementToken>` | `ReplacementTokenCursor(tokens is @string, position is int)` | `available() is bool`<br>`item() is ReplacementToken`<br>`successor() is Cursor<ReplacementToken>` |
| `RegexSubstitution` | `RegexSubstitution(origin is string, regex is Regex, replacing is ReplacingRegexMatch)`<br>`RegexSubstitution(origin is string, regex is Regex, replacing is ReplacingRegexMatch, limit is int)` | `string() is string`<br>`amount() is int` |
| `RegexSubstitutionProgress` | `RegexSubstitutionProgress(origin is string, rendered is string, offset is int, amount is int)` | `successor(found is RegexMatching, replacing is ReplacingRegexMatch) is RegexSubstitutionProgress`<br>`result() is RegexSubstitutionResult` |
| `RegexSubstitutionResult` | `RegexSubstitutionResult(string is string, amount is int)` | `string() is string`<br>`amount() is int` |
| `InvalidRegexReplacement` does `Error` | `InvalidRegexReplacement(offset is int, message is string)` | `offset() is int`<br>`#projected text() is string` |
| `InvalidRegexReplacementLimit` does `Error` | `InvalidRegexReplacementLimit(limit is int)` | `limit() is int`<br>`#projected text() is string` |

## `regex/search`

[Implementation](../../std/regex/search.ok) · [Design decision](stdlib-api-audit-v4.md#stdregexsearchok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `FirstRegexMatch` does `RegexMatching` | `FirstRegexMatch(origin is string, regex is Regex)`<br>`FirstRegexMatch(origin is string, regex is Regex, range is RegexRange)` | `text() is string`<br>`range() is RegexRange`<br>`capture(index is int) is RegexCapture`<br>`group(name is string) is RegexCapture`<br>`captures() is @RegexCapture` |
| `PrefixRegexMatch` does `RegexMatching` | `PrefixRegexMatch(origin is string, regex is Regex)`<br>`PrefixRegexMatch(origin is string, regex is Regex, range is RegexRange)` | `text() is string`<br>`range() is RegexRange`<br>`capture(index is int) is RegexCapture`<br>`group(name is string) is RegexCapture`<br>`captures() is @RegexCapture` |
| `EntireRegexMatch` does `RegexMatching` | `EntireRegexMatch(origin is string, regex is Regex)`<br>`EntireRegexMatch(origin is string, regex is Regex, range is RegexRange)` | `text() is string`<br>`range() is RegexRange`<br>`capture(index is int) is RegexCapture`<br>`group(name is string) is RegexCapture`<br>`captures() is @RegexCapture` |
| `RegexMatches` does `Iterating<RegexMatching>` | `RegexMatches(origin is string, regex is Regex)`<br>`RegexMatches(origin is string, regex is Regex, range is RegexRange)` | `cursor() is Cursor<RegexMatching>`<br>`found() is bool` |
| `RegexMatchCursor` does `Cursor<RegexMatching>` | `RegexMatchCursor(origin is string, regex is Regex, position is int, end is int, exhausted is bool)` | `available() is bool`<br>`item() is RegexMatching`<br>`successor() is Cursor<RegexMatching>` |

## `regex/split`

[Implementation](../../std/regex/split.ok) · [Design decision](stdlib-api-audit-v4.md#stdregexsplitok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `RegexParts` does `Iterating<string>` | `RegexParts(origin is string, regex is Regex)`<br>`RegexParts(origin is string, regex is Regex, limit is int)` | `cursor() is Cursor<string>` |
| `RegexPartsCursor` does `Cursor<string>` | `RegexPartsCursor(origin is string, separators is Cursor<RegexMatching>, offset is int, remaining is int, final is bool)` | `available() is bool`<br>`item() is string`<br>`successor() is Cursor<string>` |
| `InvalidRegexSplitLimit` does `Error` | `InvalidRegexSplitLimit(limit is int)` | `limit() is int`<br>`#projected text() is string` |

## `size`

[Implementation](../../std/size.ok) · [Design decision](stdlib-api-audit-v4.md#stdsizeok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Sizing` | — | `size() is int` |

## `statistics`

[Implementation](../../std/statistics.ok) · [Design decision](stdlib-api-audit-v4.md#stdstatisticsok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `QuantileMethod` | — | Language/native contract; no declared actions |
| `FloatSample` | `FloatSample(values is Iterating<float>)` | `values() is @float`<br>`size() is int` |
| `Mean` | `Mean(values is Iterating<float>)` | `#projected value() is float` |
| `GeometricMean` | `GeometricMean(values is Iterating<float>)` | `#projected value() is float` |
| `HarmonicMean` | `HarmonicMean(values is Iterating<float>)` | `#projected value() is float` |
| `Median` | `Median(values is Iterating<float>)` | `#projected value() is float` |
| `MedianLow` | `MedianLow(values is Iterating<float>)` | `#projected value() is float` |
| `MedianHigh` | `MedianHigh(values is Iterating<float>)` | `#projected value() is float` |
| `Quantiles` | `Quantiles(values is Iterating<float>, count is int)`<br>`Quantiles(values is Iterating<float>, count is int, method is QuantileMethod)` | `values() is @float` |
| `Mode<T>` | `Mode(values is Iterating<T>)` | `#projected value() is T` |
| `Modes<T>` does `Sequencing<T>`, `Iterating<T>`, `Sizing` | `Modes(values is Iterating<T>)` | `cursor() is Cursor<T>`<br>`size() is int`<br>`item(index is int) is T`<br>`first() is T`<br>`last() is T` |
| `ModesCursor<T>` does `Cursor<T>` | `ModesCursor(values is @T, position is int)` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` |
| `SampleVariance` | `SampleVariance(values is Iterating<float>)` | `#projected value() is float` |
| `PopulationVariance` | `PopulationVariance(values is Iterating<float>)` | `#projected value() is float` |
| `SampleDeviation` | `SampleDeviation(values is Iterating<float>)` | `#projected value() is float` |
| `PopulationDeviation` | `PopulationDeviation(values is Iterating<float>)` | `#projected value() is float` |
| `Covariance` | `Covariance(left is Iterating<float>, right is Iterating<float>)` | `#projected value() is float` |
| `Correlation` | `Correlation(left is Iterating<float>, right is Iterating<float>)` | `#projected value() is float` |
| `FittedLine` | `FittedLine(left is Iterating<float>, right is Iterating<float>)` | `prediction(at is float) is float`<br>`residual(at is float, observed is float) is float`<br>`slope() is float`<br>`intercept() is float` |
| `StatisticsError` does `Error` | `StatisticsError(message is string)` | `#projected text() is string` |

## `strings/format`

[Implementation](../../std/strings/format.ok) · [Design decision](stdlib-api-audit-v4.md#stdstringsformatok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Formatted` | `Formatted(pattern is string, values is @string)` **named only** | `#projected string() is string` |
| `InvalidFormat` does `Error` | `InvalidFormat()` | `#projected text() is string` |

## `strings/join`

[Implementation](../../std/strings/join.ok) · [Contract](strings-join-guide.md)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Joined` | `Joined(separator is string, values is Iterating<string>)`<br>`Joined(separator is string, values is Iterating<string>, maximum_items is int, maximum_length is int)` | `#projected string() is string` |
| `JoinedParts` | `JoinedParts(separator is string, maximum_items is int, maximum_length is int)` | `following(value is string) is ^JoinedParts`<br>`string() is string` |
| `JoinLimit` | — | items, length |
| `InvalidJoinLimits` does `Error` | `InvalidJoinLimits()` | `text() is string` |
| `JoinLimitExceeded` does `Error` | `JoinLimitExceeded(limit is JoinLimit)` | `kind() is JoinLimit`<br>`text() is string` |

JoinedParts is an importable implementation collaborator representing an immutable
prefix; no private state is exposed. Length budgets count Unicode scalars.

## `strings/parse`

[Implementation](../../std/strings/parse.ok) · [Design decision](stdlib-api-audit-v4.md#stdstringsparseok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `ParsedInt` | `ParsedInt(origin is string)`<br>`ParsedInt(origin is string, base is int)` | `#projected value() is int` |
| `ParsedFloat` | `ParsedFloat(origin is string)` | `#projected value() is float` |
| `ParsedRat` | `ParsedRat(origin is string)` | `#projected value() is rat` |
| `ParsedBool` | `ParsedBool(origin is string)` | `#projected value() is bool` |
| `ParsedByte` | `ParsedByte(origin is string)`<br>`ParsedByte(origin is string, base is int)` | `#projected value() is byte` |
| `InvalidInteger` does `Error` | `InvalidInteger(str is string)` | `source() is string`<br>`#projected text() is string` |
| `InvalidFloat` does `Error` | `InvalidFloat(str is string)` | `source() is string`<br>`#projected text() is string` |
| `InvalidRational` does `Error` | `InvalidRational(str is string)` | `source() is string`<br>`#projected text() is string` |
| `InvalidBoolean` does `Error` | `InvalidBoolean(str is string)` | `source() is string`<br>`#projected text() is string` |
| `InvalidByte` does `Error` | `InvalidByte(str is string)` | `source() is string`<br>`#projected text() is string` |

## `text/layout`

[Implementation](../../std/text/layout.ok) · [Design decision](stdlib-api-audit-v4.md#stdtextlayoutok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Indented` | `Indented(origin is string, prefix is string)` | `#projected string() is string` |
| `Dedented` | `Dedented(origin is string)` | `#projected string() is string` |
| `Wrapped` | `Wrapped(origin is string, width is int)` | `#projected string() is string` |
| `Shortened` | `Shortened(origin is string, width is int, placeholder is string)` | `#projected string() is string` |

## `time/clock`

[Implementation](../../std/time/clock.ok) · [Design decision](stdlib-api-audit-v4.md#stdtimeclockok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Instant` does `Comparing<Instant>` | `Instant(origin is Duration)` | `unix_duration() is Duration`<br>`plus(duration is Duration) is Instant`<br>`minus(other is Instant) is Duration`<br>`order(other is Instant) is Order` |
| `MonotonicInstant` does `Comparing<MonotonicInstant>` | `MonotonicInstant(origin is Duration)` | `reading() is Duration`<br>`plus(duration is Duration) is MonotonicInstant`<br>`minus(other is MonotonicInstant) is Duration`<br>`order(other is MonotonicInstant) is Order` |
| `Timing` | — | `instant() is Instant`<br>`monotonic_instant() is MonotonicInstant`<br>`async wait(duration is Duration) is void` |
| `SystemClock` does `Timing` | `SystemClock()` | `instant() is Instant`<br>`monotonic_instant() is MonotonicInstant`<br>`async wait(duration is Duration) is void` |
| `InvalidWait` does `Error` | `InvalidWait(message is string)` | `#projected text() is string` |

## `time/date`

[Implementation](../../std/time/date.ok) · [Design decision](stdlib-api-audit-v4.md#stdtimedateok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Weekday` | — | Language/native contract; no declared actions |
| `Date` does `Comparing<Date>` | `Date(year is int, month is int, day is int)` | `year() is int`<br>`month() is int`<br>`day() is int`<br>`weekday() is Weekday`<br>`order(other is Date) is Order` |
| `ShiftedDate` | `ShiftedDate(origin is Date, days is int)` | `#projected value() is Date` |
| `DaysBetween` | `DaysBetween(left is Date, right is Date)` | `#projected value() is int` |
| `DateOrdinal` | `DateOrdinal(origin is Date)` | `#projected value() is int` |
| `OrdinalDate` | `OrdinalDate(origin is int)` | `#projected value() is Date` |
| `InvalidDate` does `Error` | `InvalidDate(message is string)` | `#projected text() is string` |

## `time/day`

[Implementation](../../std/time/day.ok) · [Design decision](stdlib-api-audit-v4.md#stdtimedayok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `TimeOfDay` does `Comparing<TimeOfDay>` | `TimeOfDay(hour is int, minute is int, second is int, nanosecond is int)` | `hour() is int`<br>`minute() is int`<br>`second() is int`<br>`nanosecond() is int`<br>`order(other is TimeOfDay) is Order` |
| `InvalidTimeOfDay` does `Error` | `InvalidTimeOfDay(message is string)` | `#projected text() is string` |

## `time/duration`

[Implementation](../../std/time/duration.ok) · [Design decision](stdlib-api-audit-v4.md#stdtimedurationok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `DurationUnit` | — | Language/native contract; no declared actions |
| `Duration` does `Comparing<Duration>` | `Duration(value is rat, unit is DurationUnit)` | `seconds() is rat`<br>`nanoseconds() is BigInt`<br>`plus(other is Duration) is Duration`<br>`minus(other is Duration) is Duration`<br>`order(other is Duration) is Order` |
| `ScaledDuration` | `ScaledDuration(origin is Duration, factor is rat)` | `#projected value() is Duration` |
| `DurationString` | `DurationString(origin is Duration)` | `#projected string() is string` |
| `InvalidDuration` does `Error` | `InvalidDuration(message is string)` | `#projected text() is string` |

## `time/format`

[Implementation](../../std/time/format.ok) · [Design decision](stdlib-api-audit-v4.md#stdtimeformatok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `FormattedDate` | `FormattedDate(origin is Date, pattern is string)` | `#projected string() is string` |
| `DateFromFormat` | `DateFromFormat(origin is string, pattern is string)` | `#projected value() is Date` |
| `FormattedLocalDateTime` | `FormattedLocalDateTime(origin is LocalDateTime, pattern is string)` | `#projected string() is string` |
| `LocalDateTimeFromFormat` | `LocalDateTimeFromFormat(origin is string, pattern is string)` | `#projected value() is LocalDateTime` |
| `InvalidTimePattern` does `Error` | `InvalidTimePattern(pattern is string, offset is int)` | `offset() is int`<br>`#projected text() is string` |
| `InvalidFormattedDate` does `Error` | `InvalidFormattedDate(origin is string, offset is int)` | `offset() is int`<br>`#projected text() is string` |
| `InvalidFormattedLocalDateTime` does `Error` | `InvalidFormattedLocalDateTime(origin is string, offset is int)` | `offset() is int`<br>`#projected text() is string` |

## `time/iso`

[Implementation](../../std/time/iso.ok) · [Design decision](stdlib-api-audit-v4.md#stdtimeisook)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `IsoDate` | `IsoDate(origin is Date)` | `#projected string() is string` |
| `DateFromIso` | `DateFromIso(origin is string)` | `#projected value() is Date` |
| `IsoLocalDateTime` | `IsoLocalDateTime(origin is LocalDateTime)` | `#projected string() is string` |
| `LocalDateTimeFromIso` | `LocalDateTimeFromIso(origin is string)` | `#projected value() is LocalDateTime` |
| `IsoFraction` | `IsoFraction(origin is int)` | `#projected string() is string` |
| `InvalidIsoDate` does `Error` | `InvalidIsoDate(origin is string)` | `#projected text() is string` |
| `InvalidIsoLocalDateTime` does `Error` | `InvalidIsoLocalDateTime(origin is string)` | `#projected text() is string` |

## `time/local_datetime`

[Implementation](../../std/time/local_datetime.ok) · [Design decision](stdlib-api-audit-v4.md#stdtimelocal_datetimeok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `LocalDateTime` does `Comparing<LocalDateTime>` | `LocalDateTime(date is Date, time is TimeOfDay)` | `date() is Date`<br>`time() is TimeOfDay`<br>`plus(duration is Duration) is LocalDateTime`<br>`minus(other is LocalDateTime) is Duration`<br>`order(other is LocalDateTime) is Order` |

## `time/rfc3339`

[Implementation](../../std/time/rfc3339.ok) · [Design decision](stdlib-api-audit-v4.md#stdtimerfc3339ok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Rfc3339` | `Rfc3339(origin is Instant, offset is Duration)` | `#projected string() is string` |
| `InstantFromRfc3339` | `InstantFromRfc3339(origin is string)` | `#projected value() is Instant` |
| `InvalidRfc3339` does `Error` | `InvalidRfc3339(origin is string)` | `#projected text() is string` |
| `InvalidRfc3339Offset` does `Error` | `InvalidRfc3339Offset(message is string)` | `#projected text() is string` |

## `time/zone`

[Implementation](../../std/time/zone.ok) · [Design decision](stdlib-api-audit-v4.md#stdtimezoneok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Fold` | — | Language/native contract; no declared actions |
| `TimeZone` | — | `name() is string`<br>`offset(at is Instant) is Duration`<br>`daylight_saving(at is Instant) is bool`<br>`abbreviation(at is Instant) is string`<br>`instant(origin is LocalDateTime, fold is Fold) is Instant` |
| `Zone` does `TimeZone` | `Zone(name is string)` | `name() is string`<br>`offset(at is Instant) is Duration`<br>`daylight_saving(at is Instant) is bool`<br>`abbreviation(at is Instant) is string`<br>`instant(local is LocalDateTime, fold is Fold) is Instant` |
| `FixedZone` does `TimeZone` | `FixedZone(name is string, offset is Duration)` | `name() is string`<br>`offset(at is Instant) is Duration`<br>`daylight_saving(at is Instant) is bool`<br>`abbreviation(at is Instant) is string`<br>`instant(local is LocalDateTime, fold is Fold) is Instant` |
| `ZonedDateTime` | `ZonedDateTime(instant is Instant, zone is TimeZone)` | `instant() is Instant`<br>`zone() is TimeZone`<br>`local_datetime() is LocalDateTime`<br>`offset() is Duration` |
| `ZoneObservation` | `ZoneObservation(origin is ZonedDateTime)` | `daylight_saving() is bool`<br>`abbreviation() is string` |
| `InstantFromLocal` | `InstantFromLocal(origin is LocalDateTime, zone is TimeZone, fold is Fold)` | `#projected value() is Instant` |
| `TimeZoneDatabaseVersion` | `TimeZoneDatabaseVersion()` | `#projected string() is string` |
| `UnknownTimeZone` does `Error` | `UnknownTimeZone(name is string)` | `#projected text() is string` |
| `InvalidTimeZone` does `Error` | `InvalidTimeZone(message is string)` | `#projected text() is string` |
| `MissingLocalTime` does `Error` | `MissingLocalTime(zone is string, local is LocalDateTime)` | `#projected text() is string` |

## `unicode/character`

[Implementation](../../std/unicode/character.ok) · [Design decision](stdlib-api-audit-v4.md#stdunicodecharacterok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `UnicodeCategory` | — | Language/native contract; no declared actions |
| `UnicodeCharacter` | `UnicodeCharacter(origin is char)` | `name() is string`<br>`category() is UnicodeCategory`<br>`combining_class() is int`<br>`numeric_value() is float` |
| `NonNumericCharacter` does `Error` | `NonNumericCharacter(origin is char)` | `#projected text() is string` |

## `unicode/normalization`

[Implementation](../../std/unicode/normalization.ok) · [Design decision](stdlib-api-audit-v4.md#stdunicodenormalizationok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `NormalizationForm` | — | Language/native contract; no declared actions |
| `Normalized` | `Normalized(origin is string, form is NormalizationForm)` | `#projected string() is string` |

## `uuid/generation`

[Implementation](../../std/uuid/generation.ok) · [Design decision](stdlib-api-audit-v4.md#stduuidgenerationok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `Uuid3` | `Uuid3(namespace is Uuid, name is string)` | `#projected value() is Uuid` |
| `Uuid5` | `Uuid5(namespace is Uuid, name is string)` | `#projected value() is Uuid` |
| `Uuid8` | `Uuid8(high is int, middle is int, low is int)` | `#projected value() is Uuid` |
| `Uuid4s` does `Distribution<Uuid>` | `Uuid4s(random is SecureRandom)` | `sample() is Uuid` |

## `uuid/value`

[Implementation](../../std/uuid/value.ok) · [Design decision](stdlib-api-audit-v4.md#stduuidvalueok)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `UuidVariant` | — | Language/native contract; no declared actions |
| `Uuid` does `Comparing<Uuid>` | `Uuid(origin is string)` | `version() is int`<br>`variant() is UuidVariant`<br>`order(other is Uuid) is Order`<br>`string() is string`<br>`bytes() is @byte` |
| `UuidFromBytes` | `UuidFromBytes(origin is @byte)` | `#projected value() is Uuid` |
| `InvalidUuid` does `Error` | `InvalidUuid(message is string)` | `#projected text() is string` |


## Native substrate signatures

Low-level backend contract, not the application object model. New primitives in this refactor are bigint_difference, file_state_read_text and json_parse_has_duplicate. Their consumers are BigDifference/StorageUsage, FileText and ParsedJson. BigInt completion additionally implements bigint_sum, bigint_product, bigint_quotient, bigint_remainder, bigint_gcd, bigint_lcm and bounded bigint_power; see the BigInt guide.

### `runtime/account`

- `account_current_user() is %string:string`
- `account_user_named(name is string) is %string:string`
- `account_group_named(name is string) is %string:string`
- `account_group_users(name is string) is @string`

### `runtime/base32`

- `base32_encode(origin is @byte) is string`
- `base32_invalid_offset(origin is string) is int`
- `base32_decode(origin is string) is @byte`

### `runtime/base64`

- `base64_encode(origin is @byte, alphabet is string) is string`
- `base64_invalid_offset(origin is string, alphabet is string) is int`
- `base64_decode(origin is string, alphabet is string) is @byte`

### `runtime/base85`

- `base85_encode(origin is @byte, alphabet is string) is string`
- `base85_invalid_offset(origin is string, alphabet is string) is int`
- `base85_decode(origin is string, alphabet is string) is @byte`

### `runtime/bigint`

- `bigint_valid(origin is string) is bool`
- `bigint_canonical(origin is string) is string`
- `bigint_sign(origin is string) is int`
- `bigint_bits(origin is string) is int`
- `bigint_difference(left is string, right is string) is string`
- `bigint_order(left is string, right is string) is int`
- `bigint_sum(left is string, right is string) is string`
- `bigint_product(left is string, right is string) is string`
- `bigint_quotient(left is string, right is string) is string`
- `bigint_remainder(left is string, right is string) is string`
- `bigint_gcd(left is string, right is string) is string`
- `bigint_lcm(left is string, right is string) is string`
- `bigint_power(base is string, exponent is int, maximum_bits is int) is string`

### `runtime/binary`

- `binary_integer_fits(value is int, size is int, signedness is string) is bool`
- `binary_integer_bytes(value is int, size is int, signedness is string, order is string) is @byte`
- `binary_integer_from_bytes_fits(origin is @byte, signedness is string, order is string) is bool`
- `binary_integer_from_bytes(origin is @byte, signedness is string, order is string) is int`
- `binary_float_fits(value is float, width is int) is bool`
- `binary_float_bytes(value is float, width is int, order is string) is @byte`
- `binary_float_finite(origin is @byte, width is int, order is string) is bool`
- `binary_float_from_bytes(origin is @byte, width is int, order is string) is float`

`binary_masked_slice(origin is @byte,mask is @byte,offset is int,start is int,size is int) is @byte`
returns a new exact repeated-key XOR slice. The key has four bytes; offset is
0..3 at origin[0], and the slice starts with phase (offset+start)%4. Size is
0..65536; the complete range must lie inside origin. Invalid coordinates return
NativeFailure(core.binary,invalid_input). Inputs are unchanged; large consumers
must explicitly chunk their work. No environment, I/O or key generation occurs.

### `runtime/buffer`

- Opaque `BufferState`
- `buffer_state_create(initial is @byte) is BufferState`
- `buffer_state_read(state is BufferState, limit is int) is @byte`
- `buffer_state_append(state is BufferState, value is @byte) is int`
- `buffer_state_bytes(state is BufferState) is @byte`
- `buffer_state_size(state is BufferState) is int`
- `buffer_state_closed(state is BufferState) is bool`
- `buffer_state_close(state is BufferState) is bool`

### `runtime/decimal`

- `decimal_valid(origin is string) is bool`
- `decimal_canonical(origin is string) is string`
- `decimal_coefficient(origin is string) is string`
- `decimal_exponent(origin is string) is int`
- `decimal_order(left is string, right is string) is int`

### `runtime/environment`

- `environment_snapshot() is %string:string`
- `environment_normalize(values is %string:string) is %string:string`
- `environment_add(values is %string:string, name is string, value is string) is %string:string`
- `environment_diff(values is %string:string, name is string) is %string:string`
- `environment_name(name is string) is string`
- `environment_home(values is %string:string) is string`
- `environment_path_separator() is string`
- `environment_get(name is @byte) is @byte`
- `environment_set(name is @byte, value is @byte) is bool`
- `environment_remove(name is @byte) is bool`
- `environment_all() is @@@byte`

### `runtime/fs`

- `path_join(parts is @string, style is int) is string`
- `path_parent(origin is string, style is int) is string`
- `path_child(origin is string, part is string, style is int) is string`
- `path_absolute(origin is string, style is int) is bool`
- Opaque `FileHandle`
- Opaque `DirectoryHandle`
- Opaque `FileState`
- Opaque `FileLockState`
- Opaque `TemporaryDirectoryState`
- Opaque `FileWatchState`
- `path_encode(path is string) is @byte`
- `path_decode(path is @byte) is string`
- `open(path is @byte, flags is int, mode is int) is FileHandle`
- `read(handle is FileHandle, size is int) is @byte`
- `write(handle is FileHandle, bytes is @byte) is int`
- `seek(handle is FileHandle, offset is int, origin is int) is int`
- `flush(handle is FileHandle) is bool`
- `truncate(handle is FileHandle, size is int) is bool`
- `sync(handle is FileHandle) is bool`
- `lock_shared(handle is FileHandle) is bool`
- `lock_exclusive(handle is FileHandle) is bool`
- `unlock(handle is FileHandle) is bool`
- `close(handle is FileHandle) is bool`
- `file_state_create(path is @byte, mode is int, encoding is string, binary is bool) is FileState`
- `file_state_read_text(state is FileState) is string`
- `file_state_read_line(state is FileState) is string`
- `file_state_read(state is FileState, limit is int) is @byte`
- `file_state_write(state is FileState, value is @byte) is int`
- `file_state_write_text(state is FileState, values is @string, newline is bool) is int`
- `file_state_flush(state is FileState) is bool`
- `file_state_close(state is FileState) is bool`
- `file_state_position(state is FileState) is int`
- `file_state_seek(state is FileState, offset is int, origin is int) is int`
- `file_state_truncate(state is FileState, size is int) is bool`
- `file_state_lock(state is FileState, mode is int, waiting is int) is FileLockState`
- `file_lock_closed(state is FileLockState) is bool`
- `file_lock_close(state is FileLockState) is bool`
- `file_content(path is @byte) is @byte`
- `directory_entries(path is @byte) is @string`
- `directory_remove_tree(path is @byte) is bool`
- `matching_paths(root is @byte, pattern is string) is @string`
- `file_watch_create(path is @byte) is FileWatchState`
- `async file_watch_event(state is FileWatchState) is %string:string`
- `file_watch_closed(state is FileWatchState) is bool`
- `file_watch_close(state is FileWatchState) is bool`
- `temporary_file_create(directory is @byte, prefix is string) is FileState`
- `file_state_closed(state is FileState) is bool`
- `temporary_file_path(state is FileState) is @byte`
- `temporary_file_preserve(state is FileState, destination is @byte) is bool`
- `temporary_directory_create(directory is @byte, prefix is string) is TemporaryDirectoryState`
- `temporary_directory_path(state is TemporaryDirectoryState) is @byte`
- `temporary_directory_closed(state is TemporaryDirectoryState) is bool`
- `temporary_directory_close(state is TemporaryDirectoryState) is bool`
- `stat(path is @byte) is %string:int`
- `lstat(path is @byte) is %string:int`
- `mkdir(path is @byte, mode is int) is bool`
- `rmdir(path is @byte) is bool`
- `unlink(path is @byte) is bool`
- `rename(source is @byte, target is @byte) is bool`
- `link(source is @byte, target is @byte) is bool`
- `symlink(target is @byte, link_path is @byte) is bool`
- `readlink(path is @byte) is @byte`
- `cwd() is @byte`
- `chdir(path is @byte) is bool`
- `chmod(path is @byte, mode is int) is bool`
- `chown(path is @byte, owner is int, group is int) is bool`
- `opendir(path is @byte) is DirectoryHandle`
- `readdir(handle is DirectoryHandle) is @byte`
- `rewinddir(handle is DirectoryHandle) is bool`
- `closedir(handle is DirectoryHandle) is bool`

### `runtime/hex`

- `hex_encode(origin is @byte) is string`
- `hex_valid_prefix(origin is string) is int`
- `hex_decode(origin is string) is @byte`

### `runtime/html`

- Opaque `HtmlEventsState`
- `html_events(origin is string) is HtmlEventsState`
- `html_event_count(origin is HtmlEventsState) is int`
- `html_event_kind(origin is HtmlEventsState, index is int) is int`
- `html_event_name(origin is HtmlEventsState, index is int) is string`
- `html_event_value(origin is HtmlEventsState, index is int) is string`
- `html_event_target(origin is HtmlEventsState, index is int) is string`
- `html_event_data(origin is HtmlEventsState, index is int) is string`
- `html_event_self_closing(origin is HtmlEventsState, index is int) is bool`
- `html_attribute_count(origin is HtmlEventsState, index is int) is int`
- `html_attribute_name(origin is HtmlEventsState, event is int, attribute is int) is string`
- `html_attribute_valued(origin is HtmlEventsState, event is int, attribute is int) is bool`
- `html_attribute_value(origin is HtmlEventsState, event is int, attribute is int) is string`

### `runtime/input_state`

- Opaque `Utf8InputState`
- `input_state_create() is Utf8InputState`
- `input_state_append(state is Utf8InputState, value is @byte) is int`
- `input_state_size(state is Utf8InputState) is int`
- `input_state_line_offset(state is Utf8InputState) is int`
- `input_state_take(state is Utf8InputState, size is int) is @byte`
- `input_state_eof(state is Utf8InputState) is bool`
- `input_state_finish(state is Utf8InputState) is bool`
- `input_state_closed(state is Utf8InputState) is bool`
- `input_state_close(state is Utf8InputState) is bool`

### `runtime/io_decorators`

- Opaque `LimitedReadingState`
- Opaque `ChainedReadingState`
- `limited_reading_state_create(limit is int) is LimitedReadingState`
- `limited_reading_state_remaining(state is LimitedReadingState) is int`
- `limited_reading_state_consume(state is LimitedReadingState, amount is int) is int`
- `chained_reading_state_create() is ChainedReadingState`
- `chained_reading_state_position(state is ChainedReadingState) is int`
- `chained_reading_state_advance(state is ChainedReadingState) is int`

### `runtime/json`

- Opaque `JsonParseState`
- Opaque `JsonNodeState`
- `json_parse_state(origin is string, maximum_depth is int) is JsonParseState`
- `json_parse_valid(origin is JsonParseState) is bool`
- `json_parse_offset(origin is JsonParseState) is int`
- `json_parse_line(origin is JsonParseState) is int`
- `json_parse_column(origin is JsonParseState) is int`
- `json_parse_message(origin is JsonParseState) is string`
- `json_parse_has_duplicate(origin is JsonParseState) is bool`
- `json_parse_duplicate(origin is JsonParseState) is string`
- `json_parse_root(origin is JsonParseState) is JsonNodeState`
- `json_node_kind(origin is JsonNodeState) is int`
- `json_node_boolean(origin is JsonNodeState) is bool`
- `json_node_number(origin is JsonNodeState) is string`
- `json_node_text(origin is JsonNodeState) is string`
- `json_node_size(origin is JsonNodeState) is int`
- `json_node_item(origin is JsonNodeState, index is int) is JsonNodeState`
- `json_member_name(origin is JsonNodeState, index is int) is string`
- `json_member_value(origin is JsonNodeState, index is int) is JsonNodeState`

### `runtime/layout`

- `indent(origin is string, prefix is string) is string`
- `dedent(origin is string) is string`
- `wrap(origin is string, width is int) is string`
- `shorten(origin is string, width is int, placeholder is string) is string`

### `runtime/math_aggregate`

- `aggregate_square_root(value is float) is float`

### `runtime/net`

Private numeric conversion via existing core.net symbols, used by net/ip:

- `ip_parse(family is int, text is string, port is int) is @byte`
- `ip_format(origin is @byte) is string`

### `runtime/network`

- Opaque `SocketState`
- `socket(family is int, socket_type is int, protocol is int) is SocketState`
- `close(origin is SocketState) is bool`
- `udp_bound(address is @byte, zone is string, reuse is bool, broadcast is bool, dual_stack is bool) is SocketState`
- `local_endpoint(origin is SocketState) is @string` — numeric host, decimal port, decimal scope or empty string
- `closed(origin is SocketState) is bool`

### `runtime/tls_transport`

Private consuming ABI: opaque TlsState, client(SocketState,int,@string,string),
close/closed, async handshake/read/write/shutdown, authorize with explicit DER
roots/kind/identity/exact-time strings, and authorized peer/version/cipher/ALPN
observations. Client transfers a managed SocketState; no application I/O is
permitted before verification. Native bounds/ownership are in the
[owned TLS client contract](tls-client-transport.md). Applications acquire the
public connection through TlsClientHandshake rather than constructing handles.

### `runtime/ordering`

- Opaque `BulkOrdering`

### `runtime/pem`

- `pem_kind_invalid_offset(kind is string) is int`
- `pem_encode(kind is string, base64 is string) is string`
- `pem_invalid_offset(origin is string) is int`
- `pem_block_count(origin is string) is int`
- `pem_block_source(origin is string, index is int) is string`
- `pem_block_kind(origin is string, index is int) is string`
- `pem_block_base64(origin is string, index is int) is string`

### `runtime/process`

- Opaque `ProcessState`
- Opaque `CapturedProcess`
- Opaque `PipelineState`
- Opaque `CapturedPipeline`
- `process_state_create() is ProcessState`
- `process_state_start(state is ProcessState, program is string, arguments is @string, directory is string, environment is %string:string) is bool`
- `process_state_started(state is ProcessState) is bool`
- `process_state_id(state is ProcessState) is int`
- `process_state_running(state is ProcessState) is bool`
- `async process_state_exit(state is ProcessState) is %string:int`
- `process_state_signal(state is ProcessState, signal is int) is bool`
- `process_state_close(state is ProcessState) is bool`
- `process_state_input_write(state is ProcessState, source is @byte) is int`
- `process_state_input_flush(state is ProcessState) is bool`
- `process_state_input_closed(state is ProcessState) is bool`
- `process_state_input_close(state is ProcessState) is bool`
- `process_state_output_read(state is ProcessState, limit is int) is @byte`
- `process_state_errors_read(state is ProcessState, limit is int) is @byte`
- `async process_state_capture(state is ProcessState, input is @byte, limit is int) is CapturedProcess`
- `captured_process_status(origin is CapturedProcess) is %string:int`
- `captured_process_overflow(origin is CapturedProcess) is int`
- `captured_process_output(origin is CapturedProcess) is @byte`
- `captured_process_errors(origin is CapturedProcess) is @byte`
- `pipeline_state_create() is PipelineState`
- `pipeline_state_add(state is PipelineState, program is string, arguments is @string, directory is string, environment is %string:string) is bool`
- `async pipeline_state_capture(state is PipelineState, input is @byte, limit is int) is CapturedPipeline`
- `captured_pipeline_statuses(origin is CapturedPipeline) is @%string:int`
- `captured_pipeline_overflow_stream(origin is CapturedPipeline) is int`
- `captured_pipeline_overflow_index(origin is CapturedPipeline) is int`
- `captured_pipeline_output(origin is CapturedPipeline) is @byte`
- `captured_pipeline_errors(origin is CapturedPipeline) is @@byte`

### `runtime/quoted_printable`

- `quoted_printable_encode(origin is @byte) is @byte`
- `quoted_printable_invalid_offset(origin is @byte) is int`
- `quoted_printable_decode(origin is @byte) is @byte`

### `runtime/random`

- Opaque `PcgState`
- `pcg_state(seed is int) is PcgState`
- `pcg_integer(state is PcgState, start is int, stop is int) is int`
- `pcg_float(state is PcgState) is float`
- `pcg_bytes(state is PcgState, size is int) is @byte`
- `random_sample_accepted(sample is @byte, start is int, stop is int) is bool`
- `random_integer_from_sample(sample is @byte, start is int, stop is int) is int`
- `random_float_from_sample(sample is @byte) is float`

### `runtime/random_distribution`

- `distribution_uniform(start is float, stop is float, fraction is float) is float`
- `distribution_log(value is float) is float`
- `distribution_exp(value is float) is float`
- `distribution_sqrt(value is float) is float`
- `distribution_cos(value is float) is float`
- `distribution_acos(value is float) is float`
- `distribution_power(base is float, exponent is float) is float`
- `distribution_angle(value is float) is float`

### `runtime/regex`

- Opaque `RegexState`
- Opaque `RegexQueryState`
- Opaque `RegexMatchState`
- Opaque `RegexReplacementState`
- Opaque `RegexSubstitutionState`
- `regex_state_create(expression is string, flags is @int) is RegexState`
- `regex_state_prepare(state is RegexState) is %string:int`
- `regex_state_group_count(state is RegexState) is int`
- `regex_state_group_names(state is RegexState) is %string:int`
- `regex_query_create(state is RegexState, origin is string, start is int, end is int, mode is int) is RegexQueryState`
- `regex_query_found(state is RegexQueryState) is bool`
- `regex_query_match(state is RegexQueryState) is RegexMatchState`
- `regex_match_capture_count(state is RegexMatchState) is int`
- `regex_match_capture_name(state is RegexMatchState, index is int) is string`
- `regex_match_capture_index(state is RegexMatchState, name is string) is int`
- `regex_match_capture_span(state is RegexMatchState, index is int) is %string:int`
- `regex_replacement_state(template is string) is RegexReplacementState`
- `regex_replacement_prepare(state is RegexReplacementState) is %string:int`
- `regex_replacement_tokens(state is RegexReplacementState) is @string`
- `regex_escape(origin is string) is string`
- `regex_substitution_state() is RegexSubstitutionState`
- `regex_substitution_ready(state is RegexSubstitutionState) is bool`
- `regex_substitution_store(state is RegexSubstitutionState, string is string, amount is int) is bool`
- `regex_substitution_string(state is RegexSubstitutionState) is string`
- `regex_substitution_amount(state is RegexSubstitutionState) is int`

### `runtime/single_byte`

- `ascii_representable_prefix(origin is string) is int`
- `ascii_encode(origin is string) is @byte`
- `ascii_valid_prefix(origin is @byte) is int`
- `ascii_decode(origin is @byte) is string`
- `latin1_representable_prefix(origin is string) is int`
- `latin1_encode(origin is string) is @byte`
- `latin1_decode(origin is @byte) is string`

### `runtime/statistics`

- `statistics_mean(values is @float) is float`
- `statistics_geometric_mean(values is @float) is float`
- `statistics_harmonic_mean(values is @float) is float`
- `statistics_median(values is @float, choice is string) is float`
- `statistics_quantiles(values is @float, count is int, method is string) is @float`
- `statistics_variance(values is @float, sample is bool) is float`
- `statistics_paired_valid(left is @float, right is @float, result is string) is bool`
- `statistics_paired(left is @float, right is @float, result is string) is float`

### `runtime/stdio`

- `stdout_write(data is @byte) is int`
- `stderr_write(data is @byte) is int`
- `stdin_read(limit is int) is @byte`
- `stdin_read_line() is @byte`

### `runtime/system`

- `system_platform() is string`
- `system_architecture() is string`
- `system_byte_order() is string`
- `system_host_name() is string`
- `system_cpu_count() is int`
- `secure_random(size is int) is @byte`
- `storage_usage(path is string) is %string:string`

### `runtime/time`

- `wall_clock() is int`
- `monotonic_clock() is int`
- `async sleep_nanoseconds(duration is string) is bool`

### `runtime/time_format`

- `time_pattern_invalid_offset(pattern is string, local is bool, parsing is bool) is int`
- `time_format_date(year is int, month is int, day is int, pattern is string) is string`
- `time_format_local(year is int, month is int, day is int, hour is int, minute is int, second is int, nanosecond is int, pattern is string) is string`
- `time_parse_invalid_offset(origin is string, pattern is string, local is bool) is int`
- `time_parse_parts(origin is string, pattern is string, local is bool) is @int`

### `runtime/time_zone`

- `timezone_version() is string`
- `timezone_known(name is string) is bool`
- `timezone_offset(name is string, instant is rat) is int`
- `timezone_daylight(name is string, instant is rat) is bool`
- `timezone_abbreviation(name is string, instant is rat) is string`
- `timezone_local_candidates(name is string, year is int, month is int, day is int, hour is int, minute is int, second is int, nanosecond is int) is int`
- `timezone_local_instant(name is string, year is int, month is int, day is int, hour is int, minute is int, second is int, nanosecond is int, later is bool) is rat`

### `runtime/unicode_character`

- `character_name(origin is char) is string`
- `category_code(origin is char) is string`
- `character_combining_class(origin is char) is int`
- `character_is_numeric(origin is char) is bool`
- `character_numeric_value(origin is char) is float`

### `runtime/unicode_normalization`

- `normalized_text(origin is string, form is string) is string`

### `runtime/utf`

- `utf8_encode(origin is string) is @byte`
- `utf8_decode(origin is @byte) is string`
- `utf8_valid_prefix(origin is @byte) is int`
- `utf16_encode(origin is string, order is string) is @byte`
- `utf16_decode(origin is @byte, order is string) is string`
- `utf16_valid_prefix(origin is @byte, order is string) is int`
- `utf32_encode(origin is string, order is string) is @byte`
- `utf32_decode(origin is @byte, order is string) is string`
- `utf32_valid_prefix(origin is @byte, order is string) is int`

### `runtime/uuid`

- `uuid_valid(origin is string) is bool`
- `uuid_bytes(origin is string) is @byte`
- `uuid_string(origin is @byte) is string`
- `uuid_version(origin is @byte) is int`
- `uuid_variant(origin is @byte) is string`
- `uuid_versioned(origin is @byte, version is int) is @byte`
- `uuid_named(namespace is @byte, name is @byte, version is int) is @byte`
- `uuid_custom(high is int, middle is int, low is int) is @byte`



## `toml/value`

[Implementation](../../std/toml/value.ok) · [Contract and parser gates](toml-values-guide.md)

| Declaration / role | Constructors | Public behavior |
| --- | --- | --- |
| `TomlValue` | — | `accept(to is TomlVisitor) is void` |
| `TomlVisitor` | — | `text(value is string) is void`<br>`integer(value is int) is void`<br>`real(value is float) is void`<br>`boolean(value is bool) is void`<br>`elements(values is Sequencing<TomlValue>) is void`<br>`members(values is Sequencing<TomlMember>) is void`<br>`local_date(value is TomlLocalDate) is void`<br>`local_time(value is TomlLocalTime) is void`<br>`local_datetime(value is TomlLocalDateTime) is void`<br>`offset_datetime(value is TomlOffsetDateTime) is void`<br>`infinity(negative is bool) is void`<br>`not_a_number() is void` |
| `TomlString` does `TomlValue` | `TomlString(value is string)` | `accept(to is TomlVisitor) is void` |
| `TomlInteger` does `TomlValue` | `TomlInteger(value is int)` | `accept(to is TomlVisitor) is void` |
| `TomlFloat` does `TomlValue` | `TomlFloat(value is float)` | `accept(to is TomlVisitor) is void` |
| `TomlInfinity` does `TomlValue` | `TomlInfinity(negative is bool)` | `string() is string`<br>`accept(to is TomlVisitor) is void` |
| `TomlNaN` does `TomlValue` | `TomlNaN()` | `string() is string`<br>`accept(to is TomlVisitor) is void` |
| `TomlBoolean` does `TomlValue` | `TomlBoolean(value is bool)` | `accept(to is TomlVisitor) is void` |
| `TomlMember` | `TomlMember(name is string, value is TomlValue)` | `name() is string`<br>`value() is TomlValue` |
| `TomlArray` does `TomlValue` | `TomlArray(values is Sequencing<TomlValue>)`<br>`TomlArray(values is Sequencing<TomlValue>, maximum_items is int)` | `values() is Sequencing<TomlValue>`<br>`accept(to is TomlVisitor) is void` |
| `TomlTable` does `TomlValue` | `TomlTable(members is Sequencing<TomlMember>)`<br>`TomlTable(members is Sequencing<TomlMember>, maximum_members is int)` | `members() is Sequencing<TomlMember>`<br>`membership(name is string) is bool`<br>`value(name is string) is TomlValue`<br>`accept(to is TomlVisitor) is void` |
| `MissingTomlMember` does `Error` | `MissingTomlMember(name is string)` | `name() is string`<br>`text() is string` |
| `DuplicateTomlMember` does `Error` | `DuplicateTomlMember(name is string)` | `name() is string`<br>`text() is string` |
| `InvalidTomlLimit` does `Error` | `InvalidTomlLimit()` | `text() is string` |
| `InvalidTomlSize` does `Error` | `InvalidTomlSize()` | `text() is string` |
| `TomlLimitExceeded` does `Error` | `TomlLimitExceeded()` | `text() is string` |

The observer controls recursive traversal; scalar access is nominal dispatch,
not casts or tags. TOML 1.0 parsing is supplied by toml/parse; no document writer is implemented.

| Temporal declaration | Constructors | Public behavior |
| --- | --- | --- |
| `TomlLocalDate` does `TomlValue` | `TomlLocalDate(year is int, month is int, day is int)` | `year() is int`<br>`month() is int`<br>`day() is int`<br>`string() is string`<br>`accept(to is TomlVisitor) is void` |
| `TomlLocalTime` does `TomlValue` | `TomlLocalTime(hour is int, minute is int, second is int, nanosecond is int)` | `hour() is int`<br>`minute() is int`<br>`second() is int`<br>`nanosecond() is int`<br>`string() is string`<br>`accept(to is TomlVisitor) is void` |
| `TomlLocalDateTime` does `TomlValue` | `TomlLocalDateTime(date is TomlLocalDate, time is TomlLocalTime)` | `date() is TomlLocalDate`<br>`time() is TomlLocalTime`<br>`string() is string`<br>`accept(to is TomlVisitor) is void` |
| `TomlOffset` | `TomlOffset(minutes is int)`<br>`TomlOffset(minutes is int, known is bool)` | `minutes() is int`<br>`known() is bool`<br>`string() is string` |
| `TomlOffsetDateTime` does `TomlValue` | `TomlOffsetDateTime(local is TomlLocalDateTime, offset is TomlOffset)` | `local() is TomlLocalDateTime`<br>`offset() is TomlOffset`<br>`string() is string`<br>`accept(to is TomlVisitor) is void` |
| `InvalidTomlDate` does `Error` | `InvalidTomlDate()` | `text() is string` |
| `InvalidTomlTime` does `Error` | `InvalidTomlTime()` | `text() is string` |
| `InvalidTomlOffset` does `Error` | `InvalidTomlOffset()` | `text() is string` |

## TOML document parser

[Implementation](../../std/toml/parse.ok) · [Contract](toml-parse-guide.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| ParsedToml | origin is string; or origin, maximum_input, maximum_depth, maximum_nodes | value() is TomlTable |
| InvalidToml | offset is int, line is int, column is int, message is string | offset() is int; line() is int; column() is int; text() is string |
| InvalidTomlBudget | no arguments | text() is string |
| TomlNode (internal projection) | origin is TomlNodeState | value() is TomlValue; table() is TomlTable |

Construction is lazy except budget validation. Parsing is bounded and deterministic;
no partial root, I/O, implicit output or host-time conversion. The native runtime/toml
module supplies opaque memo/tree state; kind tags do not enter the TomlValue protocol.
The internal projection declaration is inventoried for completeness, not recommended
as a domain API. Concrete parsed arrays/tables use existing nominal model classes.

## INI values and printing protocol

[Implementation](../../std/config/ini.ok) · [Contract](ini-guide.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| IniContent | role | print(to is IniWriting) is void |
| IniWriting | role | section(name is string) is void; entry(name is string, value is string) is void |
| IniEntry | name is string, value is string | name() is string; value() is string; print(to is IniWriting) is void |
| IniSection does IniContent | name, entries is Sequencing<IniEntry>; optional maximum_entries is int | name(); entries(); membership(name); value(name); print(to) |
| IniDocument does IniContent | sections is Sequencing<IniSection>; optional maximum_sections is int | sections(); section(name); print(to) |
| MissingIniSection / DuplicateIniSection | name is string | name() is string; text() is string |
| MissingIniEntry / DuplicateIniEntry | section is string, name is string | section() is string; name() is string; text() is string |
| InvalidIniLimit / InvalidIniSize / IniLimitExceeded | no arguments | text() is string |

Defaults are 1024 entries and 128 sections, with source-observation contracts
described in the guide. Names are exact Unicode coordinates, not hierarchical
paths or an ambient DEFAULT namespace. Parser and canonical output are implemented.

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| ParsedIni does IniContent | origin is string; optional maximum_input, maximum_sections, maximum_entries are int | value() is IniDocument; print(to is IniWriting) is void |
| IniString | origin is IniContent; optional maximum_output is int | string() is string |
| InvalidIni | line is int, column is int, message is string | line() is int; column() is int; text() is string |
| InvalidIniEmission / IniOutputLimitExceeded | no arguments | text() is string |
| IniTextEmission (internal protocol adapter) | maximum is int | section(name); entry(name,value); string() |

Default parser budgets: 1048576 Unicode scalars, 128 sections, 10000 total entries,
1024 entries per section. Default output budget: 1048576 scalars. Parsing/rendering
are lazy, deterministic and memory-only. Output failure is sticky, with no partial
text. See the guide for the exact dialect and exception precedence.

### Private runtime/ini substrate

IniParseState and IniWriteState are opaque registry-owned memory. Native functions:
ini_parse_state(string,int,int,int), ini_parse_valid/line/column/message(state);
ini_section_count(state), ini_section_name(state,int), ini_entry_count(state,int),
ini_entry_name/value(state,int,int); ini_write_state(int),
ini_write_section(state,string), ini_write_entry(state,string,string),
ini_write_status/name/parent/string(state). Counts preserve insertion order.
Wrong indices fail as invalid_input; invalid document/text observations fail as
invalid_data. Registry shutdown invalidates handles. No external resources.

## Template paths

[Implementation](../../std/template/path.ok) · [Contract](template-guide.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| TemplatePath does Comparing<TemplatePath> | segments is Sequencing<string>; optional maximum_segments, maximum_length are int | segments() is Sequencing<string>; string() is string; order(other is TemplatePath) is Order |
| TemplatePathFromString | origin is string; optional maximum_segments, maximum_length are int | projected value() is TemplatePath |
| InvalidTemplatePath | reason is string | text() is string |
| InvalidTemplatePathSize / InvalidTemplatePathLimit / TemplatePathLimitExceeded | no arguments | text() is string |
| TemplatePathSnapshot (internal) | source is Sequencing<string>, maximum_segments is int, maximum_length is int | items() is @string |
| TemplatePathParts (internal) | maximum is int | following(segment is string) is ^TemplatePathParts; items() is @string |

Default bounds: 64 segments, 4096 diagnostic scalars. Identity and ordering use
segments, not joined diagnostic text. Parsed single dot is reserved for a current
section item; explicit literal-dot segments retain identity. Source size once,
ascending items once; no implicit close or later source observation.

## Explicit template string data

[Implementation](../../std/template/model.ok) · [Contract](template-guide.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| TemplateStrings | role | string(path is TemplatePath) is string |
| TemplateBinding | path is TemplatePath, value is string | path() is TemplatePath; string() is string |
| TemplateBindings does TemplateStrings | values is Sequencing<TemplateBinding>; optional maximum_bindings is int | string(path is TemplatePath) is string |
| MissingTemplateValue / DuplicateTemplateValue | path is TemplatePath | path() is TemplatePath; text() is string |
| InvalidTemplateBindingLimit / InvalidTemplateBindingSize / TemplateBindingLimitExceeded | no arguments | text() is string |
| TemplateBindingSnapshot (internal) | source is Sequencing<TemplateBinding>, maximum is int | items() is @TemplateBinding |

Default cardinality 1024; exact path identity, empty values present, no implicit
escaping/coercion, reflection, JSON inspection or I/O. Section models and rendering
limits remain separate completion tasks; named libraries are described below.

## Named template libraries

[Implementation](../../std/template/library.ok) · [Contract](template-library-guide.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| NamedTemplate | name is string, value is TextTemplate | name() is string; value() is TextTemplate |
| TemplateLibrary | (); values is Sequencing<NamedTemplate>; values plus maximum_templates is int | membership(name is string) is bool; value(name is string) is TextTemplate; names() is Sequencing<string> |
| MissingTemplate / DuplicateTemplate | name is string | name() is string; text() is string |
| InvalidTemplateName / InvalidTemplateLibraryLimit / InvalidTemplateLibrarySize / TemplateLibraryLimitExceeded | () | text() is string |
| TemplateLibrarySnapshot (internal) | source is Sequencing<NamedTemplate>, maximum is int | items() is @NamedTemplate |

Nonempty exact names; snapshot count defaults to 1024, zero permits empty.
Lookup never compiles/renders; partial syntax remains unsupported by TextTemplate.
TemplateLibrary() replaces the proposed EmptyTemplateLibrary wrapper.

## Explicit JSON template strings

[Implementation](../../std/template/json.ok) · [Contract](template-json-guide.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| JsonTemplateStrings does TemplateStrings | origin is Json; optional maximum_members, maximum_output are int | string(path is TemplatePath) is string; string(path is string) is string |
| NonScalarTemplateValue does Error | path is TemplatePath, kind is JsonKind | path() is TemplatePath; kind() is JsonKind; text() is string |
| InvalidTemplateJsonLimits / TemplateJsonLimitExceeded | () | text() is string |

Object-only paths through `Json` member lookups, depth at most 64, default per-object
1024 members/per-scalar 1048576 UTF-8 bytes. Strings remain literal, Decimal canonical, bool
true/false and null empty. String path `.` observes the bound current item; other
string paths parse dotted member names. Nominal TemplatePath retains literal-dot
identity. No parent fallback, array indexing or implicit JSON conversion.

## Bounded plain text templates

[Implementation](../../std/template/compile.ok) · [Contract](template-guide.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| TextTemplate | origin is string; optional maximum_input, maximum_nodes, maximum_output are int | source() is string; string(data is TemplateStrings) is string; write(data is TemplateStrings, destination is WritingText) is void |
| TemplateFailure | offset is int, line is int, column is int, message is string | offset(); line(); column(); text() |
| InvalidTemplateLimits / TemplateOutputLimitExceeded | no arguments | text() is string |
| TemplateTextParts (internal) | maximum is int | following(text is string) is ^TemplateTextParts; string() is string |

First implemented grammar: variables, comments, literal delimiter escapes.
Complete compilation precedes data observation; each occurrence queries data
once. Default limits: 1048576 source scalars, 10000 instructions, 1048576 UTF-8
output bytes. No implicit HTML escaping, coercion or recompilation of values.
Sections/partials remain pending and explicitly rejected; bounded streaming is implemented.

TemplateProgram and TemplateScanner parse in Okkama; TextTemplate uses no native
compiler state. The existing template_output_size(string,int) primitive counts
UTF-8 bytes without a byte-array allocation, returning -1 on budget overflow.

## Structured template compilation

[Implementation](../../std/template/tree.ok) · [Contract](template-tree-guide.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| TemplateTree | source is string; optional maximum_input, maximum_tokens, maximum_depth are int | nodes() is @TemplateTreeNode |
| TemplateTreeNode | kind is TemplateTreeKind, text is string, children is @TemplateTreeNode | kind(); text(); children() |
| TemplateTreeKind | literal, variable, section, inverted, partial | grammar categories |
| InvalidTemplateTreeNode | () | text() is string |

Default limits: 1048576 input scalars, 20000 tokens, 64 section levels.
Complete compilation checks matching/standalone lines before returning immutable
nodes. Parsing uses TemplateProgram(source,input,tokens,true); no lookup/render
or partial expansion occurs. Internal TemplateTreeFrame/TemplateTreeBuilder
retain persistent parser stacks. SectionTemplate below consumes the tree for
string/stream rendering and explicit partial expansion.

## Streamed template output

[Implementation](../../std/template/render.ok) · [Contract](template-guide.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| TemplateOutput | origin is TextTemplate, data is TemplateStrings, destination is WritingText | write() is void |
| TemplateByteBudget (internal, in template/compile) | maximum is int | following(text is string) is ^TemplateByteBudget |

Borrowed command, lazy construction, no implicit flush/close/newline. Contributions
are preflighted against the template's byte budget and emitted in chunks of at
most 1024 scalars/4096 UTF-8 bytes; completed prefixes remain visible on failure.
Source and sink errors stop later work. Repeated commands append another render.
The string builder and streamed command share exact byte-budget validation.

## Bounded JSON section rendering

[Implementation](../../std/template/sections.ok) · [Contract](template-sections-guide.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| SectionTemplate | source is string; optional maximum_iterations, maximum_output, maximum_members are int | string(data is Json) is string; write(data is Json, destination is WritingText) is void |
| TemplateIterationLimitExceeded / InvalidTemplateSectionSize | () | text() is string |
| UnsupportedTemplatePartial | name is string | name() is string; text() is string |

The source compiles with TemplateTree defaults before observing data. Arrays
iterate, objects render once, truthy scalars become current context; zero is true.
Inverted bodies keep enclosing context. No parent fallback. Defaults: 10000
executed bodies, 1048576 aggregate UTF-8 output bytes, 1024 members per object.
Without a library, static partials fail before rendering. Library constructors
enable partial expansion as specified below.

Internal TemplateIterationBudget owns a bounded byte ledger; TemplateSectionRun
borrows it and a metered text destination. TemplateSectionLookup and its visitor
resolve paths using the nominal JSON print protocol. TemplateSectionObject
materializes bounded object members; TemplateSectionVisitor applies truth and
body iteration. These collaborators are importable because modules have no
private declarations; applications use SectionTemplate.

### Explicit section partials

[Contract](template-partials-guide.md). SectionTemplate also accepts
`(source, partials is TemplateLibrary)` or `(source, partials, maximum_iterations,
maximum_output, maximum_members, maximum_partial_depth)`. Default partial depth:
32. Each expansion costs one shared iteration and uses the current context.
The library is a source catalog; TextTemplate.source() compiles with TemplateTree
defaults, independently of TextTemplate's flat-render settings. No ambient access.
Only reached references compile. MissingTemplate retains exact lookup names;
TemplatePartialDepthExceeded and TemplateSectionDepthExceeded expose text().
Active section nesting is capped at 64 across partial calls. Empty work accepts
zero budgets. Existing no-library constructors retain UnsupportedTemplatePartial.

### Streamed section and partial output

[Contract](template-section-output-guide.md). SectionTemplate.write(data,destination)
shares the traversal used by string(data). Each contribution is preflighted, then
written in nonempty chunks <=1024 scalars/4096 UTF-8 bytes. No whole-result buffer,
added newline, flush or close. Success prefixes remain visible on failure; errors
stop further reads/writes. Repeated calls have fresh limits. Partial compilation
is lazy at each reached reference, so earlier parent output may be visible.

Internal TemplateOutputMeter(maximum) owns an eight-byte binary count with
remaining() and claim(string); TemplateSectionOutput(destination,maximum) owns
that meter and borrows a WritingText sink, exposing remaining() and append(string).
They use existing std binary/buffer capabilities and add no native primitives.

## Contextual HTML templates

[Implementation](../../std/template/html.ok) · [Contract](template-html-guide.md)

| Declaration | Constructors | Public behavior |
| --- | --- | --- |
| HtmlTemplate | source is string; optional maximum_input, maximum_nodes, maximum_output are int | source() is string; string(data is Json) is string; write(data is Json, destination is WritingText) is void |
| InvalidHtmlTemplate | reason is string | text() is string |

Defaults: 65536 source scalars, 10000 instructions, 1048576 output UTF-8 bytes;
name length <=128, nesting <=64. Complete contextual validation precedes data
observation. Text and whitelisted quoted attributes have separate EscapedHtml
policies; structural/raw/comment/declaration/URL/event/CSS interpolation fails.
This is a strict fragment dialect, not DOM recovery or literal-markup sanitization.
Sections/partials remain explicitly rejected in this initial slice.

Internal HtmlTemplateProgram, HtmlTemplatePlan, HtmlTemplateInstruction and
HtmlTemplateContext/HtmlTemplateState represent the verified plan and immutable
lexical positions. HtmlTemplateCharacter provides ASCII lexical observations.
No high-level native state getter, reflection, new native API or external I/O.

## Typed HTML template library

[Implementation](../../std/template/html_library.ok) · [Contract](template-html-library-guide.md)

NamedHtmlTemplate(name is string, value is HtmlTemplate) exposes name() and value().
HtmlTemplateLibrary() is empty; constructors with Sequencing<NamedHtmlTemplate>
and optional nonnegative maximum_templates (default 1024) snapshot once.
value(name) is HtmlTemplate, membership(name) is bool, names() is Sequencing<string>.
MissingHtmlTemplate/DuplicateHtmlTemplate retain name() and text(); invalid name,
limit, source size and excess cardinality have separate typed errors. Lookup never
parses/renders; exact ordered keys and borrowed sources match the plain library.
Plain TextTemplate cannot cross this nominal boundary.

## Locale tag core

[Implementation](../../std/locale/tag.ok) · [Contract](locale-tag-guide.md)

Locale does Comparing<Locale>. Constructors accept `(language)`,
`(language, region)` or `(language, script, region, variants is Sequencing<string>)`.
language(), script(), region() and string() return strings; variants() returns
Sequencing<string>; order(other is Locale) compares complete case-normal tags.
Empty script/region denote absence only in the four-argument constructor.
MissingLocaleSubtag(kind) exposes kind()/text(); InvalidLocale(offset,reason)
exposes offset()/text(). Parser offsets count original Unicode scalars from zero;
direct-coordinate validation uses zero.

LocaleFromLanguageTag(source is string).value() is a lazy projected Locale.
The bounded complete-tag profile supports RFC 5646 syntax plus pinned IANA
2026-09-17 preferred values, extlangs, grandfathered and extension/private-use
forms. One-argument Locale also accepts a complete tag; coordinate constructor
arities/named parameters remain compatible. Missing language/script/region are
typed projections. LocaleExtensions exposes ordered nominal Unicode keywords
rather than the withdrawn map candidate. See [current contract](locale-tag-guide.md).

## Property-list values

[Implementation](../../std/plist/value.ok) · [Contract](plist-value-guide.md)

PlistValue has kind() is PlistKind and print(to is PlistWriting). It is independent
from Json. PlistKind has boolean_value, integer_value, real_value, string_value,
data_value, date_value, array_value, dictionary_value and uid_value.

| Value | Constructor payload / value() result | PlistWriting event |
| --- | --- | --- |
| PlistBoolean | bool | boolean(bool) |
| PlistInteger | BigInt | integer(BigInt) |
| PlistReal | float | real(float) |
| PlistString | string | text(string) |
| PlistData | @byte | data(@byte) |
| PlistDate | Instant | date(Instant) |
| PlistUid | nonnegative BigInt | uid(BigInt) |

PlistArray accepts Sequencing<PlistValue> and optional maximum_items, exposes
values(), and emits elements(Sequencing<PlistValue>). PlistDictionary accepts
Sequencing<PlistMember> and optional maximum_items, exposes members(),
membership(name), value(name), and emits members(Sequencing<PlistMember>).
PlistMember(name is string, value is PlistValue) exposes name()/value(). Dictionary
entries retain insertion order and exact names, including empty strings.

Both containers default to 1024 items, snapshot size/items once and never close
borrowed sources. InvalidPlistContainerLimit, InvalidPlistContainerSize and
PlistContainerLimitExceeded distinguish negative limits, negative source sizes
and exceeded limits. DuplicatePlistMember/MissingPlistMember expose name()/text();
InvalidPlistUid rejects negative identifiers. All errors implement Error.

Scalar projections retain exact payloads; format-specific restrictions belong to
future codecs. Printing emits one event, leaving recursion and output budgets to
the recipient; errors propagate. No implicit I/O, reflection or resource ownership.
Internal PlistArraySnapshot/PlistDictionarySnapshot share bounded preconstruction
between constructor overloads. XML/binary parsing and encoding are described below.

## Property-list parsing

[Implementation](../../std/plist/parse.ok) · [Contract](plist-parse-guide.md)

ParsedPlist(origin is @byte) exposes projected value() is PlistValue. An overload
adds maximum_input, maximum_nodes, maximum_depth; defaults are 1048576 bytes,
4096 visited nodes and 64 levels. Construction retains immutable input; each
value() reparses. Negative configuration throws InvalidPlistLimits. InvalidPlist
(offset is int, message is string) exposes original byte offset()/text() for input,
format and budget failures. There is no I/O, ownership or new native provider.

Recognizes UTF-8 XML with optional BOM and bplist00. The strict XML plist dialect
supports the ordinary scalar/container types, ordered keys, predefined/numeric
entities, comments and CDATA. Only the standard Apple DOCTYPE is recognized;
external loading, custom entities and internal subsets are never enabled.
Binary parsing preflights the trailer, offset table, object spans and references;
supports integers, finite reals, dates, bytes, ASCII/UTF-16 strings, UIDs and
containers. Cycles fail; shared references expand under a visited-node budget.
Dictionary keys count toward the budget. Unreachable payloads are not decoded.

Unsupported encodings/markers fail explicitly. Real spellings and exponents are
bounded; dates must fit the existing Instant nanosecond model without rounding.
See the contract for exact XML syntax and binary widths. Internal source/token,
parse-result/progress and checked-byte objects implement the grammar in Okkama;
there is no claim of a general XML processor. Serialization is described below.

## Property-list encoding

[Implementation](../../std/plist/write.ok) · [Contract](plist-write-guide.md)

PlistFormat = (xml, binary). EncodedPlist(origin is PlistValue, format is PlistFormat)
exposes bytes() is @byte. A five-argument overload adds maximum_bytes,
maximum_nodes, maximum_depth, defaulting to 1048576, 4096 and 64 respectively.
Construction is lazy. Queries traverse through one PlistWriting event per value;
kind()/reflection are never used. Dictionary insertion order is preserved.

XML output is compact UTF-8 with a declaration, plist wrapper and final LF. XML
1.0 characters are checked; CR uses a reference, data uses standard padded base64,
dates use UTC RFC3339. UIDs are unsupported. Binary output is bplist00 with
postorder records, eight-byte refs/offsets, signed 128-bit integers, unsigned
128-bit UIDs, binary64 reals and ASCII/UTF-16BE strings. Dates support UTC years
0001..9999; binary conversion must preserve the existing nanosecond roundtrip.

InvalidPlistWriteLimits rejects negative configuration. PlistWriteLimitExceeded
exposes kind()/text() for bytes/nodes/depth; UnsupportedPlistValue exposes
reason()/text(). InvalidPlistPrintProtocol rejects zero/multiple value events;
InvalidPlistWriteSource rejects negative source size; DuplicatePlistMember retains
the offending dictionary name. Caller errors propagate unchanged.

Limits include keys, binary offset/trailer and XML envelope. Container size is
observed once, checked against node/byte budgets before item reads, then ascending
items are read once without cursor/first/last/close. Writer buffers are owned and
closed; the returned immutable bytes survive their lifetime. No external I/O,
native additions or implicit deduplication. Internal PlistWireSession, PlistWireValue,
PlistWrittenNames, PlistWireInteger and PlistWireLength implement traversal and wire layout.

## Async completion without a value

`Timing.wait(duration)` and `SystemClock.wait(duration)` now declare `is void`.
Use `await clock.wait(duration);` for the effect; do not capture its result.
A pending call can be kept in an inferred local and awaited repeatedly, but
`Future<void>` is not a language type. Typed errors and cancellation are unchanged.

The compatibility library in `stdlib/` also uses async void for
`SystemClockAsync.sleep`, `Sleeping.through_native_async`, `Server.listen`,
`Server.serve_one` and its private `served_connection` helper. These actions
previously returned their receiver solely to acknowledge completion.
Other receiver-result APIs retain their existing contracts in this migration;
in particular typed `Future<Sending<T>>` channel initialization and generic
Future combinators are not silently converted to no-value completion.

## `csv/stream`

[Implementation](../../std/csv/stream.ok) · [Contract](csv-stream-guide.md)

| Declaration | Constructors | Behavior |
| --- | --- | --- |
| `CsvStream` does `Closing` | origin Reading; optionally all five CSV budgets | `row() is @string`, `closed() is bool`, `close() is void` |
| `EndOfCsv`, `ClosedCsvStream` do `Error` | no arguments | `text() is string` |

`runtime/csv_state::CsvState` is an opaque scanner substrate, not an application
representation. CsvStream owns scanner memory and borrows its Reading source.

## `http/connection`

[Contract](http1-connecting.md). Http1Connecting(HttpEndpointSelection,TlsClientPolicy
[, Http1Limits,Timing]).session() returns an owned Http1Session over actual HTTP
or verified HTTPS. Only checked numeric endpoints are attempted through bounded TcpRacing;
URL-derived DNS/IP identity and actual ALPN gate HTTPS publication. Ready unclaimed
results and unpublished transports are disposed. No shared pool or request rewrite.

## `http/endpoint`

[Contract](http-endpoints.md). EndpointAllowing.allowed(Url,IpEndpoint) is bool
is implemented by AllEndpoints, PublicEndpoints and ListedNetworks. The latter
snapshots up to 1024 CIDRs; public policy consumes pinned IP properties.
HttpEndpointSelection(Url,Resolving,EndpointAllowing).endpoints() returns a lazy
bounded checked subset; url() preserves its immutable request. Numeric targets
bypass DNS; positive HTTP/HTTPS port and exact resolver-port agreement are
mandatory. Errors: InvalidHttpEndpoint, EndpointDenied, EndpointSelectionLimit.

## `http/tls_session`

[Contract](http1-tls-session.md). Http1TlsSession(TlsConnection[, Http1Limits])
implements Closing and async exchange(Request) is Http1Exchange. It adopts only
exact http/1.1 or absent ALPN; rejection preserves caller ownership and emits no
HTTP bytes. Http1TlsProtocol(selected).version() consumes the same pure rule.

## `http/method`

[Contract](http-method.md). Pure immutable protocol values; no native boundary.

| Object | Construction | Observations |
| --- | --- | --- |
| Method | `Method(origin is string)` | `string() is string`, case preserved; InvalidMethod on non-token |
| MethodSemantics | `MethodSemantics(origin is Method)` | `safe() is bool`, `idempotent() is bool`; unknown extensions false |
| InvalidMethod | `InvalidMethod(origin is string)` | `text() is string` |

Method tokens are case-sensitive. No body prohibition or automatic retry follows
from these observations. Wire line-size policy belongs to its eventual framer.

## `http/status`

[Contract](http-status.md). No I/O, ownership or cancellation effects.

| Object | Construction | Observations |
| --- | --- | --- |
| Status | `Status(code is int)` | `code() is int`, `class() is StatusClass`, `phrase() is string` |
| InvalidStatus | `InvalidStatus(code is int)` | `code() is int`, `text() is string` |

StatusClass = informational, successful, redirection, client_error, server_error.
The accepted range is 100..599, not a closed registry enum. Phrases are an explicit
IANA snapshot for presentation; unknown/unused values yield an empty string.

## `http/header`

[Contract](http-header.md). Pure values and bounded immutable snapshots.

| Object | Construction | Observations |
| --- | --- | --- |
| HeaderName | `(origin is string)` | `string`, `lowercase`, `order(HeaderName) is Order` |
| Header | `(name is HeaderName, value is string)` | `name`, `value`; Latin-1 octets, trimmed edge OWS |
| HeaderLimits | `()` / `(entries is int, bytes is int)` | `entries`, `bytes`; default 1024 / 1048576 |
| Headers | `(entries is Sequencing<Header>[, limits is HeaderLimits])` | `entries`, `limits`, `membership(name)`, `first(name)`, `values(name)`, `addition(entry)` |
| HeadersWithout | `(origin is Headers, names is Sequencing<HeaderName>)` | `result() is Headers` |
| ReplacedHeader | `(origin is Headers, entry is Header)` | `result() is Headers`; remove matching then append |
| Http2Headers | `(origin is Headers, request is bool)` | `result() is Headers`; lowercase logical fields and validation |

Errors: InvalidHeader(name,reason).text(), MissingHeader(name).name()/text(),
HeaderLimitExceeded(kind).kind()/text(); invalid budgets use InvalidArgument.
No wire parser, HPACK, pseudo-fields or implicit joining/unfolding. Matching uses
canonical names; structural equality remains unchanged. HeaderSnapshot and
HttpAsciiLowercase are importable implementation collaborators.

## `http/body`

[Contract](http-body.md). BodyOpening has `length() is BigInt`,
`replayable() is bool`, `reading() is BodyReading`; BodyReading has
`async bytes(limit is int) is @byte`, `trailers() is Headers`,
`closed() is bool`, `close() is void`.

| Object | Construction | Behavior |
| --- | --- | --- |
| EmptyBody | `()` | exact zero length, independent empty readers |
| ByteBody | `(origin is @byte)` | exact length, independent replay |
| OneShotBody | `(origin is BodyReading)` | one atomic claim; unknown length; owns unopened reader |
| StreamBodyReading | `(input is AsyncReading, owner is Closing, trailers is Headers)` | exclusive already-delimited source, bounded serialized reads, EOF trailer publication, cancellation and cleanup |

Errors: BodyClosed, BodyAlreadyOpened, UnknownBodyLength, TrailersUnavailable;
InvalidReadLimit and InvalidReading are reused. No FileBody implementation or
wire connection reuse/reset is implied. ByteBodyInput and BodyReadCleanup are
importable implementation collaborators; one read owns one bounded FutureGroup.

## `http/content_coding`

[Contract](http-content-coding.md). `ContentCoding(origin is string).string()`
is a canonical lowercase concrete name (including the explicit identity choice),
not an installed-codec observation. Unknown extension names are accepted; `*`
is a separate negotiation wildcard and rejected here. InvalidContentCoding(origin)
implements Error.text(). Body coding decorators remain blocked by Stage 16.

## `http/date`

[Contract](http-date.md). HttpDate(origin is Instant) floors to whole seconds and
implements Comparing<HttpDate>; value() returns normalized Instant and string()
returns IMF-fixdate/GMT. ParsedHttpDate(origin is string[,reference is Instant])
parses through value() is Instant. RFC850 requires the explicit reference;
HttpDateReferenceRequired is thrown without it. InvalidHttpDate(reason) covers
invalid layout/calendar/time/range. Both errors implement text(). Helpers for
bounded fields, decimal/name/clock parsing and UTC conversion are infrastructure.

## `http/conditional`

[Contract](http-conditional.md). EntityTag(value is string, weak is bool) validates
the opaque Latin-1 octet identity, observes value()/weak(), renders string(),
and offers strong_matches(other is EntityTag)/weak_matches(other) is bool.
EntityTags() is wildcard; EntityTags(values is Sequencing<EntityTag>[,limits is
ConditionalLimits]) takes a bounded snapshot. wildcard() is bool, values() is
Sequencing<EntityTag>, string() is string, strong_matches/weak_matches are bool.
ParsedEntityTags(origin is string[,limits]).value() is EntityTags parses the
combined field under recipient list rules without unescaping.
ConditionalLimits([bytes is int, entries is int]) observes bytes()/entries();
defaults 8192/128. InvalidEntityTag(offset,reason) observes offset()/reason()/text();
ConditionalLimitExceeded(kind) observes kind()/text(). Both implement Error.
EntityTagSnapshot(values,limits).values(), ConditionalItems<T> persistent
following()/values(), and ConditionalPosition(index,slots) with index()/slots(),
following()/separator() are importable implementation collaborators.

ByteRange(start is BigInt,end is BigInt) enforces inclusive nonnegative bounds;
start()/end()/size() are BigInt and string() is its canonical range fragment.
ByteRanges(origin is string[,limits is ConditionalLimits]) validates a canonical
bytes plan; string() is the field representation, ranges(size is BigInt) is
Sequencing<ByteRange> sorted/coalesced/clipped to a known length, and
satisfiable(size) is bool, including the RFC positive-suffix rule at length zero.
InvalidRange(reason) implements Error and observes reason()/text().
ByteRangeSpec(origin).string()/ranges()/satisfiable(), ByteRangeParsing.values(),
ConditionalDecimal.value(), ConditionalText.string(), and CoalescedByteRanges
following()/values() are importable parsing/materialization infrastructure.

## `http/negotiation`

[Contract](http-negotiation.md). Quality(origin is rat) validates exact 0..1
thousandths; value() is rat, acceptable() is bool, order(other is Quality) is Order
via Comparing<Quality>, string() is canonical qvalue. ParsedQuality(origin is
string).value() is Quality parses the bounded ASCII qvalue grammar.
WeightedValue(value is string[,quality is Quality]) validates one HTTP token,
default weight 1; value() is string, quality() is Quality, string() is weighted
wire text. Token spelling is retained; profile-specific matching is separate.
InvalidQuality(reason) and InvalidWeightedValue(reason) implement Error with
reason()/text() observations. Field-specific consumers follow below.

AcceptedContentCodings() is absent; (origin is string[,limits is NegotiationLimits])
is present, including empty. present() is bool, values() is Sequencing<WeightedValue>,
string() is canonical field text (MissingNegotiationField on absence),
quality(coding is ContentCoding) is Quality, preference(available is
Sequencing<ContentCoding>) is a supplied ContentCoding or NoAcceptableValue.
NegotiationLimits([bytes,entries,available]) observes all three int budgets,
default 8192/128/128. InvalidNegotiation(reason) has reason()/text();
NegotiationLimitExceeded(kind) has kind()/text(); NoAcceptableValue() and
MissingNegotiationField() have text(); all implement Error. CodingPreference
(token,quality,position) with token()/quality()/position()/order(), CodingIndex,
CodingField, CodingMember and persistent CodingChoice are importable indexing,
parsing and selection infrastructure. All four bounded preference profiles are verified; response effects remain protocol work.

MediaRange() represents */* with quality 1; (origin is string[,limits is
HttpMediaLimits]) parses a weighted range; (type,subtype,parameters is
HttpMediaParameters,quality is Quality) constructs validated coordinates.
type()/subtype()/string() return canonical strings, parameters() returns
HttpMediaParameters, quality() returns Quality, specificity() returns
MediaSpecificity, matches(media is HttpMediaType) is bool.
MediaSpecificity(kind,parameters) does Comparing<MediaSpecificity>; both are
nonnegative int coordinates, kind 0..2; kind()/parameters()/order() observe them.
AcceptedMediaTypes() is absent; (origin is string[,limits is MediaNegotiationLimits])
is present including empty. present()/values()/string() observe field identity;
values() is Sequencing<MediaRange>. quality(media is HttpMediaType) is Quality;
preference(available is Sequencing<HttpMediaType>) returns only a supplied type or
NoAcceptableValue. MediaNegotiationLimits([bytes,entries,parameters,available])
observes four int budgets, default 8192/128/32/128. The same typed field failures
apply. Most-specific matching sets candidate quality before selection compares
positive quality, specificity, client order and server order. Infrastructure:
ParsedMediaRange.value(), MediaMatch.following()/quality()/specificity()/position(),
MediaChoice.following()/value(), MediaField.values(), MediaMembers.values() and
persistent quote-aware MediaMemberPosition.following()/values().

CharsetName(origin is string) does Comparing<CharsetName>; string() canonicalizes
ASCII case and order(other) compares spelling. Wildcard is not concrete.
AcceptedCharsets() is absent; (origin is string[,limits is NegotiationLimits])
is present including empty. present()/values()/string() observe the field;
values() is Sequencing<WeightedValue>, quality(charset is CharsetName) is Quality,
preference(available is Sequencing<CharsetName>) returns a supplied CharsetName
or NoAcceptableValue. No implicit legacy/identity preference or registry aliases.
WeightedTokenLookup(origin is Sorted<CodingPreference>,token is string).value()
is @CodingPreference of length 0/1. CharsetChoice.following()/value() are persistent
selection infrastructure. Typed grammar/budget errors are the existing field
errors. This consumes explicitly provided deprecated Accept-Charset values;
no automatic network field emission.

WeightedLanguageRange(range is BasicLanguageRange[,quality is Quality]) observes
range()/quality()/string(). AcceptedLanguages() is absent; (origin is string
[,limits is NegotiationLimits]) is present including empty. present()/values()/string()
observe the field; values() is Sequencing<WeightedLanguageRange>, quality(locale
is Locale) is Quality, preference(available is Sequencing<Locale>) is a supplied
Locale or NoAcceptableValue. Longest matching basic range determines quality;
candidate ties use quality, subtag specificity, client order and server order.
No implicit locale expansion. Infrastructure: LanguageField.values(),
LanguageMatch.quality()/specificity()/position(), LanguageChoice.following()/value().

## `locale/match`

[Contract](locale-match-guide.md). BasicLanguageRange(origin is string) validates
bounded RFC 4647 basic range syntax; string() canonicalizes ASCII case,
specificity() is int subtag count (wildcard zero), matches(locale is Locale) is bool.
LanguagePrefixes(origin is Locale).values() is Sequencing<BasicLanguageRange>,
most specific first. InvalidLanguageRange(offset,reason) does Error and observes
offset()/reason()/text(). BasicRangeSubtag is grammar/case infrastructure.
This consumes complete canonical Locale candidates with literal RFC 4647 ranges.
Lookup/filtering are implemented separately; alias-normalize complete requested
tags explicitly before forming a range when desired. No host locale effects.

## `http/media_type`

[Contract](http-media-type.md). HttpMediaType(origin is string[,limits is
HttpMediaLimits]) or (type is string,subtype is string,parameters is
HttpMediaParameters) is a concrete HTTP value; type()/subtype() are canonical
strings, parameters() is HttpMediaParameters, string() is canonical field text.
HttpMediaParameter(name,value) does Comparing<HttpMediaParameter> by name;
name()/value()/string() observe its validated association. HttpMediaParameters
(values is Sequencing<HttpMediaParameter>[,maximum is int]) observes entries()
as Sequencing, membership(name) bool, first(name) string, string() field parameters.
HttpMediaLimits([bytes is int,parameters is int]) observes both budgets, default
8192/128. InvalidHttpMediaType(offset,reason) observes offset()/reason()/text();
HttpMediaLimitExceeded(kind) observes kind()/text(); MissingHttpMediaParameter(name)
observes name()/text(); all implement Error.
Infrastructure: HttpMediaSnapshot.values(), HttpMediaScanning(origin,limits).value()
is HttpMediaFields, HttpMediaFields.type()/subtype()/parameters()/wire_parameters(),
HttpMediaWireParameter.parameter()/quoted()/offset(), HttpMediaScanned.text()/end(),
HttpMediaPosition.index()/slots()/following()/separator(), HttpMediaItems<T>
following()/values(), HttpMediaOctet(code).token() bool.


## `http/cookie`

[Contract](http-cookie.md). CookieExpiry() is a session expiry; (origin is Instant)
is persistent with the supplied exact immutable instant. persistent() is bool;
instant() is Instant or MissingCookieExpiry for a session cookie.
MissingCookieExpiry() does Error and observes text(). No current-time query,
wire-date truncation or sentinel instant.
CookieFlag=(secure,http_only,partitioned); SameSite=(unspecified,strict,lax,cross_site).
CookieAttributes() or (domain,path,expiry,same_site,flags is Set<CookieFlag>) observes
domain()/path() strings, expiry() CookieExpiry, same_site() SameSite, flags() Set,
string() explicit wire suffix. Cookie(name,value,attributes) observes name()/value(),
attributes(), pair()/string() wire and diagnostic() redacted text.
InvalidCookie(reason) does Error with reason()/text(). See contract for bounded
grammar, flag snapshot, canonical domains and case-insensitive prefix policy.
Cookie parsing/context/storage APIs remain separate queue items.

## `http/cookie_date`

ParsedCookieDate(origin is string).value() is Instant; InvalidCookieDate(reason)
does Error with reason()/text(). Bounded cookie-date token grammar, fixed two-digit
centuries and UTC; no weekday assertion or ambient locale/clock. [Contract](http-cookie.md#cookie-date-parser).

## `http/cookie_policy`

ParsedSetCookie(origin is string,url is Url,reference is Instant).value() is Cookie.
CookieContext(site is Url,top_level is bool,method is Method) observes site(),
top_level(), method(), key() string. CookieOrigin(origin is Url) observes url(),
domain()/path()/site(), address()/secure(). CookieDefaultPath(origin).string();
CookiePathMatch(cookie,request).matches(); CookieDomainMatch(cookie,request,address).
matches(). [Contract](http-cookie.md#set-cookie-and-caller-context): bounded strict
HTTP consumer profile, pinned PSL, explicit reference and context; no I/O.

## `http/cookie_jar`

CookieStoring: cookies(origin is Url,context is CookieContext) is Sequencing<Cookie>;
store(origin,headers is Headers,context) is void; clear() is void.
MemoryCookieJar(clock is Timing[,limits is CookieLimits]) does CookieStoring, Closing;
also size() int, limits() CookieLimits, closed() bool, close() void.
CookieLimits() or (total is int,per_domain is int) observes total()/per_domain().
CookieJarClosed() does Error with text(). [Contract](http-cookie.md#memory-jar)
defines expiry/replacement, caller context, deterministic eviction and publication.
SavedCookieJar(origin is MemoryCookieJar,destination is FileEntry).result() is FileEntry;
LoadedCookieJar(origin is FileEntry,clock is Timing).result() is MemoryCookieJar.
InvalidCookieStore(reason) does Error with reason()/text(). Explicit versioned
ASCII persistence, private staging, exact deadlines and session exclusion:
[contract](http-cookie.md#explicit-persistence).


## `net/endpoint` host and `net/public_suffix`

[Contract](host-policy.md). HostName(origin is string) does Comparing<HostName>;
ascii()/unicode() return canonical strings, order(other) compares ASCII identity.
HostEndpoint(host is HostName,port is int) observes host()/port()/string(); port is
0..65535. InvalidHostName(origin) does Error with origin()/text().
PublicSuffixList() observes pinned version(); suffix(origin is HostName) and
registrable(origin is HostName) return HostName; public(origin) is bool.
MissingRegistrableDomain(origin) does Error with host()/text(). Full ICANN and
PRIVATE policy uses 10336 pinned canonical rules. No resolver/transport/live
update or ambient database. Private runtime/idna and runtime/public_suffix native
functions expose only validated string conversions and portable failures.


## `url/value` full URL API

[Contract](url-value.md) supersedes unimplemented v3 sketches. UrlParts(scheme is
string,authority is UrlAuthority,path is string,query is UrlQuery,fragment is
UrlFragment) observes all five canonical coordinates and string(). Url(parts)
requires a scheme; parts()/string()/url(reference is UrlReference) observe/resolve.
UrlReference(parts) additionally observes absolute(). ParsedUrl(source).value()
and ParsedUrlReference(source).value() are deferred typed parsers.
UrlHost role: kind() UrlHostKind(empty/name/address/future), name() HostName,
address() IpAddress, string(). Implementations: NamedUrlHost(HostName),
AddressUrlHost(IpAddress), FutureUrlHost(raw string), EmptyUrlHost(). Wrong
projections throw UrlHostIsNotName/UrlHostIsNotAddress (Error.text()).
UrlAuthority(), (host), (host,port int), or (host,userinfo UrlUserInfo,port UrlPort)
observes present()/host()/userinfo()/port()/port_marker()/string().
ParsedUrlAuthority(source).value() returns present authority. UrlUserInfo() or
(source) observes present()/string(); UrlPort() or (decimal text) observes
present()/value()/string(). Empty assigned delimiters remain distinct.
MissingUrlAuthority/MissingUrlPort implement Error with text(). InvalidUrl adds
reason() alongside offset()/text(). Infrastructure: UrlHostSnapshot.value(),
UrlAsciiText.characters()/lower(), UrlSchemeText.string(), UrlDotSegments.value()
and UrlResolution.value(). Bounded native dot text is private; no native URL object.

## `http/target`

RequestTargetKind=(origin,absolute,authority,asterisk); role RequestTarget declares
kind(), path() string, query() UrlQuery, url() Url, authority() UrlAuthority, string().
OriginTarget(path,query) or (origin is UrlReference); AbsoluteTarget(origin is Url);
AuthorityTarget(origin is UrlAuthority); AsteriskTarget(). WrongRequestTargetKind
(expected,actual) observes those kinds/text(); InvalidRequestTarget(reason) reason()/text().
ParsedRequestTarget(origin is string,method is Method).value() RequestTarget;
RequestTargetSnapshot(origin is RequestTarget).value() RequestTarget;
RequestTargetMethod(method,kind).valid() bool. [Contract](http-target.md) explains
8192-byte bounds, method rules, unavailable projections and origin //path semantics.

## `http/version`

HttpVersion=(http_1_0,http_1_1,http_2); Http1VersionText(origin).string();
ParsedHttp1Version(origin is string).value() HttpVersion; UnsupportedHttpVersion()
does Error with text(). Exact HTTP1 wire tokens; negotiation remains separate.

## `http/message`

Request(method is Method,target is RequestTarget,headers is Headers[,body is
BodyOpening[,version is HttpVersion]]) observes method()/target()/headers()/body()/version().
Response(status is Status,headers is Headers[,body[,version]]) observes status()/
headers()/body()/version(). Defaults: EmptyBody and HTTP/1.1. InvalidHttpMessage
(reason) does Error with reason()/text(). [Contract](http-message.md) defines role
snapshots, no body observations during construction and consumer framing duties.

## `http/http1_head`

Http1Limits() or (head_bytes,line_bytes,fields,body_bytes) observes those budgets.
ParsedHttp1RequestHead(origin is string[,limits]).value() Http1RequestHead;
ParsedHttp1ResponseHead(origin is string[,limits]).value() Http1ResponseHead.
Request head: method()/target()/headers()/version()/host(); request(body) Request.
Response head: status()/headers()/version(); response(body) Response.
Http1LimitExceeded(kind) kind()/text(); MissingHttp1Host() text().
[Contract](http1-framing.md) separates actual head wire budgets, logical Headers,
body assembly and subsequent framing/transport duties.

## `http/http1_reading`

Http1RequestHeadReading/Http1ResponseHeadReading(origin is AsyncReading[,prefix is @byte])
or (origin,prefix,limits is Http1Limits,chunk_bytes is int). result() is one memoized
Future<Http1HeadRead<Http1RequestHead/Http1ResponseHead>>. Http1HeadRead observes
head(), remainder() is @byte and consumed() is int. Lazy borrowed source, finite
wire bounds, exact tail, scoped child cleanup; started failure consumes session.
[Contract](http1-framing.md#asynchronous-borrowed-head-reading).

## `http/http1_framing`

Http1BodyKind=(empty,fixed,chunked,close_delimited,tunnel,upgrade).
Http1BodyPlan(kind,count is BigInt,reusable is bool): kind()/length()/reuse_after_eof().
Http1RequestFraming(head[,limits]).plan(); Http1ResponseFraming(head,method[,limits]).plan().
UnsupportedTransferCoding() text(). Exact BigInt boundaries, method/status precedence,
strict ambiguity rejection and conditional persistence; [contract](http1-body-framing.md).

## `http/http1_chunk`

ParsedHttp1ChunkLine(origin is string[,limits is Http1Limits]).size() is BigInt;
ParsedHttp1Trailers(origin is string,head is Headers[,limits]).value() is Headers.
Bounded token/quoted extension grammar, ignored safe unknowns, strict CRLF trailers
and prohibited field policy. [Contract](http1-body-framing.md#chunk-lines-and-trailer-sections).

## `http/http1_body`

Http1BodyReading(origin is AsyncReading,owner is Closing,plan is Http1BodyPlan,
prefix is @byte,head is Headers[,limits is Http1Limits]) does BodyReading:
async bytes(limit), trailers(), closed(), close(); concrete remainder() requires clean EOF.
Http1IncomingBody(origin is Http1BodyReading) does BodyOpening: length(),
replayable() false, reading() once. Explicit owner closes on EOF/abort; framing
plan eligibility is not a pool lease. [Contract](http1-body-reading.md).

## `http/http1_writing`

Http1RequestHeadWriting(origin is AsyncWriting,head is Http1RequestHead[,limits]);
Http1ResponseHeadWriting(origin,head is Http1ResponseHead,method is Method[,limits]).
result() is memoized Future<AsyncWriting>. Canonical exact wire limits/framing
validate before any sink effect; borrowed sink, scoped started cancellation.
[Contract](http1-framing.md#canonical-borrowed-head-writing).

## `http/http1_body_writing`

Http1BodyWriting(origin is AsyncWriting,body is BodyOpening,plan is Http1BodyPlan,
head is Headers) or (origin,body,plan,head,limits is Http1Limits,chunk_bytes is int).
result() is memoized Future<AsyncWriting>; lazy source opening, bounded chunks,
exact fixed length/EOF and separate validated trailers. Current read/write groups
remain bounded and cancellation drains both. [Contract](http1-body-writing.md).

## `http/http1_message_writing`

Http1RequestWriting(origin is AsyncWriting,request is Request[,limits]);
Http1ResponseWriting(origin,response is Response,method is Method[,limits]).
result() is memoized Future<AsyncWriting>. Lazy known-body preflight precedes
any head emission; scoped head/body writes inherit bounded streaming/cleanup.
[Contract](http1-message-writing.md).

## `http/http1_session`

[Contract](http1-session.md). `Http1Session(input is AsyncReading, output is
AsyncWriting, owner is Closing[, limits is Http1Limits])` does Closing;
The full constructor additionally accepts `continue_wait is Duration, clock is Timing`
after limits; default waiting is one second with SystemClock.
`async exchange(request is Request) is Http1Exchange`. Http1Exchange exposes
`request() is Request`, `response() is Response`,
`informational() is Sequencing<Http1ResponseHead>`. Http1SessionClosed does Error.
Response BodyOpening is one-shot and holds admission until clean EOF; startup
groups are cleared before publication. Scope owns the supplied transport;
explicit close retries a still-open owner after a failed closure. No routing,
TLS/ALPN metadata, automatic retry or pool is fabricated.
[Continue coordination](http1-continue.md) uses an absolute waiting deadline,
opens a source only after 100 and rejects reuse after an early final response.

`http/http1_body_check` and `http/http1_continue` contain internal shared preflight,
request-start and monotonic continuation state. Their construction captures
capabilities; preflight source observations occur only when sending starts.

## `http/client_policy`

[Contract](http-client-policy.md): HttpLimits with exact BigInt body budget; ClientTimeouts with independent nonnegative durations (zero disables); ClientPolicy with bounded immutable protocol snapshot and explicit endpoint policy. Http1ClientProtocols rejects incompatible ALPN or unsupported protocol engines before effects.

## `http/client`, `http/client_request`, `http/exchange`

[Direct HTTP1 contract](http-client.md): Exchanging; HttpClient(Resolving,TlsClientPolicy,ClientPolicy,Timing) does Closing, async response(Request)ClientResponse. HttpDirectRequest exposes URL and derived origin-form request. ClientResponse/ExchangeHop/ExchangeDetails retain original request, response, history and actual endpoint/TLS metadata; PlainHttpHasNoTls, ClientClosed, HttpTimeout are typed errors. Http1Connecting.acquisition() exposes Http1Acquisition while session() stays compatible.

## `http/redirect`

[Contract](http-redirect.md): RedirectPolicy([maximum is int,https_downgrade is bool]); Redirecting(Exchanging,RedirectPolicy).response(Request)ClientResponse. TooManyRedirects, RedirectLoop, NonReplayableRedirect, UnsafeRedirect. Method-aware canonical loop coordinates allow same-URI POST-to-GET. Intermediate bodies are disposed, final ownership/history retained.

## `http/cookie_client`

[Contract](http-cookie-client.md): CookieContextualizing.context(Request)CookieContext; FirstPartyCookies; CookieExchanging(Exchanging,CookieStoring,CookieContextualizing[,CallerCookiePolicy]).response(Request)ClientResponse. CallerCookiePolicy reject/preserve/replace; CallerCookieConflict/CookieExchangeLimit. Select per hop by placing Redirecting outside this adapter.

## `crypto/secret` and scoped `http/auth` - provider gate PASS

[SecretBytes/ExportedSecret](secret-bytes.md): bounded owned opaque memory and explicit copying export; shared close and no ordinary equality/bytes/text. [Basic/Bearer auth](http-auth.md): BasicCredentials.username(), explicit wire authorization capabilities and scoped Exchanging consumers. ASCII Basic and Bearer only; Digest/challenge charset negotiation remain open. Full frozen-source regression through interpreter/473 and runtime/52: 672 files / 12185 assertions PASS.

## `http/http2_settings`

[Wire foundation](http2-wire-foundation.md): Http2Settings([header_table,initial_window,frame_size,streams]) with explicit local limits and zero capacities. Frame/settings-wire/reader collaborators remain internal and are not the stable public API. An H2 session/client is still open.

### HTTP2 internal preface and peer settings

[Client preface and peer SETTINGS](http2-preface.md) verified with lazy exact wire emission, sequential peer observations, ACK, cancellation cleanup and one real loopback TCP example. Focused 477/478/479: six/ten/three assertions. Std-only; no repeat full regression. Public H2 exchanges, live streams/HPACK/flow and outgoing settings ACK tracking remain open.

### HPACK bounded wire primitives

[Integer/string/Huffman primitives](hpack-primitives.md) verified with all RFC octet codes and provider bounds: 480/runtime53 eight/thirteen assertions, inline/example PASS. Measured slow per-bit std decoding moved to a narrow native bulk provider (PERF-22). Full regression PASS: 680 files / 12,266 assertions / 1084 seconds; table/block/header semantics remain the next slice.

### HPACK tables and compression contexts

[Bounded tables and blocks](hpack-blocks.md) verified: complete static/dynamic decoding, exact context across RFC request/response sequences, minimum table shrink and conservative never-indexed encoder. 481/482 four/ten assertions; combined compatibility thirty-five PASS, inline/example PASS. Std-only after native full gate; no repeat full run. Live H2 transport remains open.

### HTTP2 HEADERS and typed field metadata

[Block and field mapping](http2-headers.md) verified: exact continuation sequence, cancellation, preflighted fragmentation, pseudo-field validation and normalized Host/authority agreement on input/output without body observations. 483/484/485 nine/nine/three assertions; combined 480..485 six files / forty-three PASS, inline/real TCP example PASS. Std-only; no repeat full gate. Live flow/streams/ALPN remain open.

### HTTP2 flow accounting

[Signed window/DATA accounting](http2-flow.md) verified: padded payload charge, negative SETTINGS credit, scoped overflow, dual-window send preflight and retained connection charge on stream error. 486 eight cases; compatibility twenty-one PASS, inline/example PASS. Std-only, no full rerun. Async credit/scheduler/deadlock prevention remain open.

### HTTP2 control values

[Reset/PING/GOAWAY metadata](http2-controls.md) verified: exact unknown codes, no ACK echo loop, monotone admission stop and explicitly opaque diagnostics. 487 five cases; compatibility twelve PASS, inline/example PASS. Std-only, no full rerun. Live stream/connection effects and retry remain open.

### HTTP wire byte serialization

[Bounded internal bulk joins](http-wire-bytes.md) replace per-byte maps through the existing buffer provider; exact 16 KiB DATA/HPACK payloads verified. PERF-23 records about 2.59x fixture improvement with residual VM traversal open. Std-only; no full rerun.

### HTTP2 stream lifecycle

[Half-close/reset/ID state](http2-stream-life.md) verified: independent stream states, final accepted payload distinction, minimal late-frame processing and odd uint31 exhaustion. 489 five cases; compatibility ten PASS, inline/example PASS. Std-only, no full rerun. Actual multiplexing/sibling physical reset remains open.

### HTTP2 consuming connection reader

[Internal input loop](http2-input.md) routes complete fields/DATA/control frames, retains HPACK before stream validation, tracks initial local ACK and queues replies without blocking on output. 490 eight plus 483 nine assertions PASS; nested continuation cancellation settles. Std-only, no full rerun. Owning writer/flow/public H2 client remain open.

### HTTP2 connection writer

[Bounded output and tickets](http2-output.md) serialize whole header blocks and isolate producer cancellation from physical writes. 491 eight cases; 483/490/491 compatibility 25 PASS after common preflight refactor. Std-only, no full rerun. Fair DATA/flow scheduling and physical session integration remain open.

HTTP2 duplex integration: `examples/std-v4/http2-duplex.ok`, 492 three actual TCP assertions PASS. Reader/writer run concurrently, responses reverse stream order, padding charges and PING ACK verified. Std-only, no full rerun; live body/flow/client work follows.

### HTTP2 outbound DATA

[Sender scheduler](http2-sending.md) separates reservation/wire debit, waits outside the writer, queues one frame per producer, handles SETTINGS/credit notifications and retires closed slots. 493 eleven cases; flow/writer/actual TCP compatibility 30 PASS after cleanup refactor. Std-only, no full rerun. Incoming body flow and public session remain open.

### HTTP2 incoming body buffer

[Body storage/reading](http2-body.md) bounds storage, credits padding/consumption separately, preserves final DATA and validates length/trailers. 494 eleven PASS; shared trailer refactor compatibility 76 PASS. No runtime changes/full rerun. Aggregate receiver/session ownership follow. Fresh probe startup measured separately in PERF-24.

### HTTP2 aggregate incoming flow

[Receive ledger](http2-receiving.md) preserves connection debit on stream failure, credits discard once, separates request registration/response attachment and retires body storage at application EOF. 494/495/496: 22 PASS, including real TCP WINDOW_UPDATE after app reads. Std-only, no full rerun; full owned session/client/ALPN remain open.


### Owned HTTP2 session

[Owned session](http2-session.md): `Http2Session(input,output,owner,local,peer,limits)` performs typed exchanges on one previously selected H2 transport. Supervised stream holders preserve native bodies across reverse response arrival, per-stream cancellation/reset, GOAWAY and physical close. Completed buffered bodies remain readable until application EOF/close/drop. Final compatibility: nine files / 79 assertions PASS, including actual multiplexed TCP with PING and exact consumption credit. Interpreter cancellation races have independent red/green evidence; the required frozen-source full regression passed: 699 files / 12455 assertions, 1125 seconds, unchanged 1324-file source inventory. H2 HTTPS/opening/client policy and the remaining HTTP roadmap are still open.


### HTTP2 verified HTTPS and shared client pool

[Opening](http2-connecting.md) verifies URL-derived certificate identity/SNI and actual h2 ALPN before preface. An explicit H2 policy in the normal [HttpClient](http2-client.md) enables shared physical owners, multiplexed leases, stream-local abandonment/body deadlines, checked-peer reuse and GOAWAY replacement. Final root H2 ten files /83 assertions PASS; unchanged H1 client/decorator compatibility eleven files /89 PASS. Interpreter ownership correction full gate: 700/12465 PASS; std-only client changes use focused gates. Mixed ALPN, replay-safe retry and remaining HTTP modules stay open. PERF-25 records visible continuation-depth warnings and an open investigation.


### Mixed HTTP1/H2 ALPN and shared conversation admission

[Negotiation contract](http-alpn.md): HttpClientProtocols validates exact enabled/advertised agreement; HttpSelectedConnecting adopts one checked verified transport according to actual ALPN. HttpClient supports a mixed H1.1/H2 policy; absent ALPN selects enabled H1, ordinary cleartext uses H1, H2-only remains explicit prior knowledge. The common physical pool serializes actual H1 body leases and multiplexes H2. Unsent queued requests are re-admitted after conversation retirement; emitted requests are not replayed. Whole admission tasks are supervised so client close cancels queues before releasing references. Final H1/H2/decorator/mixed compatibility: 28 files /240 assertions PASS. Std-only; the last interpreter full gate stays 700/12465 PASS. Replay-safe stale retry and remaining HTTP modules follow.


### Replay-safe stale HTTP1 retry

[Optional Retrying](http-retry.md) performs at most one replacement exchange for
an idempotent request with a replayable body after zero-response-byte failure on
a reused H1 conversation. Partial/1xx bytes, fresh errors, POST, one-shot bodies,
timeout/cancellation and broad TLS failures do not qualify. Direct session errors
remain compatible. Real TCP, mixed H1 and verified close_notify TLS are covered;
affected H1/H2/decorator compatibility: 33 files /295 assertions PASS. Std-only,
no repeated full regression. H2 unprocessed replay and other HTTP modules remain
open.


### Scoped Digest SHA-256

[Digest profile](http-digest.md) supports SHA-256/SHA-256-sess, qop=auth, userhash,
fresh secure cnonce, atomic bounded nonce counts, direct HTTPS scope, one bounded
stale refresh and optional header/trailer rspauth verification. Generic received
challenge parsing handles combined schemes, token68, quoted commas and bounded
empty list members. Final client/decorator/secret/Digest compatibility: 18 files
/144 assertions PASS; inline and independent actual HTTPS CLI PASS. No interpreter
or provider code changed, so no full regression repeated. SHA-512/256, NFC
credentials, auth-int and preemptive nonce caching remain open.


### Digest SHA-512/256 and UTF-8/NFC

[Modern Digest profile](http-digest.md) adds SHA-512-256/-sess, explicit UTF-8
credential NFC normalization, userhash/username* and exact UTF-8 realm wire
handling. Existing ASCII profiles remain covered. Correct algorithm vectors
agree with independent OpenSSL; RFC7616's truncated-SHA-512 example has a
reported erratum and is documented. Final focused 3/35 PASS; required native
full gate 727 files /12773 assertions PASS, unchanged 2079-file inventory.
Auth-int and preemptive nonce caching remain open. Proxy work is separate.

### Explicit HTTP proxy routing

[Proxy contract](http-proxy.md): explicit ProxyPolicy/Proxied and EnvironmentProxy;
absolute HTTP forwarding, HTTPS CONNECT with verified origin TLS/H1/H2, separate
hop credentials, actual proxy endpoint access/reuse checks and bounded no_proxy.
Deadline/cancellation drain real CONNECT transport. Final distinct focused coverage
37 files /339 assertions PASS, inline PASS. Interpreter unchanged; no full rerun.
TLS to proxy itself/SOCKS remain separate; PERF-26 is investigated and open.

### HTTP cache metadata foundation

[Bounded cache metadata](http-cache-metadata.md) implements Cache-Control and
private-cache freshness/age over explicit wall receipt and monotonic elapsed
coordinates. Five files /90 assertions and inline PASS. Std-only; no full rerun.
Body storage, Vary, revalidation and concurrent collapse remain open.

### Completed private HTTP cache storage

[Memory and streaming consumer](http-cache.md) implement bounded completed GET200
storage, Vary/auth/cookie partitioning, newest-Date selection, independent bodies
and unsafe invalidation with a late-GET barrier. Six files /40 assertions PASS,
including actual TCP. Std-only; no full rerun. Revalidation/collapse remain open.

### HTTP cache conditional validation

[Private cache](http-cache.md) now validates ETag/Last-Modified, merges identified
304 metadata over complete bytes, constrains request freshness and handles
only-if-cached without network effects. Original no-store and hop-field rules
remain effective through sanitization. Seven distinct files /43 assertions PASS;
std-only/no full. Concurrent collapse and stale permissions remain open.

### Cache admission and immutable header names

[Admission](http-cache.md) collapses ordinary concurrent validation and retains
full-body publication ownership; explicit waiter cancellation retires its key.
Fresh hits bypass hashing and immutable HeaderName canonicalization is memoized.
58 targeted HTTP files /523 assertions PASS; 2112 source hashes unchanged. No
interpreter/native change/full rerun. PERF-27/28 report remaining performance work.
Explicit stale permission and concurrent no-cache sharing are the next slice.

### Private cache final policy slice

[Private GET200 profile](http-cache.md) adds constrained max-stale and shares a
current no-cache validation only with already waiting bookings. Publication is
revoked across invalidation. Eleven files /59 assertions PASS, actual TCP and
secondary cleanup included; no interpreter/native change/full rerun.

### WebSocket value foundation

[Typed values](websocket.md) implement text/binary projection errors, explicit
message/frame/queue limits and UTF-8-sized wire close values with pinned IANA codes.
Four assertions and inline PASS; std-only/no full rerun. Framing/transport follow.


### Owned database stack — 2026-10-13

[SQLite contract](db-sqlite.md) and [pool contract](db-pool.md) pin the implemented
API; v3 role/object and two-existing-connections backup candidates are superseded.

| Module | Capabilities and acquisition |
| --- | --- |
| db/connection | DbExecuting(execution,rows); DbConnection adds transaction,closed,close,settled; DbConnecting.connection |
| db/transaction | Isolation; TransactionOptions; Transaction execution/rows/commit/rollback/active |
| db/prepared | DbPreparing.prepared; Prepared(origin,text).statement; PreparedStatement execution/rows/closed/close |
| db/sqlite | SqliteOptions; SqliteMemory/SqliteDatabase.connection return SqliteConnection; SqliteConnector adapts file database to portable DbConnecting |
| db/sqlite | SqliteTransactions(connection,mode).transaction returns concrete SqliteTransaction; deferred/immediate/exclusive modes |
| db/sqlite/blob | SqliteBlobOpening; OpenedSqliteBlob(origin,database,table,column,row,writable).blob; SqliteBlob bytes/write/size/closed/close |
| db/sqlite/script | SqliteScripting; SqliteScript(origin,source).executions |
| db/sqlite/backup | SqliteBackupMaking; SqliteBackup(source,targetPath,pages,clock,timeout).result; BackupReport copies/total |
| db/pool | PoolOptions; DbPool(origin,options,clock[,health]); PooledConnection; PoolSnapshot; opt-in DbChecking |
| db/postgres | PostgresDatabase(transport,credentials,options,entropy).connection returns PostgresConnection (DbConnection, DbPreparing, DbPreparedOpening, server_version); PostgresTcp/PostgresTls transports; PostgresCredentials; PostgresOptions(database[,application,md5,cleartext,batch]); PostgresConnector (DbConnecting), PostgresAcquiring (DbAcquiring) — [guide](db-postgres.md) |
| db/postgres/wire | PgFrame/PgUntypedFrame/PgInt16/PgInt32/PgCString encoders; PgInput.message/buffered, PgOutput.send, PgFields readers, PgJoined, PgWireFailure |
| db/postgres/auth | ScramExchange(user,password,nonce) client_first/client_final/server_verified; Md5Password(user,password,salt).string; ScramFailure |

The concrete SQLite capability composes separate roles. Passing DbConnection does
not reveal preparation/BLOB/script/backup actions. Runtime/database opaque values
are private provider ABI, not public SQL/domain objects. Normal close is immediate
revocation; settled joins resource cleanup. File/memory independence, transaction
outcome and exact budgets are part of the linked contract.


The std composition follow-up adds `db/acquisition`: DbAcquiring.acquisition returns
DbAcquisition with connection/preparing/transactions facets. PortableDbAcquiring
adapts an ordinary DbConnecting without fabricating extra capabilities.
DbPreparedOpening prepares directly or acquires a DbPreparedTransaction; the latter
implements Transaction and DbPreparing. SqliteAcquiring supplies these facets.
DbPreparedPool uses the same FIFO/reset state as DbPool. PooledTransactions(lease)
returns concrete PooledTransaction for Prepared(transaction,text). Transaction end
and lease return revoke the corresponding statements and buffered readers.

## Application / configuration / CLI / terminal

The [v4 contract](application-cli.md) supersedes historical Stage24 candidates.
Implemented modules: cli/arguments, config/key, config/source, config/value,
config/environment, config/document, cli/spec, cli/parse, cli/help,
terminal/capability, terminal/width, terminal/style, terminal/session,
terminal/mode, terminal/screen, terminal/prompt and the runtime/application boundary.

ProcessArguments and Utf8Arguments are bounded immutable sequences. Settings and
LayeredSettings implement confined SettingReading; SettingEntry retains exact
segments, repeated strings and winning provenance. EnvironmentBinding is an
ordered explicit name-to-key mapping. JSON/TOML flatten through sealed nominal
visitors, and INI uses two literal segments. No document-format inference or
process mutation occurs.

Settings retains an exact segment-key index for duplicate validation and binary
lookup, preserving source order, winning provenance and the earliest original
duplicate. Environment mappings validate with the same key identity. CLI child
name uniqueness sorts only auxiliary names; public declaration order is intact.
Layering uses a persistent balanced index, preserving first appearance and last
complete replacement. SettingOverlay supplies SettingReading for direct
membership/entry; CLI avoids rebuilding Settings for each value option. Final
publication retains original order and has a separate O(n log n) cost. CommandSyntax caches exact aliases for binary lookup and validates segment keys
through sorted indexes. option_index(name)/flag_index(name) return original
per-group positions or -1. Declaration ordering and first-error priority stay
unchanged; preparation uses O(A log A + D log D) comparisons.

CliCommand/CommandSyntax/Option/Flag/Operand describe a validated immutable tree.
ParsedCommand returns Invocation without I/O. Usage emits deterministic grapheme-
wrapped plain text; ShellCompletion supplies static quoted bash/zsh/fish declarations.
TerminalSession owns host state; TerminalModeApplying, TerminalScreenApplying and
TerminalResizeOpening restrict consumers to the capability they actually need.
Session child capabilities are revoked at close/drop; prompts and resize waits
are async. Host flags and raw ANSI construction are absent from public std roles.
[Example](../../examples/std-v4/application-cli.ok) and
audit record validation and platform limits.

## Stage25 logging implementation (2026-10-13)

[Contract and budgets](logging.md) · delivery audit.
No runtime/logging provider is introduced; runtime/process exposes the existing
process_current_id() is int symbol. The module-specific Error declarations and
infrastructure helpers are described in source; applications use the capabilities
below. Inventory is implementation evidence, not a runtime certification claim.

| Module | Constructors / roles | Public behavior |
|---|---|---|
| logging/level | LogLevel(debug,info,warning,error,critical); LogLevelName(level); LogLevelFromName(text); LogLevelOrder(left,right) | string(); projected value() returns LogLevel /Order |
| logging/name | LogName(Sequencing<string>); ChildLogName(name,segment) | segments(), string(), order(other); projected child value() |
| logging/field | LogFields(Sequencing<JsonField>); EmptyLogFields(); CombinedLogFields(base,added) | membership(name), value(name), values() is Json, members(); projected combined/empty values |
| logging/record | LogContext(name,fields,process); LogRecord(time,level,message,context[,cause]) | context coordinates; time(), level(), message(), context(), has_cause(), cause() |
| logging/write | LogWriting; DiscardingLogs(); CollectedLogs([capacity]); BroadcastLogs(destinations); FallbackLogs(primary,fallback); BestEffortLogs(origin) | write(record); collector size()/records(); best-effort failures() is BigInt |
| logging/named | Logging; NamedLogging(name,destination,clock,fields[,process_reader]); LogProcessReading /CurrentLogProcess | three log overloads; named name()/child(segment); process id() |
| logging/enrichment | FieldLogging(origin,fields); ChildLogging(named,segment) | log overloads; projected child value() |
| logging/filter | LogAllowing; LevelOverride(name,level); LevelThresholds(fallback,Sequencing<LevelOverride>); AllowedLogs(origin,allowing); EnabledLog(allowing,name,level) | allowed(name,level); write(record); projected enabled value() |
| logging/redact | RedactedLogs(origin,Sequencing<string>,Json); RedactedCauseLogs(origin,text) | write(record) with exact field/public cause replacement |
| logging/line | TextLogLine(record,TimeZone); JsonLogLine(record) | string() with no ending, bounded one-line representation |
| logging/stream | TextStreamLogs(Writing,zone); JsonStreamLogs(Writing); LogStyle(level,TextStyle); TextTerminalLogs(output,zone,styles); JsonTerminalLogs(output,styles) | write(record), exactly one UTF8+LF submission; explicit terminal policy |
| logging/file | FileLogOptions(permissions,flush_each,sync_each); LogFile(Path,options) | permissions()/flush_each()/sync_each(); owner write/flush/reopen/closed/close |
| logging/rotation | Rotating; SizeRotating(limit); IntervalRotating(interval,zone); RotatingLogFile(path,options,rotation,backups,clock) | needed(size,opened,now), suffix(now,index); owner write/flush/reopen/closed/close |
| logging/queue | LogDraining; LogDelivering; LogQueueOptions(capacity,overflow); QueuedLogs(LogWriting,capacity,overflow,scope); QueuedLogs(LogDelivering,options,scope) | write(record); async drain() is void; async deliver(record) is int; dropped_records()/closed()/close() |
| logging/syslog | SyslogFacility; SyslogOptions(facility,host,application,maximum_payload); SyslogMessage(record,options); SyslogLogs(connection,options,queue_options,scope), seven-argument shorthand | options coordinates; bytes(); write, async drain is void, dropped_records, closed, close |

LogFileState, LogQueueState, QueuedLogRecord, LogArchiveCatalog, LogArchive,
LogFieldSnapshot and format/policy helpers are public infrastructure because the
language has no module-private declarations. They are not alternate owner APIs.

## Stage26 web application layer (2026-10-13)

[Contract](web.md) · delivery audit. Built on `http/routing`, `http/handler` and `http/server`; it adds no runtime provider. Endpoints are bound actions `async (WebRequest) is HandlingResult`.

| Module | Constructors / roles | Public behavior |
|---|---|---|
| web/routing | ExceptionScope; WebRouting; Get/Post/Put/Patch/Delete(path,endpoint); Scope(prefix,routings); Scope(prefix,exceptions,routings); WebRoutes(routings,fallback); WebRoutes(routings,exceptions,fallback); WebRejection() does RouteRejecting; InvalidWebRoute(reason) | ExceptionScope.scoped(request,endpoint); WebRouting.routes(prefix,scopes) is @Route; WebRoutes does Handling, table() is Routes; WebRejection answers std Routes 400/405 with problem replies, Allow kept |
| web/request | WebRequest(request,context); RequestJson(Json); MissingRouteParameter(name); InvalidBodyLimit(limit) | request(), context(), method(), path(name), query(name[,fallback]), parameters() is QueryParameters, header(name[,fallback]); async bytes/text/json/form(limit); json(limit) is RequestJson: JsonView reading API whose failures are Exception(422), view() |
| web/reply | Text(status,text); Html(status,html); JsonReply(status,Json); Created(location,Json); NoContent(); Redirect(status,location) | all do HandlingResult; status(); successor header(name,value) |
| web/exception | Exception(status,detail) does Error, HandlingResult | status(), detail(), title(), text(); successor header(name,value); RFC 9457 problem+json response() |
| web/app | WebApplication(routes); WebApplication(routes,policy,clock) | async server(address,port) is HttpServer; async listen(address,port) is void |
| web/internal/reply | ReplyHeaders(); ReplyHeaders(@Header); ReplyResponse; EmptyReplyResponse; ReplyFields; BodyStatus(status); InvalidReply(reason) | implementation collaborators; InvalidReply is the construction error of web/reply and web/exception |

## Stage26 locale/i18n and Stage27 testing (2026-10-13)

[Locale/i18n contracts](locale-i18n.md), [matching](locale-match-guide.md),
[testing contracts](testing.md). Inventories describe implemented profiles; they
are not a declaration that all Stage26/27 roadmap rows are complete.

| Module | Constructors / roles | Public behavior |
|---|---|---|
| locale/match | FilteredLocales(ranges,available); LookedUpLocale(ranges,available,fallback) | Sequencing<Locale>; projected value() selects first original available representatives using explicit immutable prefix indexes; small sets use cached-key scans |
| locale/data | LocaleDataVersion(); LocaleDataKey(tag); LocaleFallbacks(Locale) | cldr()/unicode()/registry(); data-key string()/locale(); Sequencing<LocaleDataKey> ends at root |
| locale/list | LocalizedList(values,locale,kind,style); ListKind(conjunction,disjunction,unit); ListStyle(long,short,narrow); InvalidLocalizedList(reason) | string(); bounded literal lists and pinned CLDR parent/root fallback |
| locale/likely | LikelyLocale(Locale) | projected value() explicitly adds pinned likely subtags |
| locale/system | PreferredLanguageReading; FixedLanguagePreferences(tags,platform_owned); SystemLanguagePreferences(); PreferredLocales(Environment[,reading]) | platform_owned()/tags(); Sequencing<Locale>, explicit POSIX/native policy |
| i18n/plural | PluralNumber(text); CanonicalPluralNumber(Decimal); PluralCategoryOf(number,locale,kind) | string()/number()/operand(name); projected number/category; exact visible precision |
| i18n/catalog | MessageKey(context,identity); PluralForm(category,text); MessageEntry; SingularMessage(key,text); PluralMessage(key,forms[,kind]); Translating; MessageCatalog(locale,entries); FallbackTranslation(primary,fallback) | exact key order; text/forms/kind projections; membership/text/plural; typed absence/misprojection |
| testing/context | TestContext(); MissingTestContext() | require() only during a running case; typed outside-context failure |
| testing/check | Checking; AsyncChecking; CloseTo(a,b,relative,absolute); TextEquals(a,b[,context]); BytesEqual(a,b[,context]); SequenceEquals<T>(a,b[,context[,rendering]]); MatchingRegex(text,pattern); Eventually/AsyncEventually(condition,clock,timeout,interval) | check() / async check() is void; bounded mismatch diagnostics and explicit clock policy |
| testing/error | ErrorMatching; AnyErrorMatching(); ErrorTextMatching(text); ErrorRegexMatching(pattern); Throwing<R>/AsyncThrowing<R>(action,matching) | match(Error)/description(); contextual sync/async check |
| testing/time | ManualClock(start); ManualClockClosed(); InvalidManualClock(reason) | Timing; advance/set_wall/pending/closed/close, destroy; test-only, no host clock |
| testing/double | Recorded<T>(index,value); RecordedCalls<T>(); Stub<T>(values) | index/value; record/records/size/close; next/size/remaining/close; fresh test-only owners |
| testing/generation | Generation<T>; Generated<T>; GeneratedElement<T>(value,text); BooleanGeneration(); IntegerGeneration(min,max); FiniteFloatGeneration(min,max); ElementGeneration<T>(elements); SequenceGeneration<T>(origin,min,max); FilteredGeneration<T>(origin,predicate,attempts) | sample(random,size); value/string/reductions; finite bounded invariant-preserving core reductions |
| testing/property | Property<T>/AsyncProperty<T>(generation,seed,cases,size,shrink,condition); PropertyReproduction(seed,index,size,text) | contextual check; seed/case_index/size/value/string coordinates; serial bounded shrinking |
| testing/result | TestPosition(source,line,column); TestFailure(message[,position]); TestFailure(message,positions,causes); TestOutput(bytes,errors,truncated); TestResult(name,status,duration,failures,output[,skip_reason]); TestCounts(passes,failures,skips); TestReport(start,duration,seed,results) | immutable projections, discovery order, counts and exit_status; typed missing position |
| testing/report | HumanTestReport(report,width); TapTestReport(report); JsonTestReport(report); JUnitTestReport(report,suite) | string(); scalar-width previews, TAP13, Base64 JSON/XML output and exact JUnit durations |
| runtime/testing | native test_context_active(); TestState; test_state_open/next/size/close | bool flag and test-context-only bounded native cursor/capacity owners |
| runtime/locale | native preferred_language_tags() | @string, bounded Unicode snapshot; POSIX policy remains in stdlib |
| runtime/debug | existing native data_dump(any) declaration | string diagnostic boundary already registered by core.debug |
| runtime/number_text | native int_parse_valid(text,radix); int_parse(text,radix) | ParsedInt spelling: optional `-`, 0b/0o/0x only with radix 0, digits 0-9a-zA-Z below the base, any length; int_parse fails with invalid_input where int_parse_valid is false |

The generated CLDR internals and checker/state/report helpers remain importable
infrastructure because module-private declarations are not available. Main Error
contracts, limits and ownership are specified in the linked guides.

## XML value serialization continuation

[Accepted contract](xml-values.md). These values do not implement source parsing.

| Module | Constructors / roles | Public behavior |
|---|---|---|
| xml/name | XmlName(local); XmlName(namespace,local,prefix); InvalidXml(offset,reason) | namespace/local_name/prefix/lexical; typed validation |
| xml/node | XmlNode; XmlText(value); XmlComment(value); XmlProcessingInstruction(target,data); XmlAttribute(name,value); XmlElement(name[,attributes],children); XmlDocument(root) | xml/depth; immutable ordered coordinates and one-root declaration |
| xml/write | EscapedXml(text,attribute); XmlCharacters(text) | string; XML character validation and separate text/attribute escaping |

## Complete language tags / IANA continuation

Locale(language) accepts complete tags; LocaleFromLanguageTag remains lazy.
Locale adds extlangs(), extensions(), private_use(), base_tag(), suffix().
LocaleExtensions(locale) exposes attributes()/unicode()/transformations()/private_use();
UnicodeLocaleKeyword.key()/values() retains empty boolean types. Complete tags
sort extension singletons and Unicode/transformed keys without discarding payloads.
LocaleDataVersion.registry() identifies IANA File-Date. See [contract](locale-tag-guide.md).

## Finite binary64 decimal bridge

| Module | Constructors / roles | Public behavior |
|---|---|---|
| numbers/float | ShortestFloat(origin is float); InvalidFloatRepresentation() | projected value() is Decimal; negative() preserves original sign including zero; exact IEEE interval verification |
| runtime/format | native sprintf_values(pattern,values is @any) | existing core.format symbol declaration; numeric candidate proposal only |

[Contract](shortest-float.md). [Localized numbers](locale-number.md) consume this bridge;
no global change to builtin float.string() is claimed.


## `locale/number`

[Contract, limits and failures](locale-number.md). Printing and parsing are separate
immutable representations with explicit Locale and policy.

| Declaration | Constructors | Behavior |
|---|---|---|
| NumberPrecision | (minimum_integer,minimum_fraction,maximum_fraction,rounding) | immutable precision projections; bounds checked |
| NumberFormat | (notation,grouping,sign,precision) | policy projections |
| LocalizedInteger / LocalizedBigInteger / LocalizedDecimal / LocalizedFloat | (origin,locale,format) | string(); exact decimal rounding and pinned CLDR |
| LocalizedPercent | (origin is Decimal,locale,format,scale) | string(); percent/per_mille scaling before rounding |
| LocalizedSpecialNumber | (origin is NumberSpecial,locale,sign) | string(); explicit NaN/infinity symbols |
| LocalizedDecimalFromString | (origin is string,locale,format) | projected value() is Decimal; compact inverse rejected |
| LocalizedIntegerFromString | (origin is string,locale,grouping) | projected value() is BigInt |
| InvalidNumberFormat / InvalidLocalizedNumber | (reason is string) | Error.text(); bounded policy/input failure |

Enums: NumberNotation(standard,scientific,engineering,compact_short,compact_long),
NumberGrouping(never,automatic,always,minimum_two),
NumberSign(automatic,always,except_zero,negative_only,never),
PercentScale(percent,per_mille), NumberSpecial(not_a_number,positive_infinity,negative_infinity).


## `locale/currency`

[Contract, snapshots and limits](locale-currency.md). No region inference or exchange rate.

| Declaration | Constructors | Behavior |
|---|---|---|
| Currency does Comparing<Currency> | (code is string) | code/digits/order; current uppercase ISO identity; undefined units are typed |
| CurrencyFormat | (numbers is NumberFormat,display is CurrencyDisplay,accounting is bool,currency_digits is bool) | immutable policy projections |
| LocalizedCurrency | (amount is Decimal,currency is Currency,locale is Locale,format is CurrencyFormat) | string(); exact rounding, CLDR plurals/frames/digits |
| CurrencyDataVersion | () | iso() and cldr() snapshot identities |
| InvalidCurrency / MissingCurrencyDigits | (code is string) | Error.text() and code() |

CurrencyDisplay: symbol, narrow_symbol, code, name. Shared internal numeric
rendering/compact roles bind behavior only, not private locale fields.


## `locale/date_time`

[Contract, validation moments and bounds](locale-date-time.md).

| Declaration | Constructors | Behavior |
|---|---|---|
| DateTimeStyle | (date is DateStyle,time is TimeStyle) | independent style projections |
| LocalizedDate | (origin is Date,locale is Locale,style is DateStyle) | string(); cached immutable Gregorian wall representation |
| LocalizedTime | (origin is TimeOfDay,locale is Locale,style is TimeStyle) | string(); timezone-free, full/long use medium template |
| LocalizedLocalDateTime | (origin is LocalDateTime,locale is Locale,style is DateTimeStyle) | string(); cached wall pair and CLDR combination |
| LocalizedInstant | (origin is Instant,zone is TimeZone,locale is Locale,style is DateTimeStyle) | string(); explicit zone observation, localized historical names/GMT |
| InvalidLocalizedDateTime | (reason is string) | Error.text(); unsupported calendar/numbering or representation failure |

DateStyle and TimeStyle: full, long, medium, short. GregorianLocaleData and the
compiled-program/naming collaborators are importable infrastructure, not general
user pattern parsers. Standard-style source parsing is deliberately absent.


## `locale/collation`

[Contract, pinned backend and bounds](locale-collation.md).

| Declaration | Constructor | Behavior |
|---|---|---|
| CollationOptions | (strength,case_first,numeric,normalization) | immutable policy; enums documented in contract |
| CollatedString does Comparing<CollatedString> | (origin is string,locale is Locale,options is CollationOptions) | original string(), key(), checked order(), locale(), options(), data_version() |
| CollationDataVersion | () | icu/unicode/uca/cldr/string snapshot identities |
| InvalidCollation / UnavailableCollation | (reason is string) | typed input/budget and pinned-backend errors |
| IncompatibleCollation | () | exact policy, locale or version mismatch |

`runtime/collation` is the bounded seven-coordinate native key boundary. Pointers
stay private and one collator closes per call. Keys are ephemeral opaque bytes.


## `i18n/gettext`

[Accepted active profile, codecs and bounds](gettext.md).

| Declaration | Constructor | Behavior |
|---|---|---|
| GettextCatalog | (origin is @byte,locale is Locale) | membership/text/plural(key,count is Decimal), locale, nominal metadata |
| GettextPluralRule | (header is string) | size/index(count is Decimal); bounded parsed unsigned64 expression |
| GettextTranslation does Translating | (catalog is GettextCatalog) | membership/text/plural(key,count is PluralNumber) |
| PoComment | (kind is PoCommentKind,text is string) | kind/text |
| PoComments | (records is Sequencing<PoComment>) or five grouped string sequences | ordered records and grouped projections |
| PoEntry | (context,identity,plural,translations,comments[,has_context]) | immutable coordinates/translation snapshots and presence |
| PoDocument | (entries is Sequencing<PoEntry>[,trailing is PoComments]) | ordered entries, trailing comments, nominal metadata |
| ParsedPo | (text is string[,escape_charset is string]) | cached value() is PoDocument |
| PoString | (document is PoDocument) | eagerly derived cached string() |
| GettextMetadata / GettextHeader | represented header payload / (name,value) | values/membership/text; name/value |
| InvalidCatalog | (offset is int,reason is string) | Error.text and offset |

MessageKey adds a third optional has_context boolean and has_context() projection.
The existing two-argument constructor remains compatible; explicit empty context
is distinct from absence and retained by MessageCatalog snapshots.


## Completed testing boundary and line diffs

[Testing contract](testing.md), [diff contract](diff.md).

| Module | Constructors / roles | Public behavior |
|---|---|---|
| diff | UnifiedDiff(left,right[,context]); ContextDiff(left,right[,context]); MatchingBlocks(left,right); MatchingRatio(left,right); MatchingBlock(left,right,size) | cached string(); values() is Sequencing<MatchingBlock>; explicit LCS value(); block left/right/size |
| testing/snapshot | SnapshotFile(DirectoryLocating,name); TextSnapshot(text); JsonSnapshot(Json); RawSnapshot(bytes); Snapshot(ByteSource,file,limit); SnapshotUpdate(ByteSource,file,limit) | root/name; ByteSource.bytes(); contextual check(); separate explicit update() |
| testing/runner | RecordedTestReport(JSON text) | value() is nominal TestReport; Base64 byte transport, positions, causes and terminal statuses |
| testing/generation | DistinctReductions<T>(source,parentText) | Sequencing<Generated<T>>; stable reproduction-key deduplication with source order |
| testing/result | TestFailureText(TestFailure) | bounded-depth cause text; TestFailure.causes()/depth(); TestResult.skip_reason() |
| runtime/diff | diff_script(@string,@string) | bounded edit/position triplets as @int; Algorithm::Diff 1.201 |
| runtime/snapshot | SnapshotRootState; snapshot_root_open/close; snapshot_read/write | explicit descriptor authority, no-follow exact read and atomic sibling replacement |
| runtime/time | clock_domain(fresh is bool) | string identity; fresh manual domains require a running case |
| time/clock | MonotonicInstant(Duration[,domain]); IncompatibleClock() | reading/domain/plus/minus/order; cross-domain comparisons rejected |
| locale/data | LocaleDataVersion() | cldr/unicode/registry/currencies/collation_cldr/icu/uca exact pinned coordinates |

The host Okkama::TestRunner accepts workers, seed, literal filter, timeout, capture
limit and report format. CLI options are documented in testing.md. Contextual
test.skip and fake.reading/fake.writing are source-only capabilities, not nominal
objects obtainable through these inventories. Legacy projections stay available.

## Bounded scalar star matching

`runtime/collections.string_star_match(pattern,target,anchored,steps)` is a pure
string operation. Only '*' is special; all other Unicode scalars are literal.
False anchored accepts a matching prefix, true requires the target end. Requested
steps must be1..262144. Each pointer/retry iteration, including the terminal
check, costs one step. Invalid budgets use core.collections/invalid_input;
exhaustion uses resource_exhausted. Inputs are unchanged; no regex expansion,
URL policy, host effect or full character snapshot is introduced. RobotsPattern
maps native exhaustion to its existing typed match_steps failure.

Routing graph validation additionally uses safe later-literal partitions and an
explicit worklist. Capture names are checked by a separate sorted projection;
source-order dispatch, root/parameter/wildcard intersections and constructor
arities remain unchanged. Unseparated families keep the exact overlap fallback.

SettingOverlay upsert finds an existing slot or insertion leaf in one balanced
path traversal, preserving first positions, complete replacement/provenance and
replacement admission at full capacity. Limits and public signatures are unchanged.

## Host cryptography — 2026-10-23

[Crypto guide](crypto-guide.md). All primitives run in `okkama-native`
(RustCrypto) through `runtime/host_crypto`; keys are SecretBytes, randomness is
an explicit Entropy, failures are InvalidCryptoInput / AuthenticationFailed /
CryptoUnavailable.

| Module | Public objects |
| --- | --- |
| digest/sha2, digest/sha3 | Sha224/Sha256/Sha384/Sha512/Sha512_256, Sha3_224..Sha3_512: Digesting with state(bytes) successors |
| digest/shake, digest/blake2 | Shake128/Shake256(length); Blake2b/Blake2s(length), Blake2bMac/Blake2sMac(key) |
| digest/read | ReadDigest(Reading,digest[,chunk]).result/continued; AsyncReadDigest over AsyncReading |
| crypto/constant_time, crypto/nonce | ConstantTimeEqual(left,right).value; Entropy (fake), SystemEntropy, RandomNonce, CounterNonce |
| crypto/hmac, crypto/kdf | Hmac(HmacHash,key[,message]) bytes/state/matches; HkdfSecret, Pbkdf2Secret(...).secret |
| crypto/aead, crypto/aes_gcm, crypto/chacha20_poly1305 | Sealing sealed/opened/nonce_size/tag_size; AesGcm, ChaCha20Poly1305, XChaCha20Poly1305 |
| crypto/key, crypto/ed25519, crypto/x25519, crypto/ecdsa | PublicKey, VerifyingKey, SigningKey; Ed25519PrivateKey/PublicKey; X25519PrivateKey.shared_secret; EcdsaPrivateKey/PublicKey (p256, p384) |
| crypto/rsa | RsaPrivateKey(pkcs8:)/(bits:,entropy:), RsaPublicKey; RsaPssSigning/Verifying, RsaPkcs1Signing/Verifying, RsaOaepEncryption/Decryption |
| crypto/key_format, crypto/password | PrivateKeyPem, PublicKeyPem, ParsedPrivateKey, ParsedPublicKey; Argon2Policy, PasswordHash, PasswordCheck |
