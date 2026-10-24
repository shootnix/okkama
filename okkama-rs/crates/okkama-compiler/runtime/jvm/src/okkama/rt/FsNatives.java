package okkama.rt;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.CharBuffer;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.channels.NonReadableChannelException;
import java.nio.channels.NonWritableChannelException;
import java.nio.channels.OverlappingFileLockException;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.Charset;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.OpenOption;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.FileAttribute;
import java.nio.file.attribute.FileTime;
import java.nio.file.attribute.PosixFilePermission;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;
import java.util.concurrent.Callable;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Natives {@code runtime/fs}: лексические пути (File::Spec Unix/Win32),
 * файлы, каталоги, блокировки, временные файлы, glob и наблюдение. Пути —
 * байты UTF-8; относительные разрешаются от виртуального cwd ({@link Host}).
 * Тексты и коды отказов — как у Perl-provider.
 */
public final class FsNatives {
    private FsNatives() {}

    private static final String DOMAIN = "core.fs";
    private static final int READ = 1;
    private static final int WRITE = 2;
    private static final int CREATE = 4;
    private static final int TRUNCATE = 8;
    private static final int APPEND = 16;
    private static final int EXCLUSIVE = 32;
    private static final int KNOWN_FLAGS = READ | WRITE | CREATE | TRUNCATE | APPEND | EXCLUSIVE;

    private static String symbol(String name, int arity) {
        return "runtime/fs@1::" + name + "/" + arity;
    }

    private static Failure fail(String code, String name, int arity, String message) {
        return NativeFailure.raise(DOMAIN, code, symbol(name, arity), message);
    }

    private static Failure host(Throwable error, String name, int arity, String message) {
        if (error instanceof Failure) {
            return (Failure) error;
        }
        return fail(Host.code(error), name, arity, message);
    }

    private static Failure closedHandle(String name, int arity) {
        return fail("already_closed", name, arity, "Native handle is already closed");
    }

    /** Байтовый путь программы ({@code _path}). */
    private static String path(List<?> value, String name, int arity) {
        byte[] bytes = Natives.toBytes(value);
        for (byte octet : bytes) {
            if (octet == 0) {
                throw fail("invalid_input", name, arity, "Invalid filesystem path");
            }
        }
        if (bytes.length == 0) {
            throw fail("invalid_input", name, arity, "Invalid filesystem path");
        }
        return new String(bytes, StandardCharsets.UTF_8);
    }

    private static boolean validMode(BigInteger mode) {
        return mode.signum() >= 0 && mode.compareTo(BigInteger.valueOf(0777)) <= 0;
    }

    private static Set<PosixFilePermission> permissions(int mode) {
        Set<PosixFilePermission> out = new HashSet<PosixFilePermission>();
        PosixFilePermission[] order = {
            PosixFilePermission.OTHERS_EXECUTE, PosixFilePermission.OTHERS_WRITE, PosixFilePermission.OTHERS_READ,
            PosixFilePermission.GROUP_EXECUTE, PosixFilePermission.GROUP_WRITE, PosixFilePermission.GROUP_READ,
            PosixFilePermission.OWNER_EXECUTE, PosixFilePermission.OWNER_WRITE, PosixFilePermission.OWNER_READ,
        };
        for (int bit = 0; bit < 9; bit++) {
            if ((mode & (1 << bit)) != 0) {
                out.add(order[bit]);
            }
        }
        return out;
    }

    // ---- лексические пути: File::Spec ----

    private static String replaceAll(String text, String regex, String replacement) {
        return Pattern.compile(regex, Pattern.DOTALL).matcher(text).replaceAll(Matcher.quoteReplacement(replacement));
    }

    private static String replaceFirst(String text, String regex, String replacement) {
        return Pattern.compile(regex, Pattern.DOTALL).matcher(text).replaceFirst(Matcher.quoteReplacement(replacement));
    }

    /** Perl {@code split} без предела: хвостовые пустые поля отбрасываются. */
    private static List<String> split(String text, String regex, boolean keepTrailing) {
        ArrayList<String> out = new ArrayList<String>();
        if (text.isEmpty()) {
            return out;
        }
        Collections.addAll(out, text.split(regex, keepTrailing ? -1 : 0));
        return out;
    }

    private abstract static class Spec {
        abstract String canonpath(String path);

        abstract String catdir(List<String> parts);

        abstract String catfile(List<String> parts);

        abstract boolean absolute(String path);

        abstract String[] splitpath(String path);

        abstract List<String> splitdir(String directories);

        abstract String catpath(String volume, String directory, String file);

        abstract String rootdir();
    }

    private static final class Unix extends Spec {
        String canonpath(String path) {
            path = replaceAll(path, "/{2,}", "/");
            path = replaceAll(path, "(?:/\\.)+(?:/|\\z)", "/");
            if (!path.equals("./")) {
                path = replaceFirst(path, "^(?:\\./)+", "");
            }
            path = replaceFirst(path, "^/(?:\\.\\./)+", "/");
            path = replaceFirst(path, "^/\\.\\.$", "/");
            if (!path.equals("/")) {
                path = replaceFirst(path, "/\\z", "");
            }
            return path;
        }

        String catdir(List<String> parts) {
            StringBuilder joined = new StringBuilder();
            for (String part : parts) {
                joined.append(part).append('/');
            }
            return canonpath(joined.toString());
        }

        String catfile(List<String> parts) {
            String file = canonpath(parts.get(parts.size() - 1));
            if (parts.size() == 1) {
                return file;
            }
            String dir = catdir(parts.subList(0, parts.size() - 1));
            if (!dir.endsWith("/")) {
                dir += "/";
            }
            return dir + file;
        }

        boolean absolute(String path) {
            return path.startsWith("/");
        }

        String[] splitpath(String path) {
            Matcher match = Pattern.compile("^((?:.*/(?:\\.\\.?\\z)?)?)([^/]*)", Pattern.DOTALL).matcher(path);
            match.find();
            return new String[] {"", match.group(1), match.group(2)};
        }

        List<String> splitdir(String directories) {
            return split(directories, "/", true);
        }

        String catpath(String volume, String directory, String file) {
            if (!directory.isEmpty() && !file.isEmpty() && !directory.endsWith("/") && !file.startsWith("/")) {
                return directory + "/" + file;
            }
            return directory + file;
        }

        String rootdir() {
            return "/";
        }
    }

    private static final class Win32 extends Spec {
        static final String DRIVE = "[a-zA-Z]:";
        static final String UNC = "(?:\\\\\\\\|//)[^\\\\/]+[\\\\/][^\\\\/]+";
        static final String VOL = "(?:" + DRIVE + "|" + UNC + ")";

        static String canonCat(List<String> parts) {
            String first = parts.get(0);
            String volume;
            Matcher drive = Pattern.compile("\\A([A-Za-z]:)([\\\\/]?)").matcher(first);
            Matcher unc = Pattern.compile("\\A(?:\\\\\\\\|//)([^\\\\/]+)(?:[\\\\/]([^\\\\/]+))?[\\\\/]?", Pattern.DOTALL)
                .matcher(first);
            if (drive.find()) {
                volume = drive.group(1).substring(0, 1).toUpperCase() + drive.group(1).substring(1)
                    + (drive.group(2).isEmpty() ? "" : "\\");
                first = first.substring(drive.end());
            } else if (unc.find()) {
                volume = "\\\\" + unc.group(1) + (unc.group(2) != null ? "\\" + unc.group(2) : "") + "\\";
                first = first.substring(unc.end());
            } else if (first.startsWith("\\") || first.startsWith("/")) {
                volume = "\\";
                first = first.substring(1);
            } else {
                volume = "";
            }
            StringBuilder joined = new StringBuilder(first);
            for (String part : parts.subList(1, parts.size())) {
                joined.append('\\').append(part);
            }
            String path = joined.toString().replaceAll("[\\\\/]+", "\\\\");
            path = replaceAll(path, "(?:(?:\\A|\\\\)\\.(?:\\\\\\.)*(?:\\\\|\\z))+", "\\");
            Pattern up = Pattern.compile(
                "(?:(?:\\A|\\\\)[^\\\\]+\\\\\\.\\.(?<!\\A\\.\\.\\\\\\.\\.)(?<!\\\\\\.\\.\\\\\\.\\.)(?:\\\\|\\z))+",
                Pattern.DOTALL);
            while (true) {
                Matcher match = up.matcher(path);
                if (!match.find()) {
                    break;
                }
                path = path.substring(0, match.start()) + "\\" + path.substring(match.end());
            }
            path = replaceFirst(path, "\\A\\\\", "");
            path = replaceFirst(path, "\\\\\\z", "");
            if (volume.endsWith("\\")) {
                path = replaceFirst(path, "\\A\\.\\.(?:\\\\\\.\\.)*(?:\\\\|\\z)", "");
                Matcher share = Pattern.compile("\\A(\\\\\\\\.*)\\\\\\z", Pattern.DOTALL).matcher(volume);
                if (path.isEmpty() && share.find()) {
                    return share.group(1);
                }
            }
            return !path.isEmpty() || !volume.isEmpty() ? volume + path : ".";
        }

        String canonpath(String path) {
            return path.isEmpty() ? "" : canonCat(Collections.singletonList(path));
        }

        private String cat(List<String> parts) {
            ArrayList<String> list = new ArrayList<String>(parts);
            if (list.get(0).isEmpty()) {
                list.set(0, "/");
            } else if (list.get(0).matches(DRIVE)) {
                list.set(0, list.get(0) + "\\");
            }
            return canonCat(list);
        }

        String catdir(List<String> parts) {
            return parts.isEmpty() ? "" : cat(parts);
        }

        String catfile(List<String> parts) {
            return parts.isEmpty() ? canonCat(Collections.singletonList("/")) : cat(parts);
        }

        boolean absolute(String path) {
            Matcher volume = Pattern.compile("^(" + VOL + ")").matcher(path);
            if (volume.find()) {
                return volume.group(1).matches("^" + UNC + ".*") || Pattern.compile("^" + DRIVE + "[\\\\/]").matcher(path).find();
            }
            return path.startsWith("\\") || path.startsWith("/");
        }

        String[] splitpath(String path) {
            Matcher match = Pattern.compile("^(" + VOL + "?)((?:.*[\\\\/](?:\\.\\.?\\z)?)?)(.*)", Pattern.DOTALL)
                .matcher(path);
            match.find();
            return new String[] {match.group(1), match.group(2), match.group(3)};
        }

        List<String> splitdir(String directories) {
            if (!Pattern.compile("[\\\\/]\\z").matcher(directories).find()) {
                return split(directories, "[\\\\/]", false);
            }
            List<String> out = split(directories + "dummy", "[\\\\/]", false);
            out.set(out.size() - 1, "");
            return out;
        }

        String catpath(String volume, String directory, String file) {
            Matcher share = Pattern.compile("^([\\\\/])[\\\\/][^\\\\/]+[\\\\/][^\\\\/]+\\z", Pattern.DOTALL).matcher(volume);
            if (share.find() && Pattern.compile("^[^\\\\/]", Pattern.DOTALL).matcher(directory).find()) {
                volume += share.group(1);
            }
            volume += directory;
            if (!volume.matches("^[a-zA-Z]:") && Pattern.compile("[^\\\\/]\\z").matcher(volume).find()
                    && Pattern.compile("[^\\\\/]").matcher(file).find()) {
                Matcher separator = Pattern.compile("([\\\\/])").matcher(volume);
                volume += separator.find() ? separator.group(1) : "\\";
            }
            return volume + file;
        }

        String rootdir() {
            return "\\";
        }
    }

    private static final Spec UNIX = new Unix();
    private static final Spec WIN32 = new Win32();

    private static Spec spec(BigInteger style, String name, int arity) {
        if (style.equals(BigInteger.ZERO)) {
            return UNIX;
        }
        if (style.equals(BigInteger.ONE)) {
            return WIN32;
        }
        throw fail("invalid_input", name, arity, "Invalid path style");
    }

    private static String joined(Spec spec, List<String> parts) {
        String joined = parts.get(0);
        for (String part : parts.subList(1, parts.size())) {
            ArrayList<String> pair = new ArrayList<String>();
            pair.add(joined);
            pair.add(part);
            joined = spec.absolute(part) ? part : spec.catfile(pair);
        }
        return joined;
    }

    public static List<Object> runtime_fs__path_encode(String path) {
        for (int index = 0; index < path.length(); index++) {
            char unit = path.charAt(index);
            if (Character.isSurrogate(unit) && !(Character.isHighSurrogate(unit) && index + 1 < path.length()
                    && Character.isLowSurrogate(path.charAt(index + 1)))) {
                throw fail("invalid_data", "path_encode", 1, "String cannot be encoded as UTF-8");
            }
            if (Character.isHighSurrogate(unit)) {
                index++;
            }
        }
        return Host.bytes(path);
    }

    public static String runtime_fs__path_decode(List<?> path) {
        try {
            return StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
                .decode(ByteBuffer.wrap(Natives.toBytes(path))).toString();
        } catch (CharacterCodingException invalid) {
            throw fail("invalid_data", "path_decode", 1, "Filesystem path is not valid UTF-8");
        }
    }

    public static String runtime_fs__path_join(List<?> partsValue, BigInteger style) {
        Spec spec = spec(style, "path_join", 2);
        ArrayList<String> parts = new ArrayList<String>();
        for (Object part : partsValue) {
            parts.add((String) part);
        }
        if (parts.isEmpty()) {
            throw fail("invalid_input", "path_join", 2, "Path join requires at least one part");
        }
        for (String part : parts) {
            if (part.isEmpty() || part.indexOf('\0') >= 0) {
                throw fail("invalid_input", "path_join", 2, "Path join rejects empty parts");
            }
        }
        return joined(spec, parts);
    }

    public static String runtime_fs__path_parent(String path, BigInteger style) {
        Spec spec = spec(style, "path_parent", 2);
        if (path.isEmpty() || path.indexOf('\0') >= 0) {
            throw fail("invalid_input", "path_parent", 2, "Invalid lexical path");
        }
        String[] split = spec.splitpath(path);
        if (!split[2].isEmpty()) {
            String parent = spec.catpath(split[0], split[1], "");
            return spec.canonpath(parent.isEmpty() ? "." : parent);
        }
        List<String> segments = new ArrayList<String>(spec.splitdir(split[1]));
        while (segments.size() > 1 && segments.get(segments.size() - 1).isEmpty()) {
            segments.remove(segments.size() - 1);
        }
        if (!segments.isEmpty()) {
            segments.remove(segments.size() - 1);
        }
        String parent = spec.catpath(split[0], segments.isEmpty() ? "" : spec.catdir(segments), "");
        if (parent.isEmpty() && spec.absolute(path)) {
            parent = spec.rootdir();
        }
        if (parent.isEmpty()) {
            parent = ".";
        }
        return spec.canonpath(parent);
    }

    public static String runtime_fs__path_child(String path, String part, BigInteger style) {
        Spec spec = spec(style, "path_child", 3);
        if (path.isEmpty() || part.isEmpty()) {
            throw fail("invalid_input", "path_child", 3, "Path child rejects empty values");
        }
        if (path.indexOf('\0') >= 0 || part.indexOf('\0') >= 0) {
            throw fail("invalid_input", "path_child", 3, "Invalid lexical path");
        }
        ArrayList<String> parts = new ArrayList<String>();
        parts.add(path);
        parts.add(part);
        return joined(spec, parts);
    }

    public static boolean runtime_fs__path_absolute(String path, BigInteger style) {
        Spec spec = spec(style, "path_absolute", 2);
        if (path.isEmpty() || path.indexOf('\0') >= 0) {
            throw fail("invalid_input", "path_absolute", 2, "Invalid lexical path");
        }
        return spec.absolute(path);
    }

    // ---- низкоуровневые файлы ----

    static final class OpenFile {
        FileChannel channel;
        String path;
        boolean readable;
        boolean writable;
        boolean append;
        boolean closed;
        FileLock lock;
    }

    private static Set<OpenOption> options(int flags) {
        if (flags < 0 || (flags & ~KNOWN_FLAGS) != 0) {
            return null;
        }
        int access = flags & (READ | WRITE);
        if (access == 0) {
            return null;
        }
        if ((flags & (TRUNCATE | APPEND)) != 0 && (flags & WRITE) == 0) {
            return null;
        }
        if ((flags & EXCLUSIVE) != 0 && (flags & CREATE) == 0) {
            return null;
        }
        Set<OpenOption> options = new HashSet<OpenOption>();
        if ((access & READ) != 0) {
            options.add(StandardOpenOption.READ);
        }
        if ((access & WRITE) != 0) {
            options.add(StandardOpenOption.WRITE);
        }
        if ((flags & EXCLUSIVE) != 0) {
            options.add(StandardOpenOption.CREATE_NEW);
        } else if ((flags & CREATE) != 0) {
            options.add(StandardOpenOption.CREATE);
        }
        if ((flags & TRUNCATE) != 0) {
            options.add(StandardOpenOption.TRUNCATE_EXISTING);
        }
        // O_APPEND эмулируется: перед каждой записью позиция — конец файла
        // (JVM не сочетает APPEND с чтением).
        return options;
    }

    private static FileChannel openChannel(String path, Set<OpenOption> options, int mode) throws IOException {
        Path host = Host.path(path);
        if (options.contains(StandardOpenOption.CREATE) || options.contains(StandardOpenOption.CREATE_NEW)) {
            try {
                FileAttribute<?> attribute = PosixFilePermissions.asFileAttribute(permissions(mode));
                return FileChannel.open(host, options, attribute);
            } catch (UnsupportedOperationException noPosix) {
                return FileChannel.open(host, options);
            }
        }
        return FileChannel.open(host, options);
    }

    private static OpenFile openFile(Object handle, String name, int arity) {
        OpenFile file = (OpenFile) handle;
        if (file.closed) {
            throw closedHandle(name, arity);
        }
        return file;
    }

    public static Object runtime_fs__open(List<?> pathValue, BigInteger flagsValue, BigInteger mode) {
        String path = path(pathValue, "open", 3);
        int flags = flagsValue.bitLength() > 31 ? -1 : flagsValue.intValue();
        Set<OpenOption> options = options(flags);
        if (options == null) {
            throw fail("invalid_input", "open", 3, "Invalid open flags");
        }
        if (!validMode(mode)) {
            throw fail("invalid_input", "open", 3, "Invalid file mode");
        }
        OpenFile file = new OpenFile();
        try {
            file.channel = openChannel(path, options, mode.intValue());
        } catch (IOException error) {
            throw host(error, "open", 3, "Cannot open file");
        }
        file.path = path;
        file.readable = (flags & READ) != 0;
        file.writable = (flags & WRITE) != 0;
        file.append = (flags & APPEND) != 0;
        return file;
    }

    private static int readInto(FileChannel channel, byte[] buffer, int size) throws IOException {
        ByteBuffer target = ByteBuffer.wrap(buffer, 0, size);
        int count = channel.read(target);
        return count < 0 ? 0 : count;
    }

    public static List<Object> runtime_fs__read(Object handle, BigInteger size) {
        OpenFile file = openFile(handle, "read", 2);
        if (!file.readable) {
            throw fail("invalid_input", "read", 2, "File handle is not readable");
        }
        if (size.signum() < 0) {
            throw fail("invalid_input", "read", 2, "Read size must be non-negative");
        }
        if (size.signum() == 0) {
            return new ArrayList<Object>();
        }
        int limit = size.compareTo(BigInteger.valueOf(1 << 26)) > 0 ? 1 << 26 : size.intValue();
        byte[] buffer = new byte[limit];
        try {
            int count = readInto(file.channel, buffer, limit);
            return Natives.fromBytes(buffer, count);
        } catch (IOException error) {
            throw host(error, "read", 2, "Cannot read file");
        }
    }

    private static int writeAppending(FileChannel channel, boolean append, byte[] bytes) throws IOException {
        if (append) {
            channel.position(channel.size());
        }
        return channel.write(ByteBuffer.wrap(bytes));
    }

    public static BigInteger runtime_fs__write(Object handle, List<?> bytes) {
        OpenFile file = openFile(handle, "write", 2);
        if (!file.writable) {
            throw fail("invalid_input", "write", 2, "File handle is not writable");
        }
        if (bytes.isEmpty()) {
            return BigInteger.ZERO;
        }
        try {
            return BigInteger.valueOf(writeAppending(file.channel, file.append, Natives.toBytes(bytes)));
        } catch (IOException error) {
            throw host(error, "write", 2, "Cannot write file");
        }
    }

    private static long seek(FileChannel channel, BigInteger offset, BigInteger origin, String name, int arity,
            String path) {
        if (!origin.equals(BigInteger.ZERO) && !origin.equals(BigInteger.ONE) && !origin.equals(BigInteger.valueOf(2))) {
            throw fail("invalid_input", name, arity, "Invalid seek origin");
        }
        try {
            long base = origin.intValue() == 0 ? 0 : origin.intValue() == 1 ? channel.position() : channel.size();
            BigInteger target = offset.add(BigInteger.valueOf(base));
            if (target.signum() < 0 || target.bitLength() > 63) {
                throw fail("invalid_input", name, arity, "Cannot seek file");
            }
            channel.position(target.longValue());
            return target.longValue();
        } catch (IOException error) {
            throw host(error, name, arity, "Cannot seek file");
        }
    }

    public static BigInteger runtime_fs__seek(Object handle, BigInteger offset, BigInteger origin) {
        OpenFile file = openFile(handle, "seek", 3);
        return BigInteger.valueOf(seek(file.channel, offset, origin, "seek", 3, file.path));
    }

    public static boolean runtime_fs__flush(Object handle) {
        OpenFile file = openFile(handle, "flush", 1);
        if (!file.writable) {
            throw fail("invalid_input", "flush", 1, "File handle is not writable");
        }
        return true;
    }

    public static boolean runtime_fs__truncate(Object handle, BigInteger size) {
        OpenFile file = openFile(handle, "truncate", 2);
        if (!file.writable) {
            throw fail("invalid_input", "truncate", 2, "File handle is not writable");
        }
        if (size.signum() < 0) {
            throw fail("invalid_input", "truncate", 2, "Truncate size must be non-negative");
        }
        try {
            truncate(file.channel, size.longValue());
        } catch (IOException error) {
            throw host(error, "truncate", 2, "Cannot truncate file");
        }
        return true;
    }

    /** {@code truncate(2)}: и укорачивает, и удлиняет нулями. */
    private static void truncate(FileChannel channel, long size) throws IOException {
        long current = channel.size();
        if (size < current) {
            long position = channel.position();
            channel.truncate(size);
            channel.position(position);
        } else if (size > current) {
            long position = channel.position();
            channel.write(ByteBuffer.allocate(1), size - 1);
            channel.position(position);
        }
    }

    public static boolean runtime_fs__sync(Object handle) {
        OpenFile file = openFile(handle, "sync", 1);
        if (!file.writable) {
            throw fail("invalid_input", "sync", 1, "File handle is not writable");
        }
        try {
            file.channel.force(true);
        } catch (IOException error) {
            throw host(error, "sync", 1, "Cannot synchronize file");
        }
        return true;
    }

    private static boolean lockFile(Object handle, String name, boolean shared, String capability) {
        OpenFile file = openFile(handle, name, 1);
        if (capability != null && !(shared ? file.readable : file.writable)) {
            throw fail("invalid_input", name, 1, "File handle is not " + capability);
        }
        try {
            if (file.lock != null) {
                file.lock.release();
                file.lock = null;
            }
            file.lock = file.channel.lock(0, Long.MAX_VALUE, shared);
        } catch (IOException | OverlappingFileLockException | NonReadableChannelException
                | NonWritableChannelException error) {
            throw host(Host.io(error), name, 1, "Cannot " + name + " file");
        }
        return true;
    }

    public static boolean runtime_fs__lock_shared(Object handle) {
        return lockFile(handle, "lock_shared", true, "readable");
    }

    public static boolean runtime_fs__lock_exclusive(Object handle) {
        return lockFile(handle, "lock_exclusive", false, "writable");
    }

    public static boolean runtime_fs__unlock(Object handle) {
        OpenFile file = openFile(handle, "unlock", 1);
        try {
            if (file.lock != null) {
                file.lock.release();
                file.lock = null;
            }
        } catch (IOException error) {
            throw host(error, "unlock", 1, "Cannot unlock file");
        }
        return true;
    }

    public static boolean runtime_fs__close(Object handle) {
        OpenFile file = (OpenFile) handle;
        if (file.closed) {
            return true;
        }
        file.closed = true;
        try {
            file.channel.close();
        } catch (IOException error) {
            throw host(error, "close", 1, "Cannot close file");
        }
        return true;
    }

    // ---- метаданные и операции над путями ----

    private static int kind(Map<String, Object> attributes) {
        int mode = (Integer) attributes.get("mode");
        switch (mode & 0170000) {
            case 0100000: return 1;
            case 0040000: return 2;
            case 0120000: return 3;
            case 0060000: return 4;
            case 0020000: return 5;
            case 0010000: return 6;
            case 0140000: return 7;
            default: return 0;
        }
    }

    private static long seconds(Object time) {
        return ((FileTime) time).to(java.util.concurrent.TimeUnit.SECONDS);
    }

    private static Map<String, Object> unixAttributes(String path, boolean follow) throws IOException {
        LinkOption[] options = follow ? new LinkOption[0] : new LinkOption[] {LinkOption.NOFOLLOW_LINKS};
        return Files.readAttributes(Host.path(path), "unix:mode,ino,dev,size,lastAccessTime,lastModifiedTime,ctime",
            options);
    }

    private static Map<Object, Object> statValue(Map<String, Object> attributes) {
        Map<Object, Object> out = Rt.newMap();
        out.put("kind", BigInteger.valueOf(kind(attributes)));
        out.put("size", BigInteger.valueOf((Long) attributes.get("size")));
        out.put("permissions", BigInteger.valueOf((Integer) attributes.get("mode") & 0777));
        out.put("accessed_sec", BigInteger.valueOf(seconds(attributes.get("lastAccessTime"))));
        out.put("modified_sec", BigInteger.valueOf(seconds(attributes.get("lastModifiedTime"))));
        out.put("changed_sec", BigInteger.valueOf(seconds(attributes.get("ctime"))));
        out.put("created_sec", BigInteger.valueOf(-1));
        out.put("device", BigInteger.valueOf((Long) attributes.get("dev")));
        out.put("inode", BigInteger.valueOf((Long) attributes.get("ino")));
        return out;
    }

    public static Map<Object, Object> runtime_fs__stat(List<?> pathValue) {
        String path = path(pathValue, "stat", 1);
        try {
            return statValue(unixAttributes(path, true));
        } catch (IOException error) {
            throw host(error, "stat", 1, "Cannot stat path");
        }
    }

    public static Map<Object, Object> runtime_fs__lstat(List<?> pathValue) {
        String path = path(pathValue, "lstat", 1);
        try {
            return statValue(unixAttributes(path, false));
        } catch (IOException error) {
            throw host(error, "lstat", 1, "Cannot lstat path");
        }
    }

    public static boolean runtime_fs__mkdir(List<?> pathValue, BigInteger mode) {
        String path = path(pathValue, "mkdir", 2);
        if (!validMode(mode)) {
            throw fail("invalid_input", "mkdir", 2, "Invalid directory mode");
        }
        try {
            Files.createDirectory(Host.path(path), PosixFilePermissions.asFileAttribute(permissions(mode.intValue())));
        } catch (IOException error) {
            throw host(error, "mkdir", 2, "Cannot create directory");
        }
        return true;
    }

    private static boolean directory(Path path) {
        return Files.isDirectory(path, LinkOption.NOFOLLOW_LINKS);
    }

    public static boolean runtime_fs__rmdir(List<?> pathValue) {
        String path = path(pathValue, "rmdir", 1);
        Path host = Host.path(path);
        try {
            if (!directory(host) && Files.exists(host, LinkOption.NOFOLLOW_LINKS)) {
                throw fail("invalid_input", "rmdir", 1, "Cannot rmdir path");
            }
            Files.delete(host);
        } catch (IOException error) {
            if (Host.code(error).equals("not_empty")) {
                throw fail("not_empty", "rmdir", 1, "Cannot remove non-empty directory");
            }
            throw host(error, "rmdir", 1, "Cannot rmdir path");
        }
        return true;
    }

    public static boolean runtime_fs__unlink(List<?> pathValue) {
        String path = path(pathValue, "unlink", 1);
        Path host = Host.path(path);
        if (directory(host)) {
            // unlink(2) каталога: EPERM на macOS/BSD.
            throw fail("permission_denied", "unlink", 1, "Cannot unlink path");
        }
        try {
            Files.delete(host);
        } catch (IOException error) {
            throw host(error, "unlink", 1, "Cannot unlink path");
        }
        return true;
    }

    public static boolean runtime_fs__rename(List<?> sourceValue, List<?> targetValue) {
        String source = path(sourceValue, "rename", 2);
        String target = path(targetValue, "rename", 2);
        try {
            Files.move(Host.path(source), Host.path(target), StandardCopyOption.ATOMIC_MOVE);
        } catch (IOException error) {
            throw host(error, "rename", 2, "Cannot rename path");
        }
        return true;
    }

    public static boolean runtime_fs__link(List<?> sourceValue, List<?> targetValue) {
        String source = path(sourceValue, "link", 2);
        String target = path(targetValue, "link", 2);
        try {
            Files.createLink(Host.path(target), Host.path(source));
        } catch (IOException error) {
            throw host(error, "link", 2, "Cannot link path");
        }
        return true;
    }

    public static boolean runtime_fs__symlink(List<?> targetValue, List<?> linkValue) {
        String target = path(targetValue, "symlink", 2);
        String link = path(linkValue, "symlink", 2);
        try {
            Files.createSymbolicLink(Host.path(link), java.nio.file.Paths.get(target));
        } catch (IOException error) {
            throw host(error, "symlink", 2, "Cannot symlink path");
        }
        return true;
    }

    public static List<Object> runtime_fs__readlink(List<?> pathValue) {
        String path = path(pathValue, "readlink", 1);
        try {
            return Host.bytes(Files.readSymbolicLink(Host.path(path)).toString());
        } catch (IOException error) {
            throw host(error, "readlink", 1, "Cannot read symbolic link");
        }
    }

    public static List<Object> runtime_fs__cwd() {
        return Host.bytes(Host.cwd);
    }

    public static boolean runtime_fs__chdir(List<?> pathValue) {
        String path = path(pathValue, "chdir", 1);
        Path host = Host.path(path);
        try {
            Path real = host.toRealPath();
            if (!Files.isDirectory(real)) {
                throw fail("invalid_input", "chdir", 1, "Cannot change current directory");
            }
            if (!Files.isExecutable(real)) {
                throw fail("permission_denied", "chdir", 1, "Cannot change current directory");
            }
            Host.cwd = real.toString();
        } catch (IOException error) {
            throw host(error, "chdir", 1, "Cannot change current directory");
        }
        return true;
    }

    public static boolean runtime_fs__chmod(List<?> pathValue, BigInteger mode) {
        String path = path(pathValue, "chmod", 2);
        if (!validMode(mode)) {
            throw fail("invalid_input", "chmod", 2, "Invalid permission mode");
        }
        try {
            Files.setPosixFilePermissions(Host.path(path), permissions(mode.intValue()));
        } catch (IOException error) {
            throw host(error, "chmod", 2, "Cannot change path permissions");
        }
        return true;
    }

    public static boolean runtime_fs__chown(List<?> pathValue, BigInteger owner, BigInteger group) {
        String path = path(pathValue, "chown", 3);
        BigInteger minusOne = BigInteger.valueOf(-1);
        if (owner.compareTo(minusOne) < 0 || group.compareTo(minusOne) < 0) {
            throw fail("invalid_input", "chown", 3, "Invalid owner or group");
        }
        try {
            if (!owner.equals(minusOne)) {
                Files.setAttribute(Host.path(path), "unix:uid", owner.intValue());
            }
            if (!group.equals(minusOne)) {
                Files.setAttribute(Host.path(path), "unix:gid", group.intValue());
            }
            if (owner.equals(minusOne) && group.equals(minusOne)) {
                Files.readAttributes(Host.path(path), "unix:uid");
            }
        } catch (IOException error) {
            throw host(error, "chown", 3, "Cannot change path ownership");
        }
        return true;
    }

    // ---- каталоги ----

    /** Имена каталога без `.`/`..`, по возрастанию code points. */
    private static List<String> names(Path path) throws IOException {
        ArrayList<String> names = new ArrayList<String>();
        try (DirectoryStream<Path> stream = Files.newDirectoryStream(path)) {
            for (Path entry : stream) {
                names.add(entry.getFileName().toString());
            }
        }
        Collections.sort(names, new Comparator<String>() {
            public int compare(String left, String right) {
                return Host.compare(left, right);
            }
        });
        return names;
    }

    static final class Dir {
        String path;
        List<String> entries;
        int index;
        boolean closed;
    }

    private static Dir dir(Object handle, String name) {
        Dir dir = (Dir) handle;
        if (dir.closed) {
            throw closedHandle(name, 1);
        }
        return dir;
    }

    public static Object runtime_fs__opendir(List<?> pathValue) {
        String path = path(pathValue, "opendir", 1);
        Dir dir = new Dir();
        dir.path = path;
        try {
            dir.entries = names(Host.path(path));
        } catch (IOException error) {
            throw host(error, "opendir", 1, "Cannot open directory");
        }
        return dir;
    }

    public static List<Object> runtime_fs__readdir(Object handle) {
        Dir dir = dir(handle, "readdir");
        if (dir.index >= dir.entries.size()) {
            return new ArrayList<Object>();
        }
        return Host.bytes(dir.entries.get(dir.index++));
    }

    public static boolean runtime_fs__rewinddir(Object handle) {
        Dir dir = dir(handle, "rewinddir");
        try {
            dir.entries = names(Host.path(dir.path));
        } catch (IOException error) {
            dir.entries = new ArrayList<String>();
        }
        dir.index = 0;
        return true;
    }

    public static boolean runtime_fs__closedir(Object handle) {
        ((Dir) handle).closed = true;
        return true;
    }

    public static List<Object> runtime_fs__directory_entries(List<?> pathValue) {
        String path = path(pathValue, "directory_entries", 1);
        try {
            return new ArrayList<Object>(names(Host.path(path)));
        } catch (IOException error) {
            throw host(error, "directory_entries", 1, "Cannot open directory");
        }
    }

    private static void removeTree(Path path, String name, int arity) {
        try {
            if (!directory(path)) {
                Files.delete(path);
                return;
            }
            for (String child : names(path)) {
                removeTree(path.resolve(child), name, arity);
            }
            Files.delete(path);
        } catch (IOException error) {
            throw host(error, name, arity, "Cannot remove directory tree");
        }
    }

    public static boolean runtime_fs__directory_remove_tree(List<?> pathValue) {
        String path = path(pathValue, "directory_remove_tree", 1);
        Path host = Host.path(path);
        if (!Files.exists(host, LinkOption.NOFOLLOW_LINKS)) {
            throw fail("not_found", "directory_remove_tree", 1, "Cannot inspect directory tree");
        }
        if (!directory(host)) {
            throw fail("invalid_input", "directory_remove_tree", 1, "Tree root must be a directory");
        }
        removeTree(host, "directory_remove_tree", 1);
        return true;
    }

    public static List<Object> runtime_fs__file_content(List<?> pathValue) {
        String path = path(pathValue, "file_content", 1);
        try {
            byte[] bytes = Files.readAllBytes(Host.path(path));
            return Natives.fromBytes(bytes, bytes.length);
        } catch (IOException error) {
            throw host(error, "file_content", 1, "Cannot open file");
        }
    }

    // ---- glob ----

    private static final class Token {
        final char kind;
        final int value;
        final boolean negated;
        final List<int[]> ranges;

        Token(char kind, int value, boolean negated, List<int[]> ranges) {
            this.kind = kind;
            this.value = value;
            this.negated = negated;
            this.ranges = ranges;
        }
    }

    private static List<Token> globSegment(String segment) {
        String name = "matching_paths";
        if (!segment.equals("**") && segment.contains("**")) {
            throw fail("invalid_input", name, 2, "Recursive ** must be a complete path segment");
        }
        if (segment.equals("**")) {
            return null;
        }
        int[] characters = segment.codePoints().toArray();
        ArrayList<Token> tokens = new ArrayList<Token>();
        for (int position = 0; position < characters.length; position++) {
            int character = characters[position];
            if (character == '*') {
                tokens.add(new Token('*', 0, false, null));
            } else if (character == '?') {
                tokens.add(new Token('?', 0, false, null));
            } else if (character == '[') {
                int ending = position + 1;
                while (ending < characters.length && characters[ending] != ']') {
                    ending++;
                }
                if (ending >= characters.length) {
                    throw fail("invalid_input", name, 2, "Unclosed glob character class");
                }
                int from = position + 1;
                if (from == ending) {
                    throw fail("invalid_input", name, 2, "Empty glob character class");
                }
                boolean negated = false;
                if (characters[from] == '!') {
                    negated = true;
                    from++;
                }
                if (from == ending) {
                    throw fail("invalid_input", name, 2, "Empty negated glob character class");
                }
                ArrayList<int[]> ranges = new ArrayList<int[]>();
                int index = from;
                while (index < ending) {
                    int first = characters[index++];
                    if (index + 1 < ending && characters[index] == '-') {
                        int last = characters[index + 1];
                        if (first > last) {
                            throw fail("invalid_input", name, 2, "Descending glob character range");
                        }
                        ranges.add(new int[] {first, last});
                        index += 2;
                    } else {
                        ranges.add(new int[] {first, first});
                    }
                }
                tokens.add(new Token('[', 0, negated, ranges));
                position = ending;
            } else {
                tokens.add(new Token('l', character, false, null));
            }
        }
        return tokens;
    }

    private static boolean globMatches(List<Token> tokens, int[] name, int token, int at, Map<Long, Boolean> memo) {
        long key = ((long) token << 32) | at;
        Boolean known = memo.get(key);
        if (known != null) {
            return known;
        }
        boolean result;
        if (token == tokens.size()) {
            result = at == name.length;
        } else {
            Token current = tokens.get(token);
            if (current.kind == '*') {
                result = globMatches(tokens, name, token + 1, at, memo)
                    || (at < name.length && globMatches(tokens, name, token, at + 1, memo));
            } else if (at >= name.length) {
                result = false;
            } else {
                int character = name[at];
                boolean accepted;
                if (current.kind == '?') {
                    accepted = true;
                } else if (current.kind == 'l') {
                    accepted = character == current.value;
                } else {
                    boolean included = false;
                    for (int[] range : current.ranges) {
                        if (character >= range[0] && character <= range[1]) {
                            included = true;
                        }
                    }
                    accepted = current.negated ? !included : included;
                }
                result = accepted && globMatches(tokens, name, token + 1, at + 1, memo);
            }
        }
        memo.put(key, result);
        return result;
    }

    public static List<Object> runtime_fs__matching_paths(List<?> rootValue, String pattern) {
        String name = "matching_paths";
        String root = path(rootValue, name, 2);
        if (pattern.isEmpty()) {
            throw fail("invalid_input", name, 2, "Glob pattern cannot be empty");
        }
        if (pattern.startsWith("/")) {
            throw fail("invalid_input", name, 2, "Glob pattern must be relative to its root");
        }
        String[] parts = pattern.split("/", -1);
        for (String part : parts) {
            if (part.isEmpty()) {
                throw fail("invalid_input", name, 2, "Glob pattern contains an empty path segment");
            }
        }
        final List<List<Token>> segments = new ArrayList<List<Token>>();
        for (String part : parts) {
            segments.add(globSegment(part));
        }
        Path rootPath = Host.path(root);
        if (!Files.exists(rootPath, LinkOption.NOFOLLOW_LINKS)) {
            throw fail("not_found", name, 2, "Cannot inspect glob root");
        }
        if (!directory(rootPath)) {
            throw fail("invalid_input", name, 2, "Glob root must be a directory");
        }
        TreeSet<String> matches = new TreeSet<String>(new Comparator<String>() {
            public int compare(String left, String right) {
                return Host.compare(left, right);
            }
        });
        HashSet<String> visited = new HashSet<String>();
        try {
            visit(root, true, 0, true, segments, matches, visited);
        } catch (IOException error) {
            throw host(error, name, 2, "Cannot open directory");
        }
        return new ArrayList<Object>(matches);
    }

    /** Путь потомка как {@code File::Spec->catfile}. */
    private static String child(String path, String name) {
        ArrayList<String> parts = new ArrayList<String>();
        parts.add(path);
        parts.add(name);
        return UNIX.catfile(parts);
    }

    private static void visit(String path, boolean isDirectory, int index, boolean isRoot,
            List<List<Token>> segments, Set<String> matches, Set<String> visited) throws IOException {
        if (!visited.add(index + "\0" + path)) {
            return;
        }
        if (index == segments.size()) {
            if (!isRoot) {
                matches.add(path);
            }
            return;
        }
        List<Token> segment = segments.get(index);
        boolean recursive = segment == null;
        if (recursive) {
            visit(path, isDirectory, index + 1, isRoot, segments, matches, visited);
        }
        if (!isDirectory) {
            return;
        }
        for (String name : names(Host.path(path))) {
            if (!recursive && !globMatches(segment, name.codePoints().toArray(), 0, 0, new HashMap<Long, Boolean>())) {
                continue;
            }
            String child = child(path, name);
            boolean childDirectory = directory(Host.path(child));
            visit(child, childDirectory, recursive ? index : index + 1, false, segments, matches, visited);
        }
    }

    // ---- FileState: файл с режимом, кодировкой и буфером строк ----

    static final class FileState {
        String path;
        int mode;
        int flags;
        boolean binary;
        String encoding;
        Charset charset;
        FileChannel channel;
        ByteArrayOutputStream buffer = new ByteArrayOutputStream();
        boolean eof;
        boolean closed;
        String last;
        LockState lock;
        boolean temporary;
        String deleteOnClose;
        boolean preserved;
    }

    static final class LockState {
        FileState file;
        FileLock lock;
        boolean closed;
    }

    private static Integer stateFlags(BigInteger mode) {
        if (mode.bitLength() > 8) {
            return null;
        }
        switch (mode.intValue()) {
            case 0: return READ;
            case 1: return WRITE | CREATE | TRUNCATE;
            case 2: return WRITE | CREATE | APPEND;
            case 3: return READ | WRITE;
            case 4: return READ | WRITE | CREATE | APPEND;
            default: return null;
        }
    }

    private static Charset charset(String encoding) {
        switch (encoding) {
            case "utf-8": return StandardCharsets.UTF_8;
            case "utf-16-le": return StandardCharsets.UTF_16LE;
            case "utf-16-be": return StandardCharsets.UTF_16BE;
            case "utf-32-le": return Charset.forName("UTF-32LE");
            case "utf-32-be": return Charset.forName("UTF-32BE");
            case "ascii": return StandardCharsets.US_ASCII;
            case "latin-1": return StandardCharsets.ISO_8859_1;
            default: return null;
        }
    }

    public static Object runtime_fs__file_state_create(List<?> pathValue, BigInteger mode, String encoding,
            boolean binary) {
        String name = "file_state_create";
        String path = path(pathValue, name, 4);
        Integer flags = stateFlags(mode);
        if (flags == null) {
            throw fail("invalid_input", name, 4, "Invalid file mode");
        }
        if (binary && !encoding.isEmpty()) {
            throw fail("invalid_input", name, 4, "Binary files cannot have an encoding");
        }
        Charset charset = binary ? null : charset(encoding);
        if (!binary && charset == null) {
            throw fail("unsupported", name, 4, "Unsupported file encoding");
        }
        FileState state = new FileState();
        state.path = path;
        state.mode = mode.intValue();
        state.flags = flags;
        state.binary = binary;
        state.encoding = encoding;
        state.charset = charset;
        return state;
    }

    private static FileState state(Object handle, String name, int arity) {
        FileState state = (FileState) handle;
        if (state.closed) {
            throw fail("already_closed", name, arity, "File is already closed");
        }
        return state;
    }

    private static FileChannel stateOpen(FileState state, String name, int arity) {
        if (state.channel != null) {
            return state.channel;
        }
        Set<OpenOption> options = options(state.flags);
        try {
            state.channel = openChannel(state.path, options, 0666);
        } catch (IOException error) {
            throw host(error, name, arity, "Cannot open file");
        }
        return state.channel;
    }

    private static boolean readable(FileState state) {
        return state.mode == 0 || state.mode == 3 || state.mode == 4;
    }

    private static boolean writable(FileState state) {
        return state.mode >= 1;
    }

    private static boolean appending(FileState state) {
        return (state.flags & APPEND) != 0;
    }

    private static FileChannel prepareRead(FileState state, boolean text, String name, int arity) {
        if (!readable(state)) {
            throw fail("invalid_input", name, arity, "File mode does not permit reading");
        }
        if (text && state.binary) {
            throw fail("invalid_input", name, arity, "Text operation required");
        }
        if (!text && !state.binary) {
            throw fail("invalid_input", name, arity, "Binary operation required");
        }
        FileChannel channel = stateOpen(state, name, arity);
        state.last = "read";
        return channel;
    }

    private static FileChannel prepareWrite(FileState state, boolean text, String name, int arity) {
        if (!writable(state)) {
            throw fail("invalid_input", name, arity, "File mode does not permit writing");
        }
        if (text && state.binary) {
            throw fail("invalid_input", name, arity, "Text operation required");
        }
        if (!text && !state.binary) {
            throw fail("invalid_input", name, arity, "Binary operation required");
        }
        FileChannel channel = stateOpen(state, name, arity);
        if ("read".equals(state.last) && state.buffer.size() > 0) {
            try {
                channel.position(channel.position() - state.buffer.size());
            } catch (IOException error) {
                throw host(error, name, arity, "Cannot restore logical file position");
            }
            state.buffer.reset();
            state.eof = false;
        }
        state.last = "write";
        return channel;
    }

    private static String decode(FileState state, byte[] bytes, String name, int arity) {
        try {
            return state.charset.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(bytes)).toString();
        } catch (CharacterCodingException invalid) {
            throw fail("invalid_data", name, arity, "File contains invalid encoded text");
        }
    }

    private static boolean fill(FileState state, FileChannel channel, String name, int arity) {
        byte[] chunk = new byte[8192];
        try {
            int count = channel.read(ByteBuffer.wrap(chunk));
            if (count <= 0) {
                state.eof = true;
                return false;
            }
            state.buffer.write(chunk, 0, count);
            return true;
        } catch (IOException error) {
            throw host(error, name, arity, "Cannot read file");
        }
    }

    public static List<Object> runtime_fs__file_state_read(Object handle, BigInteger limit) {
        FileState state = state(handle, "file_state_read", 2);
        if (limit.signum() <= 0) {
            throw fail("invalid_input", "file_state_read", 2, "Read limit must be positive");
        }
        FileChannel channel = prepareRead(state, false, "file_state_read", 2);
        int size = limit.compareTo(BigInteger.valueOf(1 << 26)) > 0 ? 1 << 26 : limit.intValue();
        byte[] buffer = new byte[size];
        try {
            return Natives.fromBytes(buffer, readInto(channel, buffer, size));
        } catch (IOException error) {
            throw host(error, "file_state_read", 2, "Cannot read file");
        }
    }

    public static String runtime_fs__file_state_read_text(Object handle) {
        FileState state = state(handle, "file_state_read_text", 1);
        FileChannel channel = prepareRead(state, true, "file_state_read_text", 1);
        while (!state.eof) {
            fill(state, channel, "file_state_read_text", 1);
        }
        byte[] bytes = state.buffer.toByteArray();
        state.buffer.reset();
        return decode(state, bytes, "file_state_read_text", 1);
    }

    private static byte[] newline(FileState state) {
        switch (state.encoding) {
            case "utf-16-le": return new byte[] {0x0a, 0};
            case "utf-16-be": return new byte[] {0, 0x0a};
            case "utf-32-le": return new byte[] {0x0a, 0, 0, 0};
            case "utf-32-be": return new byte[] {0, 0, 0, 0x0a};
            default: return new byte[] {0x0a};
        }
    }

    private static int lineOffset(byte[] buffer, byte[] needle) {
        int unit = needle.length;
        outer:
        for (int offset = 0; offset + needle.length <= buffer.length; offset++) {
            for (int index = 0; index < needle.length; index++) {
                if (buffer[offset + index] != needle[index]) {
                    continue outer;
                }
            }
            if (offset % unit == 0) {
                return offset;
            }
        }
        return -1;
    }

    public static String runtime_fs__file_state_read_line(Object handle) {
        String name = "file_state_read_line";
        FileState state = state(handle, name, 1);
        FileChannel channel = prepareRead(state, true, name, 1);
        byte[] needle = newline(state);
        while (true) {
            byte[] buffer = state.buffer.toByteArray();
            int offset = lineOffset(buffer, needle);
            if (offset >= 0) {
                state.buffer.reset();
                state.buffer.write(buffer, offset + needle.length, buffer.length - offset - needle.length);
                byte[] line = new byte[offset];
                System.arraycopy(buffer, 0, line, 0, offset);
                String decoded = decode(state, line, name, 1);
                return decoded.endsWith("\r") ? decoded.substring(0, decoded.length() - 1) : decoded;
            }
            if (state.eof) {
                if (buffer.length == 0) {
                    throw fail("end_of_input", name, 1, "End of input");
                }
                state.buffer.reset();
                return decode(state, buffer, name, 1);
            }
            fill(state, channel, name, 1);
        }
    }

    public static BigInteger runtime_fs__file_state_write(Object handle, List<?> value) {
        FileState state = state(handle, "file_state_write", 2);
        FileChannel channel = prepareWrite(state, false, "file_state_write", 2);
        if (value.isEmpty()) {
            return BigInteger.ZERO;
        }
        try {
            return BigInteger.valueOf(writeAppending(channel, appending(state), Natives.toBytes(value)));
        } catch (IOException error) {
            throw host(error, "file_state_write", 2, "Cannot write file");
        }
    }

    public static BigInteger runtime_fs__file_state_write_text(Object handle, List<?> values, boolean newline) {
        String name = "file_state_write_text";
        FileState state = state(handle, name, 3);
        FileChannel channel = prepareWrite(state, true, name, 3);
        StringBuilder text = new StringBuilder();
        for (Object value : values) {
            text.append((String) value);
        }
        if (newline) {
            text.append('\n');
        }
        byte[] bytes;
        try {
            ByteBuffer encoded = state.charset.newEncoder().onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT).encode(CharBuffer.wrap(text));
            bytes = new byte[encoded.remaining()];
            encoded.get(bytes);
        } catch (CharacterCodingException invalid) {
            throw fail("invalid_data", name, 3, "Text cannot be encoded");
        }
        try {
            if (appending(state)) {
                channel.position(channel.size());
            }
            ByteBuffer buffer = ByteBuffer.wrap(bytes);
            while (buffer.hasRemaining()) {
                channel.write(buffer);
            }
        } catch (IOException error) {
            throw host(error, name, 3, "Cannot write file");
        }
        return BigInteger.valueOf(bytes.length);
    }

    public static boolean runtime_fs__file_state_flush(Object handle) {
        FileState state = state(handle, "file_state_flush", 1);
        if (!writable(state)) {
            throw fail("invalid_input", "file_state_flush", 1, "File mode does not permit writing");
        }
        stateOpen(state, "file_state_flush", 1);
        return true;
    }

    private static void releaseLock(FileState state) {
        if (state.lock != null && !state.lock.closed) {
            try {
                if (state.lock.lock != null) {
                    state.lock.lock.release();
                }
            } catch (IOException ignored) {
                // Закрытие канала всё равно снимает блокировку.
            }
            state.lock.closed = true;
            state.lock.file = null;
            state.lock = null;
        }
    }

    private static void cleanupTemporary(FileState state, String name) {
        if (state.deleteOnClose == null) {
            return;
        }
        try {
            Files.deleteIfExists(Host.path(state.deleteOnClose));
            state.deleteOnClose = null;
        } catch (IOException error) {
            throw host(error, name, 1, "Cannot remove temporary file");
        }
    }

    public static boolean runtime_fs__file_state_close(Object handle) {
        FileState state = (FileState) handle;
        if (state.closed) {
            cleanupTemporary(state, "file_state_close");
            return true;
        }
        releaseLock(state);
        if (state.channel != null) {
            try {
                state.channel.close();
            } catch (IOException error) {
                throw host(error, "file_state_close", 1, "Cannot close file");
            }
            state.channel = null;
        }
        state.buffer.reset();
        state.closed = true;
        cleanupTemporary(state, "file_state_close");
        return true;
    }

    public static boolean runtime_fs__file_state_closed(Object handle) {
        return ((FileState) handle).closed;
    }

    public static BigInteger runtime_fs__file_state_position(Object handle) {
        FileState state = state(handle, "file_state_position", 1);
        if (!state.binary) {
            throw fail("invalid_input", "file_state_position", 1, "Byte position is unavailable in text mode");
        }
        try {
            return BigInteger.valueOf(stateOpen(state, "file_state_position", 1).position());
        } catch (IOException error) {
            throw host(error, "file_state_position", 1, "Cannot inspect file position");
        }
    }

    public static BigInteger runtime_fs__file_state_seek(Object handle, BigInteger offset, BigInteger origin) {
        FileState state = state(handle, "file_state_seek", 3);
        if (!state.binary) {
            throw fail("invalid_input", "file_state_seek", 3, "Byte seeking is unavailable in text mode");
        }
        if (!origin.equals(BigInteger.ZERO) && !origin.equals(BigInteger.ONE) && !origin.equals(BigInteger.valueOf(2))) {
            throw fail("invalid_input", "file_state_seek", 3, "Invalid seek origin");
        }
        FileChannel channel = stateOpen(state, "file_state_seek", 3);
        long position = seek(channel, offset, origin, "file_state_seek", 3, state.path);
        state.buffer.reset();
        state.eof = false;
        state.last = null;
        return BigInteger.valueOf(position);
    }

    public static boolean runtime_fs__file_state_truncate(Object handle, BigInteger size) {
        String name = "file_state_truncate";
        FileState state = state(handle, name, 2);
        if (!state.binary) {
            throw fail("invalid_input", name, 2, "Truncation is unavailable in text mode");
        }
        if (!writable(state)) {
            throw fail("invalid_input", name, 2, "File mode does not permit writing");
        }
        if (size.signum() < 0) {
            throw fail("invalid_input", name, 2, "Truncate size must be non-negative");
        }
        try {
            truncate(stateOpen(state, name, 2), size.longValue());
        } catch (IOException error) {
            throw host(error, name, 2, "Cannot truncate file");
        }
        return true;
    }

    public static Object runtime_fs__file_state_lock(Object handle, BigInteger mode, BigInteger waiting) {
        String name = "file_state_lock";
        FileState state = state(handle, name, 3);
        if (state.lock != null && !state.lock.closed) {
            throw fail("invalid_input", name, 3, "File already has a live lock");
        }
        if (!mode.equals(BigInteger.ZERO) && !mode.equals(BigInteger.ONE)) {
            throw fail("invalid_input", name, 3, "Invalid file lock mode");
        }
        if (!waiting.equals(BigInteger.ZERO) && !waiting.equals(BigInteger.ONE)) {
            throw fail("invalid_input", name, 3, "Invalid file lock waiting mode");
        }
        FileChannel channel = stateOpen(state, name, 3);
        boolean shared = mode.signum() == 0;
        FileLock lock;
        try {
            lock = waiting.signum() == 0 ? channel.lock(0, Long.MAX_VALUE, shared)
                : channel.tryLock(0, Long.MAX_VALUE, shared);
        } catch (OverlappingFileLockException overlapping) {
            if (waiting.signum() != 0) {
                throw fail("would_block", name, 3, "File lock is unavailable");
            }
            throw fail("would_block", name, 3, "Cannot acquire file lock");
        } catch (IOException | NonReadableChannelException | NonWritableChannelException error) {
            throw fail(error instanceof IOException ? Host.code(error) : "invalid_input", name, 3,
                "Cannot acquire file lock");
        }
        if (lock == null) {
            throw fail("would_block", name, 3, "File lock is unavailable");
        }
        LockState lockState = new LockState();
        lockState.file = state;
        lockState.lock = lock;
        state.lock = lockState;
        return lockState;
    }

    public static boolean runtime_fs__file_lock_closed(Object handle) {
        return ((LockState) handle).closed;
    }

    public static boolean runtime_fs__file_lock_close(Object handle) {
        LockState lock = (LockState) handle;
        if (lock.closed) {
            return true;
        }
        FileState file = lock.file;
        if (lock.lock != null && lock.lock.isValid()) {
            try {
                lock.lock.release();
            } catch (IOException error) {
                throw host(error, "file_lock_close", 1, "Cannot release file lock");
            }
        }
        if (file != null && file.lock == lock) {
            file.lock = null;
        }
        lock.file = null;
        lock.closed = true;
        return true;
    }

    // ---- временные файлы и каталоги ----

    private static String prefix(String prefix, String name) {
        if (prefix.indexOf('\0') >= 0 || prefix.indexOf('/') >= 0 || prefix.indexOf('\\') >= 0) {
            throw fail("invalid_input", name, 2, "Temporary prefix must be one path component");
        }
        return prefix;
    }

    private static final String TEMPLATE = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_";
    private static final java.security.SecureRandom NAMES = new java.security.SecureRandom();

    /** Имя как у File::Temp: префикс и восемь случайных символов. */
    private static String temporaryName(String prefix) {
        StringBuilder name = new StringBuilder(prefix);
        for (int index = 0; index < 8; index++) {
            name.append(TEMPLATE.charAt(NAMES.nextInt(TEMPLATE.length())));
        }
        return name.toString();
    }

    public static Object runtime_fs__temporary_file_create(List<?> directoryValue, String prefixValue) {
        String name = "temporary_file_create";
        String directory = path(directoryValue, name, 2);
        String prefix = prefix(prefixValue, name);
        for (int attempt = 0; attempt < 100; attempt++) {
            String path = child(directory, temporaryName(prefix));
            Set<OpenOption> options = new HashSet<OpenOption>();
            options.add(StandardOpenOption.READ);
            options.add(StandardOpenOption.WRITE);
            options.add(StandardOpenOption.CREATE_NEW);
            FileChannel channel;
            try {
                channel = openChannel(path, options, 0600);
            } catch (java.nio.file.FileAlreadyExistsException exists) {
                continue;
            } catch (IOException error) {
                throw host(error, name, 2, "Cannot create temporary file");
            }
            FileState state = new FileState();
            state.path = path;
            state.mode = 3;
            state.flags = READ | WRITE;
            state.binary = true;
            state.encoding = "";
            state.channel = channel;
            state.temporary = true;
            state.deleteOnClose = path;
            return state;
        }
        throw fail("unexpected", name, 2, "Cannot create temporary file");
    }

    public static List<Object> runtime_fs__temporary_file_path(Object handle) {
        FileState state = (FileState) handle;
        if (!state.temporary) {
            throw fail("invalid_input", "temporary_file_path", 1, "File is not temporary");
        }
        return Host.bytes(state.path);
    }

    public static boolean runtime_fs__temporary_file_preserve(Object handle, List<?> destinationValue) {
        String name = "temporary_file_preserve";
        FileState state = (FileState) handle;
        String destination = path(destinationValue, name, 2);
        if (!state.temporary) {
            throw fail("invalid_input", name, 2, "File is not temporary");
        }
        if (state.closed) {
            throw fail("already_closed", name, 2, "Temporary file is already closed");
        }
        if (state.channel != null) {
            try {
                state.channel.close();
            } catch (IOException error) {
                throw host(error, name, 2, "Cannot close temporary file");
            }
            state.channel = null;
        }
        releaseLock(state);
        state.closed = true;
        Path source = Host.path(state.path);
        Path target = Host.path(destination);
        try {
            Files.createLink(target, source);
            Files.delete(source);
        } catch (IOException linkError) {
            if (!Host.code(linkError).equals("unsupported")) {
                throw host(linkError, name, 2, "Cannot preserve temporary file");
            }
            try {
                Files.copy(source, target);
                Files.delete(source);
            } catch (IOException error) {
                throw host(error, name, 2, "Cannot preserve temporary file");
            }
        }
        state.deleteOnClose = null;
        state.path = destination;
        state.preserved = true;
        return true;
    }

    static final class TemporaryDirectory {
        String path;
        boolean closed;
    }

    public static Object runtime_fs__temporary_directory_create(List<?> parentValue, String prefixValue) {
        String name = "temporary_directory_create";
        String parent = path(parentValue, name, 2);
        String prefix = prefix(prefixValue, name);
        for (int attempt = 0; attempt < 100; attempt++) {
            String path = child(parent, temporaryName(prefix));
            try {
                Files.createDirectory(Host.path(path), PosixFilePermissions.asFileAttribute(permissions(0700)));
            } catch (java.nio.file.FileAlreadyExistsException exists) {
                continue;
            } catch (IOException error) {
                throw host(error, name, 2, "Cannot create temporary directory");
            }
            TemporaryDirectory directory = new TemporaryDirectory();
            directory.path = path;
            return directory;
        }
        throw fail("unexpected", name, 2, "Cannot create temporary directory");
    }

    public static List<Object> runtime_fs__temporary_directory_path(Object handle) {
        return Host.bytes(((TemporaryDirectory) handle).path);
    }

    public static boolean runtime_fs__temporary_directory_closed(Object handle) {
        return ((TemporaryDirectory) handle).closed;
    }

    public static boolean runtime_fs__temporary_directory_close(Object handle) {
        TemporaryDirectory directory = (TemporaryDirectory) handle;
        if (directory.closed) {
            return true;
        }
        Path path = Host.path(directory.path);
        if (Files.exists(path, LinkOption.NOFOLLOW_LINKS)) {
            removeTree(path, "temporary_directory_close", 1);
        }
        directory.closed = true;
        return true;
    }

    // ---- наблюдение за путём (опрос раз в 50 мс) ----

    static final class Metadata {
        long device;
        long inode;
        int mode;
        long size;
        long modified;
        long changed;
        boolean directory;

        String identity() {
            return device + ":" + inode;
        }

        boolean differs(Metadata other) {
            return mode != other.mode || size != other.size || modified != other.modified
                || changed != other.changed;
        }
    }

    static final class Watch {
        String path;
        Metadata metadata;
        Map<String, Metadata> children;
        final ArrayList<String[]> events = new ArrayList<String[]>();
        Future waiter;
        boolean closed;
    }

    private static Metadata metadata(String path) throws IOException {
        Map<String, Object> attributes;
        try {
            attributes = unixAttributes(path, false);
        } catch (IOException error) {
            if (Host.missing(error)) {
                return null;
            }
            throw error;
        }
        Metadata metadata = new Metadata();
        metadata.device = (Long) attributes.get("dev");
        metadata.inode = (Long) attributes.get("ino");
        metadata.mode = (Integer) attributes.get("mode");
        metadata.size = (Long) attributes.get("size");
        metadata.modified = seconds(attributes.get("lastModifiedTime"));
        metadata.changed = seconds(attributes.get("ctime"));
        metadata.directory = (metadata.mode & 0170000) == 0040000;
        return metadata;
    }

    private static Map<String, Metadata> children(String path) throws IOException {
        Map<String, Metadata> out = new LinkedHashMap<String, Metadata>();
        for (String name : names(Host.path(path))) {
            Metadata child = metadata(child(path, name));
            if (child != null) {
                out.put(name, child);
            }
        }
        return out;
    }

    private static void childEvents(String root, Map<String, Metadata> before, Map<String, Metadata> after,
            List<String[]> events) {
        ArrayList<String> removed = new ArrayList<String>();
        ArrayList<String> created = new ArrayList<String>();
        TreeSet<String> beforeNames = new TreeSet<String>(new Comparator<String>() {
            public int compare(String left, String right) {
                return Host.compare(left, right);
            }
        });
        beforeNames.addAll(before.keySet());
        TreeSet<String> afterNames = new TreeSet<String>(beforeNames.comparator());
        afterNames.addAll(after.keySet());
        for (String name : beforeNames) {
            Metadata next = after.get(name);
            if (next == null || !before.get(name).identity().equals(next.identity())) {
                removed.add(name);
            } else if (before.get(name).differs(next)) {
                events.add(new String[] {"modified", child(root, name), ""});
            }
        }
        for (String name : afterNames) {
            Metadata previous = before.get(name);
            if (previous == null || !previous.identity().equals(after.get(name).identity())) {
                created.add(name);
            }
        }
        Map<String, ArrayList<String>> byIdentity = new HashMap<String, ArrayList<String>>();
        for (String name : created) {
            String identity = after.get(name).identity();
            if (!byIdentity.containsKey(identity)) {
                byIdentity.put(identity, new ArrayList<String>());
            }
            byIdentity.get(identity).add(name);
        }
        HashSet<String> renamed = new HashSet<String>();
        for (String name : removed) {
            ArrayList<String> candidates = byIdentity.get(before.get(name).identity());
            if (candidates != null && !candidates.isEmpty()) {
                String newName = candidates.remove(0);
                renamed.add(newName);
                events.add(new String[] {"renamed", child(root, newName), child(root, name)});
            } else {
                events.add(new String[] {"removed", child(root, name), ""});
            }
        }
        for (String name : created) {
            if (!renamed.contains(name)) {
                events.add(new String[] {"created", child(root, name), ""});
            }
        }
    }

    private static String[] poll(Watch watch) throws IOException {
        if (!watch.events.isEmpty()) {
            return watch.events.remove(0);
        }
        Metadata current = metadata(watch.path);
        Metadata previous = watch.metadata;
        if (previous == null && current != null) {
            watch.events.add(new String[] {"created", watch.path, ""});
        } else if (previous != null && current == null) {
            watch.events.add(new String[] {"removed", watch.path, ""});
        } else if (previous != null && !previous.identity().equals(current.identity())) {
            watch.events.add(new String[] {"removed", watch.path, ""});
            watch.events.add(new String[] {"created", watch.path, ""});
        } else if (current != null && !current.directory && previous.differs(current)) {
            watch.events.add(new String[] {"modified", watch.path, ""});
        }
        if (current != null && current.directory) {
            Map<String, Metadata> children = children(watch.path);
            if (previous != null && previous.directory) {
                childEvents(watch.path, watch.children, children, watch.events);
            }
            watch.children = children;
        } else {
            watch.children = new HashMap<String, Metadata>();
        }
        watch.metadata = current;
        return watch.events.isEmpty() ? null : watch.events.remove(0);
    }

    private static Map<Object, Object> eventValue(String[] event) {
        Map<Object, Object> out = Rt.newMap();
        out.put("kind", event[0]);
        out.put("path", event[1]);
        out.put("previous", event[2]);
        return out;
    }

    public static Object runtime_fs__file_watch_create(List<?> pathValue) {
        String name = "file_watch_create";
        String path = path(pathValue, name, 1);
        Watch watch = new Watch();
        watch.path = path;
        try {
            watch.metadata = metadata(path);
            if (watch.metadata == null) {
                throw fail("not_found", name, 1, "Watched path does not exist");
            }
            watch.children = watch.metadata.directory ? children(path) : new HashMap<String, Metadata>();
        } catch (IOException error) {
            throw host(error, name, 1, "Cannot inspect watched path");
        }
        return watch;
    }

    public static Future runtime_fs__file_watch_event(Object handle) {
        final String name = "file_watch_event";
        final Watch watch = (Watch) handle;
        if (watch.closed) {
            return Scheduler.resolved(null, fail("already_closed", name, 1, "File watch is already closed"));
        }
        if (watch.waiter != null && watch.waiter.state == Future.RUNNING) {
            return Scheduler.resolved(null, fail("resource_exhausted", name, 1, "File watch already has a pending event"));
        }
        try {
            String[] event = poll(watch);
            if (event != null) {
                return Scheduler.resolved(eventValue(event), null);
            }
        } catch (IOException error) {
            return Scheduler.resolved(null, host(error, name, 1, "Cannot inspect watched path"));
        }
        final Future promise = Future.promise();
        watch.waiter = promise;
        schedulePoll(watch, promise);
        return promise;
    }

    private static void schedulePoll(final Watch watch, final Future promise) {
        final Scheduler.Op op = Scheduler.offload(new Callable<Object>() {
            public Object call() throws InterruptedException {
                Thread.sleep(50);
                return null;
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                if (promise.state != Future.RUNNING) {
                    return;
                }
                if (watch.closed) {
                    Scheduler.settle(promise, null, fail("already_closed", "file_watch_event", 1,
                        "File watch is already closed"));
                    return;
                }
                try {
                    String[] event = poll(watch);
                    if (event != null) {
                        watch.waiter = null;
                        Scheduler.settle(promise, eventValue(event), null);
                        return;
                    }
                } catch (IOException failure) {
                    watch.waiter = null;
                    Scheduler.settle(promise, null, host(failure, "file_watch_event", 1, "Cannot inspect watched path"));
                    return;
                }
                schedulePoll(watch, promise);
            }
        });
        promise.listen(new Runnable() {
            public void run() {
                op.detach();
            }
        });
    }

    public static boolean runtime_fs__file_watch_closed(Object handle) {
        return ((Watch) handle).closed;
    }

    public static boolean runtime_fs__file_watch_close(Object handle) {
        Watch watch = (Watch) handle;
        if (watch.closed) {
            return true;
        }
        watch.closed = true;
        if (watch.waiter != null) {
            Scheduler.settle(watch.waiter, null, fail("already_closed", "file_watch_event", 1,
                "File watch is already closed"));
            watch.waiter = null;
        }
        return true;
    }
}
