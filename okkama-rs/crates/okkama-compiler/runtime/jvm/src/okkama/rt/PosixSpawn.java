package okkama.rt;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;

/**
 * Запуск потомков {@code posix_spawn} и собственный {@code waitpid} через
 * FFM: статус точный (выход с кодом или гибель от сигнала), сигналы — сам
 * {@code kill(2)}. {@code ProcessBuilder} JDK сообщает сигнал N как код
 * {@code 128 + N} и отличить его от обычного выхода не даёт.
 */
public final class PosixSpawn {
    private PosixSpawn() {}

    private static final class Api {
        static final MethodHandle PIPE = Posix.errnoFunction("pipe", FunctionDescriptor.of(Posix.INT, Posix.PTR));
        static final MethodHandle CLOSE = Posix.function("close", FunctionDescriptor.of(Posix.INT, Posix.INT));
        static final MethodHandle FCNTL = Posix.function("fcntl",
            FunctionDescriptor.of(Posix.INT, Posix.INT, Posix.INT, Posix.INT), Linker.Option.firstVariadicArg(2));
        static final MethodHandle READ = Posix.errnoFunction("read",
            FunctionDescriptor.of(Posix.LONG, Posix.INT, Posix.PTR, Posix.LONG));
        static final MethodHandle WRITE = Posix.errnoFunction("write",
            FunctionDescriptor.of(Posix.LONG, Posix.INT, Posix.PTR, Posix.LONG));
        static final MethodHandle WAITPID = Posix.errnoFunction("waitpid",
            FunctionDescriptor.of(Posix.INT, Posix.INT, Posix.PTR, Posix.INT));
        static final MethodHandle KILL = Posix.errnoFunction("kill", FunctionDescriptor.of(Posix.INT, Posix.INT, Posix.INT));
        static final MethodHandle SPAWN = Posix.function("posix_spawn",
            FunctionDescriptor.of(Posix.INT, Posix.PTR, Posix.PTR, Posix.PTR, Posix.PTR, Posix.PTR, Posix.PTR));
        static final MethodHandle ACTIONS_INIT = Posix.function("posix_spawn_file_actions_init",
            FunctionDescriptor.of(Posix.INT, Posix.PTR));
        static final MethodHandle ACTIONS_DESTROY = Posix.function("posix_spawn_file_actions_destroy",
            FunctionDescriptor.of(Posix.INT, Posix.PTR));
        static final MethodHandle DUP2 = Posix.function("posix_spawn_file_actions_adddup2",
            FunctionDescriptor.of(Posix.INT, Posix.PTR, Posix.INT, Posix.INT));
        static final MethodHandle ADDCLOSE = Posix.function("posix_spawn_file_actions_addclose",
            FunctionDescriptor.of(Posix.INT, Posix.PTR, Posix.INT));
        static final MethodHandle CHDIR = Posix.function("posix_spawn_file_actions_addchdir_np",
            FunctionDescriptor.of(Posix.INT, Posix.PTR, Posix.PTR));
        static final MethodHandle CLOSEFROM = Posix.MAC ? null : Posix.function("posix_spawn_file_actions_addclosefrom_np",
            FunctionDescriptor.of(Posix.INT, Posix.PTR, Posix.INT));
        static final MethodHandle ATTR_INIT = Posix.function("posix_spawnattr_init", FunctionDescriptor.of(Posix.INT, Posix.PTR));
        static final MethodHandle ATTR_DESTROY = Posix.function("posix_spawnattr_destroy",
            FunctionDescriptor.of(Posix.INT, Posix.PTR));
        static final MethodHandle SET_FLAGS = Posix.function("posix_spawnattr_setflags",
            FunctionDescriptor.of(Posix.INT, Posix.PTR, Posix.SHORT));
        static final MethodHandle SET_DEFAULT = Posix.function("posix_spawnattr_setsigdefault",
            FunctionDescriptor.of(Posix.INT, Posix.PTR, Posix.PTR));
        static final MethodHandle SET_MASK = Posix.function("posix_spawnattr_setsigmask",
            FunctionDescriptor.of(Posix.INT, Posix.PTR, Posix.PTR));
        static final MethodHandle FILL = Posix.function("sigfillset", FunctionDescriptor.of(Posix.INT, Posix.PTR));
        static final MethodHandle EMPTY = Posix.function("sigemptyset", FunctionDescriptor.of(Posix.INT, Posix.PTR));
    }

    private static final int F_SETFD = 2;
    private static final int FD_CLOEXEC = 1;
    private static final short SETSIGDEF = 0x04;
    private static final short SETSIGMASK = 0x08;
    /** macOS: потомок наследует только явно назначенные дескрипторы. */
    private static final short CLOEXEC_DEFAULT = 0x4000;

    /** Команда потомка: argv (путь уже найден), каталог, окружение. */
    static final class Spec {
        final List<String> argv;
        final String directory;
        final Map<String, String> environment;

        Spec(List<String> argv, String directory, Map<String, String> environment) {
            this.argv = argv;
            this.directory = directory;
            this.environment = environment;
        }
    }

    private static IOException failure(String what, int code) {
        return new IOException(what + ": error=" + code + ", " + describe(code));
    }

    private static String describe(int errno) {
        switch (errno) {
            case 2:
                return "No such file or directory";
            case 13:
                return "Permission denied";
            case 20:
                return "Not a directory";
            case 8:
                return "Exec format error";
            default:
                return "errno " + errno;
        }
    }

    private static int[] pipe() throws Throwable {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment state = Posix.state(arena);
            MemorySegment fds = arena.allocate(Posix.INT, 2);
            if ((int) Api.PIPE.invokeExact(state, fds) != 0) {
                throw failure("pipe", Posix.errno(state));
            }
            int[] out = {fds.getAtIndex(Posix.INT, 0), fds.getAtIndex(Posix.INT, 1)};
            for (int fd : out) {
                int ignored = (int) Api.FCNTL.invokeExact(fd, F_SETFD, FD_CLOEXEC);
            }
            return out;
        }
    }

    private static void close(int fd) {
        if (fd < 0) {
            return;
        }
        try {
            int ignored = (int) Api.CLOSE.invokeExact(fd);
        } catch (Throwable error) {
            // освобождение
        }
    }

    private static MemorySegment strings(Arena arena, List<String> values) {
        MemorySegment array = arena.allocate(Posix.PTR, values.size() + 1);
        for (int index = 0; index < values.size(); index++) {
            array.setAtIndex(Posix.PTR, index, arena.allocateFrom(values.get(index)));
        }
        array.setAtIndex(Posix.PTR, values.size(), MemorySegment.NULL);
        return array;
    }

    /** Запустить потомка с stdin/stdout/stderr на данных дескрипторах. */
    private static int spawn(Spec spec, int input, int output, int errors) throws Throwable {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment actions = arena.allocate(512);
            MemorySegment attributes = arena.allocate(512);
            check((int) Api.ACTIONS_INIT.invokeExact(actions), "posix_spawn_file_actions_init");
            check((int) Api.ATTR_INIT.invokeExact(attributes), "posix_spawnattr_init");
            try {
                check((int) Api.DUP2.invokeExact(actions, input, 0), "adddup2");
                check((int) Api.DUP2.invokeExact(actions, output, 1), "adddup2");
                check((int) Api.DUP2.invokeExact(actions, errors, 2), "adddup2");
                if (Api.CLOSEFROM != null) {
                    check((int) Api.CLOSEFROM.invokeExact(actions, 3), "addclosefrom_np");
                }
                check((int) Api.CHDIR.invokeExact(actions, arena.allocateFrom(spec.directory)), "addchdir_np");
                // Сигналы по умолчанию и пустая маска: JVM игнорирует SIGPIPE.
                MemorySegment all = arena.allocate(256);
                MemorySegment none = arena.allocate(256);
                int ignored = (int) Api.FILL.invokeExact(all);
                ignored = (int) Api.EMPTY.invokeExact(none);
                check((int) Api.SET_DEFAULT.invokeExact(attributes, all), "setsigdefault");
                check((int) Api.SET_MASK.invokeExact(attributes, none), "setsigmask");
                short flags = (short) (SETSIGDEF | SETSIGMASK | (Posix.MAC ? CLOEXEC_DEFAULT : 0));
                check((int) Api.SET_FLAGS.invokeExact(attributes, flags), "setflags");
                ArrayList<String> environment = new ArrayList<String>();
                for (Map.Entry<String, String> entry : spec.environment.entrySet()) {
                    environment.add(entry.getKey() + "=" + entry.getValue());
                }
                MemorySegment pid = arena.allocate(Posix.INT);
                int result = (int) Api.SPAWN.invokeExact(pid, arena.allocateFrom(spec.argv.get(0)), actions, attributes,
                    strings(arena, spec.argv), strings(arena, environment));
                if (result != 0) {
                    throw failure("Cannot run program \"" + spec.argv.get(0) + "\"", result);
                }
                return pid.get(Posix.INT, 0);
            } finally {
                int ignored = (int) Api.ACTIONS_DESTROY.invokeExact(actions);
                ignored = (int) Api.ATTR_DESTROY.invokeExact(attributes);
            }
        }
    }

    private static void check(int result, String what) throws IOException {
        if (result != 0) {
            throw failure(what, result);
        }
    }

    /** Конвейер команд: argv, каталог и окружение каждой — по индексу. */
    public static List<Process> start(List<List<String>> argvs, List<String> directories,
            List<Map<String, String>> environments) throws IOException {
        ArrayList<Spec> specs = new ArrayList<Spec>();
        for (int index = 0; index < argvs.size(); index++) {
            specs.add(new Spec(argvs.get(index), directories.get(index), environments.get(index)));
        }
        return pipeline(specs);
    }

    /** Конвейер: stdin первого, stdout последнего и stderr каждого — каналы родителя. */
    public static List<Process> pipeline(List<Spec> specs) throws IOException {
        ArrayList<int[]> opened = new ArrayList<int[]>();
        ArrayList<Child> children = new ArrayList<Child>();
        try {
            int[] input = pipe();
            opened.add(input);
            // Читающий конец, который станет stdin следующей команды.
            int[] previous = input;
            for (int index = 0; index < specs.size(); index++) {
                boolean last = index == specs.size() - 1;
                int[] link = pipe();
                opened.add(link);
                int[] errors = pipe();
                opened.add(errors);
                int pid = spawn(specs.get(index), previous[0], link[1], errors[1]);
                Child child = new Child(pid);
                child.errors = new FdInput(errors[0]);
                errors[0] = -1;
                close(errors[1]);
                errors[1] = -1;
                close(link[1]);
                link[1] = -1;
                if (previous != input) {
                    close(previous[0]);
                    previous[0] = -1;
                }
                previous = link;
                if (last) {
                    child.output = new FdInput(link[0]);
                    link[0] = -1;
                }
                children.add(child);
            }
            close(input[0]);
            input[0] = -1;
            Child first = children.get(0);
            first.input = new FdOutput(input[1]);
            input[1] = -1;
            for (Child child : children) {
                if (child.input == null) {
                    child.input = new FdOutput(-1);
                }
                if (child.output == null) {
                    child.output = new FdInput(-1);
                }
                child.reap();
            }
            return new ArrayList<Process>(children);
        } catch (IOException error) {
            for (Child child : children) {
                child.destroyForcibly();
                child.reap();
            }
            throw error;
        } catch (Throwable error) {
            for (Child child : children) {
                child.destroyForcibly();
                child.reap();
            }
            throw new IOException(error);
        } finally {
            for (int[] fds : opened) {
                close(fds[0]);
                close(fds[1]);
            }
        }
    }

    /** Чтение дескриптора канала. */
    static final class FdInput extends InputStream {
        private int fd;

        FdInput(int fd) {
            this.fd = fd;
        }

        @Override
        public int read() throws IOException {
            byte[] one = new byte[1];
            int count = read(one, 0, 1);
            return count < 0 ? -1 : one[0] & 0xff;
        }

        @Override
        public int read(byte[] buffer, int offset, int length) throws IOException {
            if (fd < 0) {
                return -1;
            }
            if (length == 0) {
                return 0;
            }
            try (Arena arena = Arena.ofConfined()) {
                MemorySegment state = Posix.state(arena);
                MemorySegment bytes = arena.allocate(length);
                while (true) {
                    long count = (long) Api.READ.invokeExact(state, fd, bytes, (long) length);
                    if (count >= 0) {
                        if (count == 0) {
                            return -1;
                        }
                        MemorySegment.copy(bytes, ValueLayout.JAVA_BYTE, 0, buffer, offset, (int) count);
                        return (int) count;
                    }
                    int errno = Posix.errno(state);
                    if (errno != 4) {
                        throw failure("read", errno);
                    }
                }
            } catch (IOException error) {
                throw error;
            } catch (Throwable error) {
                throw new IOException(error);
            }
        }

        @Override
        public synchronized void close() {
            PosixSpawn.close(fd);
            fd = -1;
        }
    }

    /** Запись в дескриптор канала; разрыв — {@code Broken pipe}. */
    static final class FdOutput extends OutputStream {
        private int fd;

        FdOutput(int fd) {
            this.fd = fd;
        }

        @Override
        public void write(int value) throws IOException {
            write(new byte[] {(byte) value}, 0, 1);
        }

        @Override
        public void write(byte[] buffer, int offset, int length) throws IOException {
            if (fd < 0) {
                throw new IOException("Stream closed");
            }
            try (Arena arena = Arena.ofConfined()) {
                MemorySegment state = Posix.state(arena);
                MemorySegment bytes = arena.allocate(Math.max(1, length));
                MemorySegment.copy(buffer, offset, bytes, ValueLayout.JAVA_BYTE, 0, length);
                long done = 0;
                while (done < length) {
                    long count = (long) Api.WRITE.invokeExact(state, fd, bytes.asSlice(done), length - done);
                    if (count >= 0) {
                        done += count;
                        continue;
                    }
                    int errno = Posix.errno(state);
                    if (errno == 32) {
                        throw new IOException("Broken pipe");
                    }
                    if (errno != 4) {
                        throw failure("write", errno);
                    }
                }
            } catch (IOException error) {
                throw error;
            } catch (Throwable error) {
                throw new IOException(error);
            }
        }

        @Override
        public synchronized void close() {
            PosixSpawn.close(fd);
            fd = -1;
        }
    }

    /** Потомок: статус собирает поток {@code waitpid}. */
    static final class Child extends Process implements ProcessNatives.Exact {
        final int pid;
        OutputStream input;
        InputStream output;
        InputStream errors;
        private boolean done;
        private int raw;

        Child(int pid) {
            this.pid = pid;
        }

        void reap() {
            Thread thread = new Thread(() -> {
                try (Arena arena = Arena.ofConfined()) {
                    MemorySegment state = Posix.state(arena);
                    MemorySegment status = arena.allocate(Posix.INT);
                    while (true) {
                        int result = (int) Api.WAITPID.invokeExact(state, pid, status, 0);
                        if (result == pid) {
                            finish(status.get(Posix.INT, 0));
                            return;
                        }
                        if (Posix.errno(state) != 4) {
                            finish(0);
                            return;
                        }
                    }
                } catch (Throwable error) {
                    finish(0);
                }
            }, "okkama-reap-" + pid);
            thread.setDaemon(true);
            thread.start();
        }

        private synchronized void finish(int status) {
            raw = status;
            done = true;
            notifyAll();
        }

        @Override
        public OutputStream getOutputStream() {
            return input;
        }

        @Override
        public InputStream getInputStream() {
            return output;
        }

        @Override
        public InputStream getErrorStream() {
            return errors;
        }

        @Override
        public synchronized int waitFor() throws InterruptedException {
            while (!done) {
                wait();
            }
            return exitValue();
        }

        @Override
        public synchronized boolean waitFor(long timeout, TimeUnit unit) throws InterruptedException {
            long deadline = System.nanoTime() + unit.toNanos(timeout);
            while (!done) {
                long left = deadline - System.nanoTime();
                if (left <= 0) {
                    return false;
                }
                TimeUnit.NANOSECONDS.timedWait(this, left);
            }
            return true;
        }

        @Override
        public synchronized int exitValue() {
            if (!done) {
                throw new IllegalThreadStateException("process has not exited");
            }
            return signaled() ? 128 + termination() : termination();
        }

        @Override
        public synchronized boolean isAlive() {
            return !done;
        }

        @Override
        public long pid() {
            return pid;
        }

        @Override
        public synchronized boolean signaled() {
            int low = raw & 0x7f;
            return done && low != 0 && low != 0x7f;
        }

        @Override
        public synchronized int termination() {
            return signaled() ? raw & 0x7f : (raw >> 8) & 0xff;
        }

        @Override
        public boolean signal(int number) {
            if (!isAlive()) {
                return false;
            }
            try (Arena arena = Arena.ofConfined()) {
                MemorySegment state = Posix.state(arena);
                return (int) Api.KILL.invokeExact(state, pid, number) == 0;
            } catch (Throwable error) {
                return false;
            }
        }

        @Override
        public void destroy() {
            signal(15);
        }

        @Override
        public Process destroyForcibly() {
            signal(9);
            return this;
        }
    }
}
