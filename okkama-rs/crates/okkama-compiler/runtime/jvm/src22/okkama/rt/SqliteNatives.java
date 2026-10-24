package okkama.rt;

import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.math.MathContext;
import java.math.RoundingMode;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.Callable;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.RejectedExecutionException;

/**
 * Natives {@code runtime/database} (provider {@code optional.sqlite}):
 * системная libsqlite3 через Java FFM (JDK 22+). Как у Perl-provider, каждое
 * соединение обслуживает свой рабочий поток; операции идут по очереди, их
 * исход приходит событием цикла {@code async main}. Ответ — {@code ok} и
 * результат либо отказ {@code kind}/{@code code} (SQLITE_…).
 */
public final class SqliteNatives {
    private SqliteNatives() {}

    private static final String DOMAIN = "optional.sqlite";
    private static final boolean TRACE = "1".equals(System.getenv("OKKAMA_TRACE_THROW"));

    private static Failure fail(String code, String name, int arity, String message) {
        return NativeFailure.raise(DOMAIN, code, "runtime/database@1::" + name + "/" + arity, message);
    }

    // ---- libsqlite3 через FFM ----

    private static final class Api {
        static final Linker LINKER = Linker.nativeLinker();
        static final ValueLayout.OfInt INT = ValueLayout.JAVA_INT;
        static final ValueLayout.OfLong LONG = ValueLayout.JAVA_LONG;
        static final ValueLayout.OfDouble DOUBLE = ValueLayout.JAVA_DOUBLE;
        static final java.lang.foreign.AddressLayout PTR = ValueLayout.ADDRESS;
        static final SymbolLookup LIBRARY = library();

        static SymbolLookup library() {
            String[] candidates = {"/usr/lib/libsqlite3.dylib", "libsqlite3.so.0", "libsqlite3.so", "sqlite3"};
            for (String candidate : candidates) {
                try {
                    return SymbolLookup.libraryLookup(candidate, Arena.global());
                } catch (IllegalArgumentException missing) {
                    // следующий кандидат
                }
            }
            return null;
        }

        static MethodHandle function(String name, FunctionDescriptor descriptor) {
            if (LIBRARY == null) {
                return null;
            }
            Optional<MemorySegment> symbol = LIBRARY.find(name);
            return symbol.map(address -> LINKER.downcallHandle(address, descriptor)).orElse(null);
        }

        static final MethodHandle libversion = function("sqlite3_libversion_number", FunctionDescriptor.of(INT));
        static final MethodHandle open = function("sqlite3_open_v2", FunctionDescriptor.of(INT, PTR, PTR, INT, PTR));
        static final MethodHandle close = function("sqlite3_close", FunctionDescriptor.of(INT, PTR));
        static final MethodHandle extendedCodes = function("sqlite3_extended_result_codes", FunctionDescriptor.of(INT, PTR, INT));
        static final MethodHandle extendedErrcode = function("sqlite3_extended_errcode", FunctionDescriptor.of(INT, PTR));
        static final MethodHandle autocommit = function("sqlite3_get_autocommit", FunctionDescriptor.of(INT, PTR));
        static final MethodHandle prepare = function("sqlite3_prepare_v3", FunctionDescriptor.of(INT, PTR, PTR, INT, INT, PTR, PTR));
        static final MethodHandle step = function("sqlite3_step", FunctionDescriptor.of(INT, PTR));
        static final MethodHandle finalize = function("sqlite3_finalize", FunctionDescriptor.of(INT, PTR));
        static final MethodHandle reset = function("sqlite3_reset", FunctionDescriptor.of(INT, PTR));
        static final MethodHandle clearBindings = function("sqlite3_clear_bindings", FunctionDescriptor.of(INT, PTR));
        static final MethodHandle parameterCount = function("sqlite3_bind_parameter_count", FunctionDescriptor.of(INT, PTR));
        static final MethodHandle bindNull = function("sqlite3_bind_null", FunctionDescriptor.of(INT, PTR, INT));
        static final MethodHandle bindInt64 = function("sqlite3_bind_int64", FunctionDescriptor.of(INT, PTR, INT, LONG));
        static final MethodHandle bindDouble = function("sqlite3_bind_double", FunctionDescriptor.of(INT, PTR, INT, DOUBLE));
        static final MethodHandle bindText = function("sqlite3_bind_text", FunctionDescriptor.of(INT, PTR, INT, PTR, INT, PTR));
        static final MethodHandle bindBlob = function("sqlite3_bind_blob", FunctionDescriptor.of(INT, PTR, INT, PTR, INT, PTR));
        static final MethodHandle columnCount = function("sqlite3_column_count", FunctionDescriptor.of(INT, PTR));
        static final MethodHandle columnName = function("sqlite3_column_name", FunctionDescriptor.of(PTR, PTR, INT));
        static final MethodHandle columnDecltype = function("sqlite3_column_decltype", FunctionDescriptor.of(PTR, PTR, INT));
        static final MethodHandle columnType = function("sqlite3_column_type", FunctionDescriptor.of(INT, PTR, INT));
        static final MethodHandle columnInt64 = function("sqlite3_column_int64", FunctionDescriptor.of(LONG, PTR, INT));
        static final MethodHandle columnDouble = function("sqlite3_column_double", FunctionDescriptor.of(DOUBLE, PTR, INT));
        static final MethodHandle columnText = function("sqlite3_column_text", FunctionDescriptor.of(PTR, PTR, INT));
        static final MethodHandle columnBlob = function("sqlite3_column_blob", FunctionDescriptor.of(PTR, PTR, INT));
        static final MethodHandle columnBytes = function("sqlite3_column_bytes", FunctionDescriptor.of(INT, PTR, INT));
        static final MethodHandle changes = function("sqlite3_changes64", FunctionDescriptor.of(LONG, PTR));
        static final MethodHandle authorizer = function("sqlite3_set_authorizer", FunctionDescriptor.of(INT, PTR, PTR, PTR));
        static final MethodHandle interrupt = function("sqlite3_interrupt", FunctionDescriptor.ofVoid(PTR));
        static final MethodHandle compileOption = function("sqlite3_compileoption_used", FunctionDescriptor.of(INT, PTR));
        static final MethodHandle limit = function("sqlite3_limit", FunctionDescriptor.of(INT, PTR, INT, INT));
        static final MethodHandle busyTimeout = function("sqlite3_busy_timeout", FunctionDescriptor.of(INT, PTR, INT));
        static final MethodHandle loadExtension = function("sqlite3_enable_load_extension", FunctionDescriptor.of(INT, PTR, INT));
        static final MethodHandle blobOpen = function("sqlite3_blob_open", FunctionDescriptor.of(INT, PTR, PTR, PTR, PTR, LONG, INT, PTR));
        static final MethodHandle blobBytes = function("sqlite3_blob_bytes", FunctionDescriptor.of(INT, PTR));
        static final MethodHandle blobClose = function("sqlite3_blob_close", FunctionDescriptor.of(INT, PTR));
        static final MethodHandle blobRead = function("sqlite3_blob_read", FunctionDescriptor.of(INT, PTR, PTR, INT, INT));
        static final MethodHandle blobWrite = function("sqlite3_blob_write", FunctionDescriptor.of(INT, PTR, PTR, INT, INT));
        static final MethodHandle backupInit = function("sqlite3_backup_init", FunctionDescriptor.of(PTR, PTR, PTR, PTR, PTR));
        static final MethodHandle backupStep = function("sqlite3_backup_step", FunctionDescriptor.of(INT, PTR, INT));
        static final MethodHandle backupFinish = function("sqlite3_backup_finish", FunctionDescriptor.of(INT, PTR));
        static final MethodHandle backupRemaining = function("sqlite3_backup_remaining", FunctionDescriptor.of(INT, PTR));
        static final MethodHandle backupPagecount = function("sqlite3_backup_pagecount", FunctionDescriptor.of(INT, PTR));

        static boolean complete() {
            MethodHandle[] all = {libversion, open, close, extendedCodes, extendedErrcode, autocommit, prepare, step,
                finalize, reset, clearBindings, parameterCount, bindNull, bindInt64, bindDouble, bindText, bindBlob,
                columnCount, columnName, columnDecltype, columnType, columnInt64, columnDouble, columnText, columnBlob,
                columnBytes, changes, authorizer, interrupt, compileOption, limit, busyTimeout, blobOpen, blobBytes,
                blobClose, blobRead, blobWrite, backupInit, backupStep, backupFinish, backupRemaining, backupPagecount};
            for (MethodHandle handle : all) {
                if (handle == null) {
                    return false;
                }
            }
            return true;
        }
    }

    /** Отказ движка: вид, код SQLITE_… и признак autocommit. */
    private static final class SqliteFailure extends RuntimeException {
        private static final long serialVersionUID = 1L;
        final String kind;
        final String code;
        final boolean autocommit;

        SqliteFailure(String kind, String code, boolean autocommit) {
            super(code, null, false, false);
            this.kind = kind;
            this.code = code;
            this.autocommit = autocommit;
        }
    }

    private static int call(MethodHandle handle, Object... args) {
        try {
            return (int) handle.invokeWithArguments(args);
        } catch (RuntimeException | java.lang.Error error) {
            throw error;
        } catch (Throwable error) {
            throw new IllegalStateException(error);
        }
    }

    private static Object invoke(MethodHandle handle, Object... args) {
        try {
            return handle.invokeWithArguments(args);
        } catch (RuntimeException | java.lang.Error error) {
            throw error;
        } catch (Throwable error) {
            throw new IllegalStateException(error);
        }
    }

    private static String cString(MemorySegment pointer) {
        if (pointer == null || pointer.equals(MemorySegment.NULL)) {
            return null;
        }
        return pointer.reinterpret(Long.MAX_VALUE).getString(0, StandardCharsets.UTF_8);
    }

    private static byte[] cBytes(MemorySegment pointer) {
        if (pointer == null || pointer.equals(MemorySegment.NULL)) {
            return null;
        }
        MemorySegment unbounded = pointer.reinterpret(Long.MAX_VALUE);
        long length = 0;
        while (unbounded.get(ValueLayout.JAVA_BYTE, length) != 0) {
            length++;
        }
        return unbounded.asSlice(0, length).toArray(ValueLayout.JAVA_BYTE);
    }

    private static String strictText(byte[] bytes) {
        try {
            return StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(bytes)).toString();
        } catch (CharacterCodingException invalid) {
            return null;
        }
    }

    /** {@code sprintf('%.17g', number)} языка C. */
    static String formatG17(double number) {
        if (number == 0) {
            return 1 / number < 0 ? "-0" : "0";
        }
        BigDecimal exact = new BigDecimal(number);
        BigDecimal rounded = exact.round(new MathContext(17, RoundingMode.HALF_EVEN));
        int exponent = rounded.precision() - rounded.scale() - 1;
        String digits = rounded.unscaledValue().abs().toString();
        while (digits.length() < 17) {
            digits += "0";
        }
        String sign = rounded.signum() < 0 ? "-" : "";
        if (exponent < -4 || exponent >= 17) {
            String mantissa = digits.substring(0, 1) + "." + digits.substring(1);
            mantissa = mantissa.replaceAll("0+$", "").replaceAll("\\.$", "");
            String power = Integer.toString(Math.abs(exponent));
            if (power.length() < 2) {
                power = "0" + power;
            }
            return sign + mantissa + "e" + (exponent < 0 ? "-" : "+") + power;
        }
        String plain = rounded.abs().setScale(Math.max(0, 16 - exponent), RoundingMode.HALF_EVEN).toPlainString();
        if (plain.indexOf('.') >= 0) {
            plain = plain.replaceAll("0+$", "").replaceAll("\\.$", "");
        }
        return sign + plain;
    }

    // ---- движок: одно соединение, вызывается только из его потока ----

    private static final int[] SCHEMA_ACTION = {1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 15, 16, 17, 26, 27, 28, 29, 30};

    static final class Statement {
        MemorySegment pointer;
        boolean dml;
        int columns;
        long prepared;
        long id;
        boolean verified;
        List<String> rawNames;
        List<String> rawDeclarations;
        Object[] pendingRow;
        long pendingBytes;
    }

    static final class Engine {
        final Arena arena = Arena.ofShared();
        MemorySegment db = MemorySegment.NULL;
        final Connection connection;
        long serial;
        Statement cursor;
        final Map<Long, Statement> prepared = new LinkedHashMap<>();
        final Map<Long, Object[]> blobs = new LinkedHashMap<>();
        Object[] backup;
        boolean tailGuard;
        boolean control;
        boolean configuration;
        boolean dmlSeen;
        boolean ddlSeen;
        volatile boolean cancelled;
        MemorySegment authorizerStub;

        Engine(Connection connection) {
            this.connection = connection;
        }

        boolean autocommit() {
            return db.equals(MemorySegment.NULL) || call(Api.autocommit, db) != 0;
        }

        SqliteFailure raise(String kind, String code) {
            return new SqliteFailure(kind, code, autocommit());
        }

        SqliteFailure error(int rc) {
            int base = rc & 255;
            String kind = base == 5 || base == 6 ? "busy" : base == 19 ? "constraint" : base == 1 ? "syntax"
                : base == 9 ? "cancelled" : base == 7 || base == 18 ? "resource"
                : base == 3 || base == 8 || base == 23 ? "permission" : base == 14 ? "connection"
                : base == 20 || base == 21 || base == 25 ? "data" : "unexpected";
            return raise(kind, "SQLITE_" + rc);
        }

        void check(int rc) {
            if (rc != 0) {
                throw error(rc);
            }
        }

        int authorize(MemorySegment unused, int action, MemorySegment a, MemorySegment b, MemorySegment database,
                MemorySegment trigger) {
            if (tailGuard) {
                return 1;
            }
            if ((action == 22 || action == 32) && !control) {
                return 1;
            }
            if ((action == 19 || action == 24 || action == 25) && !configuration) {
                return 1;
            }
            for (int schema : SCHEMA_ACTION) {
                if (schema == action) {
                    ddlSeen = true;
                }
            }
            if (action == 9 || action == 18 || action == 23) {
                String table = cString(a);
                if (table != null && !table.equals("sqlite_master") && !table.equals("sqlite_schema")
                        && (trigger == null || trigger.equals(MemorySegment.NULL))) {
                    dmlSeen = true;
                }
            }
            return 0;
        }

        void open() {
            if (!Api.complete()) {
                throw new SqliteFailure("unsupported", Api.LIBRARY == null ? "SQLITE_UNAVAILABLE" : "SQLITE_CAPABILITY", true);
            }
            if (call(Api.libversion) < 3041000) {
                throw new SqliteFailure("unsupported", "SQLITE_VERSION", true);
            }
            MemorySegment slot = arena.allocate(Api.PTR);
            int flags = connection.readOnly ? 1 : 2 | (connection.create ? 4 : 0);
            int rc = call(Api.open, arena.allocateFrom(connection.path), slot, flags, MemorySegment.NULL);
            db = slot.get(Api.PTR, 0);
            if (rc != 0) {
                close();
                throw new SqliteFailure("connection", "SQLITE_" + rc, true);
            }
            call(Api.extendedCodes, db, 1);
            if (Api.loadExtension != null) {
                check(call(Api.loadExtension, db, 0));
            } else if (call(Api.compileOption, arena.allocateFrom("OMIT_LOAD_EXTENSION")) == 0) {
                close();
                throw new SqliteFailure("unsupported", "SQLITE_EXTENSION_POLICY", true);
            }
            int[][] limits = {{0, 8388608}, {1, 1048576}, {2, 1024}, {9, 1024}};
            for (int[] limit : limits) {
                call(Api.limit, db, limit[0], limit[1]);
            }
            call(Api.busyTimeout, db, connection.busyMs);
            try {
                MethodHandle target = MethodHandles.lookup().findVirtual(Engine.class, "authorize",
                    MethodType.methodType(int.class, MemorySegment.class, int.class, MemorySegment.class,
                        MemorySegment.class, MemorySegment.class, MemorySegment.class)).bindTo(this);
                authorizerStub = Api.LINKER.upcallStub(target, FunctionDescriptor.of(Api.INT, Api.PTR, Api.INT,
                    Api.PTR, Api.PTR, Api.PTR, Api.PTR), arena);
            } catch (ReflectiveOperationException error) {
                throw new IllegalStateException(error);
            }
            call(Api.authorizer, db, authorizerStub, MemorySegment.NULL);
            try {
                configuration = true;
                execution("PRAGMA foreign_keys=" + (connection.foreignKeys ? 1 : 0), new ArrayList<>(), false);
                Map<String, Object> foreign = rows("PRAGMA foreign_keys", new ArrayList<>());
                Map<String, Object> seen = fetch((Long) foreign.get("id"), 1);
                release((Long) foreign.get("id"));
                if (!firstCell(seen).equals(connection.foreignKeys ? "1" : "0")) {
                    throw raise("unsupported", "SQLITE_FOREIGN_KEYS");
                }
                String mode = connection.journal;
                Map<String, Object> journal = rows("PRAGMA journal_mode=" + mode, new ArrayList<>());
                Map<String, Object> actual = fetch((Long) journal.get("id"), 1);
                release((Long) journal.get("id"));
                if (!firstCell(actual).equals(mode)) {
                    throw raise("unsupported", "SQLITE_JOURNAL");
                }
            } catch (RuntimeException error) {
                close();
                throw error;
            } finally {
                configuration = false;
            }
        }

        @SuppressWarnings("unchecked")
        private static String firstCell(Map<String, Object> reply) {
            List<List<Object[]>> rows = (List<List<Object[]>>) reply.get("rows");
            if (rows.isEmpty() || rows.get(0).isEmpty()) {
                return "";
            }
            return String.valueOf(rows.get(0).get(0)[1]);
        }

        void ready() {
            if (db.equals(MemorySegment.NULL)) {
                throw raise("connection", "SQLITE_CLOSED");
            }
            if (cursor != null) {
                throw raise("busy", "SQLITE_READER_ACTIVE");
            }
        }

        Statement prepare(String sql) {
            ready();
            byte[] bytes = sql.getBytes(StandardCharsets.UTF_8);
            if (bytes.length > 1048576 || sql.indexOf('\0') >= 0) {
                throw raise("data", "SQLITE_SQL_LIMIT");
            }
            try (Arena local = Arena.ofConfined()) {
                MemorySegment text = local.allocate(bytes.length + 1L);
                MemorySegment.copy(bytes, 0, text, ValueLayout.JAVA_BYTE, 0, bytes.length);
                MemorySegment statementSlot = local.allocate(Api.PTR);
                MemorySegment tailSlot = local.allocate(Api.PTR);
                dmlSeen = false;
                ddlSeen = false;
                int rc = call(Api.prepare, db, text, bytes.length, 0, statementSlot, tailSlot);
                MemorySegment statement = statementSlot.get(Api.PTR, 0);
                if (rc != 0) {
                    if (!statement.equals(MemorySegment.NULL)) {
                        call(Api.finalize, statement);
                    }
                    throw error(rc);
                }
                if (statement.equals(MemorySegment.NULL)) {
                    throw raise("syntax", "SQLITE_EMPTY_STATEMENT");
                }
                boolean dml = dmlSeen && !ddlSeen;
                long offset = tailSlot.get(Api.PTR, 0).address() - text.address();
                try {
                    if (offset < 0 || offset > bytes.length) {
                        throw raise("unexpected", "SQLITE_PREPARE_TAIL");
                    }
                    if (offset < bytes.length) {
                        tailGuard = true;
                        try {
                            MemorySegment otherSlot = local.allocate(Api.PTR);
                            int extra = call(Api.prepare, db, text.asSlice(offset), (int) (bytes.length - offset), 0,
                                otherSlot, tailSlot);
                            MemorySegment other = otherSlot.get(Api.PTR, 0);
                            if (!other.equals(MemorySegment.NULL)) {
                                call(Api.finalize, other);
                            }
                            if (extra != 0 || !other.equals(MemorySegment.NULL)) {
                                throw raise("syntax", "SQLITE_MULTIPLE_STATEMENTS");
                            }
                        } finally {
                            tailGuard = false;
                        }
                    }
                } catch (RuntimeException failure) {
                    call(Api.finalize, statement);
                    throw failure;
                }
                Statement result = new Statement();
                result.pointer = statement;
                result.dml = dml;
                result.columns = call(Api.columnCount, statement);
                return result;
            }
        }

        void bind(Statement statement, List<Object[]> params) {
            if (params.size() != call(Api.parameterCount, statement.pointer)) {
                throw raise("data", "SQLITE_BIND_COUNT");
            }
            for (int at = 0; at < params.size(); at++) {
                String kind = (String) params.get(at)[0];
                Object value = params.get(at)[1];
                int index = at + 1;
                int rc;
                if (kind.equals("null_value")) {
                    rc = call(Api.bindNull, statement.pointer, index);
                } else if (kind.equals("integer") || kind.equals("boolean")) {
                    String text = (String) value;
                    if (!text.matches("-?(?:0|[1-9][0-9]*)") || text.length() > 20) {
                        throw raise("data", "SQLITE_INTEGER_RANGE");
                    }
                    BigInteger number = new BigInteger(text);
                    if (number.bitLength() > 63) {
                        throw raise("data", "SQLITE_INTEGER_RANGE");
                    }
                    rc = call(Api.bindInt64, statement.pointer, index, number.longValue());
                } else if (kind.equals("floating")) {
                    double number;
                    try {
                        number = Double.parseDouble(((String) value).trim());
                    } catch (NumberFormatException invalid) {
                        number = 0;
                    }
                    if (Double.isNaN(number) || Double.isInfinite(number)) {
                        throw raise("data", "SQLITE_FLOAT_RANGE");
                    }
                    rc = call(Api.bindDouble, statement.pointer, index, number);
                } else if (kind.equals("text") || kind.equals("decimal") || kind.equals("bytes")) {
                    byte[] bytes = kind.equals("bytes") ? (byte[]) value : ((String) value).getBytes(StandardCharsets.UTF_8);
                    if (bytes.length > 8388608) {
                        throw raise("resource", "SQLITE_VALUE_LIMIT");
                    }
                    try (Arena local = Arena.ofConfined()) {
                        MemorySegment data = local.allocate(Math.max(1, bytes.length));
                        MemorySegment.copy(bytes, 0, data, ValueLayout.JAVA_BYTE, 0, bytes.length);
                        MemorySegment transient_ = MemorySegment.ofAddress(-1L);
                        rc = call(kind.equals("bytes") ? Api.bindBlob : Api.bindText, statement.pointer, index, data,
                            bytes.length, transient_);
                    }
                } else {
                    throw raise("data", "SQLITE_VALUE_KIND");
                }
                check(rc);
            }
        }

        Map<String, Object> execution(String sql, List<Object[]> params, boolean control) {
            boolean previous = this.control;
            this.control = control;
            try {
                return execute(prepare(sql), params);
            } finally {
                this.control = previous;
            }
        }

        Map<String, Object> execute(Statement statement, List<Object[]> params) {
            Map<String, Object> result = new HashMap<>();
            try {
                if (statement.columns != 0) {
                    throw raise("data", "SQLITE_ROWS_REQUIRED");
                }
                bind(statement, params);
                int rc = call(Api.step, statement.pointer);
                if (rc != 101) {
                    throw error(rc);
                }
                result.put("affected", statement.dml ? Long.toString((long) invoke(Api.changes, db)) : null);
                result.put("autocommit", autocommit());
            } finally {
                finish(statement);
            }
            return result;
        }

        Map<String, Object> rows(String sql, List<Object[]> params) {
            return rows(prepare(sql), params);
        }

        Map<String, Object> rows(Statement statement, List<Object[]> params) {
            try {
                statement.columns = call(Api.columnCount, statement.pointer);
                if (statement.columns == 0) {
                    throw raise("data", "SQLITE_EXECUTION_REQUIRED");
                }
                bind(statement, params);
                List<String> names = new ArrayList<>();
                List<String> declarations = new ArrayList<>();
                List<String> rawNames = new ArrayList<>();
                List<String> rawDeclarations = new ArrayList<>();
                long used = 0;
                for (int column = 0; column < statement.columns; column++) {
                    byte[] name = cBytes((MemorySegment) invoke(Api.columnName, statement.pointer, column));
                    byte[] declaration = cBytes((MemorySegment) invoke(Api.columnDecltype, statement.pointer, column));
                    if (name == null) {
                        name = new byte[0];
                    }
                    if (declaration == null) {
                        declaration = new byte[0];
                    }
                    used += name.length + declaration.length;
                    if (name.length > 4096 || declaration.length > 4096 || used > 1048576) {
                        throw raise("resource", "SQLITE_COLUMN_LIMIT");
                    }
                    String nameText = strictText(name);
                    String declarationText = strictText(declaration);
                    if (nameText == null || declarationText == null) {
                        throw raise("data", "SQLITE_UTF8");
                    }
                    rawNames.add(new String(name, StandardCharsets.ISO_8859_1));
                    rawDeclarations.add(new String(declaration, StandardCharsets.ISO_8859_1));
                    names.add(nameText);
                    declarations.add(declarationText);
                }
                statement.verified = false;
                statement.rawNames = rawNames;
                statement.rawDeclarations = rawDeclarations;
                statement.id = ++serial;
                cursor = statement;
                Map<String, Object> result = new HashMap<>();
                result.put("id", statement.id);
                result.put("names", names);
                result.put("declarations", declarations);
                return result;
            } catch (RuntimeException failure) {
                finish(statement);
                throw failure;
            }
        }

        void verify(Statement cursor) {
            if (cursor.verified) {
                return;
            }
            if (call(Api.columnCount, cursor.pointer) != cursor.columns) {
                throw raise("data", "SQLITE_SCHEMA_CHANGED");
            }
            for (int column = 0; column < cursor.columns; column++) {
                byte[] name = cBytes((MemorySegment) invoke(Api.columnName, cursor.pointer, column));
                byte[] declaration = cBytes((MemorySegment) invoke(Api.columnDecltype, cursor.pointer, column));
                String nameText = new String(name == null ? new byte[0] : name, StandardCharsets.ISO_8859_1);
                String declarationText = new String(declaration == null ? new byte[0] : declaration,
                    StandardCharsets.ISO_8859_1);
                if (!nameText.equals(cursor.rawNames.get(column))
                        || !declarationText.equals(cursor.rawDeclarations.get(column))) {
                    throw raise("data", "SQLITE_SCHEMA_CHANGED");
                }
            }
            cursor.verified = true;
        }

        Map<String, Object> fetch(long id, int maximum) {
            Statement cursor = this.cursor;
            if (cursor == null || cursor.id != id) {
                throw raise("data", "SQLITE_READER_STALE");
            }
            if (maximum < 1 || maximum > 64) {
                throw raise("data", "SQLITE_BATCH_LIMIT");
            }
            List<List<Object[]>> rows = new ArrayList<>();
            boolean eof = false;
            long used = 0;
            if (cursor.pendingRow != null) {
                @SuppressWarnings("unchecked")
                List<Object[]> values = (List<Object[]>) cursor.pendingRow[0];
                rows.add(values);
                used = cursor.pendingBytes;
                cursor.pendingRow = null;
            }
            try {
                while (rows.size() < maximum) {
                    int rc = call(Api.step, cursor.pointer);
                    if (rc == 101) {
                        verify(cursor);
                        eof = true;
                        break;
                    }
                    if (rc != 100) {
                        throw error(rc);
                    }
                    verify(cursor);
                    List<Object[]> values = new ArrayList<>();
                    long rowBytes = 0;
                    for (int column = 0; column < cursor.columns; column++) {
                        int type = call(Api.columnType, cursor.pointer, column);
                        String kind;
                        Object value;
                        if (type == 5) {
                            kind = "null_value";
                            value = "";
                        } else if (type == 1) {
                            kind = "integer";
                            value = Long.toString((long) invoke(Api.columnInt64, cursor.pointer, column));
                            rowBytes += ((String) value).length();
                        } else if (type == 2) {
                            double number = (double) invoke(Api.columnDouble, cursor.pointer, column);
                            if (Double.isNaN(number) || Double.isInfinite(number)) {
                                throw raise("data", "SQLITE_FLOAT_RANGE");
                            }
                            kind = "floating";
                            value = formatG17(number);
                            rowBytes += ((String) value).length();
                        } else if (type == 3 || type == 4) {
                            MemorySegment pointer = (MemorySegment) invoke(type == 3 ? Api.columnText : Api.columnBlob,
                                cursor.pointer, column);
                            int size = call(Api.columnBytes, cursor.pointer, column);
                            if (size < 0 || size > 8388608 - rowBytes) {
                                throw raise("resource", "SQLITE_ROW_LIMIT");
                            }
                            byte[] bytes = size == 0 ? new byte[0]
                                : pointer.reinterpret(size).toArray(ValueLayout.JAVA_BYTE);
                            if (type == 3) {
                                String text = strictText(bytes);
                                if (text == null) {
                                    throw raise("data", "SQLITE_UTF8");
                                }
                                kind = "text";
                                value = text;
                            } else {
                                kind = "bytes";
                                value = bytes;
                            }
                            rowBytes += size;
                        } else {
                            throw raise("unexpected", "SQLITE_STORAGE_KIND");
                        }
                        if (rowBytes > 8388608) {
                            throw raise("resource", "SQLITE_ROW_LIMIT");
                        }
                        values.add(new Object[] {kind, value});
                    }
                    if (used + rowBytes > 8388608) {
                        cursor.pendingRow = new Object[] {values};
                        cursor.pendingBytes = rowBytes;
                        break;
                    }
                    rows.add(values);
                    used += rowBytes;
                }
            } catch (RuntimeException failure) {
                release(id);
                throw failure;
            }
            if (eof) {
                release(id);
            }
            Map<String, Object> result = new HashMap<>();
            result.put("rows", rows);
            result.put("eof", eof);
            result.put("autocommit", autocommit());
            return result;
        }

        void release(long id) {
            Statement cursor = this.cursor;
            if (cursor == null || cursor.id != id) {
                return;
            }
            this.cursor = null;
            finish(cursor);
        }

        void finish(Statement statement) {
            statement.pendingRow = null;
            if (statement.prepared != 0) {
                call(Api.reset, statement.pointer);
                check(call(Api.clearBindings, statement.pointer));
            } else {
                check(call(Api.finalize, statement.pointer));
            }
        }

        Map<String, Object> prepareStatement(String sql) {
            if (prepared.size() >= 64) {
                throw raise("resource", "SQLITE_PREPARED_LIMIT");
            }
            Statement statement = prepare(sql);
            long id = ++serial;
            statement.prepared = id;
            prepared.put(id, statement);
            Map<String, Object> result = new HashMap<>();
            result.put("id", id);
            return result;
        }

        Statement preparedStatement(long id) {
            ready();
            Statement statement = prepared.get(id);
            if (statement == null) {
                throw raise("data", "SQLITE_PREPARED_STALE");
            }
            return statement;
        }

        Map<String, Object> unprepare(long id) {
            Statement statement = prepared.remove(id);
            if (statement == null) {
                return new HashMap<>();
            }
            if (cursor != null && cursor.pointer.equals(statement.pointer)) {
                release(cursor.id);
            }
            check(call(Api.finalize, statement.pointer));
            return new HashMap<>();
        }

        private void identifier(String name) {
            if (name == null || name.isEmpty() || name.getBytes(StandardCharsets.UTF_8).length > 1024
                    || name.indexOf('\0') >= 0) {
                throw raise("data", "SQLITE_IDENTIFIER");
            }
        }

        private long int64(String value) {
            if (value == null || !value.matches("-?(?:0|[1-9][0-9]*)") || value.length() > 20) {
                throw raise("data", "SQLITE_INTEGER_RANGE");
            }
            BigInteger number = new BigInteger(value);
            if (number.bitLength() > 63) {
                throw raise("data", "SQLITE_INTEGER_RANGE");
            }
            return number.longValue();
        }

        Map<String, Object> blobOpen(String database, String table, String column, String row, boolean writable) {
            ready();
            if (!blobs.isEmpty()) {
                throw raise("busy", "SQLITE_BLOB_ACTIVE");
            }
            identifier(database);
            identifier(table);
            identifier(column);
            long rowid = int64(row);
            try (Arena local = Arena.ofConfined()) {
                MemorySegment slot = local.allocate(Api.PTR);
                int rc = call(Api.blobOpen, db, local.allocateFrom(database), local.allocateFrom(table),
                    local.allocateFrom(column), rowid, writable ? 1 : 0, slot);
                MemorySegment blob = slot.get(Api.PTR, 0);
                if (rc != 0) {
                    if (!blob.equals(MemorySegment.NULL)) {
                        call(Api.blobClose, blob);
                    }
                    throw error(rc);
                }
                long id = ++serial;
                int size = call(Api.blobBytes, blob);
                blobs.put(id, new Object[] {blob, size, writable});
                Map<String, Object> result = new HashMap<>();
                result.put("id", id);
                result.put("size", (long) size);
                return result;
            }
        }

        private Object[] blob(long id, String offset, long length) {
            Object[] blob = blobs.get(id);
            if (blob == null) {
                throw raise("data", "SQLITE_BLOB_STALE");
            }
            int size = (Integer) blob[1];
            if (offset == null || !offset.matches("[0-9]+") || offset.length() > 10 || length < 0 || length > 65536
                    || Long.parseLong(offset) > size || length > size - Long.parseLong(offset)) {
                throw raise("data", "SQLITE_BLOB_BOUNDS");
            }
            return blob;
        }

        Map<String, Object> blobRead(long id, String offset, long length) {
            if (length <= 0) {
                throw raise("data", "SQLITE_BLOB_BOUNDS");
            }
            Object[] blob = blob(id, offset, length);
            try (Arena local = Arena.ofConfined()) {
                MemorySegment buffer = local.allocate(length);
                check(call(Api.blobRead, blob[0], buffer, (int) length, Integer.parseInt(offset)));
                Map<String, Object> result = new HashMap<>();
                result.put("bytes", buffer.toArray(ValueLayout.JAVA_BYTE));
                return result;
            }
        }

        Map<String, Object> blobWrite(long id, String offset, byte[] bytes) {
            Object[] blob = blob(id, offset, bytes.length);
            if (!(Boolean) blob[2]) {
                throw raise("permission", "SQLITE_BLOB_READ_ONLY");
            }
            try (Arena local = Arena.ofConfined()) {
                MemorySegment buffer = local.allocate(Math.max(1, bytes.length));
                MemorySegment.copy(bytes, 0, buffer, ValueLayout.JAVA_BYTE, 0, bytes.length);
                check(call(Api.blobWrite, blob[0], buffer, bytes.length, Integer.parseInt(offset)));
            }
            return new HashMap<>();
        }

        Map<String, Object> blobClose(long id) {
            Object[] blob = blobs.remove(id);
            if (blob != null) {
                check(call(Api.blobClose, blob[0]));
            }
            Map<String, Object> result = new HashMap<>();
            result.put("autocommit", autocommit());
            return result;
        }

        Map<String, Object> backupOpen(String path) {
            ready();
            if (backup != null) {
                throw raise("busy", "SQLITE_BACKUP_ACTIVE");
            }
            if (!autocommit()) {
                throw raise("busy", "SQLITE_BACKUP_TRANSACTION");
            }
            if (path == null || path.isEmpty() || path.getBytes(StandardCharsets.UTF_8).length > 4096
                    || path.indexOf('\0') >= 0 || path.equals(":memory:")) {
                throw raise("data", "SQLITE_BACKUP_PATH");
            }
            if (path.equals(connection.path) || sameFile(connection.path, path)) {
                throw raise("data", "SQLITE_BACKUP_SAME_FILE");
            }
            MemorySegment slot = arena.allocate(Api.PTR);
            int rc = call(Api.open, arena.allocateFrom(Host.path(path).toString()), slot, 6, MemorySegment.NULL);
            MemorySegment target = slot.get(Api.PTR, 0);
            if (rc != 0) {
                if (!target.equals(MemorySegment.NULL)) {
                    call(Api.close, target);
                }
                throw error(rc);
            }
            MemorySegment main = arena.allocateFrom("main");
            MemorySegment pointer = (MemorySegment) invoke(Api.backupInit, target, main, db, main);
            if (pointer.equals(MemorySegment.NULL)) {
                int code = call(Api.extendedErrcode, target);
                call(Api.close, target);
                throw error(code);
            }
            long id = ++serial;
            backup = new Object[] {pointer, target, id};
            Map<String, Object> result = new HashMap<>();
            result.put("id", id);
            return result;
        }

        private boolean sameFile(String left, String right) {
            try {
                Path a = Host.path(left);
                Path b = Host.path(right);
                if (!Files.exists(a) || !Files.exists(b)) {
                    return false;
                }
                Object ka = Files.readAttributes(a, BasicFileAttributes.class).fileKey();
                Object kb = Files.readAttributes(b, BasicFileAttributes.class).fileKey();
                return ka != null && ka.equals(kb);
            } catch (java.io.IOException error) {
                return false;
            }
        }

        Map<String, Object> backupStep(long id, long pages) {
            if (backup == null || (Long) backup[2] != id) {
                throw raise("data", "SQLITE_BACKUP_STALE");
            }
            if (pages < 1 || pages > 256) {
                throw raise("data", "SQLITE_BACKUP_PAGES");
            }
            if (cancelled) {
                throw raise("cancelled", "SQLITE_9");
            }
            int rc = call(Api.backupStep, backup[0], (int) pages);
            if (!(rc == 0 || rc == 101 || rc == 5 || rc == 6)) {
                throw error(rc);
            }
            int total = call(Api.backupPagecount, backup[0]);
            int remaining = call(Api.backupRemaining, backup[0]);
            Map<String, Object> result = new HashMap<>();
            result.put("done", rc == 101);
            result.put("busy", rc == 5 || rc == 6);
            result.put("total", (long) total);
            result.put("copies", (long) (total - remaining));
            return result;
        }

        Map<String, Object> backupClose(long id) {
            if (backup == null || (Long) backup[2] != id) {
                return new HashMap<>();
            }
            Object[] current = backup;
            backup = null;
            int rc = call(Api.backupFinish, current[0]);
            int closed = call(Api.close, current[1]);
            check(rc);
            check(closed);
            return new HashMap<>();
        }

        Map<String, Object> script(String sql) {
            ready();
            byte[] bytes = sql.getBytes(StandardCharsets.UTF_8);
            if (bytes.length > 1048576 || sql.indexOf('\0') >= 0) {
                throw raise("data", "SQLITE_SCRIPT_LIMIT");
            }
            List<Map<String, Object>> results = new ArrayList<>();
            try (Arena local = Arena.ofConfined()) {
                MemorySegment text = local.allocate(bytes.length + 1L);
                MemorySegment.copy(bytes, 0, text, ValueLayout.JAVA_BYTE, 0, bytes.length);
                MemorySegment statementSlot = local.allocate(Api.PTR);
                MemorySegment tailSlot = local.allocate(Api.PTR);
                long offset = 0;
                while (offset < bytes.length) {
                    if (cancelled) {
                        throw raise("cancelled", "SQLITE_9");
                    }
                    dmlSeen = false;
                    ddlSeen = false;
                    int rc = call(Api.prepare, db, text.asSlice(offset), (int) (bytes.length - offset), 0,
                        statementSlot, tailSlot);
                    MemorySegment statement = statementSlot.get(Api.PTR, 0);
                    if (rc != 0) {
                        if (!statement.equals(MemorySegment.NULL)) {
                            call(Api.finalize, statement);
                        }
                        throw error(rc);
                    }
                    long next = tailSlot.get(Api.PTR, 0).address() - text.address();
                    if (next <= offset || next > bytes.length) {
                        if (!statement.equals(MemorySegment.NULL)) {
                            call(Api.finalize, statement);
                        }
                        throw raise("unexpected", "SQLITE_PREPARE_TAIL");
                    }
                    offset = next;
                    if (statement.equals(MemorySegment.NULL)) {
                        continue;
                    }
                    if (results.size() >= 256) {
                        call(Api.finalize, statement);
                        throw raise("resource", "SQLITE_SCRIPT_COUNT");
                    }
                    Statement prepared = new Statement();
                    prepared.pointer = statement;
                    prepared.dml = dmlSeen && !ddlSeen;
                    prepared.columns = call(Api.columnCount, statement);
                    results.add(execute(prepared, new ArrayList<>()));
                }
            }
            Map<String, Object> result = new HashMap<>();
            result.put("executions", results);
            result.put("autocommit", autocommit());
            return result;
        }

        Map<String, Object> rollback() {
            for (Long id : new ArrayList<>(blobs.keySet())) {
                blobClose(id);
            }
            if (cursor != null) {
                release(cursor.id);
            }
            if (autocommit()) {
                Map<String, Object> result = new HashMap<>();
                result.put("autocommit", true);
                return result;
            }
            return execution("ROLLBACK", new ArrayList<>(), true);
        }

        void close() {
            if (db.equals(MemorySegment.NULL)) {
                return;
            }
            if (backup != null) {
                backupClose((Long) backup[2]);
            }
            for (Long id : new ArrayList<>(blobs.keySet())) {
                blobClose(id);
            }
            if (cursor != null) {
                release(cursor.id);
            }
            for (Long id : new ArrayList<>(prepared.keySet())) {
                unprepare(id);
            }
            MemorySegment current = db;
            db = MemorySegment.NULL;
            check(call(Api.close, current));
        }
    }

    // ---- соединение и ответы ----

    static final class Connection {
        String path;
        boolean readOnly;
        boolean create;
        boolean foreignKeys;
        int busyMs;
        String journal;
        boolean stopped;
        boolean broken;
        boolean autocommit = true;
        boolean handleClosed;
        ExecutorService worker;
        Engine engine;
    }

    static final class Reply {
        boolean ok;
        Map<String, Object> data;
        boolean closed;
    }

    private static Connection connection(Object handle, String name, int arity) {
        Connection connection = (Connection) handle;
        if (connection.handleClosed) {
            throw fail("already_closed", name, arity, "Native handle is already closed");
        }
        return connection;
    }

    private static Reply reply(Object handle, String name, int arity) {
        Reply reply = (Reply) handle;
        if (reply.closed) {
            throw fail("already_closed", name, arity, "Native handle is already closed");
        }
        return reply;
    }

    private static String utf8(List<?> value) {
        return strictText(Natives.toBytes(value));
    }

    /** Операция движка на потоке соединения; ответ — Reply. */
    private interface Operation {
        Map<String, Object> run(Engine engine);
    }

    private static Reply perform(Engine engine, Operation operation) {
        Reply reply = new Reply();
        try {
            engine.cancelled = false;
            reply.data = operation.run(engine);
            reply.ok = true;
        } catch (SqliteFailure failure) {
            reply.ok = false;
            reply.data = new HashMap<>();
            reply.data.put("kind", failure.kind);
            reply.data.put("code", failure.code);
            reply.data.put("autocommit", failure.autocommit);
        } catch (RuntimeException | java.lang.Error unexpected) {
            if (TRACE) {
                unexpected.printStackTrace();
            }
            reply.ok = false;
            reply.data = new HashMap<>();
            reply.data.put("kind", "unexpected");
            reply.data.put("code", "SQLITE_WORKER_FAILURE");
        }
        return reply;
    }

    private static void facts(Connection connection, Reply reply) {
        Object autocommit = reply.data.get("autocommit");
        if (autocommit instanceof Boolean) {
            connection.autocommit = (Boolean) autocommit;
        }
    }

    /** {@code _request}: очередная операция; Future с Reply либо отказ. */
    private static Future request(final Connection connection, final String op, final Operation operation,
            String name, int arity) {
        if (connection.stopped || connection.broken) {
            return Scheduler.resolved(null, fail("already_closed", name, arity, "SQLite connection is closed"));
        }
        if (connection.worker == null) {
            return Scheduler.resolved(null, fail("invalid_input", name, arity, "SQLite connection is not acquired"));
        }
        final Future promise = Future.promise();
        final Engine engine = connection.engine;
        final Scheduler.Op pending;
        try {
            pending = Scheduler.offload(connection.worker, new Callable<Object>() {
                public Object call() {
                    return perform(engine, operation);
                }
            }, new Scheduler.Outcome() {
                public void deliver(Object value, Throwable error) {
                    Reply reply = (Reply) value;
                    if (reply == null) {
                        connection.broken = true;
                        Scheduler.settle(promise, null, fail("connection_reset", "sqlite_connect", 1,
                            "SQLite worker lost"));
                        return;
                    }
                    facts(connection, reply);
                    if (promise.state != Future.RUNNING && reply.ok) {
                        cleanupCancelled(connection, op, reply);
                        return;
                    }
                    Scheduler.settle(promise, reply, null);
                }
            });
        } catch (RejectedExecutionException closed) {
            return Scheduler.resolved(null, fail("already_closed", name, arity, "SQLite connection is closed"));
        }
        promise.listen(new Runnable() {
            public void run() {
                pending.detach();
                if (Future.cancelled(promise) && !engine.db.equals(MemorySegment.NULL)) {
                    engine.cancelled = true;
                    invoke(Api.interrupt, engine.db);
                }
            }
        });
        return promise;
    }

    /** Отменённая операция всё же удалась: освободить её ресурс (как Perl). */
    private static void cleanupCancelled(Connection connection, String op, Reply reply) {
        final Object id = reply.data.get("id");
        if (id == null) {
            return;
        }
        final long value = (Long) id;
        switch (op) {
            case "rows":
            case "prepared_rows":
                maintenance(connection, engine -> {
                    engine.release(value);
                    return new HashMap<>();
                });
                break;
            case "prepare":
                maintenance(connection, engine -> engine.unprepare(value));
                break;
            case "blob_open":
                maintenance(connection, engine -> engine.blobClose(value));
                break;
            case "backup_open":
                maintenance(connection, engine -> engine.backupClose(value));
                break;
            default:
                break;
        }
    }

    /** {@code _maintenance}: служебная операция в очереди; ответ не ждём. */
    private static boolean maintenance(final Connection connection, final Operation operation) {
        if (connection.stopped || connection.worker == null) {
            return true;
        }
        final Engine engine = connection.engine;
        try {
            final Scheduler.Op op = Scheduler.offload(connection.worker, new Callable<Object>() {
                public Object call() {
                    return perform(engine, operation);
                }
            }, new Scheduler.Outcome() {
                public void deliver(Object value, Throwable error) {
                    Reply reply = (Reply) value;
                    if (reply == null || !reply.ok) {
                        if (TRACE) {
                            System.err.println("okkama trace: SQLite maintenance failed: "
                                + (reply == null ? error : reply.data));
                        }
                        connection.broken = true;
                        return;
                    }
                    facts(connection, reply);
                }
            });
            // Служебная операция не держит программу.
            op.detach();
        } catch (RejectedExecutionException closed) {
            connection.broken = true;
        }
        return true;
    }

    // ---- natives ----

    public static Object runtime_database__sqlite_create(List<?> path, boolean readOnly, boolean create,
            boolean foreign, BigInteger busy, String journal, boolean memory) {
        byte[] bytes = Natives.toBytes(path);
        boolean nul = false;
        for (byte octet : bytes) {
            nul |= octet == 0;
        }
        if (bytes.length > 4096 || nul || busy.signum() < 0 || busy.compareTo(BigInteger.valueOf(60000)) > 0
                || (readOnly && create) || !journal.matches("delete|wal|memory")) {
            throw fail("invalid_input", "sqlite_create", 7, "Invalid SQLite options");
        }
        String text = strictText(bytes);
        if (text == null || (!memory && text.isEmpty())) {
            throw fail("invalid_input", "sqlite_create", 7, "SQLite path must be UTF8");
        }
        Connection connection = new Connection();
        connection.path = memory ? ":memory:" : Host.path(text).toString();
        connection.readOnly = readOnly;
        connection.create = create;
        connection.foreignKeys = foreign;
        connection.busyMs = busy.intValue();
        connection.journal = journal;
        return connection;
    }

    public static Future runtime_database__sqlite_connect(Object handle) {
        final Connection connection;
        try {
            connection = connection(handle, "sqlite_connect", 1);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
        if (connection.worker != null) {
            return Scheduler.resolved(null, fail("invalid_input", "sqlite_connect", 1,
                "SQLite connection already acquired"));
        }
        connection.worker = Executors.newSingleThreadExecutor(work -> {
            Thread thread = new Thread(work, "okkama-sqlite");
            thread.setDaemon(true);
            return thread;
        });
        connection.engine = new Engine(connection);
        return request(connection, "open", engine -> {
            engine.open();
            Map<String, Object> result = new HashMap<>();
            result.put("autocommit", true);
            return result;
        }, "sqlite_connect", 1);
    }

    private static final String[] KINDS = {"null_value", "boolean", "integer", "floating", "decimal", "text", "bytes"};

    private static List<Object[]> params(List<?> tags, List<?> payloads, String name, int arity) {
        if (tags.size() != payloads.size() || tags.size() > 1024) {
            throw fail("invalid_input", name, arity, "Invalid SQLite bindings");
        }
        List<Object[]> params = new ArrayList<>();
        long used = 0;
        for (int index = 0; index < tags.size(); index++) {
            BigInteger tag = (BigInteger) tags.get(index);
            if (tag.signum() < 0 || tag.compareTo(BigInteger.valueOf(6)) > 0) {
                throw fail("invalid_input", name, arity, "Invalid SQLite binding kind");
            }
            byte[] bytes = Natives.toBytes((List<?>) payloads.get(index));
            used += bytes.length;
            if (used > 8388608) {
                throw fail("resource_exhausted", name, arity, "SQLite binding byte budget");
            }
            Object value;
            if (tag.intValue() == 6) {
                value = bytes;
            } else {
                value = strictText(bytes);
                if (value == null) {
                    throw fail("invalid_input", name, arity, "Invalid SQLite binding encoding");
                }
            }
            params.add(new Object[] {KINDS[tag.intValue()], value});
        }
        return params;
    }

    private static Future statement(Object handle, List<?> sqlValue, List<?> tags, List<?> payloads, final boolean rows,
            String name) {
        try {
            Connection connection = connection(handle, name, 4);
            final String sql = utf8(sqlValue);
            if (sql == null || sql.isEmpty() || sql.indexOf('\0') >= 0
                    || sql.getBytes(StandardCharsets.UTF_8).length > 1048576) {
                throw fail("invalid_input", name, 4, "Invalid SQLite SQL");
            }
            final List<Object[]> params = params(tags, payloads, name, 4);
            return request(connection, rows ? "rows" : "execution",
                engine -> rows ? engine.rows(sql, params) : engine.execution(sql, params, false), name, 4);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    public static Future runtime_database__sqlite_execution(Object handle, List<?> sql, List<?> tags, List<?> payloads) {
        return statement(handle, sql, tags, payloads, false, "sqlite_execution");
    }

    public static Future runtime_database__sqlite_rows(Object handle, List<?> sql, List<?> tags, List<?> payloads) {
        return statement(handle, sql, tags, payloads, true, "sqlite_rows");
    }

    public static Future runtime_database__sqlite_prepare(Object handle, List<?> sqlValue) {
        String name = "sqlite_prepare";
        try {
            Connection connection = connection(handle, name, 2);
            final String sql = utf8(sqlValue);
            if (sql == null || sql.getBytes(StandardCharsets.UTF_8).length > 1048576 || sql.indexOf('\0') >= 0) {
                throw fail("invalid_input", name, 2, "Invalid SQLite SQL");
            }
            return request(connection, "prepare", engine -> engine.prepareStatement(sql), name, 2);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    private static Future preparedStatement(Object handle, BigInteger id, List<?> tags, List<?> payloads,
            final boolean rows, String name) {
        try {
            Connection connection = connection(handle, name, 4);
            if (id.signum() < 1 || tags.size() != payloads.size() || tags.size() > 1024) {
                throw fail("invalid_input", name, 4, "Invalid SQLite bindings");
            }
            final List<Object[]> params = params(tags, payloads, name, 4);
            final long statementId = id.longValue();
            return request(connection, rows ? "prepared_rows" : "prepared_execution",
                engine -> rows ? engine.rows(engine.preparedStatement(statementId), params)
                    : engine.execute(engine.preparedStatement(statementId), params), name, 4);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    public static Future runtime_database__sqlite_prepared_execution(Object handle, BigInteger id, List<?> tags,
            List<?> payloads) {
        return preparedStatement(handle, id, tags, payloads, false, "sqlite_prepared_execution");
    }

    public static Future runtime_database__sqlite_prepared_rows(Object handle, BigInteger id, List<?> tags,
            List<?> payloads) {
        return preparedStatement(handle, id, tags, payloads, true, "sqlite_prepared_rows");
    }

    public static boolean runtime_database__sqlite_unprepare(Object handle, BigInteger id) {
        final long value = id.longValue();
        return maintenance(connection(handle, "sqlite_unprepare", 2), engine -> engine.unprepare(value));
    }

    private static String text(List<?> value, String key, String name, int arity) {
        byte[] bytes = Natives.toBytes(value);
        String text = strictText(bytes);
        if (text == null || text.indexOf('\0') >= 0 || bytes.length > (key.equals("path") ? 4096 : 1048576)) {
            throw fail("invalid_input", name, arity, "Invalid SQLite text");
        }
        if (key.equals("path")) {
            text = Host.path(text).toString();
        }
        return text;
    }

    public static Future runtime_database__sqlite_blob_open(Object handle, String database, String table,
            String column, String row, boolean writable) {
        try {
            Connection connection = connection(handle, "sqlite_blob_open", 6);
            return request(connection, "blob_open", engine -> engine.blobOpen(database, table, column, row, writable),
                "sqlite_blob_open", 6);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    public static Future runtime_database__sqlite_blob_read(Object handle, BigInteger id, String offset,
            BigInteger length) {
        try {
            Connection connection = connection(handle, "sqlite_blob_read", 4);
            long blob = id.longValue();
            long size = length.bitLength() > 31 ? -1 : length.longValue();
            return request(connection, "blob_read", engine -> engine.blobRead(blob, offset, size),
                "sqlite_blob_read", 4);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    public static Future runtime_database__sqlite_blob_write(Object handle, BigInteger id, String offset,
            List<?> bytesValue) {
        try {
            Connection connection = connection(handle, "sqlite_blob_write", 4);
            byte[] bytes = Natives.toBytes(bytesValue);
            if (bytes.length > 65536) {
                throw fail("resource_exhausted", "sqlite_blob_write", 4, "SQLite blob byte budget");
            }
            long blob = id.longValue();
            return request(connection, "blob_write", engine -> engine.blobWrite(blob, offset, bytes),
                "sqlite_blob_write", 4);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    public static Future runtime_database__sqlite_backup_open(Object handle, List<?> pathValue) {
        try {
            Connection connection = connection(handle, "sqlite_backup_open", 2);
            String path = text(pathValue, "path", "sqlite_backup_open", 2);
            return request(connection, "backup_open", engine -> engine.backupOpen(path), "sqlite_backup_open", 2);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    public static Future runtime_database__sqlite_backup_step(Object handle, BigInteger id, BigInteger pages) {
        try {
            Connection connection = connection(handle, "sqlite_backup_step", 3);
            long backup = id.longValue();
            long count = pages.bitLength() > 31 ? -1 : pages.longValue();
            return request(connection, "backup_step", engine -> engine.backupStep(backup, count),
                "sqlite_backup_step", 3);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    public static Future runtime_database__sqlite_script(Object handle, List<?> sqlValue) {
        try {
            Connection connection = connection(handle, "sqlite_script", 2);
            String sql = text(sqlValue, "sql", "sqlite_script", 2);
            return request(connection, "script", engine -> engine.script(sql), "sqlite_script", 2);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    public static Future runtime_database__sqlite_control(Object handle, List<?> sqlValue) {
        try {
            Connection connection = connection(handle, "sqlite_control", 2);
            String sql = text(sqlValue, "sql", "sqlite_control", 2);
            return request(connection, "control", engine -> {
                if (!sql.matches("BEGIN (?:DEFERRED|IMMEDIATE|EXCLUSIVE)|COMMIT|ROLLBACK")) {
                    throw new IllegalStateException("worker control SQL");
                }
                return engine.execution(sql, new ArrayList<>(), true);
            }, "sqlite_control", 2);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    public static boolean runtime_database__sqlite_blob_close(Object handle, BigInteger id) {
        long blob = id.longValue();
        return maintenance(connection(handle, "sqlite_blob_close", 2), engine -> engine.blobClose(blob));
    }

    public static boolean runtime_database__sqlite_backup_close(Object handle, BigInteger id) {
        long backup = id.longValue();
        return maintenance(connection(handle, "sqlite_backup_close", 2), engine -> engine.backupClose(backup));
    }

    public static List<Object> runtime_database__sqlite_reply_blob_bytes(Object handle) {
        Reply reply = reply(handle, "sqlite_reply_blob_bytes", 1);
        byte[] bytes = reply.ok && reply.data.get("bytes") != null ? (byte[]) reply.data.get("bytes") : new byte[0];
        return Natives.fromBytes(bytes, bytes.length);
    }

    @SuppressWarnings("unchecked")
    public static List<Object> runtime_database__sqlite_reply_executions(Object handle) {
        Reply reply = reply(handle, "sqlite_reply_executions", 1);
        List<Map<String, Object>> entries = reply.ok && reply.data.get("executions") != null
            ? (List<Map<String, Object>>) reply.data.get("executions") : new ArrayList<>();
        if (entries.size() > 256) {
            throw fail("invalid_data", "sqlite_reply_executions", 1, "SQLite script reply limit");
        }
        List<Object> out = new ArrayList<>();
        for (Map<String, Object> entry : entries) {
            Object affected = entry.get("affected");
            out.add(affected == null ? "unknown" : affected);
        }
        return out;
    }

    public static Future runtime_database__sqlite_fetch(Object handle, BigInteger id, BigInteger maximum) {
        try {
            Connection connection = connection(handle, "sqlite_fetch", 3);
            if (id.signum() < 1 || maximum.signum() < 1 || maximum.compareTo(BigInteger.valueOf(64)) > 0) {
                throw fail("invalid_input", "sqlite_fetch", 3, "Invalid SQLite fetch coordinates");
            }
            long cursor = id.longValue();
            int count = maximum.intValue();
            return request(connection, "fetch", engine -> engine.fetch(cursor, count), "sqlite_fetch", 3);
        } catch (Failure failure) {
            return Scheduler.resolved(null, failure);
        }
    }

    public static boolean runtime_database__sqlite_release(Object handle, BigInteger id) {
        long cursor = id.longValue();
        return maintenance(connection(handle, "sqlite_release", 2), engine -> {
            engine.release(cursor);
            return new HashMap<>();
        });
    }

    public static boolean runtime_database__sqlite_rollback(Object handle) {
        return maintenance(connection(handle, "sqlite_rollback", 1), Engine::rollback);
    }

    public static boolean runtime_database__sqlite_close(Object handle) {
        Connection connection = (Connection) handle;
        if (connection.handleClosed || connection.stopped) {
            return true;
        }
        connection.stopped = true;
        if (connection.worker != null) {
            final Engine engine = connection.engine;
            try {
                connection.worker.execute(() -> {
                    try {
                        engine.close();
                    } catch (RuntimeException ignored) {
                        // Закрытие при завершении: отказ уже не наблюдаем.
                    }
                });
            } catch (RejectedExecutionException closed) {
                // уже закрыт
            }
            connection.worker.shutdown();
        }
        return true;
    }

    public static boolean runtime_database__sqlite_closed(Object handle) {
        Connection connection = (Connection) handle;
        return connection.handleClosed || connection.stopped || connection.broken;
    }

    public static Future runtime_database__sqlite_settled(Object handle) {
        final Connection connection = (Connection) handle;
        if (connection.handleClosed) {
            return Scheduler.resolved(Boolean.TRUE, null);
        }
        if (connection.worker == null) {
            if (connection.stopped) {
                connection.handleClosed = true;
            }
            return Scheduler.resolved(Boolean.TRUE, null);
        }
        final Future promise = Future.promise();
        final ExecutorService worker = connection.worker;
        Scheduler.offload(new Callable<Object>() {
            public Object call() throws Exception {
                if (worker.isShutdown()) {
                    worker.awaitTermination(2, java.util.concurrent.TimeUnit.SECONDS);
                    return Boolean.TRUE;
                }
                worker.submit(() -> Boolean.TRUE).get();
                return Boolean.TRUE;
            }
        }, new Scheduler.Outcome() {
            public void deliver(Object value, Throwable error) {
                if (connection.stopped) {
                    connection.handleClosed = true;
                }
                Scheduler.settle(promise, Boolean.TRUE, null);
            }
        });
        return promise;
    }

    public static boolean runtime_database__sqlite_autocommit(Object handle) {
        return connection(handle, "sqlite_autocommit", 1).autocommit;
    }

    @SuppressWarnings("unchecked")
    public static Map<Object, Object> runtime_database__sqlite_reply_info(Object handle) {
        Reply reply = reply(handle, "sqlite_reply_info", 1);
        Map<String, Object> data = reply.data;
        Map<Object, Object> out = Rt.newMap();
        out.put("ok", reply.ok ? "1" : "0");
        out.put("code", text(data.get("code"), ""));
        out.put("kind", text(data.get("kind"), ""));
        out.put("affected_known", data.get("affected") != null ? "1" : "0");
        out.put("affected", text(data.get("affected"), "0"));
        out.put("id", text(data.get("id"), "0"));
        out.put("eof", Boolean.TRUE.equals(data.get("eof")) ? "1" : "0");
        out.put("rows", data.containsKey("rows") ? Integer.toString(((List<Object>) data.get("rows")).size()) : "0");
        out.put("autocommit", data.containsKey("autocommit") ? (Boolean.TRUE.equals(data.get("autocommit")) ? "1" : "0") : "1");
        out.put("size", text(data.get("size"), "0"));
        out.put("done", Boolean.TRUE.equals(data.get("done")) ? "1" : "0");
        out.put("busy", Boolean.TRUE.equals(data.get("busy")) ? "1" : "0");
        out.put("copies", text(data.get("copies"), "0"));
        out.put("total", text(data.get("total"), "0"));
        return out;
    }

    private static String text(Object value, String missing) {
        return value == null ? missing : String.valueOf(value);
    }

    @SuppressWarnings("unchecked")
    private static List<Object> strings(Object handle, String key, String name) {
        Reply reply = reply(handle, name, 1);
        Object values = reply.ok ? reply.data.get(key) : null;
        return values == null ? new ArrayList<>() : new ArrayList<>((List<Object>) values);
    }

    public static List<Object> runtime_database__sqlite_reply_names(Object handle) {
        return strings(handle, "names", "sqlite_reply_names");
    }

    public static List<Object> runtime_database__sqlite_reply_declarations(Object handle) {
        return strings(handle, "declarations", "sqlite_reply_declarations");
    }

    @SuppressWarnings("unchecked")
    private static Object[] cell(Object handle, BigInteger row, BigInteger column, String name) {
        Reply reply = reply(handle, name, 3);
        List<List<Object[]>> rows = reply.ok && reply.data.get("rows") != null
            ? (List<List<Object[]>>) reply.data.get("rows") : new ArrayList<>();
        if (row.signum() < 0 || row.compareTo(BigInteger.valueOf(rows.size())) >= 0 || column.signum() < 0
                || column.compareTo(BigInteger.valueOf(rows.get(row.intValue()).size())) >= 0) {
            throw fail("invalid_input", name, 3, "SQLite reply cell outside bounds");
        }
        return rows.get(row.intValue()).get(column.intValue());
    }

    public static BigInteger runtime_database__sqlite_reply_kind(Object handle, BigInteger row, BigInteger column) {
        String kind = (String) cell(handle, row, column, "sqlite_reply_kind")[0];
        int code = kind.equals("null_value") ? 0 : kind.equals("integer") ? 1 : kind.equals("floating") ? 2
            : kind.equals("text") ? 3 : 4;
        return BigInteger.valueOf(code);
    }

    public static String runtime_database__sqlite_reply_text(Object handle, BigInteger row, BigInteger column) {
        Object[] value = cell(handle, row, column, "sqlite_reply_text");
        if (value[0].equals("bytes")) {
            throw fail("invalid_input", "sqlite_reply_text", 3, "SQLite cell is binary");
        }
        return (String) value[1];
    }

    public static List<Object> runtime_database__sqlite_reply_bytes(Object handle, BigInteger row, BigInteger column) {
        Object[] value = cell(handle, row, column, "sqlite_reply_bytes");
        if (!value[0].equals("bytes")) {
            throw fail("invalid_input", "sqlite_reply_bytes", 3, "SQLite cell is not binary");
        }
        byte[] bytes = (byte[]) value[1];
        return Natives.fromBytes(bytes, bytes.length);
    }

    public static boolean runtime_database__sqlite_reply_close(Object handle) {
        reply(handle, "sqlite_reply_close", 1).closed = true;
        return true;
    }
}
