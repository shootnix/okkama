package okkama.rt;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

/**
 * Рантайм JVM backend Okkama: проверки и операции, которые OIR-1 описывает
 * как контракты инструкций (§12, §16). Тексты программных ошибок совпадают с
 * Perl-интерпретатором. Значения: {@code int} — {@link BigInteger},
 * {@code bool} — {@code boolean} (в коллекциях {@link Boolean}), {@code byte}
 * — {@code int} 0..255 (в коллекциях {@link Byte}), {@code char} — {@code int}
 * code point (в коллекциях {@link Integer}), {@code str}
 * — {@link String}, массив — неизменяемо используемый {@link List},
 * словарь — {@link TreeMap} в порядке {@link KeyOrder}.
 */
public final class Rt {
    private Rt() {}

    private static final BigInteger MAX_BYTE = BigInteger.valueOf(255);

    // ----- отказы -----

    public static Failure trap(String message) {
        return new Failure(Failure.PROGRAM, message, null);
    }

    public static Failure panic(String message) {
        return new Failure(Failure.PANIC, message, null);
    }

    public static Failure thrown(Object error) {
        return new Failure(Failure.THROWN, null, error);
    }

    /** Текст диагностики как у Perl: {@code <Тип>: <text()>}, {@code Panic: …}, {@code RuntimeError: …}. */
    public static String describe(Failure failure) {
        if (failure.error instanceof NativeFailure) {
            NativeFailure native_ = (NativeFailure) failure.error;
            return "NativeError: " + native_.message + "\ncode: " + native_.code
                + "\ndomain: " + native_.domain + "\noperation: " + native_.operation;
        }
        if (failure.kind == Failure.THROWN) {
            return typeName(failure.error) + ": " + ((Error) failure.error).text();
        }
        if (failure.kind == Failure.PANIC) {
            return "Panic: " + failure.getMessage();
        }
        return "RuntimeError: " + failure.getMessage();
    }

    /** Новый типизированный отказ в теле catch получает причину — обрабатываемый (§6.3). */
    public static Failure caused(Failure fresh, Failure handled) {
        if (fresh.kind != Failure.THROWN || fresh == handled) {
            return fresh;
        }
        Failure last = fresh;
        while (last.cause != null) {
            if (last.cause == handled) {
                return fresh;
            }
            last = last.cause;
        }
        last.cause = handled;
        return fresh;
    }

    /** Отказ cleanup при уже идущем отказе становится вторичным. */
    public static Failure secondary(Failure primary, Failure cleanup) {
        primary.secondary.add(cleanup);
        return primary;
    }

    /** Результат {@code main}: только {@code 0..255}. */
    public static void exit(BigInteger status) {
        if (status.signum() < 0 || status.compareTo(MAX_BYTE) > 0) {
            report(trap("main() must return an int in 0..255, got " + status));
        }
        System.out.flush();
        System.exit(status.intValue());
    }

    /** Имя Okkama-типа Error-объекта: последний сегмент имени класса. */
    private static String typeName(Object error) {
        String name = error.getClass().getName();
        return name.substring(Math.max(name.lastIndexOf('$'), name.lastIndexOf('.')) + 1);
    }

    /** Аргументы командной строки программы (запоминает launcher). */
    static String[] argv = new String[0];

    /**
     * Стек потока программы: глубокая (нехвостовая) рекурсия ограничена
     * памятью, как у исполнителей IR и Sparrow с кадрами в куче, а не 8 МБ
     * стека главного потока JVM. Память стека выделяется по мере роста.
     */
    static final long PROGRAM_STACK = 4L << 30;

    /** Исполнить {@code body} в потоке программы с большим стеком. */
    static void onProgramStack(final Runnable body) {
        final Throwable[] escaped = new Throwable[1];
        Thread thread = new Thread(null, new Runnable() {
            public void run() {
                try {
                    body.run();
                } catch (Throwable other) {
                    escaped[0] = other;
                }
            }
        }, "main", PROGRAM_STACK);
        thread.start();
        boolean interrupted = false;
        while (true) {
            try {
                thread.join();
                break;
            } catch (InterruptedException interruption) {
                interrupted = true;
            }
        }
        if (interrupted) {
            Thread.currentThread().interrupt();
        }
        if (escaped[0] != null) {
            Rt.<RuntimeException>sneaky(escaped[0]);
        }
    }

    @SuppressWarnings("unchecked")
    private static <T extends Throwable> void sneaky(Throwable error) throws T {
        throw (T) error;
    }

    /**
     * Запуск программы: аргументы, обслуживание воркера, точка входа
     * {@code entry} класса {@code main} (у {@code async main} — корневой
     * Future и цикл планировщика), код выхода или диагностика отказа.
     */
    public static void launch(final String[] args, final Class<?> main, final String entry, final boolean async) {
        onProgramStack(new Runnable() {
            public void run() {
                try {
                    argv = args;
                    Worker.mainClass = main.getName();
                    if (Worker.active()) {
                        Worker.serve();
                    }
                    java.lang.reflect.Method method = main.getDeclaredMethod(entry);
                    method.setAccessible(true);
                    Object result;
                    try {
                        result = method.invoke(null);
                    } catch (java.lang.reflect.InvocationTargetException invocation) {
                        Rt.<RuntimeException>sneaky(invocation.getCause());
                        return;
                    }
                    if (async) {
                        result = Scheduler.runMain((Future) result);
                    }
                    exit((BigInteger) result);
                } catch (Failure failure) {
                    report(failure);
                } catch (NoSuchMethodException | IllegalAccessException missing) {
                    throw new IllegalStateException("program entry is missing: " + entry, missing);
                }
            }
        });
    }

    public static void start(String[] args) {
        argv = args;
        Worker.mainClass = new Throwable().getStackTrace()[1].getClassName();
        if (Worker.active()) {
            Worker.serve();
        }
    }

    /** {@code str.utf8}: строгий UTF-8 (строка Okkama — корректные скаляры). */
    public static java.util.List<Object> utf8(String text) {
        return Natives.runtime_utf__utf8_encode(text);
    }

    /** Неперехваченный отказ: диагностика в stderr и код 1. */
    public static void report(Failure failure) {
        System.out.flush();
        recordFailure(failure);
        System.err.println(describe(failure));
        for (Failure secondary : failure.secondary) {
            System.err.println("Secondary cleanup failure:");
            System.err.println("  " + describe(secondary));
        }
        System.exit(1);
    }

    /**
     * Для инструментов (адаптер conformance): если задан
     * {@code OKKAMA_FAILURE_REPORT}, в этот файл пишется строка
     * {@code вид<TAB>тип<TAB>сообщение}: вид — {@code runtime}, {@code panic}
     * или {@code uncaught-error}; тип — имя Error-объекта или пусто.
     */
    private static void recordFailure(Failure failure) {
        String path = System.getenv("OKKAMA_FAILURE_REPORT");
        if (path == null || path.isEmpty()) {
            return;
        }
        String kind;
        String type = "";
        String message;
        if (failure.kind == Failure.THROWN) {
            kind = "uncaught-error";
            type = typeName(failure.error);
            message = ((Error) failure.error).text();
        } else if (failure.kind == Failure.PANIC) {
            kind = "panic";
            message = failure.getMessage();
        } else {
            kind = "runtime";
            message = failure.getMessage();
        }
        try {
            java.io.Writer out = new java.io.OutputStreamWriter(
                new java.io.FileOutputStream(path), "UTF-8");
            out.write(kind + "\t" + type + "\t" + message.replace('\n', ' '));
            out.close();
        } catch (java.io.IOException ignored) {
            // Отчёт — вспомогательный: диагностика в stderr остаётся главной.
        }
    }

    /** {@code role.narrow}: значение делает целевую роль, иначе программная ошибка. */
    public static Object narrow(Object value, Class<?> role, String message) {
        if (!role.isInstance(value)) {
            throw trap(message);
        }
        return value;
    }

    // ----- int -----

    /** Остаток {@code %} усечённого деления, как в Go (D-031): знак делимого. */
    public static BigInteger mod(BigInteger left, BigInteger right) {
        if (right.signum() == 0) {
            throw trap("Modulo by zero");
        }
        return left.remainder(right);
    }

    /** Слово signed-64 для битовых операций. */
    public static long word(BigInteger value) {
        if (value.bitLength() > 63) {
            throw trap("Bitwise operand is outside signed-64 range");
        }
        return value.longValue();
    }

    private static int shiftCount(BigInteger count) {
        if (count.bitLength() > 31 || count.intValue() < 0 || count.intValue() > 63) {
            throw trap("Shift count must be in 0..63");
        }
        return count.intValue();
    }

    public static BigInteger and(BigInteger left, BigInteger right) {
        return BigInteger.valueOf(word(left) & word(right));
    }

    public static BigInteger or(BigInteger left, BigInteger right) {
        return BigInteger.valueOf(word(left) | word(right));
    }

    public static BigInteger xor(BigInteger left, BigInteger right) {
        return BigInteger.valueOf(word(left) ^ word(right));
    }

    public static BigInteger not(BigInteger value) {
        return BigInteger.valueOf(~word(value));
    }

    public static BigInteger shl(BigInteger value, BigInteger count) {
        long word = word(value);
        return BigInteger.valueOf(word << shiftCount(count));
    }

    public static BigInteger shr(BigInteger value, BigInteger count) {
        long word = word(value);
        return BigInteger.valueOf(word >> shiftCount(count));
    }

    public static BigInteger ushr(BigInteger value, BigInteger count) {
        long word = word(value);
        return BigInteger.valueOf(word >>> shiftCount(count));
    }

    public static BigInteger sign(BigInteger value) {
        return BigInteger.valueOf(value.signum());
    }

    /** Коды {@code BitOp} OIR в порядке объявления. */
    private static final int BIT_COUNT = 0;
    private static final int LEADING_ZEROS = 1;
    private static final int TRAILING_ZEROS = 2;
    private static final int ROTATE_LEFT = 3;
    private static final int ROTATE_RIGHT = 4;
    private static final int REVERSE_BITS = 5;

    /** Битовое действие над словом signed-64; {@code count} — у поворотов. */
    public static BigInteger intBits(int op, BigInteger value, BigInteger count) {
        long word = word(value);
        switch (op) {
            case BIT_COUNT:
                return BigInteger.valueOf(Long.bitCount(word));
            case LEADING_ZEROS:
                return BigInteger.valueOf(Long.numberOfLeadingZeros(word));
            case TRAILING_ZEROS:
                return BigInteger.valueOf(Long.numberOfTrailingZeros(word));
            case ROTATE_LEFT:
                return BigInteger.valueOf(Long.rotateLeft(word, shiftCount(count)));
            case ROTATE_RIGHT:
                return BigInteger.valueOf(Long.rotateRight(word, shiftCount(count)));
            case REVERSE_BITS:
                return BigInteger.valueOf(Long.reverse(word));
            default:
                return BigInteger.valueOf(Long.reverseBytes(word));
        }
    }

    // ----- byte -----

    /** {@code byte(x)} из {@code int}: только 0..255; {@code context} — префикс текста. */
    public static int toByte(BigInteger value, String context) {
        if (value.signum() < 0 || value.compareTo(MAX_BYTE) > 0) {
            String message = "Cannot convert int value " + value + " to byte: expected 0..255";
            throw trap(context == null ? message : context + ": " + message);
        }
        return value.intValue();
    }

    private static int byteCount(BigInteger count) {
        if (count.signum() < 0 || count.compareTo(BigInteger.valueOf(7)) > 0) {
            throw trap("Shift count must be in 0..7");
        }
        return count.intValue();
    }

    public static int byteShl(int value, BigInteger count) {
        return (value << byteCount(count)) & 0xFF;
    }

    public static int byteShr(int value, BigInteger count) {
        return value >> byteCount(count);
    }

    public static int byteUshr(int value, BigInteger count) {
        return value >>> byteCount(count);
    }

    /** Битовое действие над словом 8 бит; счётчики — число, остальные — byte. */
    public static int byteBits(int op, int value, BigInteger count) {
        switch (op) {
            case BIT_COUNT:
                return Integer.bitCount(value);
            case LEADING_ZEROS:
                return Integer.numberOfLeadingZeros(value) - 24;
            case TRAILING_ZEROS:
                return value == 0 ? 8 : Integer.numberOfTrailingZeros(value);
            case ROTATE_LEFT: {
                int amount = byteCount(count);
                return ((value << amount) | (value >>> (8 - amount))) & 0xFF;
            }
            case ROTATE_RIGHT: {
                int amount = byteCount(count);
                return ((value >>> amount) | (value << (8 - amount))) & 0xFF;
            }
            case REVERSE_BITS:
                return Integer.reverse(value) >>> 24;
            default:
                // Слово из одного байта: перестановка байтов его не меняет.
                return value;
        }
    }

    // ----- массивы -----

    public static BigInteger size(List<?> array) {
        return BigInteger.valueOf(array.size());
    }

    public static boolean inBounds(List<?> array, BigInteger index) {
        return index.signum() >= 0 && index.bitLength() <= 31 && index.intValue() < array.size();
    }

    public static Object index(List<?> array, BigInteger index) {
        if (!inBounds(array, index)) {
            throw trap("Array index out of bounds");
        }
        return array.get(index.intValue());
    }

    /** Порядок проверок и тексты — как {@code _validate_slice_bounds} Perl. */
    private static void sliceBounds(BigInteger start, BigInteger end, int length) {
        if (start.signum() < 0) {
            throw trap("Slice start must not be negative");
        }
        if (start.compareTo(end) > 0) {
            throw trap("Slice start must be less than or equal to slice end");
        }
        if (end.compareTo(BigInteger.valueOf(length)) > 0) {
            throw trap("Slice end must not exceed length");
        }
    }

    public static List<Object> slice(List<?> array, BigInteger start, BigInteger end) {
        sliceBounds(start, end, array.size());
        return new ArrayList<Object>(array.subList(start.intValue(), end.intValue()));
    }

    public static List<Object> appendRange(
            List<Object> builder, BigInteger start, BigInteger end, boolean inclusive) {
        if (start.compareTo(end) > 0) {
            throw trap("Range start must be less than or equal to range end");
        }
        for (BigInteger value = start;
                inclusive ? value.compareTo(end) <= 0 : value.compareTo(end) < 0;
                value = value.add(BigInteger.ONE)) {
            builder.add(value);
        }
        return builder;
    }

    // ----- строки (по code points) -----

    public static BigInteger length(String text) {
        return BigInteger.valueOf(text.codePointCount(0, text.length()));
    }

    public static String slice(String text, BigInteger start, BigInteger end) {
        sliceBounds(start, end, text.codePointCount(0, text.length()));
        int from = text.offsetByCodePoints(0, start.intValue());
        int to = text.offsetByCodePoints(0, end.intValue());
        return text.substring(from, to);
    }

    /** Буквальный split; пустой разделитель — по code points; пустые части сохраняются. */
    public static List<Object> split(String text, String separator) {
        ArrayList<Object> parts = new ArrayList<Object>();
        if (separator.isEmpty()) {
            int offset = 0;
            while (offset < text.length()) {
                int next = offset + Character.charCount(text.codePointAt(offset));
                parts.add(text.substring(offset, next));
                offset = next;
            }
            return parts;
        }
        int offset = 0;
        while (true) {
            int found = text.indexOf(separator, offset);
            if (found < 0) {
                parts.add(text.substring(offset));
                return parts;
            }
            parts.add(text.substring(offset, found));
            offset = found + separator.length();
        }
    }

    private static String join(String separator, List<Object> parts) {
        StringBuilder joined = new StringBuilder();
        for (int index = 0; index < parts.size(); index++) {
            if (index > 0) {
                joined.append(separator);
            }
            joined.append((String) parts.get(index));
        }
        return joined.toString();
    }

    /** Как {@code _replace_literal} Perl: пустая цель — границы code points. */
    public static String replace(String text, String target, String replacement) {
        if (target.isEmpty()) {
            if (text.isEmpty()) {
                return replacement;
            }
            return replacement + join(replacement, split(text, "")) + replacement;
        }
        return join(replacement, split(text, target));
    }

    public static String replaceFirst(String text, String target, String replacement) {
        if (target.isEmpty()) {
            return replacement + text;
        }
        int found = text.indexOf(target);
        if (found < 0) {
            return text;
        }
        return text.substring(0, found) + replacement + text.substring(found + target.length());
    }

    /** Пробельные символы {@code \s} Perl для Unicode-строк. */
    private static boolean space(int codePoint) {
        switch (codePoint) {
            case 0x09: case 0x0A: case 0x0B: case 0x0C: case 0x0D: case 0x20:
            case 0x85: case 0xA0: case 0x1680:
            case 0x2028: case 0x2029: case 0x202F: case 0x205F: case 0x3000:
                return true;
            default:
                return codePoint >= 0x2000 && codePoint <= 0x200A;
        }
    }

    public static String trim(String text) {
        int start = 0;
        int end = text.length();
        while (start < end && space(text.codePointAt(start))) {
            start += Character.charCount(text.codePointAt(start));
        }
        while (end > start && space(text.codePointBefore(end))) {
            end -= Character.charCount(text.codePointBefore(end));
        }
        return text.substring(start, end);
    }

    /** Ровно один завершающий CRLF или LF; одиночный CR остаётся (D-033). */
    public static String chomp(String text) {
        if (text.endsWith("\r\n")) {
            return text.substring(0, text.length() - 2);
        }
        if (text.endsWith("\n")) {
            return text.substring(0, text.length() - 1);
        }
        return text;
    }

    public static String joinArray(List<?> array, String separator) {
        StringBuilder joined = new StringBuilder();
        for (int index = 0; index < array.size(); index++) {
            if (index > 0) {
                joined.append(separator);
            }
            joined.append((String) array.get(index));
        }
        return joined.toString();
    }

    // ----- словари -----

    public static Map<Object, Object> newMap() {
        return new TreeMap<Object, Object>(KeyOrder.INSTANCE);
    }

    public static BigInteger size(Map<?, ?> map) {
        return BigInteger.valueOf(map.size());
    }

    /** Значение по ключу; отсутствующий ключ — ошибка с ключом как у Perl. */
    public static Object get(Map<?, ?> map, Object key) {
        Object value = map.get(key);
        if (value == null) {
            throw trap("Map key not found: " + formatKey(key));
        }
        return value;
    }

    /** Ключ в диагностике: строка в кавычках, {@code bool} — 1/0, число — цифрами. */
    private static String formatKey(Object key) {
        if (key instanceof String) {
            return "\"" + key + "\"";
        }
        if (key instanceof Boolean) {
            return ((Boolean) key) ? "1" : "0";
        }
        if (key instanceof Byte) {
            return Integer.toString(((Byte) key) & 0xFF);
        }
        if (key instanceof Double) {
            return Floats.text((Double) key);
        }
        if (key instanceof Case) {
            return "\"" + ((Case) key).name + "\"";
        }
        if (key instanceof Integer) {
            // char: как строка из одного символа.
            return "\"" + new String(Character.toChars((Integer) key)) + "\"";
        }
        return key.toString();
    }

    public static List<Object> keys(Map<?, ?> map) {
        return new ArrayList<Object>(map.keySet());
    }

    public static List<Object> values(Map<?, ?> map) {
        return new ArrayList<Object>(map.values());
    }
}
