package okkama.rt;

import java.io.ByteArrayInputStream;
import java.io.File;
import java.io.IOException;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.security.cert.CertPath;
import java.security.cert.CertPathBuilder;
import java.security.cert.CertPathBuilderException;
import java.security.cert.CertStore;
import java.security.cert.Certificate;
import java.security.cert.CertificateFactory;
import java.security.cert.CollectionCertStoreParameters;
import java.security.cert.PKIXBuilderParameters;
import java.security.cert.PKIXCertPathBuilderResult;
import java.security.cert.TrustAnchor;
import java.security.cert.X509CertSelector;
import java.security.cert.X509Certificate;
import java.time.LocalDateTime;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * Natives {@code runtime/x509}: порт Perl-provider. DER-конверт и снимок
 * фактов сертификата читаются здесь (X509Der); декодирование ключа и проверку
 * цепочки, которые Perl отдаёт OpenSSL, выполняет PKIX JDK.
 */
public final class X509Natives {
    private X509Natives() {}

    private static final String DOMAIN = "core.x509";

    /** Отказ разбора: {@code invalid}, {@code limit} или {@code unsupported}. */
    static final class Bad extends RuntimeException {
        private static final long serialVersionUID = 1L;
        final String kind;

        Bad(String kind, String message) {
            super(message, null, false, false);
            this.kind = kind;
        }
    }

    private static Bad bad(String reason) {
        return new Bad("invalid", reason);
    }

    private static Bad limit(String reason) {
        return new Bad("limit", reason);
    }

    private static String symbol(String name, int arity) {
        return "runtime/x509@1::" + name + "/" + arity;
    }

    private static Failure failure(String code, String operation, String message) {
        return NativeFailure.raise(DOMAIN, code, operation, message);
    }

    // ---- DER ----

    static final class Node {
        int tag;
        int number;
        int cls;
        boolean constructed;
        int from;
        int content;
        int length;
        int to;
        final List<Node> children = new ArrayList<Node>();
    }

    static byte[] content(byte[] source, Node node) {
        return Arrays.copyOfRange(source, node.content, node.content + node.length);
    }

    static byte[] encoded(byte[] source, Node node) {
        return Arrays.copyOfRange(source, node.from, node.to);
    }

    private static int octet(byte[] source, int offset, int end) {
        if (offset >= end) {
            throw bad("truncated DER");
        }
        return source[offset] & 0xff;
    }

    private static void bitString(byte[] s) {
        if (s.length == 0) {
            throw bad("invalid BIT STRING");
        }
        int unused = s[0] & 0xff;
        if (unused > 7 || (s.length == 1 && unused != 0)) {
            throw bad("invalid BIT STRING padding");
        }
        if (unused != 0 && ((s[s.length - 1] & 0xff) & ((1 << unused) - 1)) != 0) {
            throw bad("nonzero BIT STRING padding");
        }
    }

    private static void integer(byte[] s) {
        if (s.length == 0) {
            throw bad("empty INTEGER");
        }
        if (s.length > 1) {
            int a = s[0] & 0xff;
            int b = s[1] & 0xff;
            if ((a == 0 && (b & 128) == 0) || (a == 255 && (b & 128) != 0)) {
                throw bad("nonminimal INTEGER");
            }
        }
    }

    static String oid(byte[] s) {
        if (s.length == 0) {
            throw bad("empty OID");
        }
        if (s.length > 1024) {
            throw limit("OID encoding limit");
        }
        ArrayList<BigInteger> arcs = new ArrayList<BigInteger>();
        BigInteger value = BigInteger.ZERO;
        boolean starting = true;
        for (byte raw : s) {
            int octet = raw & 0xff;
            if (starting && octet == 128) {
                throw bad("nonminimal OID arc");
            }
            value = value.shiftLeft(7).add(BigInteger.valueOf(octet & 127));
            starting = false;
            if ((octet & 128) == 0) {
                arcs.add(value);
                if (arcs.size() > 127) {
                    throw limit("OID arc limit");
                }
                value = BigInteger.ZERO;
                starting = true;
            }
        }
        if (!starting) {
            throw bad("unterminated OID arc");
        }
        BigInteger first = arcs.remove(0);
        int root = first.compareTo(BigInteger.valueOf(40)) < 0 ? 0 : first.compareTo(BigInteger.valueOf(80)) < 0 ? 1 : 2;
        StringBuilder out = new StringBuilder();
        out.append(root).append('.').append(first.subtract(BigInteger.valueOf(40L * root)));
        for (BigInteger arc : arcs) {
            out.append('.').append(arc);
        }
        if (out.length() > 1024) {
            throw limit("OID text limit");
        }
        return out.toString();
    }

    private static int compareBytes(byte[] left, byte[] right) {
        int size = Math.min(left.length, right.length);
        for (int index = 0; index < size; index++) {
            int diff = (left[index] & 0xff) - (right[index] & 0xff);
            if (diff != 0) {
                return diff;
            }
        }
        return left.length - right.length;
    }

    private static Node node(byte[] source, int offset, int end, int depth, int[] budget) {
        if (depth > 32) {
            throw limit("DER nesting limit");
        }
        if (++budget[0] > 8192) {
            throw limit("DER node limit");
        }
        Node node = new Node();
        node.from = offset;
        int tag = octet(source, offset++, end);
        int number = tag & 31;
        if (number == 31) {
            number = 0;
            boolean first = true;
            while (true) {
                int part = octet(source, offset++, end);
                if (first && (part & 127) == 0) {
                    throw bad("nonminimal DER tag");
                }
                if (number > 0x1fffff) {
                    throw limit("DER tag limit");
                }
                number = (number << 7) | (part & 127);
                first = false;
                if ((part & 128) == 0) {
                    break;
                }
            }
            if (number < 31) {
                throw bad("nonminimal DER tag");
            }
        }
        long length = octet(source, offset++, end);
        if ((length & 128) != 0) {
            int width = (int) (length & 127);
            if (width == 0 || width > 4) {
                throw bad("indefinite or oversized DER length");
            }
            if (octet(source, offset, end) == 0) {
                throw bad("nonminimal DER length");
            }
            length = 0;
            for (int index = 0; index < width; index++) {
                length = (length << 8) | octet(source, offset++, end);
            }
            if (length < 128) {
                throw bad("nonminimal DER length");
            }
        }
        if (length > end - offset) {
            throw bad("truncated DER content");
        }
        int to = offset + (int) length;
        boolean constructed = (tag & 32) != 0;
        int cls = tag >> 6;
        if (cls == 0) {
            if (number == 0) {
                throw bad("DER end-of-content tag");
            }
            if ((number == 16 || number == 17) ? !constructed : constructed) {
                throw bad("invalid DER constructed form");
            }
        }
        node.tag = tag;
        node.number = number;
        node.cls = cls;
        node.constructed = constructed;
        node.content = offset;
        node.length = (int) length;
        node.to = to;
        if (constructed) {
            int cursor = offset;
            while (cursor < to) {
                Node child = node(source, cursor, to, depth + 1, budget);
                node.children.add(child);
                cursor = child.to;
            }
            if (cls == 0 && number == 17) {
                for (int index = 1; index < node.children.size(); index++) {
                    if (compareBytes(encoded(source, node.children.get(index - 1)),
                            encoded(source, node.children.get(index))) > 0) {
                        throw bad("noncanonical SET ordering");
                    }
                }
            }
        } else if (cls == 0) {
            byte[] s = Arrays.copyOfRange(source, offset, to);
            if (number == 1 && (length != 1 || (s[0] != 0 && s[0] != (byte) 0xff))) {
                throw bad("noncanonical BOOLEAN");
            }
            if (number == 2 || number == 10) {
                integer(s);
            }
            if (number == 3) {
                bitString(s);
            }
            if (number == 5 && length != 0) {
                throw bad("nonempty NULL");
            }
            if (number == 6) {
                oid(s);
            }
        }
        return node;
    }

    static Node parse(byte[] source) {
        if (source.length > 1048576) {
            throw limit("certificate byte limit");
        }
        Node node = node(source, 0, source.length, 0, new int[1]);
        if (node.to != source.length) {
            throw bad("trailing DER bytes");
        }
        return node;
    }

    private static void tag(Node node, int tag) {
        if (node == null || node.tag != tag) {
            throw bad("unexpected certificate field");
        }
    }

    private static List<Node> sequence(Node node, int size) {
        tag(node, 0x30);
        if (size >= 0 && node.children.size() != size) {
            throw bad("unexpected certificate field count");
        }
        return node.children;
    }

    private static void algorithm(Node node) {
        List<Node> fields = sequence(node, -1);
        if (fields.size() != 1 && fields.size() != 2) {
            throw bad("invalid AlgorithmIdentifier");
        }
        tag(fields.get(0), 6);
    }

    private static final char[] HEX = "0123456789abcdef".toCharArray();

    static String hex(byte[] bytes) {
        char[] out = new char[bytes.length * 2];
        for (int index = 0; index < bytes.length; index++) {
            out[2 * index] = HEX[(bytes[index] >> 4) & 15];
            out[2 * index + 1] = HEX[bytes[index] & 15];
        }
        return new String(out);
    }

    static byte[] unhex(String text) {
        byte[] out = new byte[text.length() / 2];
        for (int index = 0; index < out.length; index++) {
            out[index] = (byte) Integer.parseInt(text.substring(2 * index, 2 * index + 2), 16);
        }
        return out;
    }

    /** Атрибут имени: OID, тег значения, hex значения. */
    private static List<List<Object[]>> name(byte[] source, Node node) {
        List<Node> groups = sequence(node, -1);
        if (groups.size() > 128) {
            throw limit("DN group limit");
        }
        ArrayList<List<Object[]>> result = new ArrayList<List<Object[]>>();
        for (Node group : groups) {
            tag(group, 0x31);
            if (group.children.isEmpty()) {
                throw bad("empty RDN");
            }
            if (group.children.size() > 32) {
                throw limit("RDN member limit");
            }
            ArrayList<Object[]> attributes = new ArrayList<Object[]>();
            for (Node member : group.children) {
                List<Node> parts = sequence(member, 2);
                tag(parts.get(0), 6);
                if (parts.get(1).constructed) {
                    throw bad("constructed name attribute");
                }
                attributes.add(new Object[] {oid(content(source, parts.get(0))), parts.get(1).tag,
                    hex(content(source, parts.get(1)))});
            }
            result.add(attributes);
        }
        return result;
    }

    private static final java.util.regex.Pattern UTC =
        java.util.regex.Pattern.compile("\\A([0-9]{2})([0-9]{2})([0-9]{2})([0-9]{2})([0-9]{2})([0-9]{2})Z\\z");
    private static final java.util.regex.Pattern GENERALIZED =
        java.util.regex.Pattern.compile("\\A([0-9]{4})([0-9]{2})([0-9]{2})([0-9]{2})([0-9]{2})([0-9]{2})Z\\z");

    private static String time(byte[] source, Node node) {
        String text = new String(content(source, node), StandardCharsets.ISO_8859_1);
        java.util.regex.Matcher match;
        int year;
        if (node.tag == 23 && (match = UTC.matcher(text)).find()) {
            year = Integer.parseInt(match.group(1));
            year += year >= 50 ? 1900 : 2000;
        } else if (node.tag == 24 && (match = GENERALIZED.matcher(text)).find()) {
            year = Integer.parseInt(match.group(1));
        } else {
            throw bad("invalid certificate time");
        }
        int month = Integer.parseInt(match.group(2));
        int day = Integer.parseInt(match.group(3));
        int hour = Integer.parseInt(match.group(4));
        int minute = Integer.parseInt(match.group(5));
        int second = Integer.parseInt(match.group(6));
        if (!(year >= 1 && month >= 1 && month <= 12 && day >= 1 && day <= 31 && hour < 24 && minute < 60 && second < 60)) {
            throw bad("invalid certificate calendar");
        }
        try {
            return Long.toString(LocalDateTime.of(year, month, day, hour, minute, second).toEpochSecond(ZoneOffset.UTC));
        } catch (java.time.DateTimeException error) {
            throw bad("invalid certificate calendar");
        }
    }

    /** Снимок фактов сертификата (структура {@code X509Der::certificate}). */
    static final class Facts {
        List<List<Object[]>> subject;
        List<List<Object[]>> issuer;
        String[] times;
        String spki;
        /** OID, critical (0/1), hex значения. */
        List<Object[]> extensions;
    }

    static Facts certificate(byte[] source) {
        Node root = parse(source);
        List<Node> top = sequence(root, 3);
        Node tbs = top.get(0);
        Node algorithm = top.get(1);
        Node signature = top.get(2);
        algorithm(algorithm);
        tag(signature, 3);
        byte[] sig = content(source, signature);
        if (!(sig.length > 1 && sig[0] == 0)) {
            throw bad("invalid certificate signature envelope");
        }
        ArrayList<Node> fields = new ArrayList<Node>(sequence(tbs, -1));
        int version = 0;
        if (!fields.isEmpty() && fields.get(0).tag == 0xa0) {
            Node explicit = fields.remove(0);
            if (explicit.children.size() != 1) {
                throw bad("invalid certificate version");
            }
            Node value = explicit.children.get(0);
            tag(value, 2);
            byte[] s = content(source, value);
            if (!(s.length == 1 && (s[0] == 1 || s[0] == 2))) {
                throw bad("invalid or explicit default certificate version");
            }
            version = s[0];
        }
        if (fields.size() < 6) {
            throw bad("missing certificate fields");
        }
        Node serial = fields.get(0);
        Node innerAlgorithm = fields.get(1);
        Node issuer = fields.get(2);
        Node validity = fields.get(3);
        Node subject = fields.get(4);
        Node spki = fields.get(5);
        List<Node> rest = fields.subList(6, fields.size());
        tag(serial, 2);
        algorithm(innerAlgorithm);
        if (!Arrays.equals(encoded(source, algorithm), encoded(source, innerAlgorithm))) {
            throw bad("certificate signature algorithms differ");
        }
        Facts facts = new Facts();
        facts.issuer = name(source, issuer);
        if (facts.issuer.isEmpty()) {
            throw bad("empty certificate issuer");
        }
        facts.subject = name(source, subject);
        List<Node> period = sequence(validity, 2);
        facts.times = new String[] {time(source, period.get(0)), time(source, period.get(1))};
        if (Long.parseLong(facts.times[0]) > Long.parseLong(facts.times[1])) {
            throw bad("reversed certificate validity");
        }
        List<Node> key = sequence(spki, 2);
        algorithm(key.get(0));
        tag(key.get(1), 3);
        if (!(key.get(1).length > 1 && content(source, key.get(1))[0] == 0)) {
            throw bad("invalid public key bit string");
        }
        facts.spki = hex(encoded(source, spki));
        facts.extensions = new ArrayList<Object[]>();
        HashSet<String> seen = new HashSet<String>();
        int previous = 0;
        for (Node field : rest) {
            int order = field.tag == 0x81 ? 1 : field.tag == 0x82 ? 2 : field.tag == 0xa3 ? 3 : 0;
            if (order <= previous) {
                throw bad("invalid optional certificate fields");
            }
            previous = order;
            if (order < 3) {
                if (version == 0) {
                    throw bad("unique identifier requires version 2 or 3");
                }
                bitString(content(source, field));
                continue;
            }
            if (version != 2) {
                throw bad("extensions require version 3");
            }
            if (field.children.size() != 1) {
                throw bad("invalid extensions envelope");
            }
            List<Node> items = sequence(field.children.get(0), -1);
            if (items.isEmpty()) {
                throw bad("empty extensions");
            }
            if (items.size() > 128) {
                throw limit("extension count limit");
            }
            for (Node item : items) {
                ArrayList<Node> parts = new ArrayList<Node>(sequence(item, -1));
                if (parts.size() != 2 && parts.size() != 3) {
                    throw bad("invalid extension");
                }
                Node identifier = parts.remove(0);
                tag(identifier, 6);
                String oid = oid(content(source, identifier));
                if (!seen.add(oid)) {
                    throw bad("duplicate certificate extension");
                }
                int critical = 0;
                if (parts.size() == 2) {
                    Node flag = parts.remove(0);
                    tag(flag, 1);
                    if (content(source, flag)[0] != (byte) 0xff) {
                        throw bad("explicit default extension critical flag");
                    }
                    critical = 1;
                }
                tag(parts.get(0), 4);
                facts.extensions.add(new Object[] {oid, critical, hex(content(source, parts.get(0)))});
            }
        }
        return facts;
    }

    // ---- снимок: канонический JSON Perl-provider ----

    private static void quote(StringBuilder out, String text) {
        out.append('"').append(text).append('"');
    }

    private static void names(StringBuilder out, List<List<Object[]>> groups) {
        out.append('[');
        for (int group = 0; group < groups.size(); group++) {
            if (group > 0) {
                out.append(',');
            }
            out.append('[');
            List<Object[]> members = groups.get(group);
            for (int member = 0; member < members.size(); member++) {
                Object[] attribute = members.get(member);
                if (member > 0) {
                    out.append(',');
                }
                out.append('[');
                quote(out, (String) attribute[0]);
                out.append(',').append(attribute[1]).append(',');
                quote(out, (String) attribute[2]);
                out.append(']');
            }
            out.append(']');
        }
        out.append(']');
    }

    static String json(Facts facts) {
        StringBuilder out = new StringBuilder();
        out.append("{\"extensions\":[");
        for (int index = 0; index < facts.extensions.size(); index++) {
            Object[] extension = facts.extensions.get(index);
            if (index > 0) {
                out.append(',');
            }
            out.append('[');
            quote(out, (String) extension[0]);
            out.append(',').append(extension[1]).append(',');
            quote(out, (String) extension[2]);
            out.append(']');
        }
        out.append("],\"issuer\":");
        names(out, facts.issuer);
        out.append(",\"spki\":");
        quote(out, facts.spki);
        out.append(",\"subject\":");
        names(out, facts.subject);
        out.append(",\"times\":[").append(facts.times[0]).append(',').append(facts.times[1]);
        out.append("],\"version\":1}");
        return out.toString();
    }

    /** Минимальный JSON-разбор снимка (массивы, строки, целые). */
    private static final class Reader {
        final String text;
        int at;

        Reader(String text) {
            this.text = text;
        }

        Object value() {
            skip();
            if (at >= text.length()) {
                throw bad("snapshot");
            }
            char c = text.charAt(at);
            if (c == '[') {
                at++;
                ArrayList<Object> items = new ArrayList<Object>();
                skip();
                if (peek() == ']') {
                    at++;
                    return items;
                }
                while (true) {
                    items.add(value());
                    skip();
                    char next = take();
                    if (next == ']') {
                        return items;
                    }
                    if (next != ',') {
                        throw bad("snapshot");
                    }
                }
            }
            if (c == '{') {
                at++;
                HashMap<String, Object> entries = new HashMap<String, Object>();
                skip();
                if (peek() == '}') {
                    at++;
                    return entries;
                }
                while (true) {
                    Object key = value();
                    if (!(key instanceof String)) {
                        throw bad("snapshot");
                    }
                    skip();
                    if (take() != ':') {
                        throw bad("snapshot");
                    }
                    entries.put((String) key, value());
                    skip();
                    char next = take();
                    if (next == '}') {
                        return entries;
                    }
                    if (next != ',') {
                        throw bad("snapshot");
                    }
                }
            }
            if (c == '"') {
                at++;
                StringBuilder out = new StringBuilder();
                while (true) {
                    char next = take();
                    if (next == '"') {
                        return out.toString();
                    }
                    if (next == '\\') {
                        char escape = take();
                        switch (escape) {
                            case 'u':
                                out.append((char) Integer.parseInt(text.substring(at, at + 4), 16));
                                at += 4;
                                break;
                            case 'n':
                                out.append('\n');
                                break;
                            case 't':
                                out.append('\t');
                                break;
                            case 'r':
                                out.append('\r');
                                break;
                            case 'b':
                                out.append('\b');
                                break;
                            case 'f':
                                out.append('\f');
                                break;
                            default:
                                out.append(escape);
                        }
                    } else {
                        out.append(next);
                    }
                }
            }
            int start = at;
            if (c == '-') {
                at++;
            }
            while (at < text.length() && Character.isDigit(text.charAt(at))) {
                at++;
            }
            if (at == start || (at == start + 1 && c == '-')) {
                throw bad("snapshot");
            }
            return new BigInteger(text.substring(start, at));
        }

        private void skip() {
            while (at < text.length() && Character.isWhitespace(text.charAt(at))) {
                at++;
            }
        }

        private char peek() {
            if (at >= text.length()) {
                throw bad("snapshot");
            }
            return text.charAt(at);
        }

        private char take() {
            char c = peek();
            at++;
            return c;
        }
    }

    private static boolean matches(Object value, String pattern) {
        return value instanceof String && ((String) value).matches(pattern);
    }

    private static final String OID = "[012](?:\\.[0-9]+)+";
    private static final String HEXES = "(?:[0-9a-f]{2})*";

    @SuppressWarnings("unchecked")
    private static List<List<Object[]>> snapshotName(Object value) {
        if (!(value instanceof List) || ((List<?>) value).size() > 128) {
            throw bad("snapshot");
        }
        ArrayList<List<Object[]>> out = new ArrayList<List<Object[]>>();
        for (Object group : (List<Object>) value) {
            if (!(group instanceof List) || ((List<?>) group).isEmpty() || ((List<?>) group).size() > 32) {
                throw bad("snapshot");
            }
            ArrayList<Object[]> members = new ArrayList<Object[]>();
            for (Object attribute : (List<Object>) group) {
                if (!(attribute instanceof List) || ((List<?>) attribute).size() != 3) {
                    throw bad("snapshot");
                }
                List<Object> parts = (List<Object>) attribute;
                if (!(matches(parts.get(0), OID) && ((String) parts.get(0)).length() <= 1024
                        && parts.get(1) instanceof BigInteger && ((BigInteger) parts.get(1)).signum() >= 0
                        && ((BigInteger) parts.get(1)).compareTo(BigInteger.valueOf(255)) <= 0
                        && matches(parts.get(2), HEXES))) {
                    throw bad("snapshot");
                }
                members.add(new Object[] {parts.get(0), ((BigInteger) parts.get(1)).intValue(), parts.get(2)});
            }
            out.add(members);
        }
        return out;
    }

    /** {@code _facts}: разбор и проверка снимка. */
    @SuppressWarnings("unchecked")
    static Facts facts(String text, String operation) {
        try {
            if (text.length() > 4194304) {
                throw bad("snapshot");
            }
            Reader reader = new Reader(text);
            Object root = reader.value();
            if (!(root instanceof Map)) {
                throw bad("snapshot");
            }
            Map<String, Object> map = (Map<String, Object>) root;
            Object times = map.get("times");
            Object version = map.get("version");
            if (!(BigInteger.ONE.equals(version) || "1".equals(version)) || !(times instanceof List) || ((List<?>) times).size() != 2
                    || !matches(map.get("spki"), "(?:[0-9a-f]{2})+")) {
                throw bad("snapshot");
            }
            Facts facts = new Facts();
            facts.times = new String[2];
            for (int index = 0; index < 2; index++) {
                Object time = ((List<?>) times).get(index);
                String spelled = time instanceof BigInteger ? time.toString() : time instanceof String ? (String) time : "";
                if (!spelled.matches("-?[0-9]{1,12}")) {
                    throw bad("snapshot");
                }
                facts.times[index] = spelled;
            }
            facts.spki = (String) map.get("spki");
            facts.subject = snapshotName(map.get("subject"));
            facts.issuer = snapshotName(map.get("issuer"));
            Object extensions = map.get("extensions");
            if (!(extensions instanceof List) || ((List<?>) extensions).size() > 128) {
                throw bad("snapshot");
            }
            facts.extensions = new ArrayList<Object[]>();
            for (Object extension : (List<Object>) extensions) {
                if (!(extension instanceof List) || ((List<?>) extension).size() != 3) {
                    throw bad("snapshot");
                }
                List<Object> parts = (List<Object>) extension;
                Object critical = parts.get(1);
                if (!(matches(parts.get(0), OID) && ((String) parts.get(0)).length() <= 1024
                        && critical instanceof BigInteger && ((BigInteger) critical).signum() >= 0
                        && ((BigInteger) critical).compareTo(BigInteger.ONE) <= 0 && matches(parts.get(2), HEXES))) {
                    throw bad("snapshot");
                }
                facts.extensions.add(new Object[] {parts.get(0), ((BigInteger) critical).intValue(), parts.get(2)});
            }
            if (reader.at != text.length()) {
                reader.skip();
                if (reader.at != text.length()) {
                    throw bad("snapshot");
                }
            }
            return facts;
        } catch (Bad error) {
            throw failure("invalid_input", operation, "invalid private certificate snapshot");
        } catch (RuntimeException error) {
            throw failure("invalid_input", operation, "invalid private certificate snapshot");
        }
    }

    // ---- наблюдатели расширений ----

    private static byte[] extension(Facts facts, String oid) {
        byte[] found = null;
        for (Object[] extension : facts.extensions) {
            if (extension[0].equals(oid)) {
                if (found != null) {
                    throw bad("duplicate extension");
                }
                found = unhex((String) extension[2]);
            }
        }
        return found;
    }

    private static List<Object> strings(String... values) {
        ArrayList<Object> out = new ArrayList<Object>();
        for (String value : values) {
            out.add(value);
        }
        return out;
    }

    static List<Object> san(Facts facts) {
        int count = 0;
        for (Object[] extension : facts.extensions) {
            if (extension[0].equals("2.5.29.17")) {
                count++;
            }
        }
        if (count > 1) {
            throw bad("duplicate SAN");
        }
        ArrayList<Object> names = new ArrayList<Object>();
        byte[] source = extension(facts, "2.5.29.17");
        if (source == null) {
            return names;
        }
        Node node = parse(source);
        if (node.tag != 48 || node.children.isEmpty()) {
            throw bad("GeneralNames sequence required");
        }
        if (node.children.size() > 256) {
            throw limit("GeneralNames count");
        }
        for (Node name : node.children) {
            int number = name.number;
            if (!(name.cls == 2 && number <= 8)) {
                throw bad("GeneralName tag");
            }
            boolean constructed = number == 0 || number == 3 || number == 4 || number == 5;
            if (name.constructed != constructed) {
                throw bad("GeneralName form");
            }
            byte[] value = content(source, name);
            if (number == 2) {
                if (value.length == 0) {
                    throw bad("DNS SAN IA5String");
                }
                for (byte octet : value) {
                    if (octet < 0x20 || octet > 0x7e) {
                        throw bad("DNS SAN IA5String");
                    }
                }
                if (value.length > 253) {
                    throw limit("DNS SAN bytes");
                }
                names.add(strings("DNS", new String(value, StandardCharsets.ISO_8859_1)));
            } else if (number == 7) {
                if (value.length != 4 && value.length != 16) {
                    throw bad("IP SAN width");
                }
                names.add(strings("IP", NetNatives.format(value)));
            }
        }
        return names;
    }

    static List<Object> constraints(Facts facts) {
        byte[] source = extension(facts, "2.5.29.19");
        if (source == null) {
            return strings("0", "");
        }
        Node node = parse(source);
        if (node.tag != 48) {
            throw bad("basicConstraints sequence");
        }
        ArrayList<Node> fields = new ArrayList<Node>(node.children);
        String ca = "0";
        String depth = "";
        if (!fields.isEmpty() && fields.get(0).tag == 1) {
            Node flag = fields.remove(0);
            if (content(source, flag)[0] != (byte) 0xff) {
                throw bad("explicit basicConstraints default");
            }
            ca = "1";
        }
        if (!fields.isEmpty()) {
            if (!(fields.size() == 1 && fields.get(0).tag == 2 && ca.equals("1"))) {
                throw bad("basicConstraints depth");
            }
            byte[] encoded = content(source, fields.get(0));
            if ((encoded[0] & 128) != 0) {
                throw bad("negative path depth");
            }
            if (encoded.length > 8) {
                throw limit("path depth representation");
            }
            BigInteger value = new BigInteger(1, encoded);
            if (value.compareTo(BigInteger.valueOf(Long.MAX_VALUE)) > 0) {
                throw limit("path depth representation");
            }
            depth = value.toString();
        }
        return strings(ca, depth);
    }

    static List<Object> usages(Facts facts) {
        ArrayList<Object> bits = new ArrayList<Object>();
        byte[] source = extension(facts, "2.5.29.15");
        if (source != null) {
            Node node = parse(source);
            if (node.tag != 3) {
                throw bad("keyUsage BIT STRING");
            }
            byte[] value = content(source, node);
            int unused = value[0] & 0xff;
            int count = (value.length - 1) * 8 - unused;
            if (!(count > 0 && count <= 9 && ((value[value.length - 1] & 0xff) & (1 << unused)) != 0)) {
                throw bad("keyUsage named-bit encoding");
            }
            for (int index = 0; index < count; index++) {
                if (((value[1 + index / 8] & 0xff) & (128 >> (index % 8))) != 0) {
                    bits.add(BigInteger.valueOf(index));
                }
            }
        }
        return bits;
    }

    static List<Object> purposes(Facts facts) {
        ArrayList<Object> oids = new ArrayList<Object>();
        byte[] source = extension(facts, "2.5.29.37");
        if (source == null) {
            return oids;
        }
        Node node = parse(source);
        if (node.tag != 48 || node.children.isEmpty()) {
            throw bad("extendedKeyUsage sequence");
        }
        if (node.children.size() > 128) {
            throw limit("extendedKeyUsage count");
        }
        for (Node child : node.children) {
            if (child.tag != 6) {
                throw bad("extendedKeyUsage OID");
            }
            oids.add(oid(content(source, child)));
        }
        return oids;
    }

    private static Failure observation(Bad error, String operation, String message) {
        return failure(error.kind.equals("limit") ? "resource_exhausted" : "invalid_data", operation, message);
    }

    // ---- natives ----

    private static void hostCheck(byte[] der, String operation) {
        try {
            X509Certificate certificate = (X509Certificate) CertificateFactory.getInstance("X.509")
                .generateCertificate(new ByteArrayInputStream(der));
            certificate.getPublicKey().getEncoded();
        } catch (Exception error) {
            throw failure("invalid_data", operation, "certificate or public key rejected by backend");
        }
    }

    public static String runtime_x509__certificate_facts(List<?> origin) {
        String operation = symbol("certificate_facts", 1);
        if (origin.size() > 1048576) {
            throw failure("resource_exhausted", operation, "certificate byte limit");
        }
        byte[] der = Natives.toBytes(origin);
        Facts facts;
        try {
            facts = certificate(der);
        } catch (Bad error) {
            throw observation(error, operation, "invalid bounded DER certificate");
        }
        hostCheck(der, operation);
        return json(facts);
    }

    public static List<Object> runtime_x509__certificate_times(String text) {
        Facts facts = facts(text, symbol("certificate_times", 1));
        return strings(facts.times[0], facts.times[1]);
    }

    public static List<Object> runtime_x509__certificate_spki(String text) {
        byte[] bytes = unhex(facts(text, symbol("certificate_spki", 1)).spki);
        return Natives.fromBytes(bytes, bytes.length);
    }

    public static List<Object> runtime_x509__certificate_names(String text, boolean issuer) {
        String operation = symbol("certificate_names", 2);
        Facts facts = facts(text, operation);
        try {
            return decodedName(issuer ? facts.issuer : facts.subject);
        } catch (Bad error) {
            String code = error.kind.equals("unsupported") ? "unsupported"
                : error.kind.equals("limit") ? "resource_exhausted" : "invalid_data";
            throw failure(code, operation, "certificate name cannot be decoded");
        }
    }

    public static List<Object> runtime_x509__certificate_san(String text) {
        String operation = symbol("certificate_san", 1);
        Facts facts = facts(text, operation);
        try {
            return san(facts);
        } catch (Bad error) {
            throw observation(error, operation, "invalid bounded certificate subjectAltName");
        }
    }

    public static List<Object> runtime_x509__certificate_constraints(String text) {
        String operation = symbol("certificate_constraints", 1);
        Facts facts = facts(text, operation);
        try {
            return constraints(facts);
        } catch (Bad error) {
            throw observation(error, operation, "invalid bounded certificate extension");
        }
    }

    public static List<Object> runtime_x509__certificate_key_usages(String text) {
        String operation = symbol("certificate_key_usages", 1);
        Facts facts = facts(text, operation);
        try {
            return usages(facts);
        } catch (Bad error) {
            throw observation(error, operation, "invalid bounded certificate extension");
        }
    }

    public static List<Object> runtime_x509__certificate_purposes(String text) {
        String operation = symbol("certificate_purposes", 1);
        Facts facts = facts(text, operation);
        try {
            return purposes(facts);
        } catch (Bad error) {
            throw observation(error, operation, "invalid bounded certificate extension");
        }
    }

    public static List<Object> runtime_x509__certificate_extensions(String text) {
        Facts facts = facts(text, symbol("certificate_extensions", 1));
        ArrayList<Object> out = new ArrayList<Object>();
        for (Object[] extension : facts.extensions) {
            out.add(strings((String) extension[0], extension[1].toString(), (String) extension[2]));
        }
        return out;
    }

    private static int[] units(byte[] bytes, int width) {
        int[] out = new int[bytes.length / width];
        for (int index = 0; index < out.length; index++) {
            int value = 0;
            for (int part = 0; part < width; part++) {
                value = (value << 8) | (bytes[index * width + part] & 0xff);
            }
            out[index] = value;
        }
        return out;
    }

    private static List<Object> decodedName(List<List<Object[]>> groups) {
        ArrayList<Object> result = new ArrayList<Object>();
        long cost = 0;
        for (List<Object[]> group : groups) {
            ArrayList<Object> members = new ArrayList<Object>();
            for (Object[] attribute : group) {
                String oid = (String) attribute[0];
                int tag = (Integer) attribute[1];
                byte[] s = unhex((String) attribute[2]);
                String text;
                if (tag == 12) {
                    try {
                        text = StandardCharsets.UTF_8.newDecoder().decode(java.nio.ByteBuffer.wrap(s)).toString();
                    } catch (java.nio.charset.CharacterCodingException error) {
                        throw bad("invalid UTF8String");
                    }
                } else if (tag == 19) {
                    text = new String(s, StandardCharsets.ISO_8859_1);
                    if (!text.matches("[A-Za-z0-9 '()+,\\-./:=?]*")) {
                        throw bad("invalid PrintableString");
                    }
                } else if (tag == 22) {
                    for (byte octet : s) {
                        if (octet < 0) {
                            throw bad("invalid IA5String");
                        }
                    }
                    text = new String(s, StandardCharsets.ISO_8859_1);
                } else if (tag == 30) {
                    if (s.length % 2 != 0) {
                        throw bad("invalid BMPString");
                    }
                    StringBuilder out = new StringBuilder();
                    for (int unit : units(s, 2)) {
                        if (unit >= 0xd800 && unit <= 0xdfff) {
                            throw bad("invalid BMPString");
                        }
                        out.append((char) unit);
                    }
                    text = out.toString();
                } else if (tag == 28) {
                    if (s.length % 4 != 0) {
                        throw bad("invalid UniversalString");
                    }
                    StringBuilder out = new StringBuilder();
                    for (int point : units(s, 4)) {
                        if (point < 0 || point > 0x10ffff || (point >= 0xd800 && point <= 0xdfff)) {
                            throw bad("invalid UniversalString");
                        }
                        out.appendCodePoint(point);
                    }
                    text = out.toString();
                } else {
                    throw new Bad("unsupported", "name attribute encoding is not supported");
                }
                int length = text.codePointCount(0, text.length());
                if (length > 4096) {
                    throw limit("name scalar limit");
                }
                cost += oid.length() + length;
                if (cost > 65536) {
                    throw limit("name scalar limit");
                }
                members.add(strings(oid, text));
            }
            result.add(members);
        }
        return result;
    }

    // ---- системные корни ----

    private static List<String> rootFiles() {
        String fileOverride = Host.environment.get("SSL_CERT_FILE");
        String dirOverride = Host.environment.get("SSL_CERT_DIR");
        ArrayList<String> files = new ArrayList<String>();
        if (fileOverride != null && !fileOverride.isEmpty()) {
            files.add(fileOverride);
        } else {
            for (String candidate : new String[] {"/etc/ssl/cert.pem", "/etc/ssl/certs/ca-certificates.crt",
                    "/etc/pki/tls/certs/ca-bundle.crt", "/etc/ssl/ca-bundle.pem", "/usr/local/etc/openssl/cert.pem",
                    "/opt/homebrew/etc/openssl@3/cert.pem"}) {
                if (new File(candidate).isFile()) {
                    files.add(candidate);
                    break;
                }
            }
        }
        String[] directories = dirOverride != null && !dirOverride.isEmpty() ? dirOverride.split(":")
            : (files.isEmpty() && new File("/etc/ssl/certs").isDirectory() ? new String[] {"/etc/ssl/certs"} : new String[0]);
        if (directories.length > 32) {
            throw failure("resource_exhausted", symbol("system_certificates", 0), "CA directory count limit");
        }
        for (String directory : directories) {
            String[] entries = new File(directory).list();
            if (entries == null) {
                throw failure("not_found", symbol("system_certificates", 0), "cannot open default CA directory");
            }
            Arrays.sort(entries);
            for (String entry : entries) {
                if (entry.matches("[0-9a-fA-F]{8}\\.[0-9]+")) {
                    files.add(directory + File.separator + entry);
                    if (files.size() > 2048) {
                        throw failure("resource_exhausted", symbol("system_certificates", 0), "CA file count limit");
                    }
                }
            }
        }
        return files;
    }

    private static final java.util.regex.Pattern PEM =
        java.util.regex.Pattern.compile("-----BEGIN ([A-Z0-9 -]+)-----");

    public static List<Object> runtime_x509__system_certificates() {
        String operation = symbol("system_certificates", 0);
        List<String> files = rootFiles();
        ArrayList<Object> roots = new ArrayList<Object>();
        HashSet<String> seen = new HashSet<String>();
        long total = 0;
        for (String path : files) {
            byte[] raw;
            try {
                java.nio.file.Path file = Host.path(path);
                if (!Files.isRegularFile(file)) {
                    throw failure("invalid_input", operation, "default CA source must be a regular file");
                }
                raw = Files.readAllBytes(file);
            } catch (IOException error) {
                throw failure(Host.code(error), operation, "cannot read default CA file");
            }
            total += raw.length;
            if (raw.length > 8388608 || total > 16777216) {
                throw failure("resource_exhausted", operation, "CA source byte limit");
            }
            String text = new String(raw, StandardCharsets.ISO_8859_1);
            java.util.regex.Matcher match = PEM.matcher(text);
            int from = 0;
            while (match.find(from)) {
                String kind = match.group(1);
                String closing = "-----END " + kind + "-----";
                int end = text.indexOf(closing, match.end());
                if (end < 0) {
                    throw failure("invalid_data", operation, "unclosed default CA PEM block");
                }
                from = end + closing.length();
                if (kind.equals("TRUSTED CERTIFICATE")) {
                    throw failure("unsupported", operation, "auxiliary certificate trust cannot be flattened");
                }
                if (!kind.equals("CERTIFICATE") && !kind.equals("X509 CERTIFICATE")) {
                    continue;
                }
                String encoded = text.substring(match.end(), end).replaceAll("[ \\t\\r\\n\\f]", "");
                if (encoded.isEmpty() || encoded.length() % 4 != 0 || !encoded.matches("[A-Za-z0-9+/]*={0,2}")) {
                    throw failure("invalid_data", operation, "invalid default CA base64");
                }
                byte[] der = java.util.Base64.getDecoder().decode(encoded);
                if (!java.util.Base64.getEncoder().encodeToString(der).equals(encoded)) {
                    throw failure("invalid_data", operation, "noncanonical default CA base64");
                }
                if (der.length > 1048576) {
                    throw failure("resource_exhausted", operation, "CA certificate byte limit");
                }
                if (!seen.add(hex(der))) {
                    continue;
                }
                if (roots.size() >= 1024) {
                    throw failure("resource_exhausted", operation, "CA certificate count limit");
                }
                try {
                    certificate(der);
                } catch (Bad error) {
                    throw failure(error.kind.equals("limit") ? "resource_exhausted" : "invalid_data", operation,
                        "invalid bounded default CA certificate");
                }
                hostCheck(der, operation);
                roots.add(Natives.fromBytes(der, der.length));
            }
        }
        if (roots.isEmpty()) {
            throw failure("not_found", operation, "no actual default CA certificates");
        }
        return roots;
    }

    // ---- проверка цепочки ----

    private static final String VERIFY = "core.x509.verify";

    private static Failure verifyFailure(String code, String message) {
        return NativeFailure.raise(VERIFY, code, symbol("verify_chain", 6), message);
    }

    /** Исход проверки, не связанный с данными: {@code permission_denied}. */
    private static final class Rejected extends RuntimeException {
        private static final long serialVersionUID = 1L;

        Rejected(String reason) {
            super(reason, null, false, false);
        }
    }

    private static Rat rational(String time) {
        int slash = time.indexOf('/');
        if (slash >= 0) {
            return Rat.make(new BigInteger(time.substring(0, slash)), new BigInteger(time.substring(slash + 1)));
        }
        int dot = time.indexOf('.');
        if (dot >= 0) {
            String fraction = time.substring(dot + 1);
            BigInteger scale = BigInteger.TEN.pow(fraction.length());
            BigInteger whole = new BigInteger(time.substring(0, dot));
            BigInteger part = new BigInteger(fraction);
            BigInteger numerator = whole.multiply(scale).add(whole.signum() < 0 || time.startsWith("-") ? part.negate() : part);
            return Rat.make(numerator, scale);
        }
        return Rat.make(new BigInteger(time), BigInteger.ONE);
    }

    private static String timeFailure(Facts facts, Rat at) {
        if (at.compareTo(Rat.make(new BigInteger(facts.times[0]), BigInteger.ONE)) < 0) {
            return "not_yet_valid";
        }
        if (at.compareTo(Rat.make(new BigInteger(facts.times[1]), BigInteger.ONE)) > 0) {
            return "expired";
        }
        return null;
    }

    @SuppressWarnings("unchecked")
    private static boolean dnsMatches(Facts facts, String host) {
        for (Object entry : san(facts)) {
            List<Object> parts = (List<Object>) entry;
            if (!parts.get(0).equals("DNS")) {
                continue;
            }
            String name = ((String) parts.get(1)).toLowerCase(java.util.Locale.ROOT);
            if (name.equals(host)) {
                return true;
            }
            java.util.regex.Matcher wildcard = java.util.regex.Pattern.compile("\\A\\*\\.([^*]+)\\z").matcher(name);
            if (!wildcard.find()) {
                continue;
            }
            String suffix = wildcard.group(1);
            List<String> labels;
            int width;
            try {
                Object[] parts2 = IdnaNatives.parts(suffix);
                labels = (List<String>) parts2[0];
                width = (Integer) parts2[1];
            } catch (RuntimeException error) {
                continue;
            }
            if (labels.size() <= width || !String.join(".", labels).equals(suffix)) {
                continue;
            }
            int dot = host.indexOf('.');
            if (dot > 0 && host.substring(dot + 1).equals(suffix)) {
                return true;
            }
        }
        return false;
    }

    @SuppressWarnings("unchecked")
    private static boolean ipMatches(Facts facts, byte[] address) {
        for (Object entry : san(facts)) {
            List<Object> parts = (List<Object>) entry;
            if (parts.get(0).equals("IP") && parts.get(1).equals(NetNatives.format(address))) {
                return true;
            }
        }
        return false;
    }

    private static final Set<String> KNOWN_CRITICAL = new HashSet<String>(Arrays.asList("2.5.29.15", "2.5.29.17",
        "2.5.29.19", "2.5.29.37", "2.5.29.30", "2.5.29.32", "2.5.29.36", "2.5.29.54", "2.5.29.35", "2.5.29.14",
        "2.5.29.33"));

    private static boolean critical(Facts facts) {
        for (Object[] extension : facts.extensions) {
            if (((Integer) extension[1]) == 1 && !KNOWN_CRITICAL.contains(extension[0])) {
                return true;
            }
        }
        return false;
    }

    private static boolean present(Facts facts, String oid) {
        for (Object[] extension : facts.extensions) {
            if (extension[0].equals(oid)) {
                return true;
            }
        }
        return false;
    }

    private static boolean criticalExtension(Facts facts, String oid) {
        for (Object[] extension : facts.extensions) {
            if (extension[0].equals(oid)) {
                return ((Integer) extension[1]) == 1;
            }
        }
        return false;
    }

    /**
     * Профиль RFC 5280 под {@code X509_V_FLAG_X509_STRICT} OpenSSL для
     * сертификатов пути (кроме выбранного якоря): у CA — критический
     * basicConstraints, keyUsage и SKI; у не самовыпущенного — AKI; пустой
     * subject — только с критическим SAN; pathlen — только у CA.
     */
    private static void strict(Facts facts, X509Certificate certificate) {
        List<Object> basic = constraints(facts);
        boolean ca = basic.get(0).equals("1");
        if (ca && (!criticalExtension(facts, "2.5.29.19") || !present(facts, "2.5.29.15")
                || !present(facts, "2.5.29.14"))) {
            throw new Rejected("invalid_chain");
        }
        boolean selfIssued = certificate.getSubjectX500Principal().equals(certificate.getIssuerX500Principal());
        if (!selfIssued && !present(facts, "2.5.29.35")) {
            throw new Rejected("invalid_chain");
        }
        if (facts.subject.isEmpty() && !criticalExtension(facts, "2.5.29.17")) {
            throw new Rejected("invalid_chain");
        }
    }

    /** Назначение сертификата пути (OpenSSL {@code X509_PURPOSE_SSL_SERVER/CLIENT}). */
    private static void purpose(Facts facts, String purpose, boolean leaf) {
        List<Object> eku = purposes(facts);
        if (!eku.isEmpty() && !eku.contains(purpose) && !eku.contains("2.5.29.37.0")) {
            throw new Rejected("invalid_purpose");
        }
        if (leaf) {
            List<Object> usage = usages(facts);
            if (!usage.isEmpty()) {
                boolean signature = usage.contains(BigInteger.ZERO);
                boolean encipher = usage.contains(BigInteger.valueOf(2));
                boolean agreement = usage.contains(BigInteger.valueOf(4));
                boolean server = purpose.equals("1.3.6.1.5.5.7.3.1");
                if (!(signature || (server && (encipher || agreement)))) {
                    throw new Rejected("invalid_purpose");
                }
            }
        } else {
            List<Object> basic = constraints(facts);
            if (!basic.get(0).equals("1")) {
                throw new Rejected("invalid_chain");
            }
            List<Object> usage = usages(facts);
            if (!usage.isEmpty() && !usage.contains(BigInteger.valueOf(5))) {
                throw new Rejected("invalid_chain");
            }
        }
    }

    /** Причина без пути: подпись найденного по имени издателя или недоверие. */
    private static String reason(X509Certificate leaf, List<X509Certificate> pool, Set<X509Certificate> anchors) {
        X509Certificate current = leaf;
        for (int depth = 0; depth < 32; depth++) {
            boolean named = false;
            X509Certificate next = null;
            for (X509Certificate candidate : pool) {
                if (candidate == current || !candidate.getSubjectX500Principal().equals(current.getIssuerX500Principal())) {
                    continue;
                }
                named = true;
                try {
                    current.verify(candidate.getPublicKey());
                    next = candidate;
                    break;
                } catch (Exception error) {
                    // другой кандидат с тем же именем
                }
            }
            if (next == null) {
                return named ? "invalid_signature" : "untrusted";
            }
            if (anchors.contains(next)) {
                return "invalid_chain";
            }
            current = next;
        }
        return "invalid_chain";
    }

    public static List<Object> runtime_x509__verify_chain(List<?> chain, List<?> roots, String kind, String host,
            String time, String purposeOid) {
        if (chain.size() > 32 || roots.size() > 1024) {
            throw verifyFailure("resource_exhausted", "verification certificate count limit");
        }
        long total = 0;
        for (Object[] group : new Object[][] {{chain, 8388608L}, {roots, 16777216L}}) {
            long bytes = 0;
            for (Object der : (List<?>) group[0]) {
                int size = ((List<?>) der).size();
                total += size;
                bytes += size;
                if (size > 1048576 || total > 25165824 || bytes > (Long) group[1]) {
                    throw verifyFailure("resource_exhausted", "verification DER byte limit");
                }
            }
        }
        if (chain.isEmpty() || time.length() > 1024 || !time.matches("-?[0-9]+(?:\\.[0-9]+|/[1-9][0-9]*)?")
                || host.isEmpty() || host.length() > 253 || !host.matches("[A-Za-z0-9.:-]*")
                || !(kind.equals("DNS") || kind.equals("IP"))) {
            throw verifyFailure("invalid_input", "invalid verification inputs");
        }
        if (!purposeOid.equals("1.3.6.1.5.5.7.3.1") && !purposeOid.equals("1.3.6.1.5.5.7.3.2")) {
            throw verifyFailure("unsupported", "unsupported certificate purpose");
        }
        byte[] address = null;
        if (kind.equals("DNS")) {
            try {
                host = IdnaNatives.hostnameAscii(host);
            } catch (RuntimeException error) {
                throw verifyFailure("invalid_input", "invalid DNS reference identity");
            }
            if (host.endsWith(".")) {
                host = host.substring(0, host.length() - 1);
            }
        } else {
            address = host.indexOf(':') >= 0 ? NetNatives.parseIpv6(host) : NetNatives.parseIpv4(host);
            if (address == null) {
                throw verifyFailure("invalid_data", "invalid certificate verification data");
            }
        }
        Rat at = rational(time);
        try {
            CertificateFactory factory = CertificateFactory.getInstance("X.509");
            IdentityHashMap<X509Certificate, byte[]> sources = new IdentityHashMap<X509Certificate, byte[]>();
            IdentityHashMap<X509Certificate, Facts> snapshots = new IdentityHashMap<X509Certificate, Facts>();
            X509Certificate leaf = null;
            ArrayList<X509Certificate> intermediates = new ArrayList<X509Certificate>();
            ArrayList<X509Certificate> anchors = new ArrayList<X509Certificate>();
            HashSet<String> seen = new HashSet<String>();
            for (int group = 0; group < 2; group++) {
                List<?> items = group == 0 ? chain : roots;
                for (int index = 0; index < items.size(); index++) {
                    byte[] der = Natives.toBytes((List<?>) items.get(index));
                    Facts facts = certificate(der);
                    String failed = timeFailure(facts, at);
                    if (failed != null) {
                        if (group == 0 && index == 0) {
                            throw new Rejected(failed);
                        }
                        continue;
                    }
                    X509Certificate certificate;
                    try {
                        certificate = (X509Certificate) factory.generateCertificate(new ByteArrayInputStream(der));
                    } catch (java.security.cert.CertificateException error) {
                        throw bad("certificate");
                    }
                    sources.put(certificate, der);
                    snapshots.put(certificate, facts);
                    if (group == 1) {
                        // OpenSSL без PARTIAL_CHAIN: якорь — самоподписанный
                        // корень; прочие корни лишь достраивают путь.
                        if (seen.add(hex(der))) {
                            if (selfSigned(certificate)) {
                                anchors.add(certificate);
                            } else {
                                intermediates.add(certificate);
                            }
                        }
                    } else if (index == 0) {
                        if (kind.equals("DNS") ? !dnsMatches(facts, host) : !ipMatches(facts, address)) {
                            throw new Rejected("invalid_name");
                        }
                        if (critical(facts)) {
                            throw new Rejected("unknown_critical_extension");
                        }
                        leaf = certificate;
                    } else {
                        intermediates.add(certificate);
                    }
                }
            }
            if (anchors.isEmpty()) {
                throw new Rejected("untrusted");
            }
            HashSet<TrustAnchor> trust = new HashSet<TrustAnchor>();
            for (X509Certificate anchor : anchors) {
                trust.add(new TrustAnchor(anchor, null));
            }
            List<X509Certificate> path = new ArrayList<X509Certificate>();
            if (anchors.contains(leaf) || containsEncoding(anchors, leaf)) {
                path.add(leaf);
            } else {
                X509CertSelector target = new X509CertSelector();
                target.setCertificate(leaf);
                PKIXBuilderParameters parameters = new PKIXBuilderParameters(trust, target);
                parameters.setRevocationEnabled(false);
                parameters.setMaxPathLength(30);
                java.math.BigDecimal millis = new java.math.BigDecimal(Rat.numerator(at))
                    .divide(new java.math.BigDecimal(Rat.denominator(at)), 3, java.math.RoundingMode.FLOOR)
                    .multiply(java.math.BigDecimal.valueOf(1000));
                parameters.setDate(new java.util.Date(millis.longValue()));
                ArrayList<Certificate> pool = new ArrayList<Certificate>(intermediates);
                pool.add(leaf);
                parameters.addCertStore(CertStore.getInstance("Collection", new CollectionCertStoreParameters(pool)));
                PKIXCertPathBuilderResult result;
                try {
                    result = (PKIXCertPathBuilderResult) CertPathBuilder.getInstance("PKIX").build(parameters);
                } catch (CertPathBuilderException error) {
                    ArrayList<X509Certificate> all = new ArrayList<X509Certificate>(intermediates);
                    all.addAll(anchors);
                    throw new Rejected(reason(leaf, all, new HashSet<X509Certificate>(anchors)));
                }
                CertPath built = result.getCertPath();
                for (Certificate certificate : built.getCertificates()) {
                    path.add(original(sources, (X509Certificate) certificate));
                }
                path.add(original(sources, result.getTrustAnchor().getTrustedCert()));
            }
            if (path.size() > 32) {
                throw limit("verified path");
            }
            ArrayList<Object> out = new ArrayList<Object>();
            long pathBytes = 0;
            for (int index = 0; index < path.size(); index++) {
                X509Certificate certificate = path.get(index);
                byte[] der = sources.get(certificate);
                Facts facts = snapshots.get(certificate);
                if (der == null || facts == null) {
                    throw new IllegalStateException("verified source");
                }
                pathBytes += der.length;
                if (pathBytes > 8388608) {
                    throw limit("verified path bytes");
                }
                String failed = timeFailure(facts, at);
                if (failed != null) {
                    throw new Rejected(failed);
                }
                if (index < path.size() - 1) {
                    constraints(facts);
                    usages(facts);
                    purposes(facts);
                    san(facts);
                    if (critical(facts)) {
                        throw new Rejected("unknown_critical_extension");
                    }
                    strict(facts, certificate);
                    purpose(facts, purposeOid, index == 0);
                }
                out.add(Natives.fromBytes(der, der.length));
            }
            return out;
        } catch (Rejected rejected) {
            throw verifyFailure("permission_denied", rejected.getMessage());
        } catch (Bad error) {
            if (error.kind.equals("limit")) {
                throw verifyFailure("resource_exhausted", "verification limit");
            }
            throw verifyFailure("invalid_data", "invalid certificate verification data");
        } catch (Failure failure) {
            throw failure;
        } catch (Exception error) {
            throw verifyFailure("unexpected", "verification backend failure");
        }
    }

    private static boolean selfSigned(X509Certificate certificate) {
        if (!certificate.getSubjectX500Principal().equals(certificate.getIssuerX500Principal())) {
            return false;
        }
        try {
            certificate.verify(certificate.getPublicKey());
            return true;
        } catch (Exception error) {
            return false;
        }
    }

    private static boolean containsEncoding(List<X509Certificate> anchors, X509Certificate leaf) {
        try {
            for (X509Certificate anchor : anchors) {
                if (Arrays.equals(anchor.getEncoded(), leaf.getEncoded())) {
                    return true;
                }
            }
        } catch (java.security.cert.CertificateEncodingException error) {
            return false;
        }
        return false;
    }

    /** Экземпляр с исходным DER входа (PKIX может вернуть копию). */
    private static X509Certificate original(IdentityHashMap<X509Certificate, byte[]> sources, X509Certificate certificate)
            throws java.security.cert.CertificateEncodingException {
        if (sources.containsKey(certificate)) {
            return certificate;
        }
        byte[] encoded = certificate.getEncoded();
        for (Map.Entry<X509Certificate, byte[]> entry : sources.entrySet()) {
            if (Arrays.equals(entry.getValue(), encoded)) {
                return entry.getKey();
            }
        }
        return certificate;
    }
}
