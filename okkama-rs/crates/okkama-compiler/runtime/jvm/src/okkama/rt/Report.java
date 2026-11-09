package okkama.rt;

import java.io.BufferedReader;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;

/**
 * Отчёт о неперехваченном отказе, как у ядра Sparrow и движка IR (#270): сам
 * отказ, вторичные отказы cleanup и причины, место со строкой исходника и
 * стек Okkama. Стек — кадры JVM методов программы; номер «строки» в их
 * LineNumberTable — номер места, а места и имена кадров лежат в ресурсе
 * {@code okkama-debug.tsv}, который пишет компилятор. Всё это читается только
 * здесь, при отчёте: на пути без отказов ничего не стоит.
 */
final class Report {
    private Report() {}

    /** Сколько отказов показывает отчёт: дальше цепочка обрывается строкой. */
    private static final int CHAIN_ENTRIES = 64;
    /** Наибольшее число строк стека: длиннее — середина опускается. */
    private static final int STACK_LINES = 40;
    /** Глубина стека, которую HotSpot записывает по умолчанию. */
    private static final int JVM_STACK_DEPTH = 1024;

    static String text(Failure failure) {
        StringBuilder out = new StringBuilder(chain(failure)).append('\n');
        if (!"off".equals(System.getenv("OKKAMA_TRACE"))) {
            try {
                out.append(trace(failure));
            } catch (RuntimeException | java.io.IOException unavailable) {
                // Без отладочных сведений отчёт остаётся без места и стека.
            }
        }
        return out.toString();
    }

    /** Отказ, его вторичные отказы и причины обходом в глубину, как у ядра. */
    static String chain(Failure root) {
        StringBuilder out = new StringBuilder(Rt.describe(root));
        IdentityHashMap<Failure, Integer> shown = new IdentityHashMap<Failure, Integer>();
        shown.put(root, 0);
        ArrayList<Object[]> pending = new ArrayList<Object[]>();
        links(root, pending);
        while (!pending.isEmpty()) {
            Object[] next = pending.remove(pending.size() - 1);
            Failure failure = (Failure) next[0];
            out.append('\n').append((String) next[1]).append("\n  ");
            Integer index = shown.get(failure);
            if (index != null) {
                out.append("[failure #").append(index).append(" already shown]");
                continue;
            }
            if (shown.size() == CHAIN_ENTRIES) {
                out.append("... further failures not shown");
                break;
            }
            shown.put(failure, shown.size());
            out.append(Rt.describe(failure).replace("\n", "\n  "));
            links(failure, pending);
        }
        return out.toString();
    }

    /** Причина и вторичные отказы в порядке стека: первыми снимаются вторичные. */
    private static void links(Failure failure, ArrayList<Object[]> pending) {
        if (failure.cause != null) {
            pending.add(new Object[] {failure.cause, "Caused by:"});
        }
        for (int index = failure.secondary.size() - 1; index >= 0; index--) {
            pending.add(new Object[] {failure.secondary.get(index), "Secondary cleanup failure:"});
        }
    }

    /** Метод программы: имя кадра, исходник и места по номеру. */
    private static final class Method {
        String frame;
        String parent;
        boolean part;
        String path;
        boolean root;
        final ArrayList<int[]> sites = new ArrayList<int[]>();
    }

    /** Кадр стека: метод и номер места в нём (−1 — место неизвестно). */
    private static final class Frame {
        final Method method;
        final int site;

        Frame(Method method, int site) {
            this.method = method;
            this.site = site;
        }

        int[] place() {
            return site >= 0 && site < method.sites.size() ? method.sites.get(site) : null;
        }

        String text() {
            int[] place = place();
            if (place == null) {
                return method.frame;
            }
            return method.frame + " (" + display(method.path) + ":" + place[0] + ":" + place[1] + ")";
        }
    }

    private static Map<String, Method> methods;

    /** {@code M class method frame parent part path root}, затем {@code S line column endLine endColumn}. */
    private static Map<String, Method> methods() throws java.io.IOException {
        if (methods != null) {
            return methods;
        }
        HashMap<String, Method> found = new HashMap<String, Method>();
        InputStream stream = Report.class.getClassLoader().getResourceAsStream("okkama-debug.tsv");
        if (stream != null) {
            try (BufferedReader reader = new BufferedReader(new InputStreamReader(stream, StandardCharsets.UTF_8))) {
                Method current = null;
                String line;
                while ((line = reader.readLine()) != null) {
                    String[] parts = line.split("\t", -1);
                    if (parts[0].equals("M") && parts.length == 8) {
                        current = new Method();
                        current.frame = parts[3];
                        current.parent = parts[4];
                        current.part = parts[5].equals("1");
                        current.path = parts[6];
                        current.root = parts[7].equals("1");
                        found.put(parts[1] + "\t" + parts[2], current);
                    } else if (parts[0].equals("S") && parts.length == 5 && current != null) {
                        current.sites.add(new int[] {
                            Integer.parseInt(parts[1]), Integer.parseInt(parts[2]),
                            Integer.parseInt(parts[3]), Integer.parseInt(parts[4])});
                    }
                }
            }
        }
        methods = found;
        return found;
    }

    /** Место (строка исходника с подчёркиванием) и стек; пусто — стек неизвестен. */
    static String trace(Failure failure) throws java.io.IOException {
        Map<String, Method> known = methods();
        if (known.isEmpty()) {
            return "";
        }
        List<Frame> frames = new ArrayList<Frame>();
        // Часть функции, вынесенная в свой метод, — тот же кадр Okkama: её
        // вызов из функции (или из другой части) в стек не попадает.
        String absorbing = null;
        StackTraceElement[] own = failure.getStackTrace();
        // HotSpot keeps at most MaxJavaStackTraceDepth (1024) frames, the most
        // recent ones: the report says that older ones are missing.
        boolean truncated = own.length >= JVM_STACK_DEPTH;
        ArrayList<StackTraceElement> elements = new ArrayList<StackTraceElement>(java.util.Arrays.asList(own));
        if (failure.awaited != null) {
            for (StackTraceElement[] awaiting : failure.awaited) {
                elements.addAll(java.util.Arrays.asList(awaiting));
            }
        }
        for (StackTraceElement element : elements) {
            Method method = known.get(element.getClassName() + "\t" + element.getMethodName());
            if (method == null) {
                continue;
            }
            if (absorbing != null && absorbing.equals(method.parent)) {
                if (!method.part) {
                    absorbing = null;
                }
                continue;
            }
            absorbing = method.part ? method.parent : null;
            frames.add(new Frame(method, element.getLineNumber() - 1));
        }
        if (frames.isEmpty()) {
            return "";
        }
        StringBuilder out = new StringBuilder();
        Frame shown = null;
        for (Frame frame : frames) {
            if (frame.method.root && frame.place() != null) {
                shown = frame;
                break;
            }
        }
        if (shown == null) {
            for (Frame frame : frames) {
                if (frame.place() != null) {
                    shown = frame;
                    break;
                }
            }
        }
        if (shown != null) {
            out.append(snippet(shown.method.path, shown.place()));
        }
        out.append("Stack trace (most recent call first):\n");
        ArrayList<String> texts = new ArrayList<String>();
        for (Frame frame : frames) {
            texts.add(frame.text());
        }
        // Серии одинаковых кадров — одной строкой (рекурсия), не больше
        // STACK_LINES строк: середина опускается (#241).
        ArrayList<int[]> runs = new ArrayList<int[]>();
        for (int index = 0; index < texts.size(); index++) {
            int[] last = runs.isEmpty() ? null : runs.get(runs.size() - 1);
            if (last != null && texts.get(last[0]).equals(texts.get(index))) {
                last[1]++;
            } else {
                runs.add(new int[] {index, 1});
            }
        }
        ArrayList<String> lines = new ArrayList<String>();
        if (runs.size() > STACK_LINES) {
            int tail = 10;
            int head = STACK_LINES - tail - 1;
            for (int index = 0; index < head; index++) {
                lines.add(run(texts, runs.get(index)));
            }
            lines.add("  ... " + (runs.size() - head - tail) + " more frames ...");
            for (int index = runs.size() - tail; index < runs.size(); index++) {
                lines.add(run(texts, runs.get(index)));
            }
        } else {
            for (int[] run : runs) {
                lines.add(run(texts, run));
            }
        }
        if (truncated) {
            lines.add("  ... older frames are beyond the JVM's stack trace depth ...");
        }
        for (String line : lines) {
            out.append(line).append('\n');
        }
        return out.toString();
    }

    private static String run(List<String> texts, int[] run) {
        String line = "  at " + texts.get(run[0]);
        return run[1] > 1 ? line + "  (×" + run[1] + ")" : line;
    }

    /** {@code --> файл:строка:колонка}, строка исходника и подчёркивание места. */
    private static String snippet(String path, int[] place) {
        String text;
        try {
            List<String> lines = Files.readAllLines(Paths.get(path), StandardCharsets.UTF_8);
            if (place[0] < 1 || place[0] > lines.size()) {
                return "";
            }
            text = lines.get(place[0] - 1);
        } catch (java.io.IOException | RuntimeException missing) {
            // Исходника рядом нет (jar запущен в другом месте): без строки.
            return "";
        }
        String gutter = Integer.toString(place[0]);
        String pad = " ".repeat(gutter.length());
        int length = text.codePointCount(0, text.length());
        int last = place[2] == place[0] ? place[3] : length + 1;
        int width = Math.max(last - place[1], 1);
        return pad + "--> " + display(path) + ":" + place[0] + ":" + place[1] + "\n"
            + pad + " |\n"
            + gutter + " | " + text.replace('\t', ' ') + "\n"
            + pad + " | " + " ".repeat(Math.max(place[1] - 1, 0)) + "^".repeat(width) + "\n";
    }

    /** Путь файла: относительно текущего каталога, если файл внутри него. */
    private static String display(String path) {
        try {
            Path file = Paths.get(path);
            Path directory = Paths.get(System.getProperty("user.dir"));
            if (file.isAbsolute() && file.startsWith(directory)) {
                return directory.relativize(file).toString();
            }
        } catch (RuntimeException invalid) {
            // Путь как есть.
        }
        return path;
    }
}
