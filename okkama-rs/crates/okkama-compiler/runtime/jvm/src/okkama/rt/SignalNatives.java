package okkama.rt;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Callable;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.ThreadFactory;

/**
 * runtime/signal: ожидание сигналов процесса (TERM, INT, HUP), #80, как в
 * okkama-runtime-core (native/signal.rs). Обработчик sun.misc.Signal считает
 * приходы каждого сигнала; ожидающие ждут роста своих счётчиков в рабочих
 * потоках. Обработчик стоит, пока сигнал кто-то ждёт: когда ожидание кончилось
 * (сигнал пришёл или ожидание отменено), сигналу возвращается прежний
 * обработчик, и следующий завершает процесс как обычно.
 */
public final class SignalNatives {
    private SignalNatives() {}

    private static final String OPERATION = "runtime/signal@1::signal_wait/1";
    private static final String[] NAMES = {"TERM", "INT", "HUP"};
    private static final Object LOCK = new Object();
    /** Сколько раз пришёл каждый сигнал (по месту в NAMES). */
    private static final long[] ARRIVED = new long[NAMES.length];
    /** Прежние обработчики сигналов, которым поставлен свой. */
    private static final sun.misc.SignalHandler[] PRIOR = new sun.misc.SignalHandler[NAMES.length];
    private static final List<Waiter> WAITING = new ArrayList<Waiter>();
    private static final ExecutorService WORKERS = Executors.newCachedThreadPool(new ThreadFactory() {
        public Thread newThread(Runnable work) {
            Thread thread = new Thread(work, "okkama-signal");
            thread.setDaemon(true);
            return thread;
        }
    });

    private static final class Waiter {
        final int[] wanted;
        final long[] seen;

        Waiter(int[] wanted, long[] seen) {
            this.wanted = wanted;
            this.seen = seen;
        }

        boolean wants(int place) {
            for (int known : wanted) {
                if (known == place) {
                    return true;
                }
            }
            return false;
        }
    }

    private static final sun.misc.SignalHandler HANDLER = new sun.misc.SignalHandler() {
        public void handle(sun.misc.Signal signal) {
            int place = place(signal.getName());
            if (place < 0) {
                return;
            }
            synchronized (LOCK) {
                ARRIVED[place]++;
                LOCK.notifyAll();
            }
        }
    };

    private static int place(String name) {
        for (int place = 0; place < NAMES.length; place++) {
            if (NAMES[place].equals(name)) {
                return place;
            }
        }
        return -1;
    }

    private static Failure failure(String code, String message) {
        return NativeFailure.raise("core.signal", code, OPERATION, message);
    }

    /** Снять ожидание (повторный вызов ничего не делает); под LOCK. */
    private static void leave(Waiter waiter) {
        WAITING.remove(waiter);
        release();
    }

    /** Вернуть прежний обработчик сигналам, которых больше никто не ждёт; под LOCK. */
    private static void release() {
        for (int place = 0; place < NAMES.length; place++) {
            if (PRIOR[place] == null) {
                continue;
            }
            boolean wanted = false;
            for (Waiter waiter : WAITING) {
                wanted = wanted || waiter.wants(place);
            }
            if (!wanted) {
                sun.misc.Signal.handle(new sun.misc.Signal(NAMES[place]), PRIOR[place]);
                PRIOR[place] = null;
            }
        }
    }

    public static Future runtime_signal__signal_wait(List<?> names) {
        ArrayList<Integer> places = new ArrayList<Integer>();
        for (Object item : names) {
            int place = place((String) item);
            if (place < 0) {
                return Scheduler.resolved(null, failure("invalid_input", "Unknown signal name"));
            }
            if (!places.contains(place)) {
                places.add(place);
            }
        }
        if (places.isEmpty()) {
            return Scheduler.resolved(null, failure("invalid_input", "No signal to wait for"));
        }
        int[] wanted = new int[places.size()];
        for (int index = 0; index < wanted.length; index++) {
            wanted[index] = places.get(index);
        }
        final Waiter waiter;
        synchronized (LOCK) {
            for (int place : wanted) {
                if (PRIOR[place] != null) {
                    continue;
                }
                try {
                    PRIOR[place] = sun.misc.Signal.handle(new sun.misc.Signal(NAMES[place]), HANDLER);
                } catch (IllegalArgumentException refused) {
                    release();
                    return Scheduler.resolved(null, failure("unsupported", "Signals cannot be handled here"));
                }
            }
            waiter = new Waiter(wanted, ARRIVED.clone());
            WAITING.add(waiter);
        }
        final Future promise = Future.promise();
        final Scheduler.Op op = Scheduler.offload(WORKERS, new Callable<Object>() {
            public Object call() throws InterruptedException {
                synchronized (LOCK) {
                    while (WAITING.contains(waiter)) {
                        for (int place : waiter.wanted) {
                            if (ARRIVED[place] > waiter.seen[place]) {
                                leave(waiter);
                                return NAMES[place];
                            }
                        }
                        LOCK.wait();
                    }
                    return null;
                }
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                if (error != null) {
                    Scheduler.settle(promise, null, failure("invalid_state", "The signal wait failed"));
                } else if (value != null) {
                    Scheduler.settle(promise, value, null);
                }
            }
        });
        // Исход или отмена: ожидание снимается сразу, рабочий поток выходит.
        promise.listen(new Runnable() {
            public void run() {
                op.detach();
                synchronized (LOCK) {
                    leave(waiter);
                    LOCK.notifyAll();
                }
            }
        });
        return promise;
    }
}
