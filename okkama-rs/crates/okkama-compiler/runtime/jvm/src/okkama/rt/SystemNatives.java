package okkama.rt;

import java.io.IOException;
import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.FileSystemException;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

/**
 * Natives {@code runtime/fs_root} (доступ под корнем без следования
 * ссылкам), {@code runtime/snapshot} (атомарная замена файла) и
 * {@code runtime/environment}. Пути без {@code openat}: каждый компонент
 * проверяется без следования ссылкам (NIO {@code NOFOLLOW_LINKS}).
 */
public final class SystemNatives {
    private SystemNatives() {}

    private static Failure fail(String domain, String code, String symbol, String message) {
        return NativeFailure.raise(domain, code, symbol, message);
    }

    /** Ошибка как {@code _errno} Perl: код и текст {@code strerror}. */
    private static final class HostError extends RuntimeException {
        private static final long serialVersionUID = 1L;
        final String code;
        final String reason;

        HostError(String code, String reason) {
            super(reason, null, false, false);
            this.code = code;
            this.reason = reason;
        }
    }

    private static HostError reason(IOException error) {
        String code = Host.code(error);
        String text;
        switch (code) {
            case "not_found": text = "No such file or directory"; break;
            case "permission_denied": text = "Permission denied"; break;
            case "already_exists": text = "File exists"; break;
            case "not_empty": text = "Directory not empty"; break;
            default:
                text = error instanceof FileSystemException && ((FileSystemException) error).getReason() != null
                    ? ((FileSystemException) error).getReason() : String.valueOf(error.getMessage());
        }
        return new HostError(code, text);
    }

    private static final HostError LOOP = new HostError("invalid_input", "Too many levels of symbolic links");
    private static final HostError NOT_DIRECTORY = new HostError("invalid_input", "Not a directory");

    /** Открытие компонента: без ссылок; {@code directory} — только каталог. */
    private static void openCheck(Path path, boolean directory) {
        if (Files.isSymbolicLink(path)) {
            throw LOOP;
        }
        if (!Files.exists(path, LinkOption.NOFOLLOW_LINKS)) {
            throw new HostError("not_found", "No such file or directory");
        }
        if (directory && !Files.isDirectory(path, LinkOption.NOFOLLOW_LINKS)) {
            throw NOT_DIRECTORY;
        }
        if (!Files.isReadable(path)) {
            throw new HostError("permission_denied", "Permission denied");
        }
    }

    // ---- runtime/fs_root ----

    private static final String ROOT = "core.fs.root";

    static final class Rooted {
        Path path;
        boolean closed;
    }

    private static String rootSymbol(String name, int arity) {
        return "runtime/fs_root@1::" + name + "/" + arity;
    }

    private static Rooted rooted(Object handle, String name, int arity) {
        Rooted state = (Rooted) handle;
        if (state.closed) {
            throw fail(ROOT, "already_closed", rootSymbol(name, arity), "Native handle is already closed");
        }
        return state;
    }

    private static Failure errno(String domain, String symbol, String message, HostError error) {
        return fail(domain, error.code, symbol, message + ": " + error.reason);
    }

    public static Object runtime_fs_root__file_root_open(List<?> pathValue) {
        String symbol = rootSymbol("file_root_open", 1);
        byte[] bytes = Natives.toBytes(pathValue);
        boolean nul = false;
        for (byte octet : bytes) {
            nul |= octet == 0;
        }
        if (bytes.length == 0 || bytes.length > 4096 || nul) {
            throw fail(ROOT, "invalid_input", symbol, "Invalid root path");
        }
        Path path = Host.path(new String(bytes, StandardCharsets.UTF_8));
        try {
            openCheck(path, true);
        } catch (HostError error) {
            throw errno(ROOT, symbol, "Cannot open no-follow root", error);
        }
        Rooted root = new Rooted();
        root.path = path;
        return root;
    }

    public static Object runtime_fs_root__file_root_entry(Object handle, List<?> partsValue) {
        String symbol = rootSymbol("file_root_entry", 2);
        Rooted root = rooted(handle, "file_root_entry", 2);
        if (partsValue.size() > 256) {
            throw fail(ROOT, "invalid_input", symbol, "Root coordinates exceed component budget");
        }
        ArrayList<String> parts = new ArrayList<String>();
        long width = 0;
        for (Object item : partsValue) {
            String part = (String) item;
            if (part.isEmpty() || part.equals(".") || part.equals("..") || part.matches("(?s).*[/\\\\\\x00].*")) {
                throw fail(ROOT, "invalid_input", symbol, "Invalid root coordinate");
            }
            int size = part.getBytes(StandardCharsets.UTF_8).length;
            width += size + 1;
            if (size > 255 || width > 4096) {
                throw fail(ROOT, "invalid_input", symbol, "Root coordinate byte budget");
            }
            parts.add(part);
        }
        Path current = root.path;
        try {
            for (int index = 0; index < parts.size(); index++) {
                current = current.resolve(parts.get(index));
                openCheck(current, index < parts.size() - 1);
            }
        } catch (HostError error) {
            throw errno(ROOT, symbol, "Cannot open no-follow rooted entry", error);
        }
        if (!Files.isRegularFile(current, LinkOption.NOFOLLOW_LINKS)
                && !Files.isDirectory(current, LinkOption.NOFOLLOW_LINKS)) {
            throw fail(ROOT, "invalid_data", symbol, "Rooted entries must be regular files or directories");
        }
        Rooted entry = new Rooted();
        entry.path = current;
        return entry;
    }

    public static boolean runtime_fs_root__file_root_closed(Object handle) {
        return ((Rooted) handle).closed;
    }

    public static boolean runtime_fs_root__file_root_close(Object handle) {
        ((Rooted) handle).closed = true;
        return true;
    }

    public static boolean runtime_fs_root__rooted_entry_closed(Object handle) {
        return ((Rooted) handle).closed;
    }

    public static boolean runtime_fs_root__rooted_entry_close(Object handle) {
        ((Rooted) handle).closed = true;
        return true;
    }

    public static Map<Object, Object> runtime_fs_root__rooted_entry_stat(Object handle) {
        Rooted entry = rooted(handle, "rooted_entry_stat", 1);
        try {
            Map<String, Object> attributes = Files.readAttributes(entry.path,
                "unix:mode,ino,dev,size,lastAccessTime,lastModifiedTime,ctime", LinkOption.NOFOLLOW_LINKS);
            int mode = (Integer) attributes.get("mode");
            int kind = (mode & 0170000) == 0100000 ? 1 : (mode & 0170000) == 0040000 ? 2 : 0;
            Map<Object, Object> out = Rt.newMap();
            out.put("kind", BigInteger.valueOf(kind));
            out.put("size", BigInteger.valueOf((Long) attributes.get("size")));
            out.put("permissions", BigInteger.valueOf(mode & 0777));
            out.put("accessed_sec", BigInteger.valueOf(seconds(attributes.get("lastAccessTime"))));
            out.put("modified_sec", BigInteger.valueOf(seconds(attributes.get("lastModifiedTime"))));
            out.put("changed_sec", BigInteger.valueOf(seconds(attributes.get("ctime"))));
            out.put("created_sec", BigInteger.valueOf(-1));
            out.put("device", BigInteger.valueOf((Long) attributes.get("dev")));
            out.put("inode", BigInteger.valueOf((Long) attributes.get("ino")));
            return out;
        } catch (IOException error) {
            throw errno(ROOT, rootSymbol("rooted_entry_stat", 1), "Cannot stat rooted descriptor", reason(error));
        }
    }

    private static long seconds(Object time) {
        return ((java.nio.file.attribute.FileTime) time).to(java.util.concurrent.TimeUnit.SECONDS);
    }

    public static List<Object> runtime_fs_root__rooted_entry_read(Object handle, BigInteger offset, BigInteger limit) {
        String symbol = rootSymbol("rooted_entry_read", 3);
        Rooted entry = rooted(handle, "rooted_entry_read", 3);
        if (offset.signum() < 0 || offset.compareTo(BigInteger.valueOf(9007199254740991L)) > 0
                || limit.signum() <= 0 || limit.compareTo(BigInteger.valueOf(1048576)) > 0) {
            throw fail(ROOT, "invalid_input", symbol, "Invalid rooted read coordinates");
        }
        if (!Files.isRegularFile(entry.path, LinkOption.NOFOLLOW_LINKS)) {
            throw fail(ROOT, "invalid_input", symbol, "Only regular rooted files can be read");
        }
        try (FileChannel channel = FileChannel.open(entry.path, StandardOpenOption.READ, LinkOption.NOFOLLOW_LINKS)) {
            ByteBuffer buffer = ByteBuffer.allocate(limit.intValue());
            int count = channel.read(buffer, offset.longValue());
            byte[] bytes = new byte[Math.max(0, count)];
            buffer.flip();
            buffer.get(bytes);
            return Natives.fromBytes(bytes, bytes.length);
        } catch (IOException error) {
            throw errno(ROOT, symbol, "Cannot read rooted file", reason(error));
        }
    }

    public static List<Object> runtime_fs_root__rooted_entry_names(Object handle, BigInteger maximum) {
        String symbol = rootSymbol("rooted_entry_names", 2);
        Rooted entry = rooted(handle, "rooted_entry_names", 2);
        if (maximum.signum() < 0 || maximum.compareTo(BigInteger.valueOf(4096)) > 0) {
            throw fail(ROOT, "invalid_input", symbol, "Invalid directory entry budget");
        }
        if (!Files.isDirectory(entry.path, LinkOption.NOFOLLOW_LINKS)) {
            throw fail(ROOT, "invalid_input", symbol, "Rooted entry is not a directory");
        }
        ArrayList<String> names = new ArrayList<String>();
        try (java.nio.file.DirectoryStream<Path> stream = Files.newDirectoryStream(entry.path)) {
            for (Path child : stream) {
                if (names.size() >= maximum.intValue()) {
                    throw fail(ROOT, "resource_exhausted", symbol, "Directory entry budget");
                }
                names.add(child.getFileName().toString());
            }
        } catch (IOException error) {
            throw errno(ROOT, symbol, "Cannot enumerate retained directory", reason(error));
        }
        Collections.sort(names, new Comparator<String>() {
            public int compare(String left, String right) {
                return Host.compare(left, right);
            }
        });
        return new ArrayList<Object>(names);
    }

    // ---- runtime/snapshot ----

    private static final String SNAPSHOT = "core.snapshot";
    private static long serial;

    private static String snapshotSymbol(String name, int arity) {
        return "runtime/snapshot@1::" + name + "/" + arity;
    }

    private static String snapshotName(String text, String symbol) {
        if (text.isEmpty() || text.equals(".") || text.equals("..") || text.matches("(?s).*[/\\\\\\x00].*")) {
            throw fail(SNAPSHOT, "invalid_input", symbol, "Snapshot name must be one relative component");
        }
        if (text.getBytes(StandardCharsets.UTF_8).length > 255) {
            throw fail(SNAPSHOT, "invalid_input", symbol, "Snapshot name must be Unicode and at most 255 bytes");
        }
        return text;
    }

    public static Object runtime_snapshot__snapshot_root_open(List<?> pathValue) {
        String symbol = snapshotSymbol("snapshot_root_open", 1);
        byte[] bytes = Natives.toBytes(pathValue);
        boolean nul = false;
        for (byte octet : bytes) {
            nul |= octet == 0;
        }
        if (bytes.length == 0 || bytes.length > 4096 || nul) {
            throw fail(SNAPSHOT, "invalid_input", symbol, "Invalid snapshot root");
        }
        Path path = Host.path(new String(bytes, StandardCharsets.UTF_8));
        try {
            openCheck(path, true);
        } catch (HostError error) {
            throw errno(SNAPSHOT, symbol, "Cannot open snapshot root", error);
        }
        Rooted root = new Rooted();
        root.path = path;
        return root;
    }

    public static boolean runtime_snapshot__snapshot_root_close(Object handle) {
        ((Rooted) handle).closed = true;
        return true;
    }

    private static Rooted snapshotRoot(Object handle, String name, int arity) {
        Rooted root = (Rooted) handle;
        if (root.closed) {
            throw fail(SNAPSHOT, "already_closed", snapshotSymbol(name, arity), "Native handle is already closed");
        }
        return root;
    }

    /** Существующий снимок: путь регулярного файла или {@code null}. */
    private static Path existing(Rooted root, String name, String symbol) {
        Path path = root.path.resolve(name);
        if (Files.isSymbolicLink(path)) {
            throw errno(SNAPSHOT, symbol, "Cannot open no-follow snapshot", LOOP);
        }
        if (!Files.exists(path, LinkOption.NOFOLLOW_LINKS)) {
            return null;
        }
        if (!Files.isRegularFile(path, LinkOption.NOFOLLOW_LINKS)) {
            throw fail(SNAPSHOT, "invalid_data", symbol, "Snapshot must be a regular file");
        }
        return path;
    }

    public static List<Object> runtime_snapshot__snapshot_read(Object handle, String coordinate, BigInteger bound) {
        String symbol = snapshotSymbol("snapshot_read", 3);
        Rooted root = snapshotRoot(handle, "snapshot_read", 3);
        String name = snapshotName(coordinate, symbol);
        if (bound.signum() <= 0 || bound.compareTo(BigInteger.valueOf(1048576)) > 0) {
            throw fail(SNAPSHOT, "invalid_input", symbol, "Snapshot limit must be 1..1048576");
        }
        Path path = existing(root, name, symbol);
        if (path == null) {
            throw fail(SNAPSHOT, "not_found", symbol, "Snapshot is missing");
        }
        try {
            byte[] bytes = Files.readAllBytes(path);
            if (bytes.length > bound.intValue()) {
                throw fail(SNAPSHOT, "resource_exhausted", symbol, "Snapshot exceeds read limit");
            }
            return Natives.fromBytes(bytes, bytes.length);
        } catch (IOException error) {
            throw errno(SNAPSHOT, symbol, "Cannot read snapshot", reason(error));
        }
    }

    public static boolean runtime_snapshot__snapshot_write(Object handle, String coordinate, List<?> content) {
        String symbol = snapshotSymbol("snapshot_write", 3);
        Rooted root = snapshotRoot(handle, "snapshot_write", 3);
        String name = snapshotName(coordinate, symbol);
        byte[] bytes = Natives.toBytes(content);
        if (bytes.length > 1048576) {
            throw fail(SNAPSHOT, "resource_exhausted", symbol, "Snapshot exceeds 1048576 bytes");
        }
        existing(root, name, symbol);
        Path temporary = null;
        for (int attempt = 0; attempt < 64 && temporary == null; attempt++) {
            Path candidate = root.path.resolve(".okkama-snapshot-" + HostNatives.pid() + "-" + ++serial);
            try {
                Files.createFile(candidate, PosixFilePermissions.asFileAttribute(PosixFilePermissions.fromString("rw-------")));
                temporary = candidate;
            } catch (java.nio.file.FileAlreadyExistsException exists) {
                continue;
            } catch (IOException error) {
                throw errno(SNAPSHOT, symbol, "Cannot create snapshot temporary", reason(error));
            }
        }
        if (temporary == null) {
            throw fail(SNAPSHOT, "resource_exhausted", symbol, "Snapshot temporary collision budget");
        }
        try {
            try (FileChannel channel = FileChannel.open(temporary, StandardOpenOption.WRITE, LinkOption.NOFOLLOW_LINKS)) {
                ByteBuffer buffer = ByteBuffer.wrap(bytes);
                while (buffer.hasRemaining()) {
                    channel.write(buffer);
                }
                channel.force(true);
            } catch (IOException error) {
                throw errno(SNAPSHOT, symbol, "Cannot write snapshot temporary", reason(error));
            }
            try {
                Files.move(temporary, root.path.resolve(name), StandardCopyOption.ATOMIC_MOVE);
            } catch (IOException error) {
                throw errno(SNAPSHOT, symbol, "Cannot replace snapshot", reason(error));
            }
        } catch (Failure failure) {
            try {
                Files.deleteIfExists(temporary);
            } catch (IOException ignored) {
                // Временный файл без снимка не мешает следующей записи.
            }
            throw failure;
        }
        return true;
    }

    // ---- runtime/environment ----

    private static final String ENVIRONMENT = "core.environment";

    private static String environmentSymbol(String name, int arity) {
        return "runtime/environment@1::" + name + "/" + arity;
    }

    private static String textName(String name, String symbol) {
        if (name.isEmpty()) {
            throw fail(ENVIRONMENT, "invalid_input", symbol, "Environment name must not be empty");
        }
        if (name.indexOf('\0') >= 0) {
            throw fail(ENVIRONMENT, "invalid_input", symbol, "Environment name contains a zero byte");
        }
        if (name.indexOf('=') >= 0) {
            throw fail(ENVIRONMENT, "invalid_input", symbol, "Environment name contains \"=\"");
        }
        return windows() ? name.toUpperCase(java.util.Locale.ROOT) : name;
    }

    private static String textValue(String value, String symbol) {
        if (value.indexOf('\0') >= 0) {
            throw fail(ENVIRONMENT, "invalid_input", symbol, "Environment value contains a zero byte");
        }
        return value;
    }

    private static boolean windows() {
        return System.getProperty("os.name", "").toLowerCase(java.util.Locale.ROOT).contains("windows");
    }

    private static TreeMap<String, String> mapHash(Map<?, ?> values, String symbol) {
        TreeMap<String, String> out = new TreeMap<String, String>();
        for (Map.Entry<?, ?> entry : values.entrySet()) {
            String name = textName((String) entry.getKey(), symbol);
            String content = textValue((String) entry.getValue(), symbol);
            if (out.containsKey(name)) {
                throw fail(ENVIRONMENT, "invalid_input", symbol, "Environment contains names that collide on this platform");
            }
            out.put(name, content);
        }
        return out;
    }

    private static Map<Object, Object> stringMap(Map<String, String> values) {
        Map<Object, Object> out = Rt.newMap();
        out.putAll(values);
        return out;
    }

    public static Map<Object, Object> runtime_environment__environment_snapshot() {
        String symbol = environmentSymbol("environment_snapshot", 0);
        TreeMap<String, String> values = new TreeMap<String, String>();
        for (Map.Entry<String, String> entry : Host.environment.entrySet()) {
            String name = textName(entry.getKey(), symbol);
            if (values.containsKey(name)) {
                throw fail(ENVIRONMENT, "invalid_data", symbol, "Process environment contains colliding variable names");
            }
            values.put(name, textValue(entry.getValue(), symbol));
        }
        return stringMap(values);
    }

    public static Map<Object, Object> runtime_environment__environment_normalize(Map<?, ?> values) {
        return stringMap(mapHash(values, environmentSymbol("environment_normalize", 1)));
    }

    public static Map<Object, Object> runtime_environment__environment_add(Map<?, ?> valuesValue, String name,
            String content) {
        String symbol = environmentSymbol("environment_add", 3);
        TreeMap<String, String> values = mapHash(valuesValue, symbol);
        values.put(textName(name, symbol), textValue(content, symbol));
        return stringMap(values);
    }

    public static Map<Object, Object> runtime_environment__environment_diff(Map<?, ?> valuesValue, String name) {
        String symbol = environmentSymbol("environment_diff", 2);
        TreeMap<String, String> values = mapHash(valuesValue, symbol);
        values.remove(textName(name, symbol));
        return stringMap(values);
    }

    public static String runtime_environment__environment_name(String name) {
        return textName(name, environmentSymbol("environment_name", 1));
    }

    public static String runtime_environment__environment_home(Map<?, ?> valuesValue) {
        String symbol = environmentSymbol("environment_home", 1);
        TreeMap<String, String> values = mapHash(valuesValue, symbol);
        String home;
        if (windows()) {
            home = values.get("USERPROFILE");
            if ((home == null || home.isEmpty()) && values.get("HOMEDRIVE") != null && values.get("HOMEPATH") != null) {
                home = values.get("HOMEDRIVE") + values.get("HOMEPATH");
            }
        } else {
            home = values.get("HOME");
        }
        if (home == null || home.isEmpty()) {
            throw fail(ENVIRONMENT, "not_found", symbol, "Home directory is unavailable");
        }
        return home;
    }

    public static String runtime_environment__environment_path_separator() {
        return windows() ? ";" : ":";
    }

    private static String environmentName(List<?> value, String symbol) {
        byte[] bytes = Natives.toBytes(value);
        String name = new String(bytes, StandardCharsets.UTF_8);
        if (bytes.length == 0) {
            throw fail(ENVIRONMENT, "invalid_input", symbol, "Environment name must not be empty");
        }
        if (name.indexOf('\0') >= 0) {
            throw fail(ENVIRONMENT, "invalid_input", symbol, "Environment name contains a zero byte");
        }
        if (name.indexOf('=') >= 0) {
            throw fail(ENVIRONMENT, "invalid_input", symbol, "Environment name contains \"=\"");
        }
        return name;
    }

    public static List<Object> runtime_environment__environment_get(List<?> nameValue) {
        String symbol = environmentSymbol("environment_get", 1);
        String name = environmentName(nameValue, symbol);
        String value = Host.environment.get(name);
        if (value == null) {
            throw fail(ENVIRONMENT, "not_found", symbol, "Environment variable was not found");
        }
        return Host.bytes(value);
    }

    public static boolean runtime_environment__environment_set(List<?> nameValue, List<?> valueValue) {
        String symbol = environmentSymbol("environment_set", 2);
        String name = environmentName(nameValue, symbol);
        String value = new String(Natives.toBytes(valueValue), StandardCharsets.UTF_8);
        if (value.indexOf('\0') >= 0) {
            throw fail(ENVIRONMENT, "invalid_input", symbol, "Environment value contains a zero byte");
        }
        Host.environment.put(name, value);
        return true;
    }

    public static boolean runtime_environment__environment_remove(List<?> nameValue) {
        String name = environmentName(nameValue, environmentSymbol("environment_remove", 1));
        return Host.environment.remove(name) != null;
    }

    public static List<Object> runtime_environment__environment_all() {
        ArrayList<String[]> entries = new ArrayList<String[]>();
        for (Map.Entry<String, String> entry : Host.environment.entrySet()) {
            entries.add(new String[] {entry.getKey(), entry.getValue()});
        }
        Collections.sort(entries, new Comparator<String[]>() {
            public int compare(String[] left, String[] right) {
                return compareBytes(left[0].getBytes(StandardCharsets.UTF_8), right[0].getBytes(StandardCharsets.UTF_8));
            }
        });
        ArrayList<Object> out = new ArrayList<Object>();
        for (String[] entry : entries) {
            ArrayList<Object> pair = new ArrayList<Object>();
            pair.add(Host.bytes(entry[0]));
            pair.add(Host.bytes(entry[1]));
            out.add(pair);
        }
        return out;
    }

    private static int compareBytes(byte[] left, byte[] right) {
        for (int index = 0; index < Math.min(left.length, right.length); index++) {
            int a = left[index] & 0xff;
            int b = right[index] & 0xff;
            if (a != b) {
                return a < b ? -1 : 1;
            }
        }
        return Integer.compare(left.length, right.length);
    }
}
