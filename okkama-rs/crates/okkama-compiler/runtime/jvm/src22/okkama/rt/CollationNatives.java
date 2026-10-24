package okkama.rt;

import java.io.File;
import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.util.List;

/**
 * Natives {@code runtime/collation}: ключи сортировки закреплённой ICU 76.1
 * (Unicode 16, CLDR 46) через FFM, как у Perl-provider. Каталог библиотек —
 * {@code OKKAMA_ICU_LIBDIR}.
 */
public final class CollationNatives {
    private CollationNatives() {}

    private static final String OPERATION = "runtime/collation@1::collation_key/7";
    private static final ValueLayout.OfInt INT = ValueLayout.JAVA_INT;
    private static final java.lang.foreign.AddressLayout PTR = ValueLayout.ADDRESS;

    private static Failure failure(String code, String message) {
        return NativeFailure.raise("core.collation", code, OPERATION, message);
    }

    private static final class Icu {
        MethodHandle getVersion;
        MethodHandle unicodeVersion;
        MethodHandle cldrVersion;
        MethodHandle open;
        MethodHandle close;
        MethodHandle setAttribute;
        MethodHandle sortKey;
        MethodHandle ucaVersion;
        MethodHandle legacyType;
        MethodHandle keywordValues;
        MethodHandle next;
        MethodHandle enumClose;
    }

    private static Icu icu;
    private static Failure unavailable;

    private static synchronized Icu icu() {
        if (icu != null) {
            return icu;
        }
        if (unavailable != null) {
            throw unavailable;
        }
        String directory = Host.environment.get("OKKAMA_ICU_LIBDIR");
        String suffix = Posix.MAC ? "dylib" : Posix.SUPPORTED ? "so" : "dll";
        File uc = directory == null ? null : new File(directory, "libicuuc." + suffix);
        File i18n = directory == null ? null : new File(directory, "libicui18n." + suffix);
        if (uc == null || !uc.isFile() || !i18n.isFile()) {
            unavailable = failure("unsupported",
                "Pinned ICU 76.1 is unavailable; run tools/build-icu-collation.sh or set OKKAMA_ICU_LIBDIR");
            throw unavailable;
        }
        try {
            SymbolLookup common = SymbolLookup.libraryLookup(uc.toPath(), Arena.global());
            SymbolLookup library = SymbolLookup.libraryLookup(i18n.toPath(), Arena.global());
            SymbolLookup lookup = name -> library.find(name).or(() -> common.find(name));
            Icu api = new Icu();
            api.getVersion = handle(lookup, "u_getVersion", FunctionDescriptor.ofVoid(PTR));
            api.unicodeVersion = handle(lookup, "u_getUnicodeVersion", FunctionDescriptor.ofVoid(PTR));
            api.cldrVersion = handle(lookup, "ulocdata_getCLDRVersion", FunctionDescriptor.ofVoid(PTR, PTR));
            api.open = handle(lookup, "ucol_open", FunctionDescriptor.of(PTR, PTR, PTR));
            api.close = handle(lookup, "ucol_close", FunctionDescriptor.ofVoid(PTR));
            api.setAttribute = handle(lookup, "ucol_setAttribute", FunctionDescriptor.ofVoid(PTR, INT, INT, PTR));
            api.sortKey = handle(lookup, "ucol_getSortKey", FunctionDescriptor.of(INT, PTR, PTR, INT, PTR, INT));
            api.ucaVersion = handle(lookup, "ucol_getUCAVersion", FunctionDescriptor.ofVoid(PTR, PTR));
            api.legacyType = handle(lookup, "uloc_toLegacyType", FunctionDescriptor.of(PTR, PTR, PTR));
            api.keywordValues = handle(lookup, "ucol_getKeywordValuesForLocale",
                FunctionDescriptor.of(PTR, PTR, PTR, ValueLayout.JAVA_BYTE, PTR));
            api.next = handle(lookup, "uenum_next", FunctionDescriptor.of(PTR, PTR, PTR, PTR));
            api.enumClose = handle(lookup, "uenum_close", FunctionDescriptor.ofVoid(PTR));
            if (!version(api.getVersion, false).equals("76.1.0.0") || !version(api.unicodeVersion, false).equals("16.0.0.0")
                    || !version(api.cldrVersion, true).equals("46.0.0.0")) {
                unavailable = failure("unsupported", "ICU/Unicode/CLDR data version mismatch");
                throw unavailable;
            }
            icu = api;
            return api;
        } catch (Failure failure) {
            throw failure;
        } catch (Throwable error) {
            unavailable = failure("unsupported", "ICU does not export the pinned 76.1 API");
            throw unavailable;
        }
    }

    private static MethodHandle handle(SymbolLookup lookup, String name, FunctionDescriptor descriptor) {
        return Posix.LINKER.downcallHandle(lookup.find(name + "_76").orElseThrow(), descriptor);
    }

    private static String version(MethodHandle function, boolean status) throws Throwable {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment info = arena.allocate(4);
            if (status) {
                MemorySegment code = arena.allocate(INT);
                function.invokeExact(info, code);
                if (code.get(INT, 0) > 0) {
                    throw failure("unsupported", "Cannot read pinned ICU version");
                }
            } else {
                function.invokeExact(info);
            }
            return (info.get(ValueLayout.JAVA_BYTE, 0) & 0xff) + "." + (info.get(ValueLayout.JAVA_BYTE, 1) & 0xff) + "."
                + (info.get(ValueLayout.JAVA_BYTE, 2) & 0xff) + "." + (info.get(ValueLayout.JAVA_BYTE, 3) & 0xff);
        }
    }

    private static final java.util.regex.Pattern DIGITS = java.util.regex.Pattern.compile("\\p{Nd}{255}");

    public static List<Object> runtime_collation__collation_key(String text, String locale, String collation,
            java.math.BigInteger strengthValue, java.math.BigInteger caseValue, boolean numeric, boolean normalization) {
        int strength = strengthValue.bitLength() > 31 ? -99 : strengthValue.intValue();
        int caseFirst = caseValue.bitLength() > 31 ? -99 : caseValue.intValue();
        if (!locale.matches("[A-Za-z0-9-]{1,255}") || !collation.matches("(?:[a-z0-9-]{3,64})?")
                || !(strength == 0 || strength == 1 || strength == 2 || strength == 3 || strength == 15)) {
            throw failure("invalid_data", "Invalid collation coordinates");
        }
        if (!(caseFirst == -1 || caseFirst == 24 || caseFirst == 25)) {
            throw failure("invalid_data", "Invalid collation policy");
        }
        if (text.codePointCount(0, text.length()) > 8192) {
            throw failure("resource_exhausted", "Collation text exceeds 8192 scalars");
        }
        if (numeric && DIGITS.matcher(text).find()) {
            throw failure("invalid_data", "Numeric runs exceed ICU limit of 254 digits");
        }
        Icu api = icu();
        String id = locale.equals("und") ? "root" : locale.replace('-', '_');
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment status = arena.allocate(INT);
            if (!collation.isEmpty()) {
                MemorySegment legacy = (MemorySegment) api.legacyType.invokeExact(arena.allocateFrom("collation"),
                    arena.allocateFrom(collation));
                String type = Posix.string(legacy);
                if (type == null) {
                    throw failure("invalid_data", "Unknown Unicode collation type");
                }
                MemorySegment values = (MemorySegment) api.keywordValues.invokeExact(arena.allocateFrom("collation"),
                    arena.allocateFrom(id), (byte) 0, status);
                if (status.get(INT, 0) > 0 || values.address() == 0) {
                    throw failure("invalid_data", "Cannot enumerate locale collations");
                }
                boolean found = false;
                MemorySegment length = arena.allocate(INT);
                while (true) {
                    String value = Posix.string((MemorySegment) api.next.invokeExact(values, length, status));
                    if (value == null) {
                        break;
                    }
                    found |= value.equals(type);
                }
                api.enumClose.invokeExact(values);
                if (status.get(INT, 0) > 0 || !found) {
                    throw failure("invalid_data", "Unsupported collation for locale");
                }
                id = id + "@collation=" + type;
            }
            status.set(INT, 0, 0);
            MemorySegment collator = (MemorySegment) api.open.invokeExact(arena.allocateFrom(id), status);
            if (status.get(INT, 0) > 0 || collator.address() == 0) {
                if (collator.address() != 0) {
                    api.close.invokeExact(collator);
                }
                throw failure("invalid_data", "Cannot open requested collation");
            }
            try {
                MemorySegment version = arena.allocate(4);
                api.ucaVersion.invokeExact(collator, version);
                if (version.get(ValueLayout.JAVA_BYTE, 0) != 16 || version.get(ValueLayout.JAVA_BYTE, 1) != 0
                        || version.get(ValueLayout.JAVA_BYTE, 2) != 0 || version.get(ValueLayout.JAVA_BYTE, 3) != 0) {
                    throw failure("unsupported", "UCA data version mismatch");
                }
                int[][] settings = {{5, strength}, {2, caseFirst}, {7, numeric ? 17 : 16}, {4, normalization ? 17 : 16}};
                for (int[] setting : settings) {
                    status.set(INT, 0, 0);
                    api.setAttribute.invokeExact(collator, setting[0], setting[1], status);
                    if (status.get(INT, 0) > 0) {
                        throw failure("invalid_data", "Invalid collation attribute");
                    }
                }
                MemorySegment input = arena.allocate(ValueLayout.JAVA_CHAR, Math.max(1, text.length()));
                for (int index = 0; index < text.length(); index++) {
                    input.setAtIndex(ValueLayout.JAVA_CHAR, index, text.charAt(index));
                }
                int size = (int) api.sortKey.invokeExact(collator, input, text.length(), MemorySegment.NULL, 0);
                if (size <= 0 || size > 262144) {
                    throw failure("resource_exhausted", "Collation key exceeds 262144 bytes");
                }
                MemorySegment output = arena.allocate(size);
                int actual = (int) api.sortKey.invokeExact(collator, input, text.length(), output, size);
                byte[] key = output.toArray(ValueLayout.JAVA_BYTE);
                if (actual != size || key[size - 1] != 0) {
                    throw failure("invalid_data", "Inconsistent ICU sort key");
                }
                return Natives.fromBytes(key, key.length);
            } finally {
                api.close.invokeExact(collator);
            }
        } catch (Failure failure) {
            throw failure;
        } catch (Throwable error) {
            throw failure("unexpected", "Cannot compute pinned collation key");
        }
    }
}
