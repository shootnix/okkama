package okkama.rt;

import java.io.IOException;
import java.io.InputStream;
import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.lang.ref.Cleaner;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardCopyOption;
import java.util.List;

/**
 * Natives {@code runtime/host_crypto}: криптография хоста — библиотека
 * {@code okkama-native} (RustCrypto) через FFM. Библиотека берётся из
 * {@code OKKAMA_NATIVE_LIB} или из ресурса {@code okkama/native/<os>-<arch>/}
 * артефакта (её вкладывает {@code okkama build}) и распаковывается во
 * временный каталог по хешу содержимого.
 */
public final class HostCryptoNatives {
    private HostCryptoNatives() {}

    private static final String DOMAIN = "core.crypto";
    private static final ValueLayout.OfLong SIZE = ValueLayout.JAVA_LONG;
    private static final java.lang.foreign.AddressLayout PTR = ValueLayout.ADDRESS;
    private static final Cleaner CLEANER = Cleaner.create();

    private static final class Api {
        final MethodHandle crypto;
        final MethodHandle free;
        final MethodHandle digestNew;
        final MethodHandle digestUpdate;
        final MethodHandle digestFinish;
        final MethodHandle digestFree;

        Api(SymbolLookup lookup) {
            crypto = handle(lookup, "okkama_crypto", FunctionDescriptor.of(ValueLayout.JAVA_INT, PTR, SIZE, PTR, PTR, SIZE,
                PTR, PTR));
            free = handle(lookup, "okkama_free", FunctionDescriptor.ofVoid(PTR, SIZE));
            digestNew = handle(lookup, "okkama_digest_new", FunctionDescriptor.of(PTR, PTR, SIZE, PTR, SIZE, SIZE, PTR, PTR,
                PTR));
            digestUpdate = handle(lookup, "okkama_digest_update", FunctionDescriptor.of(PTR, PTR, PTR, SIZE));
            digestFinish = handle(lookup, "okkama_digest_finish", FunctionDescriptor.of(ValueLayout.JAVA_INT, PTR, SIZE,
                PTR, PTR));
            digestFree = handle(lookup, "okkama_digest_free", FunctionDescriptor.ofVoid(PTR));
        }

        private static MethodHandle handle(SymbolLookup lookup, String name, FunctionDescriptor descriptor) {
            return Posix.LINKER.downcallHandle(lookup.find(name).orElseThrow(), descriptor);
        }
    }

    private static Api api;
    private static Failure unavailable;

    private static Failure failure(String code, String operation, String message) {
        return NativeFailure.raise(DOMAIN, code, operation, message);
    }

    private static String symbol(String name, int arity) {
        return "runtime/host_crypto@1::" + name + "/" + arity;
    }

    /** Платформа ресурса: {@code macos-aarch64}, {@code linux-x86_64}. */
    static String platform() {
        String os = System.getProperty("os.name", "").toLowerCase();
        String arch = System.getProperty("os.arch", "").toLowerCase();
        String system = os.contains("mac") ? "macos" : os.contains("linux") ? "linux" : os.contains("windows") ? "windows" : os;
        String machine = arch.equals("amd64") || arch.equals("x86_64") ? "x86_64"
            : arch.equals("aarch64") || arch.equals("arm64") ? "aarch64" : arch;
        return system + "-" + machine;
    }

    static String libraryName() {
        String os = System.getProperty("os.name", "").toLowerCase();
        return os.contains("mac") ? "libokkama_native.dylib" : os.contains("windows") ? "okkama_native.dll"
            : "libokkama_native.so";
    }

    private static Path library() throws IOException {
        String override = Host.environment.get("OKKAMA_NATIVE_LIB");
        if (override != null && !override.isEmpty()) {
            return Host.path(override);
        }
        String resource = "okkama/native/" + platform() + "/" + libraryName();
        InputStream in = HostCryptoNatives.class.getClassLoader().getResourceAsStream(resource);
        if (in == null) {
            return null;
        }
        byte[] bytes;
        try {
            bytes = in.readAllBytes();
        } finally {
            in.close();
        }
        String hash;
        try {
            hash = new BigInteger(1, java.security.MessageDigest.getInstance("SHA-256").digest(bytes)).toString(16);
        } catch (java.security.NoSuchAlgorithmException error) {
            throw new IOException(error);
        }
        Path directory = Paths.get(System.getProperty("java.io.tmpdir"), "okkama-native-" + hash.substring(0, 16));
        Path target = directory.resolve(libraryName());
        if (!Files.isRegularFile(target) || Files.size(target) != bytes.length) {
            Files.createDirectories(directory);
            Path partial = Files.createTempFile(directory, "lib", ".part");
            Files.write(partial, bytes);
            Files.move(partial, target, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
        }
        return target;
    }

    private static synchronized Api api(String operation) {
        if (api != null) {
            return api;
        }
        if (unavailable != null) {
            throw unavailable;
        }
        try {
            Path path = library();
            if (path == null) {
                unavailable = failure("unsupported", operation, "Host crypto library is unavailable for " + platform());
                throw unavailable;
            }
            api = new Api(SymbolLookup.libraryLookup(path, Arena.global()));
            return api;
        } catch (Failure failure) {
            throw failure;
        } catch (Throwable error) {
            unavailable = failure("unsupported", operation, "Host crypto library cannot be loaded: " + error.getMessage());
            throw unavailable;
        }
    }

    private static String code(int value) {
        switch (value) {
            case 1:
                return "invalid_input";
            case 2:
                return "permission_denied";
            case 3:
                return "unsupported";
            default:
                return "unexpected";
        }
    }

    private static MemorySegment segment(Arena arena, byte[] bytes) {
        return bytes.length == 0 ? MemorySegment.NULL : arena.allocateFrom(ValueLayout.JAVA_BYTE, bytes);
    }

    /** Байты результата библиотеки; буфер освобождается. */
    private static byte[] take(Api api, MemorySegment out, MemorySegment length) throws Throwable {
        MemorySegment pointer = out.get(PTR, 0);
        long size = length.get(SIZE, 0);
        byte[] bytes = size == 0 ? new byte[0] : pointer.reinterpret(size).toArray(ValueLayout.JAVA_BYTE);
        api.free.invokeExact(pointer, size);
        return bytes;
    }

    public static List<Object> runtime_host_crypto__crypto(String operation, List<?> arguments) {
        String name = symbol("crypto", 2);
        Api api = api(name);
        try (Arena arena = Arena.ofConfined()) {
            byte[] op = operation.getBytes(StandardCharsets.UTF_8);
            MemorySegment pointers = arena.allocate(PTR, Math.max(1, arguments.size()));
            MemorySegment lengths = arena.allocate(SIZE, Math.max(1, arguments.size()));
            for (int index = 0; index < arguments.size(); index++) {
                byte[] bytes = Natives.toBytes((List<?>) arguments.get(index));
                pointers.setAtIndex(PTR, index, segment(arena, bytes));
                lengths.setAtIndex(SIZE, index, bytes.length);
            }
            MemorySegment out = arena.allocate(PTR);
            MemorySegment length = arena.allocate(SIZE);
            int status = (int) api.crypto.invokeExact(segment(arena, op), (long) op.length, pointers,
                lengths, (long) arguments.size(), out, length);
            byte[] bytes = take(api, out, length);
            if (status != 0) {
                throw failure(code(status), name, new String(bytes, StandardCharsets.UTF_8));
            }
            return Natives.fromBytes(bytes, bytes.length);
        } catch (Failure failure) {
            throw failure;
        } catch (Throwable error) {
            throw failure("unexpected", name, "Host crypto call failed");
        }
    }

    /** {@code DigestState}: указатель состояния; освобождается Cleaner. */
    static final class DigestState {
        final MemorySegment pointer;

        DigestState(Api api, MemorySegment pointer) {
            this.pointer = pointer;
            CLEANER.register(this, () -> {
                try {
                    api.digestFree.invokeExact(pointer);
                } catch (Throwable ignored) {
                    // освобождение
                }
            });
        }
    }

    private static long length(BigInteger value, String operation) {
        if (value.signum() < 0 || value.bitLength() > 31) {
            throw failure("invalid_input", operation, "Digest length is out of range");
        }
        return value.longValue();
    }

    public static Object runtime_host_crypto__digest_create(String algorithm, List<?> key, BigInteger size) {
        String name = symbol("digest_create", 3);
        Api api = api(name);
        long bound = length(size, name);
        try (Arena arena = Arena.ofConfined()) {
            byte[] alg = algorithm.getBytes(StandardCharsets.UTF_8);
            byte[] secret = Natives.toBytes(key);
            MemorySegment out = arena.allocate(PTR);
            MemorySegment length = arena.allocate(SIZE);
            MemorySegment status = arena.allocate(ValueLayout.JAVA_INT);
            MemorySegment state = (MemorySegment) api.digestNew.invokeExact(segment(arena, alg), (long) alg.length,
                segment(arena, secret), (long) secret.length, bound, status, out, length);
            java.util.Arrays.fill(secret, (byte) 0);
            if (state.address() == 0) {
                byte[] message = take(api, out, length);
                throw failure(code(status.get(ValueLayout.JAVA_INT, 0)), name, new String(message, StandardCharsets.UTF_8));
            }
            return new DigestState(api, state);
        } catch (Failure failure) {
            throw failure;
        } catch (Throwable error) {
            throw failure("unexpected", name, "Host digest creation failed");
        }
    }

    public static Object runtime_host_crypto__digest_update(Object handle, List<?> bytes) {
        String name = symbol("digest_update", 2);
        Api api = api(name);
        DigestState origin = (DigestState) handle;
        try (Arena arena = Arena.ofConfined()) {
            byte[] data = Natives.toBytes(bytes);
            MemorySegment next = (MemorySegment) api.digestUpdate.invokeExact(origin.pointer, segment(arena, data),
                (long) data.length);
            java.lang.ref.Reference.reachabilityFence(origin);
            return new DigestState(api, next);
        } catch (Throwable error) {
            throw failure("unexpected", name, "Host digest update failed");
        }
    }

    public static List<Object> runtime_host_crypto__digest_bytes(Object handle, BigInteger size) {
        String name = symbol("digest_bytes", 2);
        Api api = api(name);
        DigestState origin = (DigestState) handle;
        long bound = length(size, name);
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = arena.allocate(PTR);
            MemorySegment length = arena.allocate(SIZE);
            int status = (int) api.digestFinish.invokeExact(origin.pointer, bound, out, length);
            java.lang.ref.Reference.reachabilityFence(origin);
            byte[] bytes = take(api, out, length);
            if (status != 0) {
                throw failure(code(status), name, new String(bytes, StandardCharsets.UTF_8));
            }
            return Natives.fromBytes(bytes, bytes.length);
        } catch (Failure failure) {
            throw failure;
        } catch (Throwable error) {
            throw failure("unexpected", name, "Host digest finish failed");
        }
    }
}
