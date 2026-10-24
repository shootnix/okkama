package okkama.rt;

import java.math.BigInteger;

/**
 * Значение диапазона Okkama (OIR-1 §12.4): целые границы и флаг
 * {@code inclusive}. Равенство — {@code start}, {@code end}, {@code inclusive}.
 */
public final class Range {
    private final BigInteger start;
    private final BigInteger end;
    private final boolean inclusive;

    private Range(BigInteger start, BigInteger end, boolean inclusive) {
        this.start = start;
        this.end = end;
        this.inclusive = inclusive;
    }

    /** {@code range.new}: начало больше конца — программная ошибка. */
    public static Range make(BigInteger start, BigInteger end, boolean inclusive) {
        if (start.compareTo(end) > 0) {
            throw Rt.trap("Range start must be less than or equal to range end");
        }
        return new Range(start, end, inclusive);
    }

    public static BigInteger start(Range range) {
        return range.start;
    }

    /** Эксклюзивная верхняя граница: у {@code a..=b} это {@code b + 1}. */
    public static BigInteger end(Range range) {
        return range.inclusive ? range.end.add(BigInteger.ONE) : range.end;
    }

    @Override
    public boolean equals(Object other) {
        if (!(other instanceof Range)) {
            return false;
        }
        Range that = (Range) other;
        return start.equals(that.start) && end.equals(that.end) && inclusive == that.inclusive;
    }

    @Override
    public int hashCode() {
        return (start.hashCode() * 31 + end.hashCode()) * 2 + (inclusive ? 1 : 0);
    }
}
