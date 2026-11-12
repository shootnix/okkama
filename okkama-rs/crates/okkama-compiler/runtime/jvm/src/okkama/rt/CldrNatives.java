package okkama.rt;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.zip.GZIPInputStream;

/**
 * {@code runtime/cldr}: таблицы CLDR как данные рантайма (#223), те же
 * {@code cldr-<семейство>.dat}, что у Rust-рантаймов. Файл читается при
 * первом обращении к любой его таблице. Строка файла — таблица, ключ и поля
 * через табуляцию; имя таблицы начинается с семейства и точки.
 */
public final class CldrNatives {
    private CldrNatives() {}

    private static final String SYMBOL = "runtime/cldr@1::cldr_fields/2";
    private static final List<String> FAMILIES =
        List.of("numbers", "dates", "zones", "currencies", "locales");
    /** Семейство → таблица → ключ → поля строки (текст после ключа). */
    private static final Map<String, Map<String, Map<String, String>>> LOADED =
        new HashMap<String, Map<String, Map<String, String>>>();

    public static List<Object> runtime_cldr__cldr_fields(String table, String key) {
        int dot = table.indexOf('.');
        Map<String, Map<String, String>> family = dot < 0 ? null : family(table.substring(0, dot));
        Map<String, String> rows = family == null ? null : family.get(table);
        if (rows == null) {
            throw NativeFailure.raise("core.cldr", "invalid_input", SYMBOL,
                "Unknown CLDR table \"" + table + "\"");
        }
        ArrayList<Object> fields = new ArrayList<Object>();
        String row = rows.get(key);
        if (row != null) {
            int start = 0;
            while (true) {
                int tab = row.indexOf('\t', start);
                if (tab < 0) {
                    fields.add(row.substring(start));
                    break;
                }
                fields.add(row.substring(start, tab));
                start = tab + 1;
            }
        }
        return fields;
    }

    private static synchronized Map<String, Map<String, String>> family(String name) {
        Map<String, Map<String, String>> loaded = LOADED.get(name);
        if (loaded != null || !FAMILIES.contains(name)) {
            return loaded;
        }
        InputStream stream = CldrNatives.class.getResourceAsStream("cldr-" + name + ".dat");
        if (stream == null) {
            throw Rt.trap("CLDR data of the runtime is missing: " + name);
        }
        Map<String, Map<String, String>> tables = new HashMap<String, Map<String, String>>();
        try (BufferedReader reader = new BufferedReader(
                new InputStreamReader(new GZIPInputStream(stream), StandardCharsets.UTF_8))) {
            String line;
            while ((line = reader.readLine()) != null) {
                int first = line.indexOf('\t');
                int second = line.indexOf('\t', first + 1);
                String table = line.substring(0, first);
                Map<String, String> rows = tables.get(table);
                if (rows == null) {
                    rows = new HashMap<String, String>();
                    tables.put(table, rows);
                }
                rows.put(line.substring(first + 1, second), line.substring(second + 1));
            }
        } catch (IOException error) {
            throw Rt.trap("CLDR data of the runtime is unreadable: " + name);
        }
        LOADED.put(name, tables);
        return tables;
    }
}
