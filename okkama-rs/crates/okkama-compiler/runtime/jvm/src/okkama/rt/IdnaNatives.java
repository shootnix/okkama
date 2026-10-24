package okkama.rt;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.text.Normalizer;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.zip.GZIPInputStream;

/**
 * Natives {@code runtime/idna} и {@code runtime/public_suffix}: UTS #46
 * (nontransitional, STD3, без unassigned) как Net::IDN::UTS46 2.591, на
 * таблицах из того же модуля (ресурс {@code idna.dat}, см.
 * {@code tools/jvm-runtime/gen-idna.pl}), и Public Suffix List Perl-provider
 * (ресурс {@code psl.dat}).
 */
public final class IdnaNatives {
    private IdnaNatives() {}

    /** Ошибка UTS #46 (croak модуля): провайдер превращает её в отказ. */
    static final class Invalid extends RuntimeException {
        private static final long serialVersionUID = 1L;

        Invalid(String message) {
            super(message, null, false, false);
        }
    }

    // ---- таблицы ----

    private static final class Tables {
        final UnicodeNatives.Ranges status = new UnicodeNatives.Ranges();
        final Map<Integer, int[]> mapping = new HashMap<Integer, int[]>();
        final UnicodeNatives.Ranges bidi = new UnicodeNatives.Ranges();
        final UnicodeNatives.Ranges joining = new UnicodeNatives.Ranges();
    }

    private static Tables tables;

    private static synchronized Tables tables() {
        if (tables != null) {
            return tables;
        }
        Tables loaded = new Tables();
        InputStream stream = IdnaNatives.class.getResourceAsStream("idna.dat");
        if (stream == null) {
            throw Rt.trap("IDNA data of the runtime is missing");
        }
        try (BufferedReader reader = new BufferedReader(
                new InputStreamReader(new GZIPInputStream(stream), StandardCharsets.UTF_8))) {
            String line;
            while ((line = reader.readLine()) != null) {
                String[] parts = line.split(" ");
                switch (parts[0]) {
                    case "S":
                        loaded.status.add(Integer.parseInt(parts[2], 16), Integer.parseInt(parts[3], 16), parts[1]);
                        break;
                    case "B":
                        loaded.bidi.add(Integer.parseInt(parts[2], 16), Integer.parseInt(parts[3], 16), parts[1]);
                        break;
                    case "J":
                        loaded.joining.add(Integer.parseInt(parts[2], 16), Integer.parseInt(parts[3], 16), parts[1]);
                        break;
                    case "P": {
                        int[] target = new int[parts.length - 2];
                        for (int index = 2; index < parts.length; index++) {
                            target[index - 2] = Integer.parseInt(parts[index], 16);
                        }
                        loaded.mapping.put(Integer.parseInt(parts[1], 16), target);
                        break;
                    }
                    default:
                        break;
                }
            }
        } catch (IOException failure) {
            throw Rt.trap("Cannot read IDNA data of the runtime");
        }
        tables = loaded;
        return loaded;
    }

    private static String status(int point) {
        String status = tables().status.find(point);
        return status == null ? "X" : status;
    }

    private static String bidi(int point) {
        String value = tables().bidi.find(point);
        return value == null ? "L" : value;
    }

    private static String joining(int point) {
        String value = tables().joining.find(point);
        return value == null ? "U" : value;
    }

    private static boolean unassigned(int point) {
        return UnicodeNatives.category(point).equals("Cn");
    }

    // ---- Punycode (RFC 3492) ----

    private static final int BASE = 36;
    private static final int TMIN = 1;
    private static final int TMAX = 26;
    private static final int SKEW = 38;
    private static final int DAMP = 700;
    private static final int INITIAL_BIAS = 72;
    private static final int INITIAL_N = 128;

    private static int adapt(int delta, int points, boolean first) {
        delta = first ? delta / DAMP : delta / 2;
        delta += delta / points;
        int k = 0;
        while (delta > ((BASE - TMIN) * TMAX) / 2) {
            delta /= BASE - TMIN;
            k += BASE;
        }
        return k + (BASE - TMIN + 1) * delta / (delta + SKEW);
    }

    private static char digit(int value) {
        return (char) (value < 26 ? 'a' + value : '0' + value - 26);
    }

    static String encode(int[] input) {
        StringBuilder output = new StringBuilder();
        int basic = 0;
        for (int point : input) {
            if (point < 0x80) {
                output.append((char) point);
                basic++;
            }
        }
        int handled = basic;
        if (basic > 0) {
            output.append('-');
        }
        int n = INITIAL_N;
        long delta = 0;
        int bias = INITIAL_BIAS;
        while (handled < input.length) {
            int m = Integer.MAX_VALUE;
            for (int point : input) {
                if (point >= n && point < m) {
                    m = point;
                }
            }
            delta += (long) (m - n) * (handled + 1);
            if (delta > Integer.MAX_VALUE) {
                throw new Invalid("overflow");
            }
            n = m;
            for (int point : input) {
                if (point < n) {
                    delta++;
                }
                if (point == n) {
                    long q = delta;
                    for (int k = BASE;; k += BASE) {
                        int t = k <= bias ? TMIN : k >= bias + TMAX ? TMAX : k - bias;
                        if (q < t) {
                            break;
                        }
                        output.append(digit((int) (t + (q - t) % (BASE - t))));
                        q = (q - t) / (BASE - t);
                    }
                    output.append(digit((int) q));
                    bias = adapt((int) delta, handled + 1, handled == basic);
                    delta = 0;
                    handled++;
                }
            }
            delta++;
            n++;
        }
        return output.toString();
    }

    static int[] decode(String input) {
        for (int index = 0; index < input.length(); index++) {
            if (input.charAt(index) >= 0x80) {
                throw new Invalid("non-basic");
            }
        }
        ArrayList<Integer> output = new ArrayList<Integer>();
        int delimiter = input.lastIndexOf('-');
        int position = 0;
        if (delimiter > 0) {
            for (int index = 0; index < delimiter; index++) {
                output.add((int) input.charAt(index));
            }
            position = delimiter + 1;
        } else if (delimiter == 0) {
            position = 1;
        }
        int n = INITIAL_N;
        long i = 0;
        int bias = INITIAL_BIAS;
        while (position < input.length()) {
            long oldi = i;
            long w = 1;
            for (int k = BASE;; k += BASE) {
                if (position >= input.length()) {
                    throw new Invalid("truncated");
                }
                char c = input.charAt(position++);
                int value = c >= '0' && c <= '9' ? c - '0' + 26 : c >= 'a' && c <= 'z' ? c - 'a'
                    : c >= 'A' && c <= 'Z' ? c - 'A' : -1;
                if (value < 0) {
                    throw new Invalid("bad digit");
                }
                i += value * w;
                if (i > Integer.MAX_VALUE) {
                    throw new Invalid("overflow");
                }
                int t = k <= bias ? TMIN : k >= bias + TMAX ? TMAX : k - bias;
                if (value < t) {
                    break;
                }
                w *= BASE - t;
                if (w > Integer.MAX_VALUE) {
                    throw new Invalid("overflow");
                }
            }
            bias = adapt((int) (i - oldi), output.size() + 1, oldi == 0);
            n += (int) (i / (output.size() + 1));
            if (n > 0x10FFFF) {
                throw new Invalid("overflow");
            }
            i %= output.size() + 1;
            output.add((int) i, n);
            i++;
        }
        int[] result = new int[output.size()];
        for (int index = 0; index < result.length; index++) {
            result[index] = output.get(index);
        }
        return result;
    }

    // ---- UTS #46 ----

    private static String string(int[] points) {
        return new String(points, 0, points.length);
    }

    private static int[] points(String text) {
        return text.codePoints().toArray();
    }

    private static String nfc(String text) {
        return Normalizer.normalize(text, Normalizer.Form.NFC);
    }

    private static boolean isMark(int point) {
        return UnicodeNatives.category(point).startsWith("M");
    }

    private static void validateLabel(int[] label, boolean assumeNfc) {
        String text = string(label);
        if (!assumeNfc && !text.equals(nfc(text))) {
            throw new Invalid("not in Unicode Normalization Form NFC [V1]");
        }
        if (label.length >= 4 && label[2] == '-' && label[3] == '-') {
            throw new Invalid("contains U+002D HYPHEN-MINUS in both third and forth position [V2]");
        }
        if (label.length > 0 && label[0] == '-') {
            throw new Invalid("begins with U+002D HYPHEN-MINUS [V3]");
        }
        if (label.length > 0 && label[label.length - 1] == '-') {
            throw new Invalid("ends with U+002D HYPHEN-MINUS [V3]");
        }
        for (int point : label) {
            if (point == '.') {
                throw new Invalid("contains U+0023 FULL STOP [V4]");
            }
        }
        if (label.length > 0 && isMark(label[0])) {
            throw new Invalid("begins with General_Category=Mark [V5]");
        }
        for (int point : label) {
            if (unassigned(point)) {
                throw new Invalid("contains unassigned character [V6]");
            }
        }
        for (int point : label) {
            if (status(point).equals("V3")) {
                throw new Invalid("contains disallowed_STD3_valid character [V6]");
            }
        }
        for (int point : label) {
            if (status(point).equals("I")) {
                throw new Invalid("contains ignored character [V6]");
            }
        }
        for (int point : label) {
            String status = status(point);
            if (status.equals("M") || status.equals("M3")) {
                throw new Invalid("contains mapped character [V6]");
            }
        }
        for (int point : label) {
            if (status(point).equals("X")) {
                throw new Invalid("contains disallowed character [V6]");
            }
        }
    }

    private static boolean in(String value, String... options) {
        for (String option : options) {
            if (option.equals(value)) {
                return true;
            }
        }
        return false;
    }

    /** Конец метки: [ends…][NSM или unassigned]*. */
    private static boolean endsWith(int[] label, String... ends) {
        int index = label.length - 1;
        while (index >= 0 && (bidi(label[index]).equals("NSM") || unassigned(label[index]))) {
            if (in(bidi(label[index]), ends)) {
                return true;
            }
            index--;
        }
        return index >= 0 && in(bidi(label[index]), ends);
    }

    private static void validateBidi(int[] label) {
        if (label.length == 0) {
            return;
        }
        String first = bidi(label[0]);
        if (first.equals("L")) {
            for (int point : label) {
                if (!in(bidi(point), "L", "EN", "ES", "CS", "ET", "BN", "ON", "NSM")) {
                    throw new Invalid("contains characters with wrong bidi class for LTR [B5]");
                }
            }
            if (!endsWith(label, "L", "EN")) {
                throw new Invalid("ends with character of wrong bidi class for LTR [B6]");
            }
            return;
        }
        if (first.equals("R") || first.equals("AL")) {
            boolean en = false;
            boolean an = false;
            for (int point : label) {
                String bidi = bidi(point);
                if (!in(bidi, "R", "AL", "AN", "EN", "ES", "CS", "ET", "ON", "BN", "NSM")) {
                    throw new Invalid("contains characters with wrong bidi class for RTL [B2]");
                }
                en |= bidi.equals("EN");
                an |= bidi.equals("AN");
            }
            if (!endsWith(label, "R", "AL", "EN", "AN")) {
                throw new Invalid("ends with character of wrong bidi class for RTL [B3]");
            }
            if (en && an) {
                throw new Invalid("contains characters with both bidi class EN and AN [B4]");
            }
            return;
        }
        throw new Invalid("starts with character of wrong bidi class [B1]");
    }

    private static boolean virama(int point) {
        return UnicodeNatives.combiningClass(point) == 9;
    }

    /** Совпадение `(L|D) T* ZWNJ T* (R|D)` с начала {@code start}. */
    private static boolean joinedZwnj(int[] label, int start) {
        if (!in(joining(label[start]), "L", "D")) {
            return false;
        }
        int index = start + 1;
        while (index < label.length && joining(label[index]).equals("T")) {
            index++;
        }
        if (index >= label.length || label[index] != 0x200C) {
            return false;
        }
        index++;
        while (index < label.length && joining(label[index]).equals("T")) {
            index++;
        }
        return index < label.length && in(joining(label[index]), "R", "D");
    }

    private static void validateContextj(int[] label) {
        if (label.length == 0) {
            return;
        }
        // Первое (самое левое) совпадение альтернатив, как регулярное выражение модуля.
        for (int start = 0; start < label.length; start++) {
            if (virama(label[start]) && start + 1 < label.length && label[start + 1] == 0x200C) {
                break;
            }
            if (joinedZwnj(label, start)) {
                break;
            }
            if (label[start] == 0x200C) {
                throw new Invalid("rule for CONTEXTJ character U+200C not satisfied [C2]");
            }
        }
        for (int start = 0; start < label.length; start++) {
            if (virama(label[start]) && start + 1 < label.length && label[start + 1] == 0x200D) {
                break;
            }
            if (label[start] == 0x200D) {
                throw new Invalid("rule for CONTEXTJ character U+200D not satisfied [C2]");
            }
        }
    }

    /** {@code uts46_to_ascii} / {@code uts46_to_unicode} (STD3, nontransitional). */
    private static String process(String input, boolean ascii) {
        int[] source = points(input);
        for (int point : source) {
            if (status(point).equals("X")) {
                throw new Invalid("disallowed character");
            }
        }
        for (int point : source) {
            if (status(point).equals("V3")) {
                throw new Invalid("disallowed_STD3_valid character");
            }
        }
        for (int point : source) {
            if (status(point).equals("M3")) {
                throw new Invalid("disallowed_STD3_mapped character");
            }
        }
        StringBuilder mapped = new StringBuilder();
        for (int point : source) {
            String status = status(point);
            if (status.equals("I")) {
                continue;
            }
            if (status.equals("M")) {
                for (int target : tables().mapping.get(point)) {
                    mapped.appendCodePoint(target);
                }
                continue;
            }
            mapped.appendCodePoint(point);
        }
        String label = nfc(mapped.toString());
        ArrayList<String> labels = new ArrayList<String>();
        for (String part : label.split("\\.", -1)) {
            labels.add(part);
        }
        boolean rooted = !labels.isEmpty() && labels.get(labels.size() - 1).isEmpty();
        if (rooted) {
            labels.remove(labels.size() - 1);
        }
        boolean isBidi = false;
        ArrayList<int[]> decoded = new ArrayList<int[]>();
        for (String part : labels) {
            int[] value;
            if (part.length() > 4 && part.substring(0, 4).equalsIgnoreCase("xn--") && part.substring(4).matches("[\\x00-\\x7f]+")) {
                try {
                    value = decode(part.substring(4));
                } catch (Invalid invalid) {
                    throw new Invalid("Invalid Punycode sequence [P4]");
                }
                validateLabel(value, false);
            } else {
                value = points(part);
                validateLabel(value, true);
            }
            for (int point : value) {
                if (in(bidi(point), "R", "AL", "AN")) {
                    isBidi = true;
                }
            }
            decoded.add(value);
        }
        ArrayList<String> out = new ArrayList<String>();
        for (int[] value : decoded) {
            if (isBidi) {
                validateBidi(value);
            }
            validateContextj(value);
            String text = string(value);
            if (ascii) {
                boolean nonAscii = false;
                for (int point : value) {
                    nonAscii |= point >= 0x80;
                }
                if (nonAscii) {
                    if (value.length > 63 - 4) {
                        throw new Invalid("label too long [A4_2]");
                    }
                    text = "xn--" + encode(value);
                }
            }
            if (text.isEmpty()) {
                throw new Invalid("empty label [A4_2]");
            }
            if (text.codePointCount(0, text.length()) > 63 && ascii) {
                throw new Invalid("label too long [A4_2]");
            }
            out.add(text);
        }
        String domain = String.join(".", out);
        if (domain.isEmpty()) {
            throw new Invalid("empty domain name [A4_1]");
        }
        if (domain.codePointCount(0, domain.length()) > 253 && ascii) {
            throw new Invalid("domain name too long [A4_1]");
        }
        return rooted ? domain + "." : domain;
    }

    private static boolean scalar(String text) {
        for (int index = 0; index < text.length(); index++) {
            char unit = text.charAt(index);
            if (Character.isHighSurrogate(unit) && index + 1 < text.length()
                    && Character.isLowSurrogate(text.charAt(index + 1))) {
                index++;
            } else if (Character.isSurrogate(unit)) {
                return false;
            }
        }
        return true;
    }

    private static final java.util.regex.Pattern LABEL =
        java.util.regex.Pattern.compile("\\A[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\\z");

    /** {@code _ascii} Perl-provider: UTS #46 и проверки DNS и A-меток. */
    static String hostnameAscii(String source) {
        int size = source.codePointCount(0, source.length());
        if (size == 0 || size > 1024 || !scalar(source)) {
            throw new Invalid("invalid source");
        }
        String ascii = process(source, true);
        String unrooted = ascii.endsWith(".") ? ascii.substring(0, ascii.length() - 1) : ascii;
        if (unrooted.isEmpty() || unrooted.length() > 253) {
            throw new Invalid("invalid DNS width");
        }
        String[] labels = unrooted.split("\\.", -1);
        for (String label : labels) {
            if (label.length() > 63 || !LABEL.matcher(label).find()) {
                throw new Invalid("invalid DNS label");
            }
        }
        String raw = source.replace('。', '.').replace('．', '.').replace('｡', '.');
        ArrayList<String> supplied = new ArrayList<String>();
        for (String part : raw.split("\\.", -1)) {
            supplied.add(part);
        }
        if (!supplied.isEmpty() && supplied.get(supplied.size() - 1).isEmpty()) {
            supplied.remove(supplied.size() - 1);
        }
        for (int at = 0; at < supplied.size(); at++) {
            String label = Normalizer.normalize(supplied.get(at), Normalizer.Form.NFKC).toLowerCase(Locale.ROOT);
            if (label.startsWith("xn--") && (at >= labels.length || !label.equals(labels[at]))) {
                throw new Invalid("noncanonical A-label");
            }
        }
        return ascii;
    }

    private static String idna(String source, boolean unicode, String name) {
        try {
            String ascii = hostnameAscii(source);
            return unicode ? process(ascii, false) : ascii;
        } catch (Invalid invalid) {
            throw NativeFailure.raise("core.idna", "invalid_input", "runtime/idna@1::" + name + "/1",
                "invalid bounded IDNA hostname");
        }
    }

    public static String runtime_idna__hostname_ascii(String origin) {
        return idna(origin, false, "hostname_ascii");
    }

    public static String runtime_idna__hostname_unicode(String origin) {
        return idna(origin, true, "hostname_unicode");
    }

    // ---- runtime/public_suffix ----

    private static String pslVersion;
    private static HashSet<String> exact;
    private static HashSet<String> wildcard;
    private static HashSet<String> exception;

    private static synchronized void psl() {
        if (exact != null) {
            return;
        }
        InputStream stream = IdnaNatives.class.getResourceAsStream("psl.dat");
        if (stream == null) {
            throw Rt.trap("Public suffix data of the runtime is missing");
        }
        HashSet<String> exactRules = new HashSet<String>();
        HashSet<String> wildcardRules = new HashSet<String>();
        HashSet<String> exceptionRules = new HashSet<String>();
        try (BufferedReader reader = new BufferedReader(
                new InputStreamReader(new GZIPInputStream(stream), StandardCharsets.UTF_8))) {
            pslVersion = reader.readLine();
            String line;
            while ((line = reader.readLine()) != null) {
                if (line.isEmpty()) {
                    continue;
                }
                if (line.startsWith("!")) {
                    exceptionRules.add(line.substring(1));
                } else if (line.startsWith("*.")) {
                    wildcardRules.add(line.substring(2));
                } else {
                    exactRules.add(line);
                }
            }
        } catch (IOException failure) {
            throw Rt.trap("Cannot read public suffix data of the runtime");
        }
        exception = exceptionRules;
        wildcard = wildcardRules;
        exact = exactRules;
    }

    /** Метки домена и ширина публичного суффикса. */
    static Object[] parts(String source) {
        psl();
        String ascii = hostnameAscii(source);
        if (ascii.endsWith(".")) {
            ascii = ascii.substring(0, ascii.length() - 1);
        }
        ArrayList<String> labels = new ArrayList<String>();
        for (String label : ascii.split("\\.")) {
            labels.add(label);
        }
        int best = 1;
        for (int at = 0; at < labels.size(); at++) {
            String key = String.join(".", labels.subList(at, labels.size()));
            int width = labels.size() - at;
            if (exception.contains(key)) {
                return new Object[] {labels, width - 1};
            }
            if (exact.contains(key) && width > best) {
                best = width;
            }
            if (at > 0 && wildcard.contains(key) && width + 1 > best) {
                best = width + 1;
            }
        }
        return new Object[] {labels, best};
    }

    @SuppressWarnings("unchecked")
    private static String suffix(String source, boolean registrable, String name) {
        Object[] found;
        try {
            found = parts(source);
        } catch (Invalid invalid) {
            throw NativeFailure.raise("core.public_suffix", "invalid_input", "runtime/public_suffix@1::" + name + "/1",
                "invalid bounded IDNA hostname");
        }
        List<String> labels = (List<String>) found[0];
        int width = (Integer) found[1];
        if (registrable) {
            if (labels.size() <= width) {
                throw NativeFailure.raise("core.public_suffix", "not_found", "runtime/public_suffix@1::" + name + "/1",
                    "public suffix has no registrable domain");
            }
            width++;
        }
        return String.join(".", labels.subList(Math.max(0, labels.size() - width), labels.size()));
    }

    public static String runtime_public_suffix__public_suffix_name(String source) {
        return suffix(source, false, "public_suffix_name");
    }

    public static String runtime_public_suffix__registrable_name(String source) {
        return suffix(source, true, "registrable_name");
    }

    public static String runtime_public_suffix__public_suffix_version() {
        psl();
        return pslVersion;
    }
}
