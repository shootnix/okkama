package okkama.rt;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.List;

/**
 * Natives {@code runtime/hex}, {@code runtime/base32}, {@code runtime/base64},
 * {@code runtime/base85} и {@code runtime/quoted_printable} — перенос
 * providers Perl-интерпретатора один к одному. Смещения в строках — в code
 * points. Сбой provider (неверный алфавит, недопустимый вход декодера) —
 * программная ошибка {@code Native invocation "…" failed}.
 */
public final class EncodingNatives {
    private EncodingNatives() {}

    private static final String BASE32 = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    private static final String BASE64_STANDARD =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    private static final String BASE64_URL =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    private static final String ASCII85;
    private static final String RFC1924 =
        "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{|}~";

    static {
        StringBuilder ascii85 = new StringBuilder();
        for (int code = 33; code <= 117; code++) {
            ascii85.append((char) code);
        }
        ASCII85 = ascii85.toString();
    }

    private static Failure failed(String module, String name, int arity) {
        return Rt.trap("Native invocation \"runtime/" + module + "@1::" + name + "/" + arity + "\" failed");
    }

    private static int[] points(String text) {
        return text.codePoints().toArray();
    }

    private static int index(String alphabet, int symbol) {
        return symbol > 0xFFFF ? -1 : alphabet.indexOf(symbol);
    }

    private static int octet(List<?> data, int index) {
        return ((Byte) data.get(index)) & 0xFF;
    }

    private static List<Object> bytes(List<Integer> octets) {
        ArrayList<Object> out = new ArrayList<Object>(octets.size());
        for (int value : octets) {
            out.add(Byte.valueOf((byte) value));
        }
        return out;
    }

    // ----- runtime/hex -----

    public static String runtime_hex__hex_encode(List<?> origin) {
        StringBuilder out = new StringBuilder();
        for (int index = 0; index < origin.size(); index++) {
            out.append(String.format("%02x", octet(origin, index)));
        }
        return out.toString();
    }

    public static BigInteger runtime_hex__hex_valid_prefix(String origin) {
        int[] source = points(origin);
        int offset = 0;
        while (offset < source.length && hexDigit(source[offset]) >= 0) {
            offset++;
        }
        return BigInteger.valueOf(offset);
    }

    public static List<Object> runtime_hex__hex_decode(String origin) {
        int[] source = points(origin);
        ArrayList<Integer> octets = new ArrayList<Integer>();
        for (int offset = 0; offset < source.length; offset += 2) {
            int high = hexDigit(source[offset]);
            int low = offset + 1 < source.length ? hexDigit(source[offset + 1]) : -1;
            if (high < 0 || low < 0) {
                throw failed("hex", "hex_decode", 1);
            }
            octets.add(high * 16 + low);
        }
        return bytes(octets);
    }

    private static int hexDigit(int symbol) {
        if (symbol >= '0' && symbol <= '9') {
            return symbol - '0';
        }
        if (symbol >= 'a' && symbol <= 'f') {
            return symbol - 'a' + 10;
        }
        if (symbol >= 'A' && symbol <= 'F') {
            return symbol - 'A' + 10;
        }
        return -1;
    }

    // ----- runtime/base32, runtime/base64 -----

    public static String runtime_base32__base32_encode(List<?> origin) {
        return encoded(origin, BASE32, 5, 8);
    }

    public static BigInteger runtime_base32__base32_invalid_offset(String origin) {
        return BigInteger.valueOf(shape(points(origin), BASE32, 5, 8)[0]);
    }

    public static List<Object> runtime_base32__base32_decode(String origin) {
        int[] source = points(origin);
        int[] shape = shape(source, BASE32, 5, 8);
        if (shape[0] != -1) {
            throw failed("base32", "base32_decode", 1);
        }
        return decoded(source, shape[1], BASE32, 5);
    }

    private static String base64Alphabet(String name, String native_) {
        if ("standard".equals(name)) {
            return BASE64_STANDARD;
        }
        if ("url".equals(name)) {
            return BASE64_URL;
        }
        throw failed("base64", native_, 2);
    }

    public static String runtime_base64__base64_encode(List<?> origin, String name) {
        return encoded(origin, base64Alphabet(name, "base64_encode"), 6, 4);
    }

    public static BigInteger runtime_base64__base64_invalid_offset(String origin, String name) {
        String alphabet = base64Alphabet(name, "base64_invalid_offset");
        return BigInteger.valueOf(shape(points(origin), alphabet, 6, 4)[0]);
    }

    public static List<Object> runtime_base64__base64_decode(String origin, String name) {
        String alphabet = base64Alphabet(name, "base64_decode");
        int[] source = points(origin);
        int[] shape = shape(source, alphabet, 6, 4);
        if (shape[0] != -1) {
            throw failed("base64", "base64_decode", 2);
        }
        return decoded(source, shape[1], alphabet, 6);
    }

    /** Символ на каждые {@code width} бит, затем {@code =} до кратности {@code quantum}. */
    private static String encoded(List<?> octets, String alphabet, int width, int quantum) {
        StringBuilder result = new StringBuilder();
        long buffer = 0;
        int bits = 0;
        int mask = (1 << width) - 1;
        for (int index = 0; index < octets.size(); index++) {
            buffer = (buffer << 8) | octet(octets, index);
            bits += 8;
            while (bits >= width) {
                bits -= width;
                result.append(alphabet.charAt((int) ((buffer >> bits) & mask)));
            }
        }
        if (bits > 0) {
            result.append(alphabet.charAt((int) ((buffer << (width - bits)) & mask)));
        }
        while (result.length() % quantum != 0) {
            result.append('=');
        }
        return result.toString();
    }

    /** {@code [первое неверное смещение или -1, конец данных]}, как {@code _shape} в Perl. */
    private static int[] shape(int[] source, String alphabet, int width, int quantum) {
        int size = source.length;
        if (size == 0) {
            return new int[] {-1, 0};
        }
        int dataEnd = size;
        int padding = 0;
        boolean insidePadding = false;
        for (int offset = 0; offset < size; offset++) {
            if (source[offset] == '=') {
                if (!insidePadding) {
                    insidePadding = true;
                    dataEnd = offset;
                }
                padding++;
                continue;
            }
            if (insidePadding || index(alphabet, source[offset]) < 0) {
                return new int[] {offset, 0};
            }
        }
        if (size % quantum != 0) {
            return new int[] {size, 0};
        }
        int remainder = dataEnd % quantum;
        int expected = expectedPadding(width, remainder);
        if (expected < 0 || padding != expected) {
            return new int[] {dataEnd, 0};
        }
        long buffer = 0;
        int bits = 0;
        for (int offset = 0; offset < dataEnd; offset++) {
            buffer = (buffer << width) | index(alphabet, source[offset]);
            bits += width;
            while (bits >= 8) {
                bits -= 8;
            }
        }
        if (bits > 0 && (buffer & ((1L << bits) - 1)) != 0) {
            return new int[] {dataEnd - 1, 0};
        }
        return new int[] {-1, dataEnd};
    }

    private static int expectedPadding(int width, int remainder) {
        if (width == 5) {
            switch (remainder) {
                case 0: return 0;
                case 2: return 6;
                case 4: return 4;
                case 5: return 3;
                case 7: return 1;
                default: return -1;
            }
        }
        switch (remainder) {
            case 0: return 0;
            case 2: return 2;
            case 3: return 1;
            default: return -1;
        }
    }

    private static List<Object> decoded(int[] source, int dataEnd, String alphabet, int width) {
        ArrayList<Integer> octets = new ArrayList<Integer>();
        long buffer = 0;
        int bits = 0;
        for (int offset = 0; offset < dataEnd; offset++) {
            buffer = (buffer << width) | index(alphabet, source[offset]);
            bits += width;
            while (bits >= 8) {
                bits -= 8;
                octets.add((int) ((buffer >> bits) & 255));
            }
        }
        return bytes(octets);
    }

    // ----- runtime/base85 -----

    private static String base85Alphabet(String name, String native_) {
        if ("ascii85".equals(name)) {
            return ASCII85;
        }
        if ("rfc1924".equals(name)) {
            return RFC1924;
        }
        throw failed("base85", native_, 2);
    }

    public static String runtime_base85__base85_encode(List<?> origin, String name) {
        ArrayList<Integer> octets = new ArrayList<Integer>();
        for (int index = 0; index < origin.size(); index++) {
            octets.add(octet(origin, index));
        }
        return base85Encoded(octets, name, base85Alphabet(name, "base85_encode"));
    }

    private static String base85Encoded(List<Integer> octets, String name, String alphabet) {
        StringBuilder result = new StringBuilder();
        for (int offset = 0; offset < octets.size(); offset += 4) {
            int amount = Math.min(octets.size() - offset, 4);
            long word = 0;
            for (int index = 0; index < 4; index++) {
                word = (word << 8) | (index < amount ? octets.get(offset + index) : 0);
            }
            if ("ascii85".equals(name) && amount == 4 && word == 0) {
                result.append('z');
                continue;
            }
            char[] digits = new char[5];
            for (int index = 4; index >= 0; index--) {
                digits[index] = alphabet.charAt((int) (word % 85));
                word /= 85;
            }
            result.append(digits, 0, amount == 4 ? 5 : amount + 1);
        }
        return result.toString();
    }

    public static BigInteger runtime_base85__base85_invalid_offset(String origin, String name) {
        ArrayList<Integer> octets = new ArrayList<Integer>();
        return BigInteger.valueOf(
            base85Parsed(points(origin), name, base85Alphabet(name, "base85_invalid_offset"), octets));
    }

    public static List<Object> runtime_base85__base85_decode(String origin, String name) {
        ArrayList<Integer> octets = new ArrayList<Integer>();
        if (base85Parsed(points(origin), name, base85Alphabet(name, "base85_decode"), octets) != -1) {
            throw failed("base85", "base85_decode", 2);
        }
        return bytes(octets);
    }

    private static long base85Word(List<Integer> values) {
        long word = 0;
        for (int value : values) {
            word = word * 85 + value;
        }
        return word;
    }

    private static void wordBytes(long word, List<Integer> out, int count) {
        for (int index = 0; index < count; index++) {
            out.add((int) ((word >> (24 - 8 * index)) & 255));
        }
    }

    /** Первое неверное смещение или {@code -1}; {@code octets} — результат. */
    private static int base85Parsed(int[] source, String name, String alphabet, List<Integer> octets) {
        ArrayList<Integer> values = new ArrayList<Integer>();
        int groupStart = 0;
        int size = source.length;
        for (int offset = 0; offset < size; offset++) {
            int symbol = source[offset];
            if ("ascii85".equals(name) && symbol == 'z') {
                if (!values.isEmpty()) {
                    return offset;
                }
                wordBytes(0, octets, 4);
                groupStart = offset + 1;
                continue;
            }
            int value = index(alphabet, symbol);
            if (value < 0) {
                return offset;
            }
            if (values.isEmpty()) {
                groupStart = offset;
            }
            values.add(value);
            if (values.size() == 5) {
                long word = base85Word(values);
                if (word > 0xFFFFFFFFL) {
                    return offset;
                }
                wordBytes(word, octets, 4);
                values.clear();
                groupStart = offset + 1;
            }
        }
        if (values.size() == 1) {
            return size;
        }
        if (!values.isEmpty()) {
            int amount = values.size();
            while (values.size() < 5) {
                values.add(84);
            }
            long word = base85Word(values);
            if (word > 0xFFFFFFFFL) {
                return size - 1;
            }
            ArrayList<Integer> partial = new ArrayList<Integer>();
            wordBytes(word, partial, amount - 1);
            String canonical = base85Encoded(partial, name, alphabet);
            String spelling = new String(source, groupStart, Math.min(amount, size - groupStart));
            if (!canonical.equals(spelling)) {
                return size - 1;
            }
            octets.addAll(partial);
        }
        return -1;
    }

    // ----- runtime/quoted_printable -----

    public static List<Object> runtime_quoted_printable__quoted_printable_encode(List<?> origin) {
        StringBuilder result = new StringBuilder();
        ArrayList<String> tokens = new ArrayList<String>();
        int size = origin.size();
        int offset = 0;
        while (offset < size) {
            if (octet(origin, offset) == 13 && offset + 1 < size && octet(origin, offset + 1) == 10) {
                folded(result, tokens, true);
                offset += 2;
                continue;
            }
            boolean trailing = offset + 1 == size
                || (offset + 2 < size && octet(origin, offset + 1) == 13 && octet(origin, offset + 2) == 10);
            tokens.add(token(octet(origin, offset), trailing));
            offset++;
        }
        folded(result, tokens, false);
        ArrayList<Integer> octets = new ArrayList<Integer>();
        for (int index = 0; index < result.length(); index++) {
            octets.add((int) result.charAt(index));
        }
        return bytes(octets);
    }

    private static String token(int octet, boolean trailing) {
        if (octet == 61 || octet < 32 || octet > 126 || ((octet == 9 || octet == 32) && trailing)) {
            return String.format("=%02X", octet);
        }
        return String.valueOf((char) octet);
    }

    /** Мягкие переносы {@code =\r\n}: строка не длиннее 76 (75, если токены ещё есть). */
    private static void folded(StringBuilder result, List<String> tokens, boolean hardBreak) {
        StringBuilder line = new StringBuilder();
        while (!tokens.isEmpty()) {
            String token = tokens.get(0);
            int limit = tokens.size() > 1 ? 75 : 76;
            if (line.length() + token.length() > limit) {
                result.append(line).append("=\r\n");
                line.setLength(0);
                continue;
            }
            line.append(tokens.remove(0));
        }
        result.append(line);
        if (hardBreak) {
            result.append("\r\n");
        }
    }

    public static BigInteger runtime_quoted_printable__quoted_printable_invalid_offset(List<?> origin) {
        return BigInteger.valueOf(qpParsed(origin, new ArrayList<Integer>()));
    }

    public static List<Object> runtime_quoted_printable__quoted_printable_decode(List<?> origin) {
        ArrayList<Integer> decoded = new ArrayList<Integer>();
        if (qpParsed(origin, decoded) != -1) {
            throw failed("quoted_printable", "quoted_printable_decode", 1);
        }
        return bytes(decoded);
    }

    private static int qpHex(int octet) {
        if (octet >= 48 && octet <= 57) {
            return octet - 48;
        }
        if (octet >= 65 && octet <= 70) {
            return octet - 65 + 10;
        }
        if (octet >= 97 && octet <= 102) {
            return octet - 97 + 10;
        }
        return -1;
    }

    private static int qpParsed(List<?> octets, List<Integer> decoded) {
        int size = octets.size();
        int offset = 0;
        int column = 0;
        while (offset < size) {
            int octet = octet(octets, offset);
            if (octet == 13) {
                if (offset + 1 >= size || octet(octets, offset + 1) != 10) {
                    return offset;
                }
                decoded.add(13);
                decoded.add(10);
                column = 0;
                offset += 2;
                continue;
            }
            if (octet == 10) {
                return offset;
            }
            if (octet == 61) {
                if (offset + 2 < size && octet(octets, offset + 1) == 13 && octet(octets, offset + 2) == 10) {
                    if (column + 1 > 76) {
                        return offset;
                    }
                    column = 0;
                    offset += 3;
                    continue;
                }
                if (offset + 2 >= size) {
                    return offset;
                }
                int high = qpHex(octet(octets, offset + 1));
                int low = qpHex(octet(octets, offset + 2));
                if (high < 0 || low < 0) {
                    return offset;
                }
                if (column + 3 > 76) {
                    return offset + (76 - column);
                }
                decoded.add(high * 16 + low);
                column += 3;
                offset += 3;
                continue;
            }
            if (octet != 9 && octet != 32 && (octet < 33 || octet > 126)) {
                return offset;
            }
            if ((octet == 9 || octet == 32)
                && (offset + 1 == size
                    || (offset + 2 < size && octet(octets, offset + 1) == 13 && octet(octets, offset + 2) == 10))) {
                return offset;
            }
            if (column + 1 > 76) {
                return offset;
            }
            decoded.add(octet);
            column++;
            offset++;
        }
        return -1;
    }
}
