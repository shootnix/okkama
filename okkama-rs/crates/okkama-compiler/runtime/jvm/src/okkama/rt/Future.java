package okkama.rt;

import java.util.ArrayList;

/**
 * Future Okkama (OIR-1 §13): холодный до первого {@code await} или
 * {@code spawn}; исход (значение, отказ, отмена) кэшируется — повторный
 * {@code await} не повторяет тело.
 */
public final class Future {
    static final int COLD = 0;
    static final int RUNNING = 1;
    static final int DONE = 2;
    static final int FAILED = 3;

    int state = COLD;
    Object value;
    Failure failure;
    /** Тело холодного Future: {@code call(new Object[0])}. */
    Fn body;
    /** Захваты тела: через них регион видит владеемые объекты. */
    Object[] retained;
    /** Продолжения в порядке регистрации: ожидающие задачи ({@link Task}) и
     *  слушатели рантайма ({@link Runnable}), как обратные вызовы Perl. */
    final ArrayList<Object> waiters = new ArrayList<Object>();
    Task task;
    /** Задача, чей {@code await} запустил этот Future (отмена идёт вглубь). */
    Task starter;
    boolean observed;
    boolean cancelRequested;

    private Future() {}

    /** Вызов async-функции: тело — {@code Fn} без аргументов над захватами. */
    public static Future cold(Fn body) {
        Future future = new Future();
        future.body = body;
        future.retained = body.captures();
        return future;
    }

    /** Обещание: исход задаёт рантайм ({@link Scheduler#complete}). */
    static Future promise() {
        Future future = new Future();
        future.state = RUNNING;
        return future;
    }

    /** Слушатель исхода: сразу, если Future завершён, иначе при завершении. */
    void listen(Runnable listener) {
        if (state >= DONE) {
            listener.run();
        } else {
            waiters.add(listener);
        }
    }

    /** {@code fut.spawn}: запустить сейчас (до первой приостановки). */
    public static Future spawn(Future future) {
        Scheduler.register(future);
        Scheduler.start(future, null);
        return future;
    }

    public static void cancel(Future future) {
        Scheduler.cancel(future);
    }

    public static boolean done(Future future) {
        return future.state >= DONE;
    }

    /** Завершён отменой: отказ — {@code Cancelled}. */
    public static boolean cancelled(Future future) {
        return future.state == FAILED && future.failure.kind == Failure.THROWN
            && future.failure.error instanceof Cancelled;
    }

    Object outcome() {
        if (state == FAILED) {
            throw failure;
        }
        return value;
    }
}
