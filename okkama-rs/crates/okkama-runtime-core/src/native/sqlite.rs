//! `runtime/database` (provider `optional.sqlite`) — перенос `SqliteNatives`
//! JVM contract, backed by rusqlite and its bundled SQLite. Each connection
//! обслуживает свой рабочий поток; операции идут по очереди, их исход
//! приходит событием цикла. Ответ — `ok` и данные либо отказ `kind`/`code`.
#![allow(unsafe_code)]
// Адреса dlsym приводятся к типу поля таблицы функций, который и есть тип цели.
#![allow(clippy::missing_transmute_annotations)]

use super::support::{byte_array, bytes, handle, int, items, java_long, map, new_handle, string, string_array, text};
use super::Error;
use crate::failure::{Failure, Kind};
use crate::sched;
use crate::{IntValue, Value, abi::Native};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};

/// A statement and its streaming rows are confined to the connection worker.
/// Rows must be dropped before the boxed statement, and the statement before
/// its connection. The field order below is also their destruction order.
struct LibraryStatement {
    rows: Option<rusqlite::Rows<'static>>,
    statement: aliasable::boxed::AliasableBox<LibraryPrepared<'static>>,
    _connection: Rc<rusqlite::Connection>,
}

enum LibraryPrepared<'a> {
    Plain(rusqlite::Statement<'a>),
    Cached(rusqlite::CachedStatement<'a>),
}
impl std::ops::Deref for LibraryPrepared<'static> {
    type Target = rusqlite::Statement<'static>;
    fn deref(&self) -> &Self::Target {
        match self { Self::Plain(v) => v, Self::Cached(v) => v }
    }
}
impl std::ops::DerefMut for LibraryPrepared<'static> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self { Self::Plain(v) => v, Self::Cached(v) => v }
    }
}
impl LibraryStatement {
    fn new(connection: Rc<rusqlite::Connection>, sql: &str, cached: bool) -> rusqlite::Result<Self> {
        let statement = if cached {
            LibraryPrepared::Cached(connection.prepare_cached(sql)?)
        } else {
            LibraryPrepared::Plain(connection.prepare(sql)?)
        };
        // SAFETY: the connection is heap allocated and retained below. The
        // private wrapper never lets a statement escape that owner.
        let statement = unsafe { std::mem::transmute::<LibraryPrepared<'_>, LibraryPrepared<'static>>(statement) };
        Ok(Self { rows: None, statement: aliasable::boxed::AliasableBox::from_unique(Box::new(statement)), _connection: connection })
    }
    /// The statement must have been prepared on this exact connection.
    unsafe fn from_statement(connection: Rc<rusqlite::Connection>, statement: rusqlite::Statement<'_>) -> Self {
        // SAFETY: caller guarantees the retained owner is the statement's
        // connection; the same drop/address invariants as new apply.
        let statement = unsafe { std::mem::transmute::<rusqlite::Statement<'_>, rusqlite::Statement<'static>>(statement) };
        Self { rows: None, statement: aliasable::boxed::AliasableBox::from_unique(Box::new(LibraryPrepared::Plain(statement))), _connection: connection }
    }
    fn metadata(&self) -> &rusqlite::Statement<'_> {
        self.rows.as_ref().and_then(rusqlite::Rows::as_ref).unwrap_or(&self.statement)
    }
    fn reset(&mut self) {
        self.rows.take();
        self.statement.clear_bindings();
    }
    fn start_rows(&mut self) {
        debug_assert!(self.rows.is_none());
        let rows = self.statement.raw_query();
        // SAFETY: the aliasable allocation has a stable address and moving the
        // cursor cannot retag it as unique. It is retained until after rows are
        // dropped, and is not mutably accessed meanwhile.
        self.rows = Some(unsafe { std::mem::transmute::<rusqlite::Rows<'_>, rusqlite::Rows<'static>>(rows) });
    }
}

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
type Authorizer = extern "C" fn(*mut c_void, c_int, *const c_char, *const c_char, *const c_char, *const c_char) -> c_int;

#[allow(clippy::type_complexity)]
struct Api {
    libversion: unsafe extern "C" fn() -> c_int,
    open: unsafe extern "C" fn(*const c_char, *mut Db, c_int, *const c_char) -> c_int,
    close: unsafe extern "C" fn(Db) -> c_int,
    extended_codes: unsafe extern "C" fn(Db, c_int) -> c_int,
    extended_errcode: unsafe extern "C" fn(Db) -> c_int,
    autocommit: unsafe extern "C" fn(Db) -> c_int,
    changes: unsafe extern "C" fn(Db) -> i64,
    authorizer: unsafe extern "C" fn(Db, Option<Authorizer>, *mut c_void) -> c_int,
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

fn api() -> &'static Api {
    static API: OnceLock<Api> = OnceLock::new();
    API.get_or_init(|| {
        macro_rules! function {
            ($symbol:path) => {
                // SAFETY: these C ABI signatures differ only in opaque pointer
                // types (and SQLite's transient-destructor sentinel). All calls
                // use the same SQLite library as rusqlite, including extensions.
                unsafe { std::mem::transmute::<*const (), _>($symbol as *const ()) }
            };
        }
        Api {
            libversion: function!(rusqlite::ffi::sqlite3_libversion_number),
            open: function!(rusqlite::ffi::sqlite3_open_v2),
            close: function!(rusqlite::ffi::sqlite3_close),
            extended_codes: function!(rusqlite::ffi::sqlite3_extended_result_codes),
            extended_errcode: function!(rusqlite::ffi::sqlite3_extended_errcode),
            autocommit: function!(rusqlite::ffi::sqlite3_get_autocommit),
            changes: function!(rusqlite::ffi::sqlite3_changes64),
            authorizer: function!(rusqlite::ffi::sqlite3_set_authorizer),
            compile_option: function!(rusqlite::ffi::sqlite3_compileoption_used),
            limit: function!(rusqlite::ffi::sqlite3_limit),
            busy_timeout: function!(rusqlite::ffi::sqlite3_busy_timeout),
            load_extension: Some(function!(rusqlite::ffi::sqlite3_enable_load_extension)),
            blob_open: function!(rusqlite::ffi::sqlite3_blob_open),
            blob_bytes: function!(rusqlite::ffi::sqlite3_blob_bytes),
            blob_close: function!(rusqlite::ffi::sqlite3_blob_close),
            blob_read: function!(rusqlite::ffi::sqlite3_blob_read),
            blob_write: function!(rusqlite::ffi::sqlite3_blob_write),
            backup_init: function!(rusqlite::ffi::sqlite3_backup_init),
            backup_step: function!(rusqlite::ffi::sqlite3_backup_step),
            backup_finish: function!(rusqlite::ffi::sqlite3_backup_finish),
            backup_remaining: function!(rusqlite::ffi::sqlite3_backup_remaining),
            backup_pagecount: function!(rusqlite::ffi::sqlite3_backup_pagecount),
        }
    })
}

fn ready_api() -> &'static Api { api() }

fn c_bytes(pointer: *const c_char) -> Option<Vec<u8>> {
    // SAFETY: SQLite возвращает строку с завершающим нулём или NULL.
    (!pointer.is_null()).then(|| unsafe { CStr::from_ptr(pointer) }.to_bytes().to_vec())
}

fn strict(bytes: &[u8]) -> Option<String> {
    String::from_utf8(bytes.to_vec()).ok()
}

// ---- данные ответа ----

#[derive(Debug, Clone)]
enum Cell {
    Null,
    Integer(i64),
    Floating(f64),
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
    data: RefCell<Data>,
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
    library: Option<LibraryStatement>,
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
    library: Option<Rc<rusqlite::Connection>>,
    classes: HashMap<String, bool>,
    interrupt: Arc<std::sync::Mutex<Option<Arc<rusqlite::InterruptHandle>>>>,
    cancelled: Arc<AtomicBool>,
    options: Options,
    serial: i64,
    cursor: Option<Statement>,
    prepared: Vec<(i64, Statement)>,
    blobs: Vec<(i64, *mut c_void, c_int, bool)>,
    backup: Option<(*mut c_void, Db, i64)>,
    authorizer: Box<Authorization>,
}

#[derive(Default)]
struct Authorization {
    tail_guard: std::cell::Cell<bool>,
    control: std::cell::Cell<bool>,
    configuration: std::cell::Cell<bool>,
    dml_seen: std::cell::Cell<bool>,
    ddl_seen: std::cell::Cell<bool>,
}

extern "C" fn authorize(
    user: *mut c_void,
    action: c_int,
    a: *const c_char,
    _b: *const c_char,
    _database: *const c_char,
    trigger: *const c_char,
) -> c_int {
    // SAFETY: this separately allocated policy lives until the connection
    // closes. Reentrant authorization never aliases a mutable Engine borrow.
    let policy = unsafe { &*(user as *const Authorization) };
    if policy.tail_guard.get() {
        return 1;
    }
    if (action == 22 || action == 32) && !policy.control.get() {
        return 1;
    }
    if (action == 19 || action == 24 || action == 25) && !policy.configuration.get() {
        return 1;
    }
    if SCHEMA_ACTION.contains(&action) {
        policy.ddl_seen.set(true);
    }
    if action == 9 || action == 18 || action == 23 {
        if let Some(table) = c_bytes(a) {
            if table != b"sqlite_master" && table != b"sqlite_schema" && trigger.is_null() {
                policy.dml_seen.set(true);
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

    fn library_error(&self, error: rusqlite::Error) -> Stop {
        match error {
            rusqlite::Error::SqliteFailure(error, _) => self.error(error.extended_code),
            rusqlite::Error::MultipleStatement => self.raise("syntax", "SQLITE_MULTIPLE_STATEMENTS"),
            rusqlite::Error::Utf8Error(_) => self.raise("data", "SQLITE_UTF8"),
            _ => Stop::Unexpected,
        }
    }

    fn open(&mut self) -> Done<()> {
        let api = ready_api();
        // SAFETY: вызовы C API SQLite с действительными аргументами.
        unsafe {
            if (api.libversion)() < 3_041_000 {
                return Err(Stop::Sqlite { kind: "unsupported", code: "SQLITE_VERSION".into(), autocommit: true });
            }
            let flags = if self.options.read_only { 1 } else { 2 | if self.options.create { 4 } else { 0 } };
            let flags = rusqlite::OpenFlags::from_bits_truncate(flags);
            let library = rusqlite::Connection::open_with_flags(&self.options.path, flags)
                .map_err(|error| match error {
                    rusqlite::Error::SqliteFailure(error, _) => Stop::Sqlite {
                        kind: "connection", code: format!("SQLITE_{}", error.extended_code), autocommit: true,
                    },
                    _ => Stop::Unexpected,
                })?;
            library.set_prepared_statement_cache_capacity(64);
            self.db = library.handle().cast();
            *self.interrupt.lock().expect("SQLite interrupt lock") = Some(Arc::new(library.get_interrupt_handle()));
            self.library = Some(Rc::new(library));
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
            (api.authorizer)(self.db, Some(authorize), (&*self.authorizer as *const Authorization).cast_mut().cast());
        }
        self.authorizer.configuration.set(true);
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
        self.authorizer.configuration.set(false);
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
        if sql.len() > 1_048_576 || sql.contains('\0') {
            return Err(self.raise("data", "SQLITE_SQL_LIMIT"));
        }
        let cached = !self.authorizer.control.get() && !self.authorizer.configuration.get();
        let connection = self.library.as_ref().ok_or(Stop::Unexpected)?.clone();
        if cached && self.classes.len() >= 64 && !self.classes.contains_key(sql) {
            self.classes.clear();
            connection.flush_prepared_statement_cache();
        }
        self.authorizer.dml_seen.set(false);
        self.authorizer.ddl_seen.set(false);
        let library = LibraryStatement::new(connection, sql, cached).map_err(|error| self.library_error(error))?;
        if library.metadata().column_count() == 0 && library.metadata().expanded_sql().is_none() {
            return Err(self.raise("syntax", "SQLITE_EMPTY_STATEMENT"));
        }
        let dml = if cached {
            *self.classes.entry(sql.to_string()).or_insert_with(|| self.authorizer.dml_seen.get() && !self.authorizer.ddl_seen.get())
        } else {
            self.authorizer.dml_seen.get() && !self.authorizer.ddl_seen.get()
        };
        let columns = library.metadata().column_count() as c_int;
        Ok(Statement {
            library: Some(library), dml, columns,
            prepared: 0, id: 0, verified: false,
            raw_names: Vec::new(), raw_declarations: Vec::new(), pending_row: None,
        })
    }

    fn bind(&self, statement: &mut Statement, params: &[(&'static str, Param)]) -> Done<()> {
        if let Some(library) = &mut statement.library {
            use rusqlite::types::ValueRef;
            library.reset();
            if params.len() != library.statement.parameter_count() {
                return Err(self.raise("data", "SQLITE_BIND_COUNT"));
            }
            for (at, (kind, value)) in params.iter().enumerate() {
                let bound = match (*kind, value) {
                    ("null_value", _) => ValueRef::Null,
                    ("integer" | "boolean", Param::Integer(value)) => ValueRef::Integer(value.ok_or_else(|| self.raise("data", "SQLITE_INTEGER_RANGE"))?),
                    ("integer" | "boolean", Param::Text(value)) => ValueRef::Integer(self.int64(value)?),
                    ("floating", value) => {
                        let value = match value { Param::Floating(v) => *v, Param::Text(v) => super::jre::parse_double(v.trim()).unwrap_or(0.0), _ => return Err(Stop::Unexpected) };
                        if !value.is_finite() { return Err(self.raise("data", "SQLITE_FLOAT_RANGE")); }
                        ValueRef::Real(value)
                    }
                    ("text" | "decimal", Param::Text(value)) => {
                        if value.len() > 8_388_608 { return Err(self.raise("resource", "SQLITE_VALUE_LIMIT")); }
                        ValueRef::Text(value.as_bytes())
                    }
                    ("bytes", Param::Bytes(value)) => {
                        if value.len() > 8_388_608 { return Err(self.raise("resource", "SQLITE_VALUE_LIMIT")); }
                        ValueRef::Blob(value)
                    }
                    _ => return Err(self.raise("data", "SQLITE_VALUE_KIND")),
                };
                library.statement.raw_bind_parameter(at + 1, rusqlite::types::ToSqlOutput::Borrowed(bound)).map_err(|error| self.library_error(error))?;
            }
            return Ok(());
        }
        Err(Stop::Unexpected)
    }

    fn execution(&mut self, sql: &str, params: &[(&'static str, Param)], control: bool) -> Done<Data> {
        let previous = self.authorizer.control.get();
        self.authorizer.control.set(control);
        let result = self.prepare(sql).and_then(|statement| self.execute(statement, params));
        self.authorizer.control.set(previous);
        result
    }

    fn execute(&mut self, mut statement: Statement, params: &[(&'static str, Param)]) -> Done<Data> {
        let api = ready_api();
        let outcome = (|| -> Done<Data> {
            if statement.columns != 0 {
                return Err(self.raise("data", "SQLITE_ROWS_REQUIRED"));
            }
            self.bind(&mut statement, params)?;
            if let Some(library) = &mut statement.library {
                library.statement.raw_execute().map_err(|error| self.library_error(error))?;
            } else { return Err(Stop::Unexpected); }
            Ok(Data {
                affected: Some(statement.dml.then(|| unsafe { (api.changes)(self.db) }.to_string())),
                autocommit: Some(self.autocommit()),
                ..Data::default()
            })
        })();
        let finished = self.finish(&mut statement);
        if statement.prepared != 0 {
            self.restore(statement);
        }
        let data = outcome?;
        finished?;
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
        #[allow(clippy::type_complexity)] // четыре столбца метаданных
        let outcome = (|| -> Done<(Vec<String>, Vec<String>, Vec<String>, Vec<String>)> {
            // SAFETY: подготовленный оператор этого соединения.
            statement.columns = statement.library.as_ref().ok_or(Stop::Unexpected)?.metadata().column_count() as c_int;
            if statement.columns == 0 {
                return Err(self.raise("data", "SQLITE_EXECUTION_REQUIRED"));
            }
            self.bind(&mut statement, params)?;
            if let Some(library) = &mut statement.library {
                let mut names = Vec::new();
                let mut declarations = Vec::new();
                let mut used = 0usize;
                for column in library.metadata().columns() {
                    let name = column.name();
                    let declaration = column.decl_type().unwrap_or("");
                    used += name.len() + declaration.len();
                    if name.len() > 4096 || declaration.len() > 4096 || used > 1_048_576 {
                        return Err(self.raise("resource", "SQLITE_COLUMN_LIMIT"));
                    }
                    names.push(name.to_owned());declarations.push(declaration.to_owned());
                }
                library.start_rows();
                return Ok((names.clone(), declarations.clone(), names, declarations));
            }
            Err(Stop::Unexpected)
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
        if let Some(library) = &cursor.library {
            let columns = library.metadata().columns();
            if columns.len() != cursor.columns as usize || columns.iter().enumerate().any(|(index, column)| {
                column.name() != cursor.raw_names[index] || column.decl_type().unwrap_or("") != cursor.raw_declarations[index]
            }) {
                return Err(self.raise("data", "SQLITE_SCHEMA_CHANGED"));
            }
            cursor.verified = true;
            return Ok(());
        }
        Err(Stop::Unexpected)
    }

    fn library_row(&self, row: &rusqlite::Row<'_>, cursor: &Statement) -> Done<(Vec<Cell>, usize)> {
        use rusqlite::types::ValueRef;
        if !cursor.verified {
            let columns = row.as_ref().columns();
            if columns.len() != cursor.columns as usize || columns.iter().enumerate().any(|(index, column)| {
                column.name() != cursor.raw_names[index] || column.decl_type().unwrap_or("") != cursor.raw_declarations[index]
            }) { return Err(self.raise("data", "SQLITE_SCHEMA_CHANGED")); }
        }
        let mut values = Vec::with_capacity(cursor.columns as usize);
        let mut used = 0usize;
        for index in 0..cursor.columns as usize {
            let value = row.get_ref(index).map_err(|error| self.library_error(error))?;
            let (width, cell) = match value {
                ValueRef::Null => (0, Cell::Null),
                ValueRef::Integer(value) => (value.to_string().len(), Cell::Integer(value)),
                ValueRef::Real(value) => {
                    if !value.is_finite() { return Err(self.raise("data", "SQLITE_FLOAT_RANGE")); }
                    (crate::float::format_g(value, 17).len(), Cell::Floating(value))
                }
                ValueRef::Text(value) | ValueRef::Blob(value) => {
                    if value.len() > 8_388_608 - used.min(8_388_608) {
                        return Err(self.raise("resource", "SQLITE_ROW_LIMIT"));
                    }
                    let cell = if matches!(row.get_ref(index), Ok(ValueRef::Text(_))) {
                        Cell::Text(std::str::from_utf8(value).map_err(|_| self.raise("data", "SQLITE_UTF8"))?.to_owned())
                    } else { Cell::Bytes(value.to_vec()) };
                    (value.len(), cell)
                }
            };
            used += width;
            if used > 8_388_608 { return Err(self.raise("resource", "SQLITE_ROW_LIMIT")); }
            values.push(cell);
        }
        Ok((values, used))
    }

    fn fetch(&mut self, id: i64, maximum: i64) -> Done<Data> {
        if self.cursor.as_ref().is_none_or(|cursor| cursor.id != id) {
            return Err(self.raise("data", "SQLITE_READER_STALE"));
        }
        if !(1..=64).contains(&maximum) {
            return Err(self.raise("data", "SQLITE_BATCH_LIMIT"));
        }
        let mut cursor = self.cursor.take().expect("cursor");
        let mut rows = Vec::with_capacity(maximum as usize);
        let mut used = 0usize;
        let mut eof = false;
        if let Some((values, bytes)) = cursor.pending_row.take() { rows.push(values);used = bytes; }
        // Take the library handle separately so a Row borrow cannot alias the
        // cursor metadata while its lifetime is checked and values are copied.
        let mut library = cursor.library.take().ok_or(Stop::Unexpected)?;
        let outcome = (|| -> Done<()> {
            while rows.len() < maximum as usize {
                let found = library.rows.as_mut().ok_or(Stop::Unexpected)?.next().map_err(|error| self.library_error(error))?;
                let Some(row) = found else { eof = true;break; };
                let (values, width) = self.library_row(row, &cursor)?;
                cursor.verified = true;
                if used + width > 8_388_608 { cursor.pending_row = Some((values, width));break; }
                used += width;rows.push(values);
            }
            Ok(())
        })();
        cursor.library = Some(library);
        let outcome = outcome.and_then(|()| if eof { self.verify(&mut cursor) } else { Ok(()) });
        self.cursor = Some(cursor);
        if let Err(error) = outcome { self.release(id)?;return Err(error); }
        if eof { self.release(id)?; }
        Ok(Data { rows: Some(rows), eof, autocommit: Some(self.autocommit()), ..Data::default() })
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
        statement.pending_row = None;
        if let Some(library) = &mut statement.library {
            library.reset();
            if statement.prepared == 0 { statement.library.take(); }
            return Ok(());
        }
        Ok(())
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
            library: slot.1.library.take(),
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
        let (_, mut statement) = self.prepared.remove(position);
        if let Some(cursor_id) = self.cursor.as_ref().filter(|cursor| cursor.prepared == id).map(|cursor| cursor.id) {
            self.release(cursor_id)?;
            // release вернул бы его в таблицу: он уже удалён.
            self.prepared.retain(|(known, _)| *known != id);
        }
        statement.library.take();
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
        use fallible_iterator::FallibleIterator;
        self.ready()?;
        if sql.len() > 1_048_576 || sql.contains('\0') {
            return Err(self.raise("data", "SQLITE_SQL_LIMIT"));
        }
        let connection = self.library.as_ref().ok_or(Stop::Unexpected)?.clone();
        let mut batch = rusqlite::Batch::new(&connection, sql);
        let mut results = Vec::new();
        loop {
            self.authorizer.dml_seen.set(false);
            self.authorizer.ddl_seen.set(false);
            let Some(statement) = batch.next().map_err(|error| self.library_error(error))? else { break; };
            if results.len() >= 256 { return Err(self.raise("resource", "SQLITE_SCRIPT_COUNT")); }
            let columns = statement.column_count() as c_int;
            // SAFETY: Batch was constructed on this same retained connection.
            let library = unsafe { LibraryStatement::from_statement(connection.clone(), statement) };
            let statement = Statement {
                library: Some(library),
                dml: self.authorizer.dml_seen.get() && !self.authorizer.ddl_seen.get(),
                columns, prepared: 0, id: 0, verified: false,
                raw_names: Vec::new(), raw_declarations: Vec::new(), pending_row: None,
            };
            results.push(self.execute(statement, &[])?.affected.flatten());
        }
        Ok(Data { executions: Some(results), autocommit: Some(self.autocommit()), ..Data::default() })
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
        self.db = std::ptr::null_mut();
        self.interrupt.lock().expect("SQLite interrupt lock").take();
        self.classes.clear();
        if let Some(library) = self.library.take() {
            let library = Rc::try_unwrap(library).map_err(|_| Stop::Unexpected)?;
            library.close().map_err(|(_, error)| self.library_error(error))?;
        }
        Ok(())
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // Queue loss and unwinding must release cursors/BLOBs/backups too.
        let _ = self.close();
    }
}

fn first_cell(reply: &Data) -> String {
    match reply.rows.as_ref().and_then(|rows| rows.first()).and_then(|row| row.first()) {
        Some(Cell::Integer(value)) => value.to_string(),
        Some(Cell::Floating(value)) => crate::float::format_g(*value, 17),
        Some(Cell::Text(value)) => value.clone(),
        Some(Cell::Null) | None => String::new(),
        Some(Cell::Bytes(bytes)) => format!("{bytes:?}"),
    }
}

#[derive(Debug, Clone)]
enum Param {
    Integer(Option<i64>),
    Floating(f64),
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
    #[cfg(feature = "sqlite-inline-bench")]
    inline_engine: Option<Rc<RefCell<Box<Engine>>>>,
    interrupt: Arc<std::sync::Mutex<Option<Arc<rusqlite::InterruptHandle>>>>,
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
        library: None,
        classes: HashMap::new(),
        interrupt: connection.interrupt.clone(),
        cancelled: connection.cancelled.clone(),
        options: connection.options.clone(),
        serial: 0,
        cursor: None,
        prepared: Vec::new(),
        blobs: Vec::new(),
        backup: None,
        authorizer: Box::default(),
    }));
    #[cfg(feature = "sqlite-inline-bench")]
    if std::env::var("OKKAMA_SQLITE_INLINE_BENCH").as_deref() == Ok("1") {
        // Same engine and representations, but no worker or external completion.
        connection.inline_engine = Some(Rc::new(RefCell::new(engine.0)));
        return;
    }
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
    Reply { ok, data: RefCell::new(data), closed: RefCell::new(false) }
}

/// Reply пересекает границу потока до того, как его увидит программа.
struct SendReply(Reply);
unsafe impl Send for SendReply {}

fn facts(connection: &ConnectionRef, reply: &Reply) {
    if let Some(autocommit) = reply.data.borrow().autocommit {
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
        #[cfg(feature = "sqlite-inline-bench")]
        if let Some(engine) = connection.inline_engine.clone() {
            drop(connection);
            let reply = perform(&mut engine.borrow_mut(), operation);
            facts(state, &reply);
            return resolved(Ok(new_handle(Rc::new(reply))));
        }
        let Some(worker) = &connection.worker else {
            return resolved(Err(fail("invalid_input", name, arity, "SQLite connection is not acquired")));
        };
        worker.jobs.clone()
    };
    let promise = sched::promise();
    let target = promise.clone();
    let delivered = state.clone();
    let (pending, completion) = sched::external(
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
    let (interrupt, cancelled) = {
        let connection = state.borrow();
        (connection.interrupt.clone(), connection.cancelled.clone())
    };
    promise.listen(move || {
        pending.detach();
        if watched.cancelled() {
            cancelled.store(true, Ordering::SeqCst);
            let handle = interrupt.lock().expect("SQLite interrupt lock").clone();
            if let Some(handle) = handle {
                // rusqlite synchronizes interruption with connection closure.
                handle.interrupt();
            }
        }
    });
    let job: Job = Box::new(move |engine| completion.deliver(SendReply(perform(engine, operation))));
    if jobs.send(job).is_err() {
        sched::settle(&promise, Err(fail("already_closed", name, arity, "SQLite connection is closed")));
    }
    Value::Future(promise)
}

/// Отменённая операция всё же удалась: освободить её ресурс (как Perl).
fn cleanup_cancelled(state: &ConnectionRef, op: &str, reply: &Reply) {
    let Some(id) = reply.data.borrow().id else {
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
        #[cfg(feature = "sqlite-inline-bench")]
        if let Some(engine) = connection.inline_engine.clone() {
            drop(connection);
            let reply = perform(&mut engine.borrow_mut(), operation);
            if reply.ok { facts(state, &reply); }
            else { state.borrow_mut().broken = true; }
            return true;
        }
        let Some(worker) = &connection.worker else {
            return true;
        };
        worker.jobs.clone()
    };
    let delivered = state.clone();
    let (op, completion) = sched::external(
        move |reply: Option<SendReply>| match reply {
            Some(SendReply(reply)) if reply.ok => facts(&delivered, &reply),
            _ => delivered.borrow_mut().broken = true,
        },
    );
    // Служебная операция не держит программу.
    op.detach();
    let job: Job = Box::new(move |engine| completion.deliver(SendReply(perform(engine, operation))));
    if jobs.send(job).is_err() { state.borrow_mut().broken = true; }
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

/// Copy only owned typed payloads into the worker; no program Rc crosses threads.
fn cell_params(value: &Value, name: &str) -> Result<Vec<(&'static str, Param)>, Rc<Failure>> {
    use super::db::Cell as DbCell;
    let invalid = || fail("invalid_input", name, 3, "Invalid SQLite bindings");
    let cells = items(value).map_err(|_| invalid())?;
    if cells.len() > 1024 {
        return Err(invalid());
    }
    let mut used = 0usize;
    let mut params = Vec::with_capacity(cells.len());
    for value in cells {
        let cell = handle::<DbCell>(value).map_err(|_| invalid())?;
        let (width, payload) = match cell {
            DbCell::Null => (0, Param::Text(String::new())),
            DbCell::Boolean(value) => (1, Param::Integer(Some(i64::from(*value)))),
            DbCell::Integer(value) => (value.to_string().len(), Param::Integer(value.to_i64())),
            DbCell::Floating(value) => (crate::float::text(*value).len(), Param::Floating(*value)),
            DbCell::Decimal(value) | DbCell::Text(value) => (value.len(), Param::Text(value.to_string())),
            DbCell::Bytes(value) => (value.len(), Param::Bytes(value.to_vec())),
        };
        used += width;
        if used > 8_388_608 {
            return Err(fail("resource_exhausted", name, 3, "SQLite binding byte budget"));
        }
        params.push((KINDS[cell.kind() as usize], payload));
    }
    Ok(params)
}

fn typed_statement(args: &[Value], rows: bool, prepared: bool, name: &'static str) -> Value {
    let attempt = || -> Result<Value, Rc<Failure>> {
        let state = connection(&args[0], name, 3)?;
        let params = cell_params(&args[2], name)?;
        let operation: Operation = if prepared {
            let id = int(&args[1]).map_err(|_| fail("invalid_input", name, 3, "Invalid SQLite statement"))?;
            if id.signum() < 1 {
                return Err(fail("invalid_input", name, 3, "Invalid SQLite statement"));
            }
            let id = java_long(id);
            Box::new(move |engine| {
                let statement = engine.take_prepared(id)?;
                if rows { engine.rows(statement, &params) } else { engine.execute(statement, &params) }
            })
        } else {
            let sql = string(&args[1]).map_err(|_| fail("invalid_input", name, 3, "Invalid SQLite SQL"))?.to_string();
            if sql.is_empty() || sql.contains('\0') || sql.len() > 1_048_576 {
                return Err(fail("invalid_input", name, 3, "Invalid SQLite SQL"));
            }
            Box::new(move |engine| {
                if rows { engine.rows_sql(&sql, &params) } else { engine.execution(&sql, &params, false) }
            })
        };
        let op = match (prepared, rows) {
            (false, false) => "execution", (false, true) => "rows",
            (true, false) => "prepared_execution", (true, true) => "prepared_rows",
        };
        Ok(request(&state, op, operation, name, 3))
    };
    attempt().unwrap_or_else(|failure| resolved(Err(failure)))
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

fn cell<'a>(reply: &'a Reply, row: &IntValue, column: &IntValue, name: &str) -> Result<std::cell::Ref<'a, Cell>, Error> {
    let data = reply.data.borrow();
    let rows: &[Vec<Cell>] = if reply.ok { data.rows.as_deref().unwrap_or(&[]) } else { &[] };
    let outside = || as_error(fail("invalid_input", name, 3, "SQLite reply cell outside bounds"));
    let row = row.to_i64().filter(|row| *row >= 0 && (*row as usize) < rows.len()).ok_or_else(outside)? as usize;
    let column = column.to_i64().filter(|column| *column >= 0 && (*column as usize) < rows[row].len()).ok_or_else(outside)? as usize;
    Ok(std::cell::Ref::map(data, |data| &data.rows.as_ref().unwrap()[row][column]))
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
    if !stopped {
        // A FIFO barrier on an existing worker needs no blocking relay thread.
        // Keep this delivery attached even if the caller cancels its await:
        // settled is the cleanup boundary for previously queued operations.
        let (_pending, completion) = sched::external(move |_: Option<()>| {
            if delivered.borrow().stopped {
                delivered.borrow_mut().handle_closed = true;
            }
            sched::settle(&target, Ok(Value::Bool(true)));
        });
        let _ = jobs.send(Box::new(move |_engine| completion.deliver(())));
        return Value::Future(promise);
    }
    drop(jobs);
    sched::offload(
        move || {
            let (lock, changed) = &*exited;
            if let Ok(done) = lock.lock() {
                let _ = changed.wait_timeout_while(done, std::time::Duration::from_secs(2), |done| !*done);
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
                #[cfg(feature = "sqlite-inline-bench")]
                inline_engine: None,
                interrupt: Arc::new(std::sync::Mutex::new(None)),
                cancelled: Arc::new(AtomicBool::new(false)),
            }))))
        }
        "sqlite_connect" => {
            let state = match connection(&args[0], "sqlite_connect", 1) {
                Ok(state) => state,
                Err(failure) => return Ok(resolved(Err(failure))),
            };
            #[cfg(feature = "sqlite-inline-bench")]
            if state.borrow().inline_engine.is_some() {
                return Ok(resolved(Err(fail("invalid_input", "sqlite_connect", 1, "SQLite connection already acquired"))));
            }
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
        "sqlite_execution_cells" => Ok(typed_statement(args, false, false, "sqlite_execution_cells")),
        "sqlite_rows_cells" => Ok(typed_statement(args, true, false, "sqlite_rows_cells")),
        "sqlite_prepared_execution_cells" => Ok(typed_statement(args, false, true, "sqlite_prepared_execution_cells")),
        "sqlite_prepared_rows_cells" => Ok(typed_statement(args, true, true, "sqlite_prepared_rows_cells")),
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
            #[cfg(feature = "sqlite-inline-bench")]
            if let Some(engine) = connection.inline_engine.take() {
                let _ = engine.borrow_mut().close();
            }
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
        "sqlite_reply_ok" => Ok(Value::Bool(reply(&args[0], name, 1)?.ok)),
        // A batch as one page of flat columns (see `db/read.DbPage`): cell kinds,
        // integer values or ordinals into the typed arrays, and the widest row.
        "sqlite_reply_kinds" | "sqlite_reply_values" | "sqlite_reply_floats" | "sqlite_reply_texts"
        | "sqlite_reply_blobs" | "sqlite_reply_row_bytes" => {
            let reply = reply(&args[0], name, 1)?;
            let data = reply.data.borrow();
            let rows: &[Vec<Cell>] = if reply.ok { data.rows.as_deref().unwrap_or(&[]) } else { &[] };
            let cells = rows.iter().flatten();
            let int = |n: i64| Value::Int(IntValue::from_i64(n));
            Ok(match name {
                "sqlite_reply_kinds" => super::support::array(
                    cells
                        .map(|cell| {
                            int(match cell {
                                Cell::Null => 0,
                                Cell::Integer(_) => 2,
                                Cell::Floating(_) => 3,
                                Cell::Text(_) => 5,
                                Cell::Bytes(_) => 6,
                            })
                        })
                        .collect(),
                ),
                "sqlite_reply_values" => {
                    let (mut floats, mut texts, mut blobs) = (0, 0, 0);
                    let ordinal = |n: &mut i64| {
                        *n += 1;
                        *n - 1
                    };
                    super::support::array(
                        cells
                            .map(|cell| {
                                int(match cell {
                                    Cell::Null => 0,
                                    Cell::Integer(v) => *v,
                                    Cell::Floating(_) => ordinal(&mut floats),
                                    Cell::Text(_) => ordinal(&mut texts),
                                    Cell::Bytes(_) => ordinal(&mut blobs),
                                })
                            })
                            .collect(),
                    )
                }
                "sqlite_reply_floats" => super::support::array(
                    cells.filter_map(|cell| if let Cell::Floating(v) = cell { Some(Value::Float(*v)) } else { None }).collect(),
                ),
                "sqlite_reply_texts" => super::support::array(
                    cells.filter_map(|cell| if let Cell::Text(v) = cell { Some(text(v.as_str())) } else { None }).collect(),
                ),
                "sqlite_reply_blobs" => super::support::array(
                    cells.filter_map(|cell| if let Cell::Bytes(v) = cell { Some(byte_array(v.clone())) } else { None }).collect(),
                ),
                _ => {
                    // The widest row in the bytes `db_cells_bytes` counts.
                    let widest = rows
                        .iter()
                        .map(|row| {
                            row.iter()
                                .map(|cell| match cell {
                                    Cell::Null => 0,
                                    Cell::Integer(v) => v.to_string().len(),
                                    Cell::Floating(v) => crate::float::text(*v).len(),
                                    Cell::Text(v) => v.len(),
                                    Cell::Bytes(v) => v.len(),
                                })
                                .sum::<usize>()
                        })
                        .max()
                        .unwrap_or(0);
                    int(widest as i64)
                }
            })
        }
        // No first page is fetched with the cursor here, so no failure waits.
        "sqlite_reply_deferred" => {
            reply(&args[0], name, 1)?;
            Ok(super::support::array(Vec::new()))
        }
        "sqlite_reply_id" => Ok(Value::Int(IntValue::from_i64(reply(&args[0], name, 1)?.data.borrow().id.unwrap_or(0)))),
        "sqlite_reply_eof" => Ok(Value::Bool(reply(&args[0], name, 1)?.data.borrow().eof)),
        "sqlite_reply_count" => Ok(Value::Int(IntValue::from_u128(reply(&args[0], name, 1)?.data.borrow().rows.as_ref().map_or(0, Vec::len) as u128))),
        "sqlite_reply_affected_known" => Ok(Value::Bool(matches!(reply(&args[0], name, 1)?.data.borrow().affected, Some(Some(_))))),
        "sqlite_reply_affected" => Ok(text(reply(&args[0], name, 1)?.data.borrow().affected.clone().flatten().unwrap_or_else(|| "0".into()))),
        "sqlite_reply_info" => {
            let reply = reply(&args[0], "sqlite_reply_info", 1)?;
            let data = reply.data.borrow();
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
                reply.data.borrow().names.clone()
            } else {
                reply.data.borrow().declarations.clone()
            };
            Ok(string_array(values.unwrap_or_default()))
        }
        "sqlite_reply_cells" => {
            let reply = reply(&args[0], name, 2)?;
            let data = reply.data.borrow();
            let rows = data.rows.as_ref().filter(|_| reply.ok)
                .ok_or_else(|| as_error(fail("invalid_input", name, 2, "missing SQLite rows")))?;
            let at = int(&args[1])?.to_i64().and_then(|i| usize::try_from(i).ok())
                .filter(|i| *i < rows.len())
                .ok_or_else(|| as_error(fail("invalid_input", name, 2, "SQLite row outside bounds")))?;
            use super::db::{Cell as DbCell, cell_value};
            Ok(super::support::array(rows[at].iter().map(|c| cell_value(match c {
                Cell::Null => DbCell::Null,
                Cell::Integer(v) => DbCell::Integer(IntValue::from_i64(*v)),
                Cell::Floating(v) => DbCell::Floating(*v),
                Cell::Text(v) => DbCell::Text(Rc::from(v.as_str())),
                Cell::Bytes(v) => DbCell::Bytes(Rc::from(v.as_slice())),
            })).collect()))
        }
        "sqlite_reply_kind" => {
            let reply = reply(&args[0], "sqlite_reply_kind", 3)?;
            let code = match &*cell(reply, int(&args[1])?, int(&args[2])?, "sqlite_reply_kind")? {
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
            match &*cell(reply, int(&args[1])?, int(&args[2])?, "sqlite_reply_text")? {
                Cell::Bytes(_) => Err(as_error(fail("invalid_input", "sqlite_reply_text", 3, "SQLite cell is binary"))),
                Cell::Null => Ok(text("")),
                Cell::Integer(value) => Ok(text(value.to_string())),
                Cell::Floating(value) => Ok(text(crate::float::format_g(*value, 17))),
                Cell::Text(value) => Ok(text(value.clone())),
            }
        }
        "sqlite_reply_bytes" => {
            let reply = reply(&args[0], "sqlite_reply_bytes", 3)?;
            match &*cell(reply, int(&args[1])?, int(&args[2])?, "sqlite_reply_bytes")? {
                Cell::Bytes(data) => Ok(byte_array(data.clone())),
                _ => Err(as_error(fail("invalid_input", "sqlite_reply_bytes", 3, "SQLite cell is not binary"))),
            }
        }
        "sqlite_reply_blob_bytes" => {
            let reply = reply(&args[0], "sqlite_reply_blob_bytes", 1)?;
            let data = if reply.ok { reply.data.borrow().bytes.clone().unwrap_or_default() } else { Vec::new() };
            Ok(byte_array(data))
        }
        "sqlite_reply_executions" => {
            let reply = reply(&args[0], "sqlite_reply_executions", 1)?;
            let entries = if reply.ok { reply.data.borrow().executions.clone().unwrap_or_default() } else { Vec::new() };
            if entries.len() > 256 {
                return Err(as_error(fail("invalid_data", "sqlite_reply_executions", 1, "SQLite script reply limit")));
            }
            Ok(string_array(entries.into_iter().map(|affected| affected.unwrap_or_else(|| "unknown".into()))))
        }
        "sqlite_reply_close" => {
            let reply = reply(&args[0], "sqlite_reply_close", 1)?;
            *reply.closed.borrow_mut() = true;
            // Future outcomes can outlive the handle; closing releases their payload.
            *reply.data.borrow_mut() = Data::default();
            Ok(Value::Bool(true))
        }
        _ => Err(Error::Unsupported(format!("native provider is not implemented: {}", native.symbol()))),
    }
}


#[cfg(test)]
mod reply_lifetime_tests {
    use super::*;

    #[test]
    fn close_releases_payload_even_when_future_retains_reply() {
        sched::reset();
        let reply = Rc::new(Reply {
            ok: true,
            data: RefCell::new(Data {
                rows: Some(vec![vec![Cell::Text("saved".into()), Cell::Bytes(vec![7; 1024])]]),
                names: Some(vec!["text".into(), "blob".into()]),
                declarations: Some(vec!["TEXT".into(), "BLOB".into()]),
                bytes: Some(vec![9; 1024]),
                executions: Some(vec![Some("1".into())]),
                ..Data::default()
            }),
            closed: RefCell::new(false),
        });
        let value = new_handle(reply.clone());
        let future = sched::resolved(Ok(value.clone()));
        sched::register(&future);
        let cells = execute(Native::DatabaseSqliteReplyCells, &[value.clone(), Value::Int(IntValue::from_i64(0))]).unwrap();
        execute(Native::DatabaseSqliteReplyClose, std::slice::from_ref(&value)).unwrap();
        // The outcome is still cached, but its closed native resource owns no payload.
        let again = sched::await_future(&future).unwrap();
        assert!(Rc::ptr_eq(handle::<Rc<Reply>>(&again).unwrap(), &reply));
        let data = reply.data.borrow();
        assert!(data.rows.is_none() && data.bytes.is_none() && data.names.is_none());
        assert!(data.declarations.is_none() && data.executions.is_none());
        drop(data);
        assert!(execute(Native::DatabaseSqliteReplyCount, std::slice::from_ref(&again)).is_err());
        assert!(execute(Native::DatabaseSqliteReplyClose, &[again]).is_err());
        // Public row cells are owned snapshots, independent of the closed reply.
        let saved = items(&cells).unwrap();
        let text = super::super::db::execute(Native::DbTextValue, &[saved[0].clone()]).unwrap();
        assert_eq!(text.as_string().unwrap(), "saved");
        let blob = super::super::db::execute(Native::DbBytesValue, &[saved[1].clone()]).unwrap();
        assert_eq!(bytes(&blob).unwrap(), vec![7; 1024]);
        sched::reset();
    }
}

#[cfg(test)]
mod interrupt_lifetime_tests {
    use super::*;
    use std::sync::Barrier;

    fn engine(interrupt: &Arc<std::sync::Mutex<Option<Arc<rusqlite::InterruptHandle>>>>) -> Engine {
        Engine {
            db: std::ptr::null_mut(),
            library: None,
            classes: HashMap::new(),
            interrupt: interrupt.clone(),
            cancelled: Arc::new(AtomicBool::new(false)),
            options: Options {
                path: ":memory:".into(),
                read_only: false,
                create: true,
                foreign_keys: false,
                busy_ms: 0,
                journal: "memory".into(),
            },
            serial: 0,
            cursor: None,
            prepared: Vec::new(),
            blobs: Vec::new(),
            backup: None,
            authorizer: Box::default(),
        }
    }

    /// The cancel listener loads the handle, then the worker closes the
    /// connection before `interrupt` runs: the handle must not reach the
    /// freed `sqlite3*` (audit S04).
    #[test]
    fn a_handle_loaded_before_close_never_touches_the_closed_connection() {
        let slot = Arc::new(std::sync::Mutex::new(None));
        let mut engine = engine(&slot);
        engine.open().map_err(|_| "open").unwrap();
        let loaded = slot.lock().unwrap().clone().expect("open publishes the handle");
        engine.close().map_err(|_| "close").unwrap();
        assert!(slot.lock().unwrap().is_none(), "close withdraws the handle");
        loaded.interrupt();
        drop(engine);
        loaded.interrupt();
    }

    /// Interrupts race close and worker exit (drop) from another thread.
    #[test]
    fn interrupt_races_close_and_worker_exit() {
        for round in 0..64 {
            let slot = Arc::new(std::sync::Mutex::new(None));
            let mut engine = engine(&slot);
            engine.open().map_err(|_| "open").unwrap();
            let loaded = slot.lock().unwrap().clone().expect("handle");
            let barrier = Arc::new(Barrier::new(2));
            let started = barrier.clone();
            let canceller = std::thread::spawn(move || {
                started.wait();
                for _ in 0..256 {
                    loaded.interrupt();
                }
            });
            barrier.wait();
            if round % 2 == 0 {
                engine.close().map_err(|_| "close").unwrap();
            }
            drop(engine);
            canceller.join().unwrap();
        }
    }
}
