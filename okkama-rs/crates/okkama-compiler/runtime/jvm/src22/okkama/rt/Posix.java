package okkama.rt;

import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemoryLayout;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.VarHandle;
import java.nio.charset.StandardCharsets;

/**
 * Вызовы libc через FFM для natives, которых нет в JDK (учётные записи,
 * терминал): функции, {@code errno} после вызова и раскладки структур
 * macOS/Linux.
 */
final class Posix {
    private Posix() {}

    static final Linker LINKER = Linker.nativeLinker();
    static final SymbolLookup LIBC = LINKER.defaultLookup();
    static final ValueLayout.OfInt INT = ValueLayout.JAVA_INT;
    static final ValueLayout.OfLong LONG = ValueLayout.JAVA_LONG;
    static final ValueLayout.OfShort SHORT = ValueLayout.JAVA_SHORT;
    static final java.lang.foreign.AddressLayout PTR = ValueLayout.ADDRESS;
    static final boolean MAC = System.getProperty("os.name", "").toLowerCase().contains("mac");
    static final boolean LINUX = System.getProperty("os.name", "").toLowerCase().contains("linux");
    /** Раскладки структур и константы есть только для macOS и Linux. */
    static final boolean SUPPORTED = MAC || LINUX;

    private static final MemoryLayout STATE = Linker.Option.captureStateLayout();
    private static final VarHandle ERRNO = STATE.varHandle(MemoryLayout.PathElement.groupElement("errno"));

    /** Функция libc; {@code errno} пишется в первый аргумент-сегмент. */
    static MethodHandle errnoFunction(String name, FunctionDescriptor descriptor, Linker.Option... options) {
        Linker.Option[] all = new Linker.Option[options.length + 1];
        all[0] = Linker.Option.captureCallState("errno");
        System.arraycopy(options, 0, all, 1, options.length);
        return LINKER.downcallHandle(LIBC.find(name).orElseThrow(), descriptor, all);
    }

    static MethodHandle function(String name, FunctionDescriptor descriptor, Linker.Option... options) {
        return LINKER.downcallHandle(LIBC.find(name).orElseThrow(), descriptor, options);
    }

    static MemorySegment state(Arena arena) {
        return arena.allocate(STATE);
    }

    static int errno(MemorySegment state) {
        return (int) ERRNO.get(state, 0L);
    }

    /** Переносимый код по errno ({@code portable_code_for_errno}). */
    static String code(int errno) {
        switch (errno) {
            case 2:
                return "not_found";
            case 1:
            case 13:
            case 30:
                return "permission_denied";
            case 4:
                return "interrupted";
            case 9:
            case 20:
            case 21:
            case 22:
                return "invalid_input";
            case 32:
                return "broken_pipe";
            case 12:
            case 23:
            case 24:
            case 28:
                return "resource_exhausted";
            case 17:
                return "already_exists";
            default:
                if ((MAC && (errno == 35 || errno == 36 || errno == 37)) || (LINUX && errno == 11)) {
                    return "would_block";
                }
                return "unexpected";
        }
    }

    static String string(MemorySegment pointer) {
        if (pointer == null || pointer.address() == 0) {
            return null;
        }
        return pointer.reinterpret(Long.MAX_VALUE).getString(0, StandardCharsets.UTF_8);
    }
}
