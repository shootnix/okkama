package okkama.rt;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * Natives {@code runtime/regex}: собственный движок регулярных выражений
 * Okkama (порт {@code Okkama::Runtime::Regex::Compiler}) — разбор шаблона,
 * NFA Томпсона и поиск самого левого, затем самого длинного совпадения с
 * захватами. Смещения — в символах (code points).
 */
public final class RegexNatives {
    private RegexNatives() {}

    private static final String DOMAIN = "core.regex";
    private static final int MAX_PATTERN = 4096;
    private static final int MAX_STATES = 16384;
    private static final int MAX_REPETITION = 4096;
    private static final int MAX_NESTING = 256;
    private static final int MAX_GROUPS = 1024;
    private static final int MAX_INPUT = 1048576;

    private static Failure fail(String code, String name, int arity, String message) {
        return NativeFailure.raise(DOMAIN, code, "runtime/regex@1::" + name + "/" + arity, message);
    }

    // ---- ошибки компиляции ----

    private static final class CompileError extends RuntimeException {
        private static final long serialVersionUID = 1L;
        final int offset;

        CompileError(int offset) {
            super(null, null, false, false);
            this.offset = offset;
        }
    }

    private static final class LimitError extends RuntimeException {
        private static final long serialVersionUID = 1L;
        final int kind;
        final int limit;

        LimitError(int kind, int limit) {
            super(null, null, false, false);
            this.kind = kind;
            this.limit = limit;
        }
    }

    /** Ошибка исполнения движка: текст {@code die} Perl-движка. */
    private static final class EngineError extends RuntimeException {
        private static final long serialVersionUID = 1L;
        final String text;

        EngineError(String text) {
            super(text, null, false, false);
            this.text = text;
        }
    }

    // ---- matchers и инструкции ----

    private static final int RANGES = 0;
    private static final int CLASS = 1;
    private static final int DOT = 2;
    private static final int SHORTHAND = 3;

    static final class Matcher {
        int kind;
        boolean negate;
        int[][] ranges;
        List<Matcher> members;
        char code;
    }

    private static Matcher literal(int value) {
        Matcher matcher = new Matcher();
        matcher.kind = RANGES;
        matcher.ranges = new int[][] {{value, value}};
        return matcher;
    }

    private static final int JUMP = 0;
    private static final int SAVE = 1;
    private static final int SPLIT = 2;
    private static final int ASSERT_START = 3;
    private static final int ASSERT_END = 4;
    private static final int CONSUME = 5;
    private static final int MATCH = 6;

    static final class Instruction {
        int op;
        int out = -1;
        int out1 = -1;
        int slot;
        Matcher matcher;
    }

    // ---- AST ----

    private static final int EMPTY = 0;
    private static final int N_CONSUME = 1;
    private static final int ANCHOR = 2;
    private static final int CAPTURE = 3;
    private static final int CONCAT = 4;
    private static final int ALTERNATE = 5;
    private static final int REPEAT = 6;

    static final class Node {
        int kind;
        Matcher matcher;
        int anchor;
        int index;
        Node child;
        Node left;
        Node right;
        List<Node> children;
        int minimum;
        int maximum = -1;
    }

    private static Node node(int kind) {
        Node node = new Node();
        node.kind = kind;
        return node;
    }

    // ---- разбор ----

    private static final String META = "\\.^$|()[]{}?*+";

    private static final class Parser {
        final int[] source;
        int index;
        int captureCount;
        final Map<String, Integer> groupNames = new HashMap<String, Integer>();
        int nesting;
        final boolean verbose;

        Parser(String pattern, boolean verbose) {
            this.source = pattern.codePoints().toArray();
            this.verbose = verbose;
        }

        CompileError invalid() {
            return new CompileError(index);
        }

        boolean atEnd() {
            return index >= source.length;
        }

        int peek() {
            return atEnd() ? -1 : source[index];
        }

        int advance() {
            if (atEnd()) {
                throw invalid();
            }
            return source[index++];
        }

        boolean match(int expected) {
            if (atEnd() || peek() != expected) {
                return false;
            }
            index++;
            return true;
        }

        void skipIgnored() {
            if (!verbose) {
                return;
            }
            while (!atEnd()) {
                int next = peek();
                if (next == ' ' || next == '\t' || next == '\n' || next == '\r' || next == 12 || next == 11) {
                    index++;
                    continue;
                }
                if (next == '#') {
                    index++;
                    while (!atEnd() && peek() != '\n') {
                        index++;
                    }
                    continue;
                }
                break;
            }
        }

        static int hex(int point) {
            if (point >= '0' && point <= '9') {
                return point - '0';
            }
            if (point >= 'a' && point <= 'f') {
                return point - 'a' + 10;
            }
            if (point >= 'A' && point <= 'F') {
                return point - 'A' + 10;
            }
            return -1;
        }

        static Matcher shorthand(int point) {
            int lower = point;
            boolean negate = false;
            if (point == 'D' || point == 'W' || point == 'S') {
                lower += 'a' - 'A';
                negate = true;
            }
            if (lower != 'd' && lower != 'w' && lower != 's') {
                return null;
            }
            Matcher matcher = new Matcher();
            matcher.kind = SHORTHAND;
            matcher.code = (char) lower;
            matcher.negate = negate;
            return matcher;
        }

        /** Результат экранирования: matcher и признак сокращения класса. */
        Object[] escaped(boolean allowShorthand) {
            if (!match('\\')) {
                throw invalid();
            }
            int escaped = advance();
            if (escaped == 'n') {
                return new Object[] {literal('\n'), false};
            }
            if (escaped == 'r') {
                return new Object[] {literal('\r'), false};
            }
            if (escaped == 't') {
                return new Object[] {literal('\t'), false};
            }
            if (escaped == 'x') {
                if (!match('{')) {
                    throw invalid();
                }
                int valueOffset = index;
                int value = 0;
                int digits = 0;
                while (!atEnd() && peek() != '}') {
                    int digitOffset = index;
                    int digit = hex(advance());
                    if (digit < 0) {
                        throw new CompileError(digitOffset);
                    }
                    value = value * 16 + digit;
                    digits++;
                    if (value > 0x10ffff) {
                        throw new CompileError(digitOffset);
                    }
                }
                if (digits == 0 || !match('}')) {
                    throw invalid();
                }
                if (value >= 0xd800 && value <= 0xdfff) {
                    throw new CompileError(valueOffset);
                }
                return new Object[] {literal(value), false};
            }
            if (allowShorthand) {
                Matcher matcher = shorthand(escaped);
                if (matcher != null) {
                    return new Object[] {matcher, true};
                }
            }
            if (!(escaped < 128 && (META.indexOf(escaped) >= 0 || escaped == '-' || escaped == ' ' || escaped == '#'))) {
                throw new CompileError(index - 1);
            }
            return new Object[] {literal(escaped), false};
        }

        int number() {
            int value = 0;
            int digits = 0;
            while (!atEnd()) {
                int point = peek();
                if (point < '0' || point > '9') {
                    break;
                }
                advance();
                value = value * 10 + point - '0';
                digits++;
                if (value > MAX_REPETITION) {
                    throw new CompileError(index - 1);
                }
            }
            if (digits == 0) {
                throw invalid();
            }
            return value;
        }

        Object[] classItem() {
            if (peek() == '\\') {
                return escaped(true);
            }
            int value = advance();
            if (value == ']') {
                throw invalid();
            }
            return new Object[] {literal(value), false};
        }

        Matcher characterClass() {
            if (!match('[')) {
                throw invalid();
            }
            boolean negate = match('^');
            ArrayList<Matcher> members = new ArrayList<Matcher>();
            int items = 0;
            while (!atEnd() && peek() != ']') {
                Object[] first = classItem();
                items++;
                if (!(Boolean) first[1] && peek() == '-' && index + 1 < source.length && source[index + 1] != ']') {
                    advance();
                    Object[] last = classItem();
                    if ((Boolean) last[1]) {
                        throw invalid();
                    }
                    int from = ((Matcher) first[0]).ranges[0][0];
                    int to = ((Matcher) last[0]).ranges[0][0];
                    if (from > to) {
                        throw new CompileError(index - 1);
                    }
                    Matcher range = new Matcher();
                    range.kind = RANGES;
                    range.ranges = new int[][] {{from, to}};
                    members.add(range);
                    continue;
                }
                members.add((Matcher) first[0]);
            }
            if (items == 0 || !match(']')) {
                throw invalid();
            }
            Matcher matcher = new Matcher();
            matcher.kind = CLASS;
            matcher.negate = negate;
            matcher.members = members;
            return matcher;
        }

        Node primary() {
            skipIgnored();
            int next = peek();
            if (next < 0) {
                throw invalid();
            }
            if (next == '(') {
                advance();
                boolean capturing = true;
                String name = null;
                if (match('?')) {
                    if (match(':')) {
                        capturing = false;
                    } else if (match('<')) {
                        int start = index;
                        StringBuilder text = new StringBuilder();
                        while (!atEnd() && peek() != '>') {
                            text.appendCodePoint(advance());
                        }
                        if (text.length() == 0 || !match('>')) {
                            throw new CompileError(start);
                        }
                        name = text.toString();
                        if (!name.matches("[A-Za-z_][A-Za-z0-9_]*") || groupNames.containsKey(name)) {
                            throw new CompileError(start);
                        }
                    } else {
                        throw invalid();
                    }
                }
                if (capturing && captureCount >= MAX_GROUPS) {
                    throw new LimitError(2, MAX_GROUPS);
                }
                int captureIndex = capturing ? ++captureCount : -1;
                if (name != null) {
                    groupNames.put(name, captureIndex);
                }
                if (++nesting > MAX_NESTING) {
                    throw new LimitError(1, MAX_NESTING);
                }
                Node inner = alternation();
                nesting--;
                if (!match(')')) {
                    throw invalid();
                }
                if (!capturing) {
                    return inner;
                }
                Node capture = node(CAPTURE);
                capture.index = captureIndex;
                capture.child = inner;
                return capture;
            }
            if (next == '[') {
                Node consume = node(N_CONSUME);
                consume.matcher = characterClass();
                return consume;
            }
            if (next == '\\') {
                Node consume = node(N_CONSUME);
                consume.matcher = (Matcher) escaped(true)[0];
                return consume;
            }
            if (next == '.') {
                advance();
                Node consume = node(N_CONSUME);
                consume.matcher = new Matcher();
                consume.matcher.kind = DOT;
                return consume;
            }
            if (next == '^' || next == '$') {
                advance();
                Node anchor = node(ANCHOR);
                anchor.anchor = next == '^' ? ASSERT_START : ASSERT_END;
                return anchor;
            }
            if (next < 128 && META.indexOf(next) >= 0) {
                throw invalid();
            }
            advance();
            Node consume = node(N_CONSUME);
            consume.matcher = literal(next);
            return consume;
        }

        Node repetition() {
            Node inner = primary();
            skipIgnored();
            if (atEnd()) {
                return inner;
            }
            int minimum;
            int maximum;
            if (match('?')) {
                minimum = 0;
                maximum = 1;
            } else if (match('*')) {
                minimum = 0;
                maximum = -1;
            } else if (match('+')) {
                minimum = 1;
                maximum = -1;
            } else if (match('{')) {
                minimum = number();
                if (match('}')) {
                    maximum = minimum;
                } else {
                    if (!match(',')) {
                        throw invalid();
                    }
                    int maximumOffset = -1;
                    if (peek() == '}') {
                        maximum = -1;
                    } else {
                        maximumOffset = index;
                        maximum = number();
                    }
                    if (!match('}')) {
                        throw invalid();
                    }
                    if (maximum >= 0 && maximum < minimum) {
                        throw new CompileError(maximumOffset);
                    }
                }
            } else {
                return inner;
            }
            if (!atEnd()) {
                int next = peek();
                if (next == '?' || next == '*' || next == '+' || next == '{') {
                    throw invalid();
                }
            }
            Node repeat = node(REPEAT);
            repeat.child = inner;
            repeat.minimum = minimum;
            repeat.maximum = maximum;
            return repeat;
        }

        Node concatenation() {
            ArrayList<Node> children = new ArrayList<Node>();
            while (!atEnd()) {
                skipIgnored();
                if (atEnd()) {
                    break;
                }
                int next = peek();
                if (next == '|' || next == ')') {
                    break;
                }
                children.add(repetition());
            }
            if (children.isEmpty()) {
                return node(EMPTY);
            }
            if (children.size() == 1) {
                return children.get(0);
            }
            Node concat = node(CONCAT);
            concat.children = children;
            return concat;
        }

        Node alternation() {
            Node result = concatenation();
            skipIgnored();
            while (match('|')) {
                Node alternate = node(ALTERNATE);
                alternate.left = result;
                alternate.right = concatenation();
                result = alternate;
                skipIgnored();
            }
            return result;
        }

        Node parse() {
            skipIgnored();
            Node result = alternation();
            skipIgnored();
            if (!atEnd()) {
                throw invalid();
            }
            return result;
        }
    }

    // ---- компиляция в NFA ----

    private static final class Fragment {
        final int start;
        /** Незаполненные выходы: индекс инструкции * 2 + (0 — out, 1 — out1). */
        final List<Integer> outs;

        Fragment(int start, List<Integer> outs) {
            this.start = start;
            this.outs = outs;
        }
    }

    private static int emit(List<Instruction> instructions, Instruction instruction) {
        if (instructions.size() >= MAX_STATES) {
            throw new LimitError(3, MAX_STATES);
        }
        instructions.add(instruction);
        return instructions.size() - 1;
    }

    private static Instruction instruction(int op) {
        Instruction instruction = new Instruction();
        instruction.op = op;
        return instruction;
    }

    private static void patch(List<Instruction> instructions, List<Integer> outs, int target) {
        for (int out : outs) {
            Instruction instruction = instructions.get(out >> 1);
            if ((out & 1) == 0) {
                instruction.out = target;
            } else {
                instruction.out1 = target;
            }
        }
    }

    private static List<Integer> outs(int index, int field) {
        ArrayList<Integer> outs = new ArrayList<Integer>();
        outs.add(index * 2 + field);
        return outs;
    }

    private static Fragment single(List<Instruction> instructions, Instruction instruction) {
        int index = emit(instructions, instruction);
        return new Fragment(index, outs(index, 0));
    }

    private static Fragment concat(List<Instruction> instructions, Fragment left, Fragment right) {
        patch(instructions, left.outs, right.start);
        return new Fragment(left.start, right.outs);
    }

    private static Fragment compile(Node node, List<Instruction> instructions) {
        switch (node.kind) {
            case EMPTY:
                return single(instructions, instruction(JUMP));
            case N_CONSUME: {
                Instruction consume = instruction(CONSUME);
                consume.matcher = node.matcher;
                return single(instructions, consume);
            }
            case ANCHOR:
                return single(instructions, instruction(node.anchor));
            case CAPTURE: {
                Fragment fragment = compile(node.child, instructions);
                Instruction open = instruction(SAVE);
                open.slot = (node.index - 1) * 2;
                open.out = fragment.start;
                int start = emit(instructions, open);
                Instruction close = instruction(SAVE);
                close.slot = (node.index - 1) * 2 + 1;
                int end = emit(instructions, close);
                patch(instructions, fragment.outs, end);
                return new Fragment(start, outs(end, 0));
            }
            case CONCAT: {
                Fragment fragment = compile(node.children.get(0), instructions);
                for (int index = 1; index < node.children.size(); index++) {
                    fragment = concat(instructions, fragment, compile(node.children.get(index), instructions));
                }
                return fragment;
            }
            case ALTERNATE: {
                Fragment left = compile(node.left, instructions);
                Fragment right = compile(node.right, instructions);
                Instruction split = instruction(SPLIT);
                split.out = left.start;
                split.out1 = right.start;
                int index = emit(instructions, split);
                ArrayList<Integer> outs = new ArrayList<Integer>(left.outs);
                outs.addAll(right.outs);
                return new Fragment(index, outs);
            }
            case REPEAT: {
                Fragment fragment = null;
                for (int count = 0; count < node.minimum; count++) {
                    Fragment copy = compile(node.child, instructions);
                    fragment = fragment == null ? copy : concat(instructions, fragment, copy);
                }
                if (node.maximum < 0) {
                    Fragment body = compile(node.child, instructions);
                    Instruction split = instruction(SPLIT);
                    split.out = body.start;
                    int index = emit(instructions, split);
                    patch(instructions, body.outs, index);
                    Fragment star = new Fragment(index, outs(index, 1));
                    return fragment == null ? star : concat(instructions, fragment, star);
                }
                for (int count = 0; count < node.maximum - node.minimum; count++) {
                    Fragment body = compile(node.child, instructions);
                    Instruction split = instruction(SPLIT);
                    split.out = body.start;
                    int index = emit(instructions, split);
                    ArrayList<Integer> outs = new ArrayList<Integer>(body.outs);
                    outs.add(index * 2 + 1);
                    Fragment optional = new Fragment(index, outs);
                    fragment = fragment == null ? optional : concat(instructions, fragment, optional);
                }
                return fragment == null ? single(instructions, instruction(JUMP)) : fragment;
            }
            default:
                throw new CompileError(0);
        }
    }

    // ---- программа ----

    static final class Program {
        Instruction[] instructions;
        int start;
        int captureCount;
        Map<String, Integer> groupNames;
        boolean ascii;
        boolean ignoreCase;
        boolean multiline;
        boolean dotAll;

        boolean matchesCharacter(Matcher matcher, int point) {
            switch (matcher.kind) {
                case DOT:
                    return dotAll || point != '\n';
                case CLASS: {
                    boolean inside = false;
                    for (Matcher member : matcher.members) {
                        if (matchesCharacter(member, point)) {
                            inside = true;
                            break;
                        }
                    }
                    return matcher.negate != inside;
                }
                case SHORTHAND: {
                    boolean inside;
                    if (matcher.code == 'd') {
                        inside = ascii ? point >= '0' && point <= '9'
                            : Character.getType(point) == Character.DECIMAL_DIGIT_NUMBER;
                    } else if (matcher.code == 'w') {
                        if (ascii) {
                            inside = (point >= 'A' && point <= 'Z') || (point >= 'a' && point <= 'z')
                                || (point >= '0' && point <= '9') || point == '_';
                        } else {
                            inside = Character.isLetter(point) || isNumber(point) || point == '_';
                        }
                    } else {
                        inside = (point >= 9 && point <= 13) || point == 32 || (!ascii && (point == 0x85
                            || point == 0xa0 || point == 0x1680 || (point >= 0x2000 && point <= 0x200a)
                            || point == 0x2028 || point == 0x2029 || point == 0x202f || point == 0x205f
                            || point == 0x3000));
                    }
                    return matcher.negate != inside;
                }
                default: {
                    int[] candidates = candidates(point);
                    boolean inside = false;
                    outer:
                    for (int[] range : matcher.ranges) {
                        for (int candidate : candidates) {
                            if (candidate >= range[0] && candidate <= range[1]) {
                                inside = true;
                                break outer;
                            }
                        }
                    }
                    return matcher.negate != inside;
                }
            }
        }

        private static boolean isNumber(int point) {
            int type = Character.getType(point);
            return type == Character.DECIMAL_DIGIT_NUMBER || type == Character.LETTER_NUMBER
                || type == Character.OTHER_NUMBER;
        }

        private int[] candidates(int point) {
            if (!ignoreCase) {
                return new int[] {point};
            }
            if (ascii) {
                if (point >= 'A' && point <= 'Z') {
                    return new int[] {point, point + 32};
                }
                if (point >= 'a' && point <= 'z') {
                    return new int[] {point, point - 32};
                }
                return new int[] {point};
            }
            String text = new String(Character.toChars(point));
            String[] variants = {
                text.toLowerCase(Locale.ROOT),
                text.toUpperCase(Locale.ROOT),
                text.toUpperCase(Locale.ROOT).toLowerCase(Locale.ROOT),
            };
            int[] out = new int[4];
            int count = 0;
            out[count++] = point;
            for (String variant : variants) {
                if (variant.codePointCount(0, variant.length()) == 1) {
                    out[count++] = variant.codePointAt(0);
                }
            }
            int[] trimmed = new int[count];
            System.arraycopy(out, 0, trimmed, 0, count);
            return trimmed;
        }

        // Состояние с тегами: инструкция, захваты, начало совпадения.
        static final class Tagged {
            final int index;
            final int[] captures;
            final int matchStart;

            Tagged(int index, int[] captures, int matchStart) {
                this.index = index;
                this.captures = captures;
                this.matchStart = matchStart;
            }
        }

        /** Замыкание по ε-переходам в порядке приоритета (обход в глубину). */
        void addTagged(List<Tagged> states, boolean[] seen, int index, int[] captures, int matchStart,
                int position, int[] points) {
            ArrayList<Object[]> stack = new ArrayList<Object[]>();
            stack.add(new Object[] {index, captures});
            while (!stack.isEmpty()) {
                Object[] top = stack.remove(stack.size() - 1);
                int at = (Integer) top[0];
                int[] tags = (int[]) top[1];
                if (at < 0 || seen[at]) {
                    continue;
                }
                seen[at] = true;
                Instruction instruction = instructions[at];
                switch (instruction.op) {
                    case JUMP:
                        stack.add(new Object[] {instruction.out, tags});
                        break;
                    case SAVE: {
                        int[] updated = tags.clone();
                        updated[instruction.slot] = position;
                        stack.add(new Object[] {instruction.out, updated});
                        break;
                    }
                    case SPLIT:
                        stack.add(new Object[] {instruction.out1, tags});
                        stack.add(new Object[] {instruction.out, tags});
                        break;
                    case ASSERT_START:
                        if (position == 0 || (multiline && points[position - 1] == '\n')) {
                            stack.add(new Object[] {instruction.out, tags});
                        }
                        break;
                    case ASSERT_END:
                        if (position == points.length || (multiline && points[position] == '\n')) {
                            stack.add(new Object[] {instruction.out, tags});
                        }
                        break;
                    default:
                        states.add(new Tagged(at, tags, matchStart));
                }
            }
        }

        private void appendInitial(List<Tagged> states, boolean[] seen, int position, int[] points) {
            int[] captures = new int[captureCount * 2];
            java.util.Arrays.fill(captures, -1);
            addTagged(states, seen, start, captures, position, position, points);
        }

        private int[] bestAcceptance(List<Tagged> states, int position, int[] best) {
            for (Tagged state : states) {
                if (instructions[state.index].op != MATCH) {
                    continue;
                }
                if (best == null || state.matchStart < best[0] || (state.matchStart == best[0] && position > best[1])) {
                    int[] next = new int[2 + state.captures.length];
                    next[0] = state.matchStart;
                    next[1] = position;
                    System.arraycopy(state.captures, 0, next, 2, state.captures.length);
                    best = next;
                }
                break;
            }
            return best;
        }

        private boolean canExtend(List<Tagged> states, int matchStart) {
            for (Tagged state : states) {
                if (state.matchStart > matchStart) {
                    continue;
                }
                if (instructions[state.index].op == CONSUME) {
                    return true;
                }
            }
            return false;
        }

        /** [начало, конец, захваты…] или {@code null}. */
        int[] bestFrom(int[] points, int minimumStart) {
            int length = points.length;
            if (minimumStart > length) {
                return null;
            }
            List<Tagged> states = new ArrayList<Tagged>();
            boolean[] seen = new boolean[instructions.length];
            appendInitial(states, seen, minimumStart, points);
            int[] best = bestAcceptance(states, minimumStart, null);
            if (best != null && !canExtend(states, best[0])) {
                return best;
            }
            for (int index = minimumStart; index < length; index++) {
                List<Tagged> next = new ArrayList<Tagged>();
                boolean[] nextSeen = new boolean[instructions.length];
                for (Tagged state : states) {
                    Instruction instruction = instructions[state.index];
                    if (instruction.op != CONSUME || !matchesCharacter(instruction.matcher, points[index])) {
                        continue;
                    }
                    addTagged(next, nextSeen, instruction.out, state.captures, state.matchStart, index + 1, points);
                }
                appendInitial(next, nextSeen, index + 1, points);
                states = next;
                best = bestAcceptance(states, index + 1, best);
                if (best != null && !canExtend(states, best[0])) {
                    return best;
                }
            }
            return best;
        }

        /** Совпадение в [start, end) в режиме first/prefix/entire: спаны или пусто. */
        List<int[]> selectedSpans(String input, BigInteger startValue, BigInteger endValue, int mode) {
            int[] all = input.codePoints().toArray();
            if (all.length > MAX_INPUT) {
                throw new EngineError("Regex input resource limit exceeded\n");
            }
            if (startValue.signum() < 0 || endValue.compareTo(startValue) < 0
                    || endValue.compareTo(BigInteger.valueOf(all.length)) > 0) {
                throw new EngineError("Invalid regex search range\n");
            }
            int startOffset = startValue.intValue();
            int endOffset = endValue.intValue();
            int[] selected = new int[endOffset - startOffset];
            System.arraycopy(all, startOffset, selected, 0, selected.length);
            int[] best = bestFrom(selected, 0);
            ArrayList<int[]> spans = new ArrayList<int[]>();
            if (best == null || (mode == 1 && best[0] != 0)
                    || (mode == 2 && (best[0] != 0 || best[1] != selected.length))) {
                return spans;
            }
            spans.add(new int[] {best[0] + startOffset, best[1] + startOffset});
            for (int capture = 0; capture < captureCount; capture++) {
                int from = best[2 + capture * 2];
                int to = best[3 + capture * 2];
                if (from < 0) {
                    spans.add(new int[] {-1, to < 0 ? -1 : to + startOffset});
                } else {
                    spans.add(new int[] {from + startOffset, to < 0 ? -1 : to + startOffset});
                }
            }
            return spans;
        }
    }

    private static Program compilePattern(String pattern, boolean[] flags) {
        if (pattern.codePointCount(0, pattern.length()) > MAX_PATTERN) {
            throw new LimitError(0, MAX_PATTERN);
        }
        Parser parser = new Parser(pattern, flags[4]);
        Node tree = parser.parse();
        ArrayList<Instruction> instructions = new ArrayList<Instruction>();
        Fragment fragment = compile(tree, instructions);
        int match = emit(instructions, instruction(MATCH));
        patch(instructions, fragment.outs, match);
        Program program = new Program();
        program.instructions = instructions.toArray(new Instruction[0]);
        program.start = fragment.start;
        program.captureCount = parser.captureCount;
        program.groupNames = parser.groupNames;
        program.ascii = flags[0];
        program.ignoreCase = flags[1];
        program.multiline = flags[2];
        program.dotAll = flags[3];
        return program;
    }

    // ---- состояния natives ----

    static final class State {
        String expression;
        boolean[] flags = new boolean[5];
        boolean attempted;
        Program program;
        int status;
        int offset = -1;
        int kind = -1;
        int limit;
    }

    static final class Query {
        State pattern;
        String origin;
        BigInteger start;
        BigInteger end;
        int mode;
        boolean attempted;
        List<int[]> spans;
    }

    static final class Match {
        List<int[]> spans;
        Map<String, Integer> names;
    }

    static final class Replacement {
        String template;
        boolean attempted;
        int status;
        int offset = -1;
        List<Object> tokens;
    }

    static final class Substitution {
        boolean ready;
        String string;
        BigInteger amount;
    }

    private static void prepare(State state) {
        if (state.attempted) {
            return;
        }
        state.attempted = true;
        try {
            state.program = compilePattern(state.expression, state.flags);
            state.status = 0;
            state.offset = -1;
            state.kind = -1;
            state.limit = 0;
        } catch (CompileError error) {
            state.status = 1;
            state.offset = error.offset;
            state.kind = -1;
            state.limit = 0;
        } catch (LimitError error) {
            state.status = 2;
            state.offset = -1;
            state.kind = error.kind;
            state.limit = error.limit;
        } catch (StackOverflowError overflow) {
            state.status = 2;
            state.offset = -1;
            state.kind = 1;
            state.limit = MAX_NESTING;
        }
    }

    private static Map<Object, Object> intMap(String[] keys, int[] values) {
        Map<Object, Object> out = Rt.newMap();
        for (int index = 0; index < keys.length; index++) {
            out.put(keys[index], BigInteger.valueOf(values[index]));
        }
        return out;
    }

    public static Object runtime_regex__regex_state_create(String expression, List<?> flags) {
        State state = new State();
        state.expression = expression;
        for (Object flag : flags) {
            BigInteger code = (BigInteger) flag;
            if (code.signum() < 0 || code.compareTo(BigInteger.valueOf(4)) > 0) {
                throw fail("invalid_input", "regex_state_create", 2, "Unknown regex flag");
            }
            state.flags[code.intValue()] = true;
        }
        return state;
    }

    public static Map<Object, Object> runtime_regex__regex_state_prepare(Object handle) {
        State state = (State) handle;
        prepare(state);
        return intMap(new String[] {"status", "offset", "kind", "limit"},
            new int[] {state.status, state.offset, state.kind, state.limit});
    }

    public static BigInteger runtime_regex__regex_state_group_count(Object handle) {
        State state = (State) handle;
        prepare(state);
        if (state.status != 0) {
            throw fail("invalid_input", "regex_state_group_count", 1, "Regex pattern is not valid");
        }
        return BigInteger.valueOf(state.program.captureCount);
    }

    public static Map<Object, Object> runtime_regex__regex_state_group_names(Object handle) {
        State state = (State) handle;
        prepare(state);
        if (state.status != 0) {
            throw fail("invalid_input", "regex_state_group_names", 1, "Regex pattern is not valid");
        }
        Map<Object, Object> out = Rt.newMap();
        for (Map.Entry<String, Integer> entry : state.program.groupNames.entrySet()) {
            out.put(entry.getKey(), BigInteger.valueOf(entry.getValue()));
        }
        return out;
    }

    public static Object runtime_regex__regex_query_create(Object handle, String origin, BigInteger start,
            BigInteger end, BigInteger mode) {
        if (mode.signum() < 0 || mode.compareTo(BigInteger.valueOf(2)) > 0) {
            throw fail("invalid_input", "regex_query_create", 5, "Unknown regex query mode");
        }
        Query query = new Query();
        query.pattern = (State) handle;
        query.origin = origin;
        query.start = start;
        query.end = end;
        query.mode = mode.intValue();
        return query;
    }

    private static void prepareQuery(Query query, String name) {
        if (query.attempted) {
            return;
        }
        try {
            prepare(query.pattern);
            if (query.pattern.status == 2) {
                throw new EngineError("Regex pattern resource limit exceeded\n");
            }
            if (query.pattern.status != 0) {
                throw new EngineError("Invalid regex pattern\n");
            }
            query.spans = query.pattern.program.selectedSpans(query.origin, query.start, query.end, query.mode);
            query.attempted = true;
        } catch (EngineError error) {
            if (error.text.equals("Invalid regex pattern\n")) {
                throw fail("invalid_input", name, 1, "Invalid regex pattern");
            }
            if (error.text.equals("Regex pattern resource limit exceeded\n")
                    || error.text.equals("Regex input resource limit exceeded\n")) {
                throw fail("resource_exhausted", name, 1, "Regex resource limit exceeded");
            }
            throw fail("unexpected", name, 1, "Regex provider failed");
        }
    }

    public static boolean runtime_regex__regex_query_found(Object handle) {
        Query query = (Query) handle;
        prepareQuery(query, "regex_query_found");
        return !query.spans.isEmpty();
    }

    public static Object runtime_regex__regex_query_match(Object handle) {
        Query query = (Query) handle;
        prepareQuery(query, "regex_query_match");
        if (query.spans.isEmpty()) {
            throw fail("not_found", "regex_query_match", 1, "No regex match");
        }
        Match match = new Match();
        match.spans = new ArrayList<int[]>();
        for (int[] span : query.spans) {
            match.spans.add(span.clone());
        }
        match.names = new HashMap<String, Integer>(query.pattern.program.groupNames);
        return match;
    }

    public static BigInteger runtime_regex__regex_match_capture_count(Object handle) {
        return BigInteger.valueOf(((Match) handle).spans.size() - 1);
    }

    private static int capture(Match match, BigInteger index, String name) {
        if (index.signum() < 0 || index.compareTo(BigInteger.valueOf(match.spans.size())) >= 0) {
            throw fail("invalid_input", name, 2, "Missing regex capture");
        }
        return index.intValue();
    }

    public static String runtime_regex__regex_match_capture_name(Object handle, BigInteger index) {
        Match match = (Match) handle;
        int at = capture(match, index, "regex_match_capture_name");
        for (Map.Entry<String, Integer> entry : match.names.entrySet()) {
            if (entry.getValue() == at) {
                return entry.getKey();
            }
        }
        return "";
    }

    public static BigInteger runtime_regex__regex_match_capture_index(Object handle, String name) {
        Integer index = ((Match) handle).names.get(name);
        return BigInteger.valueOf(index == null ? -1 : index);
    }

    public static Map<Object, Object> runtime_regex__regex_match_capture_span(Object handle, BigInteger index) {
        Match match = (Match) handle;
        int[] span = match.spans.get(capture(match, index, "regex_match_capture_span"));
        return intMap(new String[] {"start", "end"}, span);
    }

    public static Object runtime_regex__regex_replacement_state(String template) {
        Replacement state = new Replacement();
        state.template = template;
        return state;
    }

    private static void prepareReplacement(Replacement state) {
        if (state.attempted) {
            return;
        }
        state.attempted = true;
        int[] source = state.template.codePoints().toArray();
        ArrayList<Object> tokens = new ArrayList<Object>();
        StringBuilder literal = new StringBuilder();
        int index = 0;
        while (index < source.length) {
            if (source[index] != '$') {
                literal.appendCodePoint(source[index++]);
                continue;
            }
            int offset = index++;
            if (index >= source.length) {
                state.status = 1;
                state.offset = offset;
                return;
            }
            if (source[index] == '$') {
                literal.append('$');
                index++;
                continue;
            }
            if (literal.length() > 0) {
                tokens.add("literal");
                tokens.add(literal.toString());
                literal.setLength(0);
            }
            if (source[index] >= '0' && source[index] <= '9') {
                StringBuilder digits = new StringBuilder();
                while (index < source.length && source[index] >= '0' && source[index] <= '9') {
                    digits.appendCodePoint(source[index++]);
                }
                tokens.add("index");
                tokens.add(digits.toString());
                continue;
            }
            if (source[index] == '{') {
                index++;
                StringBuilder name = new StringBuilder();
                while (index < source.length && source[index] != '}') {
                    name.appendCodePoint(source[index++]);
                }
                if (index >= source.length || !name.toString().matches("[A-Za-z_][A-Za-z0-9_]*")) {
                    state.status = 1;
                    state.offset = offset;
                    return;
                }
                index++;
                tokens.add("name");
                tokens.add(name.toString());
                continue;
            }
            state.status = 1;
            state.offset = offset;
            return;
        }
        if (literal.length() > 0) {
            tokens.add("literal");
            tokens.add(literal.toString());
        }
        state.tokens = tokens;
        state.status = 0;
        state.offset = -1;
    }

    public static Map<Object, Object> runtime_regex__regex_replacement_prepare(Object handle) {
        Replacement state = (Replacement) handle;
        prepareReplacement(state);
        return intMap(new String[] {"status", "offset"}, new int[] {state.status, state.offset});
    }

    public static List<Object> runtime_regex__regex_replacement_tokens(Object handle) {
        Replacement state = (Replacement) handle;
        prepareReplacement(state);
        if (state.status != 0) {
            throw fail("invalid_input", "regex_replacement_tokens", 1, "Invalid regex replacement");
        }
        return new ArrayList<Object>(state.tokens);
    }

    public static String runtime_regex__regex_escape(String origin) {
        StringBuilder escaped = new StringBuilder();
        for (int point : origin.codePoints().toArray()) {
            if (point == '\n') {
                escaped.append("\\n");
            } else if (point == '\r') {
                escaped.append("\\r");
            } else if (point == '\t') {
                escaped.append("\\t");
            } else if (point == 12 || point == 11) {
                escaped.append("\\x{").append(Integer.toHexString(point)).append('}');
            } else if (point < 128 && "\\.^$|()[]{}?*+ #".indexOf(point) >= 0) {
                escaped.append('\\').append((char) point);
            } else {
                escaped.appendCodePoint(point);
            }
        }
        return escaped.toString();
    }

    public static Object runtime_regex__regex_substitution_state() {
        return new Substitution();
    }

    public static boolean runtime_regex__regex_substitution_ready(Object handle) {
        return ((Substitution) handle).ready;
    }

    public static boolean runtime_regex__regex_substitution_store(Object handle, String string, BigInteger amount) {
        Substitution state = (Substitution) handle;
        state.string = string;
        state.amount = amount;
        state.ready = true;
        return true;
    }

    public static String runtime_regex__regex_substitution_string(Object handle) {
        Substitution state = (Substitution) handle;
        if (!state.ready) {
            throw fail("invalid_input", "regex_substitution_string", 1, "Regex substitution is not ready");
        }
        return state.string;
    }

    public static BigInteger runtime_regex__regex_substitution_amount(Object handle) {
        Substitution state = (Substitution) handle;
        if (!state.ready) {
            throw fail("invalid_input", "regex_substitution_amount", 1, "Regex substitution is not ready");
        }
        return state.amount;
    }
}
