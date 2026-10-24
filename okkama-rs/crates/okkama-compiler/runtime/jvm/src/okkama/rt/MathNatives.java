package okkama.rt;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.math.MathContext;
import java.math.RoundingMode;
import java.util.ArrayList;
import java.util.List;

/**
 * Числовые natives: {@code runtime/format::sprintf_values} (printf Perl),
 * {@code runtime/statistics}, генератор PCG {@code runtime/random} и
 * {@code runtime/random_distribution}. Тексты отказов — как у Perl-provider.
 */
public final class MathNatives {
    private MathNatives() {}

    private static Failure fail(String domain, String code, String symbol, String message) {
        return NativeFailure.raise(domain, code, symbol, message);
    }

    // ---- runtime/format::sprintf_values ----

    private static final String SPRINTF = "runtime/format@1::sprintf_values/2";

    private static Failure formatFailure(String message) {
        return fail("core.format", "invalid_input", SPRINTF, message);
    }

    private static Object unbox(Object value) {
        return value instanceof Any ? unbox(((Any) value).value) : value;
    }

    private static String quoted(String value, char quote) {
        return quote + value.replace("\\", "\\\\").replace("\n", "\\n").replace("\t", "\\t").replace("\r", "\\r")
            .replace(String.valueOf(quote), "\\" + quote) + quote;
    }

    /** Исходный вид значения ({@code _source} Perl-provider). */
    static String source(Object input) {
        Object value = unbox(input);
        if (value instanceof BigInteger) {
            return value.toString();
        }
        if (value instanceof Double) {
            String text = MiscNatives.formatG((Double) value, 15);
            return text.matches(".*[.eE].*") ? text : text + ".0";
        }
        if (value instanceof Rat) {
            return Rat.text((Rat) value);
        }
        if (value instanceof Boolean) {
            return ((Boolean) value) ? "true" : "false";
        }
        if (value instanceof String) {
            return quoted((String) value, '"');
        }
        if (value instanceof Integer) {
            return quoted(new String(Character.toChars((Integer) value)), '\'');
        }
        if (value instanceof Byte) {
            return "byte(" + (((Byte) value) & 0xff) + ")";
        }
        if (value instanceof Case) {
            return ((Case) value).type + "." + ((Case) value).name;
        }
        if (value instanceof List) {
            StringBuilder out = new StringBuilder("[");
            boolean first = true;
            for (Object item : (List<?>) value) {
                out.append(first ? "" : ", ").append(source(item));
                first = false;
            }
            return out.append(']').toString();
        }
        if (value instanceof java.util.Map) {
            StringBuilder out = new StringBuilder("[");
            boolean first = true;
            for (java.util.Map.Entry<?, ?> entry : ((java.util.Map<?, ?>) value).entrySet()) {
                out.append(first ? "" : ", ").append(source(entry.getKey())).append(": ").append(source(entry.getValue()));
                first = false;
            }
            return out.append(']').toString();
        }
        throw Natives.died();
    }

    static String display(Object input) {
        Object value = unbox(input);
        if (value instanceof String) {
            return (String) value;
        }
        if (value instanceof Integer) {
            return new String(Character.toChars((Integer) value));
        }
        return source(value);
    }

    private static boolean flagsAllowed(String flags, String allowed) {
        for (char flag : flags.toCharArray()) {
            if (allowed.indexOf(flag) < 0) {
                return false;
            }
        }
        return true;
    }

    /** Ширина и выравнивание (код points), флаг {@code 0} — нули после знака. */
    private static String pad(String body, String flags, int width, boolean zero) {
        int length = body.codePointCount(0, body.length());
        if (length >= width) {
            return body;
        }
        StringBuilder fill = new StringBuilder();
        for (int index = length; index < width; index++) {
            fill.append(zero ? '0' : ' ');
        }
        if (flags.indexOf('-') >= 0) {
            return body + fill.toString().replace('0', ' ');
        }
        if (zero) {
            int signs = 0;
            while (signs < body.length() && "+- ".indexOf(body.charAt(signs)) >= 0) {
                signs++;
            }
            if (body.startsWith("0x", signs) || body.startsWith("0X", signs) || body.startsWith("0b", signs)
                    || body.startsWith("0B", signs)) {
                signs += 2;
            }
            return body.substring(0, signs) + fill + body.substring(signs);
        }
        return fill + body;
    }

    private static final BigInteger TWO_64 = BigInteger.ONE.shiftLeft(64);
    private static final BigInteger IV_MAX = BigInteger.valueOf(Long.MAX_VALUE);
    private static final BigInteger IV_MIN = BigInteger.valueOf(Long.MIN_VALUE);

    /** Целое значение аргумента как IV Perl (float усекается и насыщается). */
    private static BigInteger integer(Object value) {
        if (value instanceof BigInteger) {
            return (BigInteger) value;
        }
        if (value instanceof Byte) {
            return BigInteger.valueOf(((Byte) value) & 0xff);
        }
        double number = (Double) value;
        if (Double.isNaN(number)) {
            return BigInteger.ZERO;
        }
        BigInteger truncated = new BigDecimal(number).toBigInteger();
        if (truncated.compareTo(IV_MAX) > 0) {
            return number >= 1.8446744073709552E19 ? TWO_64.subtract(BigInteger.ONE) : truncated;
        }
        return truncated.compareTo(IV_MIN) < 0 ? IV_MIN : truncated;
    }

    private static String formatInteger(Object value, char conversion, String flags, int width, int precision) {
        BigInteger number = integer(value);
        boolean signed = conversion == 'd' || conversion == 'i';
        String sign = "";
        BigInteger magnitude = number;
        if (signed) {
            if (number.signum() < 0) {
                sign = "-";
                magnitude = number.negate();
            } else if (flags.indexOf('+') >= 0) {
                sign = "+";
            } else if (flags.indexOf(' ') >= 0) {
                sign = " ";
            }
        } else if (number.signum() < 0) {
            magnitude = number.add(TWO_64);
        }
        int radix = conversion == 'o' ? 8 : conversion == 'x' || conversion == 'X' ? 16 : conversion == 'b' ? 2 : 10;
        String digits = magnitude.toString(radix);
        if (conversion == 'X') {
            digits = digits.toUpperCase();
        }
        if (precision >= 0) {
            if (precision == 0 && magnitude.signum() == 0) {
                digits = "";
            }
            while (digits.length() < precision) {
                digits = "0" + digits;
            }
        }
        String prefix = "";
        if (flags.indexOf('#') >= 0 && magnitude.signum() != 0) {
            if (conversion == 'o' && !digits.startsWith("0")) {
                prefix = "0";
            } else if (conversion == 'x') {
                prefix = "0x";
            } else if (conversion == 'X') {
                prefix = "0X";
            } else if (conversion == 'b') {
                prefix = "0b";
            }
        }
        boolean zero = flags.indexOf('0') >= 0 && flags.indexOf('-') < 0 && precision < 0;
        return pad(sign + prefix + digits, flags, width, zero);
    }

    private static double number(Object value) {
        if (value instanceof BigInteger) {
            return ((BigInteger) value).doubleValue();
        }
        if (value instanceof Byte) {
            return ((Byte) value) & 0xff;
        }
        return (Double) value;
    }

    private static String exponent(int value) {
        String digits = Integer.toString(Math.abs(value));
        return (value < 0 ? "-" : "+") + (digits.length() < 2 ? "0" + digits : digits);
    }

    private static String fixed(BigDecimal magnitude, int precision, boolean alternate) {
        String text = magnitude.setScale(precision, RoundingMode.HALF_EVEN).toPlainString();
        return precision == 0 && alternate ? text + "." : text;
    }

    private static String scientific(BigDecimal magnitude, int precision, boolean alternate, boolean upper) {
        int exp;
        String digits;
        if (magnitude.signum() == 0) {
            exp = 0;
            StringBuilder zeros = new StringBuilder("0");
            for (int index = 0; index < precision; index++) {
                zeros.append('0');
            }
            digits = zeros.toString();
        } else {
            BigDecimal rounded = magnitude.round(new MathContext(precision + 1, RoundingMode.HALF_EVEN));
            exp = rounded.precision() - rounded.scale() - 1;
            StringBuilder text = new StringBuilder(rounded.unscaledValue().toString());
            while (text.length() < precision + 1) {
                text.append('0');
            }
            digits = text.substring(0, precision + 1);
        }
        String mantissa = digits.substring(0, 1) + (precision > 0 || alternate ? "." : "") + digits.substring(1);
        return mantissa + (upper ? "E" : "e") + exponent(exp);
    }

    private static String formatFloat(Object value, char conversion, String flags, int width, int precision) {
        double number = number(value);
        boolean upper = Character.isUpperCase(conversion);
        String sign = (number < 0 || (number == 0 && 1 / number < 0)) ? "-"
            : flags.indexOf('+') >= 0 ? "+" : flags.indexOf(' ') >= 0 ? " " : "";
        if (Double.isNaN(number) || Double.isInfinite(number)) {
            String text = Double.isNaN(number) ? "NaN" : "Inf";
            if (Double.isNaN(number)) {
                sign = flags.indexOf('+') >= 0 ? "+" : "";
            }
            return pad(sign + text, flags, width, false);
        }
        int digits = precision < 0 ? 6 : precision;
        boolean alternate = flags.indexOf('#') >= 0;
        BigDecimal magnitude = new BigDecimal(Math.abs(number));
        String body;
        char kind = Character.toLowerCase(conversion);
        if (kind == 'f') {
            body = fixed(magnitude, digits, alternate);
        } else if (kind == 'e') {
            body = scientific(magnitude, digits, alternate, upper);
        } else {
            int significant = digits == 0 ? 1 : digits;
            int exp;
            if (magnitude.signum() == 0) {
                exp = 0;
            } else {
                BigDecimal rounded = magnitude.round(new MathContext(significant, RoundingMode.HALF_EVEN));
                exp = rounded.precision() - rounded.scale() - 1;
            }
            if (exp < -4 || exp >= significant) {
                body = scientific(magnitude, significant - 1, alternate, upper);
                if (!alternate) {
                    int at = body.indexOf(upper ? 'E' : 'e');
                    String mantissa = body.substring(0, at);
                    if (mantissa.indexOf('.') >= 0) {
                        mantissa = mantissa.replaceAll("0+$", "").replaceAll("\\.$", "");
                    }
                    body = mantissa + body.substring(at);
                }
            } else {
                body = fixed(magnitude, Math.max(0, significant - 1 - exp), alternate);
                if (!alternate && body.indexOf('.') >= 0) {
                    body = body.replaceAll("0+$", "").replaceAll("\\.$", "");
                }
            }
        }
        boolean zero = flags.indexOf('0') >= 0 && flags.indexOf('-') < 0;
        return pad(sign + body, flags, width, zero);
    }

    public static String runtime_format__sprintf_values(String pattern, List<?> values) {
        int[] text = pattern.codePoints().toArray();
        StringBuilder result = new StringBuilder();
        int offset = 0;
        int argument = 0;
        while (offset < text.length) {
            int symbol = text[offset];
            if (symbol != '%') {
                result.appendCodePoint(symbol);
                offset++;
                continue;
            }
            if (offset + 1 < text.length && text[offset + 1] == '%') {
                result.append('%');
                offset += 2;
                continue;
            }
            offset++;
            StringBuilder flags = new StringBuilder();
            while (offset < text.length && "-+ 0#".indexOf(text[offset]) >= 0) {
                flags.append((char) text[offset++]);
            }
            StringBuilder width = new StringBuilder();
            while (offset < text.length && text[offset] >= '0' && text[offset] <= '9') {
                width.append((char) text[offset++]);
            }
            int precision = -1;
            if (offset < text.length && text[offset] == '.') {
                offset++;
                StringBuilder digits = new StringBuilder();
                while (offset < text.length && text[offset] >= '0' && text[offset] <= '9') {
                    digits.append((char) text[offset++]);
                }
                if (digits.length() == 0) {
                    throw formatFailure("Percent precision requires digits");
                }
                precision = Integer.parseInt(digits.toString());
            }
            if (offset >= text.length) {
                throw formatFailure("Unterminated percent conversion");
            }
            int conversion = text[offset++];
            if (conversion == '*' || conversion == '$') {
                throw formatFailure("Dynamic or positional percent fields are unsupported");
            }
            if ("scdiuoxXbfFeEgGr".indexOf(conversion) < 0) {
                throw formatFailure("Unsupported percent conversion " + new String(Character.toChars(conversion)));
            }
            String flagText = flags.toString();
            if (conversion == 'r') {
                if (!flagsAllowed(flagText, "-")) {
                    throw formatFailure("Percent r supports only the minus flag");
                }
                if (precision >= 0) {
                    throw formatFailure("Percent r does not support precision");
                }
            } else if ((conversion == 's' || conversion == 'c') && !flagsAllowed(flagText, "-")) {
                throw formatFailure("Percent " + (char) conversion + " supports only the minus flag");
            }
            if (argument >= values.size()) {
                throw formatFailure("Missing percent format value");
            }
            Object value = unbox(values.get(argument++));
            int fieldWidth = width.length() == 0 ? 0 : Integer.parseInt(width.toString());
            result.append(formatOne(value, (char) conversion, flagText, fieldWidth, precision));
        }
        return result.toString();
    }

    private static String formatOne(Object value, char conversion, String flags, int width, int precision) {
        boolean numeric = value instanceof BigInteger || value instanceof Double || value instanceof Byte;
        switch (conversion) {
            case 's': {
                String text = display(value);
                if (precision >= 0 && text.codePointCount(0, text.length()) > precision) {
                    text = text.substring(0, text.offsetByCodePoints(0, precision));
                }
                return pad(text, flags, width, false);
            }
            case 'r':
                if (!(value instanceof Rat)) {
                    throw formatFailure("Percent r requires rat");
                }
                return pad(Rat.text((Rat) value), flags, width, false);
            case 'c': {
                int point;
                if (value instanceof Integer) {
                    point = (Integer) value;
                } else if (value instanceof BigInteger) {
                    point = ((BigInteger) value).intValue();
                } else if (value instanceof Byte) {
                    point = ((Byte) value) & 0xff;
                } else {
                    throw formatFailure("Percent c requires char, int, or byte");
                }
                if (point < 0 || point > 0x10FFFF) {
                    point = 0xFFFD;
                }
                return pad(new String(Character.toChars(point)), flags, width, false);
            }
            case 'd': case 'i': case 'u': case 'o': case 'x': case 'X': case 'b':
                if (!numeric) {
                    throw formatFailure("Percent " + conversion + " requires int, float, or byte");
                }
                return formatInteger(value, conversion, flags, width, precision);
            default:
                if (!numeric) {
                    throw formatFailure("Percent " + conversion + " requires int, float, or byte");
                }
                return formatFloat(value, conversion, flags, width, precision);
        }
    }

    // ---- runtime/statistics ----

    private static double[] numbers(List<?> values) {
        double[] out = new double[values.size()];
        for (int index = 0; index < out.length; index++) {
            out[index] = (Double) values.get(index);
        }
        return out;
    }

    private static double sum(double[] values) {
        double sum = 0.0;
        double correction = 0.0;
        for (double value : values) {
            double revised = sum + value;
            correction += Math.abs(sum) >= Math.abs(value) ? (sum - revised) + value : (value - revised) + sum;
            sum = revised;
        }
        return sum + correction;
    }

    private static double mean(double[] values) {
        double[] fractions = new double[values.length];
        for (int index = 0; index < values.length; index++) {
            fractions[index] = values[index] / values.length;
        }
        return sum(fractions);
    }

    private static double[] sorted(double[] values) {
        double[] copy = values.clone();
        java.util.Arrays.sort(copy);
        return copy;
    }

    public static Double runtime_statistics__statistics_mean(List<?> values) {
        return mean(numbers(values));
    }

    public static Double runtime_statistics__statistics_geometric_mean(List<?> values) {
        double[] numbers = numbers(values);
        double[] logs = new double[numbers.length];
        for (int index = 0; index < numbers.length; index++) {
            if (numbers[index] == 0.0) {
                return 0.0;
            }
            logs[index] = Math.log(numbers[index]);
        }
        return Math.exp(mean(logs));
    }

    public static Double runtime_statistics__statistics_harmonic_mean(List<?> values) {
        double[] numbers = numbers(values);
        for (double number : numbers) {
            if (number == 0.0) {
                return 0.0;
            }
        }
        double smallest = sorted(numbers)[0];
        double[] ratios = new double[numbers.length];
        for (int index = 0; index < numbers.length; index++) {
            ratios[index] = smallest / numbers[index];
        }
        return smallest * numbers.length / sum(ratios);
    }

    public static Double runtime_statistics__statistics_median(List<?> values, String choice) {
        double[] ordered = sorted(numbers(values));
        int middle = ordered.length / 2;
        if (ordered.length % 2 == 1) {
            return ordered[middle];
        }
        switch (choice) {
            case "low": return ordered[middle - 1];
            case "high": return ordered[middle];
            case "middle": return ordered[middle - 1] / 2.0 + ordered[middle] / 2.0;
            default: throw Natives.died();
        }
    }

    private static double interpolate(double left, double right, long part, long whole) {
        double weight = (double) part / whole;
        return left * (1.0 - weight) + right * weight;
    }

    public static List<Object> runtime_statistics__statistics_quantiles(List<?> values, BigInteger countValue,
            String method) {
        double[] ordered = sorted(numbers(values));
        long count = countValue.longValue();
        ArrayList<Object> result = new ArrayList<Object>();
        if (method.equals("inclusive")) {
            long span = ordered.length - 1;
            for (long index = 1; index <= count - 1; index++) {
                long scaled = index * span;
                int position = (int) (scaled / count);
                result.add(interpolate(ordered[position], ordered[position + 1], scaled % count, count));
            }
            return result;
        }
        if (!method.equals("exclusive")) {
            throw Natives.died();
        }
        long span = ordered.length + 1;
        for (long index = 1; index <= count - 1; index++) {
            long scaled = index * span;
            int position = (int) (scaled / count);
            if (position < 1) {
                position = 1;
            }
            if (position > ordered.length - 1) {
                position = ordered.length - 1;
            }
            result.add(interpolate(ordered[position - 1], ordered[position], scaled % count, count));
        }
        return result;
    }

    public static Double runtime_statistics__statistics_variance(List<?> values, boolean sample) {
        double[] numbers = numbers(values);
        long count = 0;
        double mean = 0.0;
        double squares = 0.0;
        for (double value : numbers) {
            ++count;
            double delta = value - mean;
            mean += delta / count;
            squares += delta * (value - mean);
        }
        return squares / (sample ? count - 1 : count);
    }

    private static double[] paired(List<?> leftValues, List<?> rightValues) {
        double[] left = numbers(leftValues);
        double[] right = numbers(rightValues);
        long count = 0;
        double leftMean = 0.0;
        double rightMean = 0.0;
        double leftSquares = 0.0;
        double rightSquares = 0.0;
        double products = 0.0;
        for (int index = 0; index < left.length; index++) {
            ++count;
            double leftDelta = left[index] - leftMean;
            double rightDelta = right[index] - rightMean;
            leftMean += leftDelta / count;
            rightMean += rightDelta / count;
            leftSquares += leftDelta * (left[index] - leftMean);
            rightSquares += rightDelta * (right[index] - rightMean);
            products += leftDelta * (right[index] - rightMean);
        }
        return new double[] {count, leftMean, rightMean, leftSquares, rightSquares, products};
    }

    public static boolean runtime_statistics__statistics_paired_valid(List<?> left, List<?> right, String result) {
        double[] moments = paired(left, right);
        switch (result) {
            case "covariance": return true;
            case "correlation": return moments[3] > 0 && moments[4] > 0;
            case "slope":
            case "intercept": return moments[3] > 0;
            default: throw Natives.died();
        }
    }

    public static Double runtime_statistics__statistics_paired(List<?> left, List<?> right, String result) {
        double[] moments = paired(left, right);
        switch (result) {
            case "covariance": return moments[5] / (moments[0] - 1);
            case "correlation": return moments[5] / Math.sqrt(moments[3]) / Math.sqrt(moments[4]);
            case "slope": return moments[5] / moments[3];
            case "intercept": return moments[2] - moments[5] / moments[3] * moments[1];
            default: throw Natives.died();
        }
    }

    // ---- runtime/random: PCG XSH-RR ----

    static final class PcgState {
        long state;
        boolean closed;
    }

    private static final long MULTIPLIER = 6364136223846793005L;
    private static final long INCREMENT = 1442695040888963407L;

    private static long advance(PcgState pcg) {
        long old = pcg.state;
        pcg.state = old * MULTIPLIER + INCREMENT;
        int shifted = (int) (((old >>> 18) ^ old) >>> 27);
        int rotation = (int) (old >>> 59);
        return Integer.toUnsignedLong(Integer.rotateRight(shifted, rotation));
    }

    public static Object runtime_random__pcg_state(BigInteger seed) {
        PcgState pcg = new PcgState();
        advance(pcg);
        pcg.state += seed.mod(BigInteger.ONE.shiftLeft(64)).longValue();
        advance(pcg);
        return pcg;
    }

    public static BigInteger runtime_random__pcg_integer(Object handle, BigInteger start, BigInteger stop) {
        PcgState pcg = (PcgState) handle;
        if (start.compareTo(stop) >= 0) {
            throw fail("core.random", "invalid_input", "runtime/random@1::pcg_integer/3", "Random range must be non-empty");
        }
        BigInteger span = stop.subtract(start);
        BigInteger threshold = BigInteger.ONE.shiftLeft(64).mod(span);
        while (true) {
            BigInteger sample = BigInteger.valueOf(advance(pcg)).shiftLeft(32).add(BigInteger.valueOf(advance(pcg)));
            if (sample.compareTo(threshold) >= 0) {
                return start.add(sample.mod(span));
            }
        }
    }

    public static Double runtime_random__pcg_float(Object handle) {
        PcgState pcg = (PcgState) handle;
        long high = advance(pcg) >> 5;
        long low = advance(pcg) >> 6;
        return (high * 67108864.0 + low) / 9007199254740992.0;
    }

    public static List<Object> runtime_random__pcg_bytes(Object handle, BigInteger size) {
        PcgState pcg = (PcgState) handle;
        if (size.signum() < 0) {
            throw fail("core.random", "invalid_input", "runtime/random@1::pcg_bytes/2",
                "Random byte size must be non-negative");
        }
        int count = size.intValue();
        ArrayList<Object> out = new ArrayList<Object>(count);
        while (out.size() < count) {
            long word = advance(pcg);
            for (int shift = 0; shift < 32 && out.size() < count; shift += 8) {
                out.add(Byte.valueOf((byte) (word >> shift)));
            }
        }
        return out;
    }

    // ---- runtime/random_distribution ----

    private static final double TAU = 6.283185307179586476925286766559;

    public static Double runtime_random_distribution__distribution_uniform(Double start, Double stop, Double fraction) {
        double result = start * (1.0 - fraction) + stop * fraction;
        if (result < start) {
            result = start;
        }
        if (result >= stop) {
            result = Math.nextAfter(stop.doubleValue(), start.doubleValue());
        }
        return result;
    }

    public static Double runtime_random_distribution__distribution_log(Double value) {
        if (value <= 0.0) {
            throw Natives.died();
        }
        return Math.log(value);
    }

    public static Double runtime_random_distribution__distribution_exp(Double value) {
        return Math.exp(value);
    }

    public static Double runtime_random_distribution__distribution_sqrt(Double value) {
        if (value < 0.0) {
            throw Natives.died();
        }
        return Math.sqrt(value);
    }

    public static Double runtime_random_distribution__distribution_cos(Double value) {
        return Math.cos(value);
    }

    public static Double runtime_random_distribution__distribution_acos(Double value) {
        if (value < -1.0 || value > 1.0) {
            throw Natives.died();
        }
        return Math.acos(value);
    }

    public static Double runtime_random_distribution__distribution_power(Double base, Double exponent) {
        if (base < 0.0) {
            throw Natives.died();
        }
        return Math.pow(base, exponent);
    }

    public static Double runtime_random_distribution__distribution_angle(Double value) {
        double angle = value % TAU;
        if (angle < 0.0) {
            angle += TAU;
        }
        return angle;
    }
}
