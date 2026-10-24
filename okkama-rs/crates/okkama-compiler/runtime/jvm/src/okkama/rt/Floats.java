package okkama.rt;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.math.MathContext;
import java.math.RoundingMode;

/**
 * Операции {@code float} (OIR-1 §16.2): значения — {@link Double}, неконечный
 * результат и делитель 0 — программные ошибки с текстами Perl-интерпретатора.
 * Текст — правило Go {@code strconv.FormatFloat(x, 'g', -1, 64)} (D-032).
 */
public final class Floats {
    private Floats() {}

    private static final BigDecimal INT_MIN = new BigDecimal(Long.MIN_VALUE);
    private static final BigDecimal INT_MAX = new BigDecimal(Long.MAX_VALUE);

    private static Double finite(double value) {
        if (Double.isNaN(value) || Double.isInfinite(value)) {
            throw Rt.trap("Float value must contain a finite number");
        }
        return Double.valueOf(value);
    }

    public static Double add(Double left, Double right) {
        return finite(left.doubleValue() + right.doubleValue());
    }

    public static Double sub(Double left, Double right) {
        return finite(left.doubleValue() - right.doubleValue());
    }

    public static Double mul(Double left, Double right) {
        return finite(left.doubleValue() * right.doubleValue());
    }

    public static Double div(Double left, Double right) {
        if (right.doubleValue() == 0.0) {
            throw Rt.trap("Division by zero");
        }
        return finite(left.doubleValue() / right.doubleValue());
    }

    /** Остаток со знаком делимого, как {@code math.Mod} Go (D-031 для {@code %}). */
    public static Double mod(Double left, Double right) {
        double divisor = right.doubleValue();
        if (divisor == 0.0) {
            throw Rt.trap("Modulo by zero");
        }
        return finite(left.doubleValue() % divisor);
    }

    public static Double pow(Double base, Double exponent) {
        return finite(Math.pow(base.doubleValue(), exponent.doubleValue()));
    }

    public static Double neg(Double value) {
        return Double.valueOf(-value.doubleValue());
    }

    public static Double abs(Double value) {
        return Double.valueOf(Math.abs(value.doubleValue()));
    }

    public static Double sqrt(Double value) {
        if (value.doubleValue() < 0) {
            throw Rt.trap("Can't take sqrt of " + text(value));
        }
        return Double.valueOf(Math.sqrt(value.doubleValue()));
    }

    public static BigInteger sign(Double value) {
        double x = value.doubleValue();
        return BigInteger.valueOf(x > 0 ? 1 : x < 0 ? -1 : 0);
    }

    /** −1, 0 или 1 по IEEE-сравнению ({@code -0.0 == 0.0}). */
    public static int compare(Double left, Double right) {
        double a = left.doubleValue();
        double b = right.doubleValue();
        return a < b ? -1 : a > b ? 1 : 0;
    }

    public static Double fromInt(BigInteger value) {
        return finite(value.doubleValue());
    }

    public static Double fromByte(int value) {
        return Double.valueOf(value);
    }

    /** Усечение к нулю; результат — только signed-64, как в Perl. */
    public static BigInteger toInt(Double value) {
        BigDecimal exact = new BigDecimal(value.doubleValue()).setScale(0, RoundingMode.DOWN);
        if (exact.compareTo(INT_MIN) < 0 || exact.compareTo(INT_MAX) > 0) {
            throw Rt.trap("Cannot convert float value " + text(value) + " to int: expected signed-64 range");
        }
        return exact.toBigIntegerExact();
    }

    public static int toByte(Double value, String context) {
        double truncated = value.doubleValue() < 0 ? Math.ceil(value.doubleValue()) : Math.floor(value.doubleValue());
        if (truncated < 0 || truncated > 255) {
            String message = "Cannot convert float value " + text(value) + " to byte: expected 0..255";
            throw Rt.trap(context == null ? message : context + ": " + message);
        }
        return (int) truncated;
    }

    /** Кратчайшие цифры, экспонента при порядке {@code < -4} или {@code >= 6}. */
    public static String text(Double boxed) {
        double value = boxed.doubleValue();
        String sign = (value < 0 || (value == 0.0 && 1.0 / value < 0)) ? "-" : "";
        if (value == 0.0) {
            return sign + "0";
        }
        BigDecimal exact = new BigDecimal(Math.abs(value));
        BigDecimal shortest = exact;
        for (int precision = 1; precision <= 17; precision++) {
            BigDecimal rounded = exact.round(new MathContext(precision, RoundingMode.HALF_EVEN));
            if (rounded.doubleValue() == Math.abs(value)) {
                shortest = rounded;
                break;
            }
        }
        shortest = shortest.stripTrailingZeros();
        String digits = shortest.unscaledValue().toString();
        int exponent = digits.length() - 1 - shortest.scale();
        if (exponent < -4 || exponent >= 6) {
            String fraction = digits.length() > 1 ? "." + digits.substring(1) : "";
            String power = (exponent < 0 ? "-" : "+") + (Math.abs(exponent) < 10 ? "0" : "") + Math.abs(exponent);
            return sign + digits.charAt(0) + fraction + "e" + power;
        }
        if (exponent < 0) {
            StringBuilder zeros = new StringBuilder();
            for (int index = 0; index < -exponent - 1; index++) {
                zeros.append('0');
            }
            return sign + "0." + zeros + digits;
        }
        int point = exponent + 1;
        if (digits.length() <= point) {
            StringBuilder whole = new StringBuilder(digits);
            while (whole.length() < point) {
                whole.append('0');
            }
            return sign + whole;
        }
        return sign + digits.substring(0, point) + "." + digits.substring(point);
    }
}
