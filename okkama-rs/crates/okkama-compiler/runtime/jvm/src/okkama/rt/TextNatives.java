package okkama.rt;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Map;

/**
 * Natives байтовых и текстовых преобразований: {@code runtime/buffer},
 * {@code runtime/binary}, {@code runtime/single_byte},
 * {@code runtime/hpack}, {@code runtime/collections}. Тексты отказов — как у
 * Perl-provider; {@code die} Perl — ошибка выполнения ({@link Rt#trap}).
 */
public final class TextNatives {
    private TextNatives() {}

    private static Failure failure(String domain, String code, String symbol, String message) {
        return NativeFailure.raise(domain, code, symbol, message);
    }

    private static int[] scalars(String text) {
        return text.codePoints().toArray();
    }

    // ---- runtime/buffer ----

    static final class BufferState {
        byte[] bytes;
        int size;
        boolean closed;
    }

    private static BufferState buffer(Object handle, String name, int arity, boolean allowClosed) {
        BufferState state = (BufferState) handle;
        if (state.closed && !allowClosed) {
            throw failure("core.buffer", "already_closed", "runtime/buffer@1::" + name + "/" + arity,
                "Byte buffer is already closed");
        }
        return state;
    }

    public static Object runtime_buffer__buffer_state_create(List<?> initial) {
        BufferState state = new BufferState();
        state.bytes = Natives.toBytes(initial);
        state.size = state.bytes.length;
        return state;
    }

    public static List<Object> runtime_buffer__buffer_state_read(Object handle, BigInteger limit) {
        BufferState state = buffer(handle, "buffer_state_read", 2, false);
        if (limit.signum() <= 0) {
            throw failure("core.buffer", "invalid_input", "runtime/buffer@1::buffer_state_read/2",
                "Buffer read limit must be positive");
        }
        int count = limit.compareTo(BigInteger.valueOf(state.size)) < 0 ? limit.intValue() : state.size;
        List<Object> out = Natives.fromBytes(state.bytes, count);
        System.arraycopy(state.bytes, count, state.bytes, 0, state.size - count);
        state.size -= count;
        return out;
    }

    public static BigInteger runtime_buffer__buffer_state_append(Object handle, List<?> value) {
        BufferState state = buffer(handle, "buffer_state_append", 2, false);
        byte[] bytes = Natives.toBytes(value);
        if (state.size + bytes.length > state.bytes.length) {
            byte[] grown = new byte[Math.max(state.size + bytes.length, state.bytes.length * 2 + 16)];
            System.arraycopy(state.bytes, 0, grown, 0, state.size);
            state.bytes = grown;
        }
        System.arraycopy(bytes, 0, state.bytes, state.size, bytes.length);
        state.size += bytes.length;
        return BigInteger.valueOf(bytes.length);
    }

    public static List<Object> runtime_buffer__buffer_state_bytes(Object handle) {
        BufferState state = buffer(handle, "buffer_state_bytes", 1, false);
        return Natives.fromBytes(state.bytes, state.size);
    }

    public static BigInteger runtime_buffer__buffer_state_size(Object handle) {
        return BigInteger.valueOf(buffer(handle, "buffer_state_size", 1, false).size);
    }

    public static boolean runtime_buffer__buffer_state_closed(Object handle) {
        return buffer(handle, "buffer_state_closed", 1, true).closed;
    }

    public static boolean runtime_buffer__buffer_state_close(Object handle) {
        BufferState state = buffer(handle, "buffer_state_close", 1, true);
        if (state.closed) {
            return false;
        }
        state.closed = true;
        state.bytes = new byte[0];
        state.size = 0;
        return true;
    }

    // ---- runtime/binary ----

    private static final BigInteger INT_MIN = BigInteger.valueOf(Long.MIN_VALUE);
    private static final BigInteger INT_MAX = BigInteger.valueOf(Long.MAX_VALUE);

    private static BigInteger modulus(int size) {
        return BigInteger.ONE.shiftLeft(8 * size);
    }

    private static boolean integerFits(BigInteger value, BigInteger size, String signedness) {
        if (size.signum() <= 0) {
            return false;
        }
        if (size.compareTo(BigInteger.valueOf(1 << 20)) > 0) {
            throw Natives.died();
        }
        BigInteger modulus = modulus(size.intValue());
        if (signedness.equals("unsigned")) {
            return value.signum() >= 0 && value.compareTo(modulus) < 0;
        }
        if (!signedness.equals("signed")) {
            throw Natives.died();
        }
        BigInteger half = modulus.shiftRight(1);
        return value.compareTo(half.negate()) >= 0 && value.compareTo(half) < 0;
    }

    private static byte[] ordered(List<?> origin, String order) {
        if (!order.equals("big") && !order.equals("little")) {
            throw Natives.died();
        }
        byte[] bytes = Natives.toBytes(origin);
        if (order.equals("little")) {
            for (int left = 0, right = bytes.length - 1; left < right; left++, right--) {
                byte swap = bytes[left];
                bytes[left] = bytes[right];
                bytes[right] = swap;
            }
        }
        return bytes;
    }

    private static BigInteger integerFromBytes(byte[] big, String signedness) {
        if (big.length == 0) {
            throw Natives.died();
        }
        BigInteger number = new BigInteger(1, big);
        if (signedness.equals("signed")) {
            if ((big[0] & 0x80) != 0) {
                number = number.subtract(modulus(big.length));
            }
        } else if (!signedness.equals("unsigned")) {
            throw Natives.died();
        }
        return number;
    }

    private static boolean intRange(BigInteger number) {
        return number.compareTo(INT_MIN) >= 0 && number.compareTo(INT_MAX) <= 0;
    }

    public static List<Object> runtime_binary__binary_masked_slice(List<?> origin, List<?> mask,
            BigInteger phase, BigInteger begin, BigInteger count) {
        long length = origin.size();
        if (mask.size() != 4 || phase.signum() < 0 || phase.compareTo(BigInteger.valueOf(3)) > 0
                || begin.signum() < 0 || begin.compareTo(BigInteger.valueOf(length)) > 0
                || count.signum() < 0 || count.compareTo(BigInteger.valueOf(65536)) > 0
                || count.compareTo(BigInteger.valueOf(length).subtract(begin)) > 0) {
            throw failure("core.binary", "invalid_input", "runtime/binary@1::binary_masked_slice/5",
                "invalid bounded byte mask coordinates");
        }
        int offset = phase.intValue();
        int start = begin.intValue();
        int size = count.intValue();
        byte[] key = Natives.toBytes(mask);
        int rotation = (offset + start) % 4;
        ArrayList<Object> out = new ArrayList<Object>(size);
        for (int index = 0; index < size; index++) {
            byte value = (Byte) origin.get(start + index);
            out.add(Byte.valueOf((byte) (value ^ key[(rotation + index) % 4])));
        }
        return out;
    }

    public static boolean runtime_binary__binary_integer_fits(BigInteger value, BigInteger size,
            String signedness) {
        return integerFits(value, size, signedness);
    }

    public static List<Object> runtime_binary__binary_integer_bytes(BigInteger value, BigInteger size,
            String signedness, String order) {
        if (!integerFits(value, size, signedness)) {
            throw Natives.died();
        }
        int width = size.intValue();
        BigInteger number = value.signum() < 0 ? value.add(modulus(width)) : value;
        if (!order.equals("big") && !order.equals("little")) {
            throw Natives.died();
        }
        byte[] bytes = new byte[width];
        for (int index = 0; index < width; index++) {
            byte octet = (byte) number.shiftRight(8 * index).intValue();
            bytes[order.equals("big") ? width - 1 - index : index] = octet;
        }
        return Natives.fromBytes(bytes, width);
    }

    public static boolean runtime_binary__binary_integer_from_bytes_fits(List<?> origin,
            String signedness, String order) {
        byte[] big = ordered(origin, order);
        return big.length > 0 && intRange(integerFromBytes(big, signedness));
    }

    public static BigInteger runtime_binary__binary_integer_from_bytes(List<?> origin, String signedness,
            String order) {
        BigInteger number = integerFromBytes(ordered(origin, order), signedness);
        if (!intRange(number)) {
            throw Natives.died();
        }
        return number;
    }

    private static byte[] floatBytes(double value, int width) {
        byte[] bytes = new byte[width / 8];
        long bits = width == 32 ? Float.floatToRawIntBits((float) value) & 0xffffffffL
            : Double.doubleToRawLongBits(value);
        for (int index = 0; index < bytes.length; index++) {
            bytes[bytes.length - 1 - index] = (byte) (bits >>> (8 * index));
        }
        return bytes;
    }

    private static int width(BigInteger width) {
        if (!width.equals(BigInteger.valueOf(32)) && !width.equals(BigInteger.valueOf(64))) {
            throw Natives.died();
        }
        return width.intValue();
    }

    private static boolean floatFinite(byte[] big, int width) {
        if (width == 32) {
            return big.length == 4 && ((((big[0] & 0x7f) << 1) | ((big[1] & 0xff) >> 7)) != 0xff);
        }
        return big.length == 8 && ((((big[0] & 0x7f) << 4) | ((big[1] & 0xff) >> 4)) != 0x7ff);
    }

    public static boolean runtime_binary__binary_float_fits(Double value, BigInteger width) {
        int bits = width(width);
        return floatFinite(floatBytes(value, bits), bits);
    }

    public static List<Object> runtime_binary__binary_float_bytes(Double value, BigInteger width,
            String order) {
        int bits = width(width);
        if (!order.equals("big") && !order.equals("little")) {
            throw Natives.died();
        }
        byte[] big = floatBytes(value, bits);
        return order.equals("big") ? Natives.fromBytes(big, big.length)
            : Natives.fromBytes(ordered(Natives.fromBytes(big, big.length), "little"), big.length);
    }

    public static boolean runtime_binary__binary_float_finite(List<?> origin, BigInteger width,
            String order) {
        return floatFinite(ordered(origin, order), width(width));
    }

    public static Double runtime_binary__binary_float_from_bytes(List<?> origin, BigInteger width,
            String order) {
        int bits = width(width);
        byte[] big = ordered(origin, order);
        if (!floatFinite(big, bits)) {
            throw Natives.died();
        }
        long raw = new BigInteger(1, big).longValue();
        return bits == 32 ? (double) Float.intBitsToFloat((int) raw) : Double.longBitsToDouble(raw);
    }

    // ---- runtime/single_byte ----

    private static BigInteger prefix(String origin, int maximum) {
        int[] text = scalars(origin);
        int offset = 0;
        while (offset < text.length && text[offset] <= maximum) {
            offset++;
        }
        return BigInteger.valueOf(offset);
    }

    private static List<Object> encoded(String origin) {
        int[] text = scalars(origin);
        ArrayList<Object> out = new ArrayList<Object>(text.length);
        for (int scalar : text) {
            out.add(Byte.valueOf((byte) scalar));
        }
        return out;
    }

    private static String decoded(List<?> origin) {
        StringBuilder out = new StringBuilder(origin.size());
        for (Object octet : origin) {
            out.append((char) (((Byte) octet) & 0xff));
        }
        return out.toString();
    }

    public static BigInteger runtime_single_byte__ascii_representable_prefix(String origin) {
        return prefix(origin, 0x7f);
    }

    public static List<Object> runtime_single_byte__ascii_encode(String origin) {
        return encoded(origin);
    }

    public static BigInteger runtime_single_byte__ascii_valid_prefix(List<?> origin) {
        int offset = 0;
        while (offset < origin.size() && (Byte) origin.get(offset) >= 0) {
            offset++;
        }
        return BigInteger.valueOf(offset);
    }

    public static String runtime_single_byte__ascii_decode(List<?> origin) {
        return decoded(origin);
    }

    public static BigInteger runtime_single_byte__latin1_representable_prefix(String origin) {
        return prefix(origin, 0xff);
    }

    public static List<Object> runtime_single_byte__latin1_encode(String origin) {
        return encoded(origin);
    }

    public static String runtime_single_byte__latin1_decode(List<?> origin) {
        return decoded(origin);
    }

    // ---- runtime/hpack ----

    /** Trie RFC 7541 Appendix B: узлы [ноль, единица, символ, допустимое дополнение]. */
    private static final String HUFFMAN = "98,1,-1,1,151,2,-1,1,173,3,-1,1,204,4,-1,1,263,5,-1,1,113,6,-1,1,211,7,-1,1,104,8,-1,1,"
        + "116,9,-1,0,108,10,-1,0,11,14,-1,0,12,166,-1,0,13,111,-1,0,-1,-1,0,0,220,15,-1,0,222,16,-1,"
        + "0,158,17,-1,0,270,18,-1,0,216,19,-1,0,279,20,-1,0,21,27,-1,0,377,22,-1,0,414,23,-1,0,24,"
        + "301,-1,0,25,298,-1,0,26,295,-1,0,-1,-1,1,0,314,28,-1,0,50,29,-1,0,362,30,-1,0,403,31,-1,0,"
        + "440,32,-1,0,33,55,-1,0,34,46,-1,0,35,39,-1,0,510,36,-1,0,37,38,-1,0,-1,-1,2,0,-1,-1,3,0,"
        + "40,43,-1,0,41,42,-1,0,-1,-1,4,0,-1,-1,5,0,44,45,-1,0,-1,-1,6,0,-1,-1,7,0,47,67,-1,0,48,63,"
        + "-1,0,49,62,-1,0,-1,-1,8,0,396,51,-1,0,52,309,-1,0,486,53,-1,0,54,307,-1,0,-1,-1,9,0,74,56,"
        + "-1,0,91,57,-1,0,274,58,-1,0,502,59,-1,0,60,81,-1,0,61,65,-1,0,-1,-1,10,0,-1,-1,11,0,64,66,"
        + "-1,0,-1,-1,12,0,-1,-1,13,0,-1,-1,14,0,68,71,-1,0,69,70,-1,0,-1,-1,15,0,-1,-1,16,0,72,73,"
        + "-1,0,-1,-1,17,0,-1,-1,18,0,75,84,-1,0,76,79,-1,0,77,78,-1,0,-1,-1,19,0,-1,-1,20,0,80,83,"
        + "-1,0,-1,-1,21,0,82,512,-1,0,-1,-1,22,0,-1,-1,23,0,85,88,-1,0,86,87,-1,0,-1,-1,24,0,-1,-1,"
        + "25,0,89,90,-1,0,-1,-1,26,0,-1,-1,27,0,92,95,-1,0,93,94,-1,0,-1,-1,28,0,-1,-1,29,0,96,97,"
        + "-1,0,-1,-1,30,0,-1,-1,31,0,133,99,-1,0,100,129,-1,0,258,101,-1,0,102,126,-1,0,103,112,-1,"
        + "0,-1,-1,32,0,105,119,-1,0,106,107,-1,0,-1,-1,33,0,-1,-1,34,0,271,109,-1,0,110,164,-1,0,-1,"
        + "-1,35,0,-1,-1,36,0,-1,-1,37,0,114,124,-1,0,115,122,-1,0,-1,-1,38,0,165,117,-1,0,118,123,"
        + "-1,0,-1,-1,39,0,120,121,-1,0,-1,-1,40,0,-1,-1,41,0,-1,-1,42,0,-1,-1,43,0,125,157,-1,0,-1,"
        + "-1,44,0,127,128,-1,0,-1,-1,45,0,-1,-1,46,0,130,144,-1,0,131,141,-1,0,132,140,-1,0,-1,-1,"
        + "47,0,134,229,-1,0,135,138,-1,0,136,137,-1,0,-1,-1,48,0,-1,-1,49,0,139,227,-1,0,-1,-1,50,0,"
        + "-1,-1,51,0,142,143,-1,0,-1,-1,52,0,-1,-1,53,0,145,148,-1,0,146,147,-1,0,-1,-1,54,0,-1,-1,"
        + "55,0,149,150,-1,0,-1,-1,56,0,-1,-1,57,0,160,152,-1,0,246,153,-1,0,256,154,-1,0,155,170,-1,"
        + "0,156,169,-1,0,-1,-1,58,0,-1,-1,59,0,159,226,-1,0,-1,-1,60,0,161,232,-1,0,162,224,-1,0,"
        + "163,168,-1,0,-1,-1,61,0,-1,-1,62,0,-1,-1,63,0,167,215,-1,0,-1,-1,64,0,-1,-1,65,0,-1,-1,66,"
        + "0,171,172,-1,0,-1,-1,67,0,-1,-1,68,0,174,189,-1,0,175,182,-1,0,176,179,-1,0,177,178,-1,0,"
        + "-1,-1,69,0,-1,-1,70,0,180,181,-1,0,-1,-1,71,0,-1,-1,72,0,183,186,-1,0,184,185,-1,0,-1,-1,"
        + "73,0,-1,-1,74,0,187,188,-1,0,-1,-1,75,0,-1,-1,76,0,190,197,-1,0,191,194,-1,0,192,193,-1,0,"
        + "-1,-1,77,0,-1,-1,78,0,195,196,-1,0,-1,-1,79,0,-1,-1,80,0,198,201,-1,0,199,200,-1,0,-1,-1,"
        + "81,0,-1,-1,82,0,202,203,-1,0,-1,-1,83,0,-1,-1,84,0,205,242,-1,0,206,209,-1,0,207,208,-1,0,"
        + "-1,-1,85,0,-1,-1,86,0,210,213,-1,0,-1,-1,87,0,212,214,-1,0,-1,-1,88,0,-1,-1,89,0,-1,-1,90,"
        + "0,-1,-1,91,0,217,286,-1,0,218,276,-1,0,219,410,-1,0,-1,-1,92,0,221,273,-1,0,-1,-1,93,0,"
        + "223,272,-1,0,-1,-1,94,0,225,228,-1,0,-1,-1,95,0,-1,-1,96,0,-1,-1,97,0,-1,-1,98,0,230,240,"
        + "-1,0,231,235,-1,0,-1,-1,99,0,233,237,-1,0,234,236,-1,0,-1,-1,100,0,-1,-1,101,0,-1,-1,102,"
        + "0,238,239,-1,0,-1,-1,103,0,-1,-1,104,0,241,252,-1,0,-1,-1,105,0,243,254,-1,0,244,245,-1,0,"
        + "-1,-1,106,0,-1,-1,107,0,247,250,-1,0,248,249,-1,0,-1,-1,108,0,-1,-1,109,0,251,253,-1,0,-1,"
        + "-1,110,0,-1,-1,111,0,-1,-1,112,0,255,262,-1,0,-1,-1,113,0,257,261,-1,0,-1,-1,114,0,259,"
        + "260,-1,0,-1,-1,115,0,-1,-1,116,0,-1,-1,117,0,-1,-1,118,0,264,267,-1,0,265,266,-1,0,-1,-1,"
        + "119,0,-1,-1,120,0,268,269,-1,0,-1,-1,121,0,-1,-1,122,0,-1,-1,123,0,-1,-1,124,0,-1,-1,125,"
        + "0,-1,-1,126,0,275,459,-1,0,-1,-1,127,0,436,277,-1,0,278,285,-1,0,-1,-1,128,0,372,280,-1,0,"
        + "281,332,-1,0,282,291,-1,0,473,283,-1,0,284,290,-1,0,-1,-1,129,0,-1,-1,130,0,287,328,-1,0,"
        + "288,388,-1,0,289,345,-1,0,-1,-1,131,0,-1,-1,132,0,292,296,-1,0,293,294,-1,0,-1,-1,133,0,"
        + "-1,-1,134,0,-1,-1,135,0,297,313,-1,0,-1,-1,136,0,299,300,-1,0,-1,-1,137,0,-1,-1,138,0,302,"
        + "305,-1,0,303,304,-1,0,-1,-1,139,0,-1,-1,140,0,306,308,-1,0,-1,-1,141,0,-1,-1,142,0,-1,-1,"
        + "143,0,310,319,-1,0,311,312,-1,0,-1,-1,144,0,-1,-1,145,0,-1,-1,146,0,315,350,-1,0,316,325,"
        + "-1,0,317,322,-1,0,318,321,-1,0,-1,-1,147,0,320,341,-1,0,-1,-1,148,0,-1,-1,149,0,323,324,"
        + "-1,0,-1,-1,150,0,-1,-1,151,0,326,338,-1,0,327,336,-1,0,-1,-1,152,0,465,329,-1,0,330,355,"
        + "-1,0,331,344,-1,0,-1,-1,153,0,333,347,-1,0,334,342,-1,0,335,337,-1,0,-1,-1,154,0,-1,-1,"
        + "155,0,-1,-1,156,0,339,340,-1,0,-1,-1,157,0,-1,-1,158,0,-1,-1,159,0,343,346,-1,0,-1,-1,160,"
        + "0,-1,-1,161,0,-1,-1,162,0,-1,-1,163,0,348,360,-1,0,349,359,-1,0,-1,-1,164,0,351,369,-1,0,"
        + "352,357,-1,0,353,354,-1,0,-1,-1,165,0,-1,-1,166,0,356,366,-1,0,-1,-1,167,0,358,368,-1,0,"
        + "-1,-1,168,0,-1,-1,169,0,361,367,-1,0,-1,-1,170,0,363,417,-1,0,364,449,-1,0,365,434,-1,0,"
        + "-1,-1,171,0,-1,-1,172,0,-1,-1,173,0,-1,-1,174,0,370,385,-1,0,371,383,-1,0,-1,-1,175,0,373,"
        + "451,-1,0,374,381,-1,0,375,376,-1,0,-1,-1,176,0,-1,-1,177,0,378,393,-1,0,379,390,-1,0,380,"
        + "384,-1,0,-1,-1,178,0,382,437,-1,0,-1,-1,179,0,-1,-1,180,0,-1,-1,181,0,386,387,-1,0,-1,-1,"
        + "182,0,-1,-1,183,0,389,409,-1,0,-1,-1,184,0,391,392,-1,0,-1,-1,185,0,-1,-1,186,0,394,400,"
        + "-1,0,395,399,-1,0,-1,-1,187,0,397,412,-1,0,398,402,-1,0,-1,-1,188,0,-1,-1,189,0,401,411,"
        + "-1,0,-1,-1,190,0,-1,-1,191,0,404,427,-1,0,405,424,-1,0,406,421,-1,0,407,408,-1,0,-1,-1,"
        + "192,0,-1,-1,193,0,-1,-1,194,0,-1,-1,195,0,-1,-1,196,0,413,474,-1,0,-1,-1,197,0,415,475,-1,"
        + "0,416,471,-1,0,-1,-1,198,0,481,418,-1,0,419,478,-1,0,420,435,-1,0,-1,-1,199,0,422,423,-1,"
        + "0,-1,-1,200,0,-1,-1,201,0,425,438,-1,0,426,433,-1,0,-1,-1,202,0,455,428,-1,0,490,429,-1,0,"
        + "511,430,-1,0,431,432,-1,0,-1,-1,203,0,-1,-1,204,0,-1,-1,205,0,-1,-1,206,0,-1,-1,207,0,-1,"
        + "-1,208,0,-1,-1,209,0,439,446,-1,0,-1,-1,210,0,441,494,-1,0,442,461,-1,0,443,447,-1,0,444,"
        + "445,-1,0,-1,-1,211,0,-1,-1,212,0,-1,-1,213,0,448,460,-1,0,-1,-1,214,0,450,467,-1,0,-1,-1,"
        + "215,0,452,469,-1,0,453,454,-1,0,-1,-1,216,0,-1,-1,217,0,456,484,-1,0,457,458,-1,0,-1,-1,"
        + "218,0,-1,-1,219,0,-1,-1,220,0,-1,-1,221,0,462,488,-1,0,463,464,-1,0,-1,-1,222,0,-1,-1,223,"
        + "0,466,468,-1,0,-1,-1,224,0,-1,-1,225,0,-1,-1,226,0,470,472,-1,0,-1,-1,227,0,-1,-1,228,0,"
        + "-1,-1,229,0,-1,-1,230,0,-1,-1,231,0,476,477,-1,0,-1,-1,232,0,-1,-1,233,0,479,480,-1,0,-1,"
        + "-1,234,0,-1,-1,235,0,482,483,-1,0,-1,-1,236,0,-1,-1,237,0,485,487,-1,0,-1,-1,238,0,-1,-1,"
        + "239,0,-1,-1,240,0,489,493,-1,0,-1,-1,241,0,491,492,-1,0,-1,-1,242,0,-1,-1,243,0,-1,-1,244,"
        + "0,495,503,-1,0,496,499,-1,0,497,498,-1,0,-1,-1,245,0,-1,-1,246,0,500,501,-1,0,-1,-1,247,0,"
        + "-1,-1,248,0,-1,-1,249,0,504,507,-1,0,505,506,-1,0,-1,-1,250,0,-1,-1,251,0,508,509,-1,0,-1,"
        + "-1,252,0,-1,-1,253,0,-1,-1,254,0,-1,-1,255,0,-1,-1,256,0";
    private static int[] huffman;

    private static int[] huffmanNodes() {
        if (huffman == null) {
            String[] parts = HUFFMAN.split(",");
            int[] nodes = new int[parts.length];
            for (int index = 0; index < nodes.length; index++) {
                nodes[index] = Integer.parseInt(parts[index]);
            }
            huffman = nodes;
        }
        return huffman;
    }

    public static List<Object> runtime_hpack__huffman_decode(List<?> origin, BigInteger capacity) {
        String symbol = "runtime/hpack@1::huffman_decode/2";
        if (capacity.signum() < 0 || capacity.compareTo(BigInteger.valueOf(1048576)) > 0
                || origin.size() > 1048576) {
            throw failure("core.hpack", "invalid_input", symbol, "invalid HPACK Huffman bounds");
        }
        int maximum = capacity.intValue();
        int[] nodes = huffmanNodes();
        int node = 0;
        ArrayList<Object> out = new ArrayList<Object>();
        for (Object item : origin) {
            int octet = ((Byte) item) & 0xff;
            for (int shift = 7; shift >= 0; shift--) {
                int bit = (octet >> shift) & 1;
                node = nodes[node * 4 + bit];
                if (node < 0) {
                    throw failure("core.hpack", "invalid_data", symbol, "invalid HPACK Huffman code");
                }
                int decoded = nodes[node * 4 + 2];
                if (decoded >= 0) {
                    if (decoded == 256) {
                        throw failure("core.hpack", "invalid_data", symbol, "HPACK Huffman EOS is forbidden");
                    }
                    if (out.size() >= maximum) {
                        throw failure("core.hpack", "resource_exhausted", symbol, "decoded HPACK Huffman limit");
                    }
                    out.add(Byte.valueOf((byte) decoded));
                    node = 0;
                }
            }
        }
        if (nodes[node * 4 + 3] == 0) {
            throw failure("core.hpack", "invalid_data", symbol, "invalid HPACK Huffman padding");
        }
        return out;
    }

    // ---- runtime/collections ----

    public static boolean runtime_collections__string_star_match(String pattern, String target,
            boolean anchored, BigInteger limit) {
        String symbol = "runtime/collections@1::string_star_match/4";
        if (limit.compareTo(BigInteger.ONE) < 0 || limit.compareTo(BigInteger.valueOf(262144)) > 0) {
            throw failure("core.collections", "invalid_input", symbol, "Invalid string star-match step budget");
        }
        int budget = limit.intValue();
        int[] left = scalars(pattern);
        int[] right = scalars(target);
        int position = 0;
        int at = 0;
        int star = -1;
        int retry = 0;
        int steps = 0;
        while (true) {
            if (++steps > budget) {
                throw failure("core.collections", "resource_exhausted", symbol, "String star-match step limit");
            }
            if (position == left.length && (!anchored || at == right.length)) {
                return true;
            }
            if (position < left.length && left[position] == '*') {
                star = position;
                retry = at;
                position++;
                continue;
            }
            if (at < right.length && position < left.length && left[position] == right[at]) {
                at++;
                position++;
                continue;
            }
            if (star >= 0 && retry < right.length) {
                at = ++retry;
                position = star + 1;
                continue;
            }
            return false;
        }
    }

    public static Map<Object, Object> runtime_collections__string_map_snapshot(List<?> keys, List<?> values,
            BigInteger entries, BigInteger characters) {
        String symbol = "runtime/collections@1::string_map_snapshot/4";
        if (entries.signum() < 0 || entries.compareTo(BigInteger.valueOf(4096)) > 0
                || characters.signum() < 0 || characters.compareTo(BigInteger.valueOf(1048576)) > 0) {
            throw failure("core.collections", "invalid_input", symbol, "Invalid string map snapshot budgets");
        }
        if (keys.size() != values.size()) {
            throw failure("core.collections", "invalid_input", symbol, "String map coordinates have different lengths");
        }
        if (keys.size() > entries.intValue()) {
            throw failure("core.collections", "resource_exhausted", symbol, "String map entry limit");
        }
        Map<Object, Object> out = Rt.newMap();
        HashSet<Object> seen = new HashSet<Object>();
        long used = 0;
        long maximum = characters.longValue();
        for (int index = 0; index < keys.size(); index++) {
            String key = (String) keys.get(index);
            String value = (String) values.get(index);
            if (!seen.add(key)) {
                throw failure("core.collections", "invalid_input", symbol, "String map has duplicate keys");
            }
            long width = key.codePointCount(0, key.length()) + value.codePointCount(0, value.length());
            if (width > maximum - used) {
                throw failure("core.collections", "resource_exhausted", symbol, "String map character limit");
            }
            used += width;
            out.put(key, value);
        }
        return out;
    }
}
