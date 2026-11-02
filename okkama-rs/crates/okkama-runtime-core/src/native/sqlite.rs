//! `runtime/database` (provider `optional.sqlite`) — перенос `SqliteNatives`
//! JVM: системная libsqlite3 через dlopen (как FFM JVM). Каждое соединение
//! обслуживает свой рабочий поток; операции идут по очереди, их исход
//! приходит событием цикла. Ответ — `ok` и данные либо отказ `kind`/`code`.
#![allow(unsafe_code)]

use super::support::{byte_array, bytes, handle, int, items, java_long, map, new_handle, string, string_array, text};
use super::Error;
use crate::failure::{Failure, Kind};
use crate::sched;
use crate::{IntValue, Value, abi::Native};
use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};

const DOMAIN: &str = "optional.sqlite";

fn fail(code: &str, name: &str, arity: usize, message: &str) -> Rc<Failure> {
    Failure::new(Kind::Native(Rc::new(crate::abi::NativeFailure {
        domain: DOMAIN.into(),
        code: code.into(),
        operation: format!("runtime/database@1::{name}/{arity}"),
        message: message.into(),
    })))
}

fn as_error(failure: Rc<Failure>) -> Error {
    match &failure.kind {
        Kind::Native(native) => Error::Failure((**native).clone()),
        _ => Error::Trap(failure.message().to_string()),
    }
}

fn resolved(outcome: sched::Outcome) -> Value {
    Value::Future(sched::resolved(outcome))
}

// ---- libsqlite3 ----

type Db = *mut c_void;
type Stmt = *mut c_void;
type Authorizer = extern "C" fn(*mut c_void, c_int, *const c_char, *const c_char, *const c_char, *const c_char) -> c_int;

#[allow(clippy::type_complexity)]
struct Api {
    libversion: unsafe extern "C" fn() -> c_int,
    open: unsafe extern "C" fn(*const c_char, *mut Db, c_int, *const c_char) -> c_int,
    close: unsafe extern "C" fn(Db) -> c_int,
    extended_codes: unsafe extern "C" fn(Db, c_int) -> c_int,
    extended_errcode: unsafe extern "C" fn(Db) -> c_int,
    autocommit: unsafe extern "C" fn(Db) -> c_int,
    prepare: unsafe extern "C" fn(Db, *const c_char, c_int, u32, *mut Stmt, *mut *const c_char) -> c_int,
    step: unsafe extern "C" fn(Stmt) -> c_int,
    finalize: unsafe extern "C" fn(Stmt) -> c_int,
    reset: unsafe extern "C" fn(Stmt) -> c_int,
    clear_bindings: unsafe extern "C" fn(Stmt) -> c_int,
    parameter_count: unsafe extern "C" fn(Stmt) -> c_int,
    bind_null: unsafe extern "C" fn(Stmt, c_int) -> c_int,
    bind_int64: unsafe extern "C" fn(Stmt, c_int, i64) -> c_int,
    bind_double: unsafe extern "C" fn(Stmt, c_int, f64) -> c_int,
    bind_text: unsafe extern "C" fn(Stmt, c_int, *const c_char, c_int, isize) -> c_int,
    bind_blob: unsafe extern "C" fn(Stmt, c_int, *const c_void, c_int, isize) -> c_int,
    column_count: unsafe extern "C" fn(Stmt) -> c_int,
    column_name: unsafe extern "C" fn(Stmt, c_int) -> *const c_char,
    column_decltype: unsafe extern "C" fn(Stmt, c_int) -> *const c_char,
    column_type: unsafe extern "C" fn(Stmt, c_int) -> c_int,
    column_int64: unsafe extern "C" fn(Stmt, c_int) -> i64,
    column_double: unsafe extern "C" fn(Stmt, c_int) -> f64,
    column_text: unsafe extern "C" fn(Stmt, c_int) -> *const u8,
    column_blob: unsafe extern "C" fn(Stmt, c_int) -> *const c_void,
    column_bytes: unsafe extern "C" fn(Stmt, c_int) -> c_int,
    changes: unsafe extern "C" fn(Db) -> i64,
    authorizer: unsafe extern "C" fn(Db, Option<Authorizer>, *mut c_void) -> c_int,
    interrupt: unsafe extern "C" fn(Db),
    compile_option: unsafe extern "C" fn(*const c_char) -> c_int,
    limit: unsafe extern "C" fn(Db, c_int, c_int) -> c_int,
    busy_timeout: unsafe extern "C" fn(Db, c_int) -> c_int,
    load_extension: Option<unsafe extern "C" fn(Db, c_int) -> c_int>,
    blob_open: unsafe extern "C" fn(Db, *const c_char, *const c_char, *const c_char, i64, c_int, *mut *mut c_void) -> c_int,
    blob_bytes: unsafe extern "C" fn(*mut c_void) -> c_int,
    blob_close: unsafe extern "C" fn(*mut c_void) -> c_int,
    blob_read: unsafe extern "C" fn(*mut c_void, *mut c_void, c_int, c_int) -> c_int,
    blob_write: unsafe extern "C" fn(*mut c_void, *const c_void, c_int, c_int) -> c_int,
    backup_init: unsafe extern "C" fn(Db, *const c_char, Db, *const c_char) -> *mut c_void,
    backup_step: unsafe extern "C" fn(*mut c_void, c_int) -> c_int,
    backup_finish: unsafe extern "C" fn(*mut c_void) -> c_int,
    backup_remaining: unsafe extern "C" fn(*mut c_void) -> c_int,
    backup_pagecount: unsafe extern "C" fn(*mut c_void) -> c_int,
}

/// Библиотека: найдена ли она и все ли нужные функции экспортирует.
enum Loaded {
    Missing,
    Incomplete,
    Ready(Api),
}

fn api() -> &'static Loaded {
    static API: OnceLock<Loaded> = OnceLock::new();
    API.get_or_init(|| {
        let candidates = ["/usr/lib/libsqlite3.dylib", "libsqlite3.so.0", "libsqlite3.so", "libsqlite3.dylib"];
        let library = candidates.iter().find_map(|candidate| {
            let name = CString::new(*candidate).ok()?;
            // SAFETY: имя с завершающим нулём.
            let handle = unsafe { libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
            (!handle.is_null()).then_some(handle)
        });
        let Some(library) = library else {
            return Loaded::Missing;
        };
        let find = |name: &str| -> Option<*mut c_void> {
            let name = CString::new(name).ok()?;
            // SAFETY: дескриптор действителен до конца процесса.
            let address = unsafe { libc::dlsym(library, name.as_ptr()) };
            (!address.is_null()).then_some(address)
        };
        macro_rules! function {
            ($name:literal) => {
                match find($name) {
                    // SAFETY: прототип — из sqlite3.h.
                    Some(address) => unsafe { std::mem::transmute::<*mut c_void, _>(address) },
                    None => return Loaded::Incomplete,
                }
            };
        }
        let load_extension = find("sqlite3_enable_load_extension")
            // SAFETY: прототип — из sqlite3.h.
            .map(|address| unsafe { std::mem::transmute::<*mut c_void, unsafe extern "C" fn(Db, c_int) -> c_int>(address) });
        Loaded::Ready(Api {
            libversion: function!("sqlite3_libversion_number"),
            open: function!("sqlite3_open_v2"),
            close: function!("sqlite3_close"),
            extended_codes: function!("sqlite3_extended_result_codes"),
            extended_errcode: function!("sqlite3_extended_errcode"),
            autocommit: function!("sqlite3_get_autocommit"),
            prepare: function!("sqlite3_prepare_v3"),
            step: function!("sqlite3_step"),
            finalize: function!("sqlite3_finalize"),
            reset: function!("sqlite3_reset"),
            clear_bindings: function!("sqlite3_clear_bindings"),
            parameter_count: function!("sqlite3_bind_parameter_count"),
            bind_null: function!("sqlite3_bind_null"),
            bind_int64: function!("sqlite3_bind_int64"),
            bind_double: function!("sqlite3_bind_double"),
            bind_text: function!("sqlite3_bind_text"),
            bind_blob: function!("sqlite3_bind_blob"),
            column_count: function!("sqlite3_column_count"),
            column_name: function!("sqlite3_column_name"),
            column_decltype: function!("sqlite3_column_decltype"),
            column_type: function!("sqlite3_column_type"),
            column_int64: function!("sqlite3_column_int64"),
            column_double: function!("sqlite3_column_double"),
            column_text: function!("sqlite3_column_text"),
            column_blob: function!("sqlite3_column_blob"),
            column_bytes: function!("sqlite3_column_bytes"),
            changes: function!("sqlite3_changes64"),
            authorizer: function!("sqlite3_set_authorizer"),
            interrupt: function!("sqlite3_interrupt"),
            compile_option: function!("sqlite3_compileoption_used"),
            limit: function!("sqlite3_limit"),
            busy_timeout: function!("sqlite3_busy_timeout"),
            load_extension,
            blob_open: function!("sqlite3_blob_open"),
            blob_bytes: function!("sqlite3_blob_bytes"),
            blob_close: function!("sqlite3_blob_close"),
            blob_read: function!("sqlite3_blob_read"),
            blob_write: function!("sqlite3_blob_write"),
            backup_init: function!("sqlite3_backup_init"),
            backup_step: function!("sqlite3_backup_step"),
            backup_finish: function!("sqlite3_backup_finish"),
            backup_remaining: function!("sqlite3_backup_remaining"),
            backup_pagecount: function!("sqlite3_backup_pagecount"),
        })
    })
}

fn ready_api() -> &'static Api {
    match api() {
        Loaded::Ready(api) => api,
        _ => unreachable!("checked at open"),
    }
}

fn c_bytes(pointer: *const c_char) -> Option<Vec<u8>> {
    // SAFETY: SQLite возвращает строку с завершающим нулём или NULL.
    (!pointer.is_null()).then(|| unsafe { CStr::from_ptr(pointer) }.to_bytes().to_vec())
}

fn strict(bytes: &[u8]) -> Option<String> {
    String::from_utf8(bytes.to_vec()).ok()
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|b| char::from(*b)).collect()
}

// ---- данные ответа ----

#[derive(Debug, Clone)]
enum Cell {
    Null,
    Integer(String),
    Floating(String),
    Text(String),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone, Default)]
struct Data {
    kind: Option<String>,
    code: Option<String>,
    autocommit: Option<bool>,
    /// `Some(None)` — ключ есть, значение неизвестно (не DML).
    affected: Option<Option<String>>,
    id: Option<i64>,
    eof: bool,
    rows: Option<Vec<Vec<Cell>>>,
    names: Option<Vec<String>>,
    declarations: Option<Vec<String>>,
    size: Option<i64>,
    done: bool,
    busy: bool,
    copies: Option<i64>,
    total: Option<i64>,
    bytes: Option<Vec<u8>>,
    executions: Option<Vec<Option<String>>>,
}

#[derive(Debug)]
struct Reply {
    ok: bool,
    data: Data,
    closed: RefCell<bool>,
}

/// Отказ движка: вид, код SQLITE_… и признак autocommit.
#[derive(Debug)]
enum Stop {
    Sqlite { kind: &'static str, code: String, autocommit: bool },
    Unexpected,
}

type Done<T> = Result<T, Stop>;

// ---- движок ----

const SCHEMA_ACTION: [c_int; 21] = [1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 15, 16, 17, 26, 27, 28, 29, 30];

struct Statement {
    pointer: Stmt,
    dml: bool,
    columns: c_int,
    prepared: i64,
    id: i64,
    verified: bool,
    raw_names: Vec<String>,
    raw_declarations: Vec<String>,
    pending_row: Option<(Vec<Cell>, usize)>,
}

#[derive(Clone)]
struct Options {
    path: String,
    read_only: bool,
    create: bool,
    foreign_keys: bool,
    busy_ms: c_int,
    journal: String,
}

struct Engine {
    db: Db,
    shared_db: Arc<AtomicPtr<c_void>>,
    cancelled: Arc<AtomicBool>,
    options: Options,
    serial: i64,
    cursor: Option<Statement>,
    prepared: Vec<(i64, Statement)>,
    blobs: Vec<(i64, *mut c_void, c_int, bool)>,
    backup: Option<(*mut c_void, Db, i64)>,
    tail_guard: bool,
    control: bool,
    configuration: bool,
    dml_seen: bool,
    ddl_seen: bool,
}

extern "C" fn authorize(
    user: *mut c_void,
    action: c_int,
    a: *const c_char,
    _b: *const c_char,
    _database: *const c_char,
    trigger: *const c_char,
) -> c_int {
    // SAFETY: user — адрес Engine в куче потока соединения.
    let engine = unsafe { &mut *(user as *mut Engine) };
    if engine.tail_guard {
        return 1;
    }
    if (action == 22 || action == 32) && !engine.control {
        return 1;
    }
    if (action == 19 || action == 24 || action == 25) && !engine.configuration {
        return 1;
    }
    if SCHEMA_ACTION.contains(&action) {
        engine.ddl_seen = true;
    }
    if action == 9 || action == 18 || action == 23 {
        if let Some(table) = c_bytes(a) {
            if table != b"sqlite_master" && table != b"sqlite_schema" && trigger.is_null() {
                engine.dml_seen = true;
            }
        }
    }
    0
}

fn error_kind(rc: c_int) -> &'static str {
    match rc & 255 {
        5 | 6 => "busy",
        19 => "constraint",
        1 => "syntax",
        9 => "cancelled",
        7 | 18 => "resource",
        3 | 8 | 23 => "permission",
        14 => "connection",
        20 | 21 | 25 => "data",
        _ => "unexpected",
    }
}

impl Engine {
    fn autocommit(&self) -> bool {
        // SAFETY: открытое соединение этого потока.
        self.db.is_null() || unsafe { (ready_api().autocommit)(self.db) } != 0
    }

    fn raise(&self, kind: &'static str, code: &str) -> Stop {
        Stop::Sqlite {
            kind,
            code: code.into(),
            autocommit: self.autocommit(),
        }
    }

    fn error(&self, rc: c_int) -> Stop {
        self.raise(error_kind(rc), &format!("SQLITE_{rc}"))
    }

    fn check(&self, rc: c_int) -> Done<()> {
        if rc != 0 { Err(self.error(rc)) } else { Ok(()) }
    }

    fn open(&mut self) -> Done<()> {
        let api = match api() {
            Loaded::Ready(api) => api,
            Loaded::Missing => {
                return Err(Stop::Sqlite { kind: "unsupported", code: "SQLITE_UNAVAILABLE".into(), autocommit: true });
            }
            Loaded::Incomplete => {
                return Err(Stop::Sqlite { kind: "unsupported", code: "SQLITE_CAPABILITY".into(), autocommit: true });
            }
        };
        // SAFETY: вызовы C API SQLite с действительными аргументами.
        unsafe {
            if (api.libversion)() < 3_041_000 {
                return Err(Stop::Sqlite { kind: "unsupported", code: "SQLITE_VERSION".into(), autocommit: true });
            }
            let flags = if self.options.read_only { 1 } else { 2 | if self.options.create { 4 } else { 0 } };
            let path = CString::new(self.options.path.as_bytes()).map_err(|_| Stop::Unexpected)?;
            let mut db: Db = std::ptr::null_mut();
            let rc = (api.open)(path.as_ptr(), &mut db, flags, std::ptr::null());
            self.db = db;
            self.shared_db.store(db, Ordering::SeqCst);
            if rc != 0 {
                let _ = self.close();
                return Err(Stop::Sqlite { kind: "connection", code: format!("SQLITE_{rc}"), autocommit: true });
            }
            (api.extended_codes)(self.db, 1);
            if let Some(load_extension) = api.load_extension {
                let rc = load_extension(self.db, 0);
                self.check(rc)?;
            } else {
                let option = CString::new("OMIT_LOAD_EXTENSION").expect("no NUL");
                if (api.compile_option)(option.as_ptr()) == 0 {
                    let _ = self.close();
                    return Err(Stop::Sqlite { kind: "unsupported", code: "SQLITE_EXTENSION_POLICY".into(), autocommit: true });
                }
            }
            for (limit, value) in [(0, 8_388_608), (1, 1_048_576), (2, 1024), (9, 1024)] {
                (api.limit)(self.db, limit, value);
            }
            (api.busy_timeout)(self.db, self.options.busy_ms);
            (api.authorizer)(self.db, Some(authorize), self as *mut Engine as *mut c_void);
        }
        self.configuration = true;
        let configured = (|| -> Done<()> {
            let foreign = if self.options.foreign_keys { 1 } else { 0 };
            self.execution(&format!("PRAGMA foreign_keys={foreign}"), &[], false)?;
            let reply = self.rows_sql("PRAGMA foreign_keys", &[])?;
            let seen = self.fetch(reply.id.unwrap_or(0), 1)?;
            self.release(reply.id.unwrap_or(0))?;
            if first_cell(&seen) != if self.options.foreign_keys { "1" } else { "0" } {
                return Err(self.raise("unsupported", "SQLITE_FOREIGN_KEYS"));
            }
            let mode = self.options.journal.clone();
            let reply = self.rows_sql(&format!("PRAGMA journal_mode={mode}"), &[])?;
            let actual = self.fetch(reply.id.unwrap_or(0), 1)?;
            self.release(reply.id.unwrap_or(0))?;
            if first_cell(&actual) != mode {
                return Err(self.raise("unsupported", "SQLITE_JOURNAL"));
            }
            Ok(())
        })();
        self.configuration = false;
        if let Err(error) = configured {
            let _ = self.close();
            return Err(error);
        }
        Ok(())
    }

    fn ready(&self) -> Done<()> {
        if self.db.is_null() {
            return Err(self.raise("connection", "SQLITE_CLOSED"));
        }
        if self.cursor.is_some() {
            return Err(self.raise("busy", "SQLITE_READER_ACTIVE"));
        }
        Ok(())
    }

    fn prepare(&mut self, sql: &str) -> Done<Statement> {
        self.ready()?;
        let api = ready_api();
        let bytes = sql.as_bytes();
        if bytes.len() > 1_048_576 || sql.contains('\0') {
            return Err(self.raise("data", "SQLITE_SQL_LIMIT"));
        }
        let mut text = bytes.to_vec();
        text.push(0);
        let base = text.as_ptr() as *const c_char;
        let mut statement: Stmt = std::ptr::null_mut();
        let mut tail: *const c_char = std::ptr::null();
        self.dml_seen = false;
        self.ddl_seen = false;
        // SAFETY: текст живёт до конца функции.
        let rc = unsafe { (api.prepare)(self.db, base, bytes.len() as c_int, 0, &mut statement, &mut tail) };
        if rc != 0 {
            if !statement.is_null() {
                unsafe { (api.finalize)(statement) };
            }
            return Err(self.error(rc));
        }
        if statement.is_null() {
            return Err(self.raise("syntax", "SQLITE_EMPTY_STATEMENT"));
        }
        let dml = self.dml_seen && !self.ddl_seen;
        let offset = (tail as isize) - (base as isize);
        let checked = (|| -> Done<()> {
            if offset < 0 || offset as usize > bytes.len() {
                return Err(self.raise("unexpected", "SQLITE_PREPARE_TAIL"));
            }
            let offset = offset as usize;
            if offset < bytes.len() {
                self.tail_guard = true;
                let mut other: Stmt = std::ptr::null_mut();
                let mut rest: *const c_char = std::ptr::null();
                // SAFETY: хвост внутри того же текста.
                let extra = unsafe {
                    (api.prepare)(self.db, base.add(offset), (bytes.len() - offset) as c_int, 0, &mut other, &mut rest)
                };
                if !other.is_null() {
                    unsafe { (api.finalize)(other) };
                }
                self.tail_guard = false;
                if extra != 0 || !other.is_null() {
                    return Err(self.raise("syntax", "SQLITE_MULTIPLE_STATEMENTS"));
                }
            }
            Ok(())
        })();
        if let Err(error) = checked {
            unsafe { (api.finalize)(statement) };
            return Err(error);
        }
        Ok(Statement {
            pointer: statement,
            dml,
            columns: unsafe { (api.column_count)(statement) },
            prepared: 0,
            id: 0,
            verified: false,
            raw_names: Vec::new(),
            raw_declarations: Vec::new(),
            pending_row: None,
        })
    }

    fn bind(&self, statement: &Statement, params: &[(&'static str, Param)]) -> Done<()> {
        let api = ready_api();
        // SAFETY: подготовленный оператор этого соединения.
        if params.len() as c_int != unsafe { (api.parameter_count)(statement.pointer) } {
            return Err(self.raise("data", "SQLITE_BIND_COUNT"));
        }
        for (at, (kind, value)) in params.iter().enumerate() {
            let index = at as c_int + 1;
            let rc = match *kind {
                "null_value" => unsafe { (api.bind_null)(statement.pointer, index) },
                "integer" | "boolean" => {
                    let Param::Text(text) = value else { return Err(Stop::Unexpected) };
                    let number = self.int64(text)?;
                    unsafe { (api.bind_int64)(statement.pointer, index, number) }
                }
                "floating" => {
                    let Param::Text(text) = value else { return Err(Stop::Unexpected) };
                    let number = super::jre::parse_double(text.trim()).unwrap_or(0.0);
                    if number.is_nan() || number.is_infinite() {
                        return Err(self.raise("data", "SQLITE_FLOAT_RANGE"));
                    }
                    unsafe { (api.bind_double)(statement.pointer, index, number) }
                }
                "text" | "decimal" | "bytes" => {
                    let data: &[u8] = match value {
                        Param::Text(text) => text.as_bytes(),
                        Param::Bytes(bytes) => bytes,
                    };
                    if data.len() > 8_388_608 {
                        return Err(self.raise("resource", "SQLITE_VALUE_LIMIT"));
                    }
                    let buffer = if data.is_empty() { vec![0u8] } else { data.to_vec() };
                    // SQLITE_TRANSIENT (-1): SQLite копирует значение.
                    if *kind == "bytes" {
                        unsafe { (api.bind_blob)(statement.pointer, index, buffer.as_ptr() as *const c_void, data.len() as c_int, -1) }
                    } else {
                        unsafe { (api.bind_text)(statement.pointer, index, buffer.as_ptr() as *const c_char, data.len() as c_int, -1) }
                    }
                }
                _ => return Err(self.raise("data", "SQLITE_VALUE_KIND")),
            };
            self.check(rc)?;
        }
        Ok(())
    }

    fn execution(&mut self, sql: &str, params: &[(&'static str, Param)], control: bool) -> Done<Data> {
        let previous = self.control;
        self.control = control;
        let result = self.prepare(sql).and_then(|statement| self.execute(statement, params));
        self.control = previous;
        result
    }

    fn execute(&mut self, mut statement: Statement, params: &[(&'static str, Param)]) -> Done<Data> {
        let api = ready_api();
        let outcome = (|| -> Done<Data> {
            if statement.columns != 0 {
                return Err(self.raise("data", "SQLITE_ROWS_REQUIRED"));
            }
            self.bind(&statement, params)?;
            // SAFETY: подготовленный оператор этого соединения.
            let rc = unsafe { (api.step)(statement.pointer) };
            if rc != 101 {
                return Err(self.error(rc));
            }
            Ok(Data {
                affected: Some(statement.dml.then(|| unsafe { (api.changes)(self.db) }.to_string())),
                autocommit: Some(self.autocommit()),
                ..Data::default()
            })
        })();
        let finished = self.finish(&mut statement);
        let data = outcome?;
        finished?;
        if statement.prepared != 0 {
            self.restore(statement);
        }
        Ok(data)
    }

    /// Вернуть подготовленный оператор в таблицу после выполнения.
    fn restore(&mut self, statement: Statement) {
        if let Some(slot) = self.prepared.iter_mut().find(|(id, _)| *id == statement.prepared) {
            slot.1 = statement;
        }
    }

    fn rows_sql(&mut self, sql: &str, params: &[(&'static str, Param)]) -> Done<Data> {
        let statement = self.prepare(sql)?;
        self.rows(statement, params)
    }

    fn rows(&mut self, mut statement: Statement, params: &[(&'static str, Param)]) -> Done<Data> {
        let api = ready_api();
        let outcome = (|| -> Done<(Vec<String>, Vec<String>, Vec<String>, Vec<String>)> {
            // SAFETY: подготовленный оператор этого соединения.
            statement.columns = unsafe { (api.column_count)(statement.pointer) };
            if statement.columns == 0 {
                return Err(self.raise("data", "SQLITE_EXECUTION_REQUIRED"));
            }
            self.bind(&statement, params)?;
            let (mut names, mut declarations, mut raw_names, mut raw_declarations) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            let mut used = 0usize;
            for column in 0..statement.columns {
                let name = c_bytes(unsafe { (api.column_name)(statement.pointer, column) }).unwrap_or_default();
                let declaration = c_bytes(unsafe { (api.column_decltype)(statement.pointer, column) }).unwrap_or_default();
                used += name.len() + declaration.len();
                if name.len() > 4096 || declaration.len() > 4096 || used > 1_048_576 {
                    return Err(self.raise("resource", "SQLITE_COLUMN_LIMIT"));
                }
                let (Some(name_text), Some(declaration_text)) = (strict(&name), strict(&declaration)) else {
                    return Err(self.raise("data", "SQLITE_UTF8"));
                };
                raw_names.push(latin1(&name));
                raw_declarations.push(latin1(&declaration));
                names.push(name_text);
                declarations.push(declaration_text);
            }
            Ok((names, declarations, raw_names, raw_declarations))
        })();
        match outcome {
            Err(error) => {
                let _ = self.finish(&mut statement);
                if statement.prepared != 0 {
                    self.restore(statement);
                }
                Err(error)
            }
            Ok((names, declarations, raw_names, raw_declarations)) => {
                statement.verified = false;
                statement.raw_names = raw_names;
                statement.raw_declarations = raw_declarations;
                self.serial += 1;
                statement.id = self.serial;
                let id = statement.id;
                self.cursor = Some(statement);
                Ok(Data {
                    id: Some(id),
                    names: Some(names),
                    declarations: Some(declarations),
                    ..Data::default()
                })
            }
        }
    }

    fn verify(&self, cursor: &mut Statement) -> Done<()> {
        if cursor.verified {
            return Ok(());
        }
        let api = ready_api();
        // SAFETY: открытый курсор этого соединения.
        if unsafe { (api.column_count)(cursor.pointer) } != cursor.columns {
            return Err(self.raise("data", "SQLITE_SCHEMA_CHANGED"));
        }
        for column in 0..cursor.columns {
            let name = latin1(&c_bytes(unsafe { (api.column_name)(cursor.pointer, column) }).unwrap_or_default());
            let declaration = latin1(&c_bytes(unsafe { (api.column_decltype)(cursor.pointer, column) }).unwrap_or_default());
            if name != cursor.raw_names[column as usize] || declaration != cursor.raw_declarations[column as usize] {
                return Err(self.raise("data", "SQLITE_SCHEMA_CHANGED"));
            }
        }
        cursor.verified = true;
        Ok(())
    }

    fn fetch(&mut self, id: i64, maximum: i64) -> Done<Data> {
        if self.cursor.as_ref().is_none_or(|cursor| cursor.id != id) {
            return Err(self.raise("data", "SQLITE_READER_STALE"));
        }
        if !(1..=64).contains(&maximum) {
            return Err(self.raise("data", "SQLITE_BATCH_LIMIT"));
        }
        let api = ready_api();
        let mut cursor = self.cursor.take().expect("cursor");
        let mut rows: Vec<Vec<Cell>> = Vec::new();
        let mut eof = false;
        let mut used = 0usize;
        if let Some((values, bytes)) = cursor.pending_row.take() {
            rows.push(values);
            used = bytes;
        }
        let outcome = (|| -> Done<()> {
            while (rows.len() as i64) < maximum {
                // SAFETY: открытый курсор этого соединения.
                let rc = unsafe { (api.step)(cursor.pointer) };
                if rc == 101 {
                    self.verify(&mut cursor)?;
                    eof = true;
                    break;
                }
                if rc != 100 {
                    return Err(self.error(rc));
                }
                self.verify(&mut cursor)?;
                let mut values = Vec::new();
                let mut row_bytes = 0usize;
                for column in 0..cursor.columns {
                    let kind = unsafe { (api.column_type)(cursor.pointer, column) };
                    let cell = match kind {
                        5 => Cell::Null,
                        1 => {
                            let value = unsafe { (api.column_int64)(cursor.pointer, column) }.to_string();
                            row_bytes += value.len();
                            Cell::Integer(value)
                        }
                        2 => {
                            let number = unsafe { (api.column_double)(cursor.pointer, column) };
                            if number.is_nan() || number.is_infinite() {
                                return Err(self.raise("data", "SQLITE_FLOAT_RANGE"));
                            }
                            let value = crate::float::format_g(number, 17);
                            row_bytes += value.len();
                            Cell::Floating(value)
                        }
                        3 | 4 => {
                            let pointer = if kind == 3 {
                                unsafe { (api.column_text)(cursor.pointer, column) as *const u8 }
                            } else {
                                unsafe { (api.column_blob)(cursor.pointer, column) as *const u8 }
                            };
                            let size = unsafe { (api.column_bytes)(cursor.pointer, column) };
                            if size < 0 || size as usize > 8_388_608 - row_bytes.min(8_388_608) {
                                return Err(self.raise("resource", "SQLITE_ROW_LIMIT"));
                            }
                            let data = if size == 0 || pointer.is_null() {
                                Vec::new()
                            } else {
                                // SAFETY: SQLite отдал `size` байтов по указателю.
                                unsafe { std::slice::from_raw_parts(pointer, size as usize) }.to_vec()
                            };
                            row_bytes += size as usize;
                            if kind == 3 {
                                let Some(text) = strict(&data) else {
                                    return Err(self.raise("data", "SQLITE_UTF8"));
                                };
                                Cell::Text(text)
                            } else {
                                Cell::Bytes(data)
                            }
                        }
                        _ => return Err(self.raise("unexpected", "SQLITE_STORAGE_KIND")),
                    };
                    if row_bytes > 8_388_608 {
                        return Err(self.raise("resource", "SQLITE_ROW_LIMIT"));
                    }
                    values.push(cell);
                }
                if used + row_bytes > 8_388_608 {
                    cursor.pending_row = Some((values, row_bytes));
                    break;
                }
                rows.push(values);
                used += row_bytes;
            }
            Ok(())
        })();
        self.cursor = Some(cursor);
        if let Err(error) = outcome {
            self.release(id)?;
            return Err(error);
        }
        if eof {
            self.release(id)?;
        }
        Ok(Data {
            rows: Some(rows),
            eof,
            autocommit: Some(self.autocommit()),
            ..Data::default()
        })
    }

    fn release(&mut self, id: i64) -> Done<()> {
        if self.cursor.as_ref().is_none_or(|cursor| cursor.id != id) {
            return Ok(());
        }
        let mut cursor = self.cursor.take().expect("cursor");
        let finished = self.finish(&mut cursor);
        if cursor.prepared != 0 {
            self.restore(cursor);
        }
        finished
    }

    fn finish(&self, statement: &mut Statement) -> Done<()> {
        let api = ready_api();
        statement.pending_row = None;
        // SAFETY: оператор этого соединения.
        unsafe {
            if statement.prepared != 0 {
                (api.reset)(statement.pointer);
                self.check((api.clear_bindings)(statement.pointer))
            } else {
                self.check((api.finalize)(statement.pointer))
            }
        }
    }

    fn prepare_statement(&mut self, sql: &str) -> Done<Data> {
        if self.prepared.len() >= 64 {
            return Err(self.raise("resource", "SQLITE_PREPARED_LIMIT"));
        }
        let mut statement = self.prepare(sql)?;
        self.serial += 1;
        let id = self.serial;
        statement.prepared = id;
        self.prepared.push((id, statement));
        Ok(Data { id: Some(id), ..Data::default() })
    }

    /// Подготовленный оператор забирается на время выполнения.
    fn take_prepared(&mut self, id: i64) -> Done<Statement> {
        self.ready()?;
        let Some(slot) = self.prepared.iter_mut().find(|(known, _)| *known == id) else {
            return Err(self.raise("data", "SQLITE_PREPARED_STALE"));
        };
        Ok(Statement {
            pointer: slot.1.pointer,
            dml: slot.1.dml,
            columns: slot.1.columns,
            prepared: slot.1.prepared,
            id: 0,
            verified: false,
            raw_names: Vec::new(),
            raw_declarations: Vec::new(),
            pending_row: None,
        })
    }

    fn unprepare(&mut self, id: i64) -> Done<Data> {
        let Some(position) = self.prepared.iter().position(|(known, _)| *known == id) else {
            return Ok(Data::default());
        };
        let (_, statement) = self.prepared.remove(position);
        if let Some(cursor_id) = self.cursor.as_ref().filter(|cursor| cursor.pointer == statement.pointer).map(|cursor| cursor.id) {
            self.release(cursor_id)?;
            // release вернул бы его в таблицу: он уже удалён.
            self.prepared.retain(|(known, _)| *known != id);
        }
        // SAFETY: оператор этого соединения, финализируется один раз.
        self.check(unsafe { (ready_api().finalize)(statement.pointer) })?;
        Ok(Data::default())
    }

    fn identifier(&self, name: &str) -> Done<CString> {
        if name.is_empty() || name.len() > 1024 || name.contains('\0') {
            return Err(self.raise("data", "SQLITE_IDENTIFIER"));
        }
        CString::new(name).map_err(|_| Stop::Unexpected)
    }

    fn int64(&self, value: &str) -> Done<i64> {
        let body = value.strip_prefix('-').unwrap_or(value);
        let canonical = body == "0" || (body.bytes().next().is_some_and(|b| (b'1'..=b'9').contains(&b)) && body.bytes().all(|b| b.is_ascii_digit()));
        if !canonical || value.len() > 20 {
            return Err(self.raise("data", "SQLITE_INTEGER_RANGE"));
        }
        value.parse::<i64>().map_err(|_| self.raise("data", "SQLITE_INTEGER_RANGE"))
    }

    fn blob_open(&mut self, database: &str, table: &str, column: &str, row: &str, writable: bool) -> Done<Data> {
        self.ready()?;
        if !self.blobs.is_empty() {
            return Err(self.raise("busy", "SQLITE_BLOB_ACTIVE"));
        }
        let (database, table, column) = (self.identifier(database)?, self.identifier(table)?, self.identifier(column)?);
        let rowid = self.int64(row)?;
        let api = ready_api();
        let mut blob: *mut c_void = std::ptr::null_mut();
        // SAFETY: строки с нулём, указатель на слот.
        let rc = unsafe { (api.blob_open)(self.db, database.as_ptr(), table.as_ptr(), column.as_ptr(), rowid, c_int::from(writable), &mut blob) };
        if rc != 0 {
            if !blob.is_null() {
                unsafe { (api.blob_close)(blob) };
            }
            return Err(self.error(rc));
        }
        self.serial += 1;
        let id = self.serial;
        let size = unsafe { (api.blob_bytes)(blob) };
        self.blobs.push((id, blob, size, writable));
        Ok(Data { id: Some(id), size: Some(i64::from(size)), ..Data::default() })
    }

    fn blob(&self, id: i64, offset: &str, length: i64) -> Done<(*mut c_void, c_int, bool)> {
        let Some(&(_, blob, size, writable)) = self.blobs.iter().find(|(known, ..)| *known == id) else {
            return Err(self.raise("data", "SQLITE_BLOB_STALE"));
        };
        let valid = !offset.is_empty() && offset.bytes().all(|b| b.is_ascii_digit()) && offset.len() <= 10;
        let at = if valid { offset.parse::<i64>().unwrap_or(i64::MAX) } else { -1 };
        if !valid || !(0..=65536).contains(&length) || at > i64::from(size) || length > i64::from(size) - at {
            return Err(self.raise("data", "SQLITE_BLOB_BOUNDS"));
        }
        Ok((blob, at as c_int, writable))
    }

    fn blob_read(&self, id: i64, offset: &str, length: i64) -> Done<Data> {
        if length <= 0 {
            return Err(self.raise("data", "SQLITE_BLOB_BOUNDS"));
        }
        let (blob, at, _) = self.blob(id, offset, length)?;
        let mut buffer = vec![0u8; length as usize];
        // SAFETY: буфер длины `length`.
        self.check(unsafe { (ready_api().blob_read)(blob, buffer.as_mut_ptr() as *mut c_void, length as c_int, at) })?;
        Ok(Data { bytes: Some(buffer), ..Data::default() })
    }

    fn blob_write(&self, id: i64, offset: &str, data: &[u8]) -> Done<Data> {
        let (blob, at, writable) = self.blob(id, offset, data.len() as i64)?;
        if !writable {
            return Err(self.raise("permission", "SQLITE_BLOB_READ_ONLY"));
        }
        let buffer = if data.is_empty() { vec![0u8] } else { data.to_vec() };
        // SAFETY: буфер длины не меньше `data.len()`.
        self.check(unsafe { (ready_api().blob_write)(blob, buffer.as_ptr() as *const c_void, data.len() as c_int, at) })?;
        Ok(Data::default())
    }

    fn blob_close(&mut self, id: i64) -> Done<Data> {
        if let Some(position) = self.blobs.iter().position(|(known, ..)| *known == id) {
            let (_, blob, ..) = self.blobs.remove(position);
            // SAFETY: открытый blob, закрывается один раз.
            self.check(unsafe { (ready_api().blob_close)(blob) })?;
        }
        Ok(Data { autocommit: Some(self.autocommit()), ..Data::default() })
    }

    fn same_file(&self, left: &str, right: &str) -> bool {
        use std::os::unix::fs::MetadataExt;
        let (Ok(a), Ok(b)) = (std::fs::metadata(crate::host::path(left)), std::fs::metadata(crate::host::path(right))) else {
            return false;
        };
        a.dev() == b.dev() && a.ino() == b.ino()
    }

    fn backup_open(&mut self, path: &str) -> Done<Data> {
        self.ready()?;
        if self.backup.is_some() {
            return Err(self.raise("busy", "SQLITE_BACKUP_ACTIVE"));
        }
        if !self.autocommit() {
            return Err(self.raise("busy", "SQLITE_BACKUP_TRANSACTION"));
        }
        if path.is_empty() || path.len() > 4096 || path.contains('\0') || path == ":memory:" {
            return Err(self.raise("data", "SQLITE_BACKUP_PATH"));
        }
        if path == self.options.path || self.same_file(&self.options.path, path) {
            return Err(self.raise("data", "SQLITE_BACKUP_SAME_FILE"));
        }
        let api = ready_api();
        let target_path = CString::new(crate::host::path(path).to_string_lossy().as_bytes()).map_err(|_| Stop::Unexpected)?;
        let mut target: Db = std::ptr::null_mut();
        // SAFETY: C API SQLite.
        let rc = unsafe { (api.open)(target_path.as_ptr(), &mut target, 6, std::ptr::null()) };
        if rc != 0 {
            if !target.is_null() {
                unsafe { (api.close)(target) };
            }
            return Err(self.error(rc));
        }
        let main = CString::new("main").expect("no NUL");
        let pointer = unsafe { (api.backup_init)(target, main.as_ptr(), self.db, main.as_ptr()) };
        if pointer.is_null() {
            let code = unsafe { (api.extended_errcode)(target) };
            unsafe { (api.close)(target) };
            return Err(self.error(code));
        }
        self.serial += 1;
        let id = self.serial;
        self.backup = Some((pointer, target, id));
        Ok(Data { id: Some(id), ..Data::default() })
    }

    fn backup_step(&self, id: i64, pages: i64) -> Done<Data> {
        let Some((pointer, _, known)) = self.backup.filter(|(_, _, known)| *known == id) else {
            return Err(self.raise("data", "SQLITE_BACKUP_STALE"));
        };
        let _ = known;
        if !(1..=256).contains(&pages) {
            return Err(self.raise("data", "SQLITE_BACKUP_PAGES"));
        }
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(self.raise("cancelled", "SQLITE_9"));
        }
        let api = ready_api();
        // SAFETY: открытая копия этого соединения.
        let rc = unsafe { (api.backup_step)(pointer, pages as c_int) };
        if !matches!(rc, 0 | 101 | 5 | 6) {
            return Err(self.error(rc));
        }
        let total = unsafe { (api.backup_pagecount)(pointer) };
        let remaining = unsafe { (api.backup_remaining)(pointer) };
        Ok(Data {
            done: rc == 101,
            busy: rc == 5 || rc == 6,
            total: Some(i64::from(total)),
            copies: Some(i64::from(total - remaining)),
            ..Data::default()
        })
    }

    fn backup_close(&mut self, id: i64) -> Done<Data> {
        let Some((pointer, target, _)) = self.backup.filter(|(_, _, known)| *known == id) else {
            return Ok(Data::default());
        };
        self.backup = None;
        let api = ready_api();
        // SAFETY: копия и её база закрываются один раз.
        let rc = unsafe { (api.backup_finish)(pointer) };
        let closed = unsafe { (api.close)(target) };
        self.check(rc)?;
        self.check(closed)?;
        Ok(Data::default())
    }

    fn script(&mut self, sql: &str) -> Done<Data> {
        self.ready()?;
        let bytes = sql.as_bytes();
        if bytes.len() > 1_048_576 || sql.contains('\0') {
            return Err(self.raise("data", "SQLITE_SCRIPT_LIMIT"));
        }
        let api = ready_api();
        let mut text = bytes.to_vec();
        text.push(0);
        let base = text.as_ptr() as *const c_char;
        let mut results = Vec::new();
        let mut offset = 0usize;
        while offset < bytes.len() {
            if self.cancelled.load(Ordering::SeqCst) {
                return Err(self.raise("cancelled", "SQLITE_9"));
            }
            self.dml_seen = false;
            self.ddl_seen = false;
            let mut statement: Stmt = std::ptr::null_mut();
            let mut tail: *const c_char = std::ptr::null();
            // SAFETY: хвост внутри текста.
            let rc = unsafe { (api.prepare)(self.db, base.add(offset), (bytes.len() - offset) as c_int, 0, &mut statement, &mut tail) };
            if rc != 0 {
                if !statement.is_null() {
                    unsafe { (api.finalize)(statement) };
                }
                return Err(self.error(rc));
            }
            let next = (tail as isize) - (base as isize);
            if next <= offset as isize || next as usize > bytes.len() {
                if !statement.is_null() {
                    unsafe { (api.finalize)(statement) };
                }
                return Err(self.raise("unexpected", "SQLITE_PREPARE_TAIL"));
            }
            offset = next as usize;
            if statement.is_null() {
                continue;
            }
            if results.len() >= 256 {
                unsafe { (api.finalize)(statement) };
                return Err(self.raise("resource", "SQLITE_SCRIPT_COUNT"));
            }
            let prepared = Statement {
                pointer: statement,
                dml: self.dml_seen && !self.ddl_seen,
                columns: unsafe { (api.column_count)(statement) },
                prepared: 0,
                id: 0,
                verified: false,
                raw_names: Vec::new(),
                raw_declarations: Vec::new(),
                pending_row: None,
            };
            let data = self.execute(prepared, &[])?;
            results.push(data.affected.flatten());
        }
        Ok(Data {
            executions: Some(results),
            autocommit: Some(self.autocommit()),
            ..Data::default()
        })
    }

    fn rollback(&mut self) -> Done<Data> {
        let ids: Vec<i64> = self.blobs.iter().map(|(id, ..)| *id).collect();
        for id in ids {
            self.blob_close(id)?;
        }
        if let Some(id) = self.cursor.as_ref().map(|cursor| cursor.id) {
            self.release(id)?;
        }
        if self.autocommit() {
            return Ok(Data { autocommit: Some(true), ..Data::default() });
        }
        self.execution("ROLLBACK", &[], true)
    }

    fn close(&mut self) -> Done<()> {
        if self.db.is_null() {
            return Ok(());
        }
        if let Some((_, _, id)) = self.backup {
            self.backup_close(id)?;
        }
        let ids: Vec<i64> = self.blobs.iter().map(|(id, ..)| *id).collect();
        for id in ids {
            self.blob_close(id)?;
        }
        if let Some(id) = self.cursor.as_ref().map(|cursor| cursor.id) {
            self.release(id)?;
        }
        let ids: Vec<i64> = self.prepared.iter().map(|(id, _)| *id).collect();
        for id in ids {
            self.unprepare(id)?;
        }
        let current = self.db;
        self.db = std::ptr::null_mut();
        self.shared_db.store(std::ptr::null_mut(), Ordering::SeqCst);
        // SAFETY: соединение закрывается один раз.
        self.check(unsafe { (ready_api().close)(current) })
    }
}

fn first_cell(reply: &Data) -> String {
    match reply.rows.as_ref().and_then(|rows| rows.first()).and_then(|row| row.first()) {
        Some(Cell::Integer(value) | Cell::Floating(value) | Cell::Text(value)) => value.clone(),
        Some(Cell::Null) | None => String::new(),
        Some(Cell::Bytes(bytes)) => format!("{bytes:?}"),
    }
}

#[derive(Debug, Clone)]
enum Param {
    Text(String),
    Bytes(Vec<u8>),
}

// ---- соединение ----

type Job = Box<dyn FnOnce(&mut Engine) + Send>;

struct Worker {
    jobs: mpsc::Sender<Job>,
    exited: Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
}

struct Connection {
    options: Options,
    stopped: bool,
    broken: bool,
    autocommit: bool,
    handle_closed: bool,
    worker: Option<Worker>,
    db: Arc<AtomicPtr<c_void>>,
    cancelled: Arc<AtomicBool>,
}

type ConnectionRef = Rc<RefCell<Connection>>;

// SAFETY: Engine живёт и используется только в потоке соединения.
struct SendEngine(Box<Engine>);
unsafe impl Send for SendEngine {}

fn spawn_worker(connection: &mut Connection) {
    let (jobs, queue) = mpsc::channel::<Job>();
    let exited = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
    let engine = SendEngine(Box::new(Engine {
        db: std::ptr::null_mut(),
        shared_db: connection.db.clone(),
        cancelled: connection.cancelled.clone(),
        options: connection.options.clone(),
        serial: 0,
        cursor: None,
        prepared: Vec::new(),
        blobs: Vec::new(),
        backup: None,
        tail_guard: false,
        control: false,
        configuration: false,
        dml_seen: false,
        ddl_seen: false,
    }));
    let signal = exited.clone();
    std::thread::Builder::new()
        .name("okkama-sqlite".into())
        .spawn(move || {
            let mut engine = engine;
            while let Ok(job) = queue.recv() {
                job(&mut engine.0);
            }
            let (lock, changed) = &*signal;
            if let Ok(mut done) = lock.lock() {
                *done = true;
            }
            changed.notify_all();
        })
        .expect("sqlite worker");
    connection.worker = Some(Worker { jobs, exited });
}

type Operation = Box<dyn FnOnce(&mut Engine) -> Done<Data> + Send>;

fn perform(engine: &mut Engine, operation: Operation) -> Reply {
    engine.cancelled.store(false, Ordering::SeqCst);
    let (ok, data) = match operation(engine) {
        Ok(data) => (true, data),
        Err(Stop::Sqlite { kind, code, autocommit }) => (
            false,
            Data {
                kind: Some(kind.into()),
                code: Some(code),
                autocommit: Some(autocommit),
                ..Data::default()
            },
        ),
        Err(Stop::Unexpected) => (
            false,
            Data {
                kind: Some("unexpected".into()),
                code: Some("SQLITE_WORKER_FAILURE".into()),
                ..Data::default()
            },
        ),
    };
    Reply { ok, data, closed: RefCell::new(false) }
}

/// Reply пересекает границу потока до того, как его увидит программа.
struct SendReply(Reply);
unsafe impl Send for SendReply {}

fn facts(connection: &ConnectionRef, reply: &Reply) {
    if let Some(autocommit) = reply.data.autocommit {
        connection.borrow_mut().autocommit = autocommit;
    }
}

/// `_request`: очередная операция; Future с Reply либо отказ.
fn request(state: &ConnectionRef, op: &'static str, operation: Operation, name: &'static str, arity: usize) -> Value {
    let jobs = {
        let connection = state.borrow();
        if connection.stopped || connection.broken {
            return resolved(Err(fail("already_closed", name, arity, "SQLite connection is closed")));
        }
        let Some(worker) = &connection.worker else {
            return resolved(Err(fail("invalid_input", name, arity, "SQLite connection is not acquired")));
        };
        worker.jobs.clone()
    };
    let (reply_sender, reply_receiver) = mpsc::channel::<SendReply>();
    let job: Job = Box::new(move |engine| {
        let _ = reply_sender.send(SendReply(perform(engine, operation)));
    });
    if jobs.send(job).is_err() {
        return resolved(Err(fail("already_closed", name, arity, "SQLite connection is closed")));
    }
    let promise = sched::promise();
    let target = promise.clone();
    let delivered = state.clone();
    let pending = sched::offload(
        move || reply_receiver.recv().ok(),
        move |reply: Option<SendReply>| {
            let Some(SendReply(reply)) = reply else {
                delivered.borrow_mut().broken = true;
                sched::settle(&target, Err(fail("connection_reset", "sqlite_connect", 1, "SQLite worker lost")));
                return;
            };
            facts(&delivered, &reply);
            if target.state() != sched::State::Running && reply.ok {
                cleanup_cancelled(&delivered, op, &reply);
                return;
            }
            sched::settle(&target, Ok(new_handle(Rc::new(reply))));
        },
    );
    let watched = promise.clone();
    let (db, cancelled) = {
        let connection = state.borrow();
        (connection.db.clone(), connection.cancelled.clone())
    };
    promise.listen(move || {
        pending.detach();
        let current = db.load(Ordering::SeqCst);
        if watched.cancelled() && !current.is_null() {
            cancelled.store(true, Ordering::SeqCst);
            if let Loaded::Ready(api) = api() {
                // SAFETY: sqlite3_interrupt потокобезопасен.
                unsafe { (api.interrupt)(current) };
            }
        }
    });
    Value::Future(promise)
}

/// Отменённая операция всё же удалась: освободить её ресурс (как Perl).
fn cleanup_cancelled(state: &ConnectionRef, op: &str, reply: &Reply) {
    let Some(id) = reply.data.id else {
        return;
    };
    match op {
        "rows" | "prepared_rows" => {
            maintenance(state, Box::new(move |engine| engine.release(id).map(|()| Data::default())));
        }
        "prepare" => {
            maintenance(state, Box::new(move |engine| engine.unprepare(id)));
        }
        "blob_open" => {
            maintenance(state, Box::new(move |engine| engine.blob_close(id)));
        }
        "backup_open" => {
            maintenance(state, Box::new(move |engine| engine.backup_close(id)));
        }
        _ => {}
    }
}

/// `_maintenance`: служебная операция в очереди; ответ не ждём.
fn maintenance(state: &ConnectionRef, operation: Operation) -> bool {
    let jobs = {
        let connection = state.borrow();
        if connection.stopped {
            return true;
        }
        let Some(worker) = &connection.worker else {
            return true;
        };
        worker.jobs.clone()
    };
    let (reply_sender, reply_receiver) = mpsc::channel::<SendReply>();
    let job: Job = Box::new(move |engine| {
        let _ = reply_sender.send(SendReply(perform(engine, operation)));
    });
    if jobs.send(job).is_err() {
        state.borrow_mut().broken = true;
        return true;
    }
    let delivered = state.clone();
    let op = sched::offload(
        move || reply_receiver.recv().ok(),
        move |reply: Option<SendReply>| match reply {
            Some(SendReply(reply)) if reply.ok => facts(&delivered, &reply),
            _ => delivered.borrow_mut().broken = true,
        },
    );
    // Служебная операция не держит программу.
    op.detach();
    true
}

fn connection(value: &Value, name: &str, arity: usize) -> Result<ConnectionRef, Rc<Failure>> {
    let state = handle::<ConnectionRef>(value).map_err(|_| fail("invalid_input", name, arity, "Invalid SQLite state"))?.clone();
    if state.borrow().handle_closed {
        return Err(fail("already_closed", name, arity, "Native handle is already closed"));
    }
    Ok(state)
}

fn reply<'a>(value: &'a Value, name: &str, arity: usize) -> Result<&'a Rc<Reply>, Error> {
    let reply = handle::<Rc<Reply>>(value)?;
    if *reply.closed.borrow() {
        return Err(as_error(fail("already_closed", name, arity, "Native handle is already closed")));
    }
    Ok(reply)
}

const KINDS: [&str; 7] = ["null_value", "boolean", "integer", "floating", "decimal", "text", "bytes"];

fn params(tags: &Value, payloads: &Value, name: &str, arity: usize) -> Result<Vec<(&'static str, Param)>, Rc<Failure>> {
    let invalid = |message: &str| fail("invalid_input", name, arity, message);
    let tags = items(tags).map_err(|_| invalid("Invalid SQLite bindings"))?;
    let payloads = items(payloads).map_err(|_| invalid("Invalid SQLite bindings"))?;
    if tags.len() != payloads.len() || tags.len() > 1024 {
        return Err(invalid("Invalid SQLite bindings"));
    }
    let mut out = Vec::new();
    let mut used = 0usize;
    for (tag, payload) in tags.iter().zip(payloads) {
        let tag = int(tag).map_err(|_| invalid("Invalid SQLite binding kind"))?;
        let Some(tag) = tag.to_i64().filter(|tag| (0..=6).contains(tag)) else {
            return Err(invalid("Invalid SQLite binding kind"));
        };
        let data = bytes(payload).map_err(|_| invalid("Invalid SQLite bindings"))?;
        used += data.len();
        if used > 8_388_608 {
            return Err(fail("resource_exhausted", name, arity, "SQLite binding byte budget"));
        }
        let value = if tag == 6 {
            Param::Bytes(data)
        } else {
            Param::Text(strict(&data).ok_or_else(|| invalid("Invalid SQLite binding encoding"))?)
        };
        out.push((KINDS[tag as usize], value));
    }
    Ok(out)
}

fn sql_text(value: &Value, name: &str, arity: usize) -> Result<String, Rc<Failure>> {
    let data = bytes(value).map_err(|_| fail("invalid_input", name, arity, "Invalid SQLite SQL"))?;
    strict(&data).ok_or_else(|| fail("invalid_input", name, arity, "Invalid SQLite SQL"))
}

fn statement(args: &[Value], rows: bool, name: &'static str) -> Value {
    let attempt = || -> Result<Value, Rc<Failure>> {
        let state = connection(&args[0], name, 4)?;
        let sql = sql_text(&args[1], name, 4)?;
        if sql.is_empty() || sql.contains('\0') || sql.len() > 1_048_576 {
            return Err(fail("invalid_input", name, 4, "Invalid SQLite SQL"));
        }
        let params = params(&args[2], &args[3], name, 4)?;
        let operation: Operation = if rows {
            Box::new(move |engine| engine.rows_sql(&sql, &params))
        } else {
            Box::new(move |engine| engine.execution(&sql, &params, false))
        };
        Ok(request(&state, if rows { "rows" } else { "execution" }, operation, name, 4))
    };
    attempt().unwrap_or_else(|failure| resolved(Err(failure)))
}

fn prepared_statement(args: &[Value], rows: bool, name: &'static str) -> Value {
    let attempt = || -> Result<Value, Rc<Failure>> {
        let state = connection(&args[0], name, 4)?;
        let id = int(&args[1]).map_err(|_| fail("invalid_input", name, 4, "Invalid SQLite bindings"))?;
        let counts = (items(&args[2]).map(<[Value]>::len).unwrap_or(0), items(&args[3]).map(<[Value]>::len).unwrap_or(0));
        if id.signum() < 1 || counts.0 != counts.1 || counts.0 > 1024 {
            return Err(fail("invalid_input", name, 4, "Invalid SQLite bindings"));
        }
        let params = params(&args[2], &args[3], name, 4)?;
        let id = java_long(id);
        let operation: Operation = if rows {
            Box::new(move |engine| {
                let statement = engine.take_prepared(id)?;
                engine.rows(statement, &params)
            })
        } else {
            Box::new(move |engine| {
                let statement = engine.take_prepared(id)?;
                engine.execute(statement, &params)
            })
        };
        Ok(request(&state, if rows { "prepared_rows" } else { "prepared_execution" }, operation, name, 4))
    };
    attempt().unwrap_or_else(|failure| resolved(Err(failure)))
}

fn path_or_sql(value: &Value, path: bool, name: &str, arity: usize) -> Result<String, Rc<Failure>> {
    let data = bytes(value).map_err(|_| fail("invalid_input", name, arity, "Invalid SQLite text"))?;
    let text = strict(&data);
    let Some(text) = text.filter(|text| !text.contains('\0') && data.len() <= if path { 4096 } else { 1_048_576 }) else {
        return Err(fail("invalid_input", name, arity, "Invalid SQLite text"));
    };
    Ok(if path { crate::host::path(&text).to_string_lossy().into_owned() } else { text })
}

fn cell<'a>(reply: &'a Reply, row: &IntValue, column: &IntValue, name: &str) -> Result<&'a Cell, Error> {
    let rows: &[Vec<Cell>] = if reply.ok { reply.data.rows.as_deref().unwrap_or(&[]) } else { &[] };
    let outside = || as_error(fail("invalid_input", name, 3, "SQLite reply cell outside bounds"));
    let row = row.to_i64().filter(|row| *row >= 0 && (*row as usize) < rows.len()).ok_or_else(outside)? as usize;
    let column = column.to_i64().filter(|column| *column >= 0 && (*column as usize) < rows[row].len()).ok_or_else(outside)? as usize;
    Ok(&rows[row][column])
}

fn settled(state: &ConnectionRef) -> Value {
    let worker = {
        let mut connection = state.borrow_mut();
        if connection.handle_closed {
            return resolved(Ok(Value::Bool(true)));
        }
        match &connection.worker {
            None => {
                if connection.stopped {
                    connection.handle_closed = true;
                }
                return resolved(Ok(Value::Bool(true)));
            }
            Some(worker) => (worker.jobs.clone(), worker.exited.clone(), connection.stopped),
        }
    };
    let (jobs, exited, stopped) = worker;
    let promise = sched::promise();
    let target = promise.clone();
    let delivered = state.clone();
    let (done_sender, done_receiver) = mpsc::channel::<()>();
    if !stopped {
        let _ = jobs.send(Box::new(move |_engine| {
            let _ = done_sender.send(());
        }));
    }
    drop(jobs);
    sched::offload(
        move || {
            if stopped {
                let (lock, changed) = &*exited;
                if let Ok(done) = lock.lock() {
                    let _ = changed.wait_timeout_while(done, std::time::Duration::from_secs(2), |done| !*done);
                }
            } else {
                let _ = done_receiver.recv();
            }
        },
        move |()| {
            if delivered.borrow().stopped {
                delivered.borrow_mut().handle_closed = true;
            }
            sched::settle(&target, Ok(Value::Bool(true)));
        },
    );
    Value::Future(promise)
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let name = native.name();
    match name {
        "sqlite_create" => {
            let path = bytes(&args[0])?;
            let (read_only, create, foreign) = (args[1].as_bool()?, args[2].as_bool()?, args[3].as_bool()?);
            let busy = int(&args[4])?;
            let journal = string(&args[5])?;
            let memory = args[6].as_bool()?;
            let busy_ok = !busy.is_negative() && busy.to_i64().is_some_and(|busy| busy <= 60000);
            if path.len() > 4096 || path.contains(&0) || !busy_ok || (read_only && create) || !matches!(journal, "delete" | "wal" | "memory") {
                return Err(as_error(fail("invalid_input", "sqlite_create", 7, "Invalid SQLite options")));
            }
            let Some(text) = strict(&path).filter(|text| memory || !text.is_empty()) else {
                return Err(as_error(fail("invalid_input", "sqlite_create", 7, "SQLite path must be UTF8")));
            };
            Ok(new_handle(Rc::new(RefCell::new(Connection {
                options: Options {
                    path: if memory { ":memory:".into() } else { crate::host::path(&text).to_string_lossy().into_owned() },
                    read_only,
                    create,
                    foreign_keys: foreign,
                    busy_ms: busy.to_i64().unwrap_or(0) as c_int,
                    journal: journal.to_string(),
                },
                stopped: false,
                broken: false,
                autocommit: true,
                handle_closed: false,
                worker: None,
                db: Arc::new(AtomicPtr::new(std::ptr::null_mut())),
                cancelled: Arc::new(AtomicBool::new(false)),
            }))))
        }
        "sqlite_connect" => {
            let state = match connection(&args[0], "sqlite_connect", 1) {
                Ok(state) => state,
                Err(failure) => return Ok(resolved(Err(failure))),
            };
            if state.borrow().worker.is_some() {
                return Ok(resolved(Err(fail("invalid_input", "sqlite_connect", 1, "SQLite connection already acquired"))));
            }
            spawn_worker(&mut state.borrow_mut());
            Ok(request(
                &state,
                "open",
                Box::new(|engine| {
                    engine.open()?;
                    Ok(Data { autocommit: Some(true), ..Data::default() })
                }),
                "sqlite_connect",
                1,
            ))
        }
        "sqlite_execution" => Ok(statement(args, false, "sqlite_execution")),
        "sqlite_rows" => Ok(statement(args, true, "sqlite_rows")),
        "sqlite_prepare" => {
            let attempt = || -> Result<Value, Rc<Failure>> {
                let state = connection(&args[0], "sqlite_prepare", 2)?;
                let sql = sql_text(&args[1], "sqlite_prepare", 2)?;
                if sql.len() > 1_048_576 || sql.contains('\0') {
                    return Err(fail("invalid_input", "sqlite_prepare", 2, "Invalid SQLite SQL"));
                }
                Ok(request(&state, "prepare", Box::new(move |engine| engine.prepare_statement(&sql)), "sqlite_prepare", 2))
            };
            Ok(attempt().unwrap_or_else(|failure| resolved(Err(failure))))
        }
        "sqlite_prepared_execution" => Ok(prepared_statement(args, false, "sqlite_prepared_execution")),
        "sqlite_prepared_rows" => Ok(prepared_statement(args, true, "sqlite_prepared_rows")),
        "sqlite_unprepare" => {
            let id = java_long(int(&args[1])?);
            let state = connection(&args[0], "sqlite_unprepare", 2).map_err(as_error)?;
            Ok(Value::Bool(maintenance(&state, Box::new(move |engine| engine.unprepare(id)))))
        }
        "sqlite_blob_open" => {
            let state = match connection(&args[0], "sqlite_blob_open", 6) {
                Ok(state) => state,
                Err(failure) => return Ok(resolved(Err(failure))),
            };
            let (database, table, column, row) = (
                string(&args[1])?.to_string(),
                string(&args[2])?.to_string(),
                string(&args[3])?.to_string(),
                string(&args[4])?.to_string(),
            );
            let writable = args[5].as_bool()?;
            Ok(request(
                &state,
                "blob_open",
                Box::new(move |engine| engine.blob_open(&database, &table, &column, &row, writable)),
                "sqlite_blob_open",
                6,
            ))
        }
        "sqlite_blob_read" => {
            let state = match connection(&args[0], "sqlite_blob_read", 4) {
                Ok(state) => state,
                Err(failure) => return Ok(resolved(Err(failure))),
            };
            let blob = java_long(int(&args[1])?);
            let offset = string(&args[2])?.to_string();
            let size = int(&args[3])?.to_i64().filter(|size| i32::try_from(*size).is_ok()).unwrap_or(-1);
            Ok(request(&state, "blob_read", Box::new(move |engine| engine.blob_read(blob, &offset, size)), "sqlite_blob_read", 4))
        }
        "sqlite_blob_write" => {
            let attempt = || -> Result<Value, Rc<Failure>> {
                let state = connection(&args[0], "sqlite_blob_write", 4)?;
                let data = bytes(&args[3]).map_err(|_| fail("invalid_input", "sqlite_blob_write", 4, "Invalid SQLite blob"))?;
                if data.len() > 65536 {
                    return Err(fail("resource_exhausted", "sqlite_blob_write", 4, "SQLite blob byte budget"));
                }
                let blob = int(&args[1]).map(java_long).unwrap_or(0);
                let offset = string(&args[2]).map(str::to_string).unwrap_or_default();
                Ok(request(&state, "blob_write", Box::new(move |engine| engine.blob_write(blob, &offset, &data)), "sqlite_blob_write", 4))
            };
            Ok(attempt().unwrap_or_else(|failure| resolved(Err(failure))))
        }
        "sqlite_blob_close" => {
            let blob = java_long(int(&args[1])?);
            let state = connection(&args[0], "sqlite_blob_close", 2).map_err(as_error)?;
            Ok(Value::Bool(maintenance(&state, Box::new(move |engine| engine.blob_close(blob)))))
        }
        "sqlite_backup_open" => {
            let attempt = || -> Result<Value, Rc<Failure>> {
                let state = connection(&args[0], "sqlite_backup_open", 2)?;
                let path = path_or_sql(&args[1], true, "sqlite_backup_open", 2)?;
                Ok(request(&state, "backup_open", Box::new(move |engine| engine.backup_open(&path)), "sqlite_backup_open", 2))
            };
            Ok(attempt().unwrap_or_else(|failure| resolved(Err(failure))))
        }
        "sqlite_backup_step" => {
            let state = match connection(&args[0], "sqlite_backup_step", 3) {
                Ok(state) => state,
                Err(failure) => return Ok(resolved(Err(failure))),
            };
            let backup = java_long(int(&args[1])?);
            let pages = int(&args[2])?.to_i64().filter(|pages| i32::try_from(*pages).is_ok()).unwrap_or(-1);
            Ok(request(&state, "backup_step", Box::new(move |engine| engine.backup_step(backup, pages)), "sqlite_backup_step", 3))
        }
        "sqlite_backup_close" => {
            let backup = java_long(int(&args[1])?);
            let state = connection(&args[0], "sqlite_backup_close", 2).map_err(as_error)?;
            Ok(Value::Bool(maintenance(&state, Box::new(move |engine| engine.backup_close(backup)))))
        }
        "sqlite_script" | "sqlite_control" => {
            let label: &'static str = if name == "sqlite_script" { "sqlite_script" } else { "sqlite_control" };
            let attempt = || -> Result<Value, Rc<Failure>> {
                let state = connection(&args[0], label, 2)?;
                let sql = path_or_sql(&args[1], false, label, 2)?;
                let operation: Operation = if label == "sqlite_script" {
                    Box::new(move |engine| engine.script(&sql))
                } else {
                    Box::new(move |engine| {
                        let allowed = matches!(sql.as_str(), "BEGIN DEFERRED" | "BEGIN IMMEDIATE" | "BEGIN EXCLUSIVE" | "COMMIT" | "ROLLBACK");
                        if !allowed {
                            return Err(Stop::Unexpected);
                        }
                        engine.execution(&sql, &[], true)
                    })
                };
                Ok(request(&state, if label == "sqlite_script" { "script" } else { "control" }, operation, label, 2))
            };
            Ok(attempt().unwrap_or_else(|failure| resolved(Err(failure))))
        }
        "sqlite_fetch" => {
            let attempt = || -> Result<Value, Rc<Failure>> {
                let state = connection(&args[0], "sqlite_fetch", 3)?;
                let invalid = || fail("invalid_input", "sqlite_fetch", 3, "Invalid SQLite fetch coordinates");
                let id = int(&args[1]).map_err(|_| invalid())?;
                let maximum = int(&args[2]).map_err(|_| invalid())?;
                if id.signum() < 1 || maximum.signum() < 1 || maximum.to_i64().is_none_or(|maximum| maximum > 64) {
                    return Err(invalid());
                }
                let (cursor, count) = (java_long(id), maximum.to_i64().unwrap_or(1));
                Ok(request(&state, "fetch", Box::new(move |engine| engine.fetch(cursor, count)), "sqlite_fetch", 3))
            };
            Ok(attempt().unwrap_or_else(|failure| resolved(Err(failure))))
        }
        "sqlite_release" => {
            let cursor = java_long(int(&args[1])?);
            let state = connection(&args[0], "sqlite_release", 2).map_err(as_error)?;
            Ok(Value::Bool(maintenance(&state, Box::new(move |engine| engine.release(cursor).map(|()| Data::default())))))
        }
        "sqlite_rollback" => {
            let state = connection(&args[0], "sqlite_rollback", 1).map_err(as_error)?;
            Ok(Value::Bool(maintenance(&state, Box::new(|engine| engine.rollback()))))
        }
        "sqlite_close" => {
            let state = handle::<ConnectionRef>(&args[0])?.clone();
            let mut connection = state.borrow_mut();
            if connection.handle_closed || connection.stopped {
                return Ok(Value::Bool(true));
            }
            connection.stopped = true;
            if let Some(worker) = &connection.worker {
                let _ = worker.jobs.send(Box::new(|engine| {
                    // Закрытие при завершении: отказ уже не наблюдаем.
                    let _ = engine.close();
                }));
                // Очередь закрыта: поток выйдет после закрытия.
                let exited = worker.exited.clone();
                let (jobs, _) = mpsc::channel::<Job>();
                connection.worker = Some(Worker { jobs, exited });
            }
            Ok(Value::Bool(true))
        }
        "sqlite_closed" => {
            let state = handle::<ConnectionRef>(&args[0])?.borrow();
            Ok(Value::Bool(state.handle_closed || state.stopped || state.broken))
        }
        "sqlite_settled" => Ok(settled(handle::<ConnectionRef>(&args[0])?)),
        "sqlite_autocommit" => Ok(Value::Bool(connection(&args[0], "sqlite_autocommit", 1).map_err(as_error)?.borrow().autocommit)),
        "sqlite_reply_info" => {
            let reply = reply(&args[0], "sqlite_reply_info", 1)?;
            let data = &reply.data;
            let flag = |value: bool| if value { "1" } else { "0" }.to_string();
            Ok(map([
                ("ok".to_string(), text(flag(reply.ok))),
                ("code".to_string(), text(data.code.clone().unwrap_or_default())),
                ("kind".to_string(), text(data.kind.clone().unwrap_or_default())),
                ("affected_known".to_string(), text(flag(matches!(data.affected, Some(Some(_)))))),
                ("affected".to_string(), text(data.affected.clone().flatten().unwrap_or_else(|| "0".into()))),
                ("id".to_string(), text(data.id.unwrap_or(0).to_string())),
                ("eof".to_string(), text(flag(data.eof))),
                ("rows".to_string(), text(data.rows.as_ref().map_or(0, Vec::len).to_string())),
                ("autocommit".to_string(), text(flag(data.autocommit.unwrap_or(true)))),
                ("size".to_string(), text(data.size.unwrap_or(0).to_string())),
                ("done".to_string(), text(flag(data.done))),
                ("busy".to_string(), text(flag(data.busy))),
                ("copies".to_string(), text(data.copies.unwrap_or(0).to_string())),
                ("total".to_string(), text(data.total.unwrap_or(0).to_string())),
            ]))
        }
        "sqlite_reply_names" | "sqlite_reply_declarations" => {
            let reply = reply(&args[0], name, 1)?;
            let values = if !reply.ok {
                None
            } else if name == "sqlite_reply_names" {
                reply.data.names.clone()
            } else {
                reply.data.declarations.clone()
            };
            Ok(string_array(values.unwrap_or_default()))
        }
        "sqlite_reply_kind" => {
            let reply = reply(&args[0], "sqlite_reply_kind", 3)?;
            let code = match cell(reply, int(&args[1])?, int(&args[2])?, "sqlite_reply_kind")? {
                Cell::Null => 0,
                Cell::Integer(_) => 1,
                Cell::Floating(_) => 2,
                Cell::Text(_) => 3,
                Cell::Bytes(_) => 4,
            };
            Ok(Value::Int(IntValue::from_i64(code)))
        }
        "sqlite_reply_text" => {
            let reply = reply(&args[0], "sqlite_reply_text", 3)?;
            match cell(reply, int(&args[1])?, int(&args[2])?, "sqlite_reply_text")? {
                Cell::Bytes(_) => Err(as_error(fail("invalid_input", "sqlite_reply_text", 3, "SQLite cell is binary"))),
                Cell::Null => Ok(text("")),
                Cell::Integer(value) | Cell::Floating(value) | Cell::Text(value) => Ok(text(value.clone())),
            }
        }
        "sqlite_reply_bytes" => {
            let reply = reply(&args[0], "sqlite_reply_bytes", 3)?;
            match cell(reply, int(&args[1])?, int(&args[2])?, "sqlite_reply_bytes")? {
                Cell::Bytes(data) => Ok(byte_array(data.clone())),
                _ => Err(as_error(fail("invalid_input", "sqlite_reply_bytes", 3, "SQLite cell is not binary"))),
            }
        }
        "sqlite_reply_blob_bytes" => {
            let reply = reply(&args[0], "sqlite_reply_blob_bytes", 1)?;
            let data = if reply.ok { reply.data.bytes.clone().unwrap_or_default() } else { Vec::new() };
            Ok(byte_array(data))
        }
        "sqlite_reply_executions" => {
            let reply = reply(&args[0], "sqlite_reply_executions", 1)?;
            let entries = if reply.ok { reply.data.executions.clone().unwrap_or_default() } else { Vec::new() };
            if entries.len() > 256 {
                return Err(as_error(fail("invalid_data", "sqlite_reply_executions", 1, "SQLite script reply limit")));
            }
            Ok(string_array(entries.into_iter().map(|affected| affected.unwrap_or_else(|| "unknown".into()))))
        }
        "sqlite_reply_close" => {
            let reply = reply(&args[0], "sqlite_reply_close", 1)?;
            *reply.closed.borrow_mut() = true;
            Ok(Value::Bool(true))
        }
        _ => Err(Error::Unsupported(format!("native provider is not implemented: {}", native.symbol()))),
    }
}

