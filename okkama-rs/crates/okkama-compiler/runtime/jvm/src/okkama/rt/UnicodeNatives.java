package okkama.rt;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.text.BreakIterator;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.zip.GZIPInputStream;

/**
 * Natives {@code runtime/unicode_character}: свойства символов по таблицам
 * Unicode::UCD Perl (ресурс {@code unicode.dat}, см.
 * {@code tools/jvm-runtime/gen-unicode.pl}) и ширина графем для терминала.
 */
public final class UnicodeNatives {
    private UnicodeNatives() {}

    /** Диапазоны [from, to] по возрастанию и значение каждого. */
    static final class Ranges {
        final ArrayList<int[]> items = new ArrayList<int[]>();
        final ArrayList<String> values = new ArrayList<String>();

        void add(int from, int to, String value) {
            items.add(new int[] {from, to});
            values.add(value);
        }

        String find(int point) {
            int low = 0;
            int high = items.size() - 1;
            while (low <= high) {
                int middle = (low + high) >>> 1;
                int[] range = items.get(middle);
                if (point < range[0]) {
                    high = middle - 1;
                } else if (point > range[1]) {
                    low = middle + 1;
                } else {
                    return values.get(middle);
                }
            }
            return null;
        }
    }

    private static final class Data {
        final Map<String, Ranges> properties = new HashMap<String, Ranges>();
        final Ranges category = new Ranges();
        final Ranges combining = new Ranges();
        final Ranges algorithmic = new Ranges();
        final Ranges hangul = new Ranges();
        final Map<Integer, String> numeric = new HashMap<Integer, String>();
        final Map<Integer, String> names = new HashMap<Integer, String>();
    }

    private static Data data;

    private static Data data() {
        if (data != null) {
            return data;
        }
        Data loaded = new Data();
        InputStream stream = UnicodeNatives.class.getResourceAsStream("unicode.dat");
        if (stream == null) {
            throw Rt.trap("Unicode data of the runtime is missing");
        }
        try (BufferedReader reader = new BufferedReader(
                new InputStreamReader(new GZIPInputStream(stream), StandardCharsets.UTF_8))) {
            String line;
            while ((line = reader.readLine()) != null) {
                String[] parts = line.split(" ", 4);
                switch (parts[0]) {
                    case "R": {
                        Ranges ranges = loaded.properties.get(parts[1]);
                        if (ranges == null) {
                            ranges = new Ranges();
                            loaded.properties.put(parts[1], ranges);
                        }
                        ranges.add(Integer.parseInt(parts[2], 16), Integer.parseInt(parts[3], 16), "");
                        break;
                    }
                    case "G":
                        loaded.category.add(Integer.parseInt(parts[2], 16), Integer.parseInt(parts[3], 16), parts[1]);
                        break;
                    case "C":
                        loaded.combining.add(Integer.parseInt(parts[2], 16), Integer.parseInt(parts[3], 16), parts[1]);
                        break;
                    case "A":
                        loaded.algorithmic.add(Integer.parseInt(parts[2], 16), Integer.parseInt(parts[3], 16),
                            parts[1].replace('_', ' '));
                        break;
                    case "H":
                        loaded.hangul.add(Integer.parseInt(parts[1], 16), Integer.parseInt(parts[2], 16), "");
                        break;
                    case "N":
                        loaded.numeric.put(Integer.parseInt(parts[1], 16), parts[2]);
                        break;
                    case "M":
                        String[] name = line.split(" ", 3);
                        loaded.names.put(Integer.parseInt(name[1], 16), name[2]);
                        break;
                    default:
                        break;
                }
            }
        } catch (IOException failure) {
            throw Rt.trap("Cannot read Unicode data of the runtime: " + failure.getMessage());
        }
        // Диапазоны G/C/A/H в файле идут по возрастанию; свойства — тоже.
        data = loaded;
        return loaded;
    }

    static boolean has(String property, int point) {
        Ranges ranges = data().properties.get(property);
        return ranges != null && ranges.find(point) != null;
    }

    static String category(int point) {
        String category = data().category.find(point);
        return category == null ? "Cn" : category;
    }

    private static final String[] JAMO_L = {"G", "GG", "N", "D", "DD", "R", "M", "B", "BB", "S", "SS", "", "J", "JJ",
        "C", "K", "T", "P", "H"};
    private static final String[] JAMO_V = {"A", "AE", "YA", "YAE", "EO", "E", "YEO", "YE", "O", "WA", "WAE", "OE",
        "YO", "U", "WEO", "WE", "WI", "YU", "EU", "YI", "I"};
    private static final String[] JAMO_T = {"", "G", "GG", "GS", "N", "NJ", "NH", "D", "L", "LG", "LM", "LB", "LS",
        "LT", "LP", "LH", "M", "B", "BS", "S", "SS", "NG", "J", "C", "K", "T", "P", "H"};

    public static String runtime_unicode_character__character_name(int point) {
        Data table = data();
        String name = table.names.get(point);
        if (name != null) {
            return name;
        }
        String prefix = table.algorithmic.find(point);
        if (prefix != null) {
            return prefix + "-" + Integer.toHexString(point).toUpperCase();
        }
        if (table.hangul.find(point) != null) {
            int index = point - 0xAC00;
            return "HANGUL SYLLABLE " + JAMO_L[index / 588] + JAMO_V[(index % 588) / 28] + JAMO_T[index % 28];
        }
        return "";
    }

    public static String runtime_unicode_character__category_code(int point) {
        return category(point);
    }

    /** Canonical_Combining_Class (0, если не задан). */
    static int combiningClass(int point) {
        String value = data().combining.find(point);
        return value == null ? 0 : Integer.parseInt(value);
    }

    public static BigInteger runtime_unicode_character__character_combining_class(int point) {
        String value = data().combining.find(point);
        return value == null ? BigInteger.ZERO : new BigInteger(value);
    }

    private static Double numeric(int point) {
        String value = data().numeric.get(point);
        if (value == null) {
            return null;
        }
        Matcher fraction = Pattern.compile("\\A(-?[0-9]+)/([1-9][0-9]*)\\z").matcher(value);
        if (fraction.find()) {
            return Double.parseDouble(fraction.group(1)) / Double.parseDouble(fraction.group(2));
        }
        return Double.parseDouble(value);
    }

    public static boolean runtime_unicode_character__character_is_numeric(int point) {
        return numeric(point) != null;
    }

    public static Double runtime_unicode_character__character_numeric_value(int point) {
        Double value = numeric(point);
        return value == null ? 0.0 : value;
    }

    private static List<String> clusters(String text) {
        ArrayList<String> out = new ArrayList<String>();
        try {
            Matcher matcher = Pattern.compile("\\X").matcher(text);
            while (matcher.find()) {
                out.add(matcher.group());
            }
            return out;
        } catch (java.util.regex.PatternSyntaxException old) {
            BreakIterator iterator = BreakIterator.getCharacterInstance();
            iterator.setText(text);
            int start = iterator.first();
            for (int end = iterator.next(); end != BreakIterator.DONE; start = end, end = iterator.next()) {
                out.add(text.substring(start, end));
            }
            return out;
        }
    }

    private static int width(String cluster) {
        int[] points = cluster.codePoints().toArray();
        ArrayList<Integer> visible = new ArrayList<Integer>();
        for (int point : points) {
            if (!has("M", point) && !has("DI", point)) {
                visible.add(point);
            }
        }
        if (visible.isEmpty()) {
            return 0;
        }
        int regional = 0;
        boolean fe0f = false;
        boolean fe0e = false;
        boolean zwj = false;
        boolean emoji = false;
        boolean pictographic = false;
        boolean presentation = false;
        for (int point : points) {
            regional += has("RI", point) ? 1 : 0;
            fe0f |= point == 0xFE0F;
            fe0e |= point == 0xFE0E;
            zwj |= point == 0x200D;
            emoji |= has("E", point);
            pictographic |= has("EP", point);
            presentation |= has("EPres", point);
        }
        boolean keycap = cluster.matches("(?s).*[0-9#*]\\x{FE0F}?\\x{20E3}.*");
        if (regional >= 2 || keycap || (fe0f && emoji) || (zwj && pictographic) || (!fe0e && presentation)) {
            return 2;
        }
        for (int point : visible) {
            if (has("W", point) || has("F", point)) {
                return 2;
            }
        }
        return 1;
    }

    public static List<Object> runtime_unicode_character__display_clusters(String text) {
        return new ArrayList<Object>(clusters(text));
    }

    public static List<Object> runtime_unicode_character__display_widths(String text) {
        ArrayList<Object> out = new ArrayList<Object>();
        for (String cluster : clusters(text)) {
            out.add(BigInteger.valueOf(width(cluster)));
        }
        return out;
    }

    public static BigInteger runtime_unicode_character__display_invalid_index(String text) {
        int offset = 0;
        for (int point : text.codePoints().toArray()) {
            String category = category(point);
            if (category.equals("Cc") || category.equals("Zl") || category.equals("Zp")) {
                return BigInteger.valueOf(offset);
            }
            offset++;
        }
        return BigInteger.valueOf(-1);
    }
}
