package okkama.rt;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.math.MathContext;
import java.math.RoundingMode;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;

/**
 * Небольшие natives std: {@code runtime/crypto}, {@code runtime/secret},
 * {@code runtime/debug}, {@code runtime/diff}, {@code runtime/math_aggregate},
 * {@code runtime/url_text}, {@code runtime/layout}. Тексты отказов — как у
 * Perl-provider.
 */
public final class MiscNatives {
    private MiscNatives() {}

    private static Failure fail(String domain, String code, String symbol, String message) {
        return NativeFailure.raise(domain, code, symbol, message);
    }

    private static int length(String text) {
        return text.codePointCount(0, text.length());
    }

    /** {@code sprintf('%.Ng', value)} языка C (и строка числа Perl при N=15). */
    static String formatG(double value, int precision) {
        if (Double.isNaN(value)) {
            return "NaN";
        }
        if (Double.isInfinite(value)) {
            return value > 0 ? "Inf" : "-Inf";
        }
        if (value == 0) {
            return 1 / value < 0 ? "-0" : "0";
        }
        BigDecimal rounded = new BigDecimal(value).round(new MathContext(precision, RoundingMode.HALF_EVEN));
        int exponent = rounded.precision() - rounded.scale() - 1;
        String sign = rounded.signum() < 0 ? "-" : "";
        if (exponent < -4 || exponent >= precision) {
            StringBuilder digits = new StringBuilder(rounded.unscaledValue().abs().toString());
            while (digits.length() < precision) {
                digits.append('0');
            }
            String mantissa = (digits.charAt(0) + "." + digits.substring(1)).replaceAll("0+$", "").replaceAll("\\.$", "");
            String power = Integer.toString(Math.abs(exponent));
            return sign + mantissa + "e" + (exponent < 0 ? "-" : "+") + (power.length() < 2 ? "0" + power : power);
        }
        String plain = rounded.abs().setScale(Math.max(0, precision - 1 - exponent), RoundingMode.HALF_EVEN).toPlainString();
        if (plain.indexOf('.') >= 0) {
            plain = plain.replaceAll("0+$", "").replaceAll("\\.$", "");
        }
        return sign + plain;
    }

    // ---- runtime/crypto ----

    private static String digestName(String algorithm) {
        switch (algorithm) {
            case "sha256": return "SHA-256";
            case "sha384": return "SHA-384";
            case "sha512": return "SHA-512";
            case "sha512256": return "SHA-512/256";
            default: return null;
        }
    }

    public static List<Object> runtime_crypto__digest(String algorithm, List<?> value) {
        String name = digestName(algorithm);
        if (name == null) {
            throw fail("core.crypto", "unsupported", "runtime/crypto@1::digest/2", "Unsupported digest algorithm");
        }
        try {
            byte[] digest = MessageDigest.getInstance(name).digest(Natives.toBytes(value));
            return Natives.fromBytes(digest, digest.length);
        } catch (NoSuchAlgorithmException missing) {
            throw fail("core.crypto", "unsupported", "runtime/crypto@1::digest/2", "Unsupported digest algorithm");
        }
    }

    public static List<Object> runtime_crypto__hmac(String algorithm, List<?> key, List<?> value) {
        String name = algorithm.equals("sha256") ? "HmacSHA256" : algorithm.equals("sha384") ? "HmacSHA384"
            : algorithm.equals("sha512") ? "HmacSHA512" : null;
        if (name == null) {
            throw fail("core.crypto", "unsupported", "runtime/crypto@1::hmac/3", "Unsupported HMAC algorithm");
        }
        try {
            Mac mac = Mac.getInstance(name);
            byte[] secret = Natives.toBytes(key);
            // Пустой ключ HMAC дополняется нулями, как и ключ из одного нуля.
            mac.init(new SecretKeySpec(secret.length == 0 ? new byte[1] : secret, name));
            byte[] result = mac.doFinal(Natives.toBytes(value));
            return Natives.fromBytes(result, result.length);
        } catch (java.security.GeneralSecurityException missing) {
            throw fail("core.crypto", "unsupported", "runtime/crypto@1::hmac/3", "Unsupported HMAC algorithm");
        }
    }

    public static boolean runtime_crypto__secure_equal(List<?> first, List<?> second) {
        byte[] left = Natives.toBytes(first);
        byte[] right = Natives.toBytes(second);
        int limit = Math.max(left.length, right.length);
        int difference = left.length ^ right.length;
        for (int index = 0; index < limit; index++) {
            int a = index < left.length ? left[index] & 0xff : 0;
            int b = index < right.length ? right[index] & 0xff : 0;
            difference |= a ^ b;
        }
        return difference == 0;
    }

    // ---- runtime/secret ----

    static final class Secret {
        byte[] bytes;
        boolean closed;
    }

    private static Secret secret(Object handle, String name) {
        Secret state = (Secret) handle;
        if (state.closed) {
            throw fail("core.secret", "already_closed", "runtime/secret@1::" + name + "/1",
                "Native handle is already closed");
        }
        return state;
    }

    public static Object runtime_secret__create(List<?> source) {
        if (source.size() > 1048576) {
            throw fail("core.secret", "invalid_input", "runtime/secret@1::create/1", "secret payload exceeds limit");
        }
        Secret state = new Secret();
        state.bytes = Natives.toBytes(source);
        return state;
    }

    public static BigInteger runtime_secret__size(Object handle) {
        return BigInteger.valueOf(secret(handle, "size").bytes.length);
    }

    public static List<Object> runtime_secret__export(Object handle) {
        byte[] bytes = secret(handle, "export").bytes;
        return Natives.fromBytes(bytes, bytes.length);
    }

    public static boolean runtime_secret__close(Object handle) {
        Secret state = (Secret) handle;
        if (!state.closed) {
            Arrays.fill(state.bytes, (byte) 0);
            state.bytes = null;
            state.closed = true;
        }
        return true;
    }

    public static boolean runtime_secret__closed(Object handle) {
        return ((Secret) handle).closed;
    }

    // ---- runtime/debug ----

    private static final Pattern CONTROL = Pattern.compile("[\\x00-\\x08\\x0b\\x0c\\x0e-\\x1f\\x7f]");

    private static String escaped(String value, char quote) {
        String text = value.replace("\\", "\\\\").replace("\n", "\\n").replace("\t", "\\t").replace("\r", "\\r")
            .replace(String.valueOf(quote), "\\" + quote);
        Matcher control = CONTROL.matcher(text);
        StringBuffer out = new StringBuffer();
        while (control.find()) {
            control.appendReplacement(out,
                Matcher.quoteReplacement(String.format("\\x{%02X}", (int) control.group().charAt(0))));
        }
        control.appendTail(out);
        return quote + out.toString() + quote;
    }

    private static String dump(Object value) {
        if (value instanceof Any) {
            return dump(((Any) value).value);
        }
        if (value instanceof BigInteger) {
            return value.toString();
        }
        if (value instanceof Double) {
            String text = formatG((Double) value, 15);
            return text.matches(".*[.eE].*") ? text : text + ".0";
        }
        if (value instanceof Rat) {
            return Rat.text((Rat) value);
        }
        if (value instanceof Boolean) {
            return ((Boolean) value) ? "true" : "false";
        }
        if (value instanceof String) {
            return escaped((String) value, '"');
        }
        if (value instanceof Integer) {
            return escaped(new String(Character.toChars((Integer) value)), '\'');
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
                if (!first) {
                    out.append(", ");
                }
                first = false;
                out.append(dump(item));
            }
            return out.append(']').toString();
        }
        if (value instanceof Map) {
            StringBuilder out = new StringBuilder("[");
            boolean first = true;
            for (Map.Entry<?, ?> entry : ((Map<?, ?>) value).entrySet()) {
                if (!first) {
                    out.append(", ");
                }
                first = false;
                out.append(dump(entry.getKey())).append(": ").append(dump(entry.getValue()));
            }
            return out.append(']').toString();
        }
        throw Natives.died();
    }

    public static String runtime_debug__data_dump(Any value) {
        return dump(value);
    }

    // ---- runtime/diff (Algorithm::Diff LCSidx) ----

    /** {@code _replaceNextLargerWith}: индекс замены или {@code null}. */
    private static Integer replaceNextLarger(ArrayList<Integer> array, int value, Integer high) {
        int top = high == null || high == 0 ? array.size() - 1 : high;
        if (top == -1 || value > array.get(array.size() - 1)) {
            array.add(value);
            return top + 1;
        }
        int low = 0;
        while (low <= top) {
            int index = (top + low) / 2;
            int found = array.get(index);
            if (value == found) {
                return null;
            } else if (value > found) {
                low = index + 1;
            } else {
                top = index - 1;
            }
        }
        array.set(low, value);
        return low;
    }

    private static Integer[] lcs(List<String> a, List<String> b) {
        Integer[] match = new Integer[a.size()];
        int aStart = 0;
        int aFinish = a.size() - 1;
        int bStart = 0;
        int bFinish = b.size() - 1;
        while (aStart <= aFinish && bStart <= bFinish && a.get(aStart).equals(b.get(bStart))) {
            match[aStart++] = bStart++;
        }
        while (aStart <= aFinish && bStart <= bFinish && a.get(aFinish).equals(b.get(bFinish))) {
            match[aFinish--] = bFinish--;
        }
        HashMap<String, ArrayList<Integer>> positions = new HashMap<String, ArrayList<Integer>>();
        for (int index = bStart; index <= bFinish; index++) {
            ArrayList<Integer> list = positions.get(b.get(index));
            if (list == null) {
                list = new ArrayList<Integer>();
                positions.put(b.get(index), list);
            }
            list.add(0, index);
        }
        ArrayList<Integer> thresh = new ArrayList<Integer>();
        ArrayList<Link> links = new ArrayList<Link>();
        for (int i = aStart; i <= aFinish; i++) {
            ArrayList<Integer> matches = positions.get(a.get(i));
            if (matches == null) {
                continue;
            }
            Integer k = 0;
            for (int j : matches) {
                if (k != null && k != 0 && thresh.get(k) > j && thresh.get(k - 1) < j) {
                    thresh.set(k, j);
                } else {
                    k = replaceNextLarger(thresh, j, k);
                }
                if (k != null) {
                    while (links.size() <= k) {
                        links.add(null);
                    }
                    links.set(k, new Link(k > 0 ? links.get(k - 1) : null, i, j));
                }
            }
        }
        if (!thresh.isEmpty()) {
            for (Link link = links.get(thresh.size() - 1); link != null; link = link.previous) {
                match[link.i] = link.j;
            }
        }
        return match;
    }

    /** Звено цепочки LCS: {@code [предыдущее, i, j]} Algorithm::Diff. */
    private static final class Link {
        final Link previous;
        final int i;
        final int j;

        Link(Link previous, int i, int j) {
            this.previous = previous;
            this.i = i;
            this.j = j;
        }
    }

    private static List<String> lines(List<?> values, String symbol) {
        if (values.size() > 512) {
            throw fail("core.diff", "invalid_data", symbol, "Diff requires at most 512 lines");
        }
        ArrayList<String> out = new ArrayList<String>();
        long scalars = 0;
        for (Object value : values) {
            String line = (String) value;
            int size = length(line);
            boolean surrogate = false;
            for (int index = 0; index < line.length(); index++) {
                char unit = line.charAt(index);
                if (Character.isSurrogate(unit) && !(Character.isHighSurrogate(unit) && index + 1 < line.length()
                        && Character.isLowSurrogate(line.charAt(index + 1)))) {
                    surrogate = true;
                }
                if (Character.isHighSurrogate(unit)) {
                    index++;
                }
            }
            if (size > 4096 || line.indexOf('\n') >= 0 || surrogate) {
                throw fail("core.diff", "invalid_data", symbol, "Invalid diff line");
            }
            scalars += size;
            out.add(line);
        }
        if (scalars > 1048576) {
            throw fail("core.diff", "invalid_data", symbol, "Diff input exceeds 1048576 scalars");
        }
        return out;
    }

    public static List<Object> runtime_diff__diff_script(List<?> leftValues, List<?> rightValues) {
        String symbol = "runtime/diff@1::diff_script/2";
        List<String> left = lines(leftValues, symbol);
        List<String> right = lines(rightValues, symbol);
        Integer[] match = lcs(left, right);
        ArrayList<Object> script = new ArrayList<Object>();
        int x = 0;
        int y = 0;
        for (int index = 0; index < match.length; index++) {
            if (match[index] == null) {
                continue;
            }
            int nextX = index;
            int nextY = match[index];
            while (x < nextX) {
                add(script, 1, x++, y);
            }
            while (y < nextY) {
                add(script, 2, x, y++);
            }
            add(script, 0, x++, y++);
        }
        while (x < left.size()) {
            add(script, 1, x++, y);
        }
        while (y < right.size()) {
            add(script, 2, x, y++);
        }
        return script;
    }

    private static void add(List<Object> script, int kind, int x, int y) {
        script.add(BigInteger.valueOf(kind));
        script.add(BigInteger.valueOf(x));
        script.add(BigInteger.valueOf(y));
    }

    // ---- runtime/math_aggregate ----

    public static Double runtime_math_aggregate__aggregate_square_root(Double value) {
        if (value < 0) {
            throw fail("core.math_aggregate", "invalid_input", "runtime/math_aggregate@1::aggregate_square_root/1",
                "Square-root input must be non-negative");
        }
        return Math.sqrt(value);
    }

    // ---- runtime/url_text ----

    public static String runtime_url_text__url_dot_segments(String source) {
        String symbol = "runtime/url_text@1::url_dot_segments/1";
        boolean invalid = length(source) > 8192;
        for (int index = 0; index < source.length() && !invalid; index++) {
            char unit = source.charAt(index);
            if (Character.isHighSurrogate(unit) && index + 1 < source.length()
                    && Character.isLowSurrogate(source.charAt(index + 1))) {
                index++;
            } else if (Character.isSurrogate(unit)) {
                invalid = true;
            }
        }
        if (invalid) {
            throw fail("core.url_text", "invalid_input", symbol, "invalid bounded dot path");
        }
        String[] parts = source.split("/", -1);
        if (parts.length > 1024) {
            throw fail("core.url_text", "invalid_input", symbol, "dot path segment limit exceeded");
        }
        ArrayList<String> stack = new ArrayList<String>();
        boolean history = false;
        for (int at = 0; at < parts.length; at++) {
            String part = parts[at];
            if (part.equals(".") || part.equals("..")) {
                if (part.equals("..") && !stack.isEmpty()) {
                    stack.remove(stack.size() - 1);
                }
                if (at == parts.length - 1 && history) {
                    stack.add("/");
                }
            } else {
                stack.add((history ? "/" : "") + part);
                history = true;
            }
        }
        StringBuilder out = new StringBuilder();
        for (String piece : stack) {
            out.append(piece);
        }
        return out.toString();
    }

    // ---- runtime/layout ----

    private static final Pattern LINE = Pattern.compile("\\G([^\\r\\n]*)(\\r\\n|\\n|\\r|\\z)");
    private static final Pattern NON_SPACE = Pattern.compile("\\S", Pattern.UNICODE_CHARACTER_CLASS);
    private static final Pattern SPACES = Pattern.compile("\\s+", Pattern.UNICODE_CHARACTER_CLASS);

    private static List<String[]> lines(String source) {
        ArrayList<String[]> out = new ArrayList<String[]>();
        Matcher matcher = LINE.matcher(source);
        int position = 0;
        while (position < source.length() && matcher.find(position)) {
            out.add(new String[] {matcher.group(1), matcher.group(2)});
            if (matcher.end() == position) {
                break;
            }
            position = matcher.end();
        }
        return out;
    }

    public static String runtime_layout__indent(String origin, String prefix) {
        StringBuilder out = new StringBuilder();
        for (String[] line : lines(origin)) {
            out.append(NON_SPACE.matcher(line[0]).find() ? prefix : "").append(line[0]).append(line[1]);
        }
        return out.toString();
    }

    public static String runtime_layout__dedent(String origin) {
        List<String[]> lines = lines(origin);
        String margin = null;
        for (String[] line : lines) {
            if (line[0].matches("[ \\t]*")) {
                line[0] = "";
                continue;
            }
            Matcher indent = Pattern.compile("\\A[ \\t]*").matcher(line[0]);
            indent.find();
            String current = indent.group();
            if (margin == null) {
                margin = current;
            } else {
                int limit = Math.min(margin.length(), current.length());
                int offset = 0;
                while (offset < limit && margin.charAt(offset) == current.charAt(offset)) {
                    offset++;
                }
                margin = margin.substring(0, offset);
            }
        }
        int width = margin == null ? 0 : margin.length();
        StringBuilder out = new StringBuilder();
        for (String[] line : lines) {
            out.append(line[0].isEmpty() ? "" : line[0].substring(width)).append(line[1]);
        }
        return out.toString();
    }

    private static String normalized(String source) {
        String text = SPACES.matcher(source).replaceAll(" ");
        if (text.startsWith(" ")) {
            text = text.substring(1);
        }
        if (text.endsWith(" ")) {
            text = text.substring(0, text.length() - 1);
        }
        return text;
    }

    public static String runtime_layout__wrap(String origin, BigInteger width) {
        if (width.signum() <= 0) {
            throw fail("core.layout", "invalid_argument", "runtime/layout@1::wrap/2", "wrap width must be positive");
        }
        String text = normalized(origin);
        if (text.isEmpty()) {
            return "";
        }
        ArrayList<String> out = new ArrayList<String>();
        String current = "";
        for (String word : text.split(" ")) {
            String candidate = current.isEmpty() ? word : current + " " + word;
            if (current.isEmpty() || BigInteger.valueOf(length(candidate)).compareTo(width) <= 0) {
                current = candidate;
            } else {
                out.add(current);
                current = word;
            }
        }
        out.add(current);
        return String.join("\n", out);
    }

    public static String runtime_layout__shorten(String origin, BigInteger width, String placeholder) {
        String symbol = "runtime/layout@1::shorten/3";
        if (width.signum() < 0) {
            throw fail("core.layout", "invalid_argument", symbol, "shorten width cannot be negative");
        }
        if (BigInteger.valueOf(length(placeholder)).compareTo(width) > 0) {
            throw fail("core.layout", "invalid_argument", symbol, "shorten placeholder exceeds width");
        }
        String text = normalized(origin);
        if (BigInteger.valueOf(length(text)).compareTo(width) <= 0) {
            return text;
        }
        String current = "";
        for (String word : text.split(" ")) {
            String candidate = current.isEmpty() ? word : current + " " + word;
            if (BigInteger.valueOf(length(candidate) + length(placeholder)).compareTo(width) > 0) {
                break;
            }
            current = candidate;
        }
        return current.isEmpty() ? placeholder : current + placeholder;
    }
}
