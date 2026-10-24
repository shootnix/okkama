package okkama.rt;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/**
 * Встроенный исполнитель тестов (§14): тесты корня по порядку, каждый в
 * своём состоянии; провал утверждения не прерывает тест, отказ — только
 * его. Записи {@code === RUN} / {@code --- PASS} / {@code --- FAIL} как у
 * Perl-интерпретатора; код выхода 0 — только если прошли все.
 */
public final class Tests {
    private Tests() {}

    /** Текущий тест: провалы утверждений. */
    static final class Case {
        final ArrayList<String> failures = new ArrayList<String>();
    }

    static Case current;

    public static void run(final String[] names, final Fn[] bodies) {
        Rt.onProgramStack(new Runnable() {
            public void run() {
                runAll(names, bodies);
            }
        });
    }

    private static void runAll(String[] names, Fn[] bodies) {
        if (names.length == 0) {
            Rt.report(Rt.trap("Test run requires at least one test"));
            return;
        }
        boolean passed = true;
        for (int index = 0; index < names.length; index++) {
            System.out.println("=== RUN   " + names[index]);
            Case test = new Case();
            current = test;
            try {
                Object outcome = bodies[index].call(new Object[0]);
                Scheduler.runTest(outcome instanceof Future ? (Future) outcome : null);
            } catch (Failure failure) {
                test.failures.add(Rt.describe(failure));
            } catch (StackOverflowError overflow) {
                test.failures.add("RuntimeError: Stack overflow");
            }
            current = null;
            if (test.failures.isEmpty()) {
                System.out.println("--- PASS: " + names[index]);
            } else {
                passed = false;
                System.out.println("--- FAIL: " + names[index]);
                for (String failure : test.failures) {
                    System.out.println("    " + failure.replace("\n", "\n    "));
                }
            }
        }
        System.out.println(passed ? "PASS" : "FAIL");
        System.out.flush();
        System.exit(passed ? 0 : 1);
    }

    static void fail(String message) {
        if (current == null) {
            throw Rt.trap("The test capability is available only inside a running test");
        }
        current.failures.add(message);
    }

    /** Строгое глубокое равенство утверждений: `any` сравнивается по содержимому. */
    static boolean same(Object left, Object right) {
        if (left instanceof Any && right instanceof Any) {
            return same(((Any) left).value, ((Any) right).value);
        }
        if (left instanceof List && right instanceof List) {
            List<?> a = (List<?>) left;
            List<?> b = (List<?>) right;
            if (a.size() != b.size()) {
                return false;
            }
            for (int index = 0; index < a.size(); index++) {
                if (!same(a.get(index), b.get(index))) {
                    return false;
                }
            }
            return true;
        }
        if (left instanceof Map && right instanceof Map) {
            Map<?, ?> a = (Map<?, ?>) left;
            Map<?, ?> b = (Map<?, ?>) right;
            if (!a.keySet().equals(b.keySet())) {
                return false;
            }
            for (Object key : a.keySet()) {
                if (!same(a.get(key), b.get(key))) {
                    return false;
                }
            }
            return true;
        }
        return Objects.equals(left, right);
    }

    static String show(Object value) {
        if (value instanceof String) {
            return "\"" + value + "\"";
        }
        if (value instanceof Any) {
            return show(((Any) value).value);
        }
        return String.valueOf(value);
    }
}
