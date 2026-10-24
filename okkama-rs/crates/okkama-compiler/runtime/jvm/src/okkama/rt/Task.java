package okkama.rt;

import java.util.concurrent.Semaphore;

/**
 * Задача планировщика (OIR-1 §13): поток, исполняющий тело одного Future.
 * Потоки не работают параллельно: управление передаётся строго по одному
 * ({@link Scheduler#transfer}), так что порядок исполнения детерминирован.
 */
final class Task {
    final Semaphore wake = new Semaphore(0);
    /** Кому вернуть управление, когда задача приостановится или завершится. */
    Task resumer;
    /** Future, исполняющий тело (у задачи запуска — {@code null}). */
    final Future future;
    /** Future, которого задача сейчас ждёт. */
    Future awaiting;
    /** Отмена доставляется в ближайшей точке приостановки. */
    boolean cancelPending;

    Task(Future future) {
        this.future = future;
    }
}
