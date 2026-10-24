package okkama.rt;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

/**
 * Natives {@code runtime/time_format} (шаблоны {@code %Y-%m-%d …}) и
 * {@code runtime/time_zone} (bundled tzdb 2026c, ресурс {@code tzdb.dat}):
 * как у Perl-provider, включая правило POSIX после последнего перехода.
 */
public final class TimeNatives {
    private TimeNatives() {}

    // ===================== runtime/time_format =====================

    private static int width(int directive) {
        switch (directive) {
            case 'Y': return 4;
            case 'm': case 'd': case 'H': case 'M': case 'S': return 2;
            case 'f': return 9;
            default: return -1;
        }
    }

    /** Токены: {kind 0 — литерал, 1 — поле; значение; смещение}; null и смещение ошибки. */
    private static Object[] tokens(String pattern) {
        int[] text = pattern.codePoints().toArray();
        ArrayList<Object[]> out = new ArrayList<Object[]>();
        int index = 0;
        while (index < text.length) {
            if (text[index] != '%') {
                int start = index;
                while (index < text.length && text[index] != '%') {
                    index++;
                }
                out.add(new Object[] {0, new String(text, start, index - start), start});
                continue;
            }
            if (index + 1 >= text.length) {
                return new Object[] {null, index};
            }
            int directive = text[index + 1];
            if (directive == '%') {
                out.add(new Object[] {0, "%", index});
            } else if (width(directive) > 0) {
                out.add(new Object[] {1, String.valueOf((char) directive), index});
            } else {
                return new Object[] {null, index};
            }
            index += 2;
        }
        return new Object[] {out, -1};
    }

    @SuppressWarnings("unchecked")
    public static BigInteger runtime_time_format__time_pattern_invalid_offset(String pattern, boolean local,
            boolean parsing) {
        Object[] parsed = tokens(pattern);
        if (parsed[0] == null) {
            return BigInteger.valueOf((Integer) parsed[1]);
        }
        java.util.HashSet<String> seen = new java.util.HashSet<String>();
        for (Object[] token : (List<Object[]>) parsed[0]) {
            if ((Integer) token[0] != 1) {
                continue;
            }
            String field = (String) token[1];
            if (!local && "HMSf".contains(field)) {
                return BigInteger.valueOf((Integer) token[2]);
            }
            seen.add(field);
        }
        if (parsing) {
            String[] required = local ? new String[] {"Y", "m", "d", "H", "M", "S"} : new String[] {"Y", "m", "d"};
            for (String field : required) {
                if (!seen.contains(field)) {
                    return BigInteger.valueOf(pattern.codePointCount(0, pattern.length()));
                }
            }
        }
        return BigInteger.valueOf(-1);
    }

    @SuppressWarnings("unchecked")
    private static String format(String pattern, Map<String, BigInteger> values) {
        StringBuilder out = new StringBuilder();
        for (Object[] token : (List<Object[]>) tokens(pattern)[0]) {
            if ((Integer) token[0] == 0) {
                out.append((String) token[1]);
                continue;
            }
            String field = (String) token[1];
            BigInteger value = values.get(field);
            String digits = value.abs().toString();
            int size = width(field.charAt(0)) - (value.signum() < 0 ? 1 : 0);
            StringBuilder padded = new StringBuilder();
            for (int index = digits.length(); index < size; index++) {
                padded.append('0');
            }
            out.append(value.signum() < 0 ? "-" : "").append(padded).append(digits);
        }
        return out.toString();
    }

    public static String runtime_time_format__time_format_date(BigInteger year, BigInteger month, BigInteger day,
            String pattern) {
        Map<String, BigInteger> values = new HashMap<String, BigInteger>();
        values.put("Y", year);
        values.put("m", month);
        values.put("d", day);
        return format(pattern, values);
    }

    public static String runtime_time_format__time_format_local(BigInteger year, BigInteger month, BigInteger day,
            BigInteger hour, BigInteger minute, BigInteger second, BigInteger nano, String pattern) {
        Map<String, BigInteger> values = new HashMap<String, BigInteger>();
        values.put("Y", year);
        values.put("m", month);
        values.put("d", day);
        values.put("H", hour);
        values.put("M", minute);
        values.put("S", second);
        values.put("f", nano);
        return format(pattern, values);
    }

    /** Значения полей и смещение ошибки (-1 — разбор успешен). */
    @SuppressWarnings("unchecked")
    private static Object[] parse(String originText, String pattern) {
        int[] origin = originText.codePoints().toArray();
        int offset = 0;
        Map<String, BigInteger> values = new HashMap<String, BigInteger>();
        for (Object[] token : (List<Object[]>) tokens(pattern)[0]) {
            if ((Integer) token[0] == 0) {
                int[] literal = ((String) token[1]).codePoints().toArray();
                for (int point : literal) {
                    if (offset >= origin.length || origin[offset] != point) {
                        return new Object[] {null, offset};
                    }
                    offset++;
                }
                continue;
            }
            String field = (String) token[1];
            int size = width(field.charAt(0));
            if (offset + size > origin.length) {
                return new Object[] {null, offset};
            }
            String digits = new String(origin, offset, size);
            if (!digits.matches("[0-9]+")) {
                return new Object[] {null, offset};
            }
            BigInteger value = new BigInteger(digits);
            if (values.containsKey(field) && !values.get(field).equals(value)) {
                return new Object[] {null, offset};
            }
            values.put(field, value);
            offset += size;
        }
        if (offset != origin.length) {
            return new Object[] {null, offset};
        }
        if (!values.containsKey("f")) {
            values.put("f", BigInteger.ZERO);
        }
        return new Object[] {values, -1};
    }

    public static BigInteger runtime_time_format__time_parse_invalid_offset(String origin, String pattern,
            boolean local) {
        return BigInteger.valueOf((Integer) parse(origin, pattern)[1]);
    }

    @SuppressWarnings("unchecked")
    public static List<Object> runtime_time_format__time_parse_parts(String origin, String pattern, boolean local) {
        Map<String, BigInteger> values = (Map<String, BigInteger>) parse(origin, pattern)[0];
        if (values == null) {
            throw Natives.died();
        }
        String[] fields = local ? new String[] {"Y", "m", "d", "H", "M", "S", "f"} : new String[] {"Y", "m", "d"};
        ArrayList<Object> out = new ArrayList<Object>();
        for (String field : fields) {
            BigInteger value = values.get(field);
            if (value == null) {
                throw Natives.died();
            }
            out.add(value);
        }
        return out;
    }

    // ===================== runtime/time_zone =====================

    private static final String VERSION = "2026c";
    private static Map<String, byte[]> files;

    private static synchronized Map<String, byte[]> files() {
        if (files != null) {
            return files;
        }
        Map<String, byte[]> loaded = new HashMap<String, byte[]>();
        InputStream stream = TimeNatives.class.getResourceAsStream("tzdb.dat");
        if (stream == null) {
            throw Rt.trap("Bundled timezone database is unavailable");
        }
        try (ZipInputStream zip = new ZipInputStream(stream)) {
            byte[] buffer = new byte[8192];
            for (ZipEntry entry = zip.getNextEntry(); entry != null; entry = zip.getNextEntry()) {
                if (entry.isDirectory()) {
                    continue;
                }
                ByteArrayOutputStream out = new ByteArrayOutputStream();
                for (int count = zip.read(buffer); count > 0; count = zip.read(buffer)) {
                    out.write(buffer, 0, count);
                }
                String name = entry.getName().startsWith("./") ? entry.getName().substring(2) : entry.getName();
                loaded.put(name, out.toByteArray());
            }
        } catch (IOException failure) {
            throw Rt.trap("Cannot read bundled time zone");
        }
        files = loaded;
        return loaded;
    }

    private static final Pattern ZONE_NAME = Pattern.compile("\\A[A-Za-z0-9_+.-]+(?:/[A-Za-z0-9_+.-]+)*\\z");
    private static final Pattern DOT_DOT = Pattern.compile("(?:\\A|/)\\.\\.(?:/|\\z)");

    private static byte[] zoneFile(String name) {
        if (!ZONE_NAME.matcher(name).find() || DOT_DOT.matcher(name).find()) {
            return null;
        }
        return files().get(name);
    }

    static final class Type {
        long offset;
        boolean daylight;
        String abbreviation;
    }

    static final class Rule {
        int month;
        int week;
        int weekday;
        long seconds;
    }

    static final class Footer {
        String standardName;
        long standardOffset;
        String daylightName;
        long daylightOffset;
        Rule start;
        Rule end;
    }

    static final class Zone {
        long[] transitions;
        int[] indices;
        Type[] types;
        Footer footer;
    }

    private static final Map<String, Zone> CACHE = new LinkedHashMap<String, Zone>();

    private static long[] header(ByteBuffer data, int offset) {
        if (data.get(offset) != 'T' || data.get(offset + 1) != 'Z' || data.get(offset + 2) != 'i'
                || data.get(offset + 3) != 'f') {
            throw Rt.trap("Invalid bundled TZif header");
        }
        long[] counts = new long[7];
        counts[0] = data.get(offset + 4);
        for (int index = 0; index < 6; index++) {
            counts[index + 1] = data.getInt(offset + 20 + index * 4) & 0xffffffffL;
        }
        return counts;
    }

    private static long blockSize(int timeSize, long[] counts) {
        long gmt = counts[1];
        long std = counts[2];
        long leap = counts[3];
        long time = counts[4];
        long type = counts[5];
        long chars = counts[6];
        return time * timeSize + time + type * 6 + chars + leap * (timeSize + 4) + std + gmt;
    }

    private static Zone zone(String name) {
        Zone cached = CACHE.get(name);
        if (cached != null) {
            return cached;
        }
        byte[] bytes = zoneFile(name);
        if (bytes == null) {
            throw Natives.died();
        }
        ByteBuffer data = ByteBuffer.wrap(bytes);
        long[] counts = header(data, 0);
        int offset = 44;
        int timeSize = 4;
        if (counts[0] != 0) {
            offset += (int) blockSize(4, counts);
            counts = header(data, offset);
            offset += 44;
            timeSize = 8;
        }
        int timeCount = (int) counts[4];
        int typeCount = (int) counts[5];
        int charCount = (int) counts[6];
        Zone zone = new Zone();
        zone.transitions = new long[timeCount];
        for (int index = 0; index < timeCount; index++) {
            zone.transitions[index] = timeSize == 8 ? data.getLong(offset) : data.getInt(offset);
            offset += timeSize;
        }
        zone.indices = new int[timeCount];
        for (int index = 0; index < timeCount; index++) {
            zone.indices[index] = data.get(offset + index) & 0xff;
        }
        offset += timeCount;
        long[][] raw = new long[typeCount][];
        for (int index = 0; index < typeCount; index++) {
            raw[index] = new long[] {data.getInt(offset), data.get(offset + 4) & 0xff, data.get(offset + 5) & 0xff};
            offset += 6;
        }
        String abbreviations = new String(bytes, offset, charCount, StandardCharsets.ISO_8859_1);
        offset += charCount;
        zone.types = new Type[typeCount];
        for (int index = 0; index < typeCount; index++) {
            Type type = new Type();
            type.offset = raw[index][0];
            type.daylight = raw[index][1] != 0;
            int start = (int) raw[index][2];
            int end = abbreviations.indexOf('\0', start);
            type.abbreviation = abbreviations.substring(start, end < 0 ? abbreviations.length() : end);
            zone.types[index] = type;
        }
        offset += (int) (counts[3] * (timeSize + 4) + counts[2] + counts[1]);
        if (timeSize == 8 && offset < bytes.length) {
            String remaining = new String(bytes, offset, bytes.length - offset, StandardCharsets.ISO_8859_1);
            Matcher footer = Pattern.compile("\\A\\n([^\\n]*)\\n").matcher(remaining);
            if (footer.find()) {
                zone.footer = parsePosix(footer.group(1));
            }
        }
        CACHE.put(name, zone);
        return zone;
    }

    private static long clockSeconds(String text) {
        long sign = 1;
        if (text.startsWith("+") || text.startsWith("-")) {
            sign = text.charAt(0) == '-' ? -1 : 1;
            text = text.substring(1);
        }
        String[] parts = text.split(":", -1);
        if (parts.length < 1 || parts.length > 3) {
            throw Rt.trap("Invalid bundled POSIX clock value");
        }
        for (String part : parts) {
            if (!part.matches("[0-9]+")) {
                throw Rt.trap("Invalid bundled POSIX clock value");
            }
        }
        return sign * (Long.parseLong(parts[0]) * 3600 + (parts.length > 1 ? Long.parseLong(parts[1]) : 0) * 60
            + (parts.length > 2 ? Long.parseLong(parts[2]) : 0));
    }

    private static Footer parsePosix(String source) {
        if (source == null || source.isEmpty()) {
            return null;
        }
        String[] remaining = {source};
        Footer footer = new Footer();
        footer.standardName = takeName(remaining);
        footer.standardOffset = takeOffset(remaining);
        if (remaining[0].isEmpty()) {
            return footer;
        }
        footer.daylightName = takeName(remaining);
        footer.daylightOffset = footer.standardOffset + 3600;
        if (!remaining[0].startsWith(",")) {
            footer.daylightOffset = takeOffset(remaining);
        }
        if (!remaining[0].startsWith(",")) {
            throw Rt.trap("Invalid bundled POSIX transition list");
        }
        String[] rules = remaining[0].substring(1).split(",", 2);
        if (rules.length < 2) {
            throw Rt.trap("Invalid bundled POSIX transition list");
        }
        footer.start = parseRule(rules[0]);
        footer.end = parseRule(rules[1]);
        return footer;
    }

    private static String takeName(String[] text) {
        Matcher quoted = Pattern.compile("\\A<([^>]+)>").matcher(text[0]);
        if (quoted.find()) {
            text[0] = text[0].substring(quoted.end());
            return quoted.group(1);
        }
        Matcher plain = Pattern.compile("\\A([A-Za-z]{3,})").matcher(text[0]);
        if (plain.find()) {
            text[0] = text[0].substring(plain.end());
            return plain.group(1);
        }
        throw Rt.trap("Invalid bundled POSIX zone name");
    }

    private static long takeOffset(String[] text) {
        Matcher offset = Pattern.compile("\\A([+-]?[0-9]+(?::[0-9]+(?::[0-9]+)?)?)").matcher(text[0]);
        if (!offset.find()) {
            throw Rt.trap("Invalid bundled POSIX offset");
        }
        text[0] = text[0].substring(offset.end());
        return -clockSeconds(offset.group(1));
    }

    private static Rule parseRule(String text) {
        String[] parts = text.split("/", 2);
        Matcher date = Pattern.compile("\\AM([0-9]+)\\.([0-9]+)\\.([0-9]+)\\z").matcher(parts[0]);
        if (!date.find()) {
            throw Rt.trap("Unsupported bundled POSIX transition rule");
        }
        Rule rule = new Rule();
        rule.month = Integer.parseInt(date.group(1));
        rule.week = Integer.parseInt(date.group(2));
        rule.weekday = Integer.parseInt(date.group(3));
        rule.seconds = parts.length > 1 ? clockSeconds(parts[1]) : 7200;
        return rule;
    }

    private static boolean leap(long year) {
        return year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    }

    private static long monthDays(long year, long month) {
        if (month == 2) {
            return leap(year) ? 29 : 28;
        }
        return month == 4 || month == 6 || month == 9 || month == 11 ? 30 : 31;
    }

    private static long ordinal(long year, long month, long day) {
        long previous = year - 1;
        long before = (367 * month - 362) / 12;
        if (month > 2) {
            before += leap(year) ? -1 : -2;
        }
        return previous * 365 + previous / 4 - previous / 100 + previous / 400 + before + day;
    }

    private static long ruleDay(long year, Rule rule) {
        long firstMonday = Math.floorMod(ordinal(year, rule.month, 1) - 1, 7L);
        long firstSundayNumber = (firstMonday + 1) % 7;
        long day = 1 + Math.floorMod(rule.weekday - firstSundayNumber + 7, 7L) + (rule.week - 1) * 7L;
        if (day > monthDays(year, rule.month)) {
            day -= 7;
        }
        return day;
    }

    private static long transition(long year, Rule rule, long offsetBefore) {
        long day = ruleDay(year, rule);
        return (ordinal(year, rule.month, day) - 719163) * 86400 + rule.seconds - offsetBefore;
    }

    private static long yearFromSeconds(long seconds) {
        long days = Math.floorDiv(seconds, 86400L);
        long target = days + 719163;
        long low = 1;
        long high = 9999;
        while (low < high) {
            long middle = (low + high + 1) / 2;
            if (ordinal(middle, 1, 1) <= target) {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        return low;
    }

    private static Type observation(String name, long seconds) {
        Zone zone = zone(name);
        long[] transitions = zone.transitions;
        if (transitions.length > 0 && seconds > transitions[transitions.length - 1] && zone.footer != null) {
            Footer footer = zone.footer;
            Type type = new Type();
            if (footer.daylightName == null) {
                type.offset = footer.standardOffset;
                type.abbreviation = footer.standardName;
                return type;
            }
            long year = yearFromSeconds(seconds);
            long start = transition(year, footer.start, footer.standardOffset);
            long end = transition(year, footer.end, footer.daylightOffset);
            boolean daylight = start < end ? seconds >= start && seconds < end : seconds >= start || seconds < end;
            type.offset = daylight ? footer.daylightOffset : footer.standardOffset;
            type.daylight = daylight;
            type.abbreviation = daylight ? footer.daylightName : footer.standardName;
            return type;
        }
        int low = 0;
        int high = transitions.length - 1;
        int found = -1;
        while (low <= high) {
            int middle = (low + high) / 2;
            if (transitions[middle] <= seconds) {
                found = middle;
                low = middle + 1;
            } else {
                high = middle - 1;
            }
        }
        int index = found >= 0 ? zone.indices[found] : 0;
        return zone.types[index];
    }

    private static long floorSeconds(Rat instant) {
        BigInteger numerator = new BigInteger(Rat.numerator(instant));
        BigInteger denominator = new BigInteger(Rat.denominator(instant));
        BigInteger[] division = numerator.divideAndRemainder(denominator);
        BigInteger quotient = division[0];
        if (division[1].signum() != 0 && numerator.signum() * denominator.signum() < 0) {
            quotient = quotient.subtract(BigInteger.ONE);
        }
        return quotient.longValue();
    }

    public static String runtime_time_zone__timezone_version() {
        return VERSION;
    }

    public static boolean runtime_time_zone__timezone_known(String name) {
        return zoneFile(name) != null;
    }

    public static BigInteger runtime_time_zone__timezone_offset(String name, Rat instant) {
        return BigInteger.valueOf(observation(name, floorSeconds(instant)).offset);
    }

    public static boolean runtime_time_zone__timezone_daylight(String name, Rat instant) {
        return observation(name, floorSeconds(instant)).daylight;
    }

    public static String runtime_time_zone__timezone_abbreviation(String name, Rat instant) {
        return observation(name, floorSeconds(instant)).abbreviation;
    }

    private static List<Long> candidates(String name, BigInteger year, BigInteger month, BigInteger day,
            BigInteger hour, BigInteger minute, BigInteger second) {
        long local = (ordinal(year.longValue(), month.longValue(), day.longValue()) - 719163) * 86400
            + hour.longValue() * 3600 + minute.longValue() * 60 + second.longValue();
        Zone zone = zone(name);
        TreeSet<Long> offsets = new TreeSet<Long>();
        for (Type type : zone.types) {
            offsets.add(type.offset);
        }
        if (zone.footer != null) {
            offsets.add(zone.footer.standardOffset);
            if (zone.footer.daylightName != null) {
                offsets.add(zone.footer.daylightOffset);
            }
        }
        ArrayList<Long> out = new ArrayList<Long>();
        for (long offset : offsets) {
            long instant = local - offset;
            if (observation(name, instant).offset == offset) {
                out.add(instant);
            }
        }
        Collections.sort(out);
        return out;
    }

    public static BigInteger runtime_time_zone__timezone_local_candidates(String name, BigInteger year,
            BigInteger month, BigInteger day, BigInteger hour, BigInteger minute, BigInteger second, BigInteger nano) {
        return BigInteger.valueOf(candidates(name, year, month, day, hour, minute, second).size());
    }

    public static Rat runtime_time_zone__timezone_local_instant(String name, BigInteger year, BigInteger month,
            BigInteger day, BigInteger hour, BigInteger minute, BigInteger second, BigInteger nano, boolean later) {
        List<Long> found = candidates(name, year, month, day, hour, minute, second);
        if (found.isEmpty()) {
            throw Natives.died();
        }
        long seconds = later ? found.get(found.size() - 1) : found.get(0);
        BigInteger billion = BigInteger.valueOf(1_000_000_000L);
        return Rat.make(BigInteger.valueOf(seconds).multiply(billion).add(nano), billion);
    }
}
