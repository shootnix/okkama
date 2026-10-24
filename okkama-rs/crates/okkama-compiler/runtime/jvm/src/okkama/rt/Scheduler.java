package okkama.rt;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.concurrent.Callable;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.Semaphore;
import java.util.concurrent.ThreadFactory;

/**
 * Кооперативный планировщик (OIR-1 §13, §18 спецификации). Каждая задача —
 * поток, но исполняется ровно одна: {@link #transfer} отдаёт управление и
 * ждёт, пока его вернут. Это повторяет модель обратных вызовов
 * Perl-интерпретатора: запуск Future исполняет тело до первой приостановки
 * сразу; завершение Future сразу продолжает его ожидающих по порядку;
 * {@code spawn} запускает задачу немедленно. Внешние события (таймеры,
 * ввод-вывод) приходят в очередь цикла {@code async main}.
 */
public final class Scheduler {
    private Scheduler() {}

    private static final Object[] NO_ARGS = new Object[0];
    private static final long STACK = 256L << 20;

    /** Задача, у которой сейчас управление; изначально — поток запуска. */
    static Task current = new Task(null);
    /** Future, запущенные `spawn`: их исход проверяет завершение программы. */
    static final ArrayList<Future> registered = new ArrayList<Future>();
    /** События внешних операций; их число — {@link #external}. */
    static final LinkedBlockingQueue<Runnable> events = new LinkedBlockingQueue<Runnable>();
    /** Внешние операции, исхода которых ждёт программа. */
    static int external;
    /** Рабочие потоки блокирующих операций хоста (сеть, таймеры, SQLite). */
    private static final ExecutorService WORKERS = Executors.newCachedThreadPool(new ThreadFactory() {
        public Thread newThread(Runnable work) {
            Thread thread = new Thread(work, "okkama-io");
            thread.setDaemon(true);
            return thread;
        }
    });

    /**
     * Поток задач, переиспользуемый между телами Future: создание потока
     * стоит десятки микросекунд, а async-вызовы часто короткие. Свободные
     * носители лежат в {@link #idle}; к нему обращается только задача,
     * владеющая управлением, поэтому синхронизация — семафоры передачи.
     */
    private static final class Carrier implements Runnable {
        final Semaphore assigned = new Semaphore(0);
        Runnable work;

        public void run() {
            while (true) {
                assigned.acquireUninterruptibly();
                Runnable next = work;
                work = null;
                if (next == null) {
                    return;
                }
                next.run();
            }
        }
    }

    private static final ArrayDeque<Carrier> idle = new ArrayDeque<Carrier>();
    private static final int IDLE_CARRIERS = 64;

    /** Исполнить тело задачи на свободном носителе или новом потоке. */
    private static Carrier launch(final Runnable body) {
        Carrier carrier = idle.poll();
        carrier = carrier == null ? fresh() : carrier;
        carrier.work = body;
        carrier.assigned.release();
        return carrier;
    }

    private static Carrier fresh() {
        Carrier carrier = new Carrier();
        Thread thread = new Thread(null, carrier, "okkama-task", STACK);
        thread.setDaemon(true);
        thread.start();
        return carrier;
    }

    /** Носитель после тела: в запас, если там есть место (вызывает сам носитель). */
    private static void retire(Carrier carrier) {
        if (idle.size() < IDLE_CARRIERS) {
            idle.push(carrier);
        } else {
            carrier.work = null;
            carrier.assigned.release();
        }
    }

    /** Исход внешней операции; вызывается в цикле событий. */
    interface Outcome {
        void deliver(Object value, Throwable error);
    }

    /**
     * Внешняя операция: пока она {@link #attach() присоединена}, цикл
     * {@code async main} её ждёт; отсоединённая (её Future отменён) не держит
     * программу, но её исход всё равно доставляется.
     */
    static final class Op {
        private boolean attached;

        void attach() {
            if (!attached) {
                attached = true;
                external++;
            }
        }

        void detach() {
            if (attached) {
                attached = false;
                external--;
            }
        }
    }

    /** Исполнить {@code work} на рабочем потоке; исход — событием цикла. */
    static Op offload(final Callable<?> work, final Outcome done) {
        return offload(WORKERS, work, done);
    }

    /** То же на заданном исполнителе (например, поток соединения SQLite). */
    static Op offload(java.util.concurrent.Executor executor, final Callable<?> work, final Outcome done) {
        final Op op = new Op();
        op.attach();
        executor.execute(new Runnable() {
            public void run() {
                Object value = null;
                Throwable error = null;
                try {
                    value = work.call();
                } catch (Throwable failure) {
                    error = failure;
                }
                final Object result = value;
                final Throwable problem = error;
                events.add(new Runnable() {
                    public void run() {
                        op.detach();
                        done.deliver(result, problem);
                    }
                });
            }
        });
        return op;
    }

    /** Исход обещания, если его ещё не задала отмена. */
    static void settle(Future promise, Object value, Failure failure) {
        if (promise.state == Future.RUNNING) {
            complete(promise, value, failure);
        }
    }

    /** Завершённое обещание: исход async-native, известный сразу. */
    static Future resolved(Object value, Failure failure) {
        Future promise = Future.promise();
        complete(promise, value, failure);
        return promise;
    }

    static void register(Future future) {
        if (!registered.contains(future)) {
            registered.add(future);
        }
    }

    /** Отдать управление {@code target}; вернуться, когда оно его вернёт. */
    static void transfer(Task target) {
        Task me = current;
        target.resumer = me;
        current = target;
        target.wake.release();
        me.wake.acquireUninterruptibly();
        current = me;
    }

    /** Приостановить текущую задачу: управление — тому, кто её продолжил. */
    private static void suspend() {
        Task me = current;
        Task back = me.resumer;
        me.resumer = null;
        back.wake.release();
        me.wake.acquireUninterruptibly();
    }

    /** Запустить холодный Future: тело исполняется до первой приостановки. */
    static void start(Future future, Task starter) {
        if (future.state != Future.COLD) {
            return;
        }
        future.state = Future.RUNNING;
        future.starter = starter;
        final Task task = new Task(future);
        future.task = task;
        final Fn body = future.body;
        future.body = null;
        final Carrier[] self = new Carrier[1];
        Runnable run = new Runnable() {
            public void run() {
                task.wake.acquireUninterruptibly();
                try {
                    complete(task.future, body.call(NO_ARGS), null);
                } catch (Failure failure) {
                    complete(task.future, null, failure);
                } catch (StackOverflowError overflow) {
                    complete(task.future, null, Rt.trap("Stack overflow"));
                }
                Task back = task.resumer;
                task.resumer = null;
                // В запас до передачи управления: дальше носитель уже свободен.
                retire(self[0]);
                back.wake.release();
            }
        };
        // Тело ждёт task.wake, поэтому носитель известен ему до начала.
        self[0] = launch(run);
        transfer(task);
    }

    /** Исход Future; ожидающие продолжаются сразу, в порядке ожидания. */
    static void complete(Future future, Object value, Failure failure) {
        if (failure == null) {
            future.value = value;
            future.state = Future.DONE;
        } else {
            future.failure = failure;
            future.state = Future.FAILED;
        }
        while (!future.waiters.isEmpty()) {
            Object next = future.waiters.remove(0);
            if (next instanceof Task) {
                transfer((Task) next);
            } else {
                ((Runnable) next).run();
            }
        }
    }

    /** {@code await}: запуск холодного Future, затем ожидание исхода. */
    public static Object await(Future future) {
        future.observed = true;
        Task me = current;
        start(future, me);
        while (future.state == Future.RUNNING) {
            future.waiters.add(me);
            me.awaiting = future;
            suspend();
            me.awaiting = null;
            if (me.cancelPending) {
                me.cancelPending = false;
                future.waiters.remove(me);
                throw Rt.thrown(new Cancelled());
            }
        }
        return future.outcome();
    }

    /** {@code fut.cancel}: идемпотентная кооперативная отмена. */
    static void cancel(Future future) {
        if (future.state == Future.COLD) {
            future.body = null;
            future.failure = Rt.thrown(new Cancelled());
            future.state = Future.FAILED;
            return;
        }
        if (future.state != Future.RUNNING || future.cancelRequested) {
            return;
        }
        future.cancelRequested = true;
        Task task = future.task;
        if (task == null) {
            // Обещание рантайма: отмена завершает его отказом `Cancelled`.
            complete(future, null, Rt.thrown(new Cancelled()));
            return;
        }
        task.cancelPending = true;
        if (task == current || task.awaiting == null) {
            return;
        }
        Future inner = task.awaiting;
        if (inner.starter == task && inner.state == Future.RUNNING) {
            // Отмена идёт вглубь цепочки: внутренний завершится отменой и
            // продолжит эту задачу как ожидающую.
            cancel(inner);
            if (future.state != Future.RUNNING || task.awaiting == null) {
                return;
            }
        }
        inner.waiters.remove(task);
        transfer(task);
    }

    /**
     * {@code async main}: запуск корня и цикл внешних событий до его исхода,
     * затем завершение: незавершённые задачи `spawn` отменяются, ненаблюдённый
     * отказ задачи становится отказом программы (§18, D-034).
     */
    public static Object runMain(Future root) {
        root.observed = true;
        start(root, null);
        drive(root);
        Failure unobserved = shutdown();
        registered.clear();
        if (root.state == Future.FAILED) {
            throw root.failure;
        }
        if (unobserved != null) {
            throw unobserved;
        }
        return root.value;
    }

    /**
     * Тест (§14): async-тело — как корень, затем завершение задач `spawn`
     * теста; синхронное — только завершение. Каждый тест начинает со своего
     * списка задач.
     */
    public static void runTest(Future body) {
        registered.clear();
        if (body != null) {
            runMain(body);
            return;
        }
        Failure unobserved = shutdown();
        registered.clear();
        if (unobserved != null) {
            throw unobserved;
        }
    }

    /** Обрабатывать внешние события, пока {@code until} не завершён. */
    private static void drive(Future until) {
        while (until.state == Future.RUNNING) {
            if (external == 0) {
                throw Rt.trap("async main cannot complete: no task can make progress");
            }
            Runnable event;
            try {
                event = events.take();
            } catch (InterruptedException interrupted) {
                throw Rt.trap("async runtime was interrupted");
            }
            event.run();
        }
    }

    private static Failure shutdown() {
        for (Future future : new ArrayList<Future>(registered)) {
            if (future.state < Future.DONE) {
                cancel(future);
            }
        }
        for (Future future : new ArrayList<Future>(registered)) {
            while (future.state == Future.RUNNING && external > 0) {
                drive(future);
            }
        }
        for (Future future : registered) {
            if (!future.observed && future.state == Future.FAILED && !Future.cancelled(future)) {
                return future.failure;
            }
        }
        return null;
    }
}
