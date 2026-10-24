package okkama.rt;

import java.math.BigInteger;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.text.Normalizer;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Natives {@code runtime/decimal}, {@code runtime/uuid} и
 * {@code runtime/unicode_normalization} — перенос providers
 * Perl-интерпретатора. Ожидаемые сбои — {@link NativeFailure}; прочие сбои
 * provider — программная ошибка {@code Native invocation "…" failed}.
 */
public final class ValueNatives {
    private ValueNatives() {}

    private static Failure failed(String module, String name, int arity) {
        return Rt.trap("Native invocation \"runtime/" + module + "@1::" + name + "/" + arity + "\" failed");
    }

    // ----- runtime/decimal -----

    private static final Pattern DECIMAL =
        Pattern.compile("(-?)([0-9]+)(?:\\.([0-9]+))?(?:[eE]([+-]?[0-9]+))?");
    private static final BigInteger INT_MIN = BigInteger.valueOf(Long.MIN_VALUE);
    private static final BigInteger INT_MAX = BigInteger.valueOf(Long.MAX_VALUE);

    /** Разбор {@code _components}: значащие цифры без нулей по краям и порядок. */
    private static final class Decimal {
        final String coefficient;
        final BigInteger exponent;
        final String digits;
        final int sign;

        Decimal(String coefficient, BigInteger exponent, String digits, int sign) {
            this.coefficient = coefficient;
            this.exponent = exponent;
            this.digits = digits;
            this.sign = sign;
        }
    }

    private static Decimal components(String source) {
        Matcher match = DECIMAL.matcher(source);
        if (!match.matches()) {
            return null;
        }
        String fraction = match.group(3) == null ? "" : match.group(3);
        String written = match.group(4) == null ? "0" : match.group(4);
        String digits = (match.group(2) + fraction).replaceFirst("^0+", "");
        if (digits.isEmpty()) {
            return new Decimal("0", BigInteger.ZERO, "0", 0);
        }
        BigInteger exponent = new BigInteger(written.startsWith("+") ? written.substring(1) : written)
            .subtract(BigInteger.valueOf(fraction.length()));
        int end = digits.length();
        while (digits.charAt(end - 1) == '0') {
            end--;
        }
        exponent = exponent.add(BigInteger.valueOf(digits.length() - end));
        digits = digits.substring(0, end);
        if (exponent.compareTo(INT_MIN) < 0 || exponent.compareTo(INT_MAX) > 0) {
            return null;
        }
        boolean negative = match.group(1).equals("-");
        return new Decimal((negative ? "-" : "") + digits, exponent, digits, negative ? -1 : 1);
    }

    private static Decimal decimal(String source, String name, int arity) {
        Decimal parts = components(source);
        if (parts == null) {
            throw failed("decimal", name, arity);
        }
        return parts;
    }

    public static boolean runtime_decimal__decimal_valid(String origin) {
        return components(origin) != null;
    }

    public static String runtime_decimal__decimal_canonical(String origin) {
        Decimal parts = decimal(origin, "decimal_canonical", 1);
        if (parts.sign == 0) {
            return "0";
        }
        StringBuilder mantissa = new StringBuilder();
        if (parts.sign < 0) {
            mantissa.append('-');
        }
        mantissa.append(parts.digits.charAt(0));
        if (parts.digits.length() > 1) {
            mantissa.append('.').append(parts.digits, 1, parts.digits.length());
        }
        BigInteger scientific = parts.exponent.add(BigInteger.valueOf(parts.digits.length() - 1));
        if (scientific.signum() != 0) {
            mantissa.append('e').append(scientific);
        }
        return mantissa.toString();
    }

    public static String runtime_decimal__decimal_coefficient(String origin) {
        return decimal(origin, "decimal_coefficient", 1).coefficient;
    }

    public static BigInteger runtime_decimal__decimal_exponent(String origin) {
        return decimal(origin, "decimal_exponent", 1).exponent;
    }

    public static BigInteger runtime_decimal__decimal_order(String leftOrigin, String rightOrigin) {
        Decimal left = decimal(leftOrigin, "decimal_order", 2);
        Decimal right = decimal(rightOrigin, "decimal_order", 2);
        if (left.sign != right.sign) {
            return BigInteger.valueOf(Integer.compare(left.sign, right.sign));
        }
        if (left.sign == 0) {
            return BigInteger.ZERO;
        }
        BigInteger leftMagnitude = BigInteger.valueOf(left.digits.length()).add(left.exponent);
        BigInteger rightMagnitude = BigInteger.valueOf(right.digits.length()).add(right.exponent);
        int comparison = leftMagnitude.compareTo(rightMagnitude);
        if (comparison == 0) {
            int width = Math.max(left.digits.length(), right.digits.length());
            comparison = Integer.signum(padded(left.digits, width).compareTo(padded(right.digits, width)));
        }
        return BigInteger.valueOf(left.sign < 0 ? -comparison : comparison);
    }

    private static String padded(String digits, int width) {
        StringBuilder out = new StringBuilder(digits);
        while (out.length() < width) {
            out.append('0');
        }
        return out.toString();
    }

    // ----- runtime/unicode_normalization -----

    public static String runtime_unicode_normalization__normalized_text(String origin, String form) {
        Normalizer.Form target;
        if ("NFC".equals(form)) {
            target = Normalizer.Form.NFC;
        } else if ("NFD".equals(form)) {
            target = Normalizer.Form.NFD;
        } else if ("NFKC".equals(form)) {
            target = Normalizer.Form.NFKC;
        } else if ("NFKD".equals(form)) {
            target = Normalizer.Form.NFKD;
        } else {
            throw failed("unicode_normalization", "normalized_text", 2);
        }
        return Normalizer.normalize(origin, target);
    }

    // ----- runtime/uuid -----

    private static final Pattern UUID = Pattern.compile(
        "[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}");

    private static Failure invalid(String name, int arity, String message) {
        return NativeFailure.raise("core.uuid", "invalid_input",
            "runtime/uuid@1::" + name + "/" + arity, message);
    }

    private static int[] octets(List<?> data) {
        int[] out = new int[data.size()];
        for (int index = 0; index < out.length; index++) {
            out[index] = ((Byte) data.get(index)) & 0xFF;
        }
        return out;
    }

    private static List<Object> bytes(int[] octets) {
        ArrayList<Object> out = new ArrayList<Object>(octets.length);
        for (int value : octets) {
            out.add(Byte.valueOf((byte) value));
        }
        return out;
    }

    private static int[] sixteen(List<?> data, String name, int arity) {
        if (data.size() != 16) {
            throw invalid(name, arity, "UUID binary value must contain sixteen bytes");
        }
        return octets(data);
    }

    public static boolean runtime_uuid__uuid_valid(String origin) {
        return UUID.matcher(origin).matches();
    }

    public static List<Object> runtime_uuid__uuid_bytes(String origin) {
        if (!UUID.matcher(origin).matches()) {
            throw invalid("uuid_bytes", 1, "UUID text is not canonical");
        }
        String hex = origin.replace("-", "");
        int[] out = new int[16];
        for (int index = 0; index < 16; index++) {
            out[index] = Integer.parseInt(hex.substring(2 * index, 2 * index + 2), 16);
        }
        return bytes(out);
    }

    public static String runtime_uuid__uuid_string(List<?> origin) {
        int[] octets = sixteen(origin, "uuid_string", 1);
        StringBuilder hex = new StringBuilder();
        for (int value : octets) {
            hex.append(String.format("%02x", value));
        }
        return hex.substring(0, 8) + "-" + hex.substring(8, 12) + "-" + hex.substring(12, 16)
            + "-" + hex.substring(16, 20) + "-" + hex.substring(20, 32);
    }

    public static BigInteger runtime_uuid__uuid_version(List<?> origin) {
        return BigInteger.valueOf(sixteen(origin, "uuid_version", 1)[6] >> 4);
    }

    public static String runtime_uuid__uuid_variant(List<?> origin) {
        int value = sixteen(origin, "uuid_variant", 1)[8];
        if ((value & 0x80) == 0x00) {
            return "reserved";
        }
        if ((value & 0xc0) == 0x80) {
            return "rfc";
        }
        if ((value & 0xe0) == 0xc0) {
            return "microsoft";
        }
        return "future";
    }

    private static List<Object> versioned(int[] octets, int version) {
        int[] result = octets.clone();
        result[6] = (result[6] & 0x0f) | (version << 4);
        result[8] = (result[8] & 0x3f) | 0x80;
        return bytes(result);
    }

    public static List<Object> runtime_uuid__uuid_versioned(List<?> origin, BigInteger version) {
        int[] octets = sixteen(origin, "uuid_versioned", 2);
        if (version.signum() < 0 || version.compareTo(BigInteger.valueOf(15)) > 0) {
            throw invalid("uuid_versioned", 2, "UUID version must be between zero and fifteen");
        }
        return versioned(octets, version.intValue());
    }

    public static List<Object> runtime_uuid__uuid_named(List<?> namespace, List<?> name, BigInteger version) {
        if (namespace.size() != 16) {
            throw invalid("uuid_named", 3, "UUID namespace must contain sixteen bytes");
        }
        boolean md5 = version.equals(BigInteger.valueOf(3));
        if (!md5 && !version.equals(BigInteger.valueOf(5))) {
            throw invalid("uuid_named", 3, "Named UUID version must be three or five");
        }
        try {
            MessageDigest digest = MessageDigest.getInstance(md5 ? "MD5" : "SHA-1");
            for (int value : octets(namespace)) {
                digest.update((byte) value);
            }
            for (int value : octets(name)) {
                digest.update((byte) value);
            }
            byte[] hash = digest.digest();
            int[] out = new int[16];
            for (int index = 0; index < 16; index++) {
                out[index] = hash[index] & 0xFF;
            }
            return versioned(out, version.intValue());
        } catch (NoSuchAlgorithmException missing) {
            throw failed("uuid", "uuid_named", 3);
        }
    }

    public static List<Object> runtime_uuid__uuid_custom(BigInteger high, BigInteger middle, BigInteger low) {
        if (high.signum() < 0 || high.compareTo(BigInteger.valueOf(281474976710655L)) > 0) {
            throw invalid("uuid_custom", 3, "UUID 8 high field is outside 48 bits");
        }
        if (middle.signum() < 0 || middle.compareTo(BigInteger.valueOf(4095)) > 0) {
            throw invalid("uuid_custom", 3, "UUID 8 middle field is outside 12 bits");
        }
        if (low.signum() < 0 || low.compareTo(BigInteger.valueOf(4611686018427387903L)) > 0) {
            throw invalid("uuid_custom", 3, "UUID 8 low field is outside 62 bits");
        }
        int[] out = new int[16];
        long highBits = high.longValue();
        for (int index = 0; index < 6; index++) {
            out[index] = (int) ((highBits >> (8 * (5 - index))) & 0xFF);
        }
        int middleBits = middle.intValue();
        out[6] = 0x80 | ((middleBits >> 8) & 0x0f);
        out[7] = middleBits & 0xff;
        long lowBits = low.longValue();
        for (int index = 0; index < 8; index++) {
            out[8 + index] = (int) ((lowBits >> (8 * (7 - index))) & 0xFF);
        }
        out[8] = (out[8] & 0x3f) | 0x80;
        return bytes(out);
    }
}
