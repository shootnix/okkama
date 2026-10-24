package okkama.rt;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.AccessDeniedException;
import java.nio.file.DirectoryNotEmptyException;
import java.nio.file.FileAlreadyExistsException;
import java.nio.file.FileSystemException;
import java.nio.file.FileSystemLoopException;
import java.nio.file.NoSuchFileException;
import java.nio.file.NotDirectoryException;
import java.nio.file.NotLinkException;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.List;

/**
 * Общее состояние хоста для natives: текущий каталог процесса (JVM не
 * меняет свой cwd, поэтому {@code chdir} программы — виртуальный, и все
 * относительные пути natives разрешаются от него) и переносимые коды
 * отказов по исключениям ввода-вывода (аналог errno у Perl-provider).
 */
final class Host {
    private Host() {}

    /** Абсолютный текущий каталог программы. */
    static String cwd = System.getProperty("user.dir");

    /**
     * Окружение программы: JVM не меняет env своего процесса, поэтому
     * {@code environment_set}/{@code remove} меняют эту копию, а дочерние
     * процессы получают её.
     */
    static final java.util.Map<String, String> environment =
        new java.util.TreeMap<String, String>(System.getenv());

    /** Путь хоста: относительный — от {@link #cwd}. */
    static Path path(String text) {
        return Paths.get(cwd).resolve(text);
    }

    static String text(List<?> bytes) {
        return new String(Natives.toBytes(bytes), StandardCharsets.UTF_8);
    }

    static List<Object> bytes(String text) {
        byte[] bytes = text.getBytes(StandardCharsets.UTF_8);
        return Natives.fromBytes(bytes, bytes.length);
    }

    /** Переносимый код отказа ({@code portable_code_for_errno}). */
    static String code(Throwable error) {
        if (error instanceof NoSuchFileException) {
            return "not_found";
        }
        if (error instanceof AccessDeniedException) {
            return "permission_denied";
        }
        if (error instanceof FileAlreadyExistsException) {
            return "already_exists";
        }
        if (error instanceof DirectoryNotEmptyException) {
            return "not_empty";
        }
        if (error instanceof NotDirectoryException || error instanceof NotLinkException
                || error instanceof FileSystemLoopException) {
            return "invalid_input";
        }
        String text = String.valueOf(error instanceof FileSystemException
            ? ((FileSystemException) error).getReason() : error.getMessage()).toLowerCase();
        if (text.contains("no such file")) {
            return "not_found";
        }
        if (text.contains("permission denied") || text.contains("not permitted")
                || text.contains("read-only file system")) {
            return "permission_denied";
        }
        if (text.contains("file exists")) {
            return "already_exists";
        }
        if (text.contains("not empty")) {
            return "not_empty";
        }
        if (text.contains("is a directory") || text.contains("not a directory")
                || text.contains("invalid argument") || text.contains("too many levels")
                || text.contains("name too long") || text.contains("bad file descriptor")) {
            return "invalid_input";
        }
        if (text.contains("cross-device") || text.contains("not supported")) {
            return "unsupported";
        }
        if (text.contains("no space") || text.contains("too many open files")
                || text.contains("quota") || text.contains("cannot allocate")) {
            return "resource_exhausted";
        }
        if (text.contains("broken pipe")) {
            return "broken_pipe";
        }
        if (text.contains("interrupted")) {
            return "interrupted";
        }
        if (text.contains("resource temporarily unavailable") || text.contains("would block")) {
            return "would_block";
        }
        return "unexpected";
    }

    static boolean missing(Throwable error) {
        return code(error).equals("not_found");
    }

    /** Порядок Perl {@code cmp}: по code points. */
    static int compare(String left, String right) {
        return KeyOrder.compareCodePoints(left, right);
    }

    static IOException io(Throwable error) {
        return error instanceof IOException ? (IOException) error : new IOException(error);
    }
}
