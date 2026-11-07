package okkama.rt;

import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

/**
 * {@code runtime/postgres}: the DataRow messages of one result portion (text
 * format) become one result page ({@code db/read.DbPage}) in one call, as on the
 * Sparrow core. Cell kinds: 0 null, 1 boolean, 2 integer, 3 floating, 4 decimal
 * (text), 5 text, 6 bytes; a malformed portion gives a page with a failure.
 */
public final class PostgresNatives {
    private PostgresNatives() {}

    static final class Page {
        String failure = "";
        long rows;
        final List<Object> kinds = new ArrayList<Object>();
        final List<Object> values = new ArrayList<Object>();
        final List<Object> floats = new ArrayList<Object>();
        final List<Object> texts = new ArrayList<Object>();
        final List<Object> blobs = new ArrayList<Object>();

        void cell(long kind, long value) {
            kinds.add(BigInteger.valueOf(kind));
            values.add(BigInteger.valueOf(value));
        }
    }

    private static final class Malformed extends Exception {
        Malformed(String reason) { super(reason, null, false, false); }
    }

    private static int int32(byte[] raw, int at) throws Malformed {
        if (at + 4 > raw.length) throw new Malformed("truncated");
        return ((raw[at] & 0xff) << 24) | ((raw[at + 1] & 0xff) << 16) | ((raw[at + 2] & 0xff) << 8) | (raw[at + 3] & 0xff);
    }

    private static boolean nonfinite(String text) {
        return text.equals("NaN") || text.equals("Infinity") || text.equals("-Infinity");
    }

    private static void value(Page page, byte[] raw, int from, int to, long oid) throws Malformed {
        String text;
        try {
            text = StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(raw, from, to - from)).toString();
        } catch (CharacterCodingException e) {
            throw new Malformed("utf8");
        }
        if (oid == 16) {
            page.cell(1, text.equals("t") ? 1 : 0);
        } else if (oid == 20 || oid == 21 || oid == 23 || oid == 26) {
            try {
                page.cell(2, Long.parseLong(text));
            } catch (NumberFormatException e) {
                throw new Malformed("integer");
            }
        } else if (oid == 700 || oid == 701) {
            if (nonfinite(text)) throw new Malformed("nonfinite");
            double value;
            try {
                value = Double.parseDouble(text);
            } catch (NumberFormatException e) {
                throw new Malformed("floating");
            }
            page.cell(3, page.floats.size());
            page.floats.add(Double.valueOf(value));
        } else if (oid == 1700) {
            if (nonfinite(text)) throw new Malformed("nonfinite");
            page.cell(4, page.texts.size());
            page.texts.add(text);
        } else if (oid == 17) {
            if (!text.startsWith("\\x") || text.length() % 2 != 0) throw new Malformed("bytea");
            byte[] blob = new byte[(text.length() - 2) / 2];
            for (int i = 0; i < blob.length; i++) {
                int high = Character.digit(text.charAt(2 + 2 * i), 16);
                int low = Character.digit(text.charAt(3 + 2 * i), 16);
                if (high < 0 || low < 0) throw new Malformed("bytea");
                blob[i] = (byte) (high * 16 + low);
            }
            page.cell(6, page.blobs.size());
            page.blobs.add(Natives.fromBytes(blob, blob.length));
        } else {
            page.cell(5, page.texts.size());
            page.texts.add(text);
        }
    }

    private static void fill(Page page, byte[] raw, int width, long[] oids) throws Malformed {
        int at = 0;
        while (at < raw.length) {
            if (raw[at] != 'D') throw new Malformed("message");
            int length = int32(raw, at + 1);
            if (length < 4) throw new Malformed("truncated");
            long end = (long) at + 1 + length;
            if (end > raw.length || at + 7 > raw.length) throw new Malformed("truncated");
            int count = (short) (((raw[at + 5] & 0xff) << 8) | (raw[at + 6] & 0xff));
            if (count != width) throw new Malformed("width");
            int cursor = at + 7;
            for (int column = 0; column < width; column++) {
                int size = int32(raw, cursor);
                cursor += 4;
                if (size < 0) {
                    page.cell(0, 0);
                    continue;
                }
                long stop = (long) cursor + size;
                if (stop > end) throw new Malformed("truncated");
                value(page, raw, cursor, (int) stop, oids[column]);
                cursor = (int) stop;
            }
            if (cursor != end) throw new Malformed("truncated");
            page.rows++;
            at = (int) end;
        }
    }

    public static Object runtime_postgres__pg_page(List<?> rows, BigInteger width, List<?> types) {
        if (width.signum() < 0 || width.bitLength() > 31 || types.size() != width.intValue())
            throw Rt.trap("Native invocation \"runtime/postgres@1::pg_page/3\" failed");
        long[] oids = new long[types.size()];
        for (int i = 0; i < oids.length; i++) oids[i] = ((BigInteger) types.get(i)).longValue();
        Page page = new Page();
        try {
            fill(page, Natives.toBytes(rows), width.intValue(), oids);
        } catch (Malformed malformed) {
            page = new Page();
            page.failure = malformed.getMessage();
        }
        return page;
    }

    public static String runtime_postgres__pg_page_failure(Object page) { return ((Page) page).failure; }
    public static BigInteger runtime_postgres__pg_page_rows(Object page) { return BigInteger.valueOf(((Page) page).rows); }
    public static List<Object> runtime_postgres__pg_page_kinds(Object page) { return new ArrayList<Object>(((Page) page).kinds); }
    public static List<Object> runtime_postgres__pg_page_values(Object page) { return new ArrayList<Object>(((Page) page).values); }
    public static List<Object> runtime_postgres__pg_page_floats(Object page) { return new ArrayList<Object>(((Page) page).floats); }
    public static List<Object> runtime_postgres__pg_page_texts(Object page) { return new ArrayList<Object>(((Page) page).texts); }
    public static List<Object> runtime_postgres__pg_page_blobs(Object page) { return new ArrayList<Object>(((Page) page).blobs); }
}
