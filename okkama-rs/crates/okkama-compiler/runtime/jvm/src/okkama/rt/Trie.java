package okkama.rt;

import java.util.ArrayList;
import java.util.Arrays;

/**
 * A persistent vector (#336; Tomato's #250, the IR engine's #335): a 32-way
 * trie of full leaves plus a tail of the last 1–32 elements (Bagwell 2000;
 * Hickey's {@code PersistentVector}). {@link #set} copies one path and shares
 * everything else, so every version stays as it was. Only the {@code Vec} of a
 * chain of {@code replaced} holds one.
 */
final class Trie {
    private static final int BITS = 5;
    private static final int WIDTH = 1 << BITS;
    private static final int MASK = WIDTH - 1;

    final int len;
    /** Bits to shift an index by at the root: {@code BITS} per level. */
    private final int shift;
    private final Object[] root;
    private final Object[] tail;

    private Trie(int len, int shift, Object[] root, Object[] tail) {
        this.len = len;
        this.shift = shift;
        this.root = root;
        this.tail = tail;
    }

    private int tailOffset() {
        return len == 0 ? 0 : ((len - 1) >>> BITS) << BITS;
    }

    /** A version of {@code len} elements of {@code items}, built bottom-up. */
    static Trie of(Object[] items, int from, int len) {
        int tailOffset = len == 0 ? 0 : ((len - 1) >>> BITS) << BITS;
        ArrayList<Object[]> level = new ArrayList<Object[]>(tailOffset / WIDTH);
        for (int at = 0; at < tailOffset; at += WIDTH) {
            level.add(Arrays.copyOfRange(items, from + at, from + at + WIDTH));
        }
        int shift = BITS;
        while (level.size() > WIDTH) {
            ArrayList<Object[]> parents = new ArrayList<Object[]>((level.size() + MASK) / WIDTH);
            for (int at = 0; at < level.size(); at += WIDTH) {
                parents.add(level.subList(at, Math.min(at + WIDTH, level.size())).toArray());
            }
            level = parents;
            shift += BITS;
        }
        Object[] root = level.toArray();
        return new Trie(len, shift, root, Arrays.copyOfRange(items, from + tailOffset, from + len));
    }

    Object get(int index) {
        int tailOffset = tailOffset();
        if (index >= tailOffset) {
            return tail[index - tailOffset];
        }
        Object[] node = root;
        for (int level = shift; level > 0; level -= BITS) {
            node = (Object[]) node[(index >>> level) & MASK];
        }
        return node[index & MASK];
    }

    /** A version with {@code index} set to {@code value}: one path copied. */
    Trie set(int index, Object value) {
        int tailOffset = tailOffset();
        if (index >= tailOffset) {
            Object[] copy = tail.clone();
            copy[index - tailOffset] = value;
            return new Trie(len, shift, root, copy);
        }
        return new Trie(len, shift, set(shift, root, index, value), tail);
    }

    private static Object[] set(int level, Object[] node, int index, Object value) {
        Object[] copy = node.clone();
        if (level == 0) {
            copy[index & MASK] = value;
        } else {
            int at = (index >>> level) & MASK;
            copy[at] = set(level - BITS, (Object[]) node[at], index, value);
        }
        return copy;
    }

    /** The elements in order: whole leaves at a time, then the tail. */
    Object[] toArray() {
        Object[] out = new Object[len];
        int tailOffset = tailOffset();
        int at = 0;
        // Leaves left to right by an explicit walk of the levels.
        ArrayList<Object[]> level = new ArrayList<Object[]>();
        level.add(root);
        for (int depth = shift; depth > 0; depth -= BITS) {
            ArrayList<Object[]> next = new ArrayList<Object[]>();
            for (Object[] node : level) {
                for (Object kid : node) {
                    next.add((Object[]) kid);
                }
            }
            level = next;
        }
        for (Object[] leaf : level) {
            if (at >= tailOffset) {
                break;
            }
            System.arraycopy(leaf, 0, out, at, WIDTH);
            at += WIDTH;
        }
        System.arraycopy(tail, 0, out, tailOffset, len - tailOffset);
        return out;
    }
}
