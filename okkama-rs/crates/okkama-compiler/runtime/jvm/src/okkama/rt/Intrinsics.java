package okkama.rt;

import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.math.BigInteger;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

/**
 * Intrinsics std {@code async/*} (OIR-1 §14.2): действия объектов, чьё
 * состояние держит рантайм ({@link Obj#intrinsic}). Перенос состояний
 * Perl-интерпретатора ({@code Channel.pm}, {@code AsyncEvent.pm}, …).
 *
 * <p>Заглушка программы передаёт классы объектов, которые intrinsic создаёт
 * (результаты, Error std), затем аргументы в коллекционном представлении
 * ({@code bool} — {@code Boolean}, …); результат — так же. Объекты программы
 * создаются отражением; операции ожидания — холодные Future с телом на Java,
 * ждущие обещаний ({@link Future#promise}).
 */
public final class Intrinsics {
    private Intrinsics() {}

    // ----- общее -----

    interface Op {
        Object run();
    }

    /** Тело холодного Future операции: Java-код над захватами. */
    static final class OpFn implements Fn {
        private final Op op;
        private final Object[] retained;

        OpFn(Op op, Object[] retained) {
            this.op = op;
            this.retained = retained;
        }

        public Object call(Object[] args) {
            return op.run();
        }

        public Object[] captures() {
            return retained;
        }
    }

    static Future op(Op op, Object... retained) {
        return Future.cold(new OpFn(op, retained));
    }

    /** Объект программы: конструктор класса с полями в порядке объявления. */
    static Object make(Object type, Object... fields) {
        try {
            Constructor<?> constructor = ((Class<?>) type).getDeclaredConstructors()[0];
            constructor.setAccessible(true);
            return constructor.newInstance(fields);
        } catch (ReflectiveOperationException failure) {
            throw Rt.trap("Cannot construct " + ((Class<?>) type).getName() + ": " + failure);
        }
    }

    static Failure thrown(Object type, Object... fields) {
        return Rt.thrown(make(type, fields));
    }

    static <T> T state(Object self, Class<T> type, String what) {
        Object state = ((Obj) self).intrinsic;
        if (!type.isInstance(state)) {
            throw Rt.trap(what);
        }
        return type.cast(state);
    }

    static Object attach(Object object, Object state) {
        ((Obj) object).intrinsic = state;
        return object;
    }

    static boolean running(Future future) {
        return future.state == Future.RUNNING;
    }

    /** Ожидающий: обещание и, у отправителя, значение. */
    static final class Waiter {
        final Future promise = Future.promise();
        final Object value;
        boolean active = true;

        Waiter(Object value) {
            this.value = value;
        }
    }

    /** Ждать обещание ожидающего; отмена снимает его с очереди. */
    static Object await(Waiter waiter) {
        try {
            return Scheduler.await(waiter.promise);
        } catch (Failure failure) {
            waiter.active = false;
            throw failure;
        }
    }

    static Waiter take(ArrayDeque<Waiter> queue) {
        while (!queue.isEmpty()) {
            Waiter waiter = queue.poll();
            if (waiter.active) {
                waiter.active = false;
                return waiter;
            }
        }
        return null;
    }

    /** Продолжить всех ожидающих очереди по порядку. */
    static void releaseAll(ArrayDeque<Waiter> queue) {
        Waiter waiter;
        while ((waiter = take(queue)) != null) {
            if (running(waiter.promise)) {
                Scheduler.complete(waiter.promise, null, null);
            }
        }
    }

    /** Ждать, пока очередь событий откроется: {@code wait()} Event/Cancelling. */
    static Object waitUntil(boolean open, ArrayDeque<Waiter> queue, Object self) {
        if (!open) {
            Waiter waiter = new Waiter(null);
            queue.add(waiter);
            await(waiter);
        }
        return self;
    }

    // ----- async/channel -----

    static final class ChannelState {
        final int capacity;
        boolean closed;
        final ArrayDeque<Object> messages = new ArrayDeque<Object>();
        final ArrayDeque<Waiter> senders = new ArrayDeque<Waiter>();
        final ArrayDeque<Waiter> receivers = new ArrayDeque<Waiter>();

        ChannelState(int capacity) {
            this.capacity = capacity;
        }

        void admitWaitingSender() {
            Waiter sender = take(senders);
            if (sender == null) {
                return;
            }
            Waiter receiver = take(receivers);
            if (receiver != null) {
                Scheduler.complete(receiver.promise, sender.value, null);
            } else {
                messages.add(sender.value);
            }
            if (running(sender.promise)) {
                Scheduler.complete(sender.promise, null, null);
            }
        }

        void close(Object closedType) {
            if (closed) {
                return;
            }
            closed = true;
            Waiter sender;
            while ((sender = take(senders)) != null) {
                if (running(sender.promise)) {
                    Scheduler.complete(sender.promise, null, thrown(closedType));
                }
            }
            if (messages.isEmpty()) {
                Waiter receiver;
                while ((receiver = take(receivers)) != null) {
                    if (running(receiver.promise)) {
                        Scheduler.complete(receiver.promise, null, thrown(closedType));
                    }
                }
            }
        }
    }

    private static ChannelState channel(Object self) {
        return state(self, ChannelState.class, "Invalid channel endpoint");
    }

    public static Object channel_Channel_new_capacity(Object self, Object invalid, Object capacity) {
        BigInteger value = (BigInteger) capacity;
        if (value.signum() < 0) {
            throw thrown(invalid, value);
        }
        if (value.bitLength() > 31) {
            throw Rt.trap("Channel capacity is too large");
        }
        return attach(make(self), new ChannelState(value.intValue()));
    }

    public static Object channel_Channel_sending(Object sending, Object self) {
        return attach(make(sending), channel(self));
    }

    public static Object channel_Channel_receiving(Object receiving, Object self) {
        return attach(make(receiving), channel(self));
    }

    public static Object channel_Sending_new(Object self) {
        return make(self);
    }

    public static Object channel_Receiving_new(Object self) {
        return make(self);
    }

    public static Object channel_Sending_send(Object closed, Object self, Object value) {
        return op(() -> {
            ChannelState state = channel(self);
            if (state.closed) {
                throw thrown(closed);
            }
            Waiter receiver = take(state.receivers);
            if (receiver != null) {
                Scheduler.complete(receiver.promise, value, null);
                return self;
            }
            if (state.messages.size() < state.capacity) {
                state.messages.add(value);
                return self;
            }
            Waiter waiter = new Waiter(value);
            state.senders.add(waiter);
            await(waiter);
            return self;
        }, self, value);
    }

    public static Object channel_Receiving_item(Object closed, Object self) {
        return op(() -> {
            ChannelState state = channel(self);
            if (!state.messages.isEmpty()) {
                Object value = state.messages.poll();
                if (!state.closed) {
                    state.admitWaitingSender();
                }
                return value;
            }
            Waiter sender = take(state.senders);
            if (sender != null) {
                if (running(sender.promise)) {
                    Scheduler.complete(sender.promise, null, null);
                }
                return sender.value;
            }
            if (state.closed) {
                throw thrown(closed);
            }
            Waiter waiter = new Waiter(null);
            state.receivers.add(waiter);
            return await(waiter);
        }, self);
    }

    public static void channel_Sending_close(Object closed, Object self) {
        channel(self).close(closed);
    }

    public static void channel_Receiving_close(Object closed, Object self) {
        channel(self).close(closed);
    }

    // ----- async/event -----

    static final class EventState {
        boolean ready;
        final ArrayDeque<Waiter> waiting = new ArrayDeque<Waiter>();

        EventState(boolean ready) {
            this.ready = ready;
        }
    }

    private static EventState event(Object self) {
        return state(self, EventState.class, "Invalid async event");
    }

    public static Object event_Event_new(Object self) {
        return attach(make(self, Boolean.FALSE), new EventState(false));
    }

    public static Object event_Event_new_ready(Object self, Object ready) {
        return attach(make(self, ready), new EventState((Boolean) ready));
    }

    public static Object event_Event_ready(Object self) {
        return Boolean.valueOf(event(self).ready);
    }

    public static void event_Event_set(Object self) {
        EventState state = event(self);
        if (state.ready) {
            return;
        }
        state.ready = true;
        releaseAll(state.waiting);
    }

    public static Object event_Event_wait(Object self) {
        return op(() -> waitUntil(event(self).ready, event(self).waiting, self), self);
    }

    // ----- async/atomic -----

    static final class AtomicState {
        Object value;
        boolean closed;

        AtomicState(Object value) {
            this.value = value;
        }
    }

    private static AtomicState atomic(Object self) {
        return state(self, AtomicState.class, "Invalid atomic state");
    }

    private static AtomicState open(Object closed, Object self) {
        AtomicState state = atomic(self);
        if (state.closed) {
            throw thrown(closed);
        }
        return state;
    }

    public static Object atomic_Atomic_new_value(Object self, Object value) {
        return attach(make(self), new AtomicState(value));
    }

    public static Object atomic_Atomic_value(Object closed, Object self) {
        return open(closed, self).value;
    }

    public static void atomic_Atomic_replace(Object closed, Object self, Object value) {
        open(closed, self).value = value;
    }

    public static Object atomic_Atomic_compare_and_replace(
        Object closed, Object self, Object expected, Object revised) {
        AtomicState state = open(closed, self);
        if (!Objects.equals(state.value, expected)) {
            return Boolean.FALSE;
        }
        state.value = revised;
        return Boolean.TRUE;
    }

    public static Object atomic_Atomic_closed(Object self) {
        return Boolean.valueOf(atomic(self).closed);
    }

    public static void atomic_Atomic_close(Object self) {
        AtomicState state = atomic(self);
        state.closed = true;
        state.value = null;
    }

    // ----- async/cancellation -----

    static final class CancelState {
        boolean requested;
        final ArrayDeque<Waiter> waiting = new ArrayDeque<Waiter>();
    }

    private static CancelState cancellation(Object self) {
        return state(self, CancelState.class, "Invalid cancellation capability");
    }

    public static Object cancellation_Cancellation_new(Object self) {
        return attach(make(self), new CancelState());
    }

    public static Object cancellation_CancellationToken_new(Object self) {
        return make(self);
    }

    public static Object cancellation_Cancellation_cancelling(Object token, Object self) {
        return attach(make(token), cancellation(self));
    }

    public static void cancellation_Cancellation_cancel(Object self) {
        CancelState state = cancellation(self);
        if (state.requested) {
            return;
        }
        state.requested = true;
        releaseAll(state.waiting);
    }

    public static Object cancellation_CancellationToken_requested(Object self) {
        return Boolean.valueOf(cancellation(self).requested);
    }

    public static void cancellation_CancellationToken_check(Object self) {
        if (cancellation(self).requested) {
            throw Rt.thrown(new Cancelled());
        }
    }

    public static Object cancellation_CancellationToken_wait(Object self) {
        return op(() -> waitUntil(cancellation(self).requested, cancellation(self).waiting, self), self);
    }

    // ----- async/scope -----

    static final class GroupState {
        boolean sealed;
        boolean cancelRequested;
        int active;
        final ArrayList<Future> entries = new ArrayList<Future>();
        final ArrayList<Boolean> groupCancelled = new ArrayList<Boolean>();
        Future completion;
    }

    private static GroupState group(Object self) {
        return state(self, GroupState.class, "Invalid future group");
    }

    public static Object scope_FutureGroup_new(Object self) {
        return attach(make(self), new GroupState());
    }

    public static void scope_FutureGroup_start(Object finished, Object self, Object origin) {
        GroupState state = group(self);
        Future future = (Future) origin;
        if (state.entries.contains(future)) {
            return;
        }
        if (state.sealed) {
            throw thrown(finished);
        }
        state.entries.add(future);
        state.groupCancelled.add(Boolean.FALSE);
        state.active++;
        Scheduler.register(future);
        Scheduler.start(future, null);
        future.listen(() -> state.active--);
    }

    public static Object scope_FutureGroup_active(Object self) {
        return BigInteger.valueOf(group(self).active);
    }

    public static void scope_FutureGroup_cancel(Object self) {
        cancelGroup(group(self));
    }

    private static void cancelGroup(GroupState state) {
        state.sealed = true;
        if (state.cancelRequested) {
            return;
        }
        state.cancelRequested = true;
        for (int index = 0; index < state.entries.size(); index++) {
            Future future = state.entries.get(index);
            if (future.state >= Future.DONE) {
                if (Future.cancelled(future)) {
                    state.groupCancelled.set(index, Boolean.TRUE);
                }
                continue;
            }
            state.groupCancelled.set(index, Boolean.TRUE);
            Scheduler.cancel(future);
        }
    }

    public static Object scope_FutureGroup_wait(Object self) {
        return op(() -> {
            GroupState state = group(self);
            state.sealed = true;
            if (state.completion == null) {
                state.completion = Future.promise();
                for (Future future : state.entries) {
                    future.observed = true;
                }
                whenAll(state.entries, () -> {
                    Failure primary = null;
                    for (int index = 0; index < state.entries.size(); index++) {
                        Future future = state.entries.get(index);
                        if (future.state != Future.FAILED
                            || state.groupCancelled.get(index) && Future.cancelled(future)) {
                            continue;
                        }
                        if (primary == null) {
                            primary = future.failure;
                        } else {
                            Rt.secondary(primary, future.failure);
                        }
                    }
                    Scheduler.complete(state.completion, null, primary);
                });
            }
            try {
                Scheduler.await(state.completion);
            } catch (Failure failure) {
                if (failure.error instanceof Cancelled) {
                    cancelGroup(state);
                }
                throw failure;
            }
            return self;
        }, self);
    }

    /** {@code then} — когда завершатся все {@code futures} (сразу, если уже). */
    private static void whenAll(List<Future> futures, Runnable then) {
        int[] pending = {0};
        for (Future future : futures) {
            if (future.state < Future.DONE) {
                pending[0]++;
            }
        }
        if (pending[0] == 0) {
            then.run();
            return;
        }
        for (Future future : futures) {
            if (future.state < Future.DONE) {
                future.listen(() -> {
                    if (--pending[0] == 0) {
                        then.run();
                    }
                });
            }
        }
    }

    // ----- async/collect -----

    /** Элементы {@code Sequencing<Future<T>>}: поле {@code origins} объекта. */
    private static List<Future> origins(Object self) {
        try {
            Field field = self.getClass().getDeclaredField("origins");
            field.setAccessible(true);
            Object sequence = field.get(self);
            Method size = method(sequence, "size", 0);
            Method item = method(sequence, "item", 1);
            int count = ((BigInteger) size.invoke(sequence)).intValue();
            ArrayList<Future> futures = new ArrayList<Future>(count);
            for (int index = 0; index < count; index++) {
                futures.add((Future) item.invoke(sequence, BigInteger.valueOf(index)));
            }
            return futures;
        } catch (InvocationTargetException failure) {
            Throwable cause = failure.getCause();
            if (cause instanceof Failure) {
                throw (Failure) cause;
            }
            throw Rt.trap("Cannot read origins: " + cause);
        } catch (ReflectiveOperationException failure) {
            throw Rt.trap("Cannot read origins: " + failure);
        }
    }

    private static Method method(Object target, String name, int arity) {
        for (Class<?> type = target.getClass(); type != null; type = type.getSuperclass()) {
            for (Class<?> role : type.getInterfaces()) {
                for (Method candidate : role.getMethods()) {
                    if (candidate.getName().equals(name) && candidate.getParameterTypes().length == arity) {
                        candidate.setAccessible(true);
                        return candidate;
                    }
                }
            }
        }
        throw Rt.trap("Sequencing value has no " + name + "()");
    }

    /** Повторный {@code results()}/{@code result()} даёт тот же Future. */
    static final class Retained {
        Future result;
    }

    private static Retained retained(Object self) {
        Obj object = (Obj) self;
        if (!(object.intrinsic instanceof Retained)) {
            object.intrinsic = new Retained();
        }
        return (Retained) object.intrinsic;
    }

    private static void startAll(List<Future> futures) {
        for (Future future : futures) {
            future.observed = true;
            Scheduler.start(future, null);
            Scheduler.register(future);
        }
    }

    public static Object collect_AllFutures_results(Object sequence, Object self) {
        Retained state = retained(self);
        if (state.result == null) {
            state.result = op(() -> allFutures(sequence, self), self);
        }
        return state.result;
    }

    /**
     * Все Future по порядку входа; отказ одного отменяет незавершённых,
     * ждёт их и отдаёт первый отказ с остальными вторичными.
     */
    private static Object allFutures(Object sequence, Object self) {
        List<Future> futures = origins(self);
        int count = futures.size();
        ArrayList<Object> values = new ArrayList<Object>(count);
        for (int index = 0; index < count; index++) {
            values.add(null);
        }
        if (count == 0) {
            return make(sequence, values);
        }
        startAll(futures);
        Future done = Future.promise();
        int[] remaining = {count};
        boolean[] finishing = {false};
        for (int index = 0; index < count; index++) {
            int position = index;
            Future future = futures.get(index);
            future.listen(() -> {
                if (finishing[0] || !running(done)) {
                    return;
                }
                if (future.state == Future.FAILED) {
                    finishing[0] = true;
                    ArrayList<Future> cancelledHere = cancelOthers(futures, future);
                    whenAll(futures, () -> {
                        for (Future other : futures) {
                            if (other == future || other.state != Future.FAILED
                                || cancelledHere.contains(other) && Future.cancelled(other)) {
                                continue;
                            }
                            Rt.secondary(future.failure, other.failure);
                        }
                        Scheduler.complete(done, null, future.failure);
                    });
                    return;
                }
                values.set(position, future.value);
                if (--remaining[0] == 0) {
                    Scheduler.complete(done, null, null);
                }
            });
        }
        try {
            Scheduler.await(done);
        } catch (Failure failure) {
            if (failure.error instanceof Cancelled && !finishing[0]) {
                finishing[0] = true;
                cancelOthers(futures, null);
            }
            throw failure;
        }
        return make(sequence, values);
    }

    private static ArrayList<Future> cancelOthers(List<Future> futures, Future keep) {
        ArrayList<Future> cancelled = new ArrayList<Future>();
        for (Future other : futures) {
            if (other != keep && other.state < Future.DONE) {
                cancelled.add(other);
                Scheduler.cancel(other);
            }
        }
        return cancelled;
    }

    public static Object collect_Raced_result(Object none, Object self) {
        Retained state = retained(self);
        if (state.result == null) {
            state.result = op(() -> raced(none, self), self);
        }
        return state.result;
    }

    /** Исход первого завершившегося; проигравшие отменяются и дожидаются. */
    private static Object raced(Object none, Object self) {
        List<Future> futures = origins(self);
        if (futures.isEmpty()) {
            throw thrown(none);
        }
        startAll(futures);
        Future done = Future.promise();
        Future[] winner = {null};
        for (Future future : futures) {
            future.listen(() -> {
                if (winner[0] != null) {
                    return;
                }
                winner[0] = future;
                cancelOthers(futures, future);
                whenAll(futures, () -> Scheduler.complete(done, future.value,
                    future.state == Future.FAILED ? future.failure : null));
            });
        }
        return Scheduler.await(done);
    }

    public static Object collect_FirstSuccessful_result(Object none, Object self) {
        Retained state = retained(self);
        if (state.result == null) {
            state.result = op(() -> firstSuccessful(none, self), self);
        }
        return state.result;
    }

    /**
     * Первый успешный исход; проигравшие отменяются и дожидаются. Если все
     * отказали — первый отказ с остальными вторичными.
     */
    private static Object firstSuccessful(Object none, Object self) {
        List<Future> futures = origins(self);
        if (futures.isEmpty()) {
            throw thrown(none);
        }
        startAll(futures);
        Future done = Future.promise();
        boolean[] finishing = {false};
        int[] remaining = {futures.size()};
        ArrayList<Failure> failures = new ArrayList<Failure>();
        for (Future future : futures) {
            future.listen(() -> {
                if (finishing[0] || !running(done)) {
                    return;
                }
                if (future.state == Future.DONE) {
                    finishing[0] = true;
                    cancelOthers(futures, future);
                    whenAll(futures, () -> Scheduler.complete(done, future.value, null));
                    return;
                }
                failures.add(future.failure);
                if (--remaining[0] == 0) {
                    finishing[0] = true;
                    Failure primary = failures.get(0);
                    for (Failure other : failures.subList(1, failures.size())) {
                        Rt.secondary(primary, other);
                    }
                    Scheduler.complete(done, null, primary);
                }
            });
        }
        try {
            return Scheduler.await(done);
        } catch (Failure failure) {
            if (failure.error instanceof Cancelled && !finishing[0]) {
                finishing[0] = true;
                cancelOthers(futures, null);
            }
            throw failure;
        }
    }

    /** Поток завершений: входы, очередь завершившихся и ожидающие (FIFO). */
    static final class Completions {
        List<Future> origins;
        boolean started;
        int reserved;
        int delivered;
        final ArrayDeque<Future> queue = new ArrayDeque<Future>();
        final ArrayDeque<Future> waiters = new ArrayDeque<Future>();
        boolean aborting;
        Failure aborted;
    }

    private static Completions completions(Object self) {
        Obj object = (Obj) self;
        if (!(object.intrinsic instanceof Completions)) {
            Completions state = new Completions();
            state.origins = origins(self);
            object.intrinsic = state;
        }
        return (Completions) object.intrinsic;
    }

    public static Object collect_FutureCompletions_remaining_count(Object self) {
        Completions state = completions(self);
        return BigInteger.valueOf(state.origins.size() - state.delivered);
    }

    private static void emit(Completions state, Future future) {
        if (state.aborting) {
            return;
        }
        Future waiter = state.waiters.poll();
        if (waiter != null) {
            state.delivered++;
            Scheduler.complete(waiter, future, null);
        } else {
            state.queue.add(future);
        }
    }

    private static void abort(Completions state, Failure cancelled) {
        if (state.aborting) {
            return;
        }
        state.aborting = true;
        ArrayList<Future> mine = cancelOthers(state.origins, null);
        whenAll(state.origins, () -> {
            for (Future future : state.origins) {
                if (future.state == Future.FAILED && !(mine.contains(future) && Future.cancelled(future))) {
                    Rt.secondary(cancelled, future.failure);
                }
            }
            state.aborted = cancelled;
            for (Future waiter : state.waiters) {
                if (running(waiter)) {
                    Scheduler.complete(waiter, null, cancelled);
                }
            }
            state.waiters.clear();
            state.queue.clear();
            state.delivered = state.origins.size();
        });
    }

    public static Object collect_FutureCompletions_completion(Object none, Object self) {
        Completions state = completions(self);
        return op(() -> {
            if (state.aborted != null) {
                throw state.aborted;
            }
            if (state.reserved >= state.origins.size()) {
                throw thrown(none);
            }
            state.reserved++;
            Future next = state.queue.poll();
            if (next != null) {
                state.delivered++;
                return next;
            }
            Future waiter = Future.promise();
            state.waiters.add(waiter);
            if (!state.started) {
                state.started = true;
                startAll(state.origins);
                for (Future future : state.origins) {
                    future.listen(() -> emit(state, future));
                }
            }
            try {
                return Scheduler.await(waiter);
            } catch (Failure failure) {
                if (failure.error instanceof Cancelled && running(waiter)) {
                    state.waiters.remove(waiter);
                    abort(state, failure);
                }
                throw failure;
            }
        }, self);
    }

    // ----- тестовый режим (§14) -----

    public static void test_ok(Object condition) {
        if (!(Boolean) condition) {
            Tests.fail("expected true, got false");
        }
    }

    public static void test_is(Object actual, Object expected) {
        if (!Tests.same(actual, expected)) {
            Tests.fail("expected " + Tests.show(expected) + ", got " + Tests.show(actual));
        }
    }

    public static void test_isnt(Object actual, Object expected) {
        if (Tests.same(actual, expected)) {
            Tests.fail("expected values to differ, both were " + Tests.show(actual));
        }
    }

    public static void test_fail(Object message) {
        Tests.fail((String) message);
    }

    /** Хранилище экземпляра fake: ключи — значения рантайма. */
    @SuppressWarnings("unchecked")
    private static java.util.HashMap<Object, Object> storage(Object self) {
        Obj object = (Obj) self;
        if (!(object.intrinsic instanceof java.util.HashMap)) {
            object.intrinsic = new java.util.HashMap<Object, Object>();
        }
        return (java.util.HashMap<Object, Object>) object.intrinsic;
    }

    public static void fake_storage_set(Object self, Object key, Object value) {
        storage(self).put(key, value);
    }

    public static Object fake_storage_get(Object self, Object key) {
        java.util.HashMap<Object, Object> entries = storage(self);
        if (!entries.containsKey(key)) {
            throw Rt.trap("Fake storage has no key " + Tests.show(key));
        }
        return entries.get(key);
    }

    public static Object fake_storage_has(Object self, Object key) {
        return Boolean.valueOf(storage(self).containsKey(key));
    }

    public static void fake_storage_remove(Object self, Object key) {
        java.util.HashMap<Object, Object> entries = storage(self);
        if (!entries.containsKey(key)) {
            throw Rt.trap("Fake storage has no key " + Tests.show(key));
        }
        entries.remove(key);
    }

    public static void fake_storage_clear(Object self) {
        storage(self).clear();
    }

    public static Object fake_storage_size(Object self) {
        return BigInteger.valueOf(storage(self).size());
    }

    // ----- массивы -----

    /** {@code xs.ordering()}: устойчивая сортировка по {@code order()} элементов. */
    public static Object array_ordering(Object array) {
        ArrayList<Object> sorted = new ArrayList<Object>((List<?>) array);
        sorted.sort(Intrinsics::order);
        return sorted;
    }

    /** Порядок двух значений: встроенные — сами, объекты — их {@code order(other)}. */
    static int order(Object left, Object right) {
        if (left instanceof BigInteger) {
            return ((BigInteger) left).compareTo((BigInteger) right);
        }
        if (left instanceof String) {
            return KeyOrder.compareCodePoints((String) left, (String) right);
        }
        if (left instanceof Double) {
            return Double.compare((Double) left, (Double) right);
        }
        if (left instanceof Rat) {
            return Rat.compare((Rat) left, (Rat) right);
        }
        if (left instanceof Byte) {
            return Integer.compare(((Byte) left) & 0xFF, ((Byte) right) & 0xFF);
        }
        if (left instanceof Integer) {
            return Integer.compare((Integer) left, (Integer) right);
        }
        try {
            Object result = orderMethod(left).invoke(left, right);
            return ((Case) result).ordinal - 1;
        } catch (InvocationTargetException failure) {
            Throwable cause = failure.getCause();
            if (cause instanceof RuntimeException) {
                throw (RuntimeException) cause;
            }
            throw Rt.trap("ordering failed: " + cause);
        } catch (ReflectiveOperationException failure) {
            throw Rt.trap("ordering failed: " + failure);
        }
    }

    /** Мост {@code order} роли {@code Comparing}, который объект реализует. */
    private static Method orderMethod(Object value) throws NoSuchMethodException {
        for (Method candidate : value.getClass().getMethods()) {
            if (candidate.getName().equals("order") && candidate.getParameterTypes().length == 1) {
                candidate.setAccessible(true);
                return candidate;
            }
        }
        throw new NoSuchMethodException("order");
    }

    // ----- async/lock, async/semaphore -----

    /** Владение с очередью: замок (одно разрешение) и семафор (N разрешений). */
    static final class PermitState {
        int available;
        final ArrayDeque<Waiter> waiting = new ArrayDeque<Waiter>();

        PermitState(int available) {
            this.available = available;
        }
    }

    /** Разрешение: закрытие возвращает его следующему ожидающему или в запас. */
    static final class Grant {
        final PermitState origin;
        boolean closed;

        Grant(PermitState origin) {
            this.origin = origin;
        }
    }

    private static Object acquire(PermitState state, Object grantType) {
        if (state.available > 0) {
            state.available--;
            return attach(make(grantType), new Grant(state));
        }
        Waiter waiter = new Waiter(null);
        state.waiting.add(waiter);
        return await(waiter);
    }

    private static void release(Object grantType, Grant grant) {
        if (grant.closed) {
            return;
        }
        grant.closed = true;
        PermitState state = grant.origin;
        Waiter waiter = take(state.waiting);
        if (waiter != null && running(waiter.promise)) {
            // Следующий ожидающий получает новое разрешение сразу.
            Scheduler.complete(waiter.promise, attach(make(grantType), new Grant(state)), null);
        } else {
            state.available++;
        }
    }

    private static PermitState lock(Object self) {
        return state(self, PermitState.class, "Invalid async lock");
    }

    private static Grant grant(Object self) {
        return state(self, Grant.class, "Invalid async lock guard");
    }

    public static Object lock_Lock_new(Object self) {
        return attach(make(self), new PermitState(1));
    }

    public static Object lock_Lock_locked(Object self) {
        return Boolean.valueOf(lock(self).available == 0);
    }

    public static Object lock_Lock_guard(Object guardType, Object self) {
        return op(() -> acquire(lock(self), guardType), self);
    }

    public static Object lock_LockGuard_closed(Object self) {
        return Boolean.valueOf(grant(self).closed);
    }

    public static void lock_LockGuard_close(Object self) {
        release(self.getClass(), grant(self));
    }

    /** Состояние семафора создаётся при первом использовании из поля {@code amount}. */
    private static PermitState semaphore(Object self) {
        Obj object = (Obj) self;
        if (object.intrinsic == null) {
            object.intrinsic = new PermitState(((BigInteger) field(self, "amount")).intValue());
        }
        return state(self, PermitState.class, "Invalid async semaphore");
    }

    static Object field(Object self, String name) {
        try {
            Field field = self.getClass().getDeclaredField(name);
            field.setAccessible(true);
            return field.get(self);
        } catch (ReflectiveOperationException failure) {
            throw Rt.trap("Cannot read field " + name + ": " + failure);
        }
    }

    public static Object semaphore_Semaphore_permits(Object self) {
        return BigInteger.valueOf(semaphore(self).available);
    }

    public static Object semaphore_Semaphore_permit(Object permitType, Object self) {
        return op(() -> acquire(semaphore(self), permitType), self);
    }

    public static Object semaphore_Permit_closed(Object self) {
        return Boolean.valueOf(state(self, Grant.class, "Invalid async semaphore permit").closed);
    }

    public static void semaphore_Permit_close(Object self) {
        release(self.getClass(), state(self, Grant.class, "Invalid async semaphore permit"));
    }

    // ----- async/barrier -----

    static final class BarrierState {
        final int parties;
        int generation;
        boolean broken;
        final ArrayList<Waiter> waiting = new ArrayList<Waiter>();
        Object brokenType;

        BarrierState(int parties) {
            this.parties = parties;
        }

        int active() {
            int count = 0;
            for (Waiter waiter : waiting) {
                if (waiter.active) {
                    count++;
                }
            }
            return count;
        }

        void breakGeneration() {
            if (broken) {
                return;
            }
            broken = true;
            ArrayList<Waiter> live = new ArrayList<Waiter>(waiting);
            waiting.clear();
            for (Waiter waiter : live) {
                if (waiter.active && running(waiter.promise)) {
                    waiter.active = false;
                    Scheduler.complete(waiter.promise, null, thrown(brokenType));
                }
            }
        }
    }

    private static BarrierState barrier(Object self) {
        Obj object = (Obj) self;
        if (object.intrinsic == null) {
            object.intrinsic = new BarrierState(((BigInteger) field(self, "amount")).intValue());
        }
        return state(self, BarrierState.class, "Invalid async barrier");
    }

    public static Object barrier_Barrier_wait(Object broken, Object self) {
        return op(() -> {
            BarrierState state = barrier(self);
            state.brokenType = broken;
            if (state.broken) {
                throw thrown(broken);
            }
            Waiter waiter = new Waiter(BigInteger.valueOf(state.active()));
            state.waiting.add(waiter);
            if (state.active() == state.parties) {
                ArrayList<Waiter> arrivals = new ArrayList<Waiter>(state.waiting);
                state.waiting.clear();
                state.generation++;
                for (Waiter arrival : arrivals) {
                    if (arrival.active && running(arrival.promise)) {
                        arrival.active = false;
                        Scheduler.complete(arrival.promise, arrival.value, null);
                    }
                }
            }
            try {
                return Scheduler.await(waiter.promise);
            } catch (Failure failure) {
                if (waiter.active) {
                    // Отменённый участник ломает поколение (как в Perl).
                    waiter.active = false;
                    state.breakGeneration();
                }
                throw failure;
            }
        }, self);
    }

    /** Снимок состояния барьера: (участники, ожидающие, поколение, сломан). */
    static final class BarrierSnapshot {
        final int parties;
        final int waiters;
        final int generation;
        final boolean broken;

        BarrierSnapshot(BarrierState state) {
            parties = state.parties;
            waiters = state.active();
            generation = state.generation;
            broken = state.broken;
        }
    }

    public static Object barrier_Barrier_state(Object snapshotType, Object self) {
        return attach(make(snapshotType), new BarrierSnapshot(barrier(self)));
    }

    public static void barrier_Barrier_abort(Object self) {
        BarrierState state = barrier(self);
        if (state.brokenType == null) {
            state.brokenType = findSibling(self, "BrokenBarrier");
        }
        state.breakGeneration();
    }

    public static void barrier_Barrier_reset(Object self) {
        BarrierState state = barrier(self);
        if (state.active() > 0) {
            if (state.brokenType == null) {
                state.brokenType = findSibling(self, "BrokenBarrier");
            }
            state.breakGeneration();
        }
        state.generation++;
        state.broken = false;
        state.waiting.clear();
    }

    /** Класс объекта того же модуля по имени (для Error, нужного без вызова wait). */
    private static Object findSibling(Object self, String name) {
        String own = self.getClass().getName();
        int separator = own.lastIndexOf("$$");
        try {
            return Class.forName(own.substring(0, separator + 2) + name);
        } catch (ClassNotFoundException missing) {
            throw Rt.trap(name + " is not available");
        }
    }

    private static BarrierSnapshot snapshot(Object self) {
        return state(self, BarrierSnapshot.class, "Invalid barrier state");
    }

    public static Object barrier_BarrierState_parties(Object self) {
        return BigInteger.valueOf(snapshot(self).parties);
    }

    public static Object barrier_BarrierState_waiters(Object self) {
        return BigInteger.valueOf(snapshot(self).waiters);
    }

    public static Object barrier_BarrierState_generation(Object self) {
        return BigInteger.valueOf(snapshot(self).generation);
    }

    public static Object barrier_BarrierState_broken(Object self) {
        return Boolean.valueOf(snapshot(self).broken);
    }

    // ----- async/scope: FutureScope -----

    /**
     * {@code FutureScope.result()}: группа задач для {@code action}, тело,
     * затем ожидание группы; отказ тела отменяет группу и остаётся первичным.
     */
    public static Object scope_FutureScope_result(Object groupType, Object self) {
        Retained state = retained(self);
        if (state.result == null) {
            state.result = op(() -> {
                Object group = attach(make(groupType), new GroupState());
                Fn action = (Fn) field(self, "action");
                Object value = null;
                Failure body = null;
                try {
                    value = Scheduler.await((Future) action.call(new Object[] {group}));
                } catch (Failure failure) {
                    body = failure;
                }
                if (body != null) {
                    cancelGroup(group(group));
                }
                Failure waited = null;
                try {
                    Scheduler.await((Future) scope_FutureGroup_wait(group));
                } catch (Failure failure) {
                    waited = failure;
                }
                if (body != null) {
                    if (waited != null && waited != body) {
                        Rt.secondary(body, waited);
                    }
                    throw body;
                }
                if (waited != null) {
                    throw waited;
                }
                return value;
            }, self);
        }
        return state.result;
    }

    // ----- async/timeout: Within, Before -----

    /** Длительность ожидания: {@code limit} или {@code deadline.minus(now)}. */
    private interface DurationSource {
        Object duration(Object clock);
    }

    /**
     * {@code result()} ограниченного Future (как Perl): уже завершённый origin
     * выигрывает без часов; иначе гонка origin и {@code clock.wait}; проигравший
     * отменяется и дожидается; победа часов — {@code TimedOut}. Повторный
     * вызов возвращает тот же Future.
     */
    private static Future timeoutResult(final Object timedOut, final Object self, final DurationSource source,
            final Fn seconds, final Fn wait) {
        final Obj object = (Obj) self;
        if (object.intrinsic instanceof Future) {
            return (Future) object.intrinsic;
        }
        Future result = Future.cold(new Fn() {
            public Object call(Object[] args) {
                return raceTimeout(timedOut, self, source, seconds, wait);
            }

            public Object[] captures() {
                return new Object[] {self};
            }
        });
        object.intrinsic = result;
        return result;
    }

    private static void settleQuietly(Future future) {
        try {
            Scheduler.await(future);
        } catch (Failure ignored) {
            // Исход проигравшего разбирает вызывающий.
        }
    }

    private static Object raceTimeout(Object timedOut, Object self, DurationSource source, Fn seconds, Fn wait) {
        Future origin = (Future) field(self, "origin");
        Object clock = field(self, "clock");
        origin.observed = true;
        Scheduler.register(origin);
        Scheduler.start(origin, Scheduler.current);
        if (origin.state >= Future.DONE) {
            return origin.outcome();
        }
        Future timer = null;
        Failure timerFailure = null;
        try {
            Object duration = source.duration(clock);
            Rat amount = (Rat) seconds.call(new Object[] {duration});
            if (amount.compareTo(Rat.fromInt(BigInteger.ZERO)) > 0) {
                timer = (Future) wait.call(new Object[] {clock, duration});
                timer.observed = true;
                Scheduler.register(timer);
                Scheduler.start(timer, Scheduler.current);
            }
        } catch (Failure failure) {
            timerFailure = failure;
        }
        final Future first = Future.promise();
        final String[] winner = new String[1];
        origin.listen(new Runnable() {
            public void run() {
                if (winner[0] == null) {
                    winner[0] = "origin";
                    Scheduler.settle(first, null, null);
                }
            }
        });
        if (winner[0] == null) {
            if (timer == null) {
                winner[0] = "timer";
                Scheduler.settle(first, null, null);
            } else {
                timer.listen(new Runnable() {
                    public void run() {
                        if (winner[0] == null) {
                            winner[0] = "timer";
                            Scheduler.settle(first, null, null);
                        }
                    }
                });
            }
        }
        try {
            Scheduler.await(first);
        } catch (Failure cancelled) {
            if (!(cancelled.error instanceof Cancelled)) {
                throw cancelled;
            }
            winner[0] = "parent";
        }
        boolean originCancelled = false;
        boolean timerCancelled = false;
        if (!winner[0].equals("origin") && origin.state < Future.DONE) {
            originCancelled = true;
            Scheduler.cancel(origin);
        }
        if (!winner[0].equals("timer") && timer != null && timer.state < Future.DONE) {
            timerCancelled = true;
            Scheduler.cancel(timer);
        }
        settleQuietly(origin);
        if (timer != null) {
            settleQuietly(timer);
        }
        ArrayList<Failure> related = new ArrayList<Failure>();
        if (!winner[0].equals("origin") && origin.state == Future.FAILED
                && !(originCancelled && Future.cancelled(origin))) {
            related.add(origin.failure);
        }
        if (!winner[0].equals("timer") && timer != null && timer.state == Future.FAILED
                && !(timerCancelled && Future.cancelled(timer))) {
            related.add(timer.failure);
        }
        if (winner[0].equals("parent")) {
            throw Rt.thrown(new Cancelled());
        }
        Failure primary = null;
        if (winner[0].equals("origin")) {
            primary = origin.state == Future.FAILED ? origin.failure : null;
        } else {
            primary = timerFailure != null ? timerFailure
                : timer != null && timer.state == Future.FAILED ? timer.failure : null;
            if (primary == null) {
                primary = thrown(timedOut);
            }
        }
        if (primary != null) {
            throw primary;
        }
        if (!related.isEmpty()) {
            throw related.get(0);
        }
        return origin.value;
    }

    public static Object timeout_Within_result(Object timedOut, Object seconds, Object wait, final Object self) {
        return timeoutResult(timedOut, self, new DurationSource() {
            public Object duration(Object clock) {
                return field(self, "limit");
            }
        }, (Fn) seconds, (Fn) wait);
    }

    public static Object timeout_Before_result(Object timedOut, final Object now, final Object minus,
            Object seconds, Object wait, final Object self) {
        return timeoutResult(timedOut, self, new DurationSource() {
            public Object duration(Object clock) {
                Object instant = ((Fn) now).call(new Object[] {clock});
                return ((Fn) minus).call(new Object[] {field(self, "deadline"), instant});
            }
        }, (Fn) seconds, (Fn) wait);
    }
}
