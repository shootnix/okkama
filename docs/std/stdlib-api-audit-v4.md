# std/ public API audit for v4

This file preserves the API review and dated delivery notes. Statements such as
“still open” in an earlier delivery section describe that section's date; they
are not the current implementation status. Use the
current master register, [API inventory](stdlib-api-v4.md)
and the linked module contract for current behavior. In particular H2 sessions,
clients, ALPN, WebSockets and the SQLite driver have later implemented profiles;
their remaining limits are listed separately in the master register.


Baseline: 130 modules (94 public/domain modules, 36 runtime modules), 604 domain declarations and 1175 public action signatures. Extracted from the production AST, not regex guesses about method visibility.

Each row covers all listed overloads. Identity is the private state column; stateless atoms and runtime-owned capabilities are described by the module policy. Constructors are reviewed with the construction policy in the design principles. This ledger is an architectural judgment, not an automatic claim that every implementation is bug-free.

This table intentionally preserves the pre-refactor signatures as audit evidence. Implemented changes are detailed in the [migration guide](stdlib-v4-migration.md); the [current API](stdlib-api-v4.md) contains the resulting signatures. “Keep” includes the stated Okkama compromises; it is not attribution of these compromises to Bugayenko.

## std/async/barrier.ok

coordination capabilities and future compositions. External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Barrier` | `amount:int` | `async wait() is int`<br>`state() is BarrierState`<br>`abort() is void`<br>`reset() is void` | keep with module effect policy: Barrier represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `BarrierState` | Domain atom / role / runtime-owned state | `parties() is int`<br>`waiters() is int`<br>`generation() is int`<br>`broken() is bool` | keep with module effect policy: BarrierState represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `InvalidPartyCount` | `amount:int` | `parties() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `BrokenBarrier` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/async/cancellation.ok

coordination capabilities and future compositions. External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Cancelling` | Domain atom / role / runtime-owned state | `requested() is bool`<br>`check() is void`<br>`async wait() is Cancelling` | keep: nominal observable contract; no implementation inheritance or reflection |
| `Cancellation` | Domain atom / role / runtime-owned state | `cancelling() is Cancelling`<br>`cancel() is void` | keep with module effect policy: Cancellation represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `CancellationToken` | Domain atom / role / runtime-owned state | `requested() is bool`<br>`check() is void`<br>`async wait() is Cancelling` | keep with module effect policy: CancellationToken represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
## std/async/channel.ok

coordination capabilities and future compositions. External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Channel` | Domain atom / role / runtime-owned state | `sending() is Sending<T>`<br>`receiving() is Receiving<T>` | keep with module effect policy: Channel represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `Sending` | Domain atom / role / runtime-owned state | `async send(value is T) is Sending<T>`<br>`close() is void` | keep with module effect policy: Sending represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `Receiving` | Domain atom / role / runtime-owned state | `async item() is T`<br>`close() is void` | keep with module effect policy: Receiving represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `ChannelClosed` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidChannelCapacity` | `capacity:int` | `capacity() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/async/collect.ok

coordination capabilities and future compositions. External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `AllFutures` | `origins:Sequencing<Future<T>>` | `async results() is Sequencing<T>` | keep with module effect policy: AllFutures represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `Raced` | `origins:Sequencing<Future<T>>` | `async result() is T` | keep with module effect policy: Raced represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `FirstSuccessful` | `origins:Sequencing<Future<T>>` | `async result() is T` | keep with module effect policy: FirstSuccessful represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `FutureCompletions` | `origins:Sequencing<Future<T>>` | `remaining_count() is int`<br>`async completion() is Future<T>` | keep with module effect policy: FutureCompletions represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `NoFutures` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/async/event.ok

coordination capabilities and future compositions. External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Event` | `initial:bool` | `async wait() is Event`<br>`set() is void`<br>`ready() is bool` | keep with module effect policy: Event represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
## std/async/lock.ok

coordination capabilities and future compositions. External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Lock` | Domain atom / role / runtime-owned state | `async guard() is LockGuard`<br>`locked() is bool` | keep with module effect policy: Lock represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `LockGuard` | Domain atom / role / runtime-owned state | `closed() is bool`<br>`close() is void` | keep with module effect policy: LockGuard represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
## std/async/scope.ok

coordination capabilities and future compositions. External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Supervising` | Domain atom / role / runtime-owned state | `start(origin is Future<T>) is void`<br>`active() is int`<br>`cancel() is void` | keep: nominal observable contract; no implementation inheritance or reflection |
| `FutureGroup` | Domain atom / role / runtime-owned state | `start(origin is Future<T>) is void`<br>`active() is int`<br>`cancel() is void`<br>`async wait() is FutureGroup<T>` | keep with module effect policy: FutureGroup represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `FutureScope` | `action:async (Supervising<T>) is R` | `async result() is R` | keep with module effect policy: FutureScope represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `FinishedFutureGroup` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/async/semaphore.ok

coordination capabilities and future compositions. External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Semaphore` | `amount:int` | `async permit() is Permit`<br>`permits() is int` | keep with module effect policy: Semaphore represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `Permit` | Domain atom / role / runtime-owned state | `closed() is bool`<br>`close() is void` | keep with module effect policy: Permit represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `InvalidPermitCount` | `amount:int` | `permits() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/async/timeout.ok

coordination capabilities and future compositions. External scheduler state belongs to runtime-owned capabilities; immutable object coordinates and existing cancellation/destroy rules stay intact.

Ready/suspended/error/cancel paths, waiter races and owned guard release; no async void, mutable user fields or polling loops in cursors.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Within` | `origin:Future<T>`; `limit:Duration`; `clock:Timing` | `async result() is T` | keep with module effect policy: Within represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `Before` | `origin:Future<T>`; `deadline:MonotonicInstant`; `clock:Timing` | `async result() is T` | keep with module effect policy: Before represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `TimedOut` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/calendar.ok

validated calendar representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `YearMonth` | `year_value:int`; `month_value:int` | `year() is int`<br>`month() is int`<br>`order(other is YearMonth) is Order` | keep: YearMonth denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `MonthDates` | `origin:YearMonth` | `cursor() is Cursor<Date>`<br>`size() is int`<br>`item(index is int) is Date`<br>`first() is Date`<br>`last() is Date` | keep: MonthDates denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `MonthDateCursor` | `origin:MonthDates`; `position:int` | `available() is bool`<br>`item() is Date`<br>`successor() is Cursor<Date>` | keep at implementation boundary: MonthDateCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `CalendarWeek` | `first_date:Date` | `cursor() is Cursor<Date>`<br>`size() is int`<br>`item(index is int) is Date`<br>`first() is Date`<br>`last() is Date` | keep: CalendarWeek denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `CalendarWeekCursor` | `origin:CalendarWeek`; `position:int` | `available() is bool`<br>`item() is Date`<br>`successor() is Cursor<Date>` | keep at implementation boundary: CalendarWeekCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `CalendarMonth` | `origin:YearMonth`; `first_weekday:Weekday` | `cursor() is Cursor<Sequencing<Date>>`<br>`size() is int`<br>`item(index is int) is Sequencing<Date>`<br>`first() is Sequencing<Date>`<br>`last() is Sequencing<Date>` | keep: CalendarMonth denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `CalendarMonthCursor` | `origin:CalendarMonth`; `position:int` | `available() is bool`<br>`item() is Sequencing<Date>`<br>`successor() is Cursor<Sequencing<Date>>` | keep at implementation boundary: CalendarMonthCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `CalendarMonthLayout` | `origin:YearMonth`; `first_weekday:Weekday` | `first_ordinal() is int`<br>`size() is int` | keep: CalendarMonthLayout denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `IsoWeek` | `year_value:int`; `week_value:int` | `year() is int`<br>`week() is int`<br>`first() is Date`<br>`last() is Date`<br>`order(other is IsoWeek) is Order` | keep: IsoWeek denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `LeapYear` | `origin:int` | `value() is bool` | keep: LeapYear denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `WeekdayIndex` | `origin:Weekday` | `value() is int` | keep: WeekdayIndex denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidYearMonth` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidCalendarMonth` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidIsoWeek` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/collections/combinatorics.ok

immutable sequences, associations and traversal. No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Combinations` | `origin:Sequencing<T>`; `amount:int` | `cursor() is Cursor<@T>` | keep: Combinations represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `CombinationCursor` | `origin:Sequencing<T>`; `positions:@int`; `amount:int`; `active:bool` | `available() is bool`<br>`item() is @T`<br>`successor() is Cursor<@T>` | keep at implementation boundary: CombinationCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `CombinationsWithReplacement` | `origin:Sequencing<T>`; `amount:int` | `cursor() is Cursor<@T>` | keep: CombinationsWithReplacement represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `ReplacementCombinationCursor` | `origin:Sequencing<T>`; `positions:@int`; `amount:int`; `active:bool` | `available() is bool`<br>`item() is @T`<br>`successor() is Cursor<@T>` | keep at implementation boundary: ReplacementCombinationCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `Permutations` | `origin:Sequencing<T>`; `amount:int` | `cursor() is Cursor<@T>` | keep: Permutations represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `PermutationCursor` | `origin:Sequencing<T>`; `positions:@int`; `amount:int`; `active:bool` | `available() is bool`<br>`item() is @T`<br>`successor() is Cursor<@T>` | keep at implementation boundary: PermutationCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `CartesianProduct` | `origins:Sequencing<Sequencing<T>>` | `cursor() is Cursor<@T>` | keep: CartesianProduct represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `CartesianCursor` | `origins:Sequencing<Sequencing<T>>`; `positions:@int`; `active:bool` | `available() is bool`<br>`item() is @T`<br>`successor() is Cursor<@T>` | keep at implementation boundary: CartesianCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `CombinationPositions` | `origin:@int`; `source_size:int` | `valid() is bool` | keep at implementation boundary: CombinationPositions represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `CombinationPivot` | `origin:@int`; `source_size:int` | `value() is int` | keep at implementation boundary: CombinationPivot represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `ReplacementPositions` | `origin:@int`; `source_size:int` | `valid() is bool` | keep at implementation boundary: ReplacementPositions represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `ReplacementPivot` | `origin:@int`; `source_size:int` | `value() is int` | keep at implementation boundary: ReplacementPivot represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `PermutationPositions` | `origin:@int`; `source_size:int` | `valid() is bool` | keep at implementation boundary: PermutationPositions represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `PrefixMembership` | `origin:@int`; `ending:int`; `sought:int` | `result() is bool` | keep: PrefixMembership represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `PermutationCandidate` | `origin:@int`; `ending:int`; `source_size:int`; `beginning:int` | `value() is int` | keep at implementation boundary: PermutationCandidate represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `PermutationPivot` | `origin:@int`; `source_size:int` | `value() is int` | keep at implementation boundary: PermutationPivot represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `PrefixBefore` | `origin:@int`; `ending:int`; `boundary:int` | `amount() is int` | keep: PrefixBefore represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `PermutationAvailableIndex` | `origin:@int`; `ending:int`; `selected:int`; `source_size:int`; `offset:int` | `value() is int` | keep: PermutationAvailableIndex represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `CartesianAvailability` | `origins:Sequencing<Sequencing<T>>` | `result() is bool` | keep: CartesianAvailability represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `CartesianPivot` | `origins:Sequencing<Sequencing<T>>`; `positions:@int` | `value() is int` | keep at implementation boundary: CartesianPivot represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `CartesianPositions` | `origins:Sequencing<Sequencing<T>>`; `positions:@int` | `valid() is bool` | keep at implementation boundary: CartesianPositions represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
## std/collections/combining.ok

immutable sequences, associations and traversal. No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Pair` | `left:A`; `right:B` | `first() is A`<br>`second() is B` | keep: Pair represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `Zipped` | `left:Iterating<A>`; `right:Iterating<B>` | `cursor() is Cursor<Pair<A,B>>` | keep: Zipped represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `ZippedCursor` | `left:Cursor<A>`; `right:Cursor<B>` | `available() is bool`<br>`item() is Pair<A,B>`<br>`successor() is Cursor<Pair<A,B>>` | keep at implementation boundary: ZippedCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `StrictlyZipped` | `left:Iterating<A>`; `right:Iterating<B>` | `cursor() is Cursor<Pair<A,B>>` | keep: StrictlyZipped represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `StrictlyZippedCursor` | `left:Cursor<A>`; `right:Cursor<B>` | `available() is bool`<br>`item() is Pair<A,B>`<br>`successor() is Cursor<Pair<A,B>>` | keep at implementation boundary: StrictlyZippedCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `UnequalSizes` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `Chained` | `origins:Iterating<Iterating<T>>` | `cursor() is Cursor<T>` | keep: Chained represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `ChainedCursorSearch` | `origins:Cursor<Iterating<T>>` | `result() is Cursor<T>` | keep at implementation boundary: ChainedCursorSearch represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `ChainedCursorCandidates` | `origins:Cursor<Iterating<T>>` | `cursor() is Cursor<ChainedCursorCandidateValue<T>>` | keep at implementation boundary: ChainedCursorCandidates represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `ChainedCursorCandidate` | `origins:Cursor<Iterating<T>>` | `available() is bool`<br>`item() is ChainedCursorCandidateValue<T>`<br>`successor() is Cursor<ChainedCursorCandidateValue<T>>` | keep at implementation boundary: ChainedCursorCandidate represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `ChainedCursorCandidateValue` | `source:Iterating<T>`; `following:Cursor<Iterating<T>>` | `origin() is Iterating<T>`<br>`remaining() is Cursor<Iterating<T>>` | keep at implementation boundary: ChainedCursorCandidateValue represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `ExhaustedChainedCursor` | `origins:Cursor<Iterating<T>>` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: ExhaustedChainedCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `ChainedItemCursor` | `current:Cursor<T>`; `remaining:Cursor<Iterating<T>>` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: ChainedItemCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
## std/collections/deque.ok

immutable sequences, associations and traversal. No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Deque` | `beginning:List<T>`; `ending:List<T>`; `amount:int` | `cursor() is Cursor<T>`<br>`size() is int`<br>`item(index is int) is T`<br>`front() is DequeFront<T>`<br>`back() is DequeBack<T>` | keep: Deque represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `DequeFront` | `beginning:List<T>`; `ending:List<T>`; `amount:int` | `value() is T`<br>`addition(value is T) is Deque<T>`<br>`remainder() is Deque<T>` | keep: DequeFront represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `DequeBack` | `beginning:List<T>`; `ending:List<T>`; `amount:int` | `value() is T`<br>`addition(value is T) is Deque<T>`<br>`remainder() is Deque<T>` | keep: DequeBack represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `DequeItems` | `beginning:List<T>`; `ending:List<T>` | `cursor() is Cursor<T>` | keep: DequeItems represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `ConcatenatedDequeCursor` | `beginning:Cursor<T>`; `ending:Cursor<T>` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: ConcatenatedDequeCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `ArrayItems` | `origin:@T`; `witness:Iterating<T>` | `cursor() is Cursor<T>` | keep: ArrayItems represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `ArrayCursor` | `origin:@T`; `position:int`; `witness:Iterating<T>` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: ArrayCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `ForwardList` | `origin:Iterating<T>` | `list() is List<T>` | keep: ForwardList represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `ReversedList` | `origin:Iterating<T>` | `list() is List<T>` | keep: ReversedList represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `DequeParts` | `beginning:List<T>`; `ending:List<T>`; `amount:int` | `front() is List<T>`<br>`back() is List<T>`<br>`size() is int` | keep at implementation boundary: DequeParts represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `BalancedDeque` | `origin:@T`; `witness:Iterating<T>` | `parts() is DequeParts<T>` | keep: BalancedDeque represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `NormalizedDeque` | `beginning:List<T>`; `ending:List<T>`; `amount:int` | `parts() is DequeParts<T>` | keep: NormalizedDeque represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `EmptyDequeAccess` | `origin:string` | `operation() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/collections/enumeration.ok

immutable sequences, associations and traversal. No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Indexed` | `offset:int`; `origin:T` | `index() is int`<br>`item() is T` | keep: Indexed represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `Enumerated` | `origin:Iterating<T>`; `start:int` | `cursor() is Cursor<Indexed<T>>` | keep: Enumerated represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `EnumerationCursor` | `origin:Cursor<T>`; `position:int` | `available() is bool`<br>`item() is Indexed<T>`<br>`successor() is Cursor<Indexed<T>>` | keep at implementation boundary: EnumerationCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `EnumerationOverflow` | `origin:int` | `index() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/collections/frequencies.ok

immutable sequences, associations and traversal. No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Frequency` | `origin:T`; `quantity:int` | `item() is T`<br>`amount() is int` | keep: Frequency represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `Frequencies` | `origin:List<Frequency<T>>`; `quantity:int` | `cursor() is Cursor<Frequency<T>>`<br>`size() is int`<br>`amount(value is T) is int`<br>`total() is int` | keep: Frequencies represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `FrequencyBuilding` | `origin:List<Frequency<T>>`; `quantity:int` | `frequencies() is List<Frequency<T>>`<br>`total() is int`<br>`addition(value is T) is FrequencyBuilding<T>` | keep at implementation boundary: FrequencyBuilding represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `FrequencyRevision` | `origin:List<Frequency<T>>`; `sought:T`; `present:bool` | `frequencies() is List<Frequency<T>>`<br>`membership() is bool`<br>`addition(frequency is Frequency<T>) is FrequencyRevision<T>` | keep at implementation boundary: FrequencyRevision represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
## std/collections/heap.ok

immutable sequences, associations and traversal. No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Heap` | `origin:HeapRepresenting<T>` | `size() is int`<br>`top() is T`<br>`remainder() is Heap<T>`<br>`addition(value is T) is Heap<T>` | keep: Heap represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `HeapBuilding` | `origin:HeapRepresenting<T>` | `root() is HeapRepresenting<T>`<br>`addition(value is T) is HeapBuilding<T>` | keep at implementation boundary: HeapBuilding represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `HeapRepresenting` | Domain atom / role / runtime-owned state | `size() is int`<br>`rank() is int`<br>`top() is T`<br>`remainder() is HeapRepresenting<T>`<br>`combination(other is HeapRepresenting<T>) is HeapRepresenting<T>` | keep: nominal observable contract; no implementation inheritance or reflection |
| `EmptyHeapTree` | Domain atom / role / runtime-owned state | `size() is int`<br>`rank() is int`<br>`top() is T`<br>`remainder() is HeapRepresenting<T>`<br>`combination(other is HeapRepresenting<T>) is HeapRepresenting<T>` | keep: EmptyHeapTree represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `HeapTreeNode` | `origin:T`; `beginning:HeapRepresenting<T>`; `ending:HeapRepresenting<T>`; `height:int`; `amount:int` | `size() is int`<br>`rank() is int`<br>`top() is T`<br>`remainder() is HeapRepresenting<T>`<br>`combination(other is HeapRepresenting<T>) is HeapRepresenting<T>` | keep: HeapTreeNode represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `HeapChildren` | `beginning:HeapRepresenting<T>`; `ending:HeapRepresenting<T>` | `left() is HeapRepresenting<T>`<br>`right() is HeapRepresenting<T>` | keep at implementation boundary: HeapChildren represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
## std/collections/iteration.ok

immutable sequences, associations and traversal. No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Cursor` | Domain atom / role / runtime-owned state | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep: nominal observable contract; no implementation inheritance or reflection |
| `Iterating` | Domain atom / role / runtime-owned state | `cursor() is Cursor<T>` | keep: nominal observable contract; no implementation inheritance or reflection |
| `IterationEnded` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `CursorItems` | `origin:Cursor<T>` | `cursor() is Cursor<T>` | keep at implementation boundary: CursorItems represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
## std/collections/list.ok

immutable sequences, associations and traversal. No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `List` | Domain atom / role / runtime-owned state | `cursor() is Cursor<T>`<br>`size() is int`<br>`first() is T`<br>`tail() is List<T>`<br>`prefix(value is T) is List<T>` | keep: nominal observable contract; no implementation inheritance or reflection |
| `EmptyList` | Domain atom / role / runtime-owned state | `cursor() is Cursor<T>`<br>`size() is int`<br>`first() is T`<br>`tail() is List<T>`<br>`prefix(value is T) is List<T>` | keep: EmptyList represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `LinkedList` | `origin:T`; `following:List<T>`; `amount:int` | `cursor() is Cursor<T>`<br>`size() is int`<br>`first() is T`<br>`tail() is List<T>`<br>`prefix(value is T) is List<T>` | keep: LinkedList represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `EmptyListCursor` | `origin:List<T>` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: EmptyListCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `LinkedListCursor` | `origin:T`; `following:List<T>` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: LinkedListCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `EmptyListAccess` | `origin:string` | `operation() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/collections/ordering.ok

immutable sequences, associations and traversal. No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Sorted` | `origin:@T` | `cursor() is Cursor<T>`<br>`size() is int`<br>`item(index is int) is T`<br>`first() is T`<br>`last() is T` | keep: Sorted represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `SortedBy` | `origin:@Ranked<T,K>` | `cursor() is Cursor<T>`<br>`size() is int`<br>`item(index is int) is T`<br>`first() is T`<br>`last() is T` | keep: SortedBy represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `Ranked` | `origin:T`; `ranking:K` | `item() is T`<br>`key() is K`<br>`order(other is Ranked<T,K>) is Order` | keep: Ranked represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `PositionOf` | `origin:Sequencing<T>`; `sought:T` | `value() is int` | keep: PositionOf represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `LowerBound` | `origin:Sequencing<T>`; `sought:T` | `value() is int` | keep: LowerBound represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `UpperBound` | `origin:Sequencing<T>`; `sought:T` | `value() is int` | keep: UpperBound represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `LowerSearching` | `origin:Sequencing<T>`; `sought:T`; `beginning:int`; `ending:int` | `value() is int` | keep at implementation boundary: LowerSearching represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `UpperSearching` | `origin:Sequencing<T>`; `sought:T`; `beginning:int`; `ending:int` | `value() is int` | keep at implementation boundary: UpperSearching represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `OrderedCursor` | `origin:@T`; `position:int` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: OrderedCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `RankedCursor` | `origin:@Ranked<T,K>`; `position:int` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: RankedCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `MissingItem` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/collections/repetition.ok

immutable sequences, associations and traversal. No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Repeating` | `origin:T` | `cursor() is Cursor<T>` | keep: Repeating represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `RepeatingCursor` | `origin:T` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: RepeatingCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `Repeated` | `origin:T`; `amount:int` | `cursor() is Cursor<T>`<br>`size() is int`<br>`item(index is int) is T`<br>`first() is T`<br>`last() is T` | keep: Repeated represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `RepeatedCursor` | `origin:T`; `position:int`; `amount:int` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: RepeatedCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `Cycled` | `origin:Sequencing<T>` | `cursor() is Cursor<T>` | keep: Cycled represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `CycledCursor` | `origin:Sequencing<T>`; `position:int` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: CycledCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `EmptyCycleCursor` | `origin:Sequencing<T>` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: EmptyCycleCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
## std/collections/sequence.ok

immutable sequences, associations and traversal. No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Sequencing` | Domain atom / role / runtime-owned state | `cursor() is Cursor<T>`<br>`size() is int`<br>`item(index is int) is T`<br>`first() is T`<br>`last() is T` | keep: nominal observable contract; no implementation inheritance or reflection |
| `EmptySequence` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/collections/set.ok

immutable sequences, associations and traversal. No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Set` | Domain atom / role / runtime-owned state | `cursor() is Cursor<T>`<br>`size() is int`<br>`membership(value is T) is bool`<br>`addition(value is T) is Set<T>`<br>`difference(value is T) is Set<T>` | keep: nominal observable contract; no implementation inheritance or reflection |
| `OrderedSet` | `origin:List<T>` | `cursor() is Cursor<T>`<br>`size() is int`<br>`membership(value is T) is bool`<br>`addition(value is T) is Set<T>`<br>`difference(value is T) is Set<T>` | keep: OrderedSet represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `Union` | `left:Set<T>`; `right:Set<T>` | `cursor() is Cursor<T>`<br>`size() is int`<br>`membership(value is T) is bool`<br>`addition(value is T) is Set<T>`<br>`difference(value is T) is Set<T>` | keep: Union represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `Intersection` | `left:Set<T>`; `right:Set<T>` | `cursor() is Cursor<T>`<br>`size() is int`<br>`membership(value is T) is bool`<br>`addition(value is T) is Set<T>`<br>`difference(value is T) is Set<T>` | keep: Intersection represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `Difference` | `left:Set<T>`; `right:Set<T>` | `cursor() is Cursor<T>`<br>`size() is int`<br>`membership(value is T) is bool`<br>`addition(value is T) is Set<T>`<br>`difference(value is T) is Set<T>` | keep: Difference represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `SymmetricDifference` | `left:Set<T>`; `right:Set<T>` | `cursor() is Cursor<T>`<br>`size() is int`<br>`membership(value is T) is bool`<br>`addition(value is T) is Set<T>`<br>`difference(value is T) is Set<T>` | keep: SymmetricDifference represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `SetWith` | `origin:Set<T>`; `value:T`; `present:bool` | `cursor() is Cursor<T>`<br>`size() is int`<br>`membership(value is T) is bool`<br>`addition(value is T) is Set<T>`<br>`difference(value is T) is Set<T>` | keep: SetWith represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `SetWithout` | `origin:Set<T>`; `value:T`; `present:bool` | `cursor() is Cursor<T>`<br>`size() is int`<br>`membership(value is T) is bool`<br>`addition(value is T) is Set<T>`<br>`difference(value is T) is Set<T>` | keep: SetWithout represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `SetWithCursor` | `origin:Cursor<T>`; `value:T`; `emitted:bool` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: SetWithCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `ConcatenatedSetCursor` | `left:Cursor<T>`; `right:Cursor<T>` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: ConcatenatedSetCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `SetMembership` | `origin:Iterating<T>`; `value:T` | `result() is bool` | keep: SetMembership represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `SetSize` | `origin:Iterating<T>` | `result() is int` | keep: SetSize represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `AbsentFrom` | `origin:Set<T>` | `membership(value is T) is bool` | keep: AbsentFrom represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `DifferentFrom` | `origin:T` | `comparison(value is T) is bool` | keep: DifferentFrom represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `DistinctList` | `origin:Iterating<T>` | `list() is List<T>` | keep: DistinctList represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
## std/collections/views.ok

immutable sequences, associations and traversal. No mutation or external I/O; escaping cursors retain their sources. Snapshot/materialization timing remains explicit.

Empty/singleton inputs, alias preservation, cursor exhaustion, composition and scale; no mutable Java-style collection facade.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `CursorStep` | `origin:Cursor<T>`; `rest:Cursor<T>` | `current() is Cursor<T>`<br>`following() is Cursor<T>`<br>`item() is T` | keep at implementation boundary: CursorStep represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `CursorSteps` | `origin:Cursor<T>` | `cursor() is Cursor<CursorStep<T>>` | keep at implementation boundary: CursorSteps represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `CursorStepCursor` | `origin:Cursor<T>` | `available() is bool`<br>`item() is CursorStep<T>`<br>`successor() is Cursor<CursorStep<T>>` | keep at implementation boundary: CursorStepCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `Mapped` | `origin:Iterating<A>`; `mapping:(A) is B` | `cursor() is Cursor<B>` | keep: Mapped represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `MappedCursorSearch` | `origin:Cursor<A>`; `mapping:(A) is B` | `result() is Cursor<B>` | keep at implementation boundary: MappedCursorSearch represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `MappedCursor` | `origin:B`; `remaining:Cursor<A>`; `mapping:(A) is B` | `available() is bool`<br>`item() is B`<br>`successor() is Cursor<B>` | keep at implementation boundary: MappedCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `EmptyMappedCursor` | `origin:Cursor<A>`; `mapping:(A) is B` | `available() is bool`<br>`item() is B`<br>`successor() is Cursor<B>` | keep at implementation boundary: EmptyMappedCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `Filtered` | `origin:Iterating<T>`; `condition:(T) is bool` | `cursor() is Cursor<T>` | keep: Filtered represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `FilteredCursorSearch` | `origin:Cursor<T>`; `condition:(T) is bool` | `result() is Cursor<T>` | keep at implementation boundary: FilteredCursorSearch represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `FilteredCursor` | `origin:Cursor<T>`; `condition:(T) is bool` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: FilteredCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `EmptyFilteredCursor` | `origin:Cursor<T>`; `condition:(T) is bool` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: EmptyFilteredCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `Taken` | `origin:Iterating<T>`; `amount:int` | `cursor() is Cursor<T>` | keep: Taken represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `TakenCursor` | `origin:Cursor<T>`; `remaining:int` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: TakenCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `Dropped` | `origin:Iterating<T>`; `amount:int` | `cursor() is Cursor<T>` | keep: Dropped represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `DroppedCursorSearch` | `origin:Cursor<T>`; `amount:int` | `result() is Cursor<T>` | keep at implementation boundary: DroppedCursorSearch represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `TakenWhile` | `origin:Iterating<T>`; `condition:(T) is bool` | `cursor() is Cursor<T>` | keep: TakenWhile represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `TakenWhileCursorSearch` | `origin:Cursor<T>`; `condition:(T) is bool` | `result() is Cursor<T>` | keep at implementation boundary: TakenWhileCursorSearch represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `TakenWhileCursor` | `origin:Cursor<T>`; `condition:(T) is bool` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: TakenWhileCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `EmptyTakenWhileCursor` | `origin:Cursor<T>`; `condition:(T) is bool` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: EmptyTakenWhileCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `DroppedWhile` | `origin:Iterating<T>`; `condition:(T) is bool` | `cursor() is Cursor<T>` | keep: DroppedWhile represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `DroppedWhileCursorSearch` | `origin:Cursor<T>`; `condition:(T) is bool` | `result() is Cursor<T>` | keep at implementation boundary: DroppedWhileCursorSearch represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `Chunked` | `origin:Iterating<T>`; `amount:int` | `cursor() is Cursor<@T>` | keep: Chunked represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `ChunkedCursor` | `origin:Cursor<T>`; `amount:int` | `available() is bool`<br>`item() is @T`<br>`successor() is Cursor<@T>` | keep at implementation boundary: ChunkedCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `Flattened` | `origin:Iterating<Iterating<T>>` | `cursor() is Cursor<T>` | keep: Flattened represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `Pairwise` | `origin:Iterating<T>` | `cursor() is Cursor<Pair<T,T>>` | keep: Pairwise represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `PairwiseCursor` | `origin:Cursor<T>` | `available() is bool`<br>`item() is Pair<T,T>`<br>`successor() is Cursor<Pair<T,T>>` | keep at implementation boundary: PairwiseCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `Accumulated` | `origin:Iterating<T>`; `initial:R`; `action:(R,T) is R` | `cursor() is Cursor<R>` | keep: Accumulated represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `AccumulatedCursor` | `origin:Cursor<T>`; `current:R`; `action:(R,T) is R` | `available() is bool`<br>`item() is R`<br>`successor() is Cursor<R>` | keep at implementation boundary: AccumulatedCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `ExhaustedAccumulatedCursor` | `origin:R` | `available() is bool`<br>`item() is R`<br>`successor() is Cursor<R>` | keep at implementation boundary: ExhaustedAccumulatedCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `Reduced` | `origin:Iterating<T>`; `initial:R`; `action:(R,T) is R` | `value() is R` | keep: Reduced represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `Reversed` | `origin:Sequencing<T>` | `cursor() is Cursor<T>`<br>`size() is int`<br>`item(index is int) is T`<br>`first() is T`<br>`last() is T` | keep: Reversed represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `ReversedCursor` | `origin:Sequencing<T>`; `position:int` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: ReversedCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `Sliced` | `origin:Iterating<T>`; `start:int`; `stop:int`; `step:int` | `cursor() is Cursor<T>` | keep: Sliced represents an immutable collection, position or typed relation; selectors and transformations belong to that relation |
| `SlicedCursor` | `origin:Cursor<T>`; `position:int`; `stop:int`; `step:int` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: SlicedCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
## std/compare.ok

small nominal compare contracts. No I/O or owned resource state. Preserve interpreter-provided nominal behavior.

Existing conformance, error and ordering tests; no generic reflection/base-object API.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Order` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `Comparing` | Domain atom / role / runtime-owned state | `order(other is T) is Order` | keep: nominal observable contract; no implementation inheritance or reflection |
## std/encoding/base32.ok

validated wire/text representations. Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Base32Encoded` | `origin:@byte` | `string() is string` | keep: Base32Encoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Base32Decoded` | `origin:string` | `bytes() is @byte` | keep: Base32Decoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidBase32` | `invalid_offset:int` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/encoding/base64.ok

validated wire/text representations. Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Base64Alphabet` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `Base64Encoded` | `origin:@byte`; `alphabet:Base64Alphabet` | `string() is string` | keep: Base64Encoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Base64Decoded` | `origin:string`; `alphabet:Base64Alphabet` | `bytes() is @byte` | keep: Base64Decoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidBase64` | `invalid_offset:int` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/encoding/base85.ok

validated wire/text representations. Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Base85Alphabet` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `Base85Encoded` | `origin:@byte`; `alphabet:Base85Alphabet` | `string() is string` | keep: Base85Encoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Base85Decoded` | `origin:string`; `alphabet:Base85Alphabet` | `bytes() is @byte` | keep: Base85Decoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidBase85` | `invalid_offset:int` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/encoding/binary.ok

validated wire/text representations. Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Signedness` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `FloatWidth` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `IntegerFromBytes` | `origin:@byte`; `signedness:Signedness`; `order:ByteOrder` | `value() is int` | keep: IntegerFromBytes denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `IntegerBytes` | `origin:int`; `size:int`; `signedness:Signedness`; `order:ByteOrder` | `bytes() is @byte` | keep: IntegerBytes denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `FloatFromBytes` | `origin:@byte`; `width:FloatWidth`; `order:ByteOrder` | `value() is float` | keep: FloatFromBytes denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `FloatBytes` | `origin:float`; `width:FloatWidth`; `order:ByteOrder` | `bytes() is @byte` | keep: FloatBytes denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
## std/encoding/hex.ok

validated wire/text representations. Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `HexEncoded` | `origin:@byte` | `string() is string` | keep: HexEncoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `HexDecoded` | `origin:string` | `bytes() is @byte` | keep: HexDecoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidHex` | `invalid_offset:int` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/encoding/pem.ok

validated wire/text representations. Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `PemEncoded` | `kind:string`; `origin:@byte` | `string() is string` | keep: PemEncoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `PemBlock` | `origin:string` | `kind() is string`<br>`data() is @byte` | redesign: ByteSource.bytes(), remove data() |
| `PemBlocks` | `origin:string` | `cursor() is Cursor<PemBlock>` | keep: PemBlocks denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `PemBlockCursor` | `origin:string`; `position:int`; `size:int` | `available() is bool`<br>`item() is PemBlock`<br>`successor() is Cursor<PemBlock>` | keep at implementation boundary: PemBlockCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `InvalidPem` | `invalid_offset:int` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/encoding/quoted_printable.ok

validated wire/text representations. Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `QuotedPrintableEncoded` | `origin:@byte` | `bytes() is @byte` | keep: QuotedPrintableEncoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `QuotedPrintableDecoded` | `origin:@byte` | `bytes() is @byte` | keep: QuotedPrintableDecoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidQuotedPrintable` | `invalid_offset:int` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/encoding/single_byte.ok

validated wire/text representations. Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `AsciiEncoded` | `origin:string` | `bytes() is @byte` | keep: AsciiEncoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `AsciiDecoded` | `origin:@byte` | `string() is string` | keep: AsciiDecoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Latin1Encoded` | `origin:string` | `bytes() is @byte` | keep: Latin1Encoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Latin1Decoded` | `origin:@byte` | `string() is string` | keep: Latin1Decoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `UnrepresentableAsciiCharacter` | `invalid_offset:int` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `UnrepresentableLatin1Character` | `invalid_offset:int` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidAsciiByte` | `invalid_offset:int` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/encoding/utf.ok

validated wire/text representations. Pure conversion; no external resource ownership. Decoding failures keep exact offsets.

Roundtrip, malformed input, Unicode/binary boundaries; no implicit arbitrary-object conversion.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `ByteOrder` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `Utf8Encoded` | `origin:string` | `bytes() is @byte` | keep: Utf8Encoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Utf8Decoded` | `origin:@byte` | `string() is string` | keep: Utf8Decoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Utf16Encoded` | `origin:string`; `order:ByteOrder` | `bytes() is @byte` | keep: Utf16Encoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Utf16Decoded` | `origin:@byte`; `order:ByteOrder` | `string() is string` | keep: Utf16Decoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Utf32Encoded` | `origin:string`; `order:ByteOrder` | `bytes() is @byte` | keep: Utf32Encoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Utf32Decoded` | `origin:@byte`; `order:ByteOrder` | `string() is string` | keep: Utf32Decoded denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidUtf8` | `invalid_offset:int` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidUtf16` | `invalid_offset:int` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidUtf32` | `invalid_offset:int` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/error.ok

small nominal error contracts. No I/O or owned resource state. Preserve interpreter-provided nominal behavior.

Existing conformance, error and ordering tests; no generic reflection/base-object API.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `InvalidArgument` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidState` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `OutOfRange` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `UnsupportedOperation` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/fs/directory.ok

filesystem locators, observations and owned resources. Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `DirectoryLocating` | Domain atom / role / runtime-owned state | `path() is Path` | keep: nominal observable contract; no implementation inheritance or reflection |
| `Directory` | `origin:Path` | `path() is Path`<br>`existence() is bool`<br>`create() is void`<br>`remove() is void`<br>`move(destination is Path) is void` | keep with module effect policy: Directory represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `DirectoryEntry` | `origin:Path` | `path() is Path`<br>`kind() is FileKind`<br>`stat() is FileStat` | keep with module effect policy: DirectoryEntry represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `DirectoryEntries` | `origin:DirectoryLocating` | `cursor() is Cursor<DirectoryEntry>` | keep with module effect policy: DirectoryEntries represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `DirectoryEntryCursor` | `parent:Path`; `names:@string`; `position:int` | `available() is bool`<br>`item() is DirectoryEntry`<br>`successor() is Cursor<DirectoryEntry>` | keep at implementation boundary: DirectoryEntryCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `Descendants` | `origin:DirectoryLocating` | `cursor() is Cursor<DirectoryEntry>` | keep with module effect policy: Descendants represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `DescendantCursor` | `current:Cursor<DirectoryEntry>`; `following:List<Cursor<DirectoryEntry>>` | `available() is bool`<br>`item() is DirectoryEntry`<br>`successor() is Cursor<DirectoryEntry>` | keep at implementation boundary: DescendantCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `DescendantCursorFrom` | `current:Cursor<DirectoryEntry>`; `following:List<Cursor<DirectoryEntry>>` | `value() is Cursor<DirectoryEntry>` | keep at implementation boundary: DescendantCursorFrom represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `ExhaustedDescendantCursor` | Domain atom / role / runtime-owned state | `available() is bool`<br>`item() is DirectoryEntry`<br>`successor() is Cursor<DirectoryEntry>` | keep at implementation boundary: ExhaustedDescendantCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `RemovedTree` | `origin:DirectoryLocating` | `value() is Path` | replace: DirectoryTree.remove() is void; no effectful projection |
| `DirectoryNotEmpty` | `origin:Path` | `path() is Path`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/fs/file.ok

filesystem locators, observations and owned resources. Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `FileMode` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `FileLocking` | Domain atom / role / runtime-owned state | `closed() is bool`<br>`close() is void` | keep: nominal observable contract; no implementation inheritance or reflection |
| `File` | `state:FileState` | `read() is string`<br>`read(limit is int) is @byte`<br>`write(source is ByteSource) is void`<br>`write(values is ...string) is void`<br>`writeln(values is ...string) is void`<br>`flush() is void`<br>`close() is void` | split: owner plus FileBytes/FileText capability views; remove read/write facade |
| `SeekOrigin` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `FilePosition` | `state:FileState` | `offset() is int`<br>`seek(offset is int, from is SeekOrigin) is void`<br>`truncate(size is int) is void` | keep with module effect policy: FilePosition represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `FileEntry` | `origin:Path` | `path() is Path`<br>`existence() is bool`<br>`stat() is FileStat`<br>`remove() is void`<br>`move(destination is Path) is void` | keep with module effect policy: FileEntry represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `FileContent` | `origin:FileEntry` | `bytes() is @byte` | keep with module effect policy: FileContent represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `ConfiguredFileState` | `mode:FileMode`; `path:Path`; `encoding:string`; `binary:bool` | `value() is FileState` | keep at implementation boundary: ConfiguredFileState represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `SupportedFileEncoding` | `origin:string` | `value() is bool` | keep with module effect policy: SupportedFileEncoding represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `FileModeCode` | `origin:FileMode` | `value() is int` | keep at implementation boundary: FileModeCode represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `SeekOriginCode` | `origin:SeekOrigin` | `value() is int` | keep at implementation boundary: SeekOriginCode represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `FileError` | `failure:NativeFailure` | `value() is Error` | keep at implementation boundary: FileError represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `InvalidFileOperation` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `UnsupportedFileEncoding` | `encoding:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `FileSystemFailure` | `failure_code:string`; `message:string` | `code() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/fs/glob.ok

filesystem locators, observations and owned resources. Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `MatchingPaths` | `origin:DirectoryLocating`; `glob:string` | `cursor() is Cursor<Path>` | keep with module effect policy: MatchingPaths represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `MatchingPathCursor` | `paths:@string`; `position:int` | `available() is bool`<br>`item() is Path`<br>`successor() is Cursor<Path>` | keep at implementation boundary: MatchingPathCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `InvalidGlobPattern` | `origin:string`; `message:string` | `pattern() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/fs/link.ok

filesystem locators, observations and owned resources. Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `SymbolicLink` | `origin:Path` | `path() is Path`<br>`existence() is bool`<br>`target() is Path`<br>`create(target is Path) is void`<br>`remove() is void` | keep with module effect policy: SymbolicLink represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `HardLink` | `source:FileEntry`; `destination:Path` | `create() is void` | keep with module effect policy: HardLink represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `NotSymbolicLink` | `origin:Path` | `path() is Path`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidHardLinkSource` | `origin:Path` | `path() is Path`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/fs/lock.ok

filesystem locators, observations and owned resources. Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `FileLockMode` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `FileLockWaiting` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `FileLock` | `state:FileLockState` | `closed() is bool`<br>`close() is void` | keep with module effect policy: FileLock represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `FileLockModeCode` | `origin:FileLockMode` | `value() is int` | keep at implementation boundary: FileLockModeCode represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `FileLockWaitingCode` | `origin:FileLockWaiting` | `value() is int` | keep at implementation boundary: FileLockWaitingCode represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `UnavailableFileLock` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/fs/path.ok

filesystem locators, observations and owned resources. Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `PathStyle` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `Path` | `origin:string`; `path_style:PathStyle` | `parent() is Path`<br>`child(part is string) is Path`<br>`absolute() is bool`<br>`order(other is Path) is Order`<br>`string() is string` | keep with module effect policy: Path represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `NativePathStyle` | Domain atom / role / runtime-owned state | `value() is PathStyle` | keep with module effect policy: NativePathStyle represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `PathStyleCode` | `origin:PathStyle` | `value() is int` | keep at implementation boundary: PathStyleCode represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `InvalidPath` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/fs/stat.ok

filesystem locators, observations and owned resources. Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `FileKind` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `FileStat` | `file_kind:FileKind`; `byte_size:int`; `permission_mode:PermissionMode`; `file_times:FileTimes`; `file_identity:FileIdentity` | `kind() is FileKind`<br>`size() is int`<br>`mode() is PermissionMode`<br>`times() is FileTimes`<br>`identity() is FileIdentity` | keep with module effect policy: FileStat represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `FileTimes` | `accessed:int`; `modified:int`; `changed:int`; `created:int` | `access() is Instant`<br>`modification() is Instant`<br>`change() is Instant`<br>`creation() is Instant` | keep with module effect policy: FileTimes represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `FileIdentity` | `device_number:int`; `inode_number:BigInt` | `device() is int`<br>`inode() is BigInt` | redesign: same(other), remove device/inode representation getters |
| `Permissions` | `read_bit:bool`; `write_bit:bool`; `execute_bit:bool` | `readable() is bool`<br>`writable() is bool`<br>`executable() is bool` | keep with module effect policy: Permissions represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `PermissionMode` | `bits:int` | `owner() is Permissions`<br>`group() is Permissions`<br>`others() is Permissions`<br>`special() is int` | keep with module effect policy: PermissionMode represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `FileStatFromMap` | `origin:%string:int` | `value() is FileStat` | keep at implementation boundary: FileStatFromMap represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `EpochInstant` | `seconds:int` | `value() is Instant` | keep at implementation boundary: EpochInstant represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `PermissionBits` | `bits:int` | `value() is Permissions` | keep at implementation boundary: PermissionBits represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `FileKindFromCode` | `code:int` | `value() is FileKind` | keep at implementation boundary: FileKindFromCode represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `MissingCreationTime` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidPermissionMode` | `bits:int` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/fs/temp.ok

filesystem locators, observations and owned resources. Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `TemporaryFile` | `state:FileState` | `bytes(limit is int) is @byte`<br>`write(source is ByteSource) is void`<br>`flush() is void`<br>`closed() is bool`<br>`close() is void` | keep with module effect policy: TemporaryFile represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `TemporaryFilePath` | `state:FileState` | `value() is Path` | keep with module effect policy: TemporaryFilePath represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `PreservedTemporaryFile` | `state:FileState`; `destination:Path` | `value() is FileEntry` | replace: TemporaryFilePreservation.preserve() is void |
| `TemporaryDirectory` | `state:TemporaryDirectoryState` | `path() is Path`<br>`closed() is bool`<br>`close() is void` | keep with module effect policy: TemporaryDirectory represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
## std/fs/watch.ok

filesystem locators, observations and owned resources. Path values are pure; resource operations are explicit. Lazy file views share state; tree removal/preservation are void commands.

Real temporary filesystem, symlink boundaries, missing paths, repeated close, escaped views; no raw handles or implicit deletion in queries.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `FileEventKind` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `FileEvent` | `origin:Path`; `classification:FileEventKind`; `previous:string` | `path() is Path`<br>`kind() is FileEventKind`<br>`previous_path() is Path` | keep with module effect policy: FileEvent represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `FileWatch` | `state:FileWatchState` | `async event() is FileEvent`<br>`closed() is bool`<br>`close() is void` | keep with module effect policy: FileWatch represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `MissingPreviousPath` | `origin:Path` | `path() is Path`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/html/events.ok

HTML token observations and explicit event protocols. Immutable event sequence over private parser state. Grammar kind is a wire category, not object-class reflection.

Malformed text, event ordering and typed event sinks; no browser DOM, arbitrary field inspection or sanitization promise.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `HtmlEventKind` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `HtmlAttribute` | `attribute_name:string`; `has_value:bool`; `attribute_value:string` | `name() is string`<br>`valued() is bool`<br>`value() is string` | keep: HtmlAttribute denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `StartingHtmlTag` | Domain atom / role / runtime-owned state | `name() is string`<br>`attributes() is Sequencing<HtmlAttribute>`<br>`self_closing() is bool` | keep: nominal observable contract; no implementation inheritance or reflection |
| `EndingHtmlTag` | Domain atom / role / runtime-owned state | `name() is string` | keep: nominal observable contract; no implementation inheritance or reflection |
| `ContainingHtmlText` | Domain atom / role / runtime-owned state | `value() is string` | keep: nominal observable contract; no implementation inheritance or reflection |
| `ContainingHtmlComment` | Domain atom / role / runtime-owned state | `value() is string` | keep: nominal observable contract; no implementation inheritance or reflection |
| `DeclaringHtml` | Domain atom / role / runtime-owned state | `value() is string` | keep: nominal observable contract; no implementation inheritance or reflection |
| `ProcessingHtmlInstruction` | Domain atom / role / runtime-owned state | `target() is string`<br>`data() is string` | keep: nominal observable contract; no implementation inheritance or reflection |
| `HandlingHtmlEvent` | Domain atom / role / runtime-owned state | `handle(event is StartingHtmlTag) is void`<br>`handle(event is EndingHtmlTag) is void`<br>`handle(event is ContainingHtmlText) is void`<br>`handle(event is ContainingHtmlComment) is void`<br>`handle(event is DeclaringHtml) is void`<br>`handle(event is ProcessingHtmlInstruction) is void` | keep: nominal observable contract; no implementation inheritance or reflection |
| `HtmlEvent` | Domain atom / role / runtime-owned state | `kind() is HtmlEventKind`<br>`dispatch(to is HandlingHtmlEvent) is void` | keep: nominal observable contract; no implementation inheritance or reflection |
| `HtmlStartTag` | `tag_name:string`; `tag_attributes:Sequencing<HtmlAttribute>`; `closes_self:bool` | `kind() is HtmlEventKind`<br>`name() is string`<br>`attributes() is Sequencing<HtmlAttribute>`<br>`self_closing() is bool`<br>`dispatch(to is HandlingHtmlEvent) is void` | keep: HtmlStartTag denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `HtmlEndTag` | `tag_name:string` | `kind() is HtmlEventKind`<br>`name() is string`<br>`dispatch(to is HandlingHtmlEvent) is void` | keep: HtmlEndTag denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `HtmlText` | `origin:string` | `kind() is HtmlEventKind`<br>`value() is string`<br>`dispatch(to is HandlingHtmlEvent) is void` | keep: HtmlText denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `HtmlComment` | `origin:string` | `kind() is HtmlEventKind`<br>`value() is string`<br>`dispatch(to is HandlingHtmlEvent) is void` | keep: HtmlComment denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `HtmlDeclaration` | `origin:string` | `kind() is HtmlEventKind`<br>`value() is string`<br>`dispatch(to is HandlingHtmlEvent) is void` | keep: HtmlDeclaration denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `HtmlProcessingInstruction` | `instruction_target:string`; `instruction_data:string` | `kind() is HtmlEventKind`<br>`target() is string`<br>`data() is string`<br>`dispatch(to is HandlingHtmlEvent) is void` | keep: HtmlProcessingInstruction denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `HtmlEvents` | `state:HtmlEventsState` | `cursor() is Cursor<HtmlEvent>` | keep: HtmlEvents denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `HtmlEventCursor` | `origin:HtmlEventsState`; `position:int`; `amount:int` | `available() is bool`<br>`item() is HtmlEvent`<br>`successor() is Cursor<HtmlEvent>` | keep at implementation boundary: HtmlEventCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
## std/io/buffer.ok

explicit stream capabilities and decorators. Reads/writes are capability effects. Borrowed adapters never close origins; owned buffers/states clean up deterministically.

ByteBuffer fakes and real stdio, short writes, bounded reads, EOF, aliases; no format/type inference or hidden ownership transfer.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `ByteBuffer` | `state:BufferState` | `reading() is Reading`<br>`writing() is Writing`<br>`content() is BufferContent`<br>`closing() is Closing` | keep with module effect policy: ByteBuffer represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `BufferReading` | `state:BufferState` | `bytes(limit is int) is @byte` | keep with module effect policy: BufferReading represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `BufferWriting` | `state:BufferState` | `write(source is ByteSource) is void`<br>`flush() is void` | keep with module effect policy: BufferWriting represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `BufferClosing` | `state:BufferState` | `closed() is bool`<br>`close() is void` | keep with module effect policy: BufferClosing represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `BufferContent` | `state:BufferState` | `bytes() is @byte`<br>`size() is int` | keep with module effect policy: BufferContent represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `ClosedBuffer` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/io/decorators.ok

explicit stream capabilities and decorators. Reads/writes are capability effects. Borrowed adapters never close origins; owned buffers/states clean up deterministically.

ByteBuffer fakes and real stdio, short writes, bounded reads, EOF, aliases; no format/type inference or hidden ownership transfer.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `BufferedReading` | `origin:Reading`; `capacity:int`; `cache:BufferState` | `bytes(limit is int) is @byte`<br>`closed() is bool`<br>`close() is void` | keep with module effect policy: BufferedReading represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `LimitedReading` | `origin:Reading`; `state:LimitedReadingState` | `bytes(size is int) is @byte` | keep with module effect policy: LimitedReading represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `ChainedReading` | `origins:Sequencing<Reading>`; `state:ChainedReadingState` | `bytes(size is int) is @byte` | keep with module effect policy: ChainedReading represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `ByteSnapshot` | `value:@byte` | `bytes() is @byte` | keep with module effect policy: ByteSnapshot represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `TeeWriting` | `destinations:Sequencing<Writing>` | `write(source is ByteSource) is void`<br>`flush() is void` | keep with module effect policy: TeeWriting represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `Discarding` | Domain atom / role / runtime-owned state | `write(source is ByteSource) is void`<br>`flush() is void` | keep with module effect policy: Discarding represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `ClosedReading` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidReading` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/io/stdin.ok

explicit stream capabilities and decorators. Reads/writes are capability effects. Borrowed adapters never close origins; owned buffers/states clean up deterministically.

ByteBuffer fakes and real stdio, short writes, bounded reads, EOF, aliases; no format/type inference or hidden ownership transfer.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Stdin` | Domain atom / role / runtime-owned state | `bytes(limit is int) is @byte` | keep with module effect policy: Stdin represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `Utf8Input` | `origin:Reading`; `state:Utf8InputState` | `line() is string`<br>`text() is string`<br>`closed() is bool`<br>`close() is void` | keep with module effect policy: Utf8Input represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `EndOfInput` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `ClosedInput` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/io/stdout.ok

explicit stream capabilities and decorators. Reads/writes are capability effects. Borrowed adapters never close origins; owned buffers/states clean up deterministically.

ByteBuffer fakes and real stdio, short writes, bounded reads, EOF, aliases; no format/type inference or hidden ownership transfer.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Stdout` | Domain atom / role / runtime-owned state | `write(source is ByteSource) is void`<br>`flush() is void`<br>`write(values is ...string) is void`<br>`writeln(values is ...string) is void` | keep with module effect policy: Stdout represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `Stderr` | Domain atom / role / runtime-owned state | `write(source is ByteSource) is void`<br>`flush() is void`<br>`write(values is ...string) is void`<br>`writeln(values is ...string) is void` | keep with module effect policy: Stderr represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
## std/io/stream.ok

explicit stream capabilities and decorators. Reads/writes are capability effects. Borrowed adapters never close origins; owned buffers/states clean up deterministically.

ByteBuffer fakes and real stdio, short writes, bounded reads, EOF, aliases; no format/type inference or hidden ownership transfer.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `ByteSource` | Domain atom / role / runtime-owned state | `bytes() is @byte` | keep: nominal observable contract; no implementation inheritance or reflection |
| `Writing` | Domain atom / role / runtime-owned state | `write(source is ByteSource) is void`<br>`flush() is void` | keep: nominal observable contract; no implementation inheritance or reflection |
| `Reading` | Domain atom / role / runtime-owned state | `bytes(limit is int) is @byte` | keep: nominal observable contract; no implementation inheritance or reflection |
| `WritingText` | Domain atom / role / runtime-owned state | `write(values is ...string) is void`<br>`writeln(values is ...string) is void` | keep: nominal observable contract; no implementation inheritance or reflection |
| `ReadingText` | Domain atom / role / runtime-owned state | `line() is string`<br>`text() is string` | keep: nominal observable contract; no implementation inheritance or reflection |
| `Closing` | Domain atom / role / runtime-owned state | `closed() is bool`<br>`close() is void` | keep: nominal observable contract; no implementation inheritance or reflection |
| `IncompleteWrite` | `amount:int` | `written() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidReadLimit` | `value:int` | `limit() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/json/parse.ok

explicit JSON grammar values, parsing and emission. Immutable nodes and bounded lazy parse state; print emits only through an explicit JsonWriting capability. Rendering owns its local buffer.

Nested literals, exact decimals, duplicates, invalid/deep input and custom nominal implementations; no object reflection or null-like language value.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `ParsedJson` | `state:JsonParseState` | `kind() is JsonKind`<br>`dispatch(to is HandlingJsonValue) is void` | redesign: named JSON protocol, keep bounded lazy parsing |
| `JsonNode` | `origin:JsonNodeState` | `kind() is JsonKind`<br>`dispatch(to is HandlingJsonValue) is void` | redesign: wire-decoding bridge to named JSON protocol |
| `InvalidJson` | `source_offset:int`; `source_line:int`; `source_column:int`; `message:string` | `offset() is int`<br>`line() is int`<br>`column() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/json/value.ok

explicit JSON grammar values, parsing and emission. Immutable nodes and bounded lazy parse state; print emits only through an explicit JsonWriting capability. Rendering owns its local buffer.

Nested literals, exact decimals, duplicates, invalid/deep input and custom nominal implementations; no object reflection or null-like language value.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `JsonKind` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `HandlingJsonValue` | Domain atom / role / runtime-owned state | `handle() is void`<br>`handle(value is bool) is void`<br>`handle(value is Decimal) is void`<br>`handle(value is string) is void`<br>`handle(values is Sequencing<JsonValue>) is void`<br>`handle(members is Sequencing<JsonMember>) is void` | replace: JsonWriting with named protocol callbacks |
| `JsonValue` | Domain atom / role / runtime-owned state | `kind() is JsonKind`<br>`dispatch(to is HandlingJsonValue) is void` | redesign: print(to is JsonWriting), preserve JSON grammar kinds |
| `JsonMember` | `member_name:string`; `member_value:JsonValue` | `name() is string`<br>`value() is JsonValue` | keep: JsonMember denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `JsonNull` | Domain atom / role / runtime-owned state | `kind() is JsonKind`<br>`dispatch(to is HandlingJsonValue) is void` | redesign: explicit null_value protocol event, never absent object |
| `JsonBoolean` | `origin:bool` | `kind() is JsonKind`<br>`value() is bool`<br>`dispatch(to is HandlingJsonValue) is void` | redesign: protocol emission, remove scalar value getter |
| `JsonNumber` | `origin:Decimal` | `kind() is JsonKind`<br>`value() is Decimal`<br>`dispatch(to is HandlingJsonValue) is void` | redesign: protocol emission, remove scalar value getter |
| `JsonText` | `origin:string` | `kind() is JsonKind`<br>`value() is string`<br>`dispatch(to is HandlingJsonValue) is void` | redesign: protocol emission, remove scalar value getter |
| `JsonArray` | `origin:@JsonValue` | `kind() is JsonKind`<br>`values() is Sequencing<JsonValue>`<br>`dispatch(to is HandlingJsonValue) is void` | redesign: protocol emission with immutable snapshot semantics |
| `JsonObject` | `origin:@JsonMember` | `kind() is JsonKind`<br>`membership(name is string) is bool`<br>`value(name is string) is JsonValue`<br>`members() is Sequencing<JsonMember>`<br>`dispatch(to is HandlingJsonValue) is void` | redesign: protocol emission, retain associative member selection |
| `JsonMemberIndex` | `origin:@JsonMember`; `sought:string` | `value() is int` | keep: JsonMemberIndex denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `MissingJsonMember` | `origin:string` | `name() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `DuplicateJsonMember` | `origin:string`; `source_offset:int`; `source_line:int`; `source_column:int`; `source_located:bool` | `name() is string`<br>`located() is bool`<br>`offset() is int`<br>`line() is int`<br>`column() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `MissingJsonLocation` | `origin:string` | `name() is string`<br>`text() is string` | keep: typed absence of diagnostic coordinates |
## std/math/aggregate.ok

validated math aggregate representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Minimum` | `origin:Iterating<T>` | `value() is T` | keep: Minimum denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Maximum` | `origin:Iterating<T>` | `value() is T` | keep: Maximum denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Sum` | `origin:Iterating<float>` | `value() is float` | keep: Sum denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `AccurateSum` | `origin:Iterating<float>` | `value() is float` | keep: AccurateSum denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `AccurateTotal` | `total:float`; `correction:float` | `addition(value is float) is AccurateTotal`<br>`value() is float` | keep: AccurateTotal denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Product` | `origin:Iterating<float>` | `value() is float` | keep: Product denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `SumOfProducts` | `left:Iterating<float>`; `right:Iterating<float>` | `value() is float` | keep: SumOfProducts denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Hypotenuse` | `origin:Iterating<float>` | `value() is float` | keep: Hypotenuse denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Distance` | `left:Sequencing<float>`; `right:Sequencing<float>` | `value() is float` | keep: Distance denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `ScaledSquares` | `scaling:float`; `total:float` | `addition(value is float) is ScaledSquares`<br>`value() is float` | keep: ScaledSquares denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
## std/math/constants.ok

validated math constants representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Pi` | Domain atom / role / runtime-owned state | `value() is float` | keep: Pi denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Tau` | Domain atom / role / runtime-owned state | `value() is float` | keep: Tau denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Euler` | Domain atom / role / runtime-owned state | `value() is float` | keep: Euler denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
## std/math/decimal.ok

validated math decimal representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Decimal` | `origin:string` | `coefficient() is BigInt`<br>`exponent() is int`<br>`order(other is Decimal) is Order`<br>`string() is string` | keep: Decimal denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidDecimal` | `origin:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/math/rounding.ok

validated math rounding representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Rounding` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
## std/net/failure.ok

portable networking vocabulary. Only the currently implemented enums/errors are approved; socket state lives in runtime/network.

Existing native boundary tests; full IP/TCP/TLS behavior is deferred, not implemented by these declarations.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `NetworkOperation` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `NetworkFailureKind` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `NetworkFailure` | `operation:NetworkOperation`; `kind:NetworkFailureKind`; `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `NetworkClosed` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |

2026-09-25 completion: NetworkFailure adds operation()/kind() observations while
retaining its constructor and text. NetworkError binds the attempted operation
and NativeFailure at the adapter boundary; applicable() and value() classify
portable terminal core.net outcomes without I/O, host codes or message heuristics.
It yields an ordinary Error and does not expose the transport failure as a field
schema. Inapplicable outcomes remain the original catch's responsibility.
See the [mapping contract](network-failure.md) and focused evidence.

## std/net/endpoint.ok

Numeric endpoint identity is the immutable address, port and optional IPv6 zone.
Observations preserve these coordinates and format canonical display text without
DNS, interface lookup or a socket capability. InvalidPort and InvalidIpv6Zone
retain typed diagnostics. No one-action Port wrapper or service-name lookup.
HostName/HostEndpoint remain blocked on IDNA; destination port-zero rejection is
owned by the future consuming operation. [Contract](ip-endpoint.md).

## std/net/interface.ok

NetworkInterface is one coherent immutable observation of a system interface:
exact name/index, ordered assigned host/prefix values and independent flags.
The explicit snapshot constructor supports provider data and fixtures without
OS lookup or mutable configuration. No generic record map or platform flag mask.
SystemInterfaces enumeration is not implemented. [Contract](network-interface.md).

## std/net/ip.ok

Immutable numeric address and CIDR values, pinned IANA classification, lazy
inclusive traversal and host/prefix interface observations. The existing numeric
provider handles parsing/formatting without DNS; socket state is separate.
Value, registry, every-prefix CIDR and cursor boundary tests are recorded in the
linked completion roadmap. These values do not imply implemented TCP/TLS.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `IpVersion` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `IpProperty` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `IpAddress` | version, packed bytes, canonical text | version/bytes/string/order/properties | keep: coherent numeric value with pinned classification |
| `IpNetwork` | normalized base and prefix | network/broadcast/prefix/membership/size | keep: inclusive interval, exact BigInt cardinality |
| `IpAddresses` | one IpNetwork | cursor | keep: lazy Iterating, no eager range materialization |
| `IpInterface` | host address and IpNetwork | address/network/string | keep: preserves host bits beside normalized network |
| `InvalidIpAddress`, `InvalidIpPrefix` | invalid input coordinates | text; prefix for InvalidIpPrefix | keep: typed constructor failures |

IpAddressCursor/ExhaustedIpAddressCursor, IpNetworkBytes and PackedIpText are
implementation helpers. Their roles and bounded data manipulation support the
public entities without exposing native envelopes through those public APIs.

## std/crypto/key.ok and std/crypto/x509/certificate.ok

Certificate is a cohesive immutable DER-backed object: grouped names, validity
and PublicKey observers return domain entities. The deterministic private facts
envelope is provider ABI, not an application DTO; native pointers are acquired
and freed within parsing. Equal DER yields equal objects. Unsupported name
encoding remains explicit, and parsing grants no signature/trust assertion.
PublicKey exposes standardized SPKI bytes without private-key capability.
Internal CertificateName/CertificatePublicKey adapters are not independent
validation APIs. [Contract](x509-certificate.md),
investigation.

## CertificateNames/CertificateChain and std/crypto/x509/trust.ok

SAN identities, supplied untrusted chains and locally selected roots have
separate authority. DNS text and nominal IPs remain distinct; no CN fallback,
resolution or hostname matching occurs in projection. Chain/store constructors
snapshot bounded sources once and expose only certificate entities. Explicit
empty roots grant no anchors. PemTrustStore accepts strict CERTIFICATE-only
text, rejects private-key labels and reads no files. Native bulk source snapshots
remove complete-document cursor reparsing without a global text cache. System
acquisition, extension metadata and explicit TLS-purpose path verification are implemented.
[Contract](certificate-observers.md),
investigation.

## SystemTrustStore/SystemTrustUnavailable

Only result() performs root acquisition. Discovery reads linked OpenSSL paths
without scanning candidate contents; regular-file/nonblocking checks and budgets
cover the actual discovery path, including SSL_CERT_FILE FIFO regression.
Snapshots retain validated exact DER, first-occurrence deduplication and portable
errors without host paths. Auxiliary trust is rejected rather than discarded;
OS-specific stores and revocation remain separate. No mutable global root cache.
[Contract](system-trust-store.md),
investigation.

## VerifiedCertificateChain/CertificateIdentity/CertificateFailure

Explicit identity entities preserve DNS/IP typing without changing existing
same-arity constructor dispatch. Construction snapshots roots; observations
verify exact original DER against explicit time/purpose/identity without I/O.
Native path and metadata syntax policies are separated from trust-anchor input:
actual legacy KU roots remain usable as anchors, while leaf/intermediate syntax
stays strict. No supplied intermediate gains authority, no CN fallback or
unrestricted unknown purpose is accepted, and get1_chain references are freed.
Typed reasons and a context-allocation cleanup regression preserve failure
categories. [Contract](certificate-verification.md),
investigation.

## CertificateExtensions/CertificatePurpose/CertificateKeyUsage

Extension metadata stays separate from verification policy. Exact unknown OIDs,
critical flags and payloads are addressable; purposes use OID objects instead of
a lossy closed enum. KU uses its finite named bits with minimal DER encoding.
Constraints preserve absent versus zero depth and reject overflow explicitly.
Private CertificateObjectIdentifier shares the existing exact OID rules and
translates errors into InvalidCertificate. These objects grant no signing/trust,
hostname or purpose acceptance. [Contract](certificate-extensions.md),
investigation.

## std/crypto/x509/name.ok

Explicit RDN groups retain boundaries and repeated OIDs; no flattened display
string/hash is treated as the name. Bounded snapshots sample each source item
once and preserve decoded text exactly. Native DER integration must retain the
original groups; no X509_NAME hash reconstruction is accepted.
[Contract](x509-name.md), investigation.

## std/crypto/x509/value.ok

Validity retains an exact wall-clock interval without current-time observation.
Constraints use constructor shape for depth absence/presence, preserving zero
and rejecting non-CA/negative limits. These cohesive metadata values grant no
signature/chain/identity assurance. Host parsing is [implemented](x509-certificate.md);
trust remains next.
[Contract](x509-values.md).

## std/net/dns.ok

Resolving establishes nominal async forward/reverse actions. FixedResolution
snapshots explicit typed records rather than unsupported arbitrary object keys;
bounded sorting preserves first-occurrence answer order. SystemResolution owns
worker Future lifetimes without claiming interruption of running libc calls.
UnknownHost/UnknownAddress retain requested identities; transient failures remain
DnsFailure. Raw flags/sockaddr are private. [Fixed](fixed-resolution.md),
[system](system-resolution.md).

## std/net/stream.ok

AsyncReading and AsyncWriting establish separate nominal asynchronous contracts;
no synchronous stream-write adaptation, flush or implicit resource ownership.
ConnectionEndpoints<T> retains one directional pair without resolution or I/O.
Concrete IP stream halves now share shutdown, read admission and FIFO write
ownership across aliases. StreamConnection owns the native capability; halves
borrow it and perform directional shutdown. Pending guards are owned before
eager acquisition. [Contract](tcp-stream.md), race investigation.

## std/net/tls.ok — client policy

Finite TlsVersion selection and ALPN source snapshots do not grant a connection.
Explicit empty trust never triggers system defaults. Opaque ALPN names preserve
NUL/high octets and case; Unicode transcoding and duplicate names are rejected.
Counts precede item/cursor access; later source changes cannot alter the policy.
Root limits reuse TrustedCertificates. Transport, identity verification and
negotiation remain separate. [Contract](tls-client-policy.md),
investigation.

## std/net/tls.ok — owned verified client transport

Managed TCP is consumed only at started handshake, after drained/open checks.
Every old alias becomes closed; negotiation alone cannot authorize application
I/O. The actual peer uses explicit bounded trust, nominal DNS/IP identity,
server purpose and exact post-handshake clock time. One direction multiplexer
shares WANT registrations. Read cancellation preserves ownership; write/shutdown
cancellation disposes it. Partial progress preserves offsets; raw EOF is
truncation. The FIFO guard-grant race reuses StreamWriteCleanup. Metadata remains
an immutable certificate/endpoint/version/cipher/ALPN snapshot after close.
[Contract](tls-client-transport.md),
investigation.

## std/net/tcp.ok

TcpOptions and TcpListeningOptions retain cohesive connection/listener requests
with explicit defaults and non-negative backlog validation. Construction performs
no socket I/O or runtime capability check. Their few observations do not form a
mutable socket facade; application and unsupported-setting failures belong to
the socket operations. The numeric TCP provider now applies these requests,
publishes only connected/listening owners, retains actual endpoint snapshots
and separates binding/listening failures. Accepted owners are independent.
Hostname racing and TLS remain open. [Contract](tcp-stream.md).

## std/net/udp.ok

Datagram retains one coherent packet observation: sender, exact retained bytes
and independent truncation. ByteSource allows finite payload consumption without
stream semantics or implicit sender forwarding. No I/O or inferred metadata;
live packet I/O remains open. [Contract](udp-datagram.md).
UdpOptions retains three independent boolean requests with defaults false; the
binding owns platform validation and applying settings before bind.
[Option contract](udp-options.md).
UdpBinding retains an immutable numeric request. Its async socket action explicitly
acquires a nonblocking native resource; no DNS is implied. UdpSocket currently
owns endpoint observation and closure only, including deterministic destruction
and shared closed state. Its native-state constructor is infrastructure, not the
normal acquisition API. Datagram I/O is not claimed by this initial surface.
[Binding and ownership contract](udp-binding.md).
DatagramTooLarge is an immutable Error observation with a non-negative attempted
byte count. It neither stores a payload/capability nor decides platform limits.
Send-side validation remains open. [Failure contract](udp-size-failure.md).

## std/numbers/bigint.ok

validated numbers bigint representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `BigInt` | `origin:string` | `sign() is int`<br>`bits() is int`<br>`order(other is BigInt) is Order`<br>`string() is string` | keep: BigInt denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidBigInt` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/os/account.ok

process/host observations and execution capabilities. Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `User` | `selected_name:string`; `current:bool` | `name() is string`<br>`identity() is string`<br>`home() is Directory` | keep with module effect policy: User represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `Group` | `selected_name:string` | `name() is string`<br>`identity() is string`<br>`users() is Sequencing<User>` | keep with module effect policy: Group represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `GroupUsers` | `group_name:string` | `cursor() is Cursor<User>`<br>`size() is int`<br>`item(index is int) is User`<br>`first() is User`<br>`last() is User` | keep with module effect policy: GroupUsers represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `GroupUserCursor` | `origin:@string`; `position:int` | `available() is bool`<br>`item() is User`<br>`successor() is Cursor<User>` | keep at implementation boundary: GroupUserCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `MissingUser` | `origin:string` | `name() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `MissingGroup` | `origin:string` | `name() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `MissingUserHome` | `origin:string` | `name() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidAccountName` | `origin:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `UnavailableAccountInformation` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/os/command.ok

process/host observations and execution capabilities. Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Commanding` | Domain atom / role / runtime-owned state | `program() is Path`<br>`arguments() is @string`<br>`environment() is Environment`<br>`directory() is DirectoryLocating`<br>`string() is string` | keep: nominal observable contract; no implementation inheritance or reflection |
| `Command` | `command_program:Path`; `command_arguments:@string`; `command_environment:Environment`; `command_directory:DirectoryLocating` | `program() is Path`<br>`arguments() is @string`<br>`environment() is Environment`<br>`directory() is DirectoryLocating`<br>`string() is string` | keep with module effect policy: Command represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `InheritedCommand` | `command_program:Path`; `command_arguments:@string` | `program() is Path`<br>`arguments() is @string`<br>`environment() is Environment`<br>`directory() is DirectoryLocating`<br>`string() is string` | keep with module effect policy: InheritedCommand represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
## std/os/command_capture.ok

process/host observations and execution capabilities. Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `CapturedStream` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `CommandCapture` | `command:Commanding`; `input:@byte`; `limit:int`; `state:ProcessState` | `async completion() is CompletedCommand` | keep with module effect policy: CommandCapture represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `CompletedCommand` | `completed_exit:Exit`; `captured_output:@byte`; `captured_errors:@byte` | `exit() is Exit`<br>`output() is @byte`<br>`errors() is @byte` | keep with module effect policy: CompletedCommand represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `InvalidCaptureLimit` | `origin:int` | `limit() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `CaptureLimitExceeded` | `captured_stream:CapturedStream`; `capture_limit:int` | `stream() is CapturedStream`<br>`limit() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/os/directory.ok

process/host observations and execution capabilities. Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `WorkingDirectory` | Domain atom / role / runtime-owned state | `path() is Path` | keep with module effect policy: WorkingDirectory represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
## std/os/environment.ok

process/host observations and execution capabilities. Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Environment` | `origin:%string:string` | `membership(name is string) is bool`<br>`variable(name is string) is string`<br>`variables() is %string:string`<br>`add(name is string, value is string) is Environment`<br>`diff(name is string) is Environment` | rename: add/diff → addition/difference, persistent snapshots |
| `HomeDirectory` | `environment:Environment` | `value() is Directory` | keep with module effect policy: HomeDirectory represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `SearchPath` | `paths:@Path` | `cursor() is Cursor<Path>`<br>`size() is int`<br>`item(index is int) is Path`<br>`first() is Path`<br>`last() is Path` | keep with module effect policy: SearchPath represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `SearchPathCursor` | `paths:@Path`; `position:int` | `available() is bool`<br>`item() is Path`<br>`successor() is Cursor<Path>` | keep at implementation boundary: SearchPathCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `MissingEnvironmentVariable` | `origin:string` | `name() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidEnvironmentVariable` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `MissingHomeDirectory` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/os/pipeline_capture.ok

process/host observations and execution capabilities. Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `PipelineStream` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `PipelineCapture` | `commands:@Commanding`; `input:@byte`; `limit:int`; `state:PipelineState` | `async completion() is CompletedPipeline` | keep with module effect policy: PipelineCapture represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `CompletedPipeline` | `completed_exits:@Exit`; `captured_output:@byte`; `captured_errors:@@byte` | `exits() is @Exit`<br>`output() is @byte`<br>`errors() is @@byte` | keep with module effect policy: CompletedPipeline represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `EmptyPipeline` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidPipelineCaptureLimit` | `origin:int` | `limit() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `PipelineCaptureLimitExceeded` | `origin_index:int`; `origin_stream:PipelineStream`; `origin_limit:int` | `command_index() is int`<br>`stream() is PipelineStream`<br>`limit() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/os/platform.ok

process/host observations and execution capabilities. Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `OperatingSystem` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `Platform` | `system:OperatingSystem`; `machine:string`; `order:ByteOrder` | `operating_system() is OperatingSystem`<br>`architecture() is string`<br>`byte_order() is ByteOrder` | keep with module effect policy: Platform represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `Host` | `host_name:string`; `processors:int` | `name() is string`<br>`cpu_count() is int` | keep with module effect policy: Host represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `UnavailableHostInformation` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/os/process.ok

process/host observations and execution capabilities. Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Signal` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `ExitKind` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `Exit` | `classification:ExitKind`; `exit_code:int` | `kind() is ExitKind`<br>`code() is int`<br>`success() is bool` | keep with module effect policy: Exit represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `Process` | `origin:Commanding`; `state:ProcessState` | `id() is int`<br>`running() is bool`<br>`async exit() is Exit`<br>`signal(value is Signal) is void`<br>`close() is void` | keep with module effect policy: Process represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `ProcessInput` | `command:Commanding`; `state:ProcessState` | `write(source is ByteSource) is void`<br>`flush() is void`<br>`closed() is bool`<br>`close() is void` | keep with module effect policy: ProcessInput represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `ProcessOutput` | `command:Commanding`; `state:ProcessState` | `bytes(limit is int) is @byte` | keep with module effect policy: ProcessOutput represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `ProcessErrors` | `command:Commanding`; `state:ProcessState` | `bytes(limit is int) is @byte` | keep with module effect policy: ProcessErrors represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `MissingExitCode` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `UnsupportedSignal` | `origin:Signal` | `signal() is Signal`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `ProcessFailure` | `failure_code:string`; `message:string` | `code() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/os/storage.ok

process/host observations and execution capabilities. Environment/platform/completion values are snapshots where stated. Process execution is explicit and shell-free; handles stay private.

Isolated environment, controlled subprocesses, failure/cleanup and exact argument boundaries; no global mutable service or shell concatenation.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Storage` | `origin:Path` | `path() is Path`<br>`usage() is StorageUsage` | keep with module effect policy: Storage represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `StorageUsage` | `total_capacity:BigInt`; `used_capacity:BigInt`; `available_capacity:BigInt` | `capacity() is BigInt`<br>`used() is BigInt`<br>`available() is BigInt` | redesign: two coordinates, derived used(), fits(), coherent constructor validation |
| `InvalidStorageUsage` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `StorageFailure` | `failure_code:string`; `message:string` | `code() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/random/distribution.ok

explicit entropy and distribution capabilities. Sampling advances only the encapsulated generator/entropy capability. Seeds and sequences are explicit.

Seed reproducibility, boundaries, malformed distributions and fake entropy; no hidden global random source in value objects.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Distribution` | Domain atom / role / runtime-owned state | `sample() is T` | keep: nominal observable contract; no implementation inheritance or reflection |
| `UniformIntegers` | `random:Randomizing`; `beginning:int`; `ending:int` | `sample() is int` | keep with module effect policy: UniformIntegers represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `UniformFloats` | `random:Randomizing`; `beginning:float`; `ending:float` | `sample() is float` | keep with module effect policy: UniformFloats represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `NormalDistribution` | `random:Randomizing`; `center:float`; `spread:float` | `sample() is float` | keep with module effect policy: NormalDistribution represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `StandardNormal` | `random:Randomizing` | `value() is float` | keep with module effect policy: StandardNormal represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `ExponentialDistribution` | `random:Randomizing`; `rate:float` | `sample() is float` | keep with module effect policy: ExponentialDistribution represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `GammaDistribution` | `random:Randomizing`; `shape:float`; `scale:float` | `sample() is float` | keep with module effect policy: GammaDistribution represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `GammaUnit` | `random:Randomizing`; `shape:float` | `value() is float` | keep with module effect policy: GammaUnit represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `GammaCandidate` | `random:Randomizing`; `shape:float` | `value() is float` | keep at implementation boundary: GammaCandidate represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `BetaDistribution` | `random:Randomizing`; `alpha:float`; `beta:float` | `sample() is float` | keep with module effect policy: BetaDistribution represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `BetaSample` | `random:Randomizing`; `alpha:float`; `beta:float` | `value() is float` | keep with module effect policy: BetaSample represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `LogNormalDistribution` | `random:Randomizing`; `center:float`; `spread:float` | `sample() is float` | keep with module effect policy: LogNormalDistribution represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `VonMisesDistribution` | `random:Randomizing`; `center:float`; `concentration:float` | `sample() is float` | keep with module effect policy: VonMisesDistribution represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `VonMisesSample` | `random:Randomizing`; `center:float`; `concentration:float` | `value() is float` | keep with module effect policy: VonMisesSample represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `VonMisesCandidate` | `random:Randomizing`; `center:float`; `ratio:float` | `value() is float` | keep at implementation boundary: VonMisesCandidate represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `ParetoDistribution` | `random:Randomizing`; `shape:float`; `scale:float` | `sample() is float` | keep with module effect policy: ParetoDistribution represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `WeibullDistribution` | `random:Randomizing`; `shape:float`; `scale:float` | `sample() is float` | keep with module effect policy: WeibullDistribution represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `Choices` | `random:Randomizing`; `origin:Sequencing<T>` | `sample() is T` | keep with module effect policy: Choices represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `WeightedChoices` | `random:Randomizing`; `origin:Sequencing<T>`; `weights:Sequencing<float>`; `scaling:float`; `total:float` | `sample() is T` | keep with module effect policy: WeightedChoices represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `WeightedSelection` | `values:Sequencing<T>`; `weights:Sequencing<float>`; `scaling:float`; `target:float`; `position:int`; `cumulative:float` | `value() is T` | keep with module effect policy: WeightedSelection represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `Samples` | `random:Randomizing`; `origin:Sequencing<T>`; `amount:int` | `sample() is @T` | keep with module effect policy: Samples represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `IndexSamples` | `random:Randomizing`; `positions:List<int>`; `amount:int`; `selected:List<int>` | `value() is List<int>` | keep with module effect policy: IndexSamples represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `ListRemoval` | `origin:List<T>`; `index:int` | `item() is T`<br>`remainder() is List<T>` | keep with module effect policy: ListRemoval represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `PermutationSamples` | `random:Randomizing`; `origin:Sequencing<T>` | `sample() is @T` | keep with module effect policy: PermutationSamples represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `InvalidDistribution` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/random/source.ok

explicit entropy and distribution capabilities. Sampling advances only the encapsulated generator/entropy capability. Seeds and sequences are explicit.

Seed reproducibility, boundaries, malformed distributions and fake entropy; no hidden global random source in value objects.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Randomizing` | Domain atom / role / runtime-owned state | `integer(start is int, stop is int) is int`<br>`float() is float`<br>`bytes(size is int) is @byte` | keep: nominal observable contract; no implementation inheritance or reflection |
| `PcgRandom` | `state:PcgState` | `integer(start is int, stop is int) is int`<br>`float() is float`<br>`bytes(size is int) is @byte` | keep with module effect policy: PcgRandom represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `SecureRandom` | Domain atom / role / runtime-owned state | `integer(start is int, stop is int) is int`<br>`float() is float`<br>`bytes(size is int) is @byte` | keep with module effect policy: SecureRandom represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `SecureInteger` | `beginning:int`; `ending:int` | `value() is int` | keep with module effect policy: SecureInteger represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `SecureBytes` | `amount:int` | `bytes() is @byte` | keep with module effect policy: SecureBytes represents a specific capability, locator or coherent observation; explicit coordinates/representations, no generic data protocol |
| `InvalidRandomRange` | `beginning:int`; `ending:int` | `start() is int`<br>`stop() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidRandomSize` | `amount:int`; `limit:int` | `requested() is int`<br>`maximum() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `UnavailableRandom` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/regex/escape.ok

bounded pattern/match/replacement representations. Pure lazy compilation and matching; engine state remains private and failures memoized.

Dialect boundaries, captures, ranges, limits and composed replacement; no native handle API, host regex dialect or executable patterns.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `EscapedRegex` | `source:string` | `string() is string` | keep: EscapedRegex denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
## std/regex/match.ok

bounded pattern/match/replacement representations. Pure lazy compilation and matching; engine state remains private and failures memoized.

Dialect boundaries, captures, ranges, limits and composed replacement; no native handle API, host regex dialect or executable patterns.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `RegexMatching` | Domain atom / role / runtime-owned state | `text() is string`<br>`range() is RegexRange`<br>`capture(index is int) is RegexCapture`<br>`group(name is string) is RegexCapture`<br>`captures() is @RegexCapture` | keep: nominal observable contract; no implementation inheritance or reflection |
| `RegexMatch` | `source:string`; `state:RegexMatchState` | `text() is string`<br>`range() is RegexRange`<br>`capture(index is int) is RegexCapture`<br>`group(name is string) is RegexCapture`<br>`captures() is @RegexCapture` | keep: RegexMatch denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `RegexCapture` | `source:string`; `state:RegexMatchState`; `position:int`; `label:string` | `index() is int`<br>`name() is string`<br>`matched() is bool`<br>`text() is string`<br>`range() is RegexRange` | keep: RegexCapture denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `MissingRegexCapture` | `position:int` | `index() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `MissingRegexGroup` | `label:string` | `name() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `UnmatchedRegexCapture` | `position:int` | `index() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/regex/pattern.ok

bounded pattern/match/replacement representations. Pure lazy compilation and matching; engine state remains private and failures memoized.

Dialect boundaries, captures, ranges, limits and composed replacement; no native handle API, host regex dialect or executable patterns.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `RegexFlag` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `RegexLimit` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `Regex` | `source:string`; `selected_flags:@RegexFlag`; `state:RegexState` | `expression() is string`<br>`flags() is @RegexFlag`<br>`group_count() is int`<br>`group_names() is %string:int`<br>`state() is RegexState` | redesign: remove native state getter; exact observation collaborator |
| `InvalidRegex` | `source_offset:int`; `message:string` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `RegexLimitExceeded` | `limit_kind:RegexLimit`; `maximum:int` | `kind() is RegexLimit`<br>`limit() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/regex/range.ok

bounded pattern/match/replacement representations. Pure lazy compilation and matching; engine state remains private and failures memoized.

Dialect boundaries, captures, ranges, limits and composed replacement; no native handle API, host regex dialect or executable patterns.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `RegexRange` | `first:int`; `limit:int` | `start() is int`<br>`end() is int`<br>`length() is int` | keep: RegexRange denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidRegexRange` | `first:int`; `limit:int` | `start() is int`<br>`end() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/regex/replace.ok

bounded pattern/match/replacement representations. Pure lazy compilation and matching; engine state remains private and failures memoized.

Dialect boundaries, captures, ranges, limits and composed replacement; no native handle API, host regex dialect or executable patterns.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `ReplacingRegexMatch` | Domain atom / role / runtime-owned state | `replacement(found is RegexMatching) is string` | keep: nominal observable contract; no implementation inheritance or reflection |
| `LiteralRegexReplacement` | `origin:string` | `replacement(found is RegexMatching) is string` | keep: LiteralRegexReplacement denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `ExpandedRegexReplacement` | `source:string`; `state:RegexReplacementState` | `replacement(found is RegexMatching) is string` | keep: ExpandedRegexReplacement denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `ReplacementToken` | `classification:string`; `origin:string` | `kind() is string`<br>`text() is string` | keep: ReplacementToken denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `ReplacementTokens` | `origin:@string` | `cursor() is Cursor<ReplacementToken>` | keep: ReplacementTokens denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `ReplacementTokenCursor` | `origin:@string`; `position:int` | `available() is bool`<br>`item() is ReplacementToken`<br>`successor() is Cursor<ReplacementToken>` | keep at implementation boundary: ReplacementTokenCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `RegexSubstitution` | `source:string`; `pattern:Regex`; `replacing:ReplacingRegexMatch`; `maximum:int`; `state:RegexSubstitutionState` | `string() is string`<br>`amount() is int` | keep: RegexSubstitution denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `RegexSubstitutionProgress` | `source:string`; `rendered:string`; `offset:int`; `quantity:int` | `successor(found is RegexMatching, replacing is ReplacingRegexMatch) is RegexSubstitutionProgress`<br>`result() is RegexSubstitutionResult` | keep: RegexSubstitutionProgress denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `RegexSubstitutionResult` | `rendered:string`; `quantity:int` | `string() is string`<br>`amount() is int` | keep: RegexSubstitutionResult denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidRegexReplacement` | `source_offset:int`; `message:string` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidRegexReplacementLimit` | `maximum:int` | `limit() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/regex/search.ok

bounded pattern/match/replacement representations. Pure lazy compilation and matching; engine state remains private and failures memoized.

Dialect boundaries, captures, ranges, limits and composed replacement; no native handle API, host regex dialect or executable patterns.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `FirstRegexMatch` | `observation:RegexMatchObservation` | `text() is string`<br>`range() is RegexRange`<br>`capture(index is int) is RegexCapture`<br>`group(name is string) is RegexCapture`<br>`captures() is @RegexCapture` | keep: FirstRegexMatch denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `PrefixRegexMatch` | `observation:RegexMatchObservation` | `text() is string`<br>`range() is RegexRange`<br>`capture(index is int) is RegexCapture`<br>`group(name is string) is RegexCapture`<br>`captures() is @RegexCapture` | keep: PrefixRegexMatch denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `EntireRegexMatch` | `observation:RegexMatchObservation` | `text() is string`<br>`range() is RegexRange`<br>`capture(index is int) is RegexCapture`<br>`group(name is string) is RegexCapture`<br>`captures() is @RegexCapture` | keep: EntireRegexMatch denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `RegexMatches` | `source:string`; `pattern:Regex`; `selected:RegexRange` | `cursor() is Cursor<RegexMatching>`<br>`found() is bool` | keep: RegexMatches denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `RegexMatchCursor` | `source:string`; `pattern:Regex`; `position:int`; `limit:int`; `exhausted:bool`; `observation:RegexMatchObservation` | `available() is bool`<br>`item() is RegexMatching`<br>`successor() is Cursor<RegexMatching>` | keep at implementation boundary: RegexMatchCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `RegexMatchObservation` | `source:string`; `pattern:Regex`; `selected:RegexRange`; `query:RegexQueryState` | `found() is bool`<br>`match() is RegexMatching` | relocate/redesign: pattern collaborator, typed RegexQuery |
| `RegexRangeOutsideInput` | `selected:RegexRange`; `source_length:int` | `range() is RegexRange`<br>`length() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `NoRegexMatch` | Domain atom / role / runtime-owned state | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/regex/split.ok

bounded pattern/match/replacement representations. Pure lazy compilation and matching; engine state remains private and failures memoized.

Dialect boundaries, captures, ranges, limits and composed replacement; no native handle API, host regex dialect or executable patterns.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `RegexParts` | `source:string`; `pattern:Regex`; `maximum:int` | `cursor() is Cursor<string>` | keep at implementation boundary: RegexParts represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `RegexPartsCursor` | `source:string`; `separators:Cursor<RegexMatching>`; `offset:int`; `remaining:int`; `final:bool` | `available() is bool`<br>`item() is string`<br>`successor() is Cursor<string>` | keep at implementation boundary: RegexPartsCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `InvalidRegexSplitLimit` | `maximum:int` | `limit() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/size.ok

small nominal size contracts. No I/O or owned resource state. Preserve interpreter-provided nominal behavior.

Existing conformance, error and ordering tests; no generic reflection/base-object API.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Sizing` | Domain atom / role / runtime-owned state | `size() is int` | keep: nominal observable contract; no implementation inheritance or reflection |
## std/statistics.ok

validated statistics representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `QuantileMethod` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `FloatSample` | `origin:@float` | `values() is @float`<br>`size() is int` | keep: FloatSample denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Mean` | `origin:FloatSample` | `value() is float` | keep: Mean denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `GeometricMean` | `origin:FloatSample` | `value() is float` | keep: GeometricMean denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `HarmonicMean` | `origin:FloatSample` | `value() is float` | keep: HarmonicMean denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Median` | `origin:FloatSample` | `value() is float` | keep: Median denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `MedianLow` | `origin:FloatSample` | `value() is float` | keep: MedianLow denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `MedianHigh` | `origin:FloatSample` | `value() is float` | keep: MedianHigh denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Quantiles` | `origin:FloatSample`; `amount:int`; `method:QuantileMethod` | `values() is @float` | keep: Quantiles denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Mode` | `origin:Modes<T>` | `value() is T` | keep: Mode denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Modes` | `origin:@T` | `cursor() is Cursor<T>`<br>`size() is int`<br>`item(index is int) is T`<br>`first() is T`<br>`last() is T` | keep: Modes denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `ModesCursor` | `origin:@T`; `position:int` | `available() is bool`<br>`item() is T`<br>`successor() is Cursor<T>` | keep at implementation boundary: ModesCursor represents the algorithm/adapter coordinates listed here; not a general service; module-private enforcement is unavailable |
| `SampleVariance` | `origin:FloatSample` | `value() is float` | keep: SampleVariance denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `PopulationVariance` | `origin:FloatSample` | `value() is float` | keep: PopulationVariance denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `SampleDeviation` | `origin:FloatSample` | `value() is float` | keep: SampleDeviation denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `PopulationDeviation` | `origin:FloatSample` | `value() is float` | keep: PopulationDeviation denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Covariance` | `left:FloatSample`; `right:FloatSample` | `value() is float` | keep: Covariance denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Correlation` | `left:FloatSample`; `right:FloatSample` | `value() is float` | keep: Correlation denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `LinearRegression` | `gradient:float`; `offset:float` | `slope() is float`<br>`intercept() is float` | replace: FittedLine with prediction and residual behavior |
| `StatisticsError` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/strings/format.ok

validated strings format representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `PercentFormatted` | `pattern:string`; `values:@any` | `string() is string` | remove/replace: Formatted with explicit text slots, no any/printf |
| `InvalidFormat` | `offset:int`; `message:string` | `text() is string`<br>`offset() is int` | repair: one projection; exact slot-count diagnostic |
| `UnsupportedFormatValue` | `message:string` | `text() is string` | remove: no automatic value inspection in new formatter |
## std/strings/parse.ok

validated strings parse representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `ParsedInt` | `source:string`; `radix:int` | `value() is int` | keep: ParsedInt denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `ParsedFloat` | `source:string` | `value() is float` | keep: ParsedFloat denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `ParsedRat` | `source:string` | `value() is rat` | keep: ParsedRat denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `ParsedBool` | `source:string` | `value() is bool` | keep: ParsedBool denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `ParsedByte` | `source:string`; `radix:int` | `value() is byte` | keep: ParsedByte denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidInteger` | `source:string` | `source() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidFloat` | `source:string` | `source() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidRational` | `source:string` | `source() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidBoolean` | `source:string` | `source() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidByte` | `source:string` | `source() is string`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/text/layout.ok

validated text layout representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Indented` | `source:string`; `indentation:string` | `string() is string` | keep: Indented denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Dedented` | `source:string` | `string() is string` | keep: Dedented denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Wrapped` | `source:string`; `line_width:int` | `string() is string` | keep: Wrapped denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Shortened` | `source:string`; `line_width:int`; `ending:string` | `string() is string` | keep: Shortened denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
## std/time/clock.ok

validated time clock representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Instant` | `origin:Duration` | `unix_duration() is Duration`<br>`plus(duration is Duration) is Instant`<br>`minus(other is Instant) is Duration`<br>`order(other is Instant) is Order` | keep: Instant denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `MonotonicInstant` | `origin:Duration` | `reading() is Duration`<br>`plus(duration is Duration) is MonotonicInstant`<br>`minus(other is MonotonicInstant) is Duration`<br>`order(other is MonotonicInstant) is Order` | keep: MonotonicInstant denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Timing` | Domain atom / role / runtime-owned state | `instant() is Instant`<br>`monotonic_instant() is MonotonicInstant`<br>`async wait(duration is Duration) is Timing` | keep: nominal observable contract; no implementation inheritance or reflection |
| `SystemClock` | Domain atom / role / runtime-owned state | `instant() is Instant`<br>`monotonic_instant() is MonotonicInstant`<br>`async wait(duration is Duration) is Timing` | keep: SystemClock denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidWait` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/time/date.ok

validated time date representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Weekday` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `Date` | `year_value:int`; `month_value:int`; `day_value:int` | `year() is int`<br>`month() is int`<br>`day() is int`<br>`weekday() is Weekday`<br>`order(other is Date) is Order` | keep: Date denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `ShiftedDate` | `origin:Date`; `days:int` | `value() is Date` | keep: ShiftedDate denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `DaysBetween` | `left:Date`; `right:Date` | `value() is int` | keep: DaysBetween denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `DateOrdinal` | `origin:Date` | `value() is int` | keep: DateOrdinal denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `OrdinalDate` | `origin:int` | `value() is Date` | keep: OrdinalDate denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidDate` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/time/day.ok

validated time day representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `TimeOfDay` | `hour_value:int`; `minute_value:int`; `second_value:int`; `nanosecond_value:int` | `hour() is int`<br>`minute() is int`<br>`second() is int`<br>`nanosecond() is int`<br>`order(other is TimeOfDay) is Order` | keep: TimeOfDay denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidTimeOfDay` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/time/duration.ok

validated time duration representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `DurationUnit` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `Duration` | `origin:rat` | `seconds() is rat`<br>`nanoseconds() is BigInt`<br>`plus(other is Duration) is Duration`<br>`minus(other is Duration) is Duration`<br>`order(other is Duration) is Order` | keep: Duration denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `ScaledDuration` | `origin:Duration`; `factor:rat` | `value() is Duration` | keep: ScaledDuration denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `DurationString` | `origin:Duration` | `string() is string` | keep: DurationString denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidDuration` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/time/format.ok

validated time format representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `FormattedDate` | `origin:Date`; `pattern:string` | `string() is string` | keep: FormattedDate denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `DateFromFormat` | `origin:string`; `pattern:string` | `value() is Date` | keep: DateFromFormat denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `FormattedLocalDateTime` | `origin:LocalDateTime`; `pattern:string` | `string() is string` | keep: FormattedLocalDateTime denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `LocalDateTimeFromFormat` | `origin:string`; `pattern:string` | `value() is LocalDateTime` | keep: LocalDateTimeFromFormat denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidTimePattern` | `pattern:string`; `position:int` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidFormattedDate` | `origin:string`; `position:int` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidFormattedLocalDateTime` | `origin:string`; `position:int` | `offset() is int`<br>`text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/time/iso.ok

validated time iso representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `IsoDate` | `origin:Date` | `string() is string` | keep: IsoDate denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `DateFromIso` | `origin:string` | `value() is Date` | keep: DateFromIso denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `IsoLocalDateTime` | `origin:LocalDateTime` | `string() is string` | keep: IsoLocalDateTime denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `LocalDateTimeFromIso` | `origin:string` | `value() is LocalDateTime` | keep: LocalDateTimeFromIso denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `IsoFraction` | `origin:int` | `string() is string` | keep: IsoFraction denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidIsoDate` | `origin:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidIsoLocalDateTime` | `origin:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/time/local_datetime.ok

validated time local_datetime representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `LocalDateTime` | `date_value:Date`; `time_value:TimeOfDay` | `date() is Date`<br>`time() is TimeOfDay`<br>`plus(duration is Duration) is LocalDateTime`<br>`minus(other is LocalDateTime) is Duration`<br>`order(other is LocalDateTime) is Order` | keep: LocalDateTime denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
## std/time/rfc3339.ok

validated time rfc3339 representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Rfc3339` | `origin:Instant`; `offset:Duration` | `string() is string` | keep: Rfc3339 denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InstantFromRfc3339` | `origin:string` | `value() is Instant` | keep: InstantFromRfc3339 denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidRfc3339` | `origin:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidRfc3339Offset` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/time/zone.ok

validated time zone representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Fold` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `TimeZone` | Domain atom / role / runtime-owned state | `name() is string`<br>`offset(at is Instant) is Duration`<br>`daylight_saving(at is Instant) is bool`<br>`abbreviation(at is Instant) is string`<br>`instant(origin is LocalDateTime, fold is Fold) is Instant` | keep: nominal observable contract; no implementation inheritance or reflection |
| `Zone` | `origin:string` | `name() is string`<br>`offset(at is Instant) is Duration`<br>`daylight_saving(at is Instant) is bool`<br>`abbreviation(at is Instant) is string`<br>`instant(local is LocalDateTime, fold is Fold) is Instant` | keep: Zone denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `FixedZone` | `zone_name:string`; `zone_offset:Duration` | `name() is string`<br>`offset(at is Instant) is Duration`<br>`daylight_saving(at is Instant) is bool`<br>`abbreviation(at is Instant) is string`<br>`instant(local is LocalDateTime, fold is Fold) is Instant` | keep: FixedZone denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `ZonedDateTime` | `source_instant:Instant`; `source_zone:TimeZone` | `instant() is Instant`<br>`zone() is TimeZone`<br>`local_datetime() is LocalDateTime`<br>`offset() is Duration` | keep: ZonedDateTime denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `ZoneObservation` | `origin:ZonedDateTime` | `daylight_saving() is bool`<br>`abbreviation() is string` | keep: ZoneObservation denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InstantFromLocal` | `origin:LocalDateTime`; `zone:TimeZone`; `fold:Fold` | `value() is Instant` | keep: InstantFromLocal denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `TimeZoneDatabaseVersion` | Domain atom / role / runtime-owned state | `string() is string` | keep: TimeZoneDatabaseVersion denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `UnknownTimeZone` | `name:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `InvalidTimeZone` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
| `MissingLocalTime` | `zone_name:string`; `local:LocalDateTime` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/unicode/character.ok

validated unicode character representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `UnicodeCategory` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `UnicodeCharacter` | `origin:char` | `name() is string`<br>`category() is UnicodeCategory`<br>`combining_class() is int`<br>`numeric_value() is float` | keep: UnicodeCharacter denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `NonNumericCharacter` | `origin:char` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |
## std/unicode/normalization.ok

validated unicode normalization representations. Value observations derive from private operands; clock queries/waits are explicitly effectful capabilities. Pure materialization is documented, not hidden mutation.

Value invariants, limits, malformed input, repeated observations and canonical representation; no utility namespace, automatic casts or unrelated I/O.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `NormalizationForm` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `Normalized` | `origin:string`; `form:NormalizationForm` | `string() is string` | keep: Normalized denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
## std/uuid/generation.ok

validated identifiers and explicit generation. Value encodings/comparison are pure; random UUID generation requires its entropy capability.

Canonical forms, version/variant and injected randomness; no random constructor disguised as a stable value query.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `Uuid3` | `namespace:Uuid`; `name:string` | `value() is Uuid` | keep: Uuid3 denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Uuid5` | `namespace:Uuid`; `name:string` | `value() is Uuid` | keep: Uuid5 denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Uuid8` | `high:int`; `middle:int`; `low:int` | `value() is Uuid` | keep: Uuid8 denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `Uuid4s` | `random:SecureRandom` | `sample() is Uuid` | keep: Uuid4s denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
## std/uuid/value.ok

validated identifiers and explicit generation. Value encodings/comparison are pure; random UUID generation requires its entropy capability.

Canonical forms, version/variant and injected randomness; no random constructor disguised as a stable value query.

| Declaration | Private identity/state | Public actions reviewed | Decision |
| --- | --- | --- | --- |
| `UuidVariant` | Domain atom / role / runtime-owned state | No public action declarations | keep: closed domain vocabulary; no methods or mutable namespace |
| `Uuid` | `origin:@byte` | `version() is int`<br>`variant() is UuidVariant`<br>`order(other is Uuid) is Order`<br>`string() is string`<br>`bytes() is @byte` | keep: Uuid denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `UuidFromBytes` | `origin:@byte` | `value() is Uuid` | keep: UuidFromBytes denotes its named validated/computed representation; result queries use its private operands; projections are pure, not foreign-data service calls |
| `InvalidUuid` | `message:string` | `text() is string` | keep: typed failure and explicit diagnostic coordinates, not success-data transport |

## Native substrate inventory

These 36 modules are the explicitly low-level boundary. Portable wire data and opaque states are allowed here; applications must not receive state getters through high-level objects.

| Module | Owned declarations |
| --- | --- |
| `std/runtime/account.ok` | `account_current_user`, `account_user_named`, `account_group_named`, `account_group_users` |
| `std/runtime/base32.ok` | `base32_encode`, `base32_invalid_offset`, `base32_decode` |
| `std/runtime/base64.ok` | `base64_encode`, `base64_invalid_offset`, `base64_decode` |
| `std/runtime/base85.ok` | `base85_encode`, `base85_invalid_offset`, `base85_decode` |
| `std/runtime/bigint.ok` | `bigint_valid`, `bigint_canonical`, `bigint_sign`, `bigint_bits`, `bigint_order` |
| `std/runtime/binary.ok` | `binary_integer_fits`, `binary_integer_bytes`, `binary_integer_from_bytes_fits`, `binary_integer_from_bytes`, `binary_float_fits`, `binary_float_bytes`, `binary_float_finite`, `binary_float_from_bytes` |
| `std/runtime/buffer.ok` | `BufferState`, `buffer_state_create`, `buffer_state_read`, `buffer_state_append`, `buffer_state_bytes`, `buffer_state_size`, `buffer_state_closed`, `buffer_state_close` |
| `std/runtime/decimal.ok` | `decimal_valid`, `decimal_canonical`, `decimal_coefficient`, `decimal_exponent`, `decimal_order` |
| `std/runtime/environment.ok` | `environment_snapshot`, `environment_normalize`, `environment_add`, `environment_diff`, `environment_name`, `environment_home`, `environment_path_separator`, `environment_get`, `environment_set`, `environment_remove`, `environment_all` |
| `std/runtime/fs.ok` | `path_join`, `path_parent`, `path_child`, `path_absolute`, `FileHandle`, `DirectoryHandle`, `FileState`, `FileLockState`, `TemporaryDirectoryState`, `FileWatchState`, `path_encode`, `path_decode`, `open`, `read`, `write`, `seek`, `flush`, `truncate`, `sync`, `lock_shared`, `lock_exclusive`, `unlock`, `close`, `file_state_create`, `file_state_read_line`, `file_state_read`, `file_state_write`, `file_state_write_text`, `file_state_flush`, `file_state_close`, `file_state_position`, `file_state_seek`, `file_state_truncate`, `file_state_lock`, `file_lock_closed`, `file_lock_close`, `file_content`, `directory_entries`, `directory_remove_tree`, `matching_paths`, `file_watch_create`, `file_watch_event`, `file_watch_closed`, `file_watch_close`, `temporary_file_create`, `file_state_closed`, `temporary_file_path`, `temporary_file_preserve`, `temporary_directory_create`, `temporary_directory_path`, `temporary_directory_closed`, `temporary_directory_close`, `stat`, `lstat`, `mkdir`, `rmdir`, `unlink`, `rename`, `link`, `symlink`, `readlink`, `cwd`, `chdir`, `chmod`, `chown`, `opendir`, `readdir`, `rewinddir`, `closedir` |
| `std/runtime/hex.ok` | `hex_encode`, `hex_valid_prefix`, `hex_decode` |
| `std/runtime/html.ok` | `HtmlEventsState`, `html_events`, `html_event_count`, `html_event_kind`, `html_event_name`, `html_event_value`, `html_event_target`, `html_event_data`, `html_event_self_closing`, `html_attribute_count`, `html_attribute_name`, `html_attribute_valued`, `html_attribute_value` |
| `std/runtime/input_state.ok` | `Utf8InputState`, `input_state_create`, `input_state_append`, `input_state_size`, `input_state_line_offset`, `input_state_take`, `input_state_eof`, `input_state_finish`, `input_state_closed`, `input_state_close` |
| `std/runtime/io_decorators.ok` | `LimitedReadingState`, `ChainedReadingState`, `limited_reading_state_create`, `limited_reading_state_remaining`, `limited_reading_state_consume`, `chained_reading_state_create`, `chained_reading_state_position`, `chained_reading_state_advance` |
| `std/runtime/json.ok` | `JsonParseState`, `JsonNodeState`, `json_parse_state`, `json_parse_valid`, `json_parse_offset`, `json_parse_line`, `json_parse_column`, `json_parse_message`, `json_parse_duplicate`, `json_parse_root`, `json_node_kind`, `json_node_boolean`, `json_node_number`, `json_node_text`, `json_node_size`, `json_node_item`, `json_member_name`, `json_member_value` |
| `std/runtime/layout.ok` | `indent`, `dedent`, `wrap`, `shorten` |
| `std/runtime/math_aggregate.ok` | `aggregate_square_root` |
| `std/runtime/network.ok` | `SocketState`, `socket`, `close` |
| `std/runtime/ordering.ok` | `BulkOrdering` |
| `std/runtime/pem.ok` | `pem_kind_invalid_offset`, `pem_encode`, `pem_invalid_offset`, `pem_block_count`, `pem_block_sources`, `pem_block_source`, `pem_block_kind`, `pem_block_base64` |
| `std/runtime/x509.ok` | private deterministic certificate facts, times/SPKI/grouped names/SAN projections; no live host pointers |
| `std/runtime/process.ok` | `ProcessState`, `CapturedProcess`, `PipelineState`, `CapturedPipeline`, `process_state_create`, `process_state_start`, `process_state_started`, `process_state_id`, `process_state_running`, `process_state_exit`, `process_state_signal`, `process_state_close`, `process_state_input_write`, `process_state_input_flush`, `process_state_input_closed`, `process_state_input_close`, `process_state_output_read`, `process_state_errors_read`, `process_state_capture`, `captured_process_status`, `captured_process_overflow`, `captured_process_output`, `captured_process_errors`, `pipeline_state_create`, `pipeline_state_add`, `pipeline_state_capture`, `captured_pipeline_statuses`, `captured_pipeline_overflow_stream`, `captured_pipeline_overflow_index`, `captured_pipeline_output`, `captured_pipeline_errors` |
| `std/runtime/quoted_printable.ok` | `quoted_printable_encode`, `quoted_printable_invalid_offset`, `quoted_printable_decode` |
| `std/runtime/random.ok` | `PcgState`, `pcg_state`, `pcg_integer`, `pcg_float`, `pcg_bytes`, `random_sample_accepted`, `random_integer_from_sample`, `random_float_from_sample` |
| `std/runtime/random_distribution.ok` | `distribution_uniform`, `distribution_log`, `distribution_exp`, `distribution_sqrt`, `distribution_cos`, `distribution_acos`, `distribution_power`, `distribution_angle` |
| `std/runtime/regex.ok` | `RegexState`, `RegexQueryState`, `RegexMatchState`, `RegexReplacementState`, `RegexSubstitutionState`, `regex_state_create`, `regex_state_prepare`, `regex_state_group_count`, `regex_state_group_names`, `regex_query_create`, `regex_query_found`, `regex_query_match`, `regex_match_capture_count`, `regex_match_capture_name`, `regex_match_capture_index`, `regex_match_capture_span`, `regex_replacement_state`, `regex_replacement_prepare`, `regex_replacement_tokens`, `regex_escape`, `regex_substitution_state`, `regex_substitution_ready`, `regex_substitution_store`, `regex_substitution_string`, `regex_substitution_amount` |
| `std/runtime/single_byte.ok` | `ascii_representable_prefix`, `ascii_encode`, `ascii_valid_prefix`, `ascii_decode`, `latin1_representable_prefix`, `latin1_encode`, `latin1_decode` |
| `std/runtime/statistics.ok` | `statistics_mean`, `statistics_geometric_mean`, `statistics_harmonic_mean`, `statistics_median`, `statistics_quantiles`, `statistics_variance`, `statistics_paired_valid`, `statistics_paired` |
| `std/runtime/stdio.ok` | `stdout_write`, `stderr_write`, `stdin_read`, `stdin_read_line` |
| `std/runtime/system.ok` | `system_platform`, `system_architecture`, `system_byte_order`, `system_host_name`, `system_cpu_count`, `secure_random`, `storage_usage` |
| `std/runtime/time.ok` | `wall_clock`, `monotonic_clock`, `sleep_nanoseconds` |
| `std/runtime/time_format.ok` | `time_pattern_invalid_offset`, `time_format_date`, `time_format_local`, `time_parse_invalid_offset`, `time_parse_parts` |
| `std/runtime/time_zone.ok` | `timezone_version`, `timezone_known`, `timezone_offset`, `timezone_daylight`, `timezone_abbreviation`, `timezone_local_candidates`, `timezone_local_instant` |
| `std/runtime/unicode_character.ok` | `character_name`, `category_code`, `character_combining_class`, `character_is_numeric`, `character_numeric_value` |
| `std/runtime/unicode_normalization.ok` | `normalized_text` |
| `std/runtime/utf.ok` | `utf8_encode`, `utf8_decode`, `utf8_valid_prefix`, `utf16_encode`, `utf16_decode`, `utf16_valid_prefix`, `utf32_encode`, `utf32_decode`, `utf32_valid_prefix` |
| `std/runtime/uuid.ok` | `uuid_valid`, `uuid_bytes`, `uuid_string`, `uuid_version`, `uuid_variant`, `uuid_versioned`, `uuid_named`, `uuid_custom` |

## High-risk retained shapes: rationale and limits

The following are deliberate Okkama compromises, not a claim that strict EO
endorses getter-only records. Their narrow domain boundary is the reason for
retention; do not generalize them into arbitrary `Data` classes.

- `FileStat`, `FileTimes`, `Permissions` and `PermissionMode`: a coherent filesystem
  observation and permission decisions. Splitting the snapshot into separately
  acquired field observations would introduce races. `FileIdentity` itself now
  compares identity without exporting native coordinates. Native map adapters
  remain substrate-facing and must not become application interchange formats.
- `User`, `Group`, platform/environment observations: explicit OS snapshots or
  locators, with memberships/lookups/typed absence. Names and identities are domain
  coordinates. Unrelated account mutation must be a separate capability.
- `Pair`, `Indexed`, `Frequency`, matched ranges and captures: typed relations
  intrinsic to traversal and grammar matching. Returning their endpoints/content
  is the useful observation; arbitrary private-field reflection is not allowed.
- Process exit/captured output: immutable completion observation belongs to a
  particular invocation. The Process capability executes/waits; the observation
  must never launch or mutate a process as a query effect.
- JSON member/name and grammar kind: a name/value association and wire grammar,
  not language object introspection. Scalar JSON getters were removed in favor
  of the explicit printing protocol. Empty member names are real names, not absence.
- Native-backed async capabilities: state evolves outside immutable Okkama
  coordinates. Receivers returned by existing async commands are a documented
  language limitation while async void is unavailable, not an approved pattern
  for new convenience methods. Completion receipts are a future design option.
- Algorithm-position collaborators (cursor searches, pivots, revised collection
  parts) represent fixed operands/positions. They are not public utility namespaces;
  enforceable module privacy is unavailable and remains explicit technical debt.

Every retained signature in a family is constrained by these policies and the
module-specific identity column. This architectural ledger does not substitute
for implementation correctness, asymptotic analysis or future property tests.

## Completion follow-up: BigInt arithmetic

BigSum/Product/Quotient/Remainder/Power/Gcd/Lcm represent fixed arithmetic operands,
not a stateless calculator service. value() is a meaningful computed projection.
No operands or user fields mutate; only fresh backend arithmetic temporaries do.
BigPower additionally carries an explicit positive result-bit budget. Configuration
validation belongs to arithmetic domain failure to value(). The four typed
errors expose only relevant diagnostics. Public API and [exact contracts](bigint-guide.md)
are updated; the initial audit table above remains historical evidence.

## Completion follow-up: PrettyJson

PrettyJson represents a document with explicit layout and limits. Its fresh local
buffer does not escape; dependencies stay immutable. PrettyJsonOutput dispatches
through JsonWriting and delegates scalar spelling to JsonTextOutput. JsonBoundedText
owns the budget check at append boundaries. Infrastructure is importable because
module-private declarations are unavailable, not intended as the application API.
No reflection, new native entry points or dependencies were added.

## Completion follow-up: initial CSV printing

CsvString, CsvRecord and CsvField retain the document/record/field they represent;
they are not static formatting services. CsvString snapshots a finite nominal
Iterating input once. The narrow fixed syntax avoids prematurely approving the
v3 dialect getter bundle. Parsing is completed in the follow-up below;
configurable syntax and budgets remain open.

## Completion follow-up: fixed-format CSV parsing

CsvRows represents source text under the fixed comma/quote/CRLF grammar. Its
constructor only retains that text; cursor() validates all of it, returning an
independent immutable traversal. It neither acquires a resource nor performs
hidden I/O. InvalidCsv.offset() is a meaningful source coordinate; line/column
getters and arbitrary message storage are omitted. CsvPrefix represents a
consumed prefix and only produces a successor or completed records; CsvMode is
its grammar state vocabulary. Both collaborators are necessarily importable,
without exposing private state through getters. Existing persistent lists supply
storage and typed exhaustion. No native entry points or dependencies were added.
Read limits now bound input scalars, decoded fields, rows and columns. CsvPrefix
retains source coordinates and decodes each field once rather than repeatedly
copying a growing string. CsvLimit identifies the exhausted budget; no private
parser state is exposed. Print snapshots now stop at the first row/column/field
budget breach; output checks account for encoded field sizes before creating a
record. CsvField.size() denotes scalar length of its exact wire representation.
CsvSnapshot/CsvPrintPrefix retain immutable prefixes; CsvPrintRecord represents a
record under an output allowance. No mutable public state or new native boundary.
Configurable dialects are implemented in the follow-up below; resource reading remains separate.

## Completion follow-up: Joined string snapshots

Joined denotes the exact text represented by a separator and a finite sequence
of strings. It consumes the iteration once during explicit snapshot construction;
string() has no source effect. JoinedParts represents an immutable bounded prefix,
not a mutable builder or global formatting namespace. No implicit stringification,
reflection or native boundary is added. Invalid budgets fail before traversal;
source errors keep their nominal type, borrowed capabilities are not closed.
Item and output-scalar limits bound both infinite empty sources and oversized text.

## Completion follow-up: arbitrary-precision rational values

BigRational represents a mathematical ratio, with canonical numerator/denominator
coordinates rather than a bag of mutable numeric fields. Gcd and sign normalization
at construction are the explicit canonical-value compromise used by BigInt. Exact
comparison and canonical representation give these coordinates behavioral meaning.
Expression objects retain operands and compute a new value on projection. Zero
constructor denominator and zero quotient divisor are distinct typed failures,
with the latter delayed until observation. Existing BigInt arithmetic provides the
native boundary; no host numeric coercion or new provider is introduced. Large
input/intermediate allocations are explicitly outside a bounded-resource claim.


## Completion follow-up: behavioral CSV dialect

CsvDialect represents a concrete grammar with immutable delimiter, quote and
ending coordinates. Consumers ask it to classify or encode text, rather than
reading a bag of four options. Its bounded record behavior checks the complete
encoded size before escaping. Parsing retains original source offsets; it does
not normalize newlines or rewrite separators before validating. LF and CRLF are
strict grammar choices; quoted CR/LF remain data. Only doubling is supported.
The v3 Custom/Excel/Unix/Tab wrapper candidates are resolved by constructing the
same grammar object; separate preset classes provide no additional behavior.
Existing constructors and default standalone field/record representations remain
compatible. No interpreter/native changes or resource ownership are introduced.
[Contract and evidence](csv-guide.md#configurable-dialect-contract).


## Completion follow-up: resource CSV document input

CsvInput represents a particular borrowed Reading and its CSV grammar/budgets.
read() is an explicit consuming operation, not a projected representation or
cursor action. The first resource slice reads a bounded document and returns
validated immutable rows atomically. Strict UTF-8 is applied once after bounded
byte collection, permitting arbitrary chunk boundaries without using line()
(which would erase CR/LF distinctions). An extra byte distinguishes exact EOF
from overflow; oversized upstream responses fail rather than bypassing bounds.
No mutable interpreter substrate is necessary for this document-at-a-time API.
Errors do not roll back the source and repeat reads are not cached; these facts
are part of the public contract. Incremental per-row state and owning adapters
remain explicitly deferred. See [contract](csv-input-guide.md).


## Completion follow-up: non-temporal TOML values

The v3 kind/value-only wrappers are replaced by TomlValue.accept(TomlVisitor):
values perform typed dispatch while consumers control traversal. The model is
independent of JSON. TomlMember is the explicit named binding; ordered bounded
snapshots retain each source element once. Duplicate names fail without choosing
first/last-wins semantics. Literal dots, empty names, case and Unicode spelling
are preserved; parsing and dotted-key resolution do not belong in this layer.
Arrays/tables expose their snapshots without claiming collection roles themselves.
This is an explicit immutable-document representation compromise, similar to JSON,
with behavioral observation and typed lookup rather than reflection.

Temporal candidates remain open: the current calendar range must not be presented
as full TOML date/time support. No parser or complete conformance claim is made.
See [contract, limits and remaining gates](toml-values-guide.md).


## Completion follow-up: TOML temporal coordinates

Four temporal values implement TomlValue and dispatch to typed visitor methods.
Their coordinate observations are justified by calendar/clock identity; canonical
string behavior makes them representations rather than opaque records. They do
not wrap narrower existing calendar types or claim instant conversion. TomlOffset
separates unknown-zero provenance from a numeric displacement. Validation and
formatting use no host locale, zone, resources or mutable state. Leap-second
notation is representable; actual-event validation is outside this value model.

This visitor graph also exposed two analyzer defects: import expansion could
register an implementing object before its role, and imported role signature
names could be resolved in the implementing module's scope. Dedicated RED tests
cover local/selected/namespace imports and preserve nominal rejection of foreign
same-named types. Registration and signature resolution are corrected; full
regression is required for this interpreter change.

## `std/async/atomic.ok`

**Keep:** `Atomic<T>` denotes one shared cell with stable identity and an opaque
VM state; `AtomicClosed` is its typed lifecycle failure. The initialized
constructor establishes a complete cell without I/O. `value` observes, `replace`
publishes, `compare_and_replace` conditionally publishes and reports success;
`closed`/`close` satisfy Closing. No primitive handle crosses the API. No new
generic role is introduced for a single VM-specialized collaborator.

CAS is a documented synchronization exception to command/query separation.
Comparison uses strict language equality; Atomic itself has identity equality
so different empty-field capability wrappers cannot compare equal. Resource
payloads are borrowed, close drops the reference without closing their owners.
All operations run synchronously in the cooperative VM; no OS resource or
wait queue is allocated, and dropping aliases never closes another user's cell.
No host ABI or serialized value adapter is needed: the VM retains language values.

Validated through interpreter/runtime tests and invoice evolution 09 using
unchanged CAS contract fixtures, Go reference implementation and File adapter.

## `std/csv/stream.ok`

Keep CsvStream as an explicit consuming input capability. Stable coordinates
are its borrowed Reading and opaque runtime/csv_state scanner. row() is the
consuming-I/O exception to query/command separation; close is void and releases
only scanner memory. EndOfCsv and ClosedCsvStream are explicit protocol errors.
No cursor hides I/O, no callbacks mutate state, and no native handle escapes
the high-level API. Defaults and all five budgets are documented and validated.
The default CSV grammar is incremental and retains one record plus bounded raw
read-ahead; prior records are detached. Existing Unicode scalar decoding is
reused, including noncharacters. Source errors preserve identity, whereas CSV
syntax/UTF8/budget failures retain their kind/offset and terminal parser state.
Native ABI is limited to bytes, scalar status/offsets and one string array.
Cross-language trial policy lives outside the generic parser.


## `std/mail/pop3.ok`, `std/mail/pop3_session.ok`

**Keep:** typed number and UIDL identities, bounded immutable listing and CAPA
observations. BigInt sizes avoid float loss. Snapshot observations are a deliberate
protocol representation, not getters for private transport handles. Unknown CAPA
names remain observable; TOP/UIDL are the implemented optional behavior.

**Change from v3 candidates:** mailbox count/size are observed together through
async `state()`, not stale synchronous facts on a live mailbox. DELE, RSET and
QUIT are async void commands on explicit infrastructure capabilities, not fake
receiver-valued results. `PreviewedPopMail` denotes a cached read operation.
`PopFailure` is replaced by specific validation/limit/rejection/lifecycle errors.
Closing aborts; explicit QUIT requests publication and failed QUIT can be uncertain.

**Infrastructure:** stable fields identify one exclusively owned authenticated
transport. A Lock serializes complete commands and LIST/UIDL pairs; Atomic holds
immutable unread bytes. Neither weakens domain immutability. Low-level typed
commands are importable because Okkama has no module privacy; raw commands and
credential strings are not accepted. A mailbox retains session ownership;
immutable listing results do not. Cancellation drains nested async cleanup.

**Blocked:** PopServer/PopConnecting and authentication still require verified
TLS, HostName/IDNA, TCP and SecretBytes. No insecure fallback or placeholder API.
See [contract](pop3-session.md) for effects, bounds, repeat/cancellation semantics.

## HTTP runtime gate

Pure method/status/header values need no native API. Protocol consumers must
reuse existing AsyncReading/AsyncWriting and scheduler cleanup. TCP connection,
TLS/ALPN, complete URL and compression foundations remain explicit blockers.
No speculative HTTP handle/provider or second event loop was introduced.
See [gate](http-runtime-gate.md); this is a contract decision, not verified wire support.

## HTTP method values

Method represents an exact case-sensitive protocol token; string is its explicit
wire spelling, not access to private algorithm state. MethodSemantics represents
standard behavioral properties of that immutable token. Unknown extensions stay
open and conservative. Constructor validation has no I/O/lifetime effects.
The v3 uppercase-normalization candidate is rejected because it changes method
identity. No closed enum, retry facade or network dependency is introduced.
See [contract](http-method.md).

## HTTP status values

Status denotes a validated numeric protocol identity; code/class are meaningful
protocol observations. Phrase is optional presentation from a documented registry
snapshot and never determines behavior. No arbitrary reason-string coordinate or
named alias objects are restored from legacy APIs. InvalidStatus retains the
rejected code; all observations are pure and bounded. See [contract](http-status.md).

## HTTP header values

HeaderName retains wire spelling while explicit canonical ordering expresses HTTP
identity; structural equality is not redesigned. Header is a validated logical
field, Headers a bounded ordered snapshot, and transformations are persistent
representations. Primitive name/value observations belong to this protocol
boundary. No mail unfolding, case-sensitive Set lookup, comma-joining facade or
mutable collection is introduced. Removal names use a bounded sequence because
the candidate Set<HeaderName> implies the wrong equality for preserved spelling.
Http2Headers validates a logical list, not an encoder or complete HTTP message.
Limits and ownership-free effects are in the [contract](http-header.md).

## HTTP body capabilities

BodyOpening denotes a source recipe; BodyReading owns one consumption lifetime.
ByteBody's immutable coordinates make replay exact; OneShotBody explicitly owns
an adopted reader until its atomic opening claim transfers use. StreamBodyReading
borrows no hidden transport: its already-delimited input and exclusive Closing
owner are explicit. Atomic/Event/Lock preserve domain object immutability; one
FutureGroup per read drains cancellation and retains cleanup error identity.
No accumulating shared group, fake unknown length, whole-stream buffer or
synchronous file read disguised as cancellable work is added. FileBody needs a
proper filesystem capability; wire draining/reset belongs to protocol consumers.
See [contract](http-body.md).

## HTTP content coding names

ContentCoding denotes an open, case-insensitive protocol identity. Canonical
storage makes ordinary structural equality appropriate; string is its protocol
representation. It neither reports available algorithms nor combines negotiation
with transformation. The body-decorator candidates require deferred real codecs;
no fake implementation or unsolicited codec advertisement is introduced.
See [contract](http-content-coding.md).

## HTTP dates

HttpDate denotes a normalized whole-second UTC value, so representation and
comparison share one identity. ParsedHttpDate represents a bounded wire text
interpretation; an explicit reference removes hidden clock dependence from the
obsolete two-digit-year rule. No locale parser, formatting service facade or
new time arithmetic is introduced. Stage 10 owns calendar/time invariants.
The added value()/Comparing observations make precision explicit rather than
comparing strings. See [contract](http-date.md).

## HTTP conditional validators

EntityTag represents an opaque version validator with explicit strength. Its
opaque value and quoted string are HTTP identities/representations; comparison
is behavior, not a schema export. EntityTags represents a wildcard or ordered
list condition, including the distinct empty list. Snapshotting consumes only
Sequencing methods, checks budgets before item reads and retains immutable tags.
ParsedEntityTags is a bounded interpretation of wire text, not a parser service.
Empty recipient list members consume budget even though canonical output omits
them. Backslash is opaque, unlike MIME quoted-string. No ambient resource lookup,
network, role downcast, or dependency on blocked HTTP message objects is introduced.
The small persistent collection/position helpers are importable infrastructure,
not proposed application domain roles. Full preconditions remain protocol work.
See [contract](http-conditional.md).

## HTTP byte-range plans

ByteRange denotes a nonempty inclusive interval, so start/end are intrinsic
coordinates and size is derived rather than stored redundantly. ByteRanges is
an immutable validated protocol plan; canonical coordinates and unit spelling
give stable identity independently of source formatting. Normalization requires
known length and uses exact BigInt arithmetic, existing SortedBy and persistent
list accumulation. Overlap/adjacency coalescing occurs after the raw slot budget,
so thousands of duplicate or empty requests cannot evade limits through merging.
No bytes proportional to representation length are materialized. RFC satisfiability
is separate from concrete interval count for a positive suffix at length zero.
Helpers are infrastructure; full request preconditions/statuses and multipart
body effects remain gated on actual protocol consumers. See
[contract](http-conditional.md).

## HTTP preference quality and weighted tokens

Quality denotes a bounded exact preference, backed by validated canonical
thousandths. Its value/acceptability/comparison/representation are coherent
observations; no float epsilon or rounding silently changes client policy.
ParsedQuality is a fixed-size grammar interpretation, using rational arithmetic
and strict ASCII digits. WeightedValue associates a validated HTTP token with
Quality; default 1 is the protocol's defined weight, not absent data. Its token
spelling is preserved because common syntax does not determine a field's matching
policy. Unqualified AcceptedValues from v3 is not promoted: media parameters,
coding identity/wildcards and language specificity differ. Explicit profiles
follow as separate tested consumers. No new native/interpreter API or ambient
codec availability is introduced. See [contract](http-negotiation.md).

## Accept-Encoding preference field

AcceptedContentCodings represents the client's actual field policy, including
the meaningful difference between absent and empty. It observes/chooses against
explicit supplied codings, never scans or advertises host codecs. Canonical names,
exact Quality and preserved positions form immutable protocol preferences;
duplicate canonical names are explicitly rejected. Wildcard cannot restore a
coding excluded by an exact zero. Implicit identity is only an acceptable value,
not an available provider invented by the selector. Sorted index and client-order
array serve distinct purposes, avoiding full list scans per lookup; runtime graph
traversal overhead remains a separate concern. Helpers are importable infrastructure,
not general reflection or a universal content-negotiation service. No body codec,
HTTP exchange or provider effect is introduced. See [contract](http-negotiation.md).

## HTTP media-value foundation

HttpMediaType represents a concrete protocol identity, HttpMediaParameter a named
association and HttpMediaParameters its immutable canonical set. Their observations
express media coordinates and validated parameter behavior. Charset normalization,
sorting and duplicate rejection give a coherent identity rather than frozen arbitrary
foreign fields. The HTTP grammar differs from MIME: strict tchar and adjacency,
Latin-1 obs-text, quoted-pairs and literal extension names. Existing MIME constructors,
RFC2231 decoding and APIs are preserved. The bounded scanner stores wire source
coordinates/quoting only in internal grammar collaborators for the future Accept
consumer; public identity omits syntax artifacts. Quote decoding uses the existing
owned memory ByteBuffer with explicit close, no file/network effect. Binary name
lookup is structurally tested; runtime container/ownership cost remains separate.
See [contract](http-media-type.md).


### Accept media preference profile

MediaRange represents a validated weighted media range, and MediaSpecificity its
protocol ordering. AcceptedMediaTypes is an immutable bounded preference field.
Each candidate receives quality from its most-specific applicable rule before
MediaChoice compares candidates. The chosen value must come from the supplied
sequence; no codec/rendering capability is inferred. Empty and absent fields
remain distinct. Quoted separators, q in any parameter position, literal extension
parameters and exact-zero precedence are covered independently of coding rules.
MediaMatch and MediaMemberPosition hold persistent traversal state, not a public
universal DTO or negotiation manager. No protocol response effect is introduced.
See [contract](http-negotiation.md#accept-media-profile) and
evidence.


### Charset preference profile

CharsetName represents a canonical concrete token, not charset availability.
AcceptedCharsets holds explicit preferences; an exact zero overrides wildcard,
and the selected name must come from supplied values. Alias resolution and codec
selection remain separate. The shared WeightedTokenLookup removes duplicate
binary index traversal without changing existing Coding helper import names or
content-coding identity policy. CharsetChoice retains a typed immutable candidate;
there is no fallback DTO or host-encoding manager. Deprecated field status and
no automatic emission are documented in the [contract](http-negotiation.md#accept-charset-profile).


### Basic language matching and HTTP preferences

BasicLanguageRange is a syntactic range entity distinct from a concrete Locale.
LanguagePrefixes represents a tag's descending basic prefixes. WeightedLanguageRange
adds HTTP quality without mixing registry, CLDR or host locale state into matching.
AcceptedLanguages holds a bounded field and selects only supplied Locale values;
longest matching range sets quality before candidate ranking. Range key lookup
reuses the immutable token index, rather than scanning all client entries.
The documented tie policy is explicit and does not claim a single normative RFC
server ranking. Lookup, filtered collections and full tag support remain open.
See [locale contract](locale-match-guide.md) and
[HTTP contract](http-negotiation.md#accept-language-profile).


### Cookie expiry prerequisite

CookieExpiry represents session absence or an absolute deadline, with a typed
MissingCookieExpiry observation. Private zero/one storage is not exposed as an
optional DTO and epoch remains a valid persistent Instant. Clock, Max-Age/date
interpretation and origin policy are separate consumers. Values survive their
construction scope without provider calls. The pure prerequisite does not bypass
the actual Url/public-suffix gates for cookie storage. [Contract](http-cookie.md).
CookieAttributes now snapshots only the flag role into canonical booleans and
validates domain/path and Secure dependencies. Cookie retains validated pair
identity, enforces consumer prefixes, exposes wire output deliberately and redacts
diagnostics. Errors never reflect values; default VM formatting is type-only.
Focused compatibility: 3 files / 48 groups PASS; two inline tests and example PASS.
ParsedCookieDate separately implements bounded octet token parsing, first-field
selection and fixed centuries. It returns the actual Instant and keeps grammar
helpers inside stdlib. InvalidCookieDate translates encoding/calendar failures
without echoing source. Compatibility: 2 files / 46 groups; inline and example PASS.
ParsedSetCookie now consumes actual Url and explicit reference time, with bounded
attribute grammar, immutable Cookie output and pinned domain policy. CookieContext
represents caller-provided schemeful site/navigation/method; it does not guess
browser state. Prefix permissions are checked before derived coordinates.
Focused compatibility: 3 files / 75 groups, inline and example PASS.
MemoryCookieJar is an explicit infrastructure effect owner; CookieStoring limits
the consumer to three HTTP actions. It retains immutable validated records in an
Atomic cell, borrows the clock and publishes once without suspension. Returned
cookies are pure snapshots. Reentry, closed aliases, replacement, partitions,
expiry, prefix/domain policy and deterministic eviction pass 37 focused groups
with Set-Cookie compatibility. Profiled avoidable eviction sorting is removed;
remaining VM/type/ownership cost is documented separately as PERF-13.
Saved/LoadedCookieJar are explicit filesystem command objects; restricted friends
read/publish the concrete jar's private cell without extending CookieStoring.
The file projection is a validated domain protocol, with no universal field
reflection. Sessions/expired records are excluded; schema/policy/coordinate and
identity checks precede loading publication. Private staging plus closed-file
rename preserves prior output on failure; cleanup and empty snapshots are tested.


### IDNA host identity and public suffix boundaries

HostName represents canonical DNS spelling, not resolver or connecting behavior;
HostEndpoint associates that name with a validated integer port. PublicSuffixList
represents a specific immutable ICANN/PRIVATE ruleset. Its suffix/registrable
observations apply the exact/wildcard/exception/default policy over supplied
names, without a host-installed mutable database or network refresh. Private
native IDNA conversions use a reviewed fixed upstream version, strict options,
ACE round-trip validation and bounded/scalar preflight. The generated hash index
retains only pinned rules and needs at most three key observations per label.
No native URL/MIME value, universal DTO or unchecked capability is introduced.
[Contract](host-policy.md); full runtime regression 588 files / 10921 groups PASS.


### Full URL identity and explicit resolution

Url/UrlReference represent validated canonical coordinates and explicit relative
resolution; UrlParts is the composition entity, not a parser AST or reflection
bag. Typed host projections prevent hidden string-origin fallbacks. UrlPort and
UrlUserInfo represent meaningful absent/empty/assigned wire identity. A nominal
UrlHost implementation cannot inject wire delimiters: its typed coordinates must
agree with canonical wire spelling before a fresh validated host is retained.
All 42 RFC resolution cases and an independent 442-path dot oracle pass.
A measured persistent-stack ownership bottleneck justified a private string bulk
operation; URL parsing/resolution remain std and public APIs are preserved.
[Contract](url-value.md), performance report.

### HTTP target role boundaries

RequestTarget exposes typed variant coordinates; invalid projections have explicit
kind errors. Origin-form owns path/query rather than conflating //path with URI
authority. Role snapshots bound the source wire before other observations and
reconstruct canonical coordinates, rejecting hand-written injection strings.
Actual Url/UrlAuthority values are reused. Focused target/URL compatibility:
3 files / 70 groups, inline/example PASS. [Contract](http-target.md).

### HTTP metadata and body capability

Request/Response retain typed immutable coordinates and the declared BodyOpening
role without querying or opening it; fake role observation tests enforce that
boundary. Targets are canonical snapshots. Wire version identities have an actual
message consumer, while HTTP2 cannot gain a textual HTTP1 representation. No
TLS/ALPN/connection metadata is fabricated. Focused: 3 files / 59 groups, two
inline module runs and example PASS. [Contract](http-message.md).

### Strict HTTP1 heads

HTTP1 head parsing returns actual typed metadata, without an implicit empty-body
placeholder. Wire/line/count bounds, CRLF, exact tokens, Host, field grammar and
status semantics precede optional body assembly. Raw lines stay private even in
the shared implementation helper. Absolute-form remains legal on origin servers;
Host routing precedence is a later consumer duty. 3 files / 65 groups, inline and
example PASS. [Contract](http1-framing.md).

### HTTP1 asynchronous input ownership

Head operations borrow AsyncReading, publish only typed metadata + exact binary
tail, and own every child Future through FutureScope. Cancellation reproducer
exposed eager-registration reentrancy in FutureGroupState: publish identity/join
before startup and defer cancellation until the child exists. 54 runtime/scope
compatibility groups and 78 reader/head groups PASS; final stream fixture tolerates
physical short reads. Generic result construction uses concrete factories pending
an isolated inference follow-up.

### HTTP1 immutable body plans

Framing inspects validated metadata without observing BodyOpening/source roles.
Exact BigInt length comparisons precede conversion/read; duplicate lexemes remain
strict. Successful CONNECT ignores framing only after body-forbidden status
precedence. Persistence eligibility is explicitly conditional, not a live lease.
45 consumer / 91 compatibility groups, inline/example PASS.
[Contract](http1-body-framing.md) records unsupported transfer parameters/codecs.

### Chunk/trailer wire metadata

Unknown valid extensions stay internal. Hex sizes are exact, wire/numeric budgets
precede payload reads, and separately returned trailers never mutate head fields.
Known prohibited names/Connection nominations fail without exposing field payload.
Receiving custom metadata does not imply sender permission or merge semantics.
59 consumer / 134 compatibility groups, inline/example PASS.

### HTTP1 body ownership and role restrictions

Reader adopts explicit Closing owner, serializes reads, owns source/helper Future
cleanup and preserves the exact prefetched tail. Standard BodyReading exposes
trailers only at clean EOF, never concrete remainder(); sessions retain the framer.
IncomingBody takes length from that reader's own plan, preventing contradictory
metadata. ByteBuffer/Atomic capabilities are captured synchronously inside the
allowed helper constructor, retaining private access boundaries across async work.
22 final focused / 85 compatibility groups, inline/Unix example PASS; no VM edits.

### HTTP1 head effects

Metadata remains typed; the serializer materializes bounded canonical wire bytes
before the lazy write. Exact emitted size includes optional space and generated
status phrase. Each single write is a registered scope child; failure/cancellation
is memoized and cleanup drained. Body opening and session disposal are explicit
subsequent duties. 8 focused / 38 compatibility groups, inline/round-trip PASS.

### HTTP1 streaming output ownership

BodyOpening stays borrowed before actual reader opening. Known fixed contradictions
fail before open/write; actual short/extra source bytes invalidate started framing.
The opened reader always closes; separate current groups settle read/write cleanup
without retaining every completed payload Future. Empty/HEAD framing never opens
the source. Chunk payload and emitted metadata both have cumulative ceilings.
No rollback, full-message preflight or pool policy is fabricated.
[Contract](http1-body-writing.md).

### HTTP1 session leases

Http1Session adopts one explicit Closing owner and serializes exchanges through
atomic admission rather than implicit pipelining. Response BodyOpening retains
its lease until verified EOF; both policies and exact remainder decide reuse.
Early close/unopened drop/failure/cancel discard the owner; released readers cannot
abort a later active response. Startup groups are cleared and body operations
retain one active group. Failed owner close is retryable without reopening
admission. Supplied-stream exchanges do not manufacture transport/TLS details.
[Contract](http1-session.md), investigation.

### Complete HTTP1 emission

Request/Response source roles remain unobserved in construction. Started metadata
checks reject known contradictions before header I/O; empty requests require a
known empty source, while body-forbidden responses suppress hypothetical sources
entirely. Head/body Future ownership is scoped, without retaining payload chunks
or retrying effects. 11 focused / 26 compatibility groups, inline/round-trip PASS.

### Continue permission and deadline

Request-start captures source capabilities without opening them. The shared body
check preserves complete-message preflight and response suppression. One 100
permits one source opening; other 1xx and duplicate 100 do not alter that ownership.
Early final leaves the source unopened and disables reuse. A monotonic absolute
Before deadline survives informational replies; its timeout/cancellation drains
owned head/source cleanup. No interpreter behavior was changed.
[Contract](http1-continue.md).

## std/http/tls_session.ok

Actual selected ALPN is checked before ownership adoption. Exact/absent HTTP1
selection delegates existing leases; unsupported/case-variant selection retains
caller ownership and emits no HTTP bytes. [Contract](http1-tls-session.md),
investigation.

## std/http/endpoint.ok

HTTP route coordinates are validated before resolution. Numeric targets bypass
DNS; bounded sources are sampled once, all ports agree, each exact endpoint gets
one access decision, and only immutable allowed values escape. Public policy
uses pinned affirmative global properties; list policy snapshots CIDRs. Started
cancellation waits for resolver cleanup. [Contract](http-endpoints.md),
investigation.

## std/http/connection.ok

No second DNS lookup: exact checked endpoints are raced with bounded TcpRacing. HTTPS
identity belongs to the route URL; actual ALPN is checked before publication.
Raw future ownership disposes ready unclaimed results; unpublished transports
close on failure/cancellation. [Contract](http1-connecting.md),
investigation.

## std/net/tcp_racing.ok

Snapshot numeric candidates before effects; relative starts use predecessor
signals and injected Timing. Internal ordinal winners avoid opaque-handle equality.
Cancel/drain pending attempts and dispose ready unclaimed losers before publication;
parent cancellation reclaims every descriptor. [Contract](tcp-racing.md),
investigation.

## HTTP client policy — 2026-10-12

HttpLimits/ClientTimeouts/ClientPolicy inputs VERIFIED: seven focused assertions and inline PASS; [contract](http-client-policy.md). Actual client deadline enforcement and shared pools are still open.

## Direct HTTP1 client — 2026-10-12

[Contract](http-client.md) and ownership investigation. Direct HTTP1 routing, owned pool, empty-body release, independent deadlines and actual HTTPS metadata verified. Atomic remains borrowed. No claims for H2, stale retries or decorators.

## Explicit redirects — 2026-10-12

[Redirect contract](http-redirect.md), investigation. Value semantics, real follow/reuse and empty/nonempty policy denial verified. Fresh per-candidate checks remain single; cached leases recheck actual peer/new URL without lookup. No full gate; std-only.

## Cookie exchange consumer — 2026-10-12

[Contract](http-cookie-client.md), audit. Seventeen focused assertions cover context/conflict/bounds/body failure/cancellation and actual per-hop wire cookies. Caller metadata separates generated Cookie from caller override. Full regression omitted for std-only changes.

## Secret/Basic/Bearer provider slice - gate PASS

Native+std 52/471/472: thirteen/eight/eight assertions; actual HTTPS CLI 473: twenty-four; inline PASS. [Secret](secret-bytes.md), [auth](http-auth.md), context investigation. Full frozen-source gate: 672 files / 12185 assertions PASS.

## HTTP2 internal framing - 2026-10-12

[Contract](http2-wire-foundation.md). Local settings, exact binary headers, bounded incremental input, connection versus stream structural faults, next-frame alignment and cancellation drain verified. Public negotiated engine is still unavailable. Root focused tests five/seven/six; no runtime changes/full rerun.

### HTTP2 client preface verified

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

## Stage25 supplemental logging decision — 2026-10-13

The historical ledger above remains unchanged. New logging declarations are
reviewed against the [actual contract](logging.md) and [current inventory](stdlib-api-v4.md).

| Capability/value group | Identity/state | Decision |
|---|---|---|
| LogName, LogLevel, LogFields, LogContext, LogRecord | exact immutable name/level/JSON snapshots and explicit timestamp/cause | Keep coherent event representations; no reflection or arbitrary any dump; custom Error text is observed explicitly |
| Logging, NamedLogging, enrichment/filter/redaction | borrowed destination/clock and explicit immutable policy | Keep role confinement and nearest contextual precedence; no ambient task/global state |
| delivery policies and line/stream/terminal sinks | explicit destination, limits and formatting policy | Keep all-or-error delivery; loss/fallback/partial duplicates explicit; no recursive failure logging |
| LogFile, RotatingLogFile | owned lazy descriptor plus private Atomic state | Keep non-suspending single-sink actions, explicit close/reopen, regular-file rotation and restart retention; host latency/multi-process limits documented |
| QueuedLogs, SyslogLogs | bounded persistent FIFO, one supplied-scope worker, explicit borrowed UDP | Keep supervised lifetime, captured drain boundary and cancellation; no hidden tasks or capacity wait; eager caller latency remains observable |
| LogStyle and LevelOverride | explicit ordered exact nominal key/value entries | Replace enum/name-key map candidates; preserve identity without unsupported nominal map coercion |

Mutable queues/counters live in existing Atomic/Event capabilities. Helpers are
infrastructure; application callers receive Logging/LogWriting roles. This is an
Okkama design decision, not a claim that all event representations satisfy a
particular interpretation of Elegant Objects. Investigated limits.

## Locale/i18n and testing profiles — 2026-10-13

- CLDR root is `LocaleDataKey("root")`, not a falsely registered language. Data
  inheritance and RFC language matching remain separate. IANA aliases/full BCP47,
  number/date/currency/collation data are still separate roadmap work.
- Decimal removes visible zeroes; `PluralNumber` therefore retains spelling and
  explicit CanonicalPluralNumber adapts canonical numeric values. Plain notation
  has c/e zero. Published CLDR sample annotations are the independent test oracle.
- PluralForm sequences replace the withdrawn nominal-enum map candidate; sorted
  MessageKey values preserve context identity without concatenated-key collisions.
- Generated reductions retain Generated<T> rather than raw T so domain reproduction
  does not need reflection. Integer bounds follow the existing half-open random
  contract; finite float endpoints can be reached by rounding.
- ManualClock/RecordedCalls/Stub use existing Atomic/Event owners guarded by a
  narrow active-test flag. No test capability/environment is passed to native
  providers; no global clock replacement or mock generation.
- TestPosition exposes source(), because module is a reserved source keyword.
  Structured values/renderers do not imply a completed CLI runner bridge. TAP
  directives are escaped and independently parsed; raw byte output uses explicit
  hex/Base64 transport. JUnit waits for the XML profile.
- [Contract and gates](testing.md), [locale contracts](locale-i18n.md),
  investigations.

## Sequence/list continuation — 2026-10-13

SequenceGeneration represents configured sequence bounds and its element
generation; GeneratedSequence retains value/reproduction/reduction identity.
Lazy reduction plans are immutable internal collaborators, not mutable state
services. LocalizedList represents a bounded immutable sequence with explicit
locale/kind/width; generated CLDR tables remain std implementation data.
[Contracts](locale-list.md), [generation contract](testing.md).

## XML values and JUnit continuation

XmlName separates namespace identity from serialization prefix. XmlAttribute is
an explicit immutable name/value pair; ordered attribute sequences replace an
unsupported nominal-key map. XmlNode exposes represented XML plus bounded nesting,
with no lossy inner_text projection. XmlElement snapshots sequences and binds its
validated representation. JUnitTestReport consumes TestReport and these values;
no formatter reflection, arbitrary markup insertion or native XML provider.
[Contract](xml-values.md), [report policy](testing.md).

## Complete locale tag continuation

Locale represents one bounded complete canonical tag. Constructor arities and
coordinate projections remain coherent; absent language is now typed for
private-only and unreplaced grandfathered tags. LocaleExtensions and nominal
UnicodeLocaleKeyword sequences expose only explicit extension coordinates;
IANA tables are immutable generated data, not a host registry service. Range
matching and CLDR fallback retain separate policies. [Contract](locale-tag-guide.md).

## Shortest-float prerequisite

ShortestFloat represents a finite binary64 coordinate plus its sign identity,
projecting an exact verified Decimal. Internal interval/candidate values retain
bounded numeric coordinates rather than an arbitrary data bag. Only an explicitly
boxed float crosses the existing printf boundary to propose decimal candidates;
exact BigInt arithmetic, not native stringification, verifies acceptance.
[Contract](shortest-float.md).


## Localized numeric representations

NumberFormat/NumberPrecision are immutable policies; Localized* objects represent a
value plus locale plus policy. Private coefficient access belongs to Decimal/BigInt
public protocols, never reflection. Strict parsing returns nominal Decimal/BigInt
values. Exact decimal rounding precedes CLDR representation; finite float crosses
through verified ShortestFloat, negative zero remains a separate sign coordinate.
Nonfinite output is explicit NumberSpecial rather than relaxing builtin float.
Pinned generation compiles number patterns without runtime eval or host locale
state. Compact inverse is rejected; algorithmic numbering is typed unsupported.
Internal row/pattern/rounding collaborators remain importable because declarations
are not module-private. Budgets and scientific/padding parsing policy are explicit
in [the contract](locale-number.md); no native implementation was added.


## Explicit currency identity and representations

Currency represents a pinned current ISO code, with no locale/region inference.
Missing ISO minor units are typed rather than guessed. NumberFormat remains the
explicit decimal precision/sign policy; currency_digits replaces only fraction
counts when requested. LocalizedCurrency observes a Decimal/Currency/Locale/policy
representation. Names use rounded visible plural operands; numeric/affix frames
compose without reflection or mutable intermediate state.

NumberRenderingData and NumberCompactData are internal behavior roles used by
ordinary and currency representations. NumberRendered carries the actual compact
affix frame; accounting plus signs cannot borrow negative parentheses. CLDR has
no compact accounting family, so the contract composes its accounting frame with
the requested decimal compact unit. Name display retains the CLDR unit form.

Pinned ISO/SIX, CLDR parent deltas and Unicode symbol-boundary categories are
separate versioned data. Generated translation shards bound source concentration;
lexer scalar indexing remains PERF-52. Source preservation and independent ISO
oracles cover the data boundary. [Contract](locale-currency.md). No native change.


## Localized Gregorian wall and instant representations

Gregorian domain primitives remain independent of locale infrastructure.
LocalizedDate/Time/LocalDateTime represent immutable wall coordinates under an
explicit policy and retain their derived text. LocalizedInstant represents an
absolute value plus an explicitly supplied TimeZone; each projection freezes a
consistent observation while reusing immutable locale data. There is no implicit
locale zone or calendar conversion. Invalid policy is detected at construction.

Pinned CLDR patterns compile into bounded pieces, not eval or host strftime.
Numeric substitution is separate from literal month/weekday/era/period names.
Hawaiian Roman month numbering is a finite explicit override. Flexible period
rules use their own supplemental language inheritance, not the calendar parent
chain. Wall long/full omit zone fields through the medium template.

Historical metazones and aliases choose names only for a compatible explicit
zone observation; free incompatible FixedZone labels use localized GMT.
CLDR root aliases, source-field preservation and independent date rendering
oracles cover data boundaries. The naming and program internals remain
importable because module-private declarations are unavailable.
[Contract](locale-date-time.md); no interpreter/native source changes.


## Explicit collation and pinned native ownership

CollatedString retains original text and a derived opaque key under a nominal
Locale/CollationOptions identity. Every option and the pinned data version must
match before order(); raw keys carry no such checks and are not persisted.
ICU 76.1/Unicode and UCA 16/CLDR 46.0 is loaded lazily from an explicit pinned
installation, checked at use, and never substituted with host data. Native
pointers stay private, each collator closes once on success and failure, and
wire failures are returned to the interpreter. Native and language focused
checks plus frozen full regression: 889 files / 17,515 checks PASS.
[Contract](locale-collation.md). No new language syntax or VSCode tokens.


## GNU catalog and active PO representations

GettextCatalog parses input bytes without IO/global domains and retains nominal
messages plus a bounded expression tree. Unsigned64 arithmetic and exact Decimal
counts avoid host-width/float ambiguity. GettextTranslation adapts the existing
Translating role; malformed inputs/counts cannot trigger fallback. MessageKey
presence distinguishes absent and explicitly empty context without breaking its
existing constructor or order. Copying retains the new coordinate.

PO metadata uses ordered nominal headers; comments use ordered nominal records,
not grouped bags or unordered flags. Linked fragments avoid whole-field copying,
local builder cells close on errors, and PoString retains only derived text.
Supported MO 0.0, codecs and active PO bounds are explicit; unsupported revisions,
charsets, domains and obsolete source fail. Independent GNU/Python fixtures and
GNU recompilation validate context/plural/metadata semantics. [Contract](gettext.md).
No native/interpreter or syntax changes; VSCode's existing rules cover the API.


## Testing completion — 2026-10-13

[Contract](testing.md), [diff decisions](diff.md),
evidence. Native code now owns bounded
recorder/stub cursors, manual clock domain minting, descriptor snapshot effects,
and LCS matching; nominal payloads and representations stay in std. No arbitrary
T, interpreter, contextual capability or user object crosses the new ABI.

Private rights survive async loop resumption and defer cleanup. Owned await
cancellation joins child cleanup; borrowed future detachment keeps its prior
policy. TestFailure causes retain the original Error when matching fails.
Process workers have fresh registries; reports preserve discovery order, explicit
seeds and bytes. Snapshot checking never writes and update is an explicit effect.

Compatibility decisions: fake.storage and the three-key test_results projection
remain available, while new code uses reading/writing and detailed reports.
The old "remove storage before v3" proposal is superseded by deprecation rather
than a regression-inducing deletion. NaN/Inf checker criteria are inapplicable to
the current finite float category. Custom Generated implementations retain their
role obligations; explicit DistinctReductions enforces reproduction-key identity
and core properties preserve lazy sequence shrink plans.
