package okkama.rt;

import java.util.ArrayList;

/**
 * Активный отказ Okkama (OIR-1 §6): вид, Error-объект типизированного отказа,
 * цепочка причин и вторичные отказы cleanup. Стек JVM запоминается при
 * создании — это цена только выброса; отчёт ({@link Report}) берёт из него
 * кадры методов программы и их места (#270).
 */
public final class Failure extends RuntimeException {
    private static final long serialVersionUID = 1L;

    public static final int THROWN = 0;
    public static final int PANIC = 1;
    public static final int PROGRAM = 2;

    public final int kind;
    public final Object error;
    public Failure cause;
    public final ArrayList<Failure> secondary = new ArrayList<Failure>();
    /**
     * Стеки задач, чей {@code await} перебросил этот отказ, по порядку: в
     * отчёте они идут после собственного стека, как у ядра (#270).
     */
    ArrayList<StackTraceElement[]> awaited;

    /** {@code await} перебрасывает отказ задачи в ожидающую: её кадры — дальше. */
    void awaitedBy(StackTraceElement[] frames) {
        if (awaited == null) {
            awaited = new ArrayList<StackTraceElement[]>();
        }
        // Повторный await одного упавшего Future в цикле не растит отчёт.
        if (awaited.size() < 64) {
            awaited.add(frames);
        }
    }

    /** {@code OKKAMA_TRACE_THROW=1}: каждый новый отказ печатается в stderr со стеком JVM. */
    private static final boolean TRACE = "1".equals(System.getenv("OKKAMA_TRACE_THROW"));

    public Failure(int kind, String message, Object error) {
        super(message, null, false, true);
        this.kind = kind;
        this.error = error;
        if (TRACE) {
            StringBuilder text = new StringBuilder("okkama trace: ");
            try {
                text.append(Rt.describe(this));
            } catch (RuntimeException unavailable) {
                text.append(kind).append(' ').append(message);
            }
            for (StackTraceElement frame : new Throwable().getStackTrace()) {
                if (frame.getClassName().startsWith("okkama.rt.Failure")) {
                    continue;
                }
                text.append("\n    at ").append(frame.getClassName()).append('.').append(frame.getMethodName());
            }
            System.err.println(text);
        }
    }
}
