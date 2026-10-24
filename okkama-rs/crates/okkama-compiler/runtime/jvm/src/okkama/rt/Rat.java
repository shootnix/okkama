package okkama.rt;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.math.MathContext;

/**
 * Точное рациональное {@code rat} (OIR-1 §16.2): несократимая дробь с
 * положительным знаменателем. Текст — {@code n/d}, у целого — {@code n}.
 * Ошибки — тексты Perl-интерпретатора.
 */
public final class Rat implements Comparable<Rat> {
    private final BigInteger num;
    private final BigInteger den;

    private Rat(BigInteger num, BigInteger den) {
        this.num = num;
        this.den = den;
    }

    private static Rat normalized(BigInteger num, BigInteger den) {
        if (den.signum() < 0) {
            num = num.negate();
            den = den.negate();
        }
        BigInteger gcd = num.gcd(den);
        if (gcd.signum() != 0 && !gcd.equals(BigInteger.ONE)) {
            num = num.divide(gcd);
            den = den.divide(gcd);
        }
        if (num.signum() == 0) {
            den = BigInteger.ONE;
        }
        return new Rat(num, den);
    }

    /** {@code rat(n, d)} и {@code n / d} над целыми. */
    public static Rat make(BigInteger num, BigInteger den) {
        if (den.signum() == 0) {
            throw Rt.trap("Division by zero");
        }
        return normalized(num, den);
    }

    public static Rat add(Rat left, Rat right) {
        return normalized(
            left.num.multiply(right.den).add(right.num.multiply(left.den)),
            left.den.multiply(right.den));
    }

    public static Rat sub(Rat left, Rat right) {
        return normalized(
            left.num.multiply(right.den).subtract(right.num.multiply(left.den)),
            left.den.multiply(right.den));
    }

    public static Rat mul(Rat left, Rat right) {
        return normalized(left.num.multiply(right.num), left.den.multiply(right.den));
    }

    public static Rat div(Rat left, Rat right) {
        if (right.num.signum() == 0) {
            throw Rt.trap("Division by zero");
        }
        return normalized(left.num.multiply(right.den), left.den.multiply(right.num));
    }

    /** Остаток со знаком делимого: {@code a - b * trunc(a / b)} (D-031). */
    public static Rat mod(Rat left, Rat right) {
        if (right.num.signum() == 0) {
            throw Rt.trap("Modulo by zero");
        }
        BigInteger quotient = left.num.multiply(right.den).divide(left.den.multiply(right.num));
        return sub(left, mul(right, new Rat(quotient, BigInteger.ONE)));
    }

    public static Rat neg(Rat value) {
        return new Rat(value.num.negate(), value.den);
    }

    public static Rat abs(Rat value) {
        return new Rat(value.num.abs(), value.den);
    }

    public static int compare(Rat left, Rat right) {
        return left.compareTo(right);
    }

    public static BigInteger sign(Rat value) {
        return BigInteger.valueOf(value.num.signum());
    }

    public static String numerator(Rat value) {
        return value.num.toString();
    }

    public static String denominator(Rat value) {
        return value.den.toString();
    }

    public static String text(Rat value) {
        return value.toString();
    }

    public static Rat fromInt(BigInteger value) {
        return new Rat(value, BigInteger.ONE);
    }

    public static Rat fromByte(int value) {
        return new Rat(BigInteger.valueOf(value), BigInteger.ONE);
    }

    /** {@code float.rat()}: точная дробь канонической десятичной записи float. */
    public static Rat fromFloat(Double value) {
        BigDecimal exact = new BigDecimal(Floats.text(value).replace("e+", "e"));
        BigInteger unscaled = exact.unscaledValue();
        int scale = exact.scale();
        if (scale <= 0) {
            return new Rat(unscaled.multiply(BigInteger.TEN.pow(-scale)), BigInteger.ONE);
        }
        return normalized(unscaled, BigInteger.TEN.pow(scale));
    }

    /** Усечение к нулю. */
    public static BigInteger toInt(Rat value) {
        return value.num.divide(value.den);
    }

    public static Double toFloat(Rat value) {
        double result = new BigDecimal(value.num)
            .divide(new BigDecimal(value.den), MathContext.DECIMAL128)
            .doubleValue();
        if (Double.isNaN(result) || Double.isInfinite(result)) {
            throw Rt.trap("Cannot convert rat to float: expected a finite binary64 value");
        }
        return Double.valueOf(result);
    }

    /** Усечение к нулю, затем проверка 0..255; {@code context} — номер операнда {@code @byte()}. */
    public static int toByte(Rat value, String context) {
        BigInteger truncated = toInt(value);
        if (truncated.signum() < 0 || truncated.compareTo(BigInteger.valueOf(255)) > 0) {
            String message = "Cannot convert rat value " + value + " to byte: expected 0..255";
            throw Rt.trap(context == null ? message : context + ": " + message);
        }
        return truncated.intValue();
    }

    @Override
    public int compareTo(Rat other) {
        return num.multiply(other.den).compareTo(other.num.multiply(den));
    }

    @Override
    public boolean equals(Object other) {
        return other instanceof Rat && num.equals(((Rat) other).num) && den.equals(((Rat) other).den);
    }

    @Override
    public int hashCode() {
        return num.hashCode() * 31 + den.hashCode();
    }

    @Override
    public String toString() {
        return den.equals(BigInteger.ONE) ? num.toString() : num + "/" + den;
    }
}
