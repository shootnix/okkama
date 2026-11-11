package okkama.rt;

import java.util.AbstractList;
import java.util.Arrays;
import java.util.List;
import java.util.Objects;
import java.util.RandomAccess;

/**
 * An array value made by a chain of {@code +} (#336; Tomato's #246, #247 and
 * the IR engine's #335): a marked copy, or a version of a shared buffer. A
 * chain of appends or prepends writes into the buffer in place at its
 * frontier instead of copying the array at every step. A version sees only
 * its own slots, and a written slot never changes while the buffer lives, so
 * no version ever changes. Builders stay {@code ArrayList}: a {@code Vec} is
 * made only by {@link #concat}, so a call site sees two classes at most.
 *
 * <p>One Okkama task runs at a time, and a task moves between carrier threads
 * only through the scheduler's handoff (a happens-before edge), so the
 * frontier check and the write below need no lock; they stay in one method.
 */
final class Vec extends AbstractList<Object> implements RandomAccess {
    /** Arrays shorter than this are copied as before (Tomato's {@code CHAIN_MIN}). */
    static final int CHAIN_MIN = 32;

    /** Shared buffers, unless {@code OKKAMA_ARRAYS=flat} (for A/B on one build). */
    static final boolean SHARED = !"flat".equals(System.getenv("OKKAMA_ARRAYS"));

    /**
     * The last plain list a prepend was marked on. Such a list goes at an
     * offset of the copy through a temporary copy of itself, which pays for
     * the first step of a chain but not for every branch off one array: the
     * second prepend to the same list is a branch and copies plainly.
     */
    private static java.lang.ref.WeakReference<Object> prependedTo;

    private static final byte APPENDED = 1;
    private static final byte PREPENDED = 2;

    /** The written range {@code front .. back} of a shared buffer. */
    private static final class Frontier {
        int front;
        int back;

        Frontier(int front, int back) {
            this.front = front;
            this.back = back;
        }
    }

    private final Object[] slots;
    private final int start;
    private final int len;
    /** Null for a marked copy, which owns {@code slots} exactly. */
    private final Frontier frontier;
    /** How a marked copy was made; the next step of its chain starts a buffer. */
    private final byte chain;
    /**
     * A marked copy that has started a buffer: a second step off it is a
     * branch, which copies instead of starting another buffer of twice its
     * size. Representation only; no version sees it.
     */
    private boolean extended;

    private Vec(Object[] slots, int start, int len, Frontier frontier, byte chain) {
        this.slots = slots;
        this.start = start;
        this.len = len;
        this.frontier = frontier;
        this.chain = chain;
    }

    @Override
    public Object get(int index) {
        Objects.checkIndex(index, len);
        return slots[start + index];
    }

    @Override
    public int size() {
        return len;
    }

    @Override
    public Object[] toArray() {
        return Arrays.copyOfRange(slots, start, start + len);
    }

    private boolean continuesChain() {
        return frontier != null || chain != 0;
    }

    /**
     * {@code array.slice(from, to)} ({@code take}, {@code drop}): a view that
     * shares a {@code Vec}'s slots when it keeps at least half of them, so a
     * slice never holds much more memory than it shows; else a copy, made once
     * and itself a {@code Vec}, so that the next slices of it share. Null for a
     * short slice, copied as before.
     */
    static List<Object> slice(List<?> array, int from, int to) {
        int len = to - from;
        if (!SHARED || len < CHAIN_MIN) {
            return null;
        }
        if (array instanceof Vec vec) {
            if (2 * len >= vec.slots.length) {
                return new Vec(vec.slots, vec.start + from, len, null, (byte) 0);
            }
            return new Vec(Arrays.copyOfRange(vec.slots, vec.start + from, vec.start + to), 0, len, null, (byte) 0);
        }
        return new Vec(array.subList(from, to).toArray(), 0, len, null, (byte) 0);
    }

    /** {@code a + b + ...}: a buffer version or a marked copy, or null for a plain copy. */
    static List<Object> concat(Object[] parts, int size) {
        if (!SHARED || parts.length < 2 || size < CHAIN_MIN) {
            return null;
        }
        List<?> first = (List<?>) parts[0];
        List<?> last = (List<?>) parts[parts.length - 1];
        if (first instanceof Vec head && head.continuesChain() && step(head.len, size)) {
            if (head.frontier == null && !head.start()) {
                return marked(parts, size);
            }
            // The parts appended are copied first: one may be a version of
            // the same buffer.
            Object[] rest = flatten(parts, 1, parts.length, size - head.len);
            if (head.frontier != null) {
                Frontier frontier = head.frontier;
                if (head.start + head.len != frontier.back) {
                    return marked(parts, size);
                }
                if (frontier.back + rest.length <= head.slots.length) {
                    System.arraycopy(rest, 0, head.slots, frontier.back, rest.length);
                    frontier.back += rest.length;
                    return new Vec(head.slots, head.start, size, frontier, (byte) 0);
                }
            }
            int capacity = Integer.highestOneBit(Math.max(1, 2 * size - 1)) << 1;
            boolean both = head.chain == PREPENDED || (head.frontier != null && head.start > 0);
            int front = both ? (capacity - size) / 4 : 0;
            Object[] slots = new Object[capacity];
            System.arraycopy(head.slots, head.start, slots, front, head.len);
            System.arraycopy(rest, 0, slots, front + head.len, rest.length);
            return new Vec(slots, front, size, new Frontier(front, front + size), (byte) 0);
        }
        if (last instanceof Vec tail && tail.continuesChain() && step(tail.len, size)) {
            if (tail.frontier == null && !tail.start()) {
                return marked(parts, size);
            }
            Object[] rest = flatten(parts, 0, parts.length - 1, size - tail.len);
            if (tail.frontier != null) {
                Frontier frontier = tail.frontier;
                if (tail.start != frontier.front) {
                    return marked(parts, size);
                }
                if (rest.length <= frontier.front) {
                    int at = frontier.front - rest.length;
                    System.arraycopy(rest, 0, tail.slots, at, rest.length);
                    frontier.front = at;
                    return new Vec(tail.slots, at, size, frontier, (byte) 0);
                }
            }
            int capacity = Integer.highestOneBit(Math.max(1, 2 * size - 1)) << 1;
            // Three quarters of the room in front, where the chain grows.
            int front = (capacity - size) * 3 / 4;
            Object[] slots = new Object[capacity];
            System.arraycopy(rest, 0, slots, front, rest.length);
            System.arraycopy(tail.slots, tail.start, slots, front + rest.length, tail.len);
            return new Vec(slots, front, size, new Frontier(front, front + size), (byte) 0);
        }
        return marked(parts, size);
    }

    /**
     * A step of a chain: a large part and what is added to it, at most half
     * its size. Two halves joined, or four copies of one array, are not.
     */
    private static boolean step(int large, int size) {
        return large >= CHAIN_MIN && 2 * (size - large) <= large;
    }

    /** A marked copy starts one buffer; false when it already has. */
    private boolean start() {
        if (extended) {
            return false;
        }
        extended = true;
        return true;
    }

    /**
     * A copy, marked when it is a step of a chain (see {@link #step}); else
     * null, a plain copy by {@code ArrayList.addAll}, which copies a part
     * that is not first once where this copy would copy it twice.
     */
    private static List<Object> marked(Object[] parts, int size) {
        byte chain;
        int first = ((List<?>) parts[0]).size();
        int last = ((List<?>) parts[parts.length - 1]).size();
        if (step(first, size)) {
            chain = APPENDED;
        } else if (step(last, size)) {
            Object large = parts[parts.length - 1];
            if (!(large instanceof Vec)) {
                if (prependedTo != null && prependedTo.get() == large) {
                    return null;
                }
                prependedTo = new java.lang.ref.WeakReference<Object>(large);
            }
            chain = PREPENDED;
        } else {
            return null;
        }
        return new Vec(flatten(parts, 0, parts.length, size), 0, size, null, chain);
    }

    /** The elements of {@code parts[from .. to]} in a new array: each copied once. */
    private static Object[] flatten(Object[] parts, int from, int to, int size) {
        Object[] all = new Object[size];
        int at = 0;
        for (int i = from; i < to; i++) {
            at = copy((List<?>) parts[i], all, at);
        }
        return all;
    }

    private static int copy(List<?> part, Object[] all, int at) {
        int n = part.size();
        if (part instanceof Vec vec) {
            System.arraycopy(vec.slots, vec.start, all, at, n);
        } else if (at == 0) {
            // Straight into place (it may null the slot after it, which a
            // later part then fills).
            part.toArray(all);
        } else if (n <= CHAIN_MIN) {
            for (int i = 0; i < n; i++) {
                all[at + i] = part.get(i);
            }
        } else {
            System.arraycopy(part.toArray(), 0, all, at, n);
        }
        return at + n;
    }
}
