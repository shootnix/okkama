package okkama.rt;

import java.math.BigInteger;
import java.util.ArrayList;

/**
 * runtime/xml (#144): the XML tokenizer of okkama-runtime-core/src/native/xml.rs.
 * It checks a whole document for well-formedness, with no DTD and bounded
 * depth and events, and keeps its events for std/xml/parse.
 */
public final class XmlNatives {
    private XmlNatives() {}

    private static final String DOMAIN = "core.xml";

    static final class Event {
        final int kind;
        String name = "";
        StringBuilder text;
        String data = "";
        final ArrayList<String[]> attributes = new ArrayList<>();

        Event(int kind) {
            this.kind = kind;
        }
    }

    static final class Events {
        final ArrayList<Event> events = new ArrayList<>();
        String error;
        long offset;
    }

    private static final class Bad extends RuntimeException {
        private static final long serialVersionUID = 1L;
        final int at;

        Bad(int at, String message) {
            super(message, null, false, false);
            this.at = at;
        }
    }

    static boolean allowed(int c) {
        return c == 0x9 || c == 0xA || c == 0xD || (c >= 0x20 && c <= 0xD7FF) || (c >= 0xE000 && c <= 0xFFFD)
            || (c >= 0x10000 && c <= 0x10FFFF);
    }

    static boolean nameStart(int c) {
        return Character.isLetter(c) || c == '_' || c == ':' || c > 0x7F;
    }

    static boolean nameChar(int c) {
        return nameStart(c) || (c >= '0' && c <= '9') || c == '-' || c == '.';
    }

    private static final class Parser {
        final String source;
        int position;
        final Events out = new Events();
        final ArrayList<String> open = new ArrayList<>();
        boolean rootClosed;
        boolean rootSeen;
        final int maximumDepth;
        final int maximumEvents;

        Parser(String source, int maximumDepth, int maximumEvents) {
            this.source = source;
            this.maximumDepth = maximumDepth;
            this.maximumEvents = maximumEvents;
        }

        Bad bad(String message) {
            return new Bad(position, message);
        }

        void push(Event event) {
            ArrayList<Event> events = out.events;
            if (event.kind == 2 && !events.isEmpty() && events.get(events.size() - 1).kind == 2) {
                events.get(events.size() - 1).text.append(event.text);
                return;
            }
            if (events.size() >= maximumEvents) {
                throw bad("more than " + maximumEvents + " XML events");
            }
            events.add(event);
        }

        boolean at(String prefix) {
            return source.startsWith(prefix, position);
        }

        void skipSpace() {
            while (position < source.length()) {
                char c = source.charAt(position);
                if (c != ' ' && c != '\t' && c != '\n' && c != '\r') {
                    break;
                }
                position++;
            }
        }

        String name() {
            if (position >= source.length() || !nameStart(source.codePointAt(position))) {
                throw bad("expected a name");
            }
            int start = position;
            while (position < source.length() && nameChar(source.codePointAt(position))) {
                position += Character.charCount(source.codePointAt(position));
            }
            return source.substring(start, position);
        }

        String decoded(int start, int end, boolean attribute) {
            StringBuilder out = new StringBuilder(end - start);
            int i = start;
            while (i < end) {
                int c = source.codePointAt(i);
                if (!allowed(c)) {
                    throw new Bad(i, String.format("a character U+%04X XML does not allow", c));
                }
                if (c == '&') {
                    int semi = source.indexOf(';', i);
                    if (semi < 0 || semi >= end) {
                        throw new Bad(i, "an unterminated reference");
                    }
                    String reference = source.substring(i + 1, semi);
                    int value;
                    switch (reference) {
                        case "lt" -> value = '<';
                        case "gt" -> value = '>';
                        case "amp" -> value = '&';
                        case "quot" -> value = '"';
                        case "apos" -> value = '\'';
                        default -> {
                            value = -1;
                            try {
                                if (reference.startsWith("#x")) {
                                    value = Integer.parseUnsignedInt(reference.substring(2), 16);
                                } else if (reference.startsWith("#")) {
                                    value = Integer.parseUnsignedInt(reference.substring(1));
                                } else {
                                    throw new Bad(i, "an unknown entity &" + reference + ";");
                                }
                            } catch (NumberFormatException e) {
                                value = -1;
                            }
                            if (value < 0 || !allowed(value) || (value >= 0xD800 && value <= 0xDFFF)) {
                                throw new Bad(i, "a bad character reference &" + reference + ";");
                            }
                        }
                    }
                    out.appendCodePoint(value);
                    i = semi + 1;
                    continue;
                }
                if (attribute && c == '<') {
                    throw new Bad(i, "< inside an attribute value");
                }
                out.appendCodePoint(attribute && (c == '\t' || c == '\n' || c == '\r') ? ' ' : c);
                i += Character.charCount(c);
            }
            return out.toString();
        }

        void run() {
            if (source.startsWith("﻿")) {
                position = 1;
            }
            int start = position;
            while (position < source.length()) {
                if (at("<?")) {
                    int end = source.indexOf("?>", position + 2);
                    if (end < 0) {
                        throw bad("an unterminated processing instruction");
                    }
                    position += 2;
                    String target = name();
                    String data = source.substring(position, end).replaceFirst("^[ \t\n\r]+", "");
                    if (target.equalsIgnoreCase("xml")) {
                        if (position - 5 != start) {
                            throw bad("an XML declaration that is not at the start");
                        }
                    } else if (!open.isEmpty()) {
                        Event event = new Event(4);
                        event.name = target;
                        event.data = data;
                        push(event);
                    }
                    position = end + 2;
                } else if (at("<!--")) {
                    int end = source.indexOf("-->", position + 4);
                    if (end < 0) {
                        throw bad("an unterminated comment");
                    }
                    String body = source.substring(position + 4, end);
                    if (body.contains("--")) {
                        throw bad("-- inside a comment");
                    }
                    if (!open.isEmpty()) {
                        Event event = new Event(3);
                        event.data = body;
                        push(event);
                    }
                    position = end + 3;
                } else if (at("<![CDATA[")) {
                    if (open.isEmpty()) {
                        throw bad("a CDATA section outside the root element");
                    }
                    int end = source.indexOf("]]>", position + 9);
                    if (end < 0) {
                        throw bad("an unterminated CDATA section");
                    }
                    String body = source.substring(position + 9, end);
                    body.codePoints().filter(c -> !allowed(c)).findFirst().ifPresent(c -> {
                        throw bad(String.format("a character U+%04X XML does not allow", c));
                    });
                    Event event = new Event(2);
                    event.text = new StringBuilder(body);
                    push(event);
                    position = end + 3;
                } else if (at("<!DOCTYPE")) {
                    if (rootSeen) {
                        throw bad("a DOCTYPE after the root element");
                    }
                    int quote = 0;
                    int end = -1;
                    for (int i = position; i < source.length(); i++) {
                        char c = source.charAt(i);
                        if (quote != 0) {
                            if (c == quote) {
                                quote = 0;
                            }
                        } else if (c == '"' || c == '\'') {
                            quote = c;
                        } else if (c == '[') {
                            throw new Bad(i, "a DTD internal subset is not supported");
                        } else if (c == '>') {
                            end = i;
                            break;
                        }
                    }
                    if (end < 0) {
                        throw bad("an unterminated DOCTYPE");
                    }
                    position = end + 1;
                } else if (at("</")) {
                    position += 2;
                    int namedAt = position;
                    String name = name();
                    skipSpace();
                    if (!at(">")) {
                        throw bad("expected > to close the end tag");
                    }
                    position++;
                    if (open.isEmpty()) {
                        throw new Bad(namedAt, "</" + name + "> closes nothing");
                    }
                    String top = open.remove(open.size() - 1);
                    if (!top.equals(name)) {
                        throw new Bad(namedAt, "</" + name + "> closes <" + top + ">");
                    }
                    Event event = new Event(1);
                    event.name = name;
                    push(event);
                    if (open.isEmpty()) {
                        rootClosed = true;
                    }
                } else if (at("<")) {
                    if (rootClosed) {
                        throw bad("a second root element");
                    }
                    position++;
                    String name = name();
                    Event event = new Event(0);
                    event.name = name;
                    while (true) {
                        int before = position;
                        skipSpace();
                        if (at("/>") || at(">")) {
                            break;
                        }
                        if (position == before) {
                            throw bad("expected a space before an attribute");
                        }
                        int namedAt = position;
                        String attribute = name();
                        skipSpace();
                        if (!at("=")) {
                            throw bad("expected = after " + attribute);
                        }
                        position++;
                        skipSpace();
                        if (position >= source.length() || (source.charAt(position) != '"' && source.charAt(position) != '\'')) {
                            throw bad("expected a quoted attribute value");
                        }
                        char quote = source.charAt(position);
                        position++;
                        int end = source.indexOf(quote, position);
                        if (end < 0) {
                            throw bad("an unterminated attribute value");
                        }
                        String value = decoded(position, end, true);
                        for (String[] existing : event.attributes) {
                            if (existing[0].equals(attribute)) {
                                throw new Bad(namedAt, "the attribute " + attribute + " given twice");
                            }
                        }
                        event.attributes.add(new String[] {attribute, value});
                        position = end + 1;
                    }
                    boolean closed = at("/>");
                    position += closed ? 2 : 1;
                    if (open.size() >= maximumDepth) {
                        throw bad("elements nested deeper than " + maximumDepth);
                    }
                    rootSeen = true;
                    push(event);
                    if (closed) {
                        Event end = new Event(1);
                        end.name = name;
                        push(end);
                        if (open.isEmpty()) {
                            rootClosed = true;
                        }
                    } else {
                        open.add(name);
                    }
                } else {
                    int end = source.indexOf('<', position);
                    if (end < 0) {
                        end = source.length();
                    }
                    if (open.isEmpty()) {
                        for (int i = position; i < end; i++) {
                            char c = source.charAt(i);
                            if (c != ' ' && c != '\t' && c != '\n' && c != '\r') {
                                position = i;
                                throw bad("text outside the root element");
                            }
                        }
                    } else {
                        if (source.substring(position, end).contains("]]>")) {
                            throw bad("]]> in text");
                        }
                        Event event = new Event(2);
                        event.text = new StringBuilder(decoded(position, end, false));
                        push(event);
                    }
                    position = end;
                }
            }
            if (!open.isEmpty()) {
                throw bad("<" + open.get(open.size() - 1) + "> is not closed");
            }
            if (!rootSeen) {
                throw bad("no root element");
            }
        }
    }

    private static Events state(Object handle) {
        return (Events) handle;
    }

    private static Event event(Object handle, BigInteger index, String name, int arity) {
        Events events = state(handle);
        if (index.signum() < 0 || index.compareTo(BigInteger.valueOf(events.events.size())) >= 0) {
            throw NativeFailure.raise(DOMAIN, "invalid_input", "runtime/xml@1::" + name + "/" + arity, "XML event index out of range");
        }
        return events.events.get(index.intValue());
    }

    private static int positive(BigInteger value, String what) {
        if (value.signum() <= 0 || value.bitLength() > 31) {
            throw NativeFailure.raise(DOMAIN, "invalid_input", "runtime/xml@1::xml_events/3", what + " must be positive");
        }
        return value.intValue();
    }

    public static Object runtime_xml__xml_events(String origin, BigInteger maximumDepth, BigInteger maximumEvents) {
        Parser parser = new Parser(origin, positive(maximumDepth, "maximum depth"), positive(maximumEvents, "maximum events"));
        try {
            parser.run();
            return parser.out;
        } catch (Bad bad) {
            Events failed = new Events();
            failed.error = bad.getMessage();
            int at = Math.min(bad.at, origin.length());
            failed.offset = origin.codePointCount(0, at);
            return failed;
        }
    }

    public static String runtime_xml__xml_events_error(Object handle) {
        String error = state(handle).error;
        return error == null ? "" : error;
    }

    public static BigInteger runtime_xml__xml_events_offset(Object handle) {
        return BigInteger.valueOf(state(handle).error == null ? 0 : state(handle).offset);
    }

    public static BigInteger runtime_xml__xml_event_count(Object handle) {
        return BigInteger.valueOf(state(handle).events.size());
    }

    public static BigInteger runtime_xml__xml_event_kind(Object handle, BigInteger index) {
        return BigInteger.valueOf(event(handle, index, "xml_event_kind", 2).kind);
    }

    public static String runtime_xml__xml_event_name(Object handle, BigInteger index) {
        return event(handle, index, "xml_event_name", 2).name;
    }

    public static String runtime_xml__xml_event_text(Object handle, BigInteger index) {
        Event event = event(handle, index, "xml_event_text", 2);
        return event.kind == 2 ? event.text.toString() : event.data;
    }

    public static BigInteger runtime_xml__xml_attribute_count(Object handle, BigInteger index) {
        return BigInteger.valueOf(event(handle, index, "xml_attribute_count", 2).attributes.size());
    }

    public static String runtime_xml__xml_attribute_name(Object handle, BigInteger index, BigInteger attribute) {
        return attribute(handle, index, attribute, "xml_attribute_name")[0];
    }

    public static String runtime_xml__xml_attribute_value(Object handle, BigInteger index, BigInteger attribute) {
        return attribute(handle, index, attribute, "xml_attribute_value")[1];
    }

    private static String[] attribute(Object handle, BigInteger index, BigInteger attribute, String name) {
        Event event = event(handle, index, name, 3);
        if (attribute.signum() < 0 || attribute.compareTo(BigInteger.valueOf(event.attributes.size())) >= 0) {
            throw NativeFailure.raise(DOMAIN, "invalid_input", "runtime/xml@1::" + name + "/3", "no such attribute");
        }
        return event.attributes.get(attribute.intValue());
    }
}
