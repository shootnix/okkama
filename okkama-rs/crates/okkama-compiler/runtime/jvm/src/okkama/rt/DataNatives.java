package okkama.rt;

import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Natives форматов данных: {@code runtime/io_decorators},
 * {@code runtime/testing}, {@code runtime/csv_state}, {@code runtime/pem},
 * {@code runtime/template}, {@code runtime/ini}. Смещения и длины — в
 * символах (code points), тексты отказов — как у Perl-provider.
 */
public final class DataNatives {
    private DataNatives() {}

    private static Failure fail(String domain, String code, String symbol, String message) {
        return NativeFailure.raise(domain, code, symbol, message);
    }

    private static int[] points(String text) {
        return text.codePoints().toArray();
    }

    private static String text(int[] points, int from, int to) {
        return new String(points, from, to - from);
    }

    /** Есть ли одиночный суррогат (не скалярное значение Unicode); его индекс в code points. */
    private static int lone(String text) {
        int index = 0;
        for (int offset = 0; offset < text.length(); offset++, index++) {
            char unit = text.charAt(offset);
            if (Character.isHighSurrogate(unit) && offset + 1 < text.length()
                    && Character.isLowSurrogate(text.charAt(offset + 1))) {
                offset++;
            } else if (Character.isSurrogate(unit)) {
                return index;
            }
        }
        return -1;
    }

    // ---- runtime/io_decorators ----

    static final class Counter {
        BigInteger value;
    }

    public static Object runtime_io_decorators__limited_reading_state_create(BigInteger limit) {
        if (limit.signum() < 0) {
            throw fail("core.io_decorators", "invalid_state", "runtime/io_decorators@1::limited_reading_state_create/1",
                "Limited reading state requires a non-negative limit");
        }
        Counter counter = new Counter();
        counter.value = limit;
        return counter;
    }

    public static BigInteger runtime_io_decorators__limited_reading_state_remaining(Object handle) {
        return ((Counter) handle).value;
    }

    public static BigInteger runtime_io_decorators__limited_reading_state_consume(Object handle, BigInteger amount) {
        Counter counter = (Counter) handle;
        if (amount.signum() < 0 || amount.compareTo(counter.value) > 0) {
            throw fail("core.io_decorators", "invalid_state", "runtime/io_decorators@1::limited_reading_state_consume/2",
                "Limited reading consumption exceeds remaining bytes");
        }
        counter.value = counter.value.subtract(amount);
        return counter.value;
    }

    public static Object runtime_io_decorators__chained_reading_state_create() {
        Counter counter = new Counter();
        counter.value = BigInteger.ZERO;
        return counter;
    }

    public static BigInteger runtime_io_decorators__chained_reading_state_position(Object handle) {
        return ((Counter) handle).value;
    }

    public static BigInteger runtime_io_decorators__chained_reading_state_advance(Object handle) {
        Counter counter = (Counter) handle;
        counter.value = counter.value.add(BigInteger.ONE);
        return counter.value;
    }

    // ---- runtime/testing ----

    static final class TestState {
        long limit;
        long size;
        boolean closed;
    }

    private static void requireTest(String name, int arity) {
        if (Tests.current == null) {
            throw fail("core.testing", "permission_denied", "runtime/testing@1::" + name + "/" + arity,
                "Test state requires a running case");
        }
    }

    private static TestState testState(Object handle, String name) {
        requireTest(name, 1);
        TestState state = (TestState) handle;
        if (state.closed) {
            throw fail("core.testing", "already_closed", "runtime/testing@1::" + name + "/1",
                "Native handle is already closed");
        }
        return state;
    }

    public static Object runtime_testing__test_state_open(BigInteger limit) {
        requireTest("test_state_open", 1);
        if (limit.signum() < 0 || limit.compareTo(BigInteger.valueOf(65536)) > 0) {
            throw fail("core.testing", "invalid_input", "runtime/testing@1::test_state_open/1",
                "Test state capacity must be 0..65536");
        }
        TestState state = new TestState();
        state.limit = limit.longValue();
        return state;
    }

    public static BigInteger runtime_testing__test_state_size(Object handle) {
        return BigInteger.valueOf(testState(handle, "test_state_size").size);
    }

    public static BigInteger runtime_testing__test_state_next(Object handle) {
        TestState state = testState(handle, "test_state_next");
        if (state.size >= state.limit) {
            throw fail("core.testing", "resource_exhausted", "runtime/testing@1::test_state_next/1",
                "Test state capacity exhausted");
        }
        return BigInteger.valueOf(state.size++);
    }

    public static boolean runtime_testing__test_state_close(Object handle) {
        requireTest("test_state_close", 1);
        ((TestState) handle).closed = true;
        return true;
    }

    public static boolean runtime_testing__test_context_active() {
        return Tests.current != null;
    }

    // ---- runtime/csv_state ----

    static final class Csv {
        final long[] limits;
        byte[] buffer = new byte[0];
        long fed;
        long bytes;
        long scalars;
        long rows;
        StringBuilder field = new StringBuilder();
        long fieldLength;
        ArrayList<String> fields = new ArrayList<String>();
        String mode = "start";
        boolean active;
        boolean eof;
        String status = "need";
        long offset;

        Csv(long[] limits) {
            this.limits = limits;
        }

        long request() {
            long remaining = limits[0] - fed;
            return remaining >= 4096 ? 4096 : remaining >= 0 ? remaining + 1 : 1;
        }

        String error(String kind, Long at) {
            status = kind;
            offset = at == null ? scalars : at;
            buffer = new byte[0];
            field = new StringBuilder();
            fieldLength = 0;
            fields = new ArrayList<String>();
            return kind;
        }

        String append(int point) {
            if (fieldLength >= limits[2]) {
                return error("field", null);
            }
            field.appendCodePoint(point);
            fieldLength++;
            return null;
        }

        String finishField() {
            if (fields.size() >= limits[4]) {
                return error("columns", null);
            }
            fields.add(field.toString());
            field = new StringBuilder();
            fieldLength = 0;
            return null;
        }

        String record() {
            if (rows >= limits[3]) {
                return error("rows", null);
            }
            rows++;
            active = false;
            mode = "start";
            return status = "row";
        }

        String advance() {
            if (!status.equals("need")) {
                return status;
            }
            while (true) {
                if (buffer.length == 0) {
                    if (!eof) {
                        return "need";
                    }
                    if (mode.equals("quoted") || mode.equals("cr")) {
                        return error("syntax", null);
                    }
                    if (active) {
                        if (finishField() != null) {
                            return status;
                        }
                        return record();
                    }
                    return status = "eof";
                }
                if (bytes >= limits[0]) {
                    return error("bytes", null);
                }
                int lead = buffer[0] & 0xff;
                int size = lead < 0x80 ? 1 : lead >= 0xc2 && lead <= 0xdf ? 2 : lead >= 0xe0 && lead <= 0xef ? 3
                    : lead >= 0xf0 && lead <= 0xf4 ? 4 : 0;
                if (size == 0) {
                    return error("utf8", bytes);
                }
                if (buffer.length < size) {
                    if (fed > limits[0]) {
                        return error("bytes", null);
                    }
                    if (eof) {
                        return error("utf8", bytes);
                    }
                    return "need";
                }
                if (size > limits[0] - bytes) {
                    return error("bytes", null);
                }
                int point;
                try {
                    String decoded = StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
                        .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(buffer, 0, size))
                        .toString();
                    point = decoded.codePointAt(0);
                } catch (CharacterCodingException invalid) {
                    return error("utf8", bytes);
                }
                if (scalars >= limits[1]) {
                    return error("input", null);
                }
                byte[] rest = new byte[buffer.length - size];
                System.arraycopy(buffer, size, rest, 0, rest.length);
                buffer = rest;
                bytes += size;
                if (mode.equals("cr")) {
                    if (point != '\n') {
                        return error("syntax", null);
                    }
                    scalars++;
                    return record();
                }
                if (mode.equals("quoted")) {
                    if (point == '"') {
                        mode = "closed";
                    } else if (append(point) != null) {
                        return status;
                    }
                } else if (mode.equals("closed") && point == '"') {
                    if (append(point) != null) {
                        return status;
                    }
                    mode = "quoted";
                } else if (point == ',') {
                    if (fields.size() + 1 >= limits[4]) {
                        return error("columns", null);
                    }
                    if (finishField() != null) {
                        return status;
                    }
                    mode = "start";
                    active = true;
                } else if (point == '\r') {
                    if (finishField() != null) {
                        return status;
                    }
                    mode = "cr";
                } else if (point == '\n' || mode.equals("closed")) {
                    return error("syntax", null);
                } else if (point == '"') {
                    if (!mode.equals("start")) {
                        return error("syntax", null);
                    }
                    mode = "quoted";
                    active = true;
                } else {
                    if (append(point) != null) {
                        return status;
                    }
                    mode = "plain";
                    active = true;
                }
                scalars++;
            }
        }
    }

    private static Csv csv(Object handle, String name) {
        return (Csv) handle;
    }

    private static Failure csvInvalid(String name, int arity) {
        return fail("core.csv_state", "invalid_state", "runtime/csv_state@1::" + name + "/" + arity,
            "Invalid CSV state operation");
    }

    public static Object runtime_csv_state__csv_state_create(BigInteger a, BigInteger b, BigInteger c, BigInteger d,
            BigInteger e) {
        BigInteger[] values = {a, b, c, d, e};
        long[] limits = new long[5];
        for (int index = 0; index < 5; index++) {
            if (values[index].signum() < 0) {
                throw fail("core.csv_state", "invalid_input", "runtime/csv_state@1::csv_state_create/5",
                    "CSV limits must be nonnegative");
            }
            limits[index] = values[index].bitLength() > 62 ? Long.MAX_VALUE / 2 : values[index].longValue();
        }
        return new Csv(limits);
    }

    public static boolean runtime_csv_state__csv_state_feed(Object handle, List<?> value) {
        Csv state = csv(handle, "csv_state_feed");
        byte[] bytes = Natives.toBytes(value);
        if (!state.status.equals("need") || bytes.length > state.request() || state.eof) {
            throw csvInvalid("csv_state_feed", 2);
        }
        state.fed += bytes.length;
        byte[] joined = new byte[state.buffer.length + bytes.length];
        System.arraycopy(state.buffer, 0, joined, 0, state.buffer.length);
        System.arraycopy(bytes, 0, joined, state.buffer.length, bytes.length);
        state.buffer = joined;
        if (bytes.length == 0) {
            state.eof = true;
        }
        return true;
    }

    public static String runtime_csv_state__csv_state_advance(Object handle) {
        return csv(handle, "csv_state_advance").advance();
    }

    public static List<Object> runtime_csv_state__csv_state_row(Object handle) {
        Csv state = csv(handle, "csv_state_row");
        if (!state.status.equals("row")) {
            throw csvInvalid("csv_state_row", 1);
        }
        List<Object> row = new ArrayList<Object>(state.fields);
        state.fields = new ArrayList<String>();
        state.status = "need";
        return row;
    }

    public static BigInteger runtime_csv_state__csv_state_offset(Object handle) {
        return BigInteger.valueOf(csv(handle, "csv_state_offset").offset);
    }

    public static BigInteger runtime_csv_state__csv_state_request(Object handle) {
        return BigInteger.valueOf(csv(handle, "csv_state_request").request());
    }

    public static boolean runtime_csv_state__csv_state_closed(Object handle) {
        return csv(handle, "csv_state_closed").status.equals("closed");
    }

    public static boolean runtime_csv_state__csv_state_close(Object handle) {
        csv(handle, "csv_state_close").error("closed", 0L);
        return true;
    }

    // ---- runtime/pem ----

    private static final String PEM = "core.pem";

    public static BigInteger runtime_pem__pem_kind_invalid_offset(String kind) {
        int[] text = points(kind);
        if (text.length == 0) {
            return BigInteger.ZERO;
        }
        boolean separator = true;
        for (int offset = 0; offset < text.length; offset++) {
            int symbol = text[offset];
            if (symbol == ' ' || symbol == '-') {
                if (separator || offset + 1 == text.length) {
                    return BigInteger.valueOf(offset);
                }
                separator = true;
                continue;
            }
            if (!((symbol >= 'A' && symbol <= 'Z') || (symbol >= '0' && symbol <= '9'))) {
                return BigInteger.valueOf(offset);
            }
            separator = false;
        }
        return BigInteger.valueOf(-1);
    }

    public static String runtime_pem__pem_encode(String kind, String base64) {
        StringBuilder out = new StringBuilder("-----BEGIN " + kind + "-----\n");
        int[] text = points(base64);
        for (int offset = 0; offset < text.length; offset += 64) {
            out.append(text(text, offset, Math.min(text.length, offset + 64))).append('\n');
        }
        return out.append("-----END ").append(kind).append("-----\n").toString();
    }

    private static final class PemLine {
        final String text;
        final int start;
        final int end;
        final int length;

        PemLine(int[] source, int start, int stop, int end) {
            this.text = text(source, start, stop);
            this.start = start;
            this.end = end;
            this.length = stop - start;
        }
    }

    private static final class PemBlock {
        String kind;
        String base64;
        String source;
    }

    private static final String ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    private static int base64Invalid(int[] source) {
        int size = source.length;
        if (size == 0) {
            return -1;
        }
        int dataEnd = size;
        int padding = 0;
        boolean inside = false;
        for (int offset = 0; offset < size; offset++) {
            int symbol = source[offset];
            if (symbol == '=') {
                if (!inside) {
                    inside = true;
                    dataEnd = offset;
                }
                padding++;
                continue;
            }
            if (inside || symbol > 127 || ALPHABET.indexOf(symbol) < 0) {
                return offset;
            }
        }
        if (size % 4 != 0) {
            return size;
        }
        int remainder = dataEnd % 4;
        int expected = remainder == 0 ? 0 : remainder == 2 ? 2 : remainder == 3 ? 1 : -1;
        if (expected < 0 || padding != expected) {
            return dataEnd;
        }
        long buffer = 0;
        int bits = 0;
        for (int offset = 0; offset < dataEnd; offset++) {
            buffer = ((buffer << 6) | ALPHABET.indexOf(source[offset])) & 0xffffffffL;
            bits += 6;
            while (bits >= 8) {
                bits -= 8;
            }
        }
        if (bits > 0 && (buffer & ((1L << bits) - 1)) != 0) {
            return dataEnd - 1;
        }
        return -1;
    }

    private static final Pattern PEM_BEGIN = Pattern.compile("\\A-----BEGIN ([A-Z0-9]+(?:[ -][A-Z0-9]+)*)-----\\z");
    private static final Pattern PEM_END = Pattern.compile("\\A-----END (.*)-----\\z", Pattern.DOTALL);

    /** Смещение первой ошибки (или -1) и блоки. */
    private static Object[] pemParsed(String origin) {
        int[] source = points(origin);
        ArrayList<PemLine> lines = new ArrayList<PemLine>();
        int size = source.length;
        int start = 0;
        int offset = 0;
        while (offset < size) {
            int symbol = source[offset];
            if (symbol == '\r') {
                if (offset + 1 >= size || source[offset + 1] != '\n') {
                    return new Object[] {offset, new ArrayList<PemBlock>()};
                }
                lines.add(new PemLine(source, start, offset, offset + 2));
                offset += 2;
                start = offset;
                continue;
            }
            if (symbol == '\n') {
                lines.add(new PemLine(source, start, offset, offset + 1));
                offset++;
                start = offset;
                continue;
            }
            offset++;
        }
        if (start < size) {
            lines.add(new PemLine(source, start, size, size));
        }
        ArrayList<PemBlock> blocks = new ArrayList<PemBlock>();
        int index = 0;
        while (index < lines.size()) {
            PemLine line = lines.get(index);
            if (line.text.isEmpty()) {
                index++;
                continue;
            }
            Matcher begin = PEM_BEGIN.matcher(line.text);
            if (!begin.find()) {
                return new Object[] {line.start, new ArrayList<PemBlock>()};
            }
            String kind = begin.group(1);
            int blockStart = line.start;
            index++;
            StringBuilder body = new StringBuilder();
            ArrayList<Integer> mapping = new ArrayList<Integer>();
            ArrayList<PemLine> bodyLines = new ArrayList<PemLine>();
            boolean closed = false;
            int blockEnd = line.end;
            while (index < lines.size()) {
                line = lines.get(index);
                Matcher end = PEM_END.matcher(line.text);
                if (end.find()) {
                    if (!end.group(1).equals(kind)) {
                        return new Object[] {line.start + 9, new ArrayList<PemBlock>()};
                    }
                    closed = true;
                    blockEnd = line.end;
                    index++;
                    break;
                }
                if (line.text.isEmpty() || line.text.startsWith("-----BEGIN ")) {
                    return new Object[] {line.start, new ArrayList<PemBlock>()};
                }
                if (line.length > 64) {
                    return new Object[] {line.start + 64, new ArrayList<PemBlock>()};
                }
                bodyLines.add(line);
                int[] chars = points(line.text);
                for (int column = 0; column < chars.length; column++) {
                    body.appendCodePoint(chars[column]);
                    mapping.add(line.start + column);
                }
                index++;
            }
            if (!closed) {
                return new Object[] {size, new ArrayList<PemBlock>()};
            }
            for (int at = 0; at < bodyLines.size() - 1; at++) {
                if (bodyLines.get(at).length != 64) {
                    return new Object[] {bodyLines.get(at).start + bodyLines.get(at).length, new ArrayList<PemBlock>()};
                }
            }
            String base64 = body.toString();
            int error = base64Invalid(points(base64));
            if (error != -1) {
                int at;
                if (error < mapping.size()) {
                    at = mapping.get(error);
                } else if (!bodyLines.isEmpty()) {
                    PemLine last = bodyLines.get(bodyLines.size() - 1);
                    at = last.start + last.length;
                } else {
                    at = line.start;
                }
                return new Object[] {at, new ArrayList<PemBlock>()};
            }
            PemBlock block = new PemBlock();
            block.kind = kind;
            block.base64 = base64;
            block.source = text(source, blockStart, blockEnd);
            blocks.add(block);
        }
        return new Object[] {-1, blocks};
    }

    @SuppressWarnings("unchecked")
    private static List<PemBlock> pemBlocks(String origin) {
        Object[] parsed = pemParsed(origin);
        if ((Integer) parsed[0] != -1) {
            throw Natives.died();
        }
        return (List<PemBlock>) parsed[1];
    }

    @SuppressWarnings("unchecked")
    public static List<Object> runtime_pem__pem_block_sources(String origin) {
        Object[] parsed = pemParsed(origin);
        if ((Integer) parsed[0] != -1) {
            throw fail(PEM, "invalid_data", "runtime/pem@1::pem_block_sources/1", "invalid PEM input");
        }
        ArrayList<Object> out = new ArrayList<Object>();
        for (PemBlock block : (List<PemBlock>) parsed[1]) {
            out.add(block.source);
        }
        return out;
    }

    public static BigInteger runtime_pem__pem_invalid_offset(String origin) {
        return BigInteger.valueOf((Integer) pemParsed(origin)[0]);
    }

    public static BigInteger runtime_pem__pem_block_count(String origin) {
        return BigInteger.valueOf(pemBlocks(origin).size());
    }

    private static PemBlock pemBlock(String origin, BigInteger index) {
        List<PemBlock> blocks = pemBlocks(origin);
        if (index.signum() < 0 || index.compareTo(BigInteger.valueOf(blocks.size())) >= 0) {
            throw Natives.died();
        }
        return blocks.get(index.intValue());
    }

    public static String runtime_pem__pem_block_source(String origin, BigInteger index) {
        return pemBlock(origin, index).source;
    }

    public static String runtime_pem__pem_block_kind(String origin, BigInteger index) {
        return pemBlock(origin, index).kind;
    }

    public static String runtime_pem__pem_block_base64(String origin, BigInteger index) {
        return pemBlock(origin, index).base64;
    }

    // ---- runtime/template ----

    private static final class CompileError extends RuntimeException {
        private static final long serialVersionUID = 1L;
        final int offset;
        final int line;
        final int column;
        final String message;

        CompileError(int[] source, int offset, String message) {
            super(message, null, false, false);
            String prefix = text(source, 0, Math.min(offset, source.length)).replace("\r\n", "\n");
            int newlines = 0;
            for (int index = 0; index < prefix.length(); index++) {
                newlines += prefix.charAt(index) == '\n' ? 1 : 0;
            }
            int length = prefix.codePointCount(0, prefix.length());
            int last = prefix.lastIndexOf('\n');
            int lastPoints = last < 0 ? -1 : prefix.codePointCount(0, last);
            this.offset = offset;
            this.line = 1 + newlines;
            this.column = length - lastPoints;
            this.message = message;
        }
    }

    static final class Template {
        String source;
        BigInteger input;
        BigInteger maximumNodes;
        boolean compiled;
        boolean valid;
        CompileError error;
        ArrayList<Object[]> nodes;
    }

    private static int indexOf(int[] source, String needle, int from) {
        outer:
        for (int at = Math.max(0, from); at + needle.length() <= source.length; at++) {
            for (int index = 0; index < needle.length(); index++) {
                if (source[at + index] != needle.charAt(index)) {
                    continue outer;
                }
            }
            return at;
        }
        return -1;
    }

    private static void compile(Template state) {
        if (state.compiled) {
            return;
        }
        state.compiled = true;
        int[] source = points(state.source);
        try {
            if (state.input.signum() < 0 || state.maximumNodes.signum() < 0) {
                throw new CompileError(source, 0, "negative template budget");
            }
            if (BigInteger.valueOf(source.length).compareTo(state.input) > 0) {
                throw new CompileError(source, 0, "template input limit exceeded");
            }
            int bad = lone(state.source);
            if (bad >= 0) {
                throw new CompileError(source, bad, "template source is not Unicode scalar text");
            }
            ArrayList<Object[]> nodes = new ArrayList<Object[]>();
            StringBuilder literal = new StringBuilder();
            int position = 0;
            while (position < source.length) {
                String four = text(source, position, Math.min(source.length, position + 4));
                if (four.equals("{{{{") || four.equals("}}}}")) {
                    literal.append(four, 0, 2);
                    position += 4;
                    continue;
                }
                String two = text(source, position, Math.min(source.length, position + 2));
                if (two.equals("}}")) {
                    throw new CompileError(source, position, "unexpected closing template delimiter");
                }
                if (!two.equals("{{")) {
                    int open = indexOf(source, "{{", position);
                    int close = indexOf(source, "}}", position);
                    int next = source.length;
                    if (open >= 0 && open < next) {
                        next = open;
                    }
                    if (close >= 0 && close < next) {
                        next = close;
                    }
                    literal.append(text(source, position, next));
                    position = next;
                    continue;
                }
                int start = position;
                int end = indexOf(source, "}}", start + 2);
                if (end < 0) {
                    throw new CompileError(source, start, "unclosed template tag");
                }
                String body = text(source, start + 2, end);
                position = end + 2;
                if (body.startsWith("!")) {
                    int lineStart = 0;
                    for (int at = start - 1; at >= 0; at--) {
                        if (source[at] == '\n') {
                            lineStart = at + 1;
                            break;
                        }
                    }
                    String prefix = text(source, lineStart, start);
                    Matcher rest = Pattern.compile("\\A[ \\t]*(?:\\r?\\n|\\z)").matcher(text(source, position, source.length));
                    if (prefix.matches("[ \\t]*") && rest.find()) {
                        int removed = rest.group().length();
                        if (!prefix.isEmpty()) {
                            literal.setLength(literal.length() - prefix.length());
                        }
                        position += removed;
                    }
                    continue;
                }
                body = body.replaceFirst("\\A[ \\t]+", "").replaceFirst("[ \\t]+\\z", "");
                if (body.isEmpty() || body.matches("(?s)\\A[#^/>=&{].*") || body.matches("(?s).*[{}\\r\\n].*")) {
                    throw new CompileError(source, start, "unsupported or empty template tag");
                }
                if (body.codePointCount(0, body.length()) > 4096) {
                    throw new CompileError(source, start, "template path length exceeded");
                }
                String[] parts = body.split("\\.", -1);
                boolean empty = false;
                for (String part : parts) {
                    empty |= part.isEmpty();
                }
                if (parts.length > 64 || empty) {
                    throw new CompileError(source, start, "invalid named template path");
                }
                flush(state, source, nodes, literal, start);
                appendNode(state, source, nodes, 1, body, start);
            }
            flush(state, source, nodes, literal, position);
            state.nodes = nodes;
            state.valid = true;
        } catch (CompileError error) {
            state.valid = false;
            state.error = error;
        }
    }

    private static void appendNode(Template state, int[] source, List<Object[]> nodes, int kind, String text, int offset) {
        if (BigInteger.valueOf(nodes.size()).compareTo(state.maximumNodes) >= 0) {
            throw new CompileError(source, offset, "template node limit exceeded");
        }
        nodes.add(new Object[] {kind, text});
    }

    private static void flush(Template state, int[] source, List<Object[]> nodes, StringBuilder literal, int offset) {
        if (literal.length() > 0) {
            appendNode(state, source, nodes, 0, literal.toString(), offset);
        }
        literal.setLength(0);
    }

    public static Object runtime_template__template_compile_state(String source, BigInteger input, BigInteger nodes) {
        Template state = new Template();
        state.source = source;
        state.input = input;
        state.maximumNodes = nodes;
        return state;
    }

    private static Template template(Object handle) {
        Template state = (Template) handle;
        compile(state);
        return state;
    }

    public static boolean runtime_template__template_compile_valid(Object handle) {
        return template(handle).valid;
    }

    public static BigInteger runtime_template__template_compile_offset(Object handle) {
        Template state = template(handle);
        return BigInteger.valueOf(state.valid ? 0 : state.error.offset);
    }

    public static BigInteger runtime_template__template_compile_line(Object handle) {
        Template state = template(handle);
        return BigInteger.valueOf(state.valid ? 0 : state.error.line);
    }

    public static BigInteger runtime_template__template_compile_column(Object handle) {
        Template state = template(handle);
        return BigInteger.valueOf(state.valid ? 0 : state.error.column);
    }

    public static String runtime_template__template_compile_message(Object handle) {
        Template state = template(handle);
        return state.valid ? "" : state.error.message;
    }

    private static List<Object[]> templateNodes(Object handle, String name, int arity) {
        Template state = template(handle);
        if (!state.valid) {
            throw fail("core.template", "invalid_data", "runtime/template@1::" + name + "/" + arity,
                "invalid template has no instructions");
        }
        return state.nodes;
    }

    public static BigInteger runtime_template__template_node_count(Object handle) {
        return BigInteger.valueOf(templateNodes(handle, "template_node_count", 1).size());
    }

    private static Object[] templateNode(Object handle, BigInteger index, String name) {
        List<Object[]> nodes = templateNodes(handle, name, 2);
        if (index.signum() < 0 || index.compareTo(BigInteger.valueOf(nodes.size())) >= 0) {
            throw fail("core.template", "invalid_input", "runtime/template@1::" + name + "/2",
                "template instruction index outside program");
        }
        return nodes.get(index.intValue());
    }

    public static BigInteger runtime_template__template_node_kind(Object handle, BigInteger index) {
        return BigInteger.valueOf((Integer) templateNode(handle, index, "template_node_kind")[0]);
    }

    public static String runtime_template__template_node_text(Object handle, BigInteger index) {
        return (String) templateNode(handle, index, "template_node_text")[1];
    }

    public static BigInteger runtime_template__template_output_size(String text, BigInteger maximum) {
        String symbol = "runtime/template@1::template_output_size/2";
        if (maximum.signum() < 0) {
            throw fail("core.template", "invalid_input", symbol, "negative template byte budget");
        }
        if (lone(text) >= 0) {
            throw fail("core.template", "invalid_data", symbol, "template value is not Unicode scalar text");
        }
        int[] points = points(text);
        if (BigInteger.valueOf(points.length).compareTo(maximum) > 0) {
            return BigInteger.valueOf(-1);
        }
        long limit = maximum.longValue();
        long size = 0;
        for (int point : points) {
            int width = point < 0x80 ? 1 : point < 0x800 ? 2 : point < 0x10000 ? 3 : 4;
            if (width > limit - size) {
                return BigInteger.valueOf(-1);
            }
            size += width;
        }
        return BigInteger.valueOf(size);
    }

    // ---- runtime/ini ----

    private static final class IniError extends RuntimeException {
        private static final long serialVersionUID = 1L;
        final long line;
        final long column;
        final String message;

        IniError(long line, int position, String message) {
            super(message, null, false, false);
            this.line = line;
            this.column = position + 1;
            this.message = message;
        }
    }

    static final class IniParse {
        String source;
        BigInteger input;
        BigInteger sections;
        BigInteger entries;
        boolean parsed;
        boolean valid;
        IniError error;
        ArrayList<Object[]> result;
    }

    private static int space(int[] text, int position) {
        while (position < text.length && (text[position] == ' ' || text[position] == '\t')) {
            position++;
        }
        return position;
    }

    private static Object[] quoted(int[] text, int position, long line) {
        position++;
        StringBuilder value = new StringBuilder();
        while (position < text.length) {
            int point = text[position++];
            if (point == '"') {
                return new Object[] {value.toString(), position};
            }
            if (point == '\\') {
                if (position == text.length) {
                    throw new IniError(line, position, "unfinished escape");
                }
                int escape = text[position++];
                switch (escape) {
                    case '"': value.append('"'); continue;
                    case '\\': value.append('\\'); continue;
                    case 'b': value.append('\b'); continue;
                    case 't': value.append('\t'); continue;
                    case 'n': value.append('\n'); continue;
                    case 'f': value.append('\f'); continue;
                    case 'r': value.append('\r'); continue;
                    default: break;
                }
                if (escape != 'u' && escape != 'U') {
                    throw new IniError(line, position - 1, "unknown escape");
                }
                int width = escape == 'u' ? 4 : 8;
                String digits = text(text, position, Math.min(text.length, position + width));
                if (!digits.matches("[0-9a-fA-F]{" + width + "}")) {
                    throw new IniError(line, position, "invalid Unicode escape");
                }
                long code = Long.parseLong(digits, 16);
                if (code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff)) {
                    throw new IniError(line, position, "escape is not a Unicode scalar");
                }
                value.appendCodePoint((int) code);
                position += width;
            } else {
                value.appendCodePoint(point);
            }
        }
        throw new IniError(line, position, "unterminated quoted string");
    }

    private static Object[] iniName(int[] text, int position, char delimiter, long line) {
        position = space(text, position);
        if (position < text.length && text[position] == '"') {
            Object[] quoted = quoted(text, position, line);
            return new Object[] {quoted[0], space(text, (Integer) quoted[1])};
        }
        int end = -1;
        for (int at = position; at < text.length; at++) {
            if (text[at] == delimiter) {
                end = at;
                break;
            }
        }
        if (end < 0) {
            throw new IniError(line, text.length, "missing " + delimiter);
        }
        String name = text(text, position, end).replaceFirst("[ \\t]+\\z", "");
        if (name.isEmpty()) {
            throw new IniError(line, position, "empty bare name");
        }
        return new Object[] {name, end};
    }

    private static final Pattern INI_BAD = Pattern.compile("[\\x00-\\x08\\x0a-\\x1f\\x7f]");

    private static void iniParse(IniParse state) {
        if (state.parsed) {
            return;
        }
        state.parsed = true;
        try {
            if (state.input.signum() < 0 || state.sections.signum() < 0 || state.entries.signum() < 0) {
                throw new IniError(1, 0, "negative INI budget");
            }
            int[] all = points(state.source);
            if (BigInteger.valueOf(all.length).compareTo(state.input) > 0) {
                throw new IniError(1, 0, "maximum INI input exceeded");
            }
            ArrayList<Object[]> sections = new ArrayList<Object[]>();
            HashSet<String> seen = new HashSet<String>();
            Object[] current = null;
            long line = 0;
            long entries = 0;
            ArrayList<String> rawLines = new ArrayList<String>();
            int from = 0;
            for (int at = 0; at < all.length; at++) {
                if (all[at] == '\n') {
                    rawLines.add(text(all, from, at + 1));
                    from = at + 1;
                }
            }
            if (from < all.length) {
                rawLines.add(text(all, from, all.length));
            }
            for (String piece : rawLines) {
                line++;
                String raw = piece.replaceFirst("\\r?\\n\\z", "");
                int bad = lone(raw);
                Matcher control = INI_BAD.matcher(raw);
                int controlAt = control.find() ? raw.codePointCount(0, control.start()) : -1;
                int first = bad < 0 ? controlAt : controlAt < 0 ? bad : Math.min(bad, controlAt);
                if (first >= 0) {
                    throw new IniError(line, first, "invalid scalar or raw control character");
                }
                int[] text = points(raw);
                int position = space(text, 0);
                if (position == text.length || text[position] == '#' || text[position] == ';') {
                    continue;
                }
                if (text[position] == '[') {
                    Object[] name = iniName(text, position + 1, ']', line);
                    int end = (Integer) name[1];
                    if (end >= text.length || text[end] != ']') {
                        throw new IniError(line, end, "expected closing bracket");
                    }
                    if (space(text, end + 1) != text.length) {
                        throw new IniError(line, end + 1, "trailing section material");
                    }
                    if (seen.contains(name[0])) {
                        throw new IniError(line, position, "duplicate INI section");
                    }
                    if (BigInteger.valueOf(sections.size()).compareTo(state.sections) >= 0) {
                        throw new IniError(line, position, "maximum INI sections exceeded");
                    }
                    current = new Object[] {name[0], new ArrayList<String[]>(), new HashSet<String>()};
                    sections.add(current);
                    seen.add((String) name[0]);
                } else {
                    if (current == null) {
                        throw new IniError(line, position, "entry requires a section");
                    }
                    Object[] name = iniName(text, position, '=', line);
                    int end = (Integer) name[1];
                    if (end >= text.length || text[end] != '=') {
                        throw new IniError(line, end, "expected equals");
                    }
                    int start = space(text, end + 1);
                    String value;
                    if (start < text.length && text[start] == '"') {
                        Object[] quoted = quoted(text, start, line);
                        value = (String) quoted[0];
                        int following = (Integer) quoted[1];
                        if (space(text, following) != text.length) {
                            throw new IniError(line, following, "trailing value material");
                        }
                    } else {
                        value = text(text, Math.min(start, text.length), text.length).replaceFirst("[ \\t]+\\z", "");
                    }
                    @SuppressWarnings("unchecked")
                    ArrayList<String[]> list = (ArrayList<String[]>) current[1];
                    @SuppressWarnings("unchecked")
                    HashSet<String> names = (HashSet<String>) current[2];
                    if (names.contains(name[0])) {
                        throw new IniError(line, position, "duplicate INI entry");
                    }
                    if (BigInteger.valueOf(entries).compareTo(state.entries) >= 0 || list.size() >= 1024) {
                        throw new IniError(line, position, "maximum INI entries exceeded");
                    }
                    list.add(new String[] {(String) name[0], value});
                    names.add((String) name[0]);
                    entries++;
                }
            }
            state.result = sections;
            state.valid = true;
        } catch (IniError error) {
            state.valid = false;
            state.error = error;
        }
    }

    public static Object runtime_ini__ini_parse_state(String source, BigInteger input, BigInteger sections,
            BigInteger entries) {
        IniParse state = new IniParse();
        state.source = source;
        state.input = input;
        state.sections = sections;
        state.entries = entries;
        return state;
    }

    private static IniParse ini(Object handle) {
        IniParse state = (IniParse) handle;
        iniParse(state);
        return state;
    }

    public static boolean runtime_ini__ini_parse_valid(Object handle) {
        return ini(handle).valid;
    }

    public static BigInteger runtime_ini__ini_parse_line(Object handle) {
        IniParse state = ini(handle);
        return BigInteger.valueOf(state.valid ? 0 : state.error.line);
    }

    public static BigInteger runtime_ini__ini_parse_column(Object handle) {
        IniParse state = ini(handle);
        return BigInteger.valueOf(state.valid ? 0 : state.error.column);
    }

    public static String runtime_ini__ini_parse_message(Object handle) {
        IniParse state = ini(handle);
        return state.valid ? "" : state.error.message;
    }

    private static List<Object[]> iniSections(Object handle, String name, int arity) {
        IniParse state = ini(handle);
        if (!state.valid) {
            throw fail("core.ini", "invalid_data", "runtime/ini@1::" + name + "/" + arity, "invalid INI has no sections");
        }
        return state.result;
    }

    private static Object[] iniSection(Object handle, BigInteger index, String name, int arity) {
        List<Object[]> sections = iniSections(handle, name, arity);
        if (index.signum() < 0 || index.compareTo(BigInteger.valueOf(sections.size())) >= 0) {
            throw fail("core.ini", "invalid_input", "runtime/ini@1::" + name + "/" + arity,
                "INI section index outside document");
        }
        return sections.get(index.intValue());
    }

    public static BigInteger runtime_ini__ini_section_count(Object handle) {
        return BigInteger.valueOf(iniSections(handle, "ini_section_count", 1).size());
    }

    public static String runtime_ini__ini_section_name(Object handle, BigInteger index) {
        return (String) iniSection(handle, index, "ini_section_name", 2)[0];
    }

    @SuppressWarnings("unchecked")
    public static BigInteger runtime_ini__ini_entry_count(Object handle, BigInteger index) {
        return BigInteger.valueOf(((List<String[]>) iniSection(handle, index, "ini_entry_count", 2)[1]).size());
    }

    @SuppressWarnings("unchecked")
    private static String[] iniEntry(Object handle, BigInteger section, BigInteger entry, String name) {
        List<String[]> entries = (List<String[]>) iniSection(handle, section, name, 3)[1];
        if (entry.signum() < 0 || entry.compareTo(BigInteger.valueOf(entries.size())) >= 0) {
            throw fail("core.ini", "invalid_input", "runtime/ini@1::" + name + "/3", "INI entry index outside section");
        }
        return entries.get(entry.intValue());
    }

    public static String runtime_ini__ini_entry_name(Object handle, BigInteger section, BigInteger entry) {
        return iniEntry(handle, section, entry, "ini_entry_name")[0];
    }

    public static String runtime_ini__ini_entry_value(Object handle, BigInteger section, BigInteger entry) {
        return iniEntry(handle, section, entry, "ini_entry_value")[1];
    }

    static final class IniWrite {
        BigInteger maximum;
        StringBuilder text = new StringBuilder();
        long length;
        int status;
        String name = "";
        String parent;
        final HashSet<String> sections = new HashSet<String>();
        HashSet<String> entries = new HashSet<String>();
    }

    private static boolean writeError(IniWrite state, int status, String name) {
        if (state.status != 0) {
            return false;
        }
        state.status = status;
        state.name = name;
        state.text = new StringBuilder();
        state.length = 0;
        return false;
    }

    private static boolean writeAppend(IniWrite state, String text) {
        if (state.status != 0) {
            return false;
        }
        long size = text.codePointCount(0, text.length());
        if (BigInteger.valueOf(size).compareTo(state.maximum.subtract(BigInteger.valueOf(state.length))) > 0) {
            return writeError(state, 4, "");
        }
        state.text.append(text);
        state.length += size;
        return true;
    }

    private static boolean writeQuote(IniWrite state, String text) {
        if (lone(text) >= 0) {
            return writeError(state, 5, "");
        }
        if (!writeAppend(state, "\"")) {
            return false;
        }
        for (int point : points(text)) {
            String escaped;
            switch (point) {
                case '"': escaped = "\\\""; break;
                case '\\': escaped = "\\\\"; break;
                case '\b': escaped = "\\b"; break;
                case '\t': escaped = "\\t"; break;
                case '\n': escaped = "\\n"; break;
                case '\f': escaped = "\\f"; break;
                case '\r': escaped = "\\r"; break;
                default:
                    escaped = point < 32 || point == 127 ? String.format("\\u%04x", point)
                        : new String(Character.toChars(point));
            }
            if (!writeAppend(state, escaped)) {
                return false;
            }
        }
        return writeAppend(state, "\"");
    }

    public static Object runtime_ini__ini_write_state(BigInteger maximum) {
        if (maximum.signum() < 0) {
            throw fail("core.ini", "invalid_input", "runtime/ini@1::ini_write_state/1", "negative INI output budget");
        }
        IniWrite state = new IniWrite();
        state.maximum = maximum;
        return state;
    }

    public static boolean runtime_ini__ini_write_section(Object handle, String name) {
        IniWrite state = (IniWrite) handle;
        if (state.status != 0) {
            return false;
        }
        if (state.sections.contains(name)) {
            return writeError(state, 2, name);
        }
        if (!(writeAppend(state, "[") && writeQuote(state, name) && writeAppend(state, "]\n"))) {
            return false;
        }
        state.sections.add(name);
        state.parent = name;
        state.entries = new HashSet<String>();
        return true;
    }

    public static boolean runtime_ini__ini_write_entry(Object handle, String name, String value) {
        IniWrite state = (IniWrite) handle;
        if (state.status != 0) {
            return false;
        }
        if (state.parent == null) {
            return writeError(state, 1, "");
        }
        if (state.entries.contains(name)) {
            return writeError(state, 3, name);
        }
        if (!(writeQuote(state, name) && writeAppend(state, "=") && writeQuote(state, value)
                && writeAppend(state, "\n"))) {
            return false;
        }
        state.entries.add(name);
        return true;
    }

    public static BigInteger runtime_ini__ini_write_status(Object handle) {
        return BigInteger.valueOf(((IniWrite) handle).status);
    }

    public static String runtime_ini__ini_write_name(Object handle) {
        return ((IniWrite) handle).name;
    }

    public static String runtime_ini__ini_write_parent(Object handle) {
        String parent = ((IniWrite) handle).parent;
        return parent == null ? "" : parent;
    }

    public static String runtime_ini__ini_write_string(Object handle) {
        IniWrite state = (IniWrite) handle;
        if (state.status != 0) {
            throw fail("core.ini", "invalid_data", "runtime/ini@1::ini_write_string/1", "failed INI emission has no text");
        }
        return state.text.toString();
    }
}
