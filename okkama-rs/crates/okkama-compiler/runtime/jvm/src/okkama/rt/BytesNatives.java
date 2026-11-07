package okkama.rt;

import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.List;

/** {@code runtime/bytes}: views of a byte range ({@code encoding/bytes}), as on the Sparrow core. */
public final class BytesNatives {
    private BytesNatives() {}

    private static Failure failed(String name, int arity) {
        return Rt.trap("Native invocation \"runtime/bytes@1::" + name + "/" + arity + "\" failed");
    }

    private static int index(BigInteger value, int limit, String name, int arity) {
        if (value.signum() < 0 || value.bitLength() > 31 || value.intValue() > limit) throw failed(name, arity);
        return value.intValue();
    }

    /** The checked range [start, end) of {@code source}. */
    private static int[] range(List<?> source, BigInteger start, BigInteger end, String name, int arity) {
        int to = index(end, source.size(), name, arity);
        int from = index(start, to, name, arity);
        return new int[] {from, to};
    }

    private static byte at(List<?> bytes, int index) {
        return (Byte) bytes.get(index);
    }

    public static BigInteger runtime_bytes__bytes_find(List<?> source, List<?> needle, BigInteger start) {
        int from = index(start, source.size(), "bytes_find", 3);
        if (needle.isEmpty()) throw failed("bytes_find", 3);
        outer:
        for (int i = from; i + needle.size() <= source.size(); i++) {
            for (int j = 0; j < needle.size(); j++) if (at(source, i + j) != at(needle, j)) continue outer;
            return BigInteger.valueOf(i);
        }
        return BigInteger.valueOf(-1);
    }

    public static BigInteger runtime_bytes__bytes_utf8_invalid(List<?> source, BigInteger start, BigInteger end) {
        int[] r = range(source, start, end, "bytes_utf8_invalid", 3);
        return BigInteger.valueOf(utf8Invalid(Natives.toBytes(source.subList(r[0], r[1]))));
    }

    /** The offset of the first byte that is not part of valid UTF-8 (RFC 3629), or -1. */
    private static int utf8Invalid(byte[] bytes) {
        int i = 0;
        while (i < bytes.length) {
            int b = bytes[i] & 0xff;
            int width;
            int min;
            if (b < 0x80) { i++; continue; }
            else if (b >= 0xc2 && b <= 0xdf) { width = 2; min = 0x80; }
            else if (b >= 0xe0 && b <= 0xef) { width = 3; min = 0x800; }
            else if (b >= 0xf0 && b <= 0xf4) { width = 4; min = 0x10000; }
            else return i;
            if (i + width > bytes.length) return i;
            int code = b & (0xff >> (width + 1));
            for (int k = 1; k < width; k++) {
                int c = bytes[i + k] & 0xff;
                if ((c & 0xc0) != 0x80) return i;
                code = (code << 6) | (c & 0x3f);
            }
            if (code < min || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff)) return i;
            i += width;
        }
        return -1;
    }

    private static boolean decodes(byte[] bytes, int length) {
        try {
            StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(bytes, 0, length));
            return true;
        } catch (CharacterCodingException e) {
            return false;
        }
    }

    public static String runtime_bytes__bytes_text(List<?> source, BigInteger start, BigInteger end) {
        int[] r = range(source, start, end, "bytes_text", 3);
        byte[] bytes = Natives.toBytes(source.subList(r[0], r[1]));
        if (!decodes(bytes, bytes.length)) throw failed("bytes_text", 3);
        return new String(bytes, StandardCharsets.UTF_8);
    }

    /** ASCII decimal with an optional leading '-': the offset of the first byte that breaks it, or -1. */
    private static int decimalInvalid(List<?> digits) {
        int sign = !digits.isEmpty() && at(digits, 0) == '-' ? 1 : 0;
        for (int i = sign; i < digits.size(); i++) {
            byte b = at(digits, i);
            if (b < '0' || b > '9') return i;
        }
        return digits.size() == sign ? sign : -1;
    }

    public static BigInteger runtime_bytes__bytes_decimal_invalid(List<?> source, BigInteger start, BigInteger end) {
        int[] r = range(source, start, end, "bytes_decimal_invalid", 3);
        return BigInteger.valueOf(decimalInvalid(source.subList(r[0], r[1])));
    }

    public static BigInteger runtime_bytes__bytes_decimal(List<?> source, BigInteger start, BigInteger end) {
        int[] r = range(source, start, end, "bytes_decimal", 3);
        List<?> digits = source.subList(r[0], r[1]);
        if (decimalInvalid(digits) >= 0) throw failed("bytes_decimal", 3);
        return new BigInteger(new String(Natives.toBytes(digits), StandardCharsets.US_ASCII));
    }

    public static boolean runtime_bytes__bytes_equal(List<?> source, BigInteger start, BigInteger end, List<?> other) {
        int[] r = range(source, start, end, "bytes_equal", 4);
        return source.subList(r[0], r[1]).equals(other);
    }

    /** Fixed-width integer, two's complement or magnitude, as {@code encoding/binary} decodes it. */
    private static BigInteger fixed(List<?> source, BigInteger start, BigInteger end, boolean signed, boolean big, String name) {
        int[] r = range(source, start, end, name, 5);
        int length = r[1] - r[0];
        if (length == 0) return null;
        byte[] bytes = new byte[length];
        for (int i = 0; i < length; i++) bytes[big ? i : length - 1 - i] = at(source, r[0] + i);
        BigInteger n = signed ? new BigInteger(bytes) : new BigInteger(1, bytes);
        return n.bitLength() <= 63 ? n : null;
    }

    public static boolean runtime_bytes__bytes_integer_fits(List<?> source, BigInteger start, BigInteger end, boolean signed, boolean big) {
        return fixed(source, start, end, signed, big, "bytes_integer_fits") != null;
    }

    public static BigInteger runtime_bytes__bytes_integer(List<?> source, BigInteger start, BigInteger end, boolean signed, boolean big) {
        BigInteger n = fixed(source, start, end, signed, big, "bytes_integer");
        if (n == null) throw failed("bytes_integer", 5);
        return n;
    }

    public static List<Object> runtime_bytes__bytes_join(List<?> source, BigInteger start, List<?> tail) {
        int from = index(start, source.size(), "bytes_join", 3);
        byte[] head = Natives.toBytes(source.subList(from, source.size()));
        byte[] rest = Natives.toBytes(tail);
        byte[] joined = new byte[head.length + rest.length];
        System.arraycopy(head, 0, joined, 0, head.length);
        System.arraycopy(rest, 0, joined, head.length, rest.length);
        return Natives.fromBytes(joined, joined.length);
    }
}
