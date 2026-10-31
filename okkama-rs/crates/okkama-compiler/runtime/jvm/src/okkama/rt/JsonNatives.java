package okkama.rt;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;

/**
 * Natives {@code runtime/json}: строгий разбор JSON (RFC 8259) в дерево
 * узлов. Смещения, строки и столбцы — в символах (code points), как у
 * Perl-provider; тексты отказов те же.
 */
public final class JsonNatives {
    private JsonNatives() {}

    private static final String DOMAIN = "core.json";
    private static final BigInteger INT_MIN = BigInteger.valueOf(Long.MIN_VALUE);
    private static final BigInteger INT_MAX = BigInteger.valueOf(Long.MAX_VALUE);

    static final class ParseState {
        final int[] source;
        final BigInteger maximumDepth;
        boolean parsed;
        boolean valid;
        Node root;
        int offset;
        int line;
        int column;
        String message = "";
        String duplicate;
        int position;

        ParseState(String source, BigInteger maximumDepth) {
            this.source = source.codePoints().toArray();
            this.maximumDepth = maximumDepth;
        }
    }

    static final class Node {
        int kind;
        boolean flag;
        String value;
        ArrayList<Node> values;
        ArrayList<String> names;
    }

    private static final class ParseError extends RuntimeException {
        private static final long serialVersionUID = 1L;
        final int offset;
        final String message;
        final String duplicate;

        ParseError(int offset, String message, String duplicate) {
            super(message, null, false, false);
            this.offset = offset;
            this.message = message;
            this.duplicate = duplicate;
        }
    }

    private static Failure failure(String code, String name, int arity, String message) {
        return NativeFailure.raise(DOMAIN, code, "runtime/json@1::" + name + "/" + arity, message);
    }

    private static ParseError error(ParseState state, String message) {
        return new ParseError(state.position, message, null);
    }

    private static int at(ParseState state, int position) {
        return position < state.source.length ? state.source[position] : -1;
    }

    private static boolean digit(int character) {
        return character >= '0' && character <= '9';
    }

    private static void skipWhitespace(ParseState state) {
        while (state.position < state.source.length) {
            int character = state.source[state.position];
            if (character != ' ' && character != '\t' && character != '\n' && character != '\r') {
                return;
            }
            state.position++;
        }
    }

    private static int hex4(ParseState state, int position) {
        if (position + 4 > state.source.length) {
            return -1;
        }
        int value = 0;
        for (int index = position; index < position + 4; index++) {
            int digit = Character.digit(state.source[index], 16);
            if (digit < 0 || state.source[index] > 'f') {
                return -1;
            }
            value = value * 16 + digit;
        }
        return value;
    }

    private static String string(ParseState state) {
        if (at(state, state.position) != '"') {
            throw error(state, "expected JSON string");
        }
        state.position++;
        StringBuilder result = new StringBuilder();
        while (state.position < state.source.length) {
            int offset = state.position;
            int character = state.source[state.position++];
            if (character == '"') {
                return result.toString();
            }
            if (character < 0x20) {
                throw new ParseError(offset, "unescaped control character in JSON string", null);
            }
            if (character >= 0xD800 && character <= 0xDFFF) {
                throw new ParseError(offset, "unpaired UTF-16 surrogate in JSON string", null);
            }
            if (character != '\\') {
                result.appendCodePoint(character);
                continue;
            }
            if (state.position >= state.source.length) {
                throw new ParseError(offset, "unfinished JSON escape", null);
            }
            int escape = state.source[state.position++];
            switch (escape) {
                case '"': result.append('"'); continue;
                case '\\': result.append('\\'); continue;
                case '/': result.append('/'); continue;
                case 'b': result.append('\b'); continue;
                case 'f': result.append('\f'); continue;
                case 'n': result.append('\n'); continue;
                case 'r': result.append('\r'); continue;
                case 't': result.append('\t'); continue;
                case 'u': break;
                default: throw new ParseError(offset, "invalid JSON escape", null);
            }
            if (state.position + 4 > state.source.length) {
                throw new ParseError(offset, "incomplete Unicode escape", null);
            }
            int codepoint = hex4(state, state.position);
            if (codepoint < 0) {
                throw new ParseError(offset, "invalid Unicode escape", null);
            }
            state.position += 4;
            if (codepoint >= 0xD800 && codepoint <= 0xDBFF) {
                if (!(at(state, state.position) == '\\' && at(state, state.position + 1) == 'u'
                        && state.position + 6 <= state.source.length)) {
                    throw new ParseError(offset, "unpaired UTF-16 high surrogate", null);
                }
                int low = hex4(state, state.position + 2);
                if (low < 0xDC00 || low > 0xDFFF) {
                    throw new ParseError(offset, "unpaired UTF-16 high surrogate", null);
                }
                state.position += 6;
                codepoint = 0x10000 + ((codepoint - 0xD800) << 10) + (low - 0xDC00);
            } else if (codepoint >= 0xDC00 && codepoint <= 0xDFFF) {
                throw new ParseError(offset, "unpaired UTF-16 low surrogate", null);
            }
            result.appendCodePoint(codepoint);
        }
        throw error(state, "unterminated JSON string");
    }

    private static String number(ParseState state) {
        int start = state.position;
        if (at(state, state.position) == '-') {
            state.position++;
        }
        if (!digit(at(state, state.position))) {
            throw error(state, "expected digit in JSON number");
        }
        int wholeStart = state.position;
        if (at(state, state.position) == '0') {
            state.position++;
            if (digit(at(state, state.position))) {
                throw error(state, "leading zero in JSON number");
            }
        } else {
            while (digit(at(state, state.position))) {
                state.position++;
            }
        }
        int wholeEnd = state.position;
        int fractionStart = state.position;
        int fractionEnd = state.position;
        if (at(state, state.position) == '.') {
            state.position++;
            if (!digit(at(state, state.position))) {
                throw error(state, "expected digit after decimal point");
            }
            fractionStart = state.position;
            while (digit(at(state, state.position))) {
                state.position++;
            }
            fractionEnd = state.position;
        }
        String written = "0";
        if (at(state, state.position) == 'e' || at(state, state.position) == 'E') {
            state.position++;
            int exponentStart = state.position;
            if (at(state, state.position) == '+' || at(state, state.position) == '-') {
                state.position++;
            }
            if (!digit(at(state, state.position))) {
                throw error(state, "expected digit in JSON exponent");
            }
            while (digit(at(state, state.position))) {
                state.position++;
            }
            written = new String(state.source, exponentStart, state.position - exponentStart);
        }
        String digits = new String(state.source, wholeStart, wholeEnd - wholeStart)
            + new String(state.source, fractionStart, fractionEnd - fractionStart);
        BigInteger exponent = new BigInteger(written.startsWith("+") ? written.substring(1) : written)
            .subtract(BigInteger.valueOf(fractionEnd - fractionStart));
        digits = digits.replaceFirst("^0+", "");
        if (!digits.isEmpty()) {
            int zeros = 0;
            while (zeros < digits.length() && digits.charAt(digits.length() - 1 - zeros) == '0') {
                zeros++;
            }
            exponent = exponent.add(BigInteger.valueOf(zeros));
        }
        if (exponent.compareTo(INT_MIN) < 0 || exponent.compareTo(INT_MAX) > 0) {
            throw new ParseError(start, "JSON number exponent is outside supported range", null);
        }
        return new String(state.source, start, state.position - start);
    }

    private static Node array(ParseState state, long depth) {
        state.position++;
        skipWhitespace(state);
        Node node = new Node();
        node.kind = 4;
        node.values = new ArrayList<Node>();
        if (at(state, state.position) == ']') {
            state.position++;
            return node;
        }
        while (true) {
            node.values.add(value(state, depth + 1));
            skipWhitespace(state);
            int character = at(state, state.position);
            if (character == ']') {
                state.position++;
                return node;
            }
            if (character != ',') {
                throw error(state, "expected comma or closing bracket in JSON array");
            }
            state.position++;
            skipWhitespace(state);
            if (at(state, state.position) == ']') {
                throw error(state, "trailing comma in JSON array");
            }
        }
    }

    private static Node object(ParseState state, long depth) {
        state.position++;
        skipWhitespace(state);
        Node node = new Node();
        node.kind = 5;
        node.values = new ArrayList<Node>();
        node.names = new ArrayList<String>();
        HashSet<String> seen = new HashSet<String>();
        if (at(state, state.position) == '}') {
            state.position++;
            return node;
        }
        while (true) {
            int nameOffset = state.position;
            String name = string(state);
            if (!seen.add(name)) {
                throw new ParseError(nameOffset, "duplicate JSON member \"" + name + "\"", name);
            }
            skipWhitespace(state);
            if (at(state, state.position) != ':') {
                throw error(state, "expected colon after JSON member name");
            }
            state.position++;
            node.names.add(name);
            node.values.add(value(state, depth + 1));
            skipWhitespace(state);
            int character = at(state, state.position);
            if (character == '}') {
                state.position++;
                return node;
            }
            if (character != ',') {
                throw error(state, "expected comma or closing brace in JSON object");
            }
            state.position++;
            skipWhitespace(state);
            if (at(state, state.position) == '}') {
                throw error(state, "trailing comma in JSON object");
            }
        }
    }

    private static boolean literal(ParseState state, String word) {
        for (int index = 0; index < word.length(); index++) {
            if (at(state, state.position + index) != word.charAt(index)) {
                return false;
            }
        }
        state.position += word.length();
        return true;
    }

    private static Node value(ParseState state, long depth) {
        skipWhitespace(state);
        if (BigInteger.valueOf(depth).compareTo(state.maximumDepth) > 0) {
            throw error(state, "maximum JSON depth exceeded");
        }
        if (state.position >= state.source.length) {
            throw error(state, "expected JSON value");
        }
        int character = state.source[state.position];
        Node node = new Node();
        if (character == '"') {
            node.kind = 3;
            node.value = string(state);
            return node;
        }
        if (character == '[') {
            return array(state, depth);
        }
        if (character == '{') {
            return object(state, depth);
        }
        if (character == '-' || digit(character)) {
            node.kind = 2;
            node.value = number(state);
            return node;
        }
        if (literal(state, "null")) {
            node.kind = 0;
            return node;
        }
        if (literal(state, "false")) {
            node.kind = 1;
            return node;
        }
        if (literal(state, "true")) {
            node.kind = 1;
            node.flag = true;
            return node;
        }
        throw error(state, "expected JSON value");
    }

    private static ParseState parsed(Object handle) {
        ParseState state = (ParseState) handle;
        if (state.parsed) {
            return state;
        }
        state.parsed = true;
        try {
            state.root = value(state, 1);
            skipWhitespace(state);
            if (state.position != state.source.length) {
                throw error(state, "trailing input after JSON value");
            }
            state.valid = true;
        } catch (ParseError failure) {
            state.valid = false;
            state.offset = failure.offset;
            state.message = failure.message;
            state.duplicate = failure.duplicate;
            int line = 1;
            int column = 1;
            int position = 0;
            while (position < failure.offset) {
                int character = state.source[position];
                if (character == '\r') {
                    position++;
                    if (position < failure.offset && state.source[position] == '\n') {
                        position++;
                    }
                    line++;
                    column = 1;
                    continue;
                }
                if (character == '\n') {
                    line++;
                    column = 1;
                } else {
                    column++;
                }
                position++;
            }
            state.line = line;
            state.column = column;
        } catch (StackOverflowError overflow) {
            state.valid = false;
            state.offset = state.position;
            state.message = "maximum JSON depth exceeded";
        }
        return state;
    }

    public static Object runtime_json__json_parse_state(String source, BigInteger maximumDepth) {
        return new ParseState(source, maximumDepth);
    }

    public static boolean runtime_json__json_parse_valid(Object handle) {
        return parsed(handle).valid;
    }

    public static boolean runtime_json__json_parse_has_duplicate(Object handle) {
        return parsed(handle).duplicate != null;
    }

    public static BigInteger runtime_json__json_parse_offset(Object handle) {
        ParseState state = parsed(handle);
        return BigInteger.valueOf(state.valid ? 0 : state.offset);
    }

    public static BigInteger runtime_json__json_parse_line(Object handle) {
        ParseState state = parsed(handle);
        return BigInteger.valueOf(state.valid ? 0 : state.line);
    }

    public static BigInteger runtime_json__json_parse_column(Object handle) {
        ParseState state = parsed(handle);
        return BigInteger.valueOf(state.valid ? 0 : state.column);
    }

    public static String runtime_json__json_parse_message(Object handle) {
        return parsed(handle).message;
    }

    public static String runtime_json__json_parse_duplicate(Object handle) {
        String duplicate = parsed(handle).duplicate;
        return duplicate == null ? "" : duplicate;
    }

    public static Object runtime_json__json_parse_root(Object handle) {
        ParseState state = parsed(handle);
        if (!state.valid) {
            throw failure("invalid_data", "json_parse_root", 1, "invalid JSON has no root");
        }
        return state.root;
    }

    public static BigInteger runtime_json__json_node_kind(Object handle) {
        return BigInteger.valueOf(((Node) handle).kind);
    }

    public static boolean runtime_json__json_node_boolean(Object handle) {
        Node node = (Node) handle;
        if (node.kind != 1) {
            throw failure("invalid_input", "json_node_boolean", 1, "JSON node is not boolean");
        }
        return node.flag;
    }

    public static String runtime_json__json_node_number(Object handle) {
        Node node = (Node) handle;
        if (node.kind != 2) {
            throw failure("invalid_input", "json_node_number", 1, "JSON node has the wrong kind");
        }
        return node.value;
    }

    public static String runtime_json__json_node_text(Object handle) {
        Node node = (Node) handle;
        if (node.kind != 3) {
            throw failure("invalid_input", "json_node_text", 1, "JSON node has the wrong kind");
        }
        return node.value;
    }

    public static BigInteger runtime_json__json_node_size(Object handle) {
        Node node = (Node) handle;
        if (node.kind != 4 && node.kind != 5) {
            throw failure("invalid_input", "json_node_size", 1, "JSON node has no children");
        }
        return BigInteger.valueOf(node.values.size());
    }

    private static int index(Node node, BigInteger index, String name, String message) {
        if (index.signum() < 0 || index.compareTo(BigInteger.valueOf(node.values.size())) >= 0) {
            throw failure("invalid_input", name, 2, message);
        }
        return index.intValue();
    }

    public static Object runtime_json__json_node_item(Object handle, BigInteger position) {
        Node node = (Node) handle;
        if (node.kind != 4) {
            throw failure("invalid_input", "json_node_item", 2, "JSON node is not an array");
        }
        return node.values.get(index(node, position, "json_node_item", "JSON array index is outside the node"));
    }

    public static String runtime_json__json_member_name(Object handle, BigInteger position) {
        Node node = (Node) handle;
        if (node.kind != 5) {
            throw failure("invalid_input", "json_member_name", 2, "JSON node is not an object");
        }
        return node.names.get(index(node, position, "json_member_name", "JSON member index is outside the node"));
    }

    public static Object runtime_json__json_member_value(Object handle, BigInteger position) {
        Node node = (Node) handle;
        if (node.kind != 5) {
            throw failure("invalid_input", "json_member_value", 2, "JSON node is not an object");
        }
        return node.values.get(index(node, position, "json_member_value", "JSON member index is outside the node"));
    }

    public static BigInteger runtime_json__json_member_position(Object handle, String name) {
        Node node = (Node) handle;
        if (node.kind != 5) {
            throw failure("invalid_input", "json_member_position", 2, "JSON node is not an object");
        }
        return BigInteger.valueOf(node.names.indexOf(name));
    }

    // A JSON string literal: '"' and backslash escaped, \n, \r, \t by name, the
    // other controls U+0000..U+001F as lowercase u00xx escapes, everything else as is.
    public static String runtime_json__json_quote(String origin) {
        StringBuilder quoted = new StringBuilder(origin.length() + 2);
        appendQuoted(quoted, origin);
        return quoted.toString();
    }

    private static void appendQuoted(StringBuilder quoted, String origin) {
        quoted.append('"');
        for (int at = 0; at < origin.length(); at++) {
            char character = origin.charAt(at);
            switch (character) {
                case '"': quoted.append("\\\""); break;
                case '\\': quoted.append("\\\\"); break;
                case '\n': quoted.append("\\n"); break;
                case '\r': quoted.append("\\r"); break;
                case '\t': quoted.append("\\t"); break;
                default:
                    if (character < 0x20) {
                        quoted.append(String.format("\\u%04x", (int) character));
                    } else {
                        quoted.append(character);
                    }
            }
        }
        quoted.append('"');
    }

    // The JSON spelling of an exact decimal, as json.Json writes it:
    // positional for integers up to 21 digits and fractions down to 1e-6, the
    // canonical scientific form beyond them.
    public static String runtime_json__json_number_text(String origin) {
        ValueNatives.Decimal parts = ValueNatives.components(origin);
        if (parts == null) {
            throw failure("invalid_input", "json_number_text", 1, "not a decimal number");
        }
        String sign = parts.sign < 0 ? "-" : "";
        BigInteger length = BigInteger.valueOf(parts.digits.length());
        if (parts.exponent.signum() >= 0) {
            if (length.add(parts.exponent).compareTo(BigInteger.valueOf(21)) <= 0) {
                return sign + parts.digits + zeros(parts.exponent.intValue());
            }
            return ValueNatives.runtime_decimal__decimal_canonical(origin);
        }
        BigInteger point = length.add(parts.exponent);
        if (point.signum() > 0) {
            int at = point.intValue();
            return sign + parts.digits.substring(0, at) + "." + parts.digits.substring(at);
        }
        if (point.negate().compareTo(BigInteger.valueOf(5)) <= 0) {
            return sign + "0." + zeros(point.negate().intValue()) + parts.digits;
        }
        return ValueNatives.runtime_decimal__decimal_canonical(origin);
    }

    private static String zeros(int count) {
        StringBuilder zeros = new StringBuilder(count);
        for (int at = 0; at < count; at++) {
            zeros.append('0');
        }
        return zeros.toString();
    }

    // JSON text being written: appends go to the end, and every append answers
    // the UTF-8 size of the whole text so far.
    static final class Output {
        final StringBuilder text = new StringBuilder();
        long bytes;

        BigInteger appended(int from) {
            for (int at = from; at < text.length(); at++) {
                char unit = text.charAt(at);
                if (unit < 0x80) {
                    bytes += 1;
                } else if (unit < 0x800) {
                    bytes += 2;
                } else if (Character.isHighSurrogate(unit) && at + 1 < text.length()
                        && Character.isLowSurrogate(text.charAt(at + 1))) {
                    bytes += 4;
                    at++;
                } else {
                    bytes += 3;
                }
            }
            return BigInteger.valueOf(bytes);
        }
    }

    public static Object runtime_json__json_output() {
        return new Output();
    }

    public static BigInteger runtime_json__json_output_raw(Object handle, String raw) {
        Output output = (Output) handle;
        int from = output.text.length();
        output.text.append(raw);
        return output.appended(from);
    }

    public static BigInteger runtime_json__json_output_text(Object handle, String origin) {
        Output output = (Output) handle;
        int from = output.text.length();
        appendQuoted(output.text, origin);
        return output.appended(from);
    }

    public static BigInteger runtime_json__json_output_number(Object handle, String origin) {
        Output output = (Output) handle;
        int from = output.text.length();
        output.text.append(runtime_json__json_number_text(origin));
        return output.appended(from);
    }

    public static BigInteger runtime_json__json_output_member(Object handle, String name, boolean first) {
        Output output = (Output) handle;
        int from = output.text.length();
        if (!first) {
            output.text.append(',');
        }
        appendQuoted(output.text, name);
        output.text.append(':');
        return output.appended(from);
    }

    public static String runtime_json__json_output_string(Object handle) {
        return ((Output) handle).text.toString();
    }

    // The first index whose name repeats an earlier one, or the count.
    public static BigInteger runtime_json__json_repeated_name(List<?> names) {
        HashSet<Object> seen = new HashSet<Object>();
        for (int index = 0; index < names.size(); index++) {
            if (!seen.add(names.get(index))) {
                return BigInteger.valueOf(index);
            }
        }
        return BigInteger.valueOf(names.size());
    }

    // The member `name` of an object node, or null when the node is not an
    // object or has no such member.
    private static Node member(Object handle, String name) {
        Node node = (Node) handle;
        if (node.kind != 5) {
            return null;
        }
        int at = node.names.indexOf(name);
        return at < 0 ? null : node.values.get(at);
    }

    public static String runtime_json__json_member_text(Object handle, String name) {
        Node member = member(handle, name);
        if (member == null || member.kind != 3) {
            throw failure("invalid_input", "json_member_text", 2, "JSON member is missing or not a string");
        }
        return member.value;
    }

    public static String runtime_json__json_member_spelling(Object handle, String name) {
        Node member = member(handle, name);
        if (member == null || member.kind != 2) {
            throw failure("invalid_input", "json_member_spelling", 2, "JSON member is missing or not a number");
        }
        return member.value;
    }

    public static boolean runtime_json__json_member_boolean(Object handle, String name) {
        Node member = member(handle, name);
        if (member == null || member.kind != 1) {
            throw failure("invalid_input", "json_member_boolean", 2, "JSON member is missing or not boolean");
        }
        return member.flag;
    }

    // Only a plain integer spelling of up to 18 digits: always within int.
    public static BigInteger runtime_json__json_member_int(Object handle, String name) {
        Node member = member(handle, name);
        if (member != null && member.kind == 2) {
            String digits = member.value.startsWith("-") ? member.value.substring(1) : member.value;
            boolean plain = !digits.isEmpty() && digits.length() <= 18;
            for (int at = 0; plain && at < digits.length(); at++) {
                plain = Character.isDigit(digits.charAt(at)) && digits.charAt(at) < 128;
            }
            if (plain) {
                return BigInteger.valueOf(Long.parseLong(member.value));
            }
        }
        throw failure("invalid_input", "json_member_int", 2, "JSON member is missing or not a short integer");
    }

    private static Node leaf(int kind) {
        Node node = new Node();
        node.kind = kind;
        return node;
    }

    public static Object runtime_json__json_text_node(String value) {
        Node node = leaf(3);
        node.value = value;
        return node;
    }

    public static Object runtime_json__json_number_node(String spelling) {
        if (ValueNatives.components(spelling) == null) {
            throw failure("invalid_input", "json_number_node", 1, "not a decimal number");
        }
        Node node = leaf(2);
        node.value = spelling;
        return node;
    }

    public static Object runtime_json__json_boolean_node(boolean flag) {
        Node node = leaf(1);
        node.flag = flag;
        return node;
    }

    public static Object runtime_json__json_null_node() {
        return leaf(0);
    }

    public static Object runtime_json__json_array_node(List<?> values) {
        Node node = leaf(4);
        node.values = new ArrayList<Node>(values.size());
        for (Object value : values) {
            node.values.add((Node) value);
        }
        return node;
    }

    public static Object runtime_json__json_object_node(List<?> names, List<?> values) {
        if (names.size() != values.size()) {
            throw failure("invalid_input", "json_object_node", 2, "JSON member names and values differ in number");
        }
        Node node = leaf(5);
        node.names = new ArrayList<String>(names.size());
        node.values = new ArrayList<Node>(values.size());
        HashSet<String> seen = new HashSet<String>();
        for (int index = 0; index < names.size(); index++) {
            String name = (String) names.get(index);
            if (!seen.add(name)) {
                throw failure("invalid_input", "json_object_node", 2, "duplicate JSON member name");
            }
            node.names.add(name);
            node.values.add((Node) values.get(index));
        }
        return node;
    }

    // JSON text of a node: compact for a negative indentation, otherwise one
    // member or element per line, level * indentation spaces, ": " after
    // names, empty containers as [] and {} and no final line break.
    public static String runtime_json__json_node_write(Object handle, BigInteger indentation) {
        StringBuilder out = new StringBuilder();
        write(out, (Node) handle, indentation.signum() < 0 ? -1 : indentation.intValue(), 0);
        return out.toString();
    }

    private static void line(StringBuilder out, int indentation, int level) {
        if (indentation >= 0) {
            out.append('\n');
            for (int space = 0; space < indentation * level; space++) {
                out.append(' ');
            }
        }
    }

    private static void write(StringBuilder out, Node node, int indentation, int level) {
        switch (node.kind) {
            case 0: out.append("null"); break;
            case 1: out.append(node.flag ? "true" : "false"); break;
            case 2: out.append(runtime_json__json_number_text(node.value)); break;
            case 3: appendQuoted(out, node.value); break;
            case 4:
                out.append('[');
                for (int index = 0; index < node.values.size(); index++) {
                    if (index > 0) {
                        out.append(',');
                    }
                    line(out, indentation, level + 1);
                    write(out, node.values.get(index), indentation, level + 1);
                }
                if (!node.values.isEmpty()) {
                    line(out, indentation, level);
                }
                out.append(']');
                break;
            default:
                out.append('{');
                for (int index = 0; index < node.values.size(); index++) {
                    if (index > 0) {
                        out.append(',');
                    }
                    line(out, indentation, level + 1);
                    appendQuoted(out, node.names.get(index));
                    out.append(indentation >= 0 ? ": " : ":");
                    write(out, node.values.get(index), indentation, level + 1);
                }
                if (!node.values.isEmpty()) {
                    line(out, indentation, level);
                }
                out.append('}');
        }
    }

    // The member names and item indices from root down to target (the same
    // node), depth first, as an RFC 6901 pointer.
    public static String runtime_json__json_node_pointer(Object root, Object target) {
        ArrayList<String> path = new ArrayList<String>();
        if (!findPath((Node) root, (Node) target, path)) {
            throw failure("invalid_input", "json_node_pointer", 2, "JSON node is not inside the document");
        }
        StringBuilder pointer = new StringBuilder();
        for (String token : path) {
            pointer.append('/').append(token.replace("~", "~0").replace("/", "~1"));
        }
        return pointer.toString();
    }

    private static boolean findPath(Node root, Node target, ArrayList<String> path) {
        if (root == target) {
            return true;
        }
        if (root.kind != 4 && root.kind != 5) {
            return false;
        }
        for (int index = 0; index < root.values.size(); index++) {
            path.add(root.kind == 4 ? String.valueOf(index) : root.names.get(index));
            if (findPath(root.values.get(index), target, path)) {
                return true;
            }
            path.remove(path.size() - 1);
        }
        return false;
    }

    public static List<Object> runtime_json__json_member_values(Object handle) {
        Node node = (Node) handle;
        if (node.kind != 5) {
            throw failure("invalid_input", "json_member_values", 1, "JSON node is not an object");
        }
        return new ArrayList<Object>(node.values);
    }

    public static List<Object> runtime_json__json_node_items(Object handle) {
        Node node = (Node) handle;
        if (node.kind != 4) {
            throw failure("invalid_input", "json_node_items", 1, "JSON node is not an array");
        }
        return new ArrayList<Object>(node.values);
    }
}
