package okkama.rt;

import java.io.Serializable;
import java.math.BigInteger;
import java.util.Comparator;

/**
 * Естественный порядок ключей словаря (OIR-1 §12.3, решение R06): {@code int}
 * и {@code byte} по значению, {@code char} — по code point, {@code bool} —
 * {@code false < true}, enum — порядок объявления case, {@code str} — по Unicode
 * code points (а не по UTF-16, как {@link String#compareTo}). У одного словаря
 * все ключи одного статического типа.
 */
public final class KeyOrder implements Comparator<Object>, Serializable {
    private static final long serialVersionUID = 1L;

    public static final KeyOrder INSTANCE = new KeyOrder();

    private KeyOrder() {}

    @Override
    public int compare(Object left, Object right) {
        if (left instanceof String) {
            return compareCodePoints((String) left, (String) right);
        }
        if (left instanceof BigInteger) {
            return ((BigInteger) left).compareTo((BigInteger) right);
        }
        if (left instanceof Double) {
            return Floats.compare((Double) left, (Double) right);
        }
        if (left instanceof Case) {
            return Integer.compare(((Case) left).ordinal, ((Case) right).ordinal);
        }
        if (left instanceof Byte) {
            return Integer.compare(((Byte) left) & 0xFF, ((Byte) right) & 0xFF);
        }
        if (left instanceof Integer) {
            // char — по code point.
            return Integer.compare((Integer) left, (Integer) right);
        }
        return ((Boolean) left).compareTo((Boolean) right);
    }

    /** Порядок строк по code points: и ключи словаря, и {@code str.lt … ge}. */
    public static int compareCodePoints(String left, String right) {
        int i = 0;
        int j = 0;
        while (i < left.length() && j < right.length()) {
            int a = left.codePointAt(i);
            int b = right.codePointAt(j);
            if (a != b) {
                return a < b ? -1 : 1;
            }
            i += Character.charCount(a);
            j += Character.charCount(b);
        }
        if (i < left.length()) {
            return 1;
        }
        return j < right.length() ? -1 : 0;
    }
}
