package okkama.rt;

import java.util.ArrayList;
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

    /** {@link #serial} последней созданной идентичности. */
    static long newest() {
        return created;
    }

    /** Тело {@code destroy} объекта; без блока {@code destroy} — ничего. */
    public void destroyBody() {}

    /** Future блока {@code async destroy} (#154); null — блок синхронный или его нет. */
    public Future destroyAsync() {
        return null;
    }

    /** Значения полей, которые могут достигать идентичностей. */
    public void refs(List<Object> out) {}

    /** Имя объекта Okkama ({@code Box<int>}) для дампа; классы программы задают его. */
    public String okkamaName() {
        return getClass().getSimpleName();
    }

    /** Пары «имя поля, значение» по порядку объявления — для дампа. */
    public void okkamaFields(List<Object> out) {}

    /**
     * Поля равны полям {@code other} того же класса; задают классы объектов,
     * конструктор которых отдаёт {@code self} (они могут лежать на цикле).
     */
    public boolean okkamaEqualFields(Object other) {
        return this == other;
    }

    /** Хеш полей; пара к {@link #okkamaEqualFields}. */
    public int okkamaHashFields() {
        return System.identityHashCode(this);
    }

    /** Пары, которые сравниваются выше по стеку этого потока (#278). */
    private static final ThreadLocal<ArrayList<Object>> COMPARING = ThreadLocal.withInitial(ArrayList::new);
    /** Объекты, хеш которых считается выше по стеку этого потока. */
    private static final ThreadLocal<ArrayList<Object>> HASHING = ThreadLocal.withInitial(ArrayList::new);

    /**
     * Структурное равенство объекта, который может лежать на цикле (#278).
     * Пара, уже сравниваемая выше по стеку, считается равной: два графа равны,
     * если нигде не нашлось различающихся полей, как в IR-движке и ядре.
     */
    public static boolean cyclicEquals(Obj self, Object other) {
        ArrayList<Object> pairs = COMPARING.get();
        for (int index = 0; index < pairs.size(); index += 2) {
            if (pairs.get(index) == self && pairs.get(index + 1) == other) {
                return true;
            }
        }
        pairs.add(self);
        pairs.add(other);
        try {
            return self.okkamaEqualFields(other);
        } finally {
            pairs.remove(pairs.size() - 1);
            pairs.remove(pairs.size() - 1);
        }
    }

    /** Хеш объекта, который может лежать на цикле: повторный вход даёт 0. */
    public static int cyclicHash(Obj self) {
        ArrayList<Object> open = HASHING.get();
        for (Object entered : open) {
            if (entered == self) {
                return 0;
            }
        }
        open.add(self);
        try {
            return self.okkamaHashFields();
        } finally {
            open.remove(open.size() - 1);
        }
    }
}
