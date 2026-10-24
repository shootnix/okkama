package okkama.rt;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Natives {@code runtime/toml} (TOML 1.0 в дерево узлов) и
 * {@code runtime/html} (поток событий HTML с именованными ссылками WHATWG).
 * Смещения — в символах; тексты отказов — как у Perl-provider.
 */
public final class MarkupNatives {
    private MarkupNatives() {}

    private static Failure fail(String domain, String code, String symbol, String message) {
        return NativeFailure.raise(domain, code, symbol, message);
    }

    private static String text(int[] points, int from, int to) {
        return new String(points, from, Math.max(0, to - from));
    }

    // ===================== runtime/toml =====================

    private static final int INTEGER = 1;
    private static final int REAL = 2;
    private static final int TEXT = 3;
    private static final int BOOLEAN = 4;
    private static final int ARRAY = 5;
    private static final int TABLE = 6;
    private static final int DATE = 7;
    private static final int TIME = 8;
    private static final int LOCAL = 9;
    private static final int OFFSET = 10;
    private static final int INFINITY = 11;
    private static final int NAN = 12;

    private static final BigInteger MIN = BigInteger.valueOf(Long.MIN_VALUE);
    private static final BigInteger MAX = BigInteger.valueOf(Long.MAX_VALUE);

    static final class Node {
        int kind;
        long depth;
        String definition;
        boolean sealed;
        boolean aot;
        final ArrayList<String> names = new ArrayList<String>();
        final ArrayList<Node> members = new ArrayList<Node>();
        final HashMap<String, Node> index = new HashMap<String, Node>();
        final ArrayList<Node> values = new ArrayList<Node>();
        String text;
        double real;
        boolean flag;
        long[] coordinates;
    }

    private static final class TomlError extends RuntimeException {
        private static final long serialVersionUID = 1L;
        final int offset;
        final int line;
        final int column;
        final String message;

        TomlError(int[] source, int offset, String message) {
            super(message, null, false, false);
            String prefix = text(source, 0, Math.min(offset, source.length)).replace("\r\n", "\n");
            int newlines = 0;
            for (int index = 0; index < prefix.length(); index++) {
                newlines += prefix.charAt(index) == '\n' ? 1 : 0;
            }
            int length = prefix.codePointCount(0, prefix.length());
            int last = prefix.lastIndexOf('\n');
            this.offset = offset;
            this.line = 1 + newlines;
            this.column = length - (last < 0 ? -1 : prefix.codePointCount(0, last));
            this.message = message;
        }
    }

    private static final class Parser {
        final int[] source;
        int position;
        final BigInteger maximumDepth;
        final BigInteger maximumNodes;
        long nodes;

        Parser(String source, BigInteger maximumDepth, BigInteger maximumNodes) {
            this.source = source.codePoints().toArray();
            this.maximumDepth = maximumDepth;
            this.maximumNodes = maximumNodes;
        }

        TomlError error(String message) {
            return new TomlError(source, position, message);
        }

        TomlError error(String message, int offset) {
            return new TomlError(source, offset, message);
        }

        String peek(int count) {
            return text(source, Math.min(position, source.length), Math.min(source.length, position + count));
        }

        int at(int index) {
            return index < source.length ? source[index] : -1;
        }

        int peek() {
            return at(position);
        }

        void horizontal() {
            while (peek() == ' ' || peek() == '\t') {
                position++;
            }
        }

        boolean newline() {
            if (peek(2).equals("\r\n")) {
                position += 2;
                return true;
            }
            if (peek() == '\n') {
                position++;
                return true;
            }
            if (peek() == '\r') {
                throw error("bare carriage return is not a TOML newline");
            }
            return false;
        }

        void comment() {
            if (peek() != '#') {
                return;
            }
            while (position < source.length && peek() != '\n' && peek() != '\r') {
                int code = peek();
                if ((code < 32 && code != 9) || code == 127) {
                    throw error("control character in comment");
                }
                position++;
            }
        }

        void space() {
            while (true) {
                horizontal();
                comment();
                if (!newline()) {
                    return;
                }
            }
        }

        void expect(String token) {
            if (!peek(token.length()).equals(token)) {
                throw error("expected " + token);
            }
            position += token.length();
        }

        Node node(int kind, long depth) {
            if (BigInteger.valueOf(depth).compareTo(maximumDepth) > 0) {
                throw error("maximum TOML depth exceeded");
            }
            if (BigInteger.valueOf(nodes).compareTo(maximumNodes) >= 0) {
                throw error("maximum TOML nodes exceeded");
            }
            nodes++;
            Node node = new Node();
            node.kind = kind;
            node.depth = depth;
            return node;
        }

        Node table(long depth, String definition) {
            Node node = node(TABLE, depth);
            node.definition = definition;
            return node;
        }

        void put(Node table, String name, Node value) {
            if (table.index.containsKey(name)) {
                throw error("duplicate TOML key");
            }
            if (table.members.size() >= 1024) {
                throw error("maximum TOML table members exceeded");
            }
            table.index.put(name, value);
            table.names.add(name);
            table.members.add(value);
        }

        String string(boolean key) {
            int quote = peek();
            String triple = new String(new int[] {quote, quote, quote}, 0, 3);
            boolean multiline = peek(3).equals(triple);
            if (key && multiline) {
                throw error("multiline key is not allowed");
            }
            position += multiline ? 3 : 1;
            if (multiline) {
                newline();
            }
            StringBuilder result = new StringBuilder();
            while (position < source.length) {
                int start = position;
                int c = peek();
                if (c == quote) {
                    if (!multiline) {
                        position++;
                        return result.toString();
                    }
                    int count = 0;
                    while (at(start + count) == quote) {
                        count++;
                    }
                    if (count > 5) {
                        throw error("too many closing quotes");
                    }
                    position += count;
                    if (count >= 3) {
                        for (int index = 0; index < count - 3; index++) {
                            result.appendCodePoint(quote);
                        }
                        return result.toString();
                    }
                    for (int index = 0; index < count; index++) {
                        result.appendCodePoint(quote);
                    }
                    continue;
                }
                if (c == '\n' || c == '\r') {
                    if (!multiline) {
                        throw error("newline in single-line string");
                    }
                    newline();
                    result.append('\n');
                    continue;
                }
                if ((c < 32 && c != 9) || c == 127) {
                    throw error("control character in string");
                }
                position++;
                if (c != '\\' || quote == '\'') {
                    result.appendCodePoint(c);
                    continue;
                }
                if (multiline) {
                    int saved = position;
                    horizontal();
                    if (newline()) {
                        do {
                            horizontal();
                        } while (newline());
                        continue;
                    }
                    position = saved;
                }
                if (position >= source.length) {
                    throw error("unfinished string escape", start);
                }
                int escape = source[position++];
                switch (escape) {
                    case 'b': result.append('\b'); continue;
                    case 't': result.append('\t'); continue;
                    case 'n': result.append('\n'); continue;
                    case 'f': result.append('\f'); continue;
                    case 'r': result.append('\r'); continue;
                    case '"': result.append('"'); continue;
                    case '\\': result.append('\\'); continue;
                    default: break;
                }
                if (escape != 'u' && escape != 'U') {
                    throw error("unknown string escape", start);
                }
                int length = escape == 'u' ? 4 : 8;
                String hex = peek(length);
                if (hex.codePointCount(0, hex.length()) != length || !hex.matches("[0-9A-Fa-f]+")) {
                    throw error("invalid Unicode escape", start);
                }
                long point = Long.parseLong(hex, 16);
                if (point > 0x10ffff || (point >= 0xd800 && point <= 0xdfff)) {
                    throw error("escape is not a Unicode scalar", start);
                }
                position += length;
                result.appendCodePoint((int) point);
            }
            throw error("unterminated string");
        }

        List<String> key() {
            ArrayList<String> parts = new ArrayList<String>();
            while (true) {
                horizontal();
                int c = peek();
                if (c == '"' || c == '\'') {
                    parts.add(string(true));
                } else {
                    int start = position;
                    while (position < source.length && isBare(peek())) {
                        position++;
                    }
                    if (start == position) {
                        throw error("expected key");
                    }
                    parts.add(text(source, start, position));
                }
                horizontal();
                if (peek() != '.') {
                    return parts;
                }
                position++;
            }
        }

        boolean isBare(int c) {
            return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '_' || c == '-';
        }

        Node descend(Node table, String name, String definition) {
            if (table.sealed) {
                throw error("inline table cannot be extended");
            }
            Node node = table.index.get(name);
            if (node == null) {
                node = table(table.depth + 1, definition);
                put(table, name, node);
            }
            if (definition.equals("dotted") && (node.kind == ARRAY
                    || (node.kind == TABLE && "explicit".equals(node.definition)))) {
                throw error("dotted key cannot extend a header-defined table");
            }
            if (node.kind == ARRAY && node.aot) {
                node = node.values.get(node.values.size() - 1);
            }
            if (node.kind != TABLE || node.sealed) {
                throw error("key is not an extensible table");
            }
            if (definition.equals("dotted") && "implicit".equals(node.definition)) {
                node.definition = "dotted";
            }
            return node;
        }

        void binding(Node table) {
            int keyOffset = position;
            List<String> path = key();
            expect("=");
            horizontal();
            String name = path.remove(path.size() - 1);
            for (String part : path) {
                table = descend(table, part, "dotted");
            }
            if (table.index.containsKey(name)) {
                throw error("duplicate TOML key", keyOffset);
            }
            if (table.members.size() >= 1024) {
                throw error("maximum TOML table members exceeded", keyOffset);
            }
            put(table, name, value(table.depth + 1));
        }

        Node header(Node root) {
            boolean array = peek(2).equals("[[");
            position += array ? 2 : 1;
            List<String> path = key();
            expect(array ? "]]" : "]");
            String name = path.remove(path.size() - 1);
            Node parent = root;
            for (String part : path) {
                parent = descend(parent, part, "implicit");
            }
            Node node = parent.index.get(name);
            if (array) {
                if (node == null) {
                    node = node(ARRAY, parent.depth + 1);
                    node.aot = true;
                    put(parent, name, node);
                }
                if (node.kind != ARRAY || !node.aot) {
                    throw error("array-of-tables conflicts with existing value");
                }
                if (node.values.size() >= 10000) {
                    throw error("maximum TOML array items exceeded");
                }
                Node element = table(node.depth + 1, "explicit");
                node.values.add(element);
                return element;
            }
            if (node == null) {
                node = table(parent.depth + 1, "explicit");
                put(parent, name, node);
            } else {
                if (!(node.kind == TABLE && !node.sealed && "implicit".equals(node.definition))) {
                    throw error("table redefinition or conflict");
                }
                node.definition = "explicit";
            }
            return node;
        }

        boolean dateValid(long y, long m, long d) {
            if (m < 1 || m > 12 || d < 1) {
                return false;
            }
            boolean leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
            int[] days = {31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
            return d <= days[(int) m - 1];
        }

        final Pattern dateTime = Pattern.compile(
            "\\A([0-9]{4})-([0-9]{2})-([0-9]{2})(?:[Tt ]([0-9]{2}):([0-9]{2}):([0-9]{2})(?:\\.([0-9]+))?([Zz]|[+-][0-9]{2}:[0-9]{2})?)?\\z");
        final Pattern timeOnly = Pattern.compile("\\A([0-9]{2}):([0-9]{2}):([0-9]{2})(?:\\.([0-9]+))?\\z");

        Node temporal(String raw, long depth, int at) {
            ArrayList<Long> d = new ArrayList<Long>();
            ArrayList<Long> t = new ArrayList<Long>();
            String[] time = null;
            String zone = null;
            Matcher full = dateTime.matcher(raw);
            if (full.find()) {
                d.add(Long.parseLong(full.group(1)));
                d.add(Long.parseLong(full.group(2)));
                d.add(Long.parseLong(full.group(3)));
                if (full.group(4) != null) {
                    time = new String[] {full.group(4), full.group(5), full.group(6), full.group(7)};
                }
                zone = full.group(8);
            } else {
                Matcher only = timeOnly.matcher(raw);
                if (!only.find()) {
                    return null;
                }
                time = new String[] {only.group(1), only.group(2), only.group(3), only.group(4)};
            }
            if (!d.isEmpty() && !dateValid(d.get(0), d.get(1), d.get(2))) {
                throw error("invalid Gregorian date", at);
            }
            if (time != null) {
                long hour = Long.parseLong(time[0]);
                long minute = Long.parseLong(time[1]);
                long second = Long.parseLong(time[2]);
                if (hour > 23 || minute > 59 || second > 60) {
                    throw error("invalid local time", at);
                }
                String fraction = ((time[3] == null ? "" : time[3]) + "000000000").substring(0, 9);
                t.add(hour);
                t.add(minute);
                t.add(second);
                t.add(Long.parseLong(fraction));
            }
            ArrayList<Long> coordinates = new ArrayList<Long>(d);
            coordinates.addAll(t);
            Node node;
            if (zone != null) {
                long minutes = 0;
                long known = 1;
                if (!zone.equalsIgnoreCase("Z")) {
                    long sign = zone.charAt(0) == '-' ? -1 : 1;
                    long hour = Long.parseLong(zone.substring(1, 3));
                    long minute = Long.parseLong(zone.substring(4, 6));
                    if (hour > 23 || minute > 59) {
                        throw error("invalid fixed offset", at);
                    }
                    minutes = (hour * 60 + minute) * sign;
                    if (zone.equals("-00:00")) {
                        known = 0;
                    }
                }
                coordinates.add(minutes);
                coordinates.add(known);
                node = node(OFFSET, depth);
            } else {
                node = node(!d.isEmpty() && !t.isEmpty() ? LOCAL : !d.isEmpty() ? DATE : TIME, depth);
            }
            node.coordinates = new long[coordinates.size()];
            for (int index = 0; index < coordinates.size(); index++) {
                node.coordinates[index] = coordinates.get(index);
            }
            return node;
        }

        final Pattern datePrefix = Pattern.compile("\\A[0-9]{4}-[0-9]{2}-[0-9]{2} (?=[0-9]{2}:)");
        static final String DECIMAL = "[+-]?(?:0|[1-9](?:_?[0-9])*)";

        Node scalar(long depth) {
            int start = position;
            if (datePrefix.matcher(text(source, position, source.length)).find()) {
                position += 11;
            }
            while (position < source.length && " \t\r\n,]}#".indexOf(peek()) < 0) {
                position++;
            }
            String raw = text(source, start, position);
            Node temporal = temporal(raw, depth, start);
            if (temporal != null) {
                return temporal;
            }
            if (raw.equals("true") || raw.equals("false")) {
                Node node = node(BOOLEAN, depth);
                node.flag = raw.equals("true");
                return node;
            }
            if (raw.matches("[+-]?inf")) {
                Node node = node(INFINITY, depth);
                node.flag = raw.startsWith("-");
                return node;
            }
            if (raw.matches("[+-]?nan")) {
                return node(NAN, depth);
            }
            String clean = raw.replace("_", "");
            BigInteger integer = null;
            if (raw.matches(DECIMAL)) {
                integer = new BigInteger(clean.startsWith("+") ? clean.substring(1) : clean);
            } else if (raw.matches("0x[0-9A-Fa-f](?:_?[0-9A-Fa-f])*")) {
                integer = new BigInteger(clean.substring(2), 16);
            } else if (raw.matches("0o[0-7](?:_?[0-7])*")) {
                integer = new BigInteger(clean.substring(2), 8);
            } else if (raw.matches("0b[01](?:_?[01])*")) {
                integer = new BigInteger(clean.substring(2), 2);
            }
            if (integer != null) {
                if (integer.compareTo(MIN) < 0 || integer.compareTo(MAX) > 0) {
                    throw error("integer outside signed 64-bit range", start);
                }
                Node node = node(INTEGER, depth);
                node.text = integer.toString();
                return node;
            }
            if (raw.matches(DECIMAL + "(?:\\.[0-9](?:_?[0-9])*)?(?:[eE][+-]?[0-9](?:_?[0-9])*)?")
                    && raw.matches(".*[.eE].*")) {
                double number = Double.parseDouble(clean);
                if (Double.isInfinite(number) || Double.isNaN(number)) {
                    throw error("floating-point overflow", start);
                }
                Node node = node(REAL, depth);
                node.real = number;
                return node;
            }
            throw error("expected TOML value", start);
        }

        Node value(long depth) {
            int c = peek();
            if (c == '"' || c == '\'') {
                Node node = node(TEXT, depth);
                node.text = string(false);
                return node;
            }
            if (c == '[') {
                Node node = node(ARRAY, depth);
                position++;
                space();
                while (peek() != ']') {
                    if (node.values.size() >= 10000) {
                        throw error("maximum TOML array items exceeded");
                    }
                    node.values.add(value(depth + 1));
                    space();
                    if (peek() == ']') {
                        break;
                    }
                    expect(",");
                    space();
                }
                position++;
                return node;
            }
            if (c == '{') {
                Node node = table(depth, "inline");
                position++;
                horizontal();
                if (peek() != '}') {
                    while (true) {
                        binding(node);
                        horizontal();
                        if (peek() == '}') {
                            break;
                        }
                        expect(",");
                        horizontal();
                    }
                }
                position++;
                node.sealed = true;
                return node;
            }
            return scalar(depth);
        }
    }

    static final class TomlState {
        String source;
        BigInteger input;
        BigInteger depth;
        BigInteger nodes;
        boolean parsed;
        Node root;
        TomlError error;
    }

    private static TomlState parsed(Object handle) {
        TomlState state = (TomlState) handle;
        if (state.parsed) {
            return state;
        }
        state.parsed = true;
        Parser s = new Parser(state.source, state.depth, state.nodes);
        try {
            if (state.input.signum() < 0 || state.depth.compareTo(BigInteger.ONE) < 0
                    || state.depth.compareTo(BigInteger.valueOf(128)) > 0 || state.nodes.compareTo(BigInteger.ONE) < 0) {
                throw s.error("invalid TOML budgets");
            }
            if (BigInteger.valueOf(s.source.length).compareTo(state.input) > 0) {
                throw s.error("maximum TOML input exceeded", state.input.intValue());
            }
            for (int index = 0; index < s.source.length; index++) {
                if (s.source[index] >= 0xd800 && s.source[index] <= 0xdfff) {
                    throw s.error("input is not Unicode scalar text", index);
                }
            }
            Node root = s.table(1, "explicit");
            Node current = root;
            s.space();
            while (s.position < s.source.length) {
                if (s.peek() == '[') {
                    current = s.header(root);
                } else {
                    s.binding(current);
                }
                s.horizontal();
                s.comment();
                if (s.position == s.source.length) {
                    break;
                }
                if (!s.newline()) {
                    throw s.error("expected newline after statement");
                }
                s.space();
            }
            state.root = root;
        } catch (TomlError error) {
            state.error = error;
        } catch (StackOverflowError overflow) {
            state.error = s.error("maximum TOML depth exceeded");
        }
        return state;
    }

    public static Object runtime_toml__toml_parse_state(String source, BigInteger input, BigInteger depth,
            BigInteger nodes) {
        TomlState state = new TomlState();
        state.source = source;
        state.input = input;
        state.depth = depth;
        state.nodes = nodes;
        return state;
    }

    public static boolean runtime_toml__toml_parse_valid(Object handle) {
        return parsed(handle).error == null;
    }

    public static BigInteger runtime_toml__toml_parse_offset(Object handle) {
        TomlError error = parsed(handle).error;
        return BigInteger.valueOf(error == null ? 0 : error.offset);
    }

    public static BigInteger runtime_toml__toml_parse_line(Object handle) {
        TomlError error = parsed(handle).error;
        return BigInteger.valueOf(error == null ? 0 : error.line);
    }

    public static BigInteger runtime_toml__toml_parse_column(Object handle) {
        TomlError error = parsed(handle).error;
        return BigInteger.valueOf(error == null ? 0 : error.column);
    }

    public static String runtime_toml__toml_parse_message(Object handle) {
        TomlError error = parsed(handle).error;
        return error == null ? "" : error.message;
    }

    public static Object runtime_toml__toml_parse_root(Object handle) {
        TomlState state = parsed(handle);
        if (state.error != null) {
            throw fail("core.toml", "invalid_data", "runtime/toml@1::toml_parse_root/1", "invalid TOML has no root");
        }
        return state.root;
    }

    private static Failure unsupported(String field) {
        return fail("core.toml", "invalid_input", "runtime/toml@1::toml_node_" + field + "/1",
            "TOML node does not support " + field);
    }

    public static BigInteger runtime_toml__toml_node_kind(Object handle) {
        return BigInteger.valueOf(((Node) handle).kind);
    }

    public static BigInteger runtime_toml__toml_node_size(Object handle) {
        Node node = (Node) handle;
        if (node.kind == TABLE) {
            return BigInteger.valueOf(node.members.size());
        }
        if (node.kind == ARRAY) {
            return BigInteger.valueOf(node.values.size());
        }
        throw unsupported("size");
    }

    public static BigInteger runtime_toml__toml_node_integer(Object handle) {
        Node node = (Node) handle;
        if (node.kind != INTEGER) {
            throw unsupported("integer");
        }
        return new BigInteger(node.text);
    }

    public static Double runtime_toml__toml_node_real(Object handle) {
        Node node = (Node) handle;
        if (node.kind != REAL) {
            throw unsupported("real");
        }
        return node.real;
    }

    public static boolean runtime_toml__toml_node_boolean(Object handle) {
        Node node = (Node) handle;
        if (node.kind != BOOLEAN && node.kind != INFINITY) {
            throw unsupported("boolean");
        }
        return node.flag;
    }

    public static String runtime_toml__toml_node_text(Object handle) {
        Node node = (Node) handle;
        if (node.kind != TEXT && node.kind != INTEGER) {
            throw unsupported("text");
        }
        return node.text;
    }

    public static List<Object> runtime_toml__toml_node_coordinates(Object handle) {
        Node node = (Node) handle;
        if (node.kind < DATE || node.kind > OFFSET) {
            throw unsupported("coordinates");
        }
        ArrayList<Object> out = new ArrayList<Object>();
        for (long value : node.coordinates) {
            out.add(BigInteger.valueOf(value));
        }
        return out;
    }

    private static int child(Node node, BigInteger position, String name) {
        boolean table = node.kind == TABLE;
        if (!table && !(node.kind == ARRAY && name.equals("toml_node_item"))) {
            throw fail("core.toml", "invalid_input", "runtime/toml@1::" + name + "/2", "TOML node has no such children");
        }
        int size = table ? node.members.size() : node.values.size();
        if (position.signum() < 0 || position.compareTo(BigInteger.valueOf(size)) >= 0) {
            throw fail("core.toml", "invalid_input", "runtime/toml@1::" + name + "/2", "TOML child index outside container");
        }
        return position.intValue();
    }

    public static Object runtime_toml__toml_node_item(Object handle, BigInteger position) {
        Node node = (Node) handle;
        int index = child(node, position, "toml_node_item");
        return node.kind == TABLE ? node.members.get(index) : node.values.get(index);
    }

    public static String runtime_toml__toml_member_name(Object handle, BigInteger position) {
        Node node = (Node) handle;
        return node.names.get(child(node, position, "toml_member_name"));
    }

    // ===================== runtime/html =====================

    private static Map<String, String> entities;

    private static Map<String, String> entities() {
        if (entities != null) {
            return entities;
        }
        HashMap<String, String> loaded = new HashMap<String, String>();
        InputStream stream = MarkupNatives.class.getResourceAsStream("html_entities.dat");
        if (stream == null) {
            throw Rt.trap("HTML entity data of the runtime is missing");
        }
        try (BufferedReader reader = new BufferedReader(new InputStreamReader(stream, StandardCharsets.UTF_8))) {
            String line;
            while ((line = reader.readLine()) != null) {
                String[] parts = line.split("\t");
                StringBuilder value = new StringBuilder();
                for (String hex : parts[1].split(" ")) {
                    value.appendCodePoint(Integer.parseInt(hex, 16));
                }
                loaded.put(parts[0], value.toString());
            }
        } catch (IOException failure) {
            throw Rt.trap("Cannot read HTML entity data: " + failure.getMessage());
        }
        entities = loaded;
        return loaded;
    }

    private static final int[][] NUMERIC_REPLACEMENT = {
        {0x80, 0x20AC}, {0x82, 0x201A}, {0x83, 0x0192}, {0x84, 0x201E}, {0x85, 0x2026}, {0x86, 0x2020},
        {0x87, 0x2021}, {0x88, 0x02C6}, {0x89, 0x2030}, {0x8A, 0x0160}, {0x8B, 0x2039}, {0x8C, 0x0152},
        {0x8E, 0x017D}, {0x91, 0x2018}, {0x92, 0x2019}, {0x93, 0x201C}, {0x94, 0x201D}, {0x95, 0x2022},
        {0x96, 0x2013}, {0x97, 0x2014}, {0x98, 0x02DC}, {0x99, 0x2122}, {0x9A, 0x0161}, {0x9B, 0x203A},
        {0x9C, 0x0153}, {0x9E, 0x017E}, {0x9F, 0x0178},
    };

    private static String numericCharacter(String digits, int base) {
        BigInteger value = new BigInteger(digits, base);
        if (value.signum() == 0 || value.compareTo(BigInteger.valueOf(0x10FFFF)) > 0
                || (value.compareTo(BigInteger.valueOf(0xD800)) >= 0 && value.compareTo(BigInteger.valueOf(0xDFFF)) <= 0)) {
            return "�";
        }
        int point = value.intValue();
        for (int[] replacement : NUMERIC_REPLACEMENT) {
            if (replacement[0] == point) {
                point = replacement[1];
            }
        }
        return new String(Character.toChars(point));
    }

    private static boolean ascii(int c, String set) {
        return c >= 0 && c < 128 && set.indexOf(c) >= 0;
    }

    private static boolean hexDigit(int c) {
        return (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F');
    }

    /** Ссылка {@code &…;} в позиции: {значение, длина} или {@code null}. */
    private static Object[] referenceAt(int[] source, int offset, boolean attribute) {
        int length = source.length;
        if (offset >= length || source[offset] != '&') {
            return null;
        }
        int following = offset + 1;
        if (following >= length) {
            return null;
        }
        if (source[following] == '#') {
            int position = following + 1;
            int base = 10;
            if (position < length && (source[position] == 'x' || source[position] == 'X')) {
                base = 16;
                position++;
            }
            int start = position;
            while (position < length && (base == 16 ? hexDigit(source[position])
                    : source[position] >= '0' && source[position] <= '9')) {
                position++;
            }
            if (position == start) {
                return null;
            }
            String digits = text(source, start, position);
            if (position < length && source[position] == ';') {
                position++;
            }
            return new Object[] {numericCharacter(digits, base), position - offset};
        }
        int available = length - following;
        int maximum = Math.min(available, 32);
        Map<String, String> table = entities();
        for (int amount = maximum; amount > 0; amount--) {
            String name = text(source, following, following + amount);
            String value = table.get(name);
            if (value == null) {
                continue;
            }
            if (attribute && !name.endsWith(";")) {
                int after = following + amount;
                if (after < length && (Character.isLetterOrDigit(source[after]) && source[after] < 128
                        || source[after] == '=')) {
                    continue;
                }
            }
            return new Object[] {value, amount + 1};
        }
        return null;
    }

    private static String decodeReferences(String value, boolean attribute) {
        int[] source = value.codePoints().toArray();
        StringBuilder result = new StringBuilder();
        int offset = 0;
        while (offset < source.length) {
            if (source[offset] == '&') {
                Object[] decoded = referenceAt(source, offset, attribute);
                if (decoded != null) {
                    result.append((String) decoded[0]);
                    offset += (Integer) decoded[1];
                    continue;
                }
            }
            result.appendCodePoint(source[offset]);
            offset++;
        }
        return result.toString();
    }

    static final class Event {
        int kind;
        String name;
        String value;
        String target;
        String data;
        Boolean selfClosing;
        List<String[]> attributes;
        List<Boolean> valued;
    }

    private static void pushText(List<Event> events, String value, boolean raw) {
        if (value.isEmpty()) {
            return;
        }
        String decoded = raw ? value : decodeReferences(value, false);
        if (!events.isEmpty() && events.get(events.size() - 1).kind == 2) {
            Event last = events.get(events.size() - 1);
            last.value = last.value + decoded;
        } else {
            Event event = new Event();
            event.kind = 2;
            event.value = decoded;
            events.add(event);
        }
    }

    private static final Pattern WHITE = Pattern.compile("\\s", Pattern.UNICODE_CHARACTER_CLASS);

    private static boolean space(int c) {
        return c >= 0 && WHITE.matcher(new String(Character.toChars(c))).matches();
    }

    private static int tagEnd(int[] source, int offset) {
        int quote = 0;
        for (int position = offset; position < source.length; position++) {
            int c = source[position];
            if (quote != 0) {
                if (c == quote) {
                    quote = 0;
                }
            } else if (c == '"' || c == '\'') {
                quote = c;
            } else if (c == '>') {
                return position;
            }
        }
        return -1;
    }

    private static String lower(String value) {
        StringBuilder out = new StringBuilder(value.length());
        for (int index = 0; index < value.length(); index++) {
            char c = value.charAt(index);
            out.append(c >= 'A' && c <= 'Z' ? (char) (c + 32) : c);
        }
        return out.toString();
    }

    private static void attributes(int[] body, int position, Event event) {
        event.attributes = new ArrayList<String[]>();
        event.valued = new ArrayList<Boolean>();
        int length = body.length;
        while (position < length) {
            while (position < length && space(body[position])) {
                position++;
            }
            if (position >= length) {
                break;
            }
            int start = position;
            while (position < length && !space(body[position]) && body[position] != '=') {
                position++;
            }
            String name = text(body, start, position);
            if (name.isEmpty()) {
                break;
            }
            name = lower(name);
            while (position < length && space(body[position])) {
                position++;
            }
            boolean valued = false;
            String value = "";
            if (position < length && body[position] == '=') {
                valued = true;
                position++;
                while (position < length && space(body[position])) {
                    position++;
                }
                if (position < length && (body[position] == '"' || body[position] == '\'')) {
                    int quote = body[position++];
                    int valueStart = position;
                    while (position < length && body[position] != quote) {
                        position++;
                    }
                    value = text(body, valueStart, position);
                    if (position < length) {
                        position++;
                    }
                } else {
                    int valueStart = position;
                    while (position < length && !space(body[position])) {
                        position++;
                    }
                    value = text(body, valueStart, position);
                }
                value = decodeReferences(value, true);
            }
            event.attributes.add(new String[] {name, value});
            event.valued.add(valued);
        }
    }

    private static final Pattern TAG_NAME = Pattern.compile("\\A([A-Za-z][^\\s/>]*)", Pattern.UNICODE_CHARACTER_CLASS);
    private static final Pattern END_NAME = Pattern.compile("\\A\\s*([A-Za-z][^\\s/>]*)", Pattern.UNICODE_CHARACTER_CLASS);
    private static final Pattern SELF_CLOSE = Pattern.compile("/\\s*\\z", Pattern.UNICODE_CHARACTER_CLASS);

    private static Object[] startTag(int[] source, int offset) {
        if (source[offset] != '<') {
            return null;
        }
        int end = tagEnd(source, offset + 1);
        if (end < 0) {
            return null;
        }
        String body = text(source, offset + 1, end);
        Matcher name = TAG_NAME.matcher(body);
        if (!name.find()) {
            return null;
        }
        Event event = new Event();
        event.kind = 0;
        event.name = lower(name.group(1));
        int position = name.group(1).codePointCount(0, name.group(1).length());
        Matcher self = SELF_CLOSE.matcher(body);
        boolean selfClosing = self.find();
        if (selfClosing) {
            body = body.substring(0, self.start());
        }
        event.selfClosing = selfClosing;
        attributes(body.codePoints().toArray(), position, event);
        return new Object[] {event, end + 1};
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

    private static Object[] endTag(int[] source, int offset) {
        if (!text(source, offset, Math.min(source.length, offset + 2)).equals("</")) {
            return null;
        }
        int end = indexOf(source, ">", offset + 2);
        if (end < 0) {
            return null;
        }
        Matcher name = END_NAME.matcher(text(source, offset + 2, end));
        if (!name.find()) {
            return null;
        }
        Event event = new Event();
        event.kind = 1;
        event.name = lower(name.group(1));
        return new Object[] {event, end + 1};
    }

    private static String trim(String value) {
        return value.replaceFirst("(?U)\\A\\s+", "").replaceFirst("(?U)\\s+\\z", "");
    }

    private static List<Event> tokenize(String origin) {
        int[] source = origin.codePoints().toArray();
        ArrayList<Event> events = new ArrayList<Event>();
        int offset = 0;
        String rawName = null;
        while (offset < source.length) {
            if (rawName != null) {
                String tail = text(source, offset, source.length);
                Matcher close = Pattern.compile("</\\s*" + Pattern.quote(rawName) + "(?=[\\s/>])",
                    Pattern.CASE_INSENSITIVE | Pattern.UNICODE_CASE | Pattern.UNICODE_CHARACTER_CLASS).matcher(tail);
                if (close.find()) {
                    int position = offset + tail.codePointCount(0, close.start());
                    pushText(events, text(source, offset, position), true);
                    offset = position;
                    rawName = null;
                    continue;
                }
                pushText(events, tail, true);
                break;
            }
            String four = text(source, offset, Math.min(source.length, offset + 4));
            String two = text(source, offset, Math.min(source.length, offset + 2));
            if (four.equals("<!--")) {
                int end = indexOf(source, "-->", offset + 4);
                Event event = new Event();
                event.kind = 3;
                if (end < 0) {
                    event.value = text(source, offset + 4, source.length);
                    events.add(event);
                    break;
                }
                event.value = text(source, offset + 4, end);
                events.add(event);
                offset = end + 3;
                continue;
            }
            if (two.equals("<?")) {
                int end = indexOf(source, "?>", offset + 2);
                int ending = 2;
                if (end < 0) {
                    end = indexOf(source, ">", offset + 2);
                    ending = 1;
                }
                if (end < 0) {
                    end = source.length;
                    ending = 0;
                }
                String body = trim(text(source, offset + 2, end));
                Matcher parts = Pattern.compile("(?s)\\A(\\S+)(?:\\s+(.*))?\\z", Pattern.UNICODE_CHARACTER_CLASS)
                    .matcher(body);
                if (parts.find()) {
                    Event event = new Event();
                    event.kind = 5;
                    event.target = parts.group(1);
                    event.data = parts.group(2) == null ? "" : parts.group(2);
                    events.add(event);
                } else {
                    pushText(events, text(source, offset, end + ending), false);
                }
                offset = end + ending;
                continue;
            }
            if (two.equals("<!")) {
                int end = indexOf(source, ">", offset + 2);
                Event event = new Event();
                event.kind = 4;
                if (end < 0) {
                    event.value = trim(text(source, offset + 2, source.length));
                    events.add(event);
                    break;
                }
                event.value = trim(text(source, offset + 2, end));
                events.add(event);
                offset = end + 1;
                continue;
            }
            if (two.equals("</")) {
                Object[] tag = endTag(source, offset);
                if (tag != null) {
                    events.add((Event) tag[0]);
                    offset = (Integer) tag[1];
                    continue;
                }
            }
            if (source[offset] == '<') {
                Object[] tag = startTag(source, offset);
                if (tag != null) {
                    Event event = (Event) tag[0];
                    events.add(event);
                    offset = (Integer) tag[1];
                    if (!event.selfClosing && (event.name.equals("script") || event.name.equals("style"))) {
                        rawName = event.name;
                    }
                    continue;
                }
            }
            int following = indexOf(source, "<", offset + 1);
            if (following < 0) {
                following = source.length;
            }
            pushText(events, text(source, offset, following), false);
            offset = following;
        }
        return events;
    }

    @SuppressWarnings("unchecked")
    public static Object runtime_html__html_events(String source) {
        return tokenize(source);
    }

    @SuppressWarnings("unchecked")
    private static Event event(Object handle, BigInteger index) {
        List<Event> events = (List<Event>) handle;
        if (index.signum() < 0 || index.compareTo(BigInteger.valueOf(events.size())) >= 0) {
            throw fail("core.html", "invalid_input", "runtime/html@1::html_event/2", "HTML event index is outside the stream");
        }
        return events.get(index.intValue());
    }

    @SuppressWarnings("unchecked")
    public static BigInteger runtime_html__html_event_count(Object handle) {
        return BigInteger.valueOf(((List<Event>) handle).size());
    }

    private static Failure missing(String name, String field) {
        return fail("core.html", "invalid_input", "runtime/html@1::" + name + "/2", "HTML event has no " + field);
    }

    public static BigInteger runtime_html__html_event_kind(Object handle, BigInteger index) {
        return BigInteger.valueOf(event(handle, index).kind);
    }

    public static String runtime_html__html_event_name(Object handle, BigInteger index) {
        Event event = event(handle, index);
        if (event.name == null) {
            throw missing("html_event_name", "name");
        }
        return event.name;
    }

    public static String runtime_html__html_event_value(Object handle, BigInteger index) {
        Event event = event(handle, index);
        if (event.value == null) {
            throw missing("html_event_value", "value");
        }
        return event.value;
    }

    public static String runtime_html__html_event_target(Object handle, BigInteger index) {
        Event event = event(handle, index);
        if (event.target == null) {
            throw missing("html_event_target", "target");
        }
        return event.target;
    }

    public static String runtime_html__html_event_data(Object handle, BigInteger index) {
        Event event = event(handle, index);
        if (event.data == null) {
            throw missing("html_event_data", "data");
        }
        return event.data;
    }

    public static boolean runtime_html__html_event_self_closing(Object handle, BigInteger index) {
        Event event = event(handle, index);
        if (event.selfClosing == null) {
            throw missing("html_event_self_closing", "self_closing");
        }
        return event.selfClosing;
    }

    private static Event tag(Object handle, BigInteger index, String name, int arity) {
        Event event = event(handle, index);
        if (event.kind != 0) {
            throw fail("core.html", "invalid_input", "runtime/html@1::" + name + "/" + arity, "HTML event has no attributes");
        }
        return event;
    }

    public static BigInteger runtime_html__html_attribute_count(Object handle, BigInteger index) {
        return BigInteger.valueOf(tag(handle, index, "html_attribute_count", 2).attributes.size());
    }

    private static int attribute(Event event, BigInteger index, String name) {
        if (index.signum() < 0 || index.compareTo(BigInteger.valueOf(event.attributes.size())) >= 0) {
            throw fail("core.html", "invalid_input", "runtime/html@1::" + name + "/3",
                "HTML attribute index is outside the tag");
        }
        return index.intValue();
    }

    public static String runtime_html__html_attribute_name(Object handle, BigInteger event, BigInteger index) {
        Event tag = tag(handle, event, "html_attribute_name", 3);
        return tag.attributes.get(attribute(tag, index, "html_attribute_name"))[0];
    }

    public static boolean runtime_html__html_attribute_valued(Object handle, BigInteger event, BigInteger index) {
        Event tag = tag(handle, event, "html_attribute_valued", 3);
        return tag.valued.get(attribute(tag, index, "html_attribute_valued"));
    }

    public static String runtime_html__html_attribute_value(Object handle, BigInteger event, BigInteger index) {
        Event tag = tag(handle, event, "html_attribute_value", 3);
        return tag.attributes.get(attribute(tag, index, "html_attribute_value"))[1];
    }
}
