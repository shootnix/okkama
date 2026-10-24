package okkama.rt;

import java.io.IOException;
import java.lang.management.ManagementFactory;
import java.math.BigInteger;
import java.security.SecureRandom;
import java.time.Instant;
import java.util.List;
import java.util.concurrent.Callable;
import java.util.concurrent.TimeUnit;

/**
 * Natives часов, ожидания и случайности хоста: {@code runtime/time},
 * {@code runtime/system::secure_random} и выборки {@code runtime/random}.
 * Тексты отказов — как у Perl-provider.
 */
public final class HostNatives {
    private HostNatives() {}

    private static final BigInteger BILLION = BigInteger.valueOf(1_000_000_000L);
    private static final BigInteger MAXIMUM_NANOSECOND = BigInteger.valueOf(Long.MAX_VALUE);
    private static final BigInteger MODULUS = BigInteger.ONE.shiftLeft(64);
    private static final long ORIGIN = System.nanoTime();
    private static final String PID = processId();
    private static final String SYSTEM_DOMAIN = "system:" + PID + ":1";
    private static final int MAX_RANDOM_SIZE = 16 * 1024 * 1024;
    private static int domainSerial = 1;
    private static SecureRandom secure;

    static String pid() {
        return PID;
    }

    private static String processId() {
        String name = ManagementFactory.getRuntimeMXBean().getName();
        int at = name.indexOf('@');
        return at > 0 ? name.substring(0, at) : name;
    }

    private static Failure failure(String domain, String code, String symbol, String message) {
        return NativeFailure.raise(domain, code, symbol, message);
    }

    public static BigInteger runtime_time__wall_clock() {
        Instant now = Instant.now();
        return BigInteger.valueOf(now.getEpochSecond()).multiply(BILLION)
            .add(BigInteger.valueOf(now.getNano()));
    }

    public static BigInteger runtime_time__monotonic_clock() {
        return BigInteger.valueOf(System.nanoTime() - ORIGIN);
    }

    public static String runtime_time__clock_domain(boolean fresh) {
        if (!fresh) {
            return SYSTEM_DOMAIN;
        }
        if (Tests.current == null) {
            throw failure("core.time", "permission_denied", "runtime/time@1::clock_domain/1",
                "Manual clock domains require a running test case");
        }
        return "manual:" + PID + ":" + ++domainSerial;
    }

    public static Future runtime_time__sleep_nanoseconds(String source) {
        String symbol = "runtime/time@1::sleep_nanoseconds/1";
        if (!source.matches("0|[1-9][0-9]*")) {
            return Scheduler.resolved(null, failure("core.time", "invalid_input", symbol,
                "Wait duration must be a canonical non-negative integer"));
        }
        BigInteger duration = new BigInteger(source);
        if (duration.compareTo(MAXIMUM_NANOSECOND) > 0) {
            return Scheduler.resolved(null, failure("core.time", "invalid_input", symbol,
                "Wait duration exceeds the runtime timer range"));
        }
        if (duration.signum() == 0) {
            return Scheduler.resolved(Boolean.TRUE, null);
        }
        final long nanoseconds = duration.longValue();
        final Future promise = Future.promise();
        final Scheduler.Op op = Scheduler.offload(new Callable<Object>() {
            public Object call() throws InterruptedException {
                TimeUnit.NANOSECONDS.sleep(nanoseconds);
                return Boolean.TRUE;
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                Scheduler.settle(promise, Boolean.TRUE, null);
            }
        });
        // Отменённое ожидание не держит программу.
        promise.listen(new Runnable() {
            public void run() {
                op.detach();
            }
        });
        return promise;
    }

    public static List<Object> runtime_system__secure_random(BigInteger size) {
        String symbol = "runtime/system@1::secure_random/1";
        if (size.signum() < 0) {
            throw failure("core.system", "invalid_input", symbol,
                "Secure random size must be non-negative");
        }
        if (size.compareTo(BigInteger.valueOf(MAX_RANDOM_SIZE)) > 0) {
            throw failure("core.system", "resource_exhausted", symbol,
                "Secure random request exceeds 16 MiB");
        }
        if (secure == null) {
            secure = new SecureRandom();
        }
        byte[] bytes = new byte[size.intValue()];
        secure.nextBytes(bytes);
        return Natives.fromBytes(bytes, bytes.length);
    }

    private static BigInteger unsigned(List<?> sample) {
        BigInteger number = BigInteger.ZERO;
        for (Object octet : sample) {
            number = number.shiftLeft(8).add(BigInteger.valueOf(((Byte) octet) & 0xff));
        }
        return number;
    }

    private static List<?> integerSample(List<?> sample, String name) {
        if (sample.size() != 8) {
            throw failure("core.random", "invalid_input", "runtime/random@1::" + name + "/3",
                "Random integer sample must have eight bytes");
        }
        return sample;
    }

    public static boolean runtime_random__random_sample_accepted(List<?> sample, BigInteger start,
            BigInteger stop) {
        BigInteger value = unsigned(integerSample(sample, "random_sample_accepted"));
        BigInteger span = stop.subtract(start);
        if (span.signum() <= 0) {
            return false;
        }
        return value.compareTo(MODULUS.mod(span)) >= 0;
    }

    public static BigInteger runtime_random__random_integer_from_sample(List<?> sample,
            BigInteger start, BigInteger stop) {
        BigInteger value = unsigned(integerSample(sample, "random_integer_from_sample"));
        BigInteger span = stop.subtract(start);
        if (span.signum() == 0) {
            // Math::BigInt: x mod 0 = x.
            return start.add(value);
        }
        // Остаток с знаком делителя, как у Math::BigInt.
        BigInteger rest = value.mod(span.abs());
        if (span.signum() < 0 && rest.signum() != 0) {
            rest = rest.add(span);
        }
        return start.add(rest);
    }

    public static Double runtime_random__random_float_from_sample(List<?> sample) {
        if (sample.size() != 7) {
            throw failure("core.random", "invalid_input", "runtime/random@1::random_float_from_sample/1",
                "Random float sample must have seven bytes");
        }
        return unsigned(sample).shiftRight(3).doubleValue() / 9007199254740992.0;
    }

    // ---- runtime/system ----

    public static String runtime_system__system_platform() {
        String name = System.getProperty("os.name", "").toLowerCase(java.util.Locale.ROOT);
        if (name.contains("mac") || name.contains("darwin")) {
            return "macos";
        }
        if (name.contains("linux")) {
            return "linux";
        }
        if (name.contains("windows")) {
            return "windows";
        }
        for (String known : new String[] {"freebsd", "openbsd", "netbsd", "dragonfly", "solaris", "aix"}) {
            if (name.contains(known)) {
                return known;
            }
        }
        if (name.contains("sunos")) {
            return "solaris";
        }
        return "unknown";
    }

    public static String runtime_system__system_architecture() {
        String arch = System.getProperty("os.arch", "").toLowerCase(java.util.Locale.ROOT);
        if (arch.matches(".*(?:x86_64|amd64).*")) {
            return "x86_64";
        }
        if (arch.matches(".*(?:i[3-6]86|x86).*")) {
            return "x86";
        }
        if (arch.matches(".*(?:aarch64|arm64).*")) {
            return "arm64";
        }
        if (arch.contains("arm")) {
            return "arm";
        }
        if (arch.matches(".*(?:powerpc64|ppc64).*")) {
            return "ppc64";
        }
        if (arch.matches(".*(?:powerpc|ppc).*")) {
            return "ppc";
        }
        if (arch.contains("riscv64")) {
            return "riscv64";
        }
        if (arch.contains("s390x")) {
            return "s390x";
        }
        return "unknown";
    }

    public static String runtime_system__system_byte_order() {
        return java.nio.ByteOrder.nativeOrder() == java.nio.ByteOrder.LITTLE_ENDIAN ? "little" : "big";
    }

    public static String runtime_system__system_host_name() {
        String name = null;
        try {
            Process process = new ProcessBuilder("hostname").redirectErrorStream(true).start();
            java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
            byte[] buffer = new byte[256];
            java.io.InputStream input = process.getInputStream();
            for (int count = input.read(buffer); count > 0; count = input.read(buffer)) {
                out.write(buffer, 0, count);
            }
            if (process.waitFor() == 0) {
                name = new String(out.toByteArray(), java.nio.charset.StandardCharsets.UTF_8).trim();
            }
        } catch (IOException | InterruptedException unavailable) {
            name = null;
        }
        if (name == null || name.isEmpty()) {
            try {
                name = java.net.InetAddress.getLocalHost().getHostName();
            } catch (java.net.UnknownHostException unknown) {
                name = null;
            }
        }
        if (name == null || name.isEmpty()) {
            throw failure("core.system", "unexpected", "runtime/system@1::system_host_name/0", "Host name is unavailable");
        }
        return name;
    }

    public static BigInteger runtime_system__system_cpu_count() {
        String environment = System.getenv("NUMBER_OF_PROCESSORS");
        if (environment != null && environment.trim().matches("[1-9][0-9]*")) {
            return new BigInteger(environment.trim());
        }
        return BigInteger.valueOf(Math.max(1, Runtime.getRuntime().availableProcessors()));
    }

    public static java.util.Map<Object, Object> runtime_system__storage_usage(String path) {
        String symbol = "runtime/system@1::storage_usage/1";
        if (path.indexOf('\0') >= 0) {
            throw failure("core.system", "invalid_input", symbol, "Storage path contains a zero byte");
        }
        String absolute;
        try {
            absolute = Host.path(path).toRealPath().toString();
        } catch (IOException error) {
            throw failure("core.system", Host.code(error), symbol, "Cannot resolve storage path");
        }
        String df = new java.io.File("/bin/df").canExecute() ? "/bin/df"
            : new java.io.File("/usr/bin/df").canExecute() ? "/usr/bin/df" : null;
        if (df == null) {
            throw failure("core.system", "unsupported", symbol, "Storage usage is unavailable");
        }
        String last = "";
        try {
            ProcessBuilder builder = new ProcessBuilder(df, "-Pk", absolute);
            builder.environment().put("LC_ALL", "C");
            builder.environment().put("LANG", "C");
            Process process = builder.start();
            BufferedLines lines = new BufferedLines(process.getInputStream());
            for (String line = lines.next(); line != null; line = lines.next()) {
                last = line;
            }
            if (process.waitFor() != 0) {
                throw failure("core.system", "unexpected", symbol, "Cannot inspect storage usage");
            }
        } catch (IOException | InterruptedException error) {
            throw failure("core.system", "unexpected", symbol, "Cannot inspect storage usage");
        }
        java.util.regex.Matcher match = java.util.regex.Pattern
            .compile("^\\S+\\s+([0-9]+)\\s+[0-9]+\\s+([0-9]+)\\s+").matcher(last);
        if (!match.find()) {
            throw failure("core.system", "invalid_data", symbol, "Invalid storage usage result");
        }
        BigInteger capacity = new BigInteger(match.group(1)).multiply(BigInteger.valueOf(1024));
        BigInteger available = new BigInteger(match.group(2)).multiply(BigInteger.valueOf(1024));
        if (available.signum() < 0 || available.compareTo(capacity) > 0) {
            throw failure("core.system", "invalid_data", symbol, "Inconsistent storage usage result");
        }
        java.util.Map<Object, Object> out = Rt.newMap();
        out.put("capacity", capacity.toString());
        out.put("used", capacity.subtract(available).toString());
        out.put("available", available.toString());
        return out;
    }

    /** Строки потока процесса (UTF-8). */
    static final class BufferedLines {
        private final java.io.BufferedReader reader;

        BufferedLines(java.io.InputStream input) {
            reader = new java.io.BufferedReader(new java.io.InputStreamReader(input, java.nio.charset.StandardCharsets.UTF_8));
        }

        String next() throws IOException {
            return reader.readLine();
        }
    }
}
