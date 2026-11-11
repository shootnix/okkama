package okkama.rt;

import java.lang.ref.WeakReference;
import java.util.AbstractMap;
import java.util.AbstractSet;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Iterator;
import java.util.Map;
import java.util.NoSuchElementException;
import java.util.Set;
import java.util.SortedMap;

/**
 * A map value made by a write that versions a large map (#336; Tomato's
 * #252–#254, the IR engine's #335): an immutable B+ tree in {@link KeyOrder}
 * whose versions share every node off the path a write copies. Builders and
 * literals stay {@code TreeMap}; a {@code PMap} is made only by
 * {@link #merge} and {@link #without}, so a call site sees two classes at
 * most. A map of at most {@link #SMALL} entries is copied as before.
 *
 * <p>A version of a {@code TreeMap} cannot turn persistent in place, as it does
 * on the other engines, because the program holds the {@code TreeMap}
 * itself: the runtime keeps the persistent form of the last one it converted,
 * so branches off one map share it. Nodes are immutable once published.
 */
final class PMap extends AbstractMap<Object, Object> {
    /** Maps of at most this many entries are copied (Tomato's {@code SMALL}). */
    static final int SMALL = 64;

    /** Persistent maps, unless {@code OKKAMA_MAPS=btree} (for A/B on one build). */
    static final boolean PERSISTENT = !"btree".equals(System.getenv("OKKAMA_MAPS"));

    /** Entries or children a node holds at most. */
    private static final int MAX = 16;
    /** Entries or children a node gets when a map is built at once. */
    private static final int BULK = 12;

    /** The last plain map converted, and its persistent form. */
    private static WeakReference<Map<?, ?>> lastSource;
    private static WeakReference<PMap> lastPersistent;

    /** A leaf ({@code kids == null}) or a branch, whose keys are the smallest of each child. */
    private static final class Node {
        final Object[] keys;
        final Object[] vals;
        final Node[] kids;

        Node(Object[] keys, Object[] vals, Node[] kids) {
            this.keys = keys;
            this.vals = vals;
            this.kids = kids;
        }
    }

    private final Node root;
    private final int size;

    private PMap(Node root, int size) {
        this.root = root;
        this.size = size;
    }

    @Override
    public int size() {
        return size;
    }

    @Override
    public Object get(Object key) {
        Node node = root;
        if (node == null) {
            return null;
        }
        while (node.kids != null) {
            node = node.kids[child(node.keys, key)];
        }
        int at = search(node.keys, key);
        return at >= 0 ? node.vals[at] : null;
    }

    @Override
    public boolean containsKey(Object key) {
        return get(key) != null;
    }

    private static int search(Object[] keys, Object key) {
        int lo = 0;
        int hi = keys.length - 1;
        while (lo <= hi) {
            int mid = (lo + hi) >>> 1;
            int order = KeyOrder.INSTANCE.compare(keys[mid], key);
            if (order < 0) {
                lo = mid + 1;
            } else if (order > 0) {
                hi = mid - 1;
            } else {
                return mid;
            }
        }
        return -(lo + 1);
    }

    /** The child of a branch whose range holds {@code key}. */
    private static int child(Object[] keys, Object key) {
        int at = search(keys, key);
        return at >= 0 ? at : Math.max(0, -at - 2);
    }

    // ----- writes: each copies one path -----

    /** The stored key stays and the value is replaced, as {@code TreeMap.put}. */
    private static final int VALUE = 0;
    /** The key is replaced and the stored value stays: a part left of a merge's base. */
    private static final int KEY = 1;

    private PMap with(Object key, Object value, int mode) {
        boolean[] added = new boolean[1];
        Node[] parts = root == null
            ? new Node[] {new Node(new Object[] {key}, new Object[] {value}, null)}
            : put(root, key, value, mode, added);
        if (root == null) {
            added[0] = true;
        }
        Node top = parts.length == 1 ? parts[0]
            : new Node(new Object[] {parts[0].keys[0], parts[1].keys[0]}, null, parts);
        return new PMap(top, added[0] ? size + 1 : size);
    }

    private static Node[] put(Node node, Object key, Object value, int mode, boolean[] added) {
        if (node.kids == null) {
            int at = search(node.keys, key);
            if (at >= 0) {
                Object[] keys = node.keys;
                Object[] vals = node.vals;
                if (mode == VALUE) {
                    vals = vals.clone();
                    vals[at] = value;
                } else {
                    keys = keys.clone();
                    keys[at] = key;
                }
                return new Node[] {new Node(keys, vals, null)};
            }
            added[0] = true;
            at = -at - 1;
            return split(insert(node.keys, at, key), insert(node.vals, at, value), null);
        }
        int at = child(node.keys, key);
        Node[] parts = put(node.kids[at], key, value, mode, added);
        Object[] keys = node.keys.clone();
        Node[] kids = node.kids.clone();
        kids[at] = parts[0];
        keys[at] = parts[0].keys[0];
        if (parts.length == 1) {
            return new Node[] {new Node(keys, null, kids)};
        }
        return split(insert(keys, at + 1, parts[1].keys[0]), null, insert(kids, at + 1, parts[1]));
    }

    /** A node of these keys, or two halves when it holds more than {@link #MAX}. */
    private static Node[] split(Object[] keys, Object[] vals, Node[] kids) {
        int n = keys.length;
        if (n <= MAX) {
            return new Node[] {new Node(keys, vals, kids)};
        }
        int half = n / 2;
        return new Node[] {
            new Node(Arrays.copyOfRange(keys, 0, half), vals == null ? null : Arrays.copyOfRange(vals, 0, half),
                kids == null ? null : Arrays.copyOfRange(kids, 0, half)),
            new Node(Arrays.copyOfRange(keys, half, n), vals == null ? null : Arrays.copyOfRange(vals, half, n),
                kids == null ? null : Arrays.copyOfRange(kids, half, n)),
        };
    }

    private PMap without(Object key) {
        if (root == null) {
            return this;
        }
        Node next = remove(root, key);
        if (next == root) {
            return this;
        }
        // A branch left with one child gives way to it.
        while (next != null && next.kids != null && next.kids.length == 1) {
            next = next.kids[0];
        }
        return new PMap(next, size - 1);
    }

    /**
     * The node without {@code key}: the same node when it is absent, null when
     * the node is left empty. Nodes are not merged when they shrink; a branch's
     * key may then be below its child's smallest, which every search still
     * finds its way through.
     */
    private static Node remove(Node node, Object key) {
        if (node.kids == null) {
            int at = search(node.keys, key);
            if (at < 0) {
                return node;
            }
            if (node.keys.length == 1) {
                return null;
            }
            return new Node(delete(node.keys, at), delete(node.vals, at), null);
        }
        int at = child(node.keys, key);
        Node kid = remove(node.kids[at], key);
        if (kid == node.kids[at]) {
            return node;
        }
        if (kid == null) {
            if (node.kids.length == 1) {
                return null;
            }
            return new Node(delete(node.keys, at), null, delete(node.kids, at));
        }
        Node[] kids = node.kids.clone();
        kids[at] = kid;
        return new Node(node.keys, null, kids);
    }

    private static <T> T[] insert(T[] items, int at, T item) {
        T[] out = Arrays.copyOf(items, items.length + 1);
        System.arraycopy(items, at, out, at + 1, items.length - at);
        out[at] = item;
        return out;
    }

    private static <T> T[] delete(T[] items, int at) {
        T[] out = Arrays.copyOf(items, items.length - 1);
        System.arraycopy(items, at + 1, out, at, items.length - at - 1);
        return out;
    }

    /** A map of entries already sorted by {@link KeyOrder}, built bottom-up. */
    private static PMap fromSorted(Map<?, ?> sorted) {
        int n = sorted.size();
        ArrayList<Node> level = new ArrayList<Node>((n + BULK - 1) / BULK);
        Iterator<? extends Map.Entry<?, ?>> entries = sorted.entrySet().iterator();
        while (entries.hasNext()) {
            int count = Math.min(BULK, n - level.size() * BULK);
            Object[] keys = new Object[count];
            Object[] vals = new Object[count];
            for (int i = 0; i < count; i++) {
                Map.Entry<?, ?> entry = entries.next();
                keys[i] = entry.getKey();
                vals[i] = entry.getValue();
            }
            level.add(new Node(keys, vals, null));
        }
        while (level.size() > 1) {
            ArrayList<Node> parents = new ArrayList<Node>((level.size() + BULK - 1) / BULK);
            for (int from = 0; from < level.size(); from += BULK) {
                int count = Math.min(BULK, level.size() - from);
                Node[] kids = level.subList(from, from + count).toArray(new Node[0]);
                Object[] keys = new Object[count];
                for (int i = 0; i < count; i++) {
                    keys[i] = kids[i].keys[0];
                }
                parents.add(new Node(keys, null, kids));
            }
            level = parents;
        }
        return new PMap(level.isEmpty() ? null : level.get(0), n);
    }

    /** The persistent form of a large map, or null for one that is not in {@link KeyOrder}. */
    private static PMap persistent(Map<?, ?> map) {
        if (map instanceof PMap pmap) {
            return pmap;
        }
        if (!(map instanceof SortedMap<?, ?> sorted) || sorted.comparator() != KeyOrder.INSTANCE) {
            return null;
        }
        if (lastSource != null && lastSource.get() == map) {
            PMap known = lastPersistent.get();
            if (known != null) {
                return known;
            }
        }
        PMap made = fromSorted(map);
        lastSource = new WeakReference<Map<?, ?>>(map);
        lastPersistent = new WeakReference<PMap>(made);
        return made;
    }

    /**
     * {@code a + b + ...}: a version of the largest part when it holds more
     * than {@link #SMALL} entries and the other parts add at most a sixteenth
     * of it, else null (a copy: {@code TreeMap.putAll} builds a sorted map in
     * one linear pass, which a path per entry does not beat). Parts right of it put
     * their values; parts left of it, right to left, add their keys and give
     * their key to an equal stored one, as {@code TreeMap.putAll} from the
     * left did.
     */
    static Map<Object, Object> merge(Object[] parts) {
        if (!PERSISTENT || parts.length < 2) {
            return null;
        }
        int base = 0;
        for (int i = 1; i < parts.length; i++) {
            if (((Map<?, ?>) parts[i]).size() > ((Map<?, ?>) parts[base]).size()) {
                base = i;
            }
        }
        int large = ((Map<?, ?>) parts[base]).size();
        int added = 0;
        for (int i = 0; i < parts.length; i++) {
            added += i == base ? 0 : ((Map<?, ?>) parts[i]).size();
        }
        if (large <= SMALL || added > large / 16) {
            return null;
        }
        PMap map = persistent((Map<?, ?>) parts[base]);
        if (map == null) {
            return null;
        }
        for (int i = base - 1; i >= 0; i--) {
            for (Map.Entry<?, ?> entry : ((Map<?, ?>) parts[i]).entrySet()) {
                map = map.with(entry.getKey(), entry.getValue(), KEY);
            }
        }
        for (int i = base + 1; i < parts.length; i++) {
            for (Map.Entry<?, ?> entry : ((Map<?, ?>) parts[i]).entrySet()) {
                map = map.with(entry.getKey(), entry.getValue(), VALUE);
            }
        }
        return map;
    }

    /** {@code map - a - ...}: a version without those keys when {@code map} is large and few go, else null. */
    static Map<Object, Object> without(Map<?, ?> source, Object[] removed) {
        if (!PERSISTENT || source.size() <= SMALL) {
            return null;
        }
        int gone = 0;
        for (Object part : removed) {
            gone += ((Map<?, ?>) part).size();
        }
        if (gone > source.size() / 16) {
            return null;
        }
        PMap map = persistent(source);
        if (map == null) {
            return null;
        }
        for (Object part : removed) {
            for (Object key : ((Map<?, ?>) part).keySet()) {
                map = map.without(key);
            }
        }
        return map;
    }

    // ----- reads in key order -----

    private static final int KEYS = 0;
    private static final int VALUES = 1;
    private static final int ENTRIES = 2;

    @Override
    @SuppressWarnings("unchecked")
    public Set<Map.Entry<Object, Object>> entrySet() {
        return new AbstractSet<Map.Entry<Object, Object>>() {
            @Override
            public int size() {
                return size;
            }

            @Override
            public Iterator<Map.Entry<Object, Object>> iterator() {
                return (Iterator<Map.Entry<Object, Object>>) (Iterator<?>) new Walk(root, ENTRIES);
            }
        };
    }

    /** Keys in order without an entry object each ({@code Rt.keys}, a merge's parts). */
    @Override
    public Set<Object> keySet() {
        return new AbstractSet<Object>() {
            @Override
            public int size() {
                return size;
            }

            @Override
            public Iterator<Object> iterator() {
                return new Walk(root, KEYS);
            }
        };
    }

    @Override
    public java.util.Collection<Object> values() {
        return new java.util.AbstractCollection<Object>() {
            @Override
            public int size() {
                return size;
            }

            @Override
            public Iterator<Object> iterator() {
                return new Walk(root, VALUES);
            }
        };
    }

    /** In-order walk: the leaf being read, and the branches above it with their next child. */
    private static final class Walk implements Iterator<Object> {
        private final Node[] branches = new Node[32];
        private final int[] next = new int[32];
        private int depth;
        private Node leaf;
        private int at;
        private final int kind;

        Walk(Node root, int kind) {
            this.kind = kind;
            if (root != null) {
                descend(root);
            }
        }

        private void descend(Node node) {
            while (node.kids != null) {
                branches[depth] = node;
                next[depth] = 1;
                depth++;
                node = node.kids[0];
            }
            leaf = node;
            at = 0;
        }

        @Override
        public boolean hasNext() {
            if (leaf == null) {
                return false;
            }
            while (at == leaf.keys.length) {
                // The leaf is done: the next child of the nearest branch that has one.
                while (depth > 0 && next[depth - 1] == branches[depth - 1].kids.length) {
                    depth--;
                }
                if (depth == 0) {
                    leaf = null;
                    return false;
                }
                Node kid = branches[depth - 1].kids[next[depth - 1]++];
                descend(kid);
            }
            return true;
        }

        @Override
        public Object next() {
            if (!hasNext()) {
                throw new NoSuchElementException();
            }
            int i = at++;
            return switch (kind) {
                case KEYS -> leaf.keys[i];
                case VALUES -> leaf.vals[i];
                default -> new AbstractMap.SimpleImmutableEntry<Object, Object>(leaf.keys[i], leaf.vals[i]);
            };
        }
    }
}
