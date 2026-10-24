package okkama.rt;

import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.File;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.Callable;

/**
 * Процессы-воркеры {@code spawn} синхронного действия (§18.1). Родитель
 * запускает ту же программу в новой JVM с {@code OKKAMA_SPAWN_WORKER}; её
 * launcher вместо {@code main} читает запрос из stdin: каталог, класс
 * функции-входа (её сгенерировал компилятор: конструирует receiver и
 * вызывает действие) и аргументы. Ответ — последняя рамка stdout: печать
 * самого действия идёт перед ней.
 */
public final class Worker {
    private Worker() {}

    private static final String MODE = "OKKAMA_SPAWN_WORKER";
    private static final byte[] FRAME = {'O', 'K', 'W', '1'};
    private static final int NONE = 0;
    private static final int INT = 1;
    private static final int FLOAT = 2;
    private static final int STRING = 3;
    private static final int CHAR = 4;
    private static final int BYTE = 5;
    private static final int BOOL = 6;
    private static final int ARRAY = 7;
    private static final int MAP = 8;

    /** Класс launcher программы: его {@code main} запускает воркер. */
    static String mainClass;

    /** Bound action, которое {@code worker.spawn} исполнит в воркере. */
    static final class Bound implements Fn {
        final Fn action;
        final Fn entry;
        final Object[] fields;

        Bound(Fn action, Fn entry, Object[] fields) {
            this.action = action;
            this.entry = entry;
            this.fields = fields;
        }

        public Object call(Object[] args) {
            return action.call(args);
        }

        public Object[] captures() {
            return action.captures();
        }
    }

    public static Fn bind(Fn action, Fn entry, Object[] fields) {
        return new Bound(action, entry, fields);
    }

    static boolean active() {
        return System.getenv(MODE) != null;
    }

    // ---- родитель ----

    public static Future spawn(Fn entry, Object[] args, boolean direct) {
        String target;
        Object[] all;
        if (direct) {
            target = entry.getClass().getName();
            all = args;
        } else if (entry instanceof Bound) {
            Bound bound = (Bound) entry;
            target = bound.entry.getClass().getName();
            all = new Object[bound.fields.length + args.length];
            System.arraycopy(bound.fields, 0, all, 0, bound.fields.length);
            System.arraycopy(args, 0, all, bound.fields.length, args.length);
        } else {
            throw Rt.trap("spawn requires a synchronous bound object action whose receiver fields reconstruct it");
        }
        final byte[] request;
        try {
            ByteArrayOutputStream buffer = new ByteArrayOutputStream();
            DataOutputStream out = new DataOutputStream(buffer);
            text(out, Host.cwd);
            text(out, target);
            out.writeInt(all.length);
            for (Object value : all) {
                encode(out, value);
            }
            out.flush();
            request = buffer.toByteArray();
        } catch (IOException error) {
            throw Rt.trap("spawn cannot encode its request: " + error.getMessage());
        }
        final Future promise = Future.promise();
        Scheduler.register(promise);
        final Process[] child = new Process[1];
        final Scheduler.Op op = Scheduler.offload(new Callable<Object>() {
            public Object call() throws Exception {
                return run(request, child);
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                if (error != null) {
                    Failure failure = error instanceof Failure ? (Failure) error
                        : Rt.trap("cannot execute spawn worker: " + error.getMessage());
                    Scheduler.settle(promise, null, failure);
                    return;
                }
                Object[] outcome = (Object[]) value;
                if (outcome[1] != null) {
                    Scheduler.settle(promise, null, Rt.trap((String) outcome[1]));
                } else {
                    Scheduler.settle(promise, outcome[0], null);
                }
            }
        });
        promise.listen(new Runnable() {
            public void run() {
                op.detach();
                synchronized (child) {
                    if (child[0] != null && child[0].isAlive()) {
                        child[0].destroy();
                    }
                }
            }
        });
        return promise;
    }

    /** Исполнить запрос в дочерней JVM: {@code {значение, текст отказа}}. */
    private static Object[] run(byte[] request, Process[] child) throws Exception {
        ArrayList<String> command = new ArrayList<String>();
        command.add(System.getProperty("java.home") + File.separator + "bin" + File.separator + "java");
        command.addAll(java.lang.management.ManagementFactory.getRuntimeMXBean().getInputArguments());
        command.add("-cp");
        command.add(System.getProperty("java.class.path"));
        command.add(mainClass);
        ProcessBuilder builder = new ProcessBuilder(command);
        builder.environment().clear();
        builder.environment().putAll(Host.environment);
        builder.environment().put(MODE, "1");
        builder.directory(new File(Host.cwd));
        final Process process;
        synchronized (child) {
            process = builder.start();
            child[0] = process;
        }
        final ByteArrayOutputStream errors = new ByteArrayOutputStream();
        Thread drain = new Thread(new Runnable() {
            public void run() {
                copy(process.getErrorStream(), errors);
            }
        }, "okkama-worker-stderr");
        drain.setDaemon(true);
        drain.start();
        OutputStream input = process.getOutputStream();
        try {
            input.write(request);
        } finally {
            input.close();
        }
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        copy(process.getInputStream(), output);
        int status = process.waitFor();
        drain.join();
        String stderr = new String(errors.toByteArray(), StandardCharsets.UTF_8);
        byte[] bytes = output.toByteArray();
        int at = frame(bytes);
        if (status != 0 || at < 0) {
            return new Object[] {null, !stderr.isEmpty() ? stderr.trim() : "spawn worker exited with status " + status};
        }
        // Печать действия до рамки — его stdout.
        System.out.write(bytes, 0, at);
        System.out.flush();
        if (!stderr.isEmpty()) {
            System.err.print(stderr);
            System.err.flush();
        }
        DataInputStream in = new DataInputStream(new java.io.ByteArrayInputStream(bytes, at + FRAME.length,
            bytes.length - at - FRAME.length));
        if (in.readBoolean()) {
            return new Object[] {decode(in), null};
        }
        return new Object[] {null, text(in)};
    }

    private static int frame(byte[] bytes) {
        outer:
        for (int at = bytes.length - FRAME.length; at >= 0; at--) {
            for (int index = 0; index < FRAME.length; index++) {
                if (bytes[at + index] != FRAME[index]) {
                    continue outer;
                }
            }
            return at;
        }
        return -1;
    }

    private static void copy(InputStream in, ByteArrayOutputStream out) {
        byte[] buffer = new byte[8192];
        try {
            for (int count; (count = in.read(buffer)) >= 0; ) {
                out.write(buffer, 0, count);
            }
        } catch (IOException ignored) {
            // поток закрыт завершением процесса
        }
    }

    // ---- воркер ----

    /** Исполнить запрос родителя и завершить процесс; {@code main} не идёт. */
    static void serve() {
        Host.environment.remove(MODE);
        ByteArrayOutputStream response = new ByteArrayOutputStream();
        try {
            DataOutputStream out = new DataOutputStream(response);
            out.write(FRAME);
            try {
                ByteArrayOutputStream raw = new ByteArrayOutputStream();
                copy(System.in, raw);
                DataInputStream in = new DataInputStream(new java.io.ByteArrayInputStream(raw.toByteArray()));
                Host.cwd = text(in);
                String target = text(in);
                Object[] args = new Object[in.readInt()];
                for (int index = 0; index < args.length; index++) {
                    args[index] = decode(in);
                }
                java.lang.reflect.Constructor<?> constructor = Class.forName(target).getDeclaredConstructor();
                constructor.setAccessible(true);
                Object result = ((Fn) constructor.newInstance()).call(args);
                ByteArrayOutputStream value = new ByteArrayOutputStream();
                encode(new DataOutputStream(value), result);
                out.writeBoolean(true);
                out.write(value.toByteArray());
            } catch (Failure failure) {
                out.writeBoolean(false);
                text(out, "runtime: " + Rt.describe(failure));
            } catch (Throwable error) {
                out.writeBoolean(false);
                text(out, "runtime: " + error);
            }
            out.flush();
            System.out.flush();
            FileOutputStream stdout = new FileOutputStream(FileDescriptor.out);
            stdout.write(response.toByteArray());
            stdout.flush();
        } catch (IOException error) {
            System.exit(70);
        }
        System.exit(0);
    }

    // ---- значения границы ----

    private static void text(DataOutputStream out, String value) throws IOException {
        byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
        out.writeInt(bytes.length);
        out.write(bytes);
    }

    private static String text(DataInputStream in) throws IOException {
        byte[] bytes = new byte[in.readInt()];
        in.readFully(bytes);
        return new String(bytes, StandardCharsets.UTF_8);
    }

    /** Передаваемые значения: скаляры, массивы и maps из них (§18.1). */
    private static void encode(DataOutputStream out, Object value) throws IOException {
        if (value == null) {
            out.writeByte(NONE);
        } else if (value instanceof BigInteger) {
            byte[] bytes = ((BigInteger) value).toByteArray();
            out.writeByte(INT);
            out.writeInt(bytes.length);
            out.write(bytes);
        } else if (value instanceof Double) {
            out.writeByte(FLOAT);
            out.writeDouble((Double) value);
        } else if (value instanceof String) {
            out.writeByte(STRING);
            text(out, (String) value);
        } else if (value instanceof Integer) {
            out.writeByte(CHAR);
            out.writeInt((Integer) value);
        } else if (value instanceof Byte) {
            out.writeByte(BYTE);
            out.writeByte((Byte) value);
        } else if (value instanceof Boolean) {
            out.writeByte(BOOL);
            out.writeBoolean((Boolean) value);
        } else if (value instanceof List) {
            List<?> items = (List<?>) value;
            out.writeByte(ARRAY);
            out.writeInt(items.size());
            for (Object item : items) {
                encode(out, item);
            }
        } else if (value instanceof Map) {
            Map<?, ?> entries = (Map<?, ?>) value;
            out.writeByte(MAP);
            out.writeInt(entries.size());
            for (Map.Entry<?, ?> entry : entries.entrySet()) {
                encode(out, entry.getKey());
                encode(out, entry.getValue());
            }
        } else {
            throw Rt.trap("spawn cannot transfer value of type " + typeName(value));
        }
    }

    private static Object decode(DataInputStream in) throws IOException {
        int tag = in.readByte();
        switch (tag) {
            case NONE:
                return null;
            case INT: {
                byte[] bytes = new byte[in.readInt()];
                in.readFully(bytes);
                return new BigInteger(bytes);
            }
            case FLOAT:
                return in.readDouble();
            case STRING:
                return text(in);
            case CHAR:
                return in.readInt();
            case BYTE:
                return in.readByte();
            case BOOL:
                return in.readBoolean();
            case ARRAY: {
                int size = in.readInt();
                ArrayList<Object> items = new ArrayList<Object>(size);
                for (int index = 0; index < size; index++) {
                    items.add(decode(in));
                }
                return items;
            }
            case MAP: {
                int size = in.readInt();
                Map<Object, Object> entries = Rt.newMap();
                for (int index = 0; index < size; index++) {
                    Object key = decode(in);
                    entries.put(key, decode(in));
                }
                return entries;
            }
            default:
                throw new IOException("invalid spawn value tag " + tag);
        }
    }

    private static String typeName(Object value) {
        if (value instanceof Rat) {
            return "rat";
        }
        if (value instanceof Case) {
            return ((Case) value).type;
        }
        if (value instanceof Future) {
            return "Future";
        }
        if (value instanceof Fn) {
            return "action";
        }
        if (value instanceof Any) {
            return "any";
        }
        String name = value.getClass().getName();
        return name.substring(Math.max(name.lastIndexOf('$'), name.lastIndexOf('.')) + 1);
    }
}
