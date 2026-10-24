package okkama.rt;

import java.util.List;

/**
 * Идентичность объекта Okkama для регионов владения (OIR-1 §7). Каждый класс
 * объекта программы наследует {@code Obj}: {@link #serial} — порядок
 * создания, {@link #owner} — регион-владелец ({@code null} — свободен,
 * {@link Region#DEAD} — уже уничтожен).
 */
public abstract class Obj {
    private static long created;

    public Region owner;
    public final long serial;
    /** Состояние рантайма объекта std с intrinsic-действиями (канал, Event, …). */
    public Object intrinsic;

    protected Obj() {
        serial = ++created;
    }

    /** Тело {@code destroy} объекта; без блока {@code destroy} — ничего. */
    public void destroyBody() {}

    /** Значения полей, которые могут достигать идентичностей. */
    public void refs(List<Object> out) {}
}
