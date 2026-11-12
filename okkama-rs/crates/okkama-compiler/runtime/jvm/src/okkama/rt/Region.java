package okkama.rt;

import java.lang.ref.WeakReference;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.Iterator;
import java.util.List;
import java.util.ListIterator;
import java.util.Map;

/**
 * Регион владения лексической области (OIR-1 §7). Идентичности
 * регистрируются партиями в порядке создания; закрытие уничтожает их в
 * обратном порядке регистрации, поэтому родитель уничтожается раньше
 * своих детей. Каждая идентичность входит в destroy не более одного раза.
 */
public final class Region {
    private static final boolean TRACE = "1".equals(System.getenv("OKKAMA_TRACE_THROW"));

    /** Владелец уже уничтоженных идентичностей. */
    public static final Region DEAD = new Region();

    // Создаются при первой регистрации: регион есть у каждого вызова, а
    // большинство вызовов не владеет ни одной идентичностью.
    private ArrayList<Obj> entries;
    private IdentityHashMap<Obj, Boolean> known;
    /** Самый старый {@link Obj#serial} среди {@link #entries}. */
    private long oldest = Long.MAX_VALUE;

    private void remember(Obj obj) {
        if (known == null) {
            known = new IdentityHashMap<Obj, Boolean>();
            entries = new ArrayList<Obj>();
        }
        if (known.put(obj, Boolean.TRUE) == null) {
            entries.add(obj);
            oldest = Math.min(oldest, obj.serial);
        }
    }

    /** Шагов обхода под контейнером, ради которых его стоит запомнить. */
    private static final int SUMMARY_STEPS = 64;

    /** Запомненный контейнер и {@link Obj#newest()} в тот момент. */
    private static final class Summary extends WeakReference<Object> {
        final long serial;

        Summary(Object container, long serial) {
            super(container);
            this.serial = serial;
        }
    }

    /**
     * Контейнеры (списки, словари, Any, Fn), обход которых не встретил
     * свободных идентичностей (#9), по {@code identityHashCode}. Всё, что
     * контейнер достигает, не новее его сводки; освобождение идентичности не
     * новее {@link #summarized} сбрасывает все сводки.
     */
    private static final HashMap<Integer, ArrayList<Summary>> summaries = new HashMap<Integer, ArrayList<Summary>>();
    private static int summaryCount;
    private static int pruned;
    private static long summarized;

    private static void summarize(Object container) {
        long serial = Obj.newest();
        Integer key = System.identityHashCode(container);
        ArrayList<Summary> bucket = summaries.get(key);
        if (bucket == null) {
            bucket = new ArrayList<Summary>(1);
            summaries.put(key, bucket);
        }
        for (int i = 0; i < bucket.size(); i++) {
            if (bucket.get(i).get() == container) {
                bucket.set(i, new Summary(container, serial));
                return;
            }
        }
        bucket.add(new Summary(container, serial));
        summaryCount++;
        summarized = Math.max(summarized, serial);
        if (summaryCount > 2 * Math.max(pruned, 1024)) {
            summaryCount = 0;
            for (Iterator<ArrayList<Summary>> buckets = summaries.values().iterator(); buckets.hasNext();) {
                ArrayList<Summary> each = buckets.next();
                for (Iterator<Summary> it = each.iterator(); it.hasNext();) {
                    if (it.next().get() == null) {
                        it.remove();
                    }
                }
                if (each.isEmpty()) {
                    buckets.remove();
                } else {
                    summaryCount += each.size();
                }
            }
            pruned = summaryCount;
        }
    }

    /** Может ли обход (через {@code via} с самой старой записью {@code oldest}) пропустить контейнер? */
    private static boolean summaryHolds(Object container, Region via, long oldest) {
        ArrayList<Summary> bucket = summaries.get(System.identityHashCode(container));
        if (bucket == null) {
            return false;
        }
        for (Summary summary : bucket) {
            if (summary.get() == container) {
                return via == null || summary.serial < oldest;
            }
        }
        return false;
    }

    /** Свободной снова стала идентичность не новее сводки: сводки могут её достигать. */
    private static void freed(long serial) {
        if (serial > summarized) {
            return;
        }
        summaries.clear();
        summaryCount = 0;
        pruned = 0;
        summarized = 0;
    }

    /** Значение без идентичностей: обходить нечего. */
    private static boolean plain(Object value) {
        return !(value instanceof Obj || value instanceof List || value instanceof Map || value instanceof Any
            || value instanceof Future || value instanceof Fn);
    }

    /** Владеть свободными идентичностями, достижимыми из {@code value}. */
    public void adopt(Object value) {
        register(value, true);
    }

    /** Идентичности {@code value}, принадлежащие этому региону, — во владение {@code to}. */
    public void transfer(Region to, Object value) {
        if (entries == null || plain(value)) {
            return;
        }
        ArrayList<Obj> reachable = new ArrayList<Obj>();
        collect(value, reachable, new IdentityHashMap<Obj, Boolean>(), this);
        sortBySerial(reachable);
        for (Obj obj : reachable) {
            if (obj.owner != this) {
                continue;
            }
            obj.owner = to;
            to.remember(obj);
        }
    }

    /** Учитывать свободные идентичности без владения (временные значения). */
    public void track(Object value) {
        register(value, false);
    }

    private void register(Object value, boolean own) {
        if (plain(value)) {
            return;
        }
        ArrayList<Obj> fresh = new ArrayList<Obj>();
        collect(value, fresh, new IdentityHashMap<Obj, Boolean>(), null);
        sortBySerial(fresh);
        for (Obj obj : fresh) {
            if (own) {
                obj.owner = this;
            }
            remember(obj);
        }
    }

    /** Порядок создания; партии малы, вставки достаточно. */
    private static void sortBySerial(ArrayList<Obj> objs) {
        for (int i = 1; i < objs.size(); i++) {
            Obj current = objs.get(i);
            int j = i - 1;
            while (j >= 0 && objs.get(j).serial > current.serial) {
                objs.set(j + 1, objs.get(j));
                j--;
            }
            objs.set(j + 1, current);
        }
    }

    /** Контейнер, элементы которого обходит {@link #collect}. */
    private static final class Walked {
        final Object container;
        final int base;
        final int steps;
        final int events;

        Walked(Object container, int base, int steps, int events) {
            this.container = container;
            this.base = base;
            this.steps = steps;
            this.events = events;
        }
    }

    /**
     * Идентичности, достижимые из {@code value}: свободные, а при {@code via}
     * ещё и принадлежащие {@code via}. Чужие идентичности не обходятся. Без
     * рекурсии: глубина данных не растит стек.
     *
     * <p>Контейнеры неизменяемы, поэтому всё, что контейнер достигает, не новее
     * его. Контейнер, обход которого не встретил свободной идентичности,
     * запоминается (#9): следующие обходы его пропускают, пока не освободится
     * идентичность не новее сводки, а обход через {@code via} — пока все
     * записи {@code via} новее её.
     */
    private static void collect(Object value, List<Obj> out, IdentityHashMap<Obj, Boolean> seen, Region via) {
        ArrayList<Object> pending = new ArrayList<Object>();
        ArrayList<Walked> open = new ArrayList<Walked>();
        pending.add(value);
        boolean any = !summaries.isEmpty();
        long oldest = via == null ? Long.MAX_VALUE : via.oldest;
        // events — то, что мешает сводке: свободная идентичность или
        // идентичность via, Future (его исход меняется).
        int steps = 0;
        int events = 0;
        while (true) {
            while (!open.isEmpty() && pending.size() <= open.get(open.size() - 1).base) {
                Walked walked = open.remove(open.size() - 1);
                if (walked.events == events && steps - walked.steps >= SUMMARY_STEPS) {
                    summarize(walked.container);
                }
            }
            if (pending.isEmpty()) {
                break;
            }
            Object item = pending.remove(pending.size() - 1);
            steps++;
            int base = pending.size();
            if (item instanceof Obj) {
                Obj obj = (Obj) item;
                if (!(obj.owner == null || (via != null && obj.owner == via))) {
                    continue;
                }
                events++;
                if (seen.containsKey(obj)) {
                    continue;
                }
                seen.put(obj, Boolean.TRUE);
                out.add(obj);
                ArrayList<Object> refs = new ArrayList<Object>();
                obj.refs(refs);
                pushReversed(pending, refs);
            } else if (item instanceof List || item instanceof Map || item instanceof Any || item instanceof Fn) {
                if (any && summaryHolds(item, via, oldest)) {
                    continue;
                }
                if (item instanceof List) {
                    pushReversed(pending, (List<?>) item);
                } else if (item instanceof Map) {
                    pushReversed(pending, new ArrayList<Object>(((Map<?, ?>) item).values()));
                } else if (item instanceof Any) {
                    pending.add(((Any) item).value);
                } else {
                    pushReversed(pending, java.util.Arrays.asList(((Fn) item).captures()));
                }
                open.add(new Walked(item, base, steps, events));
            } else if (item instanceof Future) {
                events++;
                Future future = (Future) item;
                pending.add(future.value);
                if (future.retained != null) {
                    pushReversed(pending, java.util.Arrays.asList(future.retained));
                }
            }
        }
    }

    private static void pushReversed(ArrayList<Object> pending, List<?> items) {
        for (ListIterator<?> it = items.listIterator(items.size()); it.hasPrevious();) {
            pending.add(it.previous());
        }
    }

    /** Закрыть регион; идентичности, достижимые из {@code keep}, освобождаются. */
    public void leave(Object keep) {
        Failure primary = close(keep == null ? new Object[0] : new Object[] {keep}, null, false);
        if (primary != null) {
            throw primary;
        }
    }

    /** Закрыть регион на пути отказа: Error-объекты отказа освобождаются. */
    public Failure unwind(Failure failure) {
        ArrayList<Object> errors = new ArrayList<Object>();
        collectErrors(failure, errors);
        close(errors.toArray(), failure, false);
        return failure;
    }

    /** {@code region.settle} async-функции: {@code async destroy} ожидаются (#154). */
    public void settle(Object keep) {
        Failure primary = close(keep == null ? new Object[0] : new Object[] {keep}, null, true);
        if (primary != null) {
            throw primary;
        }
    }

    /** {@code region.settle_unwind}: то же на пути отказа. */
    public Failure settleUnwind(Failure failure) {
        ArrayList<Object> errors = new ArrayList<Object>();
        collectErrors(failure, errors);
        close(errors.toArray(), failure, true);
        return failure;
    }

    /** Each failure of the graph once: it may share links or even loop. */
    private static void collectErrors(Failure failure, List<Object> out) {
        IdentityHashMap<Failure, Boolean> seen = new IdentityHashMap<Failure, Boolean>();
        ArrayList<Failure> pending = new ArrayList<Failure>();
        pending.add(failure);
        while (!pending.isEmpty()) {
            Failure start = pending.remove(pending.size() - 1);
            for (Failure link = start; link != null; link = link.cause) {
                if (seen.put(link, Boolean.TRUE) != null) {
                    break;
                }
                if (link.error != null) {
                    out.add(link.error);
                }
                for (int index = link.secondary.size() - 1; index >= 0; index--) {
                    pending.add(link.secondary.get(index));
                }
            }
        }
    }

    /**
     * Уничтожить идентичности региона в обратном порядке регистрации. На пути
     * отказа ({@code active}) отказы destroy становятся вторичными; иначе
     * первый — основной, остальные — вторичные к нему.
     */
    private Failure close(Object[] keep, Failure active, boolean awaits) {
        if (entries == null || entries.isEmpty()) {
            return null;
        }
        IdentityHashMap<Obj, Boolean> kept = new IdentityHashMap<Obj, Boolean>();
        for (Object value : keep) {
            ArrayList<Obj> reachable = new ArrayList<Obj>();
            collect(value, reachable, new IdentityHashMap<Obj, Boolean>(), this);
            for (Obj obj : reachable) {
                kept.put(obj, Boolean.TRUE);
            }
        }
        Failure primary = null;
        for (int index = entries.size() - 1; index >= 0; index--) {
            Obj obj = entries.get(index);
            if (kept.containsKey(obj)) {
                if (obj.owner == this) {
                    obj.owner = null;
                    freed(obj.serial);
                }
                continue;
            }
            if (obj.owner != this && obj.owner != null) {
                continue; // принят другим владельцем или уже уничтожен
            }
            obj.owner = DEAD;
            if (TRACE) {
                StringBuilder text = new StringBuilder("okkama trace: destroy " + obj.getClass().getName());
                for (StackTraceElement frame : new Throwable().getStackTrace()) {
                    text.append("\n    at ").append(frame.getClassName()).append('.').append(frame.getMethodName());
                }
                System.err.println(text);
            }
            try {
                Future pending = obj.destroyAsync();
                if (pending == null) {
                    obj.destroyBody();
                } else if (awaits) {
                    Scheduler.awaitShielded(pending);
                } else {
                    throw Rt.trap("an object with an async destroy was released in a synchronous scope");
                }
            } catch (Failure failure) {
                if (active != null) {
                    Rt.secondary(active, failure);
                } else if (primary == null) {
                    primary = failure;
                } else {
                    Rt.secondary(primary, failure);
                }
            }
        }
        entries.clear();
        known.clear();
        oldest = Long.MAX_VALUE;
        return primary;
    }
}
