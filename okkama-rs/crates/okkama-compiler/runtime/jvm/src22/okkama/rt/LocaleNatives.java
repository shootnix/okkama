package okkama.rt;

import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

/**
 * Natives {@code runtime/locale}: снимок предпочитаемых языков платформы.
 * macOS — {@code CFLocaleCopyPreferredLanguages} через FFM; прочие POSIX —
 * пусто, как у Perl-provider.
 */
public final class LocaleNatives {
    private LocaleNatives() {}

    private static final String OPERATION = "runtime/locale@1::preferred_language_tags/0";

    private static Failure failure(String code, String message) {
        return NativeFailure.raise("core.locale", code, OPERATION, message);
    }

    public static List<Object> runtime_locale__preferred_language_tags() {
        ArrayList<Object> out = new ArrayList<Object>();
        if (!Posix.MAC) {
            return out;
        }
        try (Arena arena = Arena.ofConfined()) {
            SymbolLookup cf = SymbolLookup.libraryLookup(
                "/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation", arena);
            MethodHandle copy = Posix.LINKER.downcallHandle(cf.find("CFLocaleCopyPreferredLanguages").orElseThrow(),
                FunctionDescriptor.of(Posix.PTR));
            MethodHandle count = Posix.LINKER.downcallHandle(cf.find("CFArrayGetCount").orElseThrow(),
                FunctionDescriptor.of(Posix.LONG, Posix.PTR));
            MethodHandle at = Posix.LINKER.downcallHandle(cf.find("CFArrayGetValueAtIndex").orElseThrow(),
                FunctionDescriptor.of(Posix.PTR, Posix.PTR, Posix.LONG));
            MethodHandle length = Posix.LINKER.downcallHandle(cf.find("CFStringGetLength").orElseThrow(),
                FunctionDescriptor.of(Posix.LONG, Posix.PTR));
            MethodHandle cstring = Posix.LINKER.downcallHandle(cf.find("CFStringGetCString").orElseThrow(),
                FunctionDescriptor.of(ValueLayout.JAVA_BYTE, Posix.PTR, Posix.PTR, Posix.LONG, Posix.INT));
            MethodHandle release = Posix.LINKER.downcallHandle(cf.find("CFRelease").orElseThrow(),
                FunctionDescriptor.ofVoid(Posix.PTR));
            MemorySegment array = (MemorySegment) copy.invokeExact();
            if (array.address() == 0) {
                throw failure("unexpected", "No Apple language snapshot");
            }
            try {
                long size = (long) count.invokeExact(array);
                if (size < 0 || size > 64) {
                    throw failure("resource_exhausted", "Too many platform languages");
                }
                for (long index = 0; index < size; index++) {
                    MemorySegment string = (MemorySegment) at.invokeExact(array, index);
                    long units = (long) length.invokeExact(string);
                    if (units < 0 || units > 255) {
                        throw failure("invalid_data", "Platform language length");
                    }
                    MemorySegment buffer = arena.allocate(units * 4 + 1);
                    byte ok = (byte) cstring.invokeExact(string, buffer, units * 4 + 1, 0x08000100);
                    if (ok == 0) {
                        throw failure("invalid_data", "Cannot convert platform language to UTF8");
                    }
                    String value = buffer.getString(0, StandardCharsets.UTF_8);
                    if (value.length() > 255) {
                        throw failure("invalid_data", "Invalid preferred language scalar");
                    }
                    out.add(value);
                }
            } finally {
                release.invokeExact(array);
            }
            return out;
        } catch (Failure failure) {
            throw failure;
        } catch (Throwable error) {
            throw failure("unexpected", "Cannot capture platform language preferences");
        }
    }
}
