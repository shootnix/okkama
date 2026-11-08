package okkama.rt;

import java.io.Serializable;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

/**
 * Естественный порядок ключей словаря (OIR-1 §12.3, решение R06): {@code int}
 * и {@code byte} по значению, {@code char} — по code point, {@code bool} —
 * {@code false < true}, enum — порядок объявления case, {@code str} — по Unicode
 * code points (а не по UTF-16, как {@link String#compareTo}), объект-значение
 * (#212) — по полям в порядке объявления, как {@code ==}. У одного словаря все
 * ключи одного статического типа, поэтому объекты-ключи одного класса.
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
        if (left instanceof Obj) {
            List<Object> a = new ArrayList<Object>();
            List<Object> b = new ArrayList<Object>();
            ((Obj) left).okkamaFields(a);
            ((Obj) right).okkamaFields(b);
            // Пары «имя, значение»: сравниваются значения.
            for (int i = 1; i < a.size() && i < b.size(); i += 2) {
                int order = compare(a.get(i), b.get(i));
                if (order != 0) {
                    return order;
                }
            }
            return Integer.compare(a.size(), b.size());
        }
        if (left instanceof Rat) {
            // Поле-дробь ключа-объекта.
            return ((Rat) left).compareTo((Rat) right);
        }
        if (left instanceof Boolean) {
            return ((Boolean) left).compareTo((Boolean) right);
        }
        // Массив или словарь под generic-ключом, прошедший мимо проверки (#242).
        throw Rt.trap("map key must be a scalar, enum, string or value object");
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
