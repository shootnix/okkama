package okkama.rt;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.PrintStream;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.List;

/**
 * Natives модулей std (OIR-1 §14.1): {@code runtime/stdio} и {@code runtime/utf}.
 * Имя метода — символ native, где {@code /} заменён на {@code _}, а
 * {@code ::} — на {@code __}. {@code @byte} — {@link List} из {@link Byte}.
 */
public final class Natives {
    private Natives() {}

    // ----- runtime/stdio -----

    public static BigInteger runtime_stdio__stdout_write(List<?> data) {
        return write(System.out, data);
    }

    public static BigInteger runtime_stdio__stderr_write(List<?> data) {
        return write(System.err, data);
    }

    private static BigInteger write(PrintStream stream, List<?> data) {
        byte[] bytes = toBytes(data);
        stream.write(bytes, 0, bytes.length);
        stream.flush();
        return BigInteger.valueOf(bytes.length);
    }

    public static List<Object> runtime_stdio__stdin_read(BigInteger limit) {
        int count = limit.signum() <= 0 ? 0 : (limit.bitLength() > 31 ? Integer.MAX_VALUE : limit.intValue());
        byte[] buffer = new byte[Math.min(count, 65536)];
        try {
            int read = buffer.length == 0 ? 0 : System.in.read(buffer);
            return fromBytes(buffer, Math.max(read, 0));
        } catch (IOException failure) {
            throw Rt.trap("Cannot read standard input: " + failure.getMessage());
        }
    }

    public static List<Object> runtime_stdio__stdin_read_line() {
        ByteArrayOutputStream line = new ByteArrayOutputStream();
        InputStream in = System.in;
        try {
            int next;
            while ((next = in.read()) >= 0) {
                line.write(next);
                if (next == '\n') {
                    break;
                }
            }
        } catch (IOException failure) {
            throw Rt.trap("Cannot read standard input: " + failure.getMessage());
        }
        byte[] bytes = line.toByteArray();
        return fromBytes(bytes, bytes.length);
    }

    // ----- runtime/utf -----

    public static List<Object> runtime_utf__utf8_encode(String origin) {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        for (int offset = 0; offset < origin.length(); ) {
            int scalar = origin.codePointAt(offset);
            offset += Character.charCount(scalar);
            if (scalar < 0x80) {
                out.write(scalar);
            } else if (scalar < 0x800) {
                out.write(0xC0 | (scalar >> 6));
                out.write(0x80 | (scalar & 0x3F));
            } else if (scalar < 0x10000) {
                out.write(0xE0 | (scalar >> 12));
                out.write(0x80 | ((scalar >> 6) & 0x3F));
                out.write(0x80 | (scalar & 0x3F));
            } else {
                out.write(0xF0 | (scalar >> 18));
                out.write(0x80 | ((scalar >> 12) & 0x3F));
                out.write(0x80 | ((scalar >> 6) & 0x3F));
                out.write(0x80 | (scalar & 0x3F));
            }
        }
        byte[] bytes = out.toByteArray();
        return fromBytes(bytes, bytes.length);
    }

    public static BigInteger runtime_utf__utf8_valid_prefix(List<?> origin) {
        return BigInteger.valueOf(utf8(toBytes(origin), null));
    }

    public static String runtime_utf__utf8_decode(List<?> origin) {
        byte[] bytes = toBytes(origin);
        StringBuilder text = new StringBuilder();
        if (utf8(bytes, text) != bytes.length) {
            throw invalidData("UTF-8", "utf8_decode", 1);
        }
        return text.toString();
    }

    /** Длина корректного префикса UTF-8; {@code text} получает его скаляры. */
    private static int utf8(byte[] bytes, StringBuilder text) {
        int offset = 0;
        while (offset < bytes.length) {
            int lead = bytes[offset] & 0xFF;
            int length;
            int scalar;
            int min;
            if (lead < 0x80) {
                length = 1;
                scalar = lead;
                min = 0;
            } else if (lead >= 0xC2 && lead <= 0xDF) {
                length = 2;
                scalar = lead & 0x1F;
                min = 0x80;
            } else if (lead >= 0xE0 && lead <= 0xEF) {
                length = 3;
                scalar = lead & 0x0F;
                min = 0x800;
            } else if (lead >= 0xF0 && lead <= 0xF4) {
                length = 4;
                scalar = lead & 0x07;
                min = 0x10000;
            } else {
                return offset;
            }
            if (offset + length > bytes.length) {
                return offset;
            }
            for (int index = 1; index < length; index++) {
                int next = bytes[offset + index] & 0xFF;
                if ((next & 0xC0) != 0x80) {
                    return offset;
                }
                scalar = (scalar << 6) | (next & 0x3F);
            }
            if (scalar < min || scalar > 0x10FFFF || (scalar >= 0xD800 && scalar <= 0xDFFF)) {
                return offset;
            }
            if (text != null) {
                text.appendCodePoint(scalar);
            }
            offset += length;
        }
        return offset;
    }

    public static List<Object> runtime_utf__utf16_encode(String origin, String order) {
        boolean big = big(order, "utf16_encode");
        ArrayList<Object> out = new ArrayList<Object>();
        for (int index = 0; index < origin.length(); index++) {
            unit(out, origin.charAt(index), 2, big);
        }
        return out;
    }

    public static BigInteger runtime_utf__utf16_valid_prefix(List<?> origin, String order) {
        return BigInteger.valueOf(utf16(toBytes(origin), big(order, "utf16_valid_prefix"), null));
    }

    public static String runtime_utf__utf16_decode(List<?> origin, String order) {
        byte[] bytes = toBytes(origin);
        StringBuilder text = new StringBuilder();
        if (utf16(bytes, big(order, "utf16_decode"), text) != bytes.length) {
            throw invalidData("UTF-16" + (big(order, "utf16_decode") ? "BE" : "LE"), "utf16_decode", 2);
        }
        return text.toString();
    }

    private static int utf16(byte[] bytes, boolean big, StringBuilder text) {
        int offset = 0;
        while (offset < bytes.length) {
            if (offset + 2 > bytes.length) {
                return offset;
            }
            int first = read(bytes, offset, 2, big);
            if (first >= 0xDC00 && first <= 0xDFFF) {
                return offset;
            }
            if (first >= 0xD800 && first <= 0xDBFF) {
                if (offset + 4 > bytes.length) {
                    return offset;
                }
                int second = read(bytes, offset + 2, 2, big);
                if (second < 0xDC00 || second > 0xDFFF) {
                    return offset;
                }
                if (text != null) {
                    text.append((char) first).append((char) second);
                }
                offset += 4;
            } else {
                if (text != null) {
                    text.append((char) first);
                }
                offset += 2;
            }
        }
        return offset;
    }

    public static List<Object> runtime_utf__utf32_encode(String origin, String order) {
        boolean big = big(order, "utf32_encode");
        ArrayList<Object> out = new ArrayList<Object>();
        for (int offset = 0; offset < origin.length(); ) {
            int scalar = origin.codePointAt(offset);
            offset += Character.charCount(scalar);
            unit(out, scalar, 4, big);
        }
        return out;
    }

    public static BigInteger runtime_utf__utf32_valid_prefix(List<?> origin, String order) {
        return BigInteger.valueOf(utf32(toBytes(origin), big(order, "utf32_valid_prefix"), null));
    }

    public static String runtime_utf__utf32_decode(List<?> origin, String order) {
        byte[] bytes = toBytes(origin);
        StringBuilder text = new StringBuilder();
        if (utf32(bytes, big(order, "utf32_decode"), text) != bytes.length) {
            throw invalidData("UTF-32" + (big(order, "utf32_decode") ? "BE" : "LE"), "utf32_decode", 2);
        }
        return text.toString();
    }

    private static int utf32(byte[] bytes, boolean big, StringBuilder text) {
        int offset = 0;
        while (offset + 4 <= bytes.length) {
            long scalar = read(bytes, offset, 4, big) & 0xFFFFFFFFL;
            if (scalar > 0x10FFFF || (scalar >= 0xD800 && scalar <= 0xDFFF)) {
                return offset;
            }
            if (text != null) {
                text.appendCodePoint((int) scalar);
            }
            offset += 4;
        }
        return offset;
    }

    // ----- runtime/process -----

    /**
     * Аргументы программы как байты. {@code OKKAMA_ARGV_HEX} (через запятую,
     * каждый — {@code x} и hex) сохраняет исходные байты, когда локаль JVM их
     * исказила бы; иначе — UTF-8 строк {@code main(String[])}.
     */
    public static List<Object> runtime_process__process_arguments() {
        ArrayList<Object> out = new ArrayList<Object>();
        String hex = System.getenv("OKKAMA_ARGV_HEX");
        if (hex != null) {
            if (hex.isEmpty()) {
                return out;
            }
            for (String part : hex.split(",", -1)) {
                part = part.substring(1);
                byte[] bytes = new byte[part.length() / 2];
                for (int index = 0; index < bytes.length; index++) {
                    bytes[index] = (byte) Integer.parseInt(part.substring(2 * index, 2 * index + 2), 16);
                }
                out.add(fromBytes(bytes, bytes.length));
            }
            return out;
        }
        for (String argument : Rt.argv) {
            byte[] bytes = argument.getBytes(java.nio.charset.StandardCharsets.UTF_8);
            out.add(fromBytes(bytes, bytes.length));
        }
        return out;
    }

    // ----- runtime/input_state -----

    /** Буфер UTF-8 ввода: байты, признак конца и закрытия. */
    static final class InputState {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        boolean eof;
        boolean closed;
    }

    private static InputState input(Object handle, boolean allowClosed, String name, int arity) {
        InputState state = (InputState) handle;
        if (state.closed && !allowClosed) {
            throw NativeFailure.raise("core.input_state", "already_closed",
                "runtime/input_state@1::" + name + "/" + arity, "UTF-8 input is already closed");
        }
        return state;
    }

    public static Object runtime_input_state__input_state_create() {
        return new InputState();
    }

    public static BigInteger runtime_input_state__input_state_append(Object handle, List<?> value) {
        byte[] bytes = toBytes(value);
        input(handle, false, "input_state_append", 2).bytes.write(bytes, 0, bytes.length);
        return BigInteger.valueOf(bytes.length);
    }

    public static BigInteger runtime_input_state__input_state_size(Object handle) {
        return BigInteger.valueOf(input(handle, false, "input_state_size", 1).bytes.size());
    }

    public static BigInteger runtime_input_state__input_state_line_offset(Object handle) {
        byte[] bytes = input(handle, false, "input_state_line_offset", 1).bytes.toByteArray();
        for (int index = 0; index < bytes.length; index++) {
            if (bytes[index] == '\n') {
                return BigInteger.valueOf(index);
            }
        }
        return BigInteger.valueOf(-1);
    }

    public static List<Object> runtime_input_state__input_state_take(Object handle, BigInteger size) {
        InputState state = input(handle, false, "input_state_take", 2);
        byte[] bytes = state.bytes.toByteArray();
        if (size.signum() < 0 || size.compareTo(BigInteger.valueOf(bytes.length)) > 0) {
            throw NativeFailure.raise("core.input_state", "invalid_input",
                "runtime/input_state@1::input_state_take/2", "Input-state take size is invalid");
        }
        int count = size.intValue();
        state.bytes = new ByteArrayOutputStream();
        state.bytes.write(bytes, count, bytes.length - count);
        return fromBytes(bytes, count);
    }

    public static boolean runtime_input_state__input_state_eof(Object handle) {
        return input(handle, false, "input_state_eof", 1).eof;
    }

    public static boolean runtime_input_state__input_state_finish(Object handle) {
        InputState state = input(handle, false, "input_state_finish", 1);
        boolean changed = !state.eof;
        state.eof = true;
        return changed;
    }

    public static boolean runtime_input_state__input_state_closed(Object handle) {
        return input(handle, true, "input_state_closed", 1).closed;
    }

    public static boolean runtime_input_state__input_state_close(Object handle) {
        InputState state = input(handle, true, "input_state_close", 1);
        if (state.closed) {
            return false;
        }
        state.bytes = new ByteArrayOutputStream();
        state.eof = true;
        state.closed = true;
        return true;
    }

    // ----- общее -----

    /** Порядок байтов {@code "big"}/{@code "little"}; иное — сбой вызова, как в Perl. */
    private static boolean big(String order, String name) {
        if ("big".equals(order)) {
            return true;
        }
        if ("little".equals(order)) {
            return false;
        }
        throw Rt.trap("Native invocation \"runtime/utf@1::" + name + "/2\" failed");
    }

    private static int read(byte[] bytes, int offset, int width, boolean big) {
        int value = 0;
        for (int index = 0; index < width; index++) {
            int octet = bytes[offset + (big ? index : width - 1 - index)] & 0xFF;
            value = (value << 8) | octet;
        }
        return value;
    }

    private static void unit(List<Object> out, int value, int width, boolean big) {
        for (int index = 0; index < width; index++) {
            int shift = 8 * (big ? width - 1 - index : index);
            out.add(Byte.valueOf((byte) (value >>> shift)));
        }
    }

    /** Ожидаемый сбой provider {@code core.utf}: байты не в кодировке {@code encoding}. */
    private static Failure invalidData(String encoding, String name, int arity) {
        return NativeFailure.raise("core.utf", "invalid_data",
            "runtime/utf@1::" + name + "/" + arity, "Bytes are not valid " + encoding);
    }

    static byte[] toBytes(List<?> data) {
        byte[] bytes = new byte[data.size()];
        for (int index = 0; index < bytes.length; index++) {
            bytes[index] = (Byte) data.get(index);
        }
        return bytes;
    }

    static List<Object> fromBytes(byte[] bytes, int length) {
        ArrayList<Object> out = new ArrayList<Object>(length);
        for (int index = 0; index < length; index++) {
            out.add(Byte.valueOf(bytes[index]));
        }
        return out;
    }

    /**
     * Ошибка provider ({@code die} Perl-native): программная ошибка
     * {@code Native invocation "<символ>" failed}; символ — по вызванному
     * методу-provider на стеке ({@code runtime_<модуль>__<имя>}).
     */
    static Failure died() {
        for (StackTraceElement frame : new Throwable().getStackTrace()) {
            String method = frame.getMethodName();
            int split = method.indexOf("__");
            if (!frame.getClassName().startsWith("okkama.rt.") || !method.startsWith("runtime_") || split < 0) {
                continue;
            }
            int arity = 0;
            try {
                for (java.lang.reflect.Method candidate : Class.forName(frame.getClassName()).getMethods()) {
                    if (candidate.getName().equals(method)) {
                        arity = candidate.getParameterTypes().length;
                    }
                }
            } catch (ClassNotFoundException unknown) {
                arity = 0;
            }
            String symbol = "runtime/" + method.substring("runtime_".length(), split) + "@1::"
                + method.substring(split + 2) + "/" + arity;
            return Rt.trap("Native invocation \"" + symbol + "\" failed");
        }
        return Rt.trap("Native invocation failed");
    }
}
