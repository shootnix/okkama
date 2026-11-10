package okkama.rt;

import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.locks.LockSupport;

/**
 * Передача управления между потоками задач (#327): семафор, который перед
 * парковкой недолго крутится. Управление обычно возвращается через
 * микросекунды (короткий async-вызов, ответ канала), а парковка и
 * пробуждение потока стоят единицы микросекунд на каждую передачу. Порядок
 * передач не меняется: меняется только то, как поток ждёт своей очереди.
 */
final class Baton {
    /** Сколько ждать с прокруткой, прежде чем припарковаться. */
    private static final long SPIN_NANOS = 20_000;
    /**
     * Сколько потоков крутятся одновременно: передача управления обычно
     * идёт между двумя (задача и её носитель, отправитель и получатель),
     * остальные паркуются сразу. С тремя и больше восемь задач, разбуженных
     * одним Event, делили ядра с крутящимися соседями: +29…+61% (#327).
     */
    private static final int SPINNERS = 2;
    private static final AtomicInteger spinning = new AtomicInteger();

    private final AtomicInteger permits = new AtomicInteger();
    private volatile Thread parked;

    /** Разрешение одному ожидающему: как {@code Semaphore.release()}. */
    void release() {
        permits.incrementAndGet();
        Thread waiting = parked;
        if (waiting != null) {
            LockSupport.unpark(waiting);
        }
    }

    /** Взять разрешение, дождавшись его: как {@code acquireUninterruptibly()}. */
    void acquireUninterruptibly() {
        if (take()) {
            return;
        }
        if (spinning.incrementAndGet() <= SPINNERS) {
            try {
                long deadline = System.nanoTime() + SPIN_NANOS;
                int turns = 0;
                while (true) {
                    Thread.onSpinWait();
                    if (take()) {
                        return;
                    }
                    // Часы читаются реже, чем проверяется разрешение.
                    if ((++turns & 63) == 0 && System.nanoTime() - deadline > 0) {
                        break;
                    }
                }
            } finally {
                spinning.decrementAndGet();
            }
        } else {
            spinning.decrementAndGet();
        }
        parked = Thread.currentThread();
        // Разрешение, выданное после этой записи, будит парковку ниже;
        // выданное до неё видно повторной проверке.
        while (!take()) {
            LockSupport.park(this);
        }
        parked = null;
    }

    private boolean take() {
        int now = permits.get();
        return now > 0 && permits.compareAndSet(now, now - 1);
    }
}
