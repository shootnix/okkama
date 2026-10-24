package okkama.rt;

/** Системный Error {@code Cancelled} (OIR-1 §9): отмена Future. */
public final class Cancelled extends Obj implements Error {
    @Override
    public String text() {
        return "Operation was cancelled";
    }
}
