package okkama.rt;

import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.Callable;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * Natives {@code runtime/application} (POSIX-терминал): дескрипторы
 * stdin/stdout/stderr или {@code /dev/tty}, режимы termios (стек аренд),
 * геометрия {@code TIOCGWINSZ} и {@code SIGWINCH}, — через FFM. Чтение
 * блокирующее на собственном потоке сессии (Perl-provider делает дескриптор
 * неблокирующим и ждёт готовности).
 */
public final class TerminalNatives {
    private TerminalNatives() {}

    private static final String DOMAIN = "core.application";

    private static final class Api {
        static final MethodHandle DUP = Posix.errnoFunction("dup", FunctionDescriptor.of(Posix.INT, Posix.INT));
        static final MethodHandle OPEN = Posix.errnoFunction("open",
            FunctionDescriptor.of(Posix.INT, Posix.PTR, Posix.INT, Posix.INT), Linker.Option.firstVariadicArg(2));
        static final MethodHandle CLOSE = Posix.function("close", FunctionDescriptor.of(Posix.INT, Posix.INT));
        static final MethodHandle ISATTY = Posix.function("isatty", FunctionDescriptor.of(Posix.INT, Posix.INT));
        static final MethodHandle READ = Posix.errnoFunction("read",
            FunctionDescriptor.of(Posix.LONG, Posix.INT, Posix.PTR, Posix.LONG));
        static final MethodHandle WRITE = Posix.errnoFunction("write",
            FunctionDescriptor.of(Posix.LONG, Posix.INT, Posix.PTR, Posix.LONG));
        static final MethodHandle TCGETATTR = Posix.errnoFunction("tcgetattr",
            FunctionDescriptor.of(Posix.INT, Posix.INT, Posix.PTR));
        static final MethodHandle TCSETATTR = Posix.errnoFunction("tcsetattr",
            FunctionDescriptor.of(Posix.INT, Posix.INT, Posix.INT, Posix.PTR));
        static final MethodHandle IOCTL = Posix.errnoFunction("ioctl",
            FunctionDescriptor.of(Posix.INT, Posix.INT, Posix.LONG, Posix.PTR), Linker.Option.firstVariadicArg(2));
    }

    // termios: macOS — tcflag_t/speed_t по 8 байт, NCCS 20; Linux — по 4, NCCS 32.
    private static final boolean MAC = Posix.MAC;
    private static final int SIZE = MAC ? 72 : 60;
    private static final int FLAG = MAC ? 8 : 4;
    private static final int CC = MAC ? 32 : 17;
    private static final int VMIN = MAC ? 16 : 6;
    private static final int VTIME = MAC ? 17 : 5;
    private static final long ECHO = 0x8;
    private static final long ECHONL = MAC ? 0x10 : 0x40;
    private static final long ICANON = MAC ? 0x100 : 0x2;
    private static final long ISIG = MAC ? 0x80 : 0x1;
    private static final long IEXTEN = MAC ? 0x400 : 0x8000;
    private static final long IGNBRK = 0x1;
    private static final long BRKINT = 0x2;
    private static final long PARMRK = 0x8;
    private static final long INPCK = 0x10;
    private static final long ISTRIP = 0x20;
    private static final long INLCR = 0x40;
    private static final long IGNCR = 0x80;
    private static final long ICRNL = 0x100;
    private static final long IXON = MAC ? 0x200 : 0x400;
    private static final long OPOST = 0x1;
    private static final long CSIZE = MAC ? 0x300 : 0x30;
    private static final long CS8 = MAC ? 0x300 : 0x30;
    private static final long PARENB = MAC ? 0x1000 : 0x100;
    private static final long TIOCGWINSZ = MAC ? 0x40087468L : 0x5413L;
    private static final int O_RDWR = 2;

    private static final HashSet<String> OWNERS = new HashSet<String>();

    static final class Mode {
        final int id;
        final byte[] before;
        boolean released;

        Mode(int id, byte[] before) {
            this.id = id;
            this.before = before;
        }
    }

    static final class Session {
        int input = -1;
        int output = -1;
        boolean controlling;
        boolean inputInteractive;
        boolean outputInteractive;
        byte[] original;
        boolean closed;
        final ArrayList<Mode> modes = new ArrayList<Mode>();
        int serial;
        byte[] cleanup = new byte[0];
        final ArrayList<String> claims = new ArrayList<String>();
        final ArrayList<Resizes> resizes = new ArrayList<Resizes>();
        ExecutorService reader;
        Future reading;
        byte[] pending;
    }

    static final class Lease {
        final Session session;
        final int id;
        boolean dropped;

        Lease(Session session, int id) {
            this.session = session;
            this.id = id;
        }
    }

    static final class Resizes {
        Session session;
        boolean closed;
        boolean notified;
        Future pending;
        Scheduler.Op op;
    }

    private static String symbol(String name, int arity) {
        return "runtime/application@1::" + name + "/" + arity;
    }

    private static Failure failure(String code, String operation, String message) {
        return NativeFailure.raise(DOMAIN, code, operation, message);
    }

    private static Failure errno(int errno, String operation, String message) {
        return failure(Posix.code(errno), operation, message);
    }

    private static Failure caught(Throwable error, String operation) {
        if (!(error instanceof Failure) && "1".equals(System.getenv("OKKAMA_TRACE_THROW"))) {
            error.printStackTrace();
        }
        return error instanceof Failure ? (Failure) error
            : failure("unexpected", operation, "Terminal provider operation failed");
    }

    // ---- открытие ----

    private static int dup(int fd, String operation) throws Throwable {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment state = Posix.state(arena);
            int copy = (int) Api.DUP.invokeExact(state, fd);
            if (copy < 0) {
                throw errno(Posix.errno(state), operation, "Cannot duplicate terminal descriptor");
            }
            return copy;
        }
    }

    private static boolean tty(int fd) throws Throwable {
        return (int) Api.ISATTY.invokeExact(fd) == 1;
    }

    private static Session opened(Session session, String[] keys, String operation) throws Throwable {
        synchronized (OWNERS) {
            for (String key : keys) {
                if (OWNERS.contains(key)) {
                    close(session);
                    throw failure("resource_exhausted", operation, "Terminal descriptor already belongs to an open session");
                }
            }
            for (String key : keys) {
                OWNERS.add(key);
                session.claims.add(key);
            }
        }
        session.inputInteractive = tty(session.input);
        session.outputInteractive = tty(session.output);
        if (session.inputInteractive) {
            session.original = snapshot(session, operation);
        }
        session.reader = Executors.newSingleThreadExecutor(work -> {
            Thread thread = new Thread(work, "okkama-terminal-read");
            thread.setDaemon(true);
            return thread;
        });
        return session;
    }

    private static void supported(String operation) {
        if (!Posix.SUPPORTED) {
            throw failure("unsupported", operation, "Terminal sessions are supported on macOS and Linux only");
        }
    }

    public static Future runtime_application__terminal_standard(BigInteger output) {
        String operation = symbol("terminal_standard", 1);
        try {
            supported(operation);
            int kind = output.bitLength() > 31 ? -1 : output.intValue();
            if (kind != 0 && kind != 1) {
                throw failure("invalid_input", operation, "Terminal output must be stdout (0) or stderr (1)");
            }
            System.out.flush();
            System.err.flush();
            Session session = new Session();
            session.input = dup(0, operation);
            session.output = dup(kind == 0 ? 1 : 2, operation);
            return Scheduler.resolved(opened(session, new String[] {"fd:0", "fd:" + (kind + 1)}, operation), null);
        } catch (Throwable error) {
            return Scheduler.resolved(null, caught(error, operation));
        }
    }

    public static Future runtime_application__terminal_controlling() {
        String operation = symbol("terminal_controlling", 0);
        try (Arena arena = Arena.ofConfined()) {
            supported(operation);
            MemorySegment state = Posix.state(arena);
            int fd = (int) Api.OPEN.invokeExact(state, arena.allocateFrom("/dev/tty"), O_RDWR, 0);
            if (fd < 0) {
                throw failure("not_found", operation, "No controlling terminal is available");
            }
            Session session = new Session();
            session.input = fd;
            session.output = dup(fd, operation);
            session.controlling = true;
            return Scheduler.resolved(opened(session, new String[] {"tty"}, operation), null);
        } catch (Throwable error) {
            return Scheduler.resolved(null, caught(error, operation));
        }
    }

    private static Session session(Object handle, String operation) {
        Session session = (Session) handle;
        if (session.closed) {
            throw failure("already_closed", operation, "Terminal session is closed");
        }
        return session;
    }

    // ---- геометрия и метаданные ----

    private static int[] size(Session session, String operation) throws Throwable {
        if (!session.outputInteractive) {
            return new int[] {0, 0};
        }
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment state = Posix.state(arena);
            MemorySegment winsize = arena.allocate(8);
            int result = (int) Api.IOCTL.invokeExact(state, session.output, TIOCGWINSZ, winsize);
            if (result < 0) {
                throw errno(Posix.errno(state), operation, "Cannot observe terminal geometry");
            }
            int rows = winsize.get(Posix.SHORT, 0) & 0xffff;
            int columns = winsize.get(Posix.SHORT, 2) & 0xffff;
            return new int[] {columns, rows};
        }
    }

    public static Map<Object, Object> runtime_application__terminal_metadata(Object handle) {
        String operation = symbol("terminal_metadata", 1);
        Session session = session(handle, operation);
        try {
            int[] size = size(session, operation);
            Map<Object, Object> out = Rt.newMap();
            out.put("colors", BigInteger.valueOf(session.outputInteractive ? 1 : 0));
            out.put("columns", BigInteger.valueOf(size[0]));
            out.put("controlling", BigInteger.valueOf(session.controlling ? 1 : 0));
            out.put("input", BigInteger.valueOf(session.inputInteractive ? 1 : 0));
            out.put("output", BigInteger.valueOf(session.outputInteractive ? 1 : 0));
            out.put("rows", BigInteger.valueOf(size[1]));
            return out;
        } catch (Throwable error) {
            throw caught(error, operation);
        }
    }

    // ---- ввод-вывод ----

    private static long write(int fd, byte[] bytes, String operation) throws Throwable {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment state = Posix.state(arena);
            MemorySegment buffer = arena.allocateFrom(ValueLayout.JAVA_BYTE, bytes);
            while (true) {
                long count = (long) Api.WRITE.invokeExact(state, fd, buffer, (long) bytes.length);
                if (count >= 0) {
                    return count;
                }
                int code = Posix.errno(state);
                if (code != 4) {
                    throw errno(code, operation, "Cannot write terminal output");
                }
            }
        }
    }

    public static BigInteger runtime_application__terminal_write(Object handle, List<?> source) {
        String operation = symbol("terminal_write", 2);
        Session session = session(handle, operation);
        byte[] bytes = Natives.toBytes(source);
        if (bytes.length > 65536) {
            throw failure("invalid_input", operation, "Terminal write exceeds 64 KiB");
        }
        if (bytes.length == 0) {
            return BigInteger.ZERO;
        }
        try {
            return BigInteger.valueOf(write(session.output, bytes, operation));
        } catch (Throwable error) {
            throw caught(error, operation);
        }
    }

    public static Future runtime_application__terminal_read(Object handle, BigInteger limitValue) {
        final String operation = symbol("terminal_read", 2);
        final Session session;
        try {
            session = session(handle, operation);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
        if (limitValue.signum() <= 0 || limitValue.compareTo(BigInteger.valueOf(65536)) > 0) {
            return Scheduler.resolved(null, failure("invalid_input", operation, "Terminal read limit must be 1..65536"));
        }
        if (session.reading != null) {
            return Scheduler.resolved(null, failure("resource_exhausted", operation, "Terminal already has a pending read"));
        }
        final int limit = limitValue.intValue();
        if (session.pending != null) {
            byte[] bytes = session.pending;
            int count = Math.min(limit, bytes.length);
            session.pending = count == bytes.length ? null : java.util.Arrays.copyOfRange(bytes, count, bytes.length);
            return Scheduler.resolved(Natives.fromBytes(bytes, count), null);
        }
        final Future promise = Future.promise();
        session.reading = promise;
        final int fd = session.input;
        final Scheduler.Op op = Scheduler.offload(session.reader, new Callable<Object>() {
            public Object call() throws Exception {
                try (Arena arena = Arena.ofConfined()) {
                    MemorySegment state = Posix.state(arena);
                    MemorySegment buffer = arena.allocate(limit);
                    while (true) {
                        long count;
                        try {
                            count = (long) Api.READ.invokeExact(state, fd, buffer, (long) limit);
                        } catch (Throwable error) {
                            throw new java.io.IOException(error);
                        }
                        if (count >= 0) {
                            return buffer.asSlice(0, count).toArray(ValueLayout.JAVA_BYTE);
                        }
                        int code = Posix.errno(state);
                        if (code != 4) {
                            throw errno(code, operation, "Cannot read terminal input");
                        }
                    }
                }
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                if (promise.state != Future.RUNNING) {
                    if (value != null && ((byte[]) value).length > 0) {
                        session.pending = (byte[]) value;
                    }
                    return;
                }
                if (error != null) {
                    Scheduler.settle(promise, null, session.closed
                        ? failure("already_closed", operation, "Terminal session is closed") : caught(error, operation));
                    return;
                }
                byte[] bytes = (byte[]) value;
                Scheduler.settle(promise, Natives.fromBytes(bytes, bytes.length), null);
            }
        });
        promise.listen(new Runnable() {
            public void run() {
                session.reading = null;
                op.detach();
            }
        });
        return promise;
    }

    public static boolean runtime_application__terminal_cleanup(Object handle, List<?> bytes) {
        String operation = symbol("terminal_cleanup", 2);
        Session session = session(handle, operation);
        if (bytes.size() > 256) {
            throw failure("invalid_input", operation, "Terminal cleanup exceeds 256 bytes");
        }
        session.cleanup = Natives.toBytes(bytes);
        return true;
    }

    public static boolean runtime_application__terminal_closed(Object handle) {
        return ((Session) handle).closed;
    }

    private static void close(Session session) throws Throwable {
        if (session.closed) {
            return;
        }
        session.closed = true;
        Throwable first = null;
        for (Resizes source : new ArrayList<Resizes>(session.resizes)) {
            closeResizes(source);
        }
        String operation = symbol("terminal_close", 1);
        if (session.reading != null) {
            Scheduler.settle(session.reading, null, failure("already_closed", operation, "Terminal session is closed"));
        }
        if (session.cleanup.length > 0 && session.output >= 0) {
            try {
                if (write(session.output, session.cleanup, operation) != session.cleanup.length) {
                    throw failure("unexpected", operation, "Terminal screen restoration was incomplete");
                }
            } catch (Throwable error) {
                first = error;
            }
        }
        if (session.original != null) {
            try {
                apply(session, session.original, operation);
            } catch (Throwable error) {
                first = first == null ? error : first;
            }
        }
        session.modes.clear();
        for (int fd : new int[] {session.input, session.output}) {
            if (fd >= 0) {
                int ignored = (int) Api.CLOSE.invokeExact(fd);
            }
        }
        if (session.reader != null) {
            session.reader.shutdownNow();
        }
        synchronized (OWNERS) {
            OWNERS.removeAll(session.claims);
        }
        session.claims.clear();
        if (first != null) {
            throw first;
        }
    }

    public static boolean runtime_application__terminal_close(Object handle) {
        try {
            close((Session) handle);
            return true;
        } catch (Throwable error) {
            throw caught(error, symbol("terminal_close", 1));
        }
    }

    // ---- режимы ----

    private static byte[] snapshot(Session session, String operation) throws Throwable {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment state = Posix.state(arena);
            MemorySegment term = arena.allocate(SIZE);
            if ((int) Api.TCGETATTR.invokeExact(state, session.input, term) < 0) {
                throw errno(Posix.errno(state), operation, "Cannot observe terminal mode");
            }
            return term.toArray(ValueLayout.JAVA_BYTE);
        }
    }

    private static void apply(Session session, byte[] snapshot, String operation) throws Throwable {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment state = Posix.state(arena);
            MemorySegment term = arena.allocateFrom(ValueLayout.JAVA_BYTE, snapshot);
            if ((int) Api.TCSETATTR.invokeExact(state, session.input, 0, term) < 0) {
                throw errno(Posix.errno(state), operation, "Cannot apply terminal mode");
            }
        }
    }

    private static long flag(MemorySegment term, int index) {
        return FLAG == 8 ? term.get(ValueLayout.JAVA_LONG_UNALIGNED, index * 8L)
            : term.get(ValueLayout.JAVA_INT_UNALIGNED, index * 4L) & 0xffffffffL;
    }

    private static void flag(MemorySegment term, int index, long value) {
        if (FLAG == 8) {
            term.set(ValueLayout.JAVA_LONG_UNALIGNED, index * 8L, value);
        } else {
            term.set(ValueLayout.JAVA_INT_UNALIGNED, index * 4L, (int) value);
        }
    }

    public static Object runtime_application__terminal_mode_open(Object handle, BigInteger modeValue) {
        String operation = symbol("terminal_mode_open", 2);
        Session session = session(handle, operation);
        try {
            if (!session.inputInteractive) {
                throw failure("unsupported", operation, "Terminal modes require interactive input");
            }
            int mode = modeValue.bitLength() > 31 ? -1 : modeValue.intValue();
            if (mode < 0 || mode > 3) {
                throw failure("invalid_input", operation, "Unknown terminal mode");
            }
            byte[] before = snapshot(session, operation);
            byte[] profile = (mode == 0 || mode == 3) ? session.original : before;
            MemorySegment term = MemorySegment.ofArray(profile.clone());
            long iflag = flag(term, 0);
            long oflag = flag(term, 1);
            long cflag = flag(term, 2);
            long lflag = flag(term, 3);
            if (mode == 0 || mode == 3) {
                lflag |= ICANON | ISIG;
                iflag = (iflag & ~(IGNCR | INLCR)) | ICRNL;
                oflag |= OPOST;
                if (mode == 0) {
                    lflag |= ECHO;
                } else {
                    lflag &= ~(ECHO | ECHONL);
                }
            } else {
                lflag &= ~(ICANON | ECHO);
                if (mode == 1) {
                    lflag |= ISIG;
                }
                term.set(ValueLayout.JAVA_BYTE, CC + VMIN, (byte) 1);
                term.set(ValueLayout.JAVA_BYTE, CC + VTIME, (byte) 0);
                if (mode == 2) {
                    lflag &= ~(ISIG | IEXTEN);
                    iflag &= ~(IGNBRK | BRKINT | PARMRK | INLCR | IGNCR | ICRNL | INPCK | ISTRIP | IXON);
                    oflag &= ~OPOST;
                    cflag = (cflag & ~(CSIZE | PARENB)) | CS8;
                }
            }
            flag(term, 0, iflag);
            flag(term, 1, oflag);
            flag(term, 2, cflag);
            flag(term, 3, lflag);
            apply(session, term.toArray(ValueLayout.JAVA_BYTE), operation);
            Mode lease = new Mode(++session.serial, before);
            session.modes.add(lease);
            return new Lease(session, lease.id);
        } catch (Throwable error) {
            throw caught(error, operation);
        }
    }

    private static boolean modeClosed(Session session, int id) {
        for (Mode mode : session.modes) {
            if (mode.id == id && !mode.released) {
                return false;
            }
        }
        return true;
    }

    private static void modeClose(Session session, int id, boolean dropping, String operation) throws Throwable {
        if (session.closed || modeClosed(session, id)) {
            return;
        }
        if (!dropping && session.modes.get(session.modes.size() - 1).id != id) {
            throw failure("invalid_input", operation, "Terminal mode leases must close in reverse order");
        }
        for (Mode mode : session.modes) {
            if (mode.id == id) {
                mode.released = true;
            }
        }
        while (!session.modes.isEmpty() && session.modes.get(session.modes.size() - 1).released) {
            apply(session, session.modes.get(session.modes.size() - 1).before, operation);
            session.modes.remove(session.modes.size() - 1);
        }
    }

    public static boolean runtime_application__terminal_mode_closed(Object handle) {
        Lease lease = (Lease) handle;
        return lease.dropped || modeClosed(lease.session, lease.id);
    }

    public static boolean runtime_application__terminal_mode_close(Object handle) {
        String operation = symbol("terminal_mode_close", 1);
        Lease lease = (Lease) handle;
        if (lease.dropped) {
            return true;
        }
        try {
            modeClose(lease.session, lease.id, false, operation);
            lease.dropped = true;
            return true;
        } catch (Throwable error) {
            throw caught(error, operation);
        }
    }

    public static boolean runtime_application__terminal_mode_drop(Object handle) {
        String operation = symbol("terminal_mode_drop", 1);
        Lease lease = (Lease) handle;
        if (lease.dropped) {
            return true;
        }
        lease.dropped = true;
        try {
            modeClose(lease.session, lease.id, true, operation);
            return true;
        } catch (Throwable error) {
            throw caught(error, operation);
        }
    }

    // ---- SIGWINCH ----

    private static final ArrayList<Resizes> WATCHERS = new ArrayList<Resizes>();
    private static boolean installed;

    @SuppressWarnings("removal")
    private static synchronized void install() {
        if (installed) {
            return;
        }
        installed = true;
        final sun.misc.Signal signal = new sun.misc.Signal("WINCH");
        final sun.misc.SignalHandler[] prior = new sun.misc.SignalHandler[1];
        prior[0] = sun.misc.Signal.handle(signal, new sun.misc.SignalHandler() {
            public void handle(sun.misc.Signal received) {
                Scheduler.events.add(new Runnable() {
                    public void run() {
                        for (Resizes source : new ArrayList<Resizes>(WATCHERS)) {
                            notifyResize(source);
                        }
                    }
                });
                sun.misc.SignalHandler chained = prior[0];
                if (chained != null && chained != sun.misc.SignalHandler.SIG_DFL
                        && chained != sun.misc.SignalHandler.SIG_IGN) {
                    chained.handle(received);
                }
            }
        });
    }

    private static void notifyResize(Resizes source) {
        if (source.closed) {
            return;
        }
        source.notified = true;
        if (source.pending != null) {
            deliver(source);
        }
    }

    private static void deliver(Resizes source) {
        String operation = symbol("terminal_resize_next", 1);
        Future promise = source.pending;
        source.notified = false;
        try {
            int[] size = size(source.session, operation);
            ArrayList<Object> out = new ArrayList<Object>();
            out.add(BigInteger.valueOf(size[0]));
            out.add(BigInteger.valueOf(size[1]));
            Scheduler.settle(promise, out, null);
        } catch (Throwable error) {
            Scheduler.settle(promise, null, caught(error, operation));
        }
    }

    public static Future runtime_application__terminal_resize_open(Object handle) {
        String operation = symbol("terminal_resize_open", 1);
        try {
            Session session = session(handle, operation);
            if (!session.outputInteractive) {
                throw failure("unsupported", operation, "Resize notifications require interactive output");
            }
            install();
            Resizes source = new Resizes();
            source.session = session;
            source.notified = true;
            session.resizes.add(source);
            WATCHERS.add(source);
            return Scheduler.resolved(source, null);
        } catch (Throwable error) {
            return Scheduler.resolved(null, caught(error, operation));
        }
    }

    public static Future runtime_application__terminal_resize_next(Object handle) {
        String operation = symbol("terminal_resize_next", 1);
        final Resizes source = (Resizes) handle;
        if (source.closed) {
            return Scheduler.resolved(null, failure("already_closed", operation, "Terminal resize source is closed"));
        }
        if (source.pending != null) {
            return Scheduler.resolved(null, failure("resource_exhausted", operation,
                "Resize source already has a pending observation"));
        }
        final Future promise = Future.promise();
        source.pending = promise;
        source.op = new Scheduler.Op();
        source.op.attach();
        promise.listen(new Runnable() {
            public void run() {
                source.pending = null;
                source.op.detach();
            }
        });
        if (source.notified) {
            deliver(source);
        }
        return promise;
    }

    public static boolean runtime_application__terminal_resize_closed(Object handle) {
        return ((Resizes) handle).closed;
    }

    private static void closeResizes(Resizes source) {
        if (source.closed) {
            return;
        }
        source.closed = true;
        WATCHERS.remove(source);
        if (source.session != null) {
            source.session.resizes.remove(source);
        }
        if (source.pending != null) {
            Scheduler.settle(source.pending, null, failure("already_closed", symbol("terminal_resize_next", 1),
                "Terminal resize source is closed"));
        }
    }

    public static boolean runtime_application__terminal_resize_close(Object handle) {
        closeResizes((Resizes) handle);
        return true;
    }
}
