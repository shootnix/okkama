package okkama.rt;

import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.List;
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

    private void remember(Obj obj) {
        if (known == null) {
            known = new IdentityHashMap<Obj, Boolean>();
            entries = new ArrayList<Obj>();
        }
        if (known.put(obj, Boolean.TRUE) == null) {
            entries.add(obj);
        }
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

    /**
     * Идентичности, достижимые из {@code value}: свободные, а при {@code via}
     * ещё и принадлежащие {@code via}. Чужие идентичности не обходятся.
     */
    private static void collect(Object value, List<Obj> out, IdentityHashMap<Obj, Boolean> seen, Region via) {
        if (value instanceof Obj) {
            Obj obj = (Obj) value;
            if (seen.containsKey(obj) || !(obj.owner == null || (via != null && obj.owner == via))) {
                return;
            }
            seen.put(obj, Boolean.TRUE);
            out.add(obj);
            ArrayList<Object> refs = new ArrayList<Object>();
            obj.refs(refs);
            for (Object ref : refs) {
                collect(ref, out, seen, via);
            }
        } else if (value instanceof List) {
            for (Object item : (List<?>) value) {
                collect(item, out, seen, via);
            }
        } else if (value instanceof Map) {
            for (Object item : ((Map<?, ?>) value).values()) {
                collect(item, out, seen, via);
            }
        } else if (value instanceof Any) {
            collect(((Any) value).value, out, seen, via);
        } else if (value instanceof Future) {
            Future future = (Future) value;
            if (future.retained != null) {
                for (Object item : future.retained) {
                    collect(item, out, seen, via);
                }
            }
            collect(future.value, out, seen, via);
        } else if (value instanceof Fn) {
            for (Object item : ((Fn) value).captures()) {
                collect(item, out, seen, via);
            }
        }
    }

    /** Закрыть регион; идентичности, достижимые из {@code keep}, освобождаются. */
    public void leave(Object keep) {
        Failure primary = close(keep == null ? new Object[0] : new Object[] {keep}, null);
        if (primary != null) {
            throw primary;
        }
    }

    /** Закрыть регион на пути отказа: Error-объекты отказа освобождаются. */
    public Failure unwind(Failure failure) {
        ArrayList<Object> errors = new ArrayList<Object>();
        collectErrors(failure, errors);
        close(errors.toArray(), failure);
        return failure;
    }

    private static void collectErrors(Failure failure, List<Object> out) {
        for (Failure link = failure; link != null; link = link.cause) {
            if (link.error != null) {
                out.add(link.error);
            }
            for (Failure secondary : link.secondary) {
                collectErrors(secondary, out);
            }
        }
    }

    /**
     * Уничтожить идентичности региона в обратном порядке регистрации. На пути
     * отказа ({@code active}) отказы destroy становятся вторичными; иначе
     * первый — основной, остальные — вторичные к нему.
     */
    private Failure close(Object[] keep, Failure active) {
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
                obj.destroyBody();
            } catch (Failure failure) {
                if (active != null) {
                    active.secondary.add(failure);
                } else if (primary == null) {
                    primary = failure;
                } else {
                    primary.secondary.add(failure);
                }
            }
        }
        entries.clear();
        known.clear();
        return primary;
    }
}
