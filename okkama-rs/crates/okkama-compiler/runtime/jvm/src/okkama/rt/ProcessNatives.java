package okkama.rt;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.Callable;

/**
 * Natives {@code runtime/process}: дочерние процессы. На JDK 22+ (macOS,
 * Linux) — {@code posix_spawn} и собственный {@code waitpid} через FFM
 * ({@link Exact}: точный статус и сигналы). Иначе — {@link ProcessBuilder}:
 * JVM сообщает гибель от сигнала N кодом {@code 128 + N}, поэтому коды
 * 129..192 трактуются как сигнал, а {@code SIGINT} посылает утилита
 * {@code kill}.
 */
public final class ProcessNatives {
    private ProcessNatives() {}

    private static final String DOMAIN = "core.process";
    private static final int CHUNK = 64 * 1024;

    /** Потомок с точным статусом {@code waitpid} (src22 {@code PosixSpawn}). */
    public interface Exact {
        boolean signaled();

        int termination();

        boolean signal(int number);
    }

    static final class ProcessState {
        Process process;
        OutputStream input;
        InputStream output;
        InputStream errors;
        boolean inputClosed;
        boolean closed;
        boolean capturing;
    }

    static final class Captured {
        Map<Object, Object> status;
        int overflow;
        byte[] output;
        byte[] errors;
    }

    static final class Command {
        String program;
        List<String> arguments;
        String directory;
        Map<String, String> environment;
    }

    static final class PipelineState {
        final ArrayList<Command> commands = new ArrayList<Command>();
        List<Process> processes;
        boolean started;
        boolean closed;
        boolean capturing;
    }

    static final class CapturedPipeline {
        List<Object> statuses;
        int overflowStream;
        int overflowIndex = -1;
        byte[] output;
        List<byte[]> errors;
    }

    private static String symbol(String name, int arity) {
        return "runtime/process@1::" + name + "/" + arity;
    }

    private static Failure failure(String code, String operation, String message) {
        return NativeFailure.raise(DOMAIN, code, operation, message);
    }

    private static Failure hostFailure(String operation, String message, Throwable error) {
        if (error instanceof Failure) {
            return (Failure) error;
        }
        return failure(Host.code(error), operation, message);
    }

    // ---- статус ----

    private static Map<Object, Object> status(int state, int code) {
        Map<Object, Object> out = Rt.newMap();
        out.put("code", BigInteger.valueOf(code));
        out.put("state", BigInteger.valueOf(state));
        return out;
    }

    private static int portableSignal(int signal) {
        switch (signal) {
            case 15:
                return 1;
            case 9:
                return 2;
            case 2:
                return 3;
            default:
                return 0;
        }
    }

    /** Статус завершившегося потомка. */
    private static Map<Object, Object> project(Process process) {
        if (process instanceof Exact) {
            Exact exact = (Exact) process;
            return exact.signaled() ? status(2, portableSignal(exact.termination())) : status(1, exact.termination());
        }
        // JDK: {@code 128 + N} у убитого сигналом N.
        int value = process.exitValue();
        if (value > 128 && value <= 128 + 64) {
            return status(2, portableSignal(value - 128));
        }
        return status(1, value);
    }

    // ---- запуск ----

    private static String text(String value, String description, String operation) {
        if (value.indexOf('\0') >= 0) {
            throw failure("invalid_input", operation, description + " contains a zero byte");
        }
        return value;
    }

    private static Command command(String kind, String program, List<?> arguments, String directory, Map<?, ?> environment,
            String operation) {
        Command command = new Command();
        command.program = text(program, kind + " program", operation);
        if (program.isEmpty()) {
            throw failure("invalid_input", operation, kind + " program is empty");
        }
        command.arguments = new ArrayList<String>();
        for (Object argument : arguments) {
            command.arguments.add(text((String) argument, kind + " argument", operation));
        }
        command.directory = text(directory, kind + " directory", operation);
        if (directory.isEmpty()) {
            throw failure("invalid_input", operation, kind + " directory is empty");
        }
        command.environment = new java.util.LinkedHashMap<String, String>();
        for (Map.Entry<?, ?> entry : environment.entrySet()) {
            String name = text((String) entry.getKey(), kind + " environment name", operation);
            if (name.isEmpty()) {
                throw failure("invalid_input", operation, kind.equals("Process") ? "Process environment name is empty"
                    : "Invalid pipeline environment name");
            }
            if (name.indexOf('=') >= 0) {
                throw failure("invalid_input", operation, kind.equals("Process")
                    ? "Process environment name contains equals" : "Invalid pipeline environment name");
            }
            command.environment.put(name, text((String) entry.getValue(), kind + " environment value", operation));
        }
        return command;
    }

    /**
     * Программа без {@code /} ищется по {@code PATH} окружения потомка, как
     * {@code execvp} после замены {@code %ENV} у Perl; JVM искала бы по PATH
     * родителя.
     */
    private static String resolve(Command command, File directory) {
        if (command.program.indexOf('/') >= 0) {
            return command.program;
        }
        String path = command.environment.get("PATH");
        if (path == null) {
            path = "/usr/bin:/bin";
        }
        for (String entry : path.split(":", -1)) {
            File base = entry.isEmpty() ? directory : new File(entry);
            if (!base.isAbsolute()) {
                base = new File(directory, entry);
            }
            File candidate = new File(base, command.program);
            if (candidate.isFile() && candidate.canExecute()) {
                return candidate.getPath();
            }
        }
        return command.program;
    }

    private static List<String> line(Command command) {
        File directory = Host.path(command.directory).toFile();
        ArrayList<String> line = new ArrayList<String>();
        line.add(resolve(command, directory));
        line.addAll(command.arguments);
        return line;
    }

    private static ProcessBuilder builder(Command command) {
        ProcessBuilder builder = new ProcessBuilder(line(command));
        builder.directory(Host.path(command.directory).toFile());
        builder.environment().clear();
        builder.environment().putAll(command.environment);
        return builder;
    }

    private static java.lang.reflect.Method posix;
    private static boolean posixChecked;

    /** {@code PosixSpawn.start} рантайма JDK 22+, если он загружается. */
    private static synchronized java.lang.reflect.Method posix() {
        if (!posixChecked) {
            posixChecked = true;
            try {
                java.lang.reflect.Method method = Class.forName("okkama.rt.PosixSpawn")
                    .getMethod("start", List.class, List.class, List.class);
                String os = System.getProperty("os.name", "").toLowerCase();
                if (os.contains("mac") || os.contains("linux")) {
                    posix = method;
                }
            } catch (ReflectiveOperationException | LinkageError unavailable) {
                posix = null;
            }
        }
        return posix;
    }

    /** Запустить команды конвейером (одна команда — один процесс). */
    @SuppressWarnings("unchecked")
    private static List<Process> launch(List<Command> commands, String operation) throws IOException {
        java.lang.reflect.Method spawn = posix();
        if (spawn != null) {
            ArrayList<List<String>> lines = new ArrayList<List<String>>();
            ArrayList<String> directories = new ArrayList<String>();
            ArrayList<Map<String, String>> environments = new ArrayList<Map<String, String>>();
            for (Command command : commands) {
                lines.add(line(command));
                directories.add(Host.path(command.directory).toString());
                environments.add(command.environment);
            }
            try {
                return (List<Process>) spawn.invoke(null, lines, directories, environments);
            } catch (java.lang.reflect.InvocationTargetException error) {
                Throwable cause = error.getCause();
                if (cause instanceof IOException) {
                    throw (IOException) cause;
                }
                throw new IOException(cause);
            } catch (IllegalAccessException error) {
                throw new IOException(error);
            }
        }
        ArrayList<ProcessBuilder> builders = new ArrayList<ProcessBuilder>();
        for (Command command : commands) {
            builders.add(builder(command));
        }
        if (builders.size() == 1) {
            return java.util.Collections.singletonList(builders.get(0).start());
        }
        return startPipeline(builders, operation);
    }

    private static ProcessState state(Object handle, String operation) {
        ProcessState state = (ProcessState) handle;
        if (state.closed) {
            throw failure("already_closed", operation, "Process is already closed");
        }
        return state;
    }

    private static ProcessState started(Object handle, String operation) {
        ProcessState state = state(handle, operation);
        if (state.process == null) {
            throw failure("invalid_input", operation, "Process has not been started");
        }
        return state;
    }

    private static void closeQuietly(java.io.Closeable stream) {
        try {
            if (stream != null) {
                stream.close();
            }
        } catch (IOException ignored) {
            // освобождение
        }
    }

    private static void kill(Process process) {
        if (process.isAlive()) {
            process.destroyForcibly();
        }
        reap(process);
    }

    private static void reap(Process process) {
        while (true) {
            try {
                process.waitFor();
                return;
            } catch (InterruptedException ignored) {
                // ждём дальше
            }
        }
    }

    private static void close(ProcessState state) {
        if (state.closed) {
            return;
        }
        state.closed = true;
        if (state.process != null) {
            kill(state.process);
            closeQuietly(state.input);
            closeQuietly(state.output);
            closeQuietly(state.errors);
        }
    }

    public static BigInteger runtime_process__process_current_id() {
        return new BigInteger(HostNatives.pid());
    }

    public static Object runtime_process__process_state_create() {
        return new ProcessState();
    }

    public static boolean runtime_process__process_state_start(Object handle, String program, List<?> arguments,
            String directory, Map<?, ?> environment) {
        String operation = symbol("process_state_start", 5);
        ProcessState state = state(handle, operation);
        if (state.process != null) {
            return true;
        }
        Command command = command("Process", program, arguments, directory, environment, operation);
        Process process;
        try {
            process = launch(java.util.Collections.singletonList(command), operation).get(0);
        } catch (IOException error) {
            throw hostFailure(operation, "Cannot configure or execute process", error);
        }
        state.process = process;
        state.input = process.getOutputStream();
        state.output = process.getInputStream();
        state.errors = process.getErrorStream();
        return true;
    }

    public static boolean runtime_process__process_state_started(Object handle) {
        return state(handle, symbol("process_state_started", 1)).process != null;
    }

    /** {@code Process.pid()} (Java 9+) рефлексией: исходники собираются под Java 8. */
    private static long pid(Process process, String operation) {
        try {
            return ((Number) Process.class.getMethod("pid").invoke(process)).longValue();
        } catch (ReflectiveOperationException error) {
            throw failure("unsupported", operation, "Process id is unavailable on this JVM");
        }
    }

    public static BigInteger runtime_process__process_state_id(Object handle) {
        String operation = symbol("process_state_id", 1);
        return BigInteger.valueOf(pid(started(handle, operation).process, operation));
    }

    public static boolean runtime_process__process_state_running(Object handle) {
        return started(handle, symbol("process_state_running", 1)).process.isAlive();
    }

    public static Future runtime_process__process_state_exit(Object handle) {
        final ProcessState state;
        try {
            state = started(handle, symbol("process_state_exit", 1));
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
        final Process process = state.process;
        if (!process.isAlive()) {
            return Scheduler.resolved(project(process), null);
        }
        final Future promise = Future.promise();
        final Scheduler.Op op = Scheduler.offload(new Callable<Object>() {
            public Object call() throws InterruptedException {
                return process.waitFor();
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                Scheduler.settle(promise, project(process), null);
            }
        });
        promise.listen(new Runnable() {
            public void run() {
                op.detach();
            }
        });
        return promise;
    }

    public static boolean runtime_process__process_state_signal(Object handle, BigInteger signal) {
        String operation = symbol("process_state_signal", 2);
        ProcessState state = started(handle, operation);
        if (!state.process.isAlive()) {
            throw failure("invalid_input", operation, "Process has already completed");
        }
        int code = signal.bitLength() > 31 ? -1 : signal.intValue();
        if (state.process instanceof Exact && code >= 0 && code <= 2) {
            if (!((Exact) state.process).signal(code == 0 ? 2 : code == 1 ? 15 : 9)) {
                throw failure("unexpected", operation, "Cannot signal child process");
            }
            return true;
        }
        switch (code) {
            case 0:
                try {
                    Process kill = new ProcessBuilder("kill", "-INT", Long.toString(pid(state.process, operation)))
                        .redirectErrorStream(true).start();
                    kill.getInputStream().close();
                    if (kill.waitFor() != 0) {
                        throw failure("unexpected", operation, "Cannot signal child process");
                    }
                } catch (IOException error) {
                    throw hostFailure(operation, "Cannot signal child process", error);
                } catch (InterruptedException error) {
                    throw failure("interrupted", operation, "Cannot signal child process");
                }
                return true;
            case 1:
                state.process.destroy();
                return true;
            case 2:
                state.process.destroyForcibly();
                return true;
            default:
                throw failure("unsupported", operation, "Process signal is unsupported");
        }
    }

    public static boolean runtime_process__process_state_close(Object handle) {
        close((ProcessState) handle);
        return true;
    }

    public static BigInteger runtime_process__process_state_input_write(Object handle, List<?> source) {
        String operation = symbol("process_state_input_write", 2);
        ProcessState state = started(handle, operation);
        if (state.inputClosed) {
            throw failure("already_closed", operation, "Process input is already closed");
        }
        byte[] bytes = Natives.toBytes(source);
        if (bytes.length == 0) {
            return BigInteger.ZERO;
        }
        try {
            state.input.write(bytes);
            state.input.flush();
        } catch (IOException error) {
            throw failure(brokenPipe(error) ? "broken_pipe" : Host.code(error), operation, "Cannot write process input");
        }
        return BigInteger.valueOf(bytes.length);
    }

    private static boolean brokenPipe(IOException error) {
        String text = String.valueOf(error.getMessage()).toLowerCase();
        return text.contains("broken pipe") || text.contains("stream closed");
    }

    public static boolean runtime_process__process_state_input_flush(Object handle) {
        String operation = symbol("process_state_input_flush", 1);
        ProcessState state = started(handle, operation);
        if (state.inputClosed) {
            throw failure("already_closed", operation, "Process input is already closed");
        }
        return true;
    }

    public static boolean runtime_process__process_state_input_closed(Object handle) {
        ProcessState state = (ProcessState) handle;
        return state.closed || state.inputClosed;
    }

    public static boolean runtime_process__process_state_input_close(Object handle) {
        String operation = symbol("process_state_input_close", 1);
        ProcessState state = (ProcessState) handle;
        if (state.closed || state.inputClosed) {
            return true;
        }
        if (state.process == null) {
            throw failure("invalid_input", operation, "Process has not been started");
        }
        state.inputClosed = true;
        try {
            state.input.close();
        } catch (IOException error) {
            // как close(2) после EPIPE: конец канала освобождён
            if (!brokenPipe(error)) {
                throw hostFailure(operation, "Cannot close pipe endpoint", error);
            }
        }
        return true;
    }

    private static List<Object> read(InputStream stream, BigInteger limit, String operation) {
        if (limit.signum() <= 0) {
            throw failure("invalid_input", operation, "Process read size must be positive");
        }
        int size = limit.compareTo(BigInteger.valueOf(CHUNK)) > 0 ? CHUNK : limit.intValue();
        byte[] buffer = new byte[size];
        try {
            int count = stream.read(buffer, 0, size);
            return Natives.fromBytes(buffer, Math.max(count, 0));
        } catch (IOException error) {
            throw hostFailure(operation, "Cannot read process output", error);
        }
    }

    public static List<Object> runtime_process__process_state_output_read(Object handle, BigInteger limit) {
        String operation = symbol("process_state_output_read", 2);
        return read(started(handle, operation).output, limit, operation);
    }

    public static List<Object> runtime_process__process_state_errors_read(Object handle, BigInteger limit) {
        String operation = symbol("process_state_errors_read", 2);
        return read(started(handle, operation).errors, limit, operation);
    }

    // ---- захват ----

    /** Общий захват: ввод в первый процесс, выводы, переполнение, статусы. */
    private static final class Capture {
        final List<Process> processes;
        final OutputStream input;
        final InputStream output;
        final List<InputStream> errors;
        final byte[] payload;
        final long limit;
        final ByteArrayOutputStream outputBytes = new ByteArrayOutputStream();
        final List<ByteArrayOutputStream> errorBytes = new ArrayList<ByteArrayOutputStream>();
        int overflowStream;
        int overflowIndex = -1;
        IOException problem;

        Capture(List<Process> processes, OutputStream input, InputStream output, List<InputStream> errors,
                byte[] payload, long limit) {
            this.processes = processes;
            this.input = input;
            this.output = output;
            this.errors = errors;
            this.payload = payload;
            this.limit = limit;
            for (int index = 0; index < errors.size(); index++) {
                errorBytes.add(new ByteArrayOutputStream());
            }
        }

        synchronized void overflow(int stream, int index) {
            if (overflowStream != 0) {
                return;
            }
            overflowStream = stream;
            overflowIndex = index;
            closeQuietly(input);
            for (Process process : processes) {
                process.destroyForcibly();
            }
        }

        synchronized void fail(IOException error) {
            if (problem == null) {
                problem = error;
            }
            closeQuietly(input);
            for (Process process : processes) {
                process.destroyForcibly();
            }
        }

        Thread drain(final InputStream stream, final ByteArrayOutputStream target, final int kind, final int index) {
            Thread thread = new Thread(new Runnable() {
                public void run() {
                    byte[] buffer = new byte[CHUNK];
                    try {
                        while (true) {
                            int count = stream.read(buffer);
                            if (count < 0) {
                                break;
                            }
                            synchronized (target) {
                                target.write(buffer, 0, count);
                                if (target.size() > limit) {
                                    overflow(kind, index);
                                }
                            }
                        }
                    } catch (IOException error) {
                        fail(error);
                    } finally {
                        closeQuietly(stream);
                    }
                }
            }, "okkama-capture");
            thread.setDaemon(true);
            thread.start();
            return thread;
        }

        void run() throws IOException {
            ArrayList<Thread> threads = new ArrayList<Thread>();
            threads.add(drain(output, outputBytes, 1, processes.size() - 1));
            for (int index = 0; index < errors.size(); index++) {
                threads.add(drain(errors.get(index), errorBytes.get(index), 2, index));
            }
            try {
                int offset = 0;
                while (offset < payload.length && overflowStream == 0) {
                    int size = Math.min(CHUNK, payload.length - offset);
                    input.write(payload, offset, size);
                    input.flush();
                    offset += size;
                }
            } catch (IOException error) {
                if (!brokenPipe(error) && overflowStream == 0) {
                    fail(error);
                }
            }
            closeQuietly(input);
            for (Thread thread : threads) {
                join(thread);
            }
            for (Process process : processes) {
                if (problem != null) {
                    process.destroyForcibly();
                }
                reap(process);
            }
            if (problem != null) {
                throw problem;
            }
        }

        private static void join(Thread thread) {
            while (true) {
                try {
                    thread.join();
                    return;
                } catch (InterruptedException ignored) {
                    // ждём дальше
                }
            }
        }
    }

    private static Future capture(final Capture capture, final String operation, final String message,
            final Runnable finish, final java.util.concurrent.Callable<Object> result) {
        final Future promise = Future.promise();
        final Scheduler.Op op = Scheduler.offload(new Callable<Object>() {
            public Object call() throws IOException {
                capture.run();
                return null;
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                finish.run();
                if (error != null) {
                    Scheduler.settle(promise, null, hostFailure(operation, message, error));
                    return;
                }
                try {
                    Scheduler.settle(promise, result.call(), null);
                } catch (Exception problem) {
                    Scheduler.settle(promise, null, hostFailure(operation, message, problem));
                }
            }
        });
        promise.listen(new Runnable() {
            public void run() {
                op.detach();
            }
        });
        return promise;
    }

    public static Future runtime_process__process_state_capture(Object handle, List<?> input, BigInteger limit) {
        final String operation = symbol("process_state_capture", 3);
        final ProcessState state;
        try {
            if (limit.signum() <= 0) {
                throw failure("invalid_input", operation, "Capture limit must be positive");
            }
            if (((ProcessState) handle).capturing) {
                throw failure("invalid_input", operation, "Process capture is already active");
            }
            state = started(handle, operation);
            if (state.inputClosed) {
                throw failure("invalid_input", operation, "Process input is unavailable");
            }
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
        state.capturing = true;
        state.inputClosed = true;
        long bound = limit.bitLength() > 62 ? Long.MAX_VALUE : limit.longValue();
        final List<Process> processes = java.util.Collections.singletonList(state.process);
        final Capture capture = new Capture(processes, state.input, state.output,
            java.util.Collections.singletonList(state.errors), Natives.toBytes(input), bound);
        return capture(capture, operation, "Cannot capture process output", new Runnable() {
            public void run() {
                close(state);
            }
        }, new Callable<Object>() {
            public Object call() {
                Captured captured = new Captured();
                captured.status = project(state.process);
                captured.overflow = capture.overflowStream;
                captured.output = capture.outputBytes.toByteArray();
                captured.errors = capture.errorBytes.get(0).toByteArray();
                return captured;
            }
        });
    }

    public static Map<Object, Object> runtime_process__captured_process_status(Object handle) {
        Map<Object, Object> out = Rt.newMap();
        out.putAll(((Captured) handle).status);
        return out;
    }

    public static BigInteger runtime_process__captured_process_overflow(Object handle) {
        return BigInteger.valueOf(((Captured) handle).overflow);
    }

    public static List<Object> runtime_process__captured_process_output(Object handle) {
        byte[] bytes = ((Captured) handle).output;
        return Natives.fromBytes(bytes, bytes.length);
    }

    public static List<Object> runtime_process__captured_process_errors(Object handle) {
        byte[] bytes = ((Captured) handle).errors;
        return Natives.fromBytes(bytes, bytes.length);
    }

    // ---- конвейер ----

    public static Object runtime_process__pipeline_state_create() {
        return new PipelineState();
    }

    public static boolean runtime_process__pipeline_state_add(Object handle, String program, List<?> arguments,
            String directory, Map<?, ?> environment) {
        String operation = symbol("pipeline_state_add", 5);
        PipelineState state = (PipelineState) handle;
        if (state.closed) {
            throw failure("already_closed", operation, "Pipeline is already closed");
        }
        if (state.started) {
            throw failure("invalid_input", operation, "Pipeline has already started");
        }
        state.commands.add(command("Pipeline", program, arguments, directory, environment, operation));
        return true;
    }

    /** {@code ProcessBuilder.startPipeline} (Java 9+): настоящие каналы ОС между командами. */
    @SuppressWarnings("unchecked")
    private static List<Process> startPipeline(List<ProcessBuilder> builders, String operation) throws IOException {
        try {
            return (List<Process>) ProcessBuilder.class.getMethod("startPipeline", List.class).invoke(null, builders);
        } catch (java.lang.reflect.InvocationTargetException error) {
            Throwable cause = error.getCause();
            if (cause instanceof IOException) {
                throw (IOException) cause;
            }
            throw new IOException(cause);
        } catch (ReflectiveOperationException error) {
            throw failure("unsupported", operation, "Process pipelines require Java 9 or newer");
        }
    }

    private static void closePipeline(PipelineState state) {
        if (state.closed) {
            return;
        }
        state.closed = true;
        if (state.processes != null) {
            for (Process process : state.processes) {
                kill(process);
            }
        }
    }

    public static Future runtime_process__pipeline_state_capture(Object handle, List<?> input, BigInteger limit) {
        final String operation = symbol("pipeline_state_capture", 3);
        final PipelineState state = (PipelineState) handle;
        final List<Process> processes;
        try {
            if (limit.signum() <= 0) {
                throw failure("invalid_input", operation, "Capture limit must be positive");
            }
            if (state.capturing) {
                throw failure("invalid_input", operation, "Pipeline capture is already active");
            }
            if (state.closed) {
                throw failure("already_closed", operation, "Pipeline is already closed");
            }
            if (state.commands.isEmpty()) {
                throw failure("invalid_input", operation, "Pipeline must contain a command");
            }
            try {
                processes = launch(state.commands, operation);
            } catch (IOException error) {
                throw hostFailure(operation, "Cannot configure or execute pipeline command", error);
            }
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
        state.started = true;
        state.capturing = true;
        state.processes = processes;
        ArrayList<InputStream> errors = new ArrayList<InputStream>();
        for (Process process : processes) {
            errors.add(process.getErrorStream());
        }
        long bound = limit.bitLength() > 62 ? Long.MAX_VALUE : limit.longValue();
        final Capture capture = new Capture(processes, processes.get(0).getOutputStream(),
            processes.get(processes.size() - 1).getInputStream(), errors, Natives.toBytes(input), bound);
        return capture(capture, operation, "Cannot capture pipeline output", new Runnable() {
            public void run() {
                closePipeline(state);
            }
        }, new Callable<Object>() {
            public Object call() {
                CapturedPipeline captured = new CapturedPipeline();
                captured.statuses = new ArrayList<Object>();
                for (Process process : processes) {
                    captured.statuses.add(project(process));
                }
                captured.overflowStream = capture.overflowStream;
                captured.overflowIndex = capture.overflowIndex;
                captured.output = capture.outputBytes.toByteArray();
                captured.errors = new ArrayList<byte[]>();
                for (ByteArrayOutputStream bytes : capture.errorBytes) {
                    captured.errors.add(bytes.toByteArray());
                }
                return captured;
            }
        });
    }

    public static List<Object> runtime_process__captured_pipeline_statuses(Object handle) {
        ArrayList<Object> out = new ArrayList<Object>();
        for (Object status : ((CapturedPipeline) handle).statuses) {
            Map<Object, Object> copy = Rt.newMap();
            copy.putAll((Map<?, ?>) status);
            out.add(copy);
        }
        return out;
    }

    public static BigInteger runtime_process__captured_pipeline_overflow_stream(Object handle) {
        return BigInteger.valueOf(((CapturedPipeline) handle).overflowStream);
    }

    public static BigInteger runtime_process__captured_pipeline_overflow_index(Object handle) {
        return BigInteger.valueOf(((CapturedPipeline) handle).overflowIndex);
    }

    public static List<Object> runtime_process__captured_pipeline_output(Object handle) {
        byte[] bytes = ((CapturedPipeline) handle).output;
        return Natives.fromBytes(bytes, bytes.length);
    }

    public static List<Object> runtime_process__captured_pipeline_errors(Object handle) {
        ArrayList<Object> out = new ArrayList<Object>();
        for (byte[] bytes : ((CapturedPipeline) handle).errors) {
            out.add(Natives.fromBytes(bytes, bytes.length));
        }
        return out;
    }
}
