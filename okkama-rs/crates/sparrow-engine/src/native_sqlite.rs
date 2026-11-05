//! `runtime/database` SQLite provider on rusqlite with bundled SQLite. Each
//! connection owns one worker thread that creates and exclusively uses its
//! Engine; requests cross as owned Send data and replies return through Tokio
//! oneshot channels into native I/O Futures. Rows reach the program as DB cells
//! (Rust data), never as intermediate VM objects.
//!
//! The second unsafe boundary of the engine (after jit.rs): a cursor borrows its
//! statement, and the authorizer, BLOB and backup use the SQLite C API directly,
//! following the reference provider.
#![allow(unsafe_code)]

use crate::{
    Error,
    handle::Handle,
    native::{Outcome, ProviderError},
    native_io::{CancelHook, Output},
    value::*,
};
use num_traits::ToPrimitive;
use okkama_runtime_abi::{Native, NativeFailure};
use rusqlite::ffi;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

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
        match self {
            Self::Plain(v) => v,
            Self::Cached(v) => v,
        }
    }
}
impl std::ops::DerefMut for LibraryPrepared<'static> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Plain(v) => v,
            Self::Cached(v) => v,
        }
    }
}
impl LibraryStatement {
    fn new(
        connection: Rc<rusqlite::Connection>,
        sql: &str,
        cached: bool,
    ) -> rusqlite::Result<Self> {
        let statement = if cached {
            LibraryPrepared::Cached(connection.prepare_cached(sql)?)
        } else {
            LibraryPrepared::Plain(connection.prepare(sql)?)
        };
        // SAFETY: the connection is heap allocated and retained below. The
        // private wrapper never lets a statement escape that owner.
        let statement = unsafe {
            std::mem::transmute::<LibraryPrepared<'_>, LibraryPrepared<'static>>(statement)
        };
        Ok(Self {
            rows: None,
            statement: aliasable::boxed::AliasableBox::from_unique(Box::new(statement)),
            _connection: connection,
        })
    }
    /// The statement must have been prepared on this exact connection.
    unsafe fn from_statement(
        connection: Rc<rusqlite::Connection>,
        statement: rusqlite::Statement<'_>,
    ) -> Self {
        // SAFETY: caller guarantees the retained owner is the statement's
        // connection; the same drop/address invariants as new apply.
        let statement = unsafe {
            std::mem::transmute::<rusqlite::Statement<'_>, rusqlite::Statement<'static>>(statement)
        };
        Self {
            rows: None,
            statement: aliasable::boxed::AliasableBox::from_unique(Box::new(
                LibraryPrepared::Plain(statement),
            )),
            _connection: connection,
        }
    }
    fn metadata(&self) -> &rusqlite::Statement<'_> {
        self.rows
            .as_ref()
            .and_then(rusqlite::Rows::as_ref)
            .unwrap_or(&self.statement)
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
        self.rows = Some(unsafe {
            std::mem::transmute::<rusqlite::Rows<'_>, rusqlite::Rows<'static>>(rows)
        });
    }
}

const DOMAIN: &str = "optional.sqlite";

fn fail(n: Native, code: &str, message: &str) -> NativeFailure {
    NativeFailure {
        domain: DOMAIN.into(),
        code: code.into(),
        operation: n.symbol().into(),
        message: message.into(),
    }
}

fn c_bytes(pointer: *const c_char) -> Option<Vec<u8>> {
    // SAFETY: SQLite passes a NUL-terminated string or NULL.
    (!pointer.is_null()).then(|| unsafe { CStr::from_ptr(pointer) }.to_bytes().to_vec())
}

fn strict(bytes: &[u8]) -> Option<String> {
    String::from_utf8(bytes.to_vec()).ok()
}

fn float_width(value: f64) -> usize {
    crate::scalar::float_text(value).len()
}

// ---- reply data ----

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
    /// `Some(None)`: the key is present, the count is unknown (not DML).
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
pub(crate) struct Reply {
    ok: bool,
    data: RefCell<Data>,
    closed: std::cell::Cell<bool>,
}

/// Engine refusal: kind, SQLITE_… code and the autocommit state.
#[derive(Debug)]
enum Stop {
    Sqlite {
        kind: &'static str,
        code: String,
        autocommit: bool,
    },
    Unexpected,
}

type Done<T> = Result<T, Stop>;

// ---- engine ----

const SCHEMA_ACTION: [c_int; 21] = [
    1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 15, 16, 17, 26, 27, 28, 29, 30,
];

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

#[derive(Debug, Clone)]
struct Options {
    path: String,
    read_only: bool,
    create: bool,
    foreign_keys: bool,
    busy_ms: c_int,
    journal: String,
}

/// Facts the worker publishes after each operation; read by the program thread.
#[derive(Default)]
struct Shared {
    autocommit: AtomicBool,
    broken: AtomicBool,
    cancelled: AtomicBool,
    interrupt: Mutex<Option<Arc<rusqlite::InterruptHandle>>>,
}

struct Engine {
    db: *mut ffi::sqlite3,
    library: Option<Rc<rusqlite::Connection>>,
    classes: HashMap<String, bool>,
    shared: Arc<Shared>,
    options: Options,
    serial: i64,
    cursor: Option<Statement>,
    prepared: Vec<(i64, Statement)>,
    blobs: Vec<(i64, *mut ffi::sqlite3_blob, c_int, bool)>,
    backup: Option<(*mut ffi::sqlite3_backup, *mut ffi::sqlite3, i64)>,
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

unsafe extern "C" fn authorize(
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
    fn new(options: Options, shared: Arc<Shared>) -> Self {
        Self {
            db: std::ptr::null_mut(),
            library: None,
            classes: HashMap::new(),
            shared,
            options,
            serial: 0,
            cursor: None,
            prepared: Vec::new(),
            blobs: Vec::new(),
            backup: None,
            authorizer: Box::default(),
        }
    }

    fn autocommit(&self) -> bool {
        // SAFETY: the open connection of this worker thread.
        self.db.is_null() || unsafe { ffi::sqlite3_get_autocommit(self.db) } != 0
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
            // With bundled (modern) SQLite, prepare failures carry their offset.
            // The reference provider missed this variant and reported syntax
            // errors as "unexpected"; the SQLite code decides the kind here.
            rusqlite::Error::SqliteFailure(error, _)
            | rusqlite::Error::SqlInputError { error, .. } => self.error(error.extended_code),
            rusqlite::Error::MultipleStatement => {
                self.raise("syntax", "SQLITE_MULTIPLE_STATEMENTS")
            }
            rusqlite::Error::Utf8Error(..) => self.raise("data", "SQLITE_UTF8"),
            _ => Stop::Unexpected,
        }
    }

    fn open(&mut self) -> Done<()> {
        if rusqlite::version_number() < 3_041_000 {
            return Err(Stop::Sqlite {
                kind: "unsupported",
                code: "SQLITE_VERSION".into(),
                autocommit: true,
            });
        }
        let flags = if self.options.read_only {
            1
        } else {
            2 | if self.options.create { 4 } else { 0 }
        };
        let flags = rusqlite::OpenFlags::from_bits_truncate(flags);
        let library =
            rusqlite::Connection::open_with_flags(&self.options.path, flags).map_err(|error| {
                match error {
                    rusqlite::Error::SqliteFailure(error, _) => Stop::Sqlite {
                        kind: "connection",
                        code: format!("SQLITE_{}", error.extended_code),
                        autocommit: true,
                    },
                    _ => Stop::Unexpected,
                }
            })?;
        library.set_prepared_statement_cache_capacity(64);
        // SAFETY: the raw handle is used only on this thread while `library` lives.
        self.db = unsafe { library.handle() };
        *self.shared.interrupt.lock().expect("SQLite interrupt lock") =
            Some(Arc::new(library.get_interrupt_handle()));
        self.library = Some(Rc::new(library));
        // SAFETY: C API calls on the open connection with valid arguments.
        unsafe {
            ffi::sqlite3_extended_result_codes(self.db, 1);
            let rc = ffi::sqlite3_enable_load_extension(self.db, 0);
            self.check(rc)?;
            for (limit, value) in [(0, 8_388_608), (1, 1_048_576), (2, 1024), (9, 1024)] {
                ffi::sqlite3_limit(self.db, limit, value);
            }
            ffi::sqlite3_busy_timeout(self.db, self.options.busy_ms);
            ffi::sqlite3_set_authorizer(
                self.db,
                Some(authorize),
                (&*self.authorizer as *const Authorization)
                    .cast_mut()
                    .cast(),
            );
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
        let library = LibraryStatement::new(connection, sql, cached)
            .map_err(|error| self.library_error(error))?;
        if library.metadata().column_count() == 0 && library.metadata().expanded_sql().is_none() {
            return Err(self.raise("syntax", "SQLITE_EMPTY_STATEMENT"));
        }
        let dml = if cached {
            *self.classes.entry(sql.to_string()).or_insert_with(|| {
                self.authorizer.dml_seen.get() && !self.authorizer.ddl_seen.get()
            })
        } else {
            self.authorizer.dml_seen.get() && !self.authorizer.ddl_seen.get()
        };
        let columns = library.metadata().column_count() as c_int;
        Ok(Statement {
            library: Some(library),
            dml,
            columns,
            prepared: 0,
            id: 0,
            verified: false,
            raw_names: Vec::new(),
            raw_declarations: Vec::new(),
            pending_row: None,
        })
    }

    fn bind(&self, statement: &mut Statement, params: &[(&'static str, Param)]) -> Done<()> {
        let Some(library) = &mut statement.library else {
            return Err(Stop::Unexpected);
        };
        use rusqlite::types::ValueRef;
        library.reset();
        if params.len() != library.statement.parameter_count() {
            return Err(self.raise("data", "SQLITE_BIND_COUNT"));
        }
        for (at, (kind, value)) in params.iter().enumerate() {
            let bound = match (*kind, value) {
                ("null_value", _) => ValueRef::Null,
                ("integer" | "boolean", Param::Integer(value)) => ValueRef::Integer(
                    value.ok_or_else(|| self.raise("data", "SQLITE_INTEGER_RANGE"))?,
                ),
                ("integer" | "boolean", Param::Text(value)) => {
                    ValueRef::Integer(self.int64(value)?)
                }
                ("floating", value) => {
                    let value = match value {
                        Param::Floating(v) => *v,
                        Param::Text(v) => v.trim().parse::<f64>().unwrap_or(0.0),
                        _ => return Err(Stop::Unexpected),
                    };
                    if !value.is_finite() {
                        return Err(self.raise("data", "SQLITE_FLOAT_RANGE"));
                    }
                    ValueRef::Real(value)
                }
                ("text" | "decimal", Param::Text(value)) => {
                    if value.len() > 8_388_608 {
                        return Err(self.raise("resource", "SQLITE_VALUE_LIMIT"));
                    }
                    ValueRef::Text(value.as_bytes())
                }
                ("bytes", Param::Bytes(value)) => {
                    if value.len() > 8_388_608 {
                        return Err(self.raise("resource", "SQLITE_VALUE_LIMIT"));
                    }
                    ValueRef::Blob(value)
                }
                _ => return Err(self.raise("data", "SQLITE_VALUE_KIND")),
            };
            library
                .statement
                .raw_bind_parameter(at + 1, rusqlite::types::ToSqlOutput::Borrowed(bound))
                .map_err(|error| self.library_error(error))?;
        }
        Ok(())
    }

    fn execution(
        &mut self,
        sql: &str,
        params: &[(&'static str, Param)],
        control: bool,
    ) -> Done<Data> {
        let previous = self.authorizer.control.get();
        self.authorizer.control.set(control);
        let result = self
            .prepare(sql)
            .and_then(|statement| self.execute(statement, params));
        self.authorizer.control.set(previous);
        result
    }

    fn execute(
        &mut self,
        mut statement: Statement,
        params: &[(&'static str, Param)],
    ) -> Done<Data> {
        let outcome = (|| -> Done<Data> {
            if statement.columns != 0 {
                return Err(self.raise("data", "SQLITE_ROWS_REQUIRED"));
            }
            self.bind(&mut statement, params)?;
            let Some(library) = &mut statement.library else {
                return Err(Stop::Unexpected);
            };
            library
                .statement
                .raw_execute()
                .map_err(|error| self.library_error(error))?;
            Ok(Data {
                // SAFETY: the open connection of this worker thread.
                affected: Some(
                    statement
                        .dml
                        .then(|| unsafe { ffi::sqlite3_changes64(self.db) }.to_string()),
                ),
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

    /// Return a prepared statement to the table after execution.
    fn restore(&mut self, statement: Statement) {
        if let Some(slot) = self
            .prepared
            .iter_mut()
            .find(|(id, _)| *id == statement.prepared)
        {
            slot.1 = statement;
        }
    }

    fn rows_sql(&mut self, sql: &str, params: &[(&'static str, Param)]) -> Done<Data> {
        let statement = self.prepare(sql)?;
        self.rows(statement, params)
    }

    fn rows(&mut self, mut statement: Statement, params: &[(&'static str, Param)]) -> Done<Data> {
        let outcome = (|| -> Done<(Vec<String>, Vec<String>)> {
            statement.columns = statement
                .library
                .as_ref()
                .ok_or(Stop::Unexpected)?
                .metadata()
                .column_count() as c_int;
            if statement.columns == 0 {
                return Err(self.raise("data", "SQLITE_EXECUTION_REQUIRED"));
            }
            self.bind(&mut statement, params)?;
            let Some(library) = &mut statement.library else {
                return Err(Stop::Unexpected);
            };
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
                names.push(name.to_owned());
                declarations.push(declaration.to_owned());
            }
            library.start_rows();
            Ok((names, declarations))
        })();
        match outcome {
            Err(error) => {
                let _ = self.finish(&mut statement);
                if statement.prepared != 0 {
                    self.restore(statement);
                }
                Err(error)
            }
            Ok((names, declarations)) => {
                statement.verified = false;
                statement.raw_names = names.clone();
                statement.raw_declarations = declarations.clone();
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
        let Some(library) = &cursor.library else {
            return Err(Stop::Unexpected);
        };
        let columns = library.metadata().columns();
        if columns.len() != cursor.columns as usize
            || columns.iter().enumerate().any(|(index, column)| {
                column.name() != cursor.raw_names[index]
                    || column.decl_type().unwrap_or("") != cursor.raw_declarations[index]
            })
        {
            return Err(self.raise("data", "SQLITE_SCHEMA_CHANGED"));
        }
        cursor.verified = true;
        Ok(())
    }

    fn library_row(&self, row: &rusqlite::Row<'_>, cursor: &Statement) -> Done<(Vec<Cell>, usize)> {
        use rusqlite::types::ValueRef;
        if !cursor.verified {
            let columns = row.as_ref().columns();
            if columns.len() != cursor.columns as usize
                || columns.iter().enumerate().any(|(index, column)| {
                    column.name() != cursor.raw_names[index]
                        || column.decl_type().unwrap_or("") != cursor.raw_declarations[index]
                })
            {
                return Err(self.raise("data", "SQLITE_SCHEMA_CHANGED"));
            }
        }
        let mut values = Vec::with_capacity(cursor.columns as usize);
        let mut used = 0usize;
        for index in 0..cursor.columns as usize {
            let value = row
                .get_ref(index)
                .map_err(|error| self.library_error(error))?;
            let (width, cell) = match value {
                ValueRef::Null => (0, Cell::Null),
                ValueRef::Integer(value) => (value.to_string().len(), Cell::Integer(value)),
                ValueRef::Real(value) => {
                    if !value.is_finite() {
                        return Err(self.raise("data", "SQLITE_FLOAT_RANGE"));
                    }
                    (float_width(value), Cell::Floating(value))
                }
                ValueRef::Text(value) | ValueRef::Blob(value) => {
                    if value.len() > 8_388_608 - used.min(8_388_608) {
                        return Err(self.raise("resource", "SQLITE_ROW_LIMIT"));
                    }
                    let cell = if matches!(row.get_ref(index), Ok(ValueRef::Text(_))) {
                        Cell::Text(
                            std::str::from_utf8(value)
                                .map_err(|_| self.raise("data", "SQLITE_UTF8"))?
                                .to_owned(),
                        )
                    } else {
                        Cell::Bytes(value.to_vec())
                    };
                    (value.len(), cell)
                }
            };
            used += width;
            if used > 8_388_608 {
                return Err(self.raise("resource", "SQLITE_ROW_LIMIT"));
            }
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
        if let Some((values, bytes)) = cursor.pending_row.take() {
            rows.push(values);
            used = bytes;
        }
        // Take the library handle separately so a Row borrow cannot alias the
        // cursor metadata while its lifetime is checked and values are copied.
        let mut library = cursor.library.take().ok_or(Stop::Unexpected)?;
        let outcome = (|| -> Done<()> {
            while rows.len() < maximum as usize {
                let found = library
                    .rows
                    .as_mut()
                    .ok_or(Stop::Unexpected)?
                    .next()
                    .map_err(|error| self.library_error(error))?;
                let Some(row) = found else {
                    eof = true;
                    break;
                };
                let (values, width) = self.library_row(row, &cursor)?;
                cursor.verified = true;
                if used + width > 8_388_608 {
                    cursor.pending_row = Some((values, width));
                    break;
                }
                used += width;
                rows.push(values);
            }
            Ok(())
        })();
        cursor.library = Some(library);
        let outcome = outcome.and_then(|()| {
            if eof {
                self.verify(&mut cursor)
            } else {
                Ok(())
            }
        });
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
        statement.pending_row = None;
        if let Some(library) = &mut statement.library {
            library.reset();
            if statement.prepared == 0 {
                statement.library.take();
            }
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
        Ok(Data {
            id: Some(id),
            ..Data::default()
        })
    }

    /// A prepared statement is taken out of the table for its execution.
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
        if let Some(cursor_id) = self
            .cursor
            .as_ref()
            .filter(|cursor| cursor.prepared == id)
            .map(|cursor| cursor.id)
        {
            self.release(cursor_id)?;
            // release would have returned it to the table: it is removed already.
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
        let canonical = body == "0"
            || (body
                .bytes()
                .next()
                .is_some_and(|b| (b'1'..=b'9').contains(&b))
                && body.bytes().all(|b| b.is_ascii_digit()));
        if !canonical || value.len() > 20 {
            return Err(self.raise("data", "SQLITE_INTEGER_RANGE"));
        }
        value
            .parse::<i64>()
            .map_err(|_| self.raise("data", "SQLITE_INTEGER_RANGE"))
    }

    fn blob_open(
        &mut self,
        database: &str,
        table: &str,
        column: &str,
        row: &str,
        writable: bool,
    ) -> Done<Data> {
        self.ready()?;
        if !self.blobs.is_empty() {
            return Err(self.raise("busy", "SQLITE_BLOB_ACTIVE"));
        }
        let (database, table, column) = (
            self.identifier(database)?,
            self.identifier(table)?,
            self.identifier(column)?,
        );
        let rowid = self.int64(row)?;
        let mut blob: *mut ffi::sqlite3_blob = std::ptr::null_mut();
        // SAFETY: NUL-terminated names and a valid output slot.
        let rc = unsafe {
            ffi::sqlite3_blob_open(
                self.db,
                database.as_ptr(),
                table.as_ptr(),
                column.as_ptr(),
                rowid,
                c_int::from(writable),
                &mut blob,
            )
        };
        if rc != 0 {
            if !blob.is_null() {
                // SAFETY: SQLite may return a handle that must still be closed.
                unsafe { ffi::sqlite3_blob_close(blob) };
            }
            return Err(self.error(rc));
        }
        self.serial += 1;
        let id = self.serial;
        // SAFETY: the BLOB handle opened above.
        let size = unsafe { ffi::sqlite3_blob_bytes(blob) };
        self.blobs.push((id, blob, size, writable));
        Ok(Data {
            id: Some(id),
            size: Some(i64::from(size)),
            ..Data::default()
        })
    }

    fn blob(
        &self,
        id: i64,
        offset: &str,
        length: i64,
    ) -> Done<(*mut ffi::sqlite3_blob, c_int, bool)> {
        let Some(&(_, blob, size, writable)) = self.blobs.iter().find(|(known, ..)| *known == id)
        else {
            return Err(self.raise("data", "SQLITE_BLOB_STALE"));
        };
        let valid =
            !offset.is_empty() && offset.bytes().all(|b| b.is_ascii_digit()) && offset.len() <= 10;
        let at = if valid {
            offset.parse::<i64>().unwrap_or(i64::MAX)
        } else {
            -1
        };
        if !valid
            || !(0..=65536).contains(&length)
            || at > i64::from(size)
            || length > i64::from(size) - at
        {
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
        // SAFETY: the buffer holds `length` bytes.
        self.check(unsafe {
            ffi::sqlite3_blob_read(blob, buffer.as_mut_ptr().cast(), length as c_int, at)
        })?;
        Ok(Data {
            bytes: Some(buffer),
            ..Data::default()
        })
    }

    fn blob_write(&self, id: i64, offset: &str, data: &[u8]) -> Done<Data> {
        let (blob, at, writable) = self.blob(id, offset, data.len() as i64)?;
        if !writable {
            return Err(self.raise("permission", "SQLITE_BLOB_READ_ONLY"));
        }
        let buffer = if data.is_empty() {
            vec![0u8]
        } else {
            data.to_vec()
        };
        // SAFETY: the buffer holds at least `data.len()` bytes.
        self.check(unsafe {
            ffi::sqlite3_blob_write(blob, buffer.as_ptr().cast(), data.len() as c_int, at)
        })?;
        Ok(Data::default())
    }

    fn blob_close(&mut self, id: i64) -> Done<Data> {
        if let Some(position) = self.blobs.iter().position(|(known, ..)| *known == id) {
            let (_, blob, ..) = self.blobs.remove(position);
            // SAFETY: an open BLOB, closed exactly once.
            self.check(unsafe { ffi::sqlite3_blob_close(blob) })?;
        }
        Ok(Data {
            autocommit: Some(self.autocommit()),
            ..Data::default()
        })
    }

    fn same_file(&self, left: &str, right: &str) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let (Ok(a), Ok(b)) = (std::fs::metadata(left), std::fs::metadata(right)) else {
                return false;
            };
            a.dev() == b.dev() && a.ino() == b.ino()
        }
        #[cfg(not(unix))]
        {
            left == right
        }
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
        let target_path = CString::new(path).map_err(|_| Stop::Unexpected)?;
        let mut target: *mut ffi::sqlite3 = std::ptr::null_mut();
        // SAFETY: C API with a NUL-terminated path and a valid output slot.
        let rc =
            unsafe { ffi::sqlite3_open_v2(target_path.as_ptr(), &mut target, 6, std::ptr::null()) };
        if rc != 0 {
            if !target.is_null() {
                // SAFETY: a failed open still returns a handle to close.
                unsafe { ffi::sqlite3_close(target) };
            }
            return Err(self.error(rc));
        }
        let main = c"main";
        // SAFETY: both connections are open; names are NUL-terminated.
        let pointer =
            unsafe { ffi::sqlite3_backup_init(target, main.as_ptr(), self.db, main.as_ptr()) };
        if pointer.is_null() {
            // SAFETY: the target connection opened above.
            let code = unsafe { ffi::sqlite3_extended_errcode(target) };
            unsafe { ffi::sqlite3_close(target) };
            return Err(self.error(code));
        }
        self.serial += 1;
        let id = self.serial;
        self.backup = Some((pointer, target, id));
        Ok(Data {
            id: Some(id),
            ..Data::default()
        })
    }

    fn backup_step(&self, id: i64, pages: i64) -> Done<Data> {
        let Some((pointer, _, _)) = self.backup.filter(|(_, _, known)| *known == id) else {
            return Err(self.raise("data", "SQLITE_BACKUP_STALE"));
        };
        if !(1..=256).contains(&pages) {
            return Err(self.raise("data", "SQLITE_BACKUP_PAGES"));
        }
        if self.shared.cancelled.load(Ordering::SeqCst) {
            return Err(self.raise("cancelled", "SQLITE_9"));
        }
        // SAFETY: the open backup of this connection.
        let rc = unsafe { ffi::sqlite3_backup_step(pointer, pages as c_int) };
        if !matches!(rc, 0 | 101 | 5 | 6) {
            return Err(self.error(rc));
        }
        let total = unsafe { ffi::sqlite3_backup_pagecount(pointer) };
        let remaining = unsafe { ffi::sqlite3_backup_remaining(pointer) };
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
        // SAFETY: the backup and its target are finished/closed exactly once.
        let rc = unsafe { ffi::sqlite3_backup_finish(pointer) };
        let closed = unsafe { ffi::sqlite3_close(target) };
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
            let Some(statement) = batch.next().map_err(|error| self.library_error(error))? else {
                break;
            };
            if results.len() >= 256 {
                return Err(self.raise("resource", "SQLITE_SCRIPT_COUNT"));
            }
            let columns = statement.column_count() as c_int;
            // SAFETY: Batch was constructed on this same retained connection.
            let library =
                unsafe { LibraryStatement::from_statement(connection.clone(), statement) };
            let statement = Statement {
                library: Some(library),
                dml: self.authorizer.dml_seen.get() && !self.authorizer.ddl_seen.get(),
                columns,
                prepared: 0,
                id: 0,
                verified: false,
                raw_names: Vec::new(),
                raw_declarations: Vec::new(),
                pending_row: None,
            };
            results.push(self.execute(statement, &[])?.affected.flatten());
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
            return Ok(Data {
                autocommit: Some(true),
                ..Data::default()
            });
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
        self.shared
            .interrupt
            .lock()
            .expect("SQLite interrupt lock")
            .take();
        self.classes.clear();
        if let Some(library) = self.library.take() {
            let library = Rc::try_unwrap(library).map_err(|_| Stop::Unexpected)?;
            library
                .close()
                .map_err(|(_, error)| self.library_error(error))?;
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
    match reply
        .rows
        .as_ref()
        .and_then(|rows| rows.first())
        .and_then(|row| row.first())
    {
        Some(Cell::Integer(value)) => value.to_string(),
        Some(Cell::Floating(value)) => crate::scalar::float_text(*value),
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

// ---- connection ----

type Job = Box<dyn FnOnce(&mut Engine) + Send>;
type Operation = Box<dyn FnOnce(&mut Engine) -> Done<Data> + Send>;

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteShared")
            .field("autocommit", &self.autocommit)
            .field("broken", &self.broken)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct Worker {
    /// None once close was queued: the worker exits after closing.
    jobs: Option<mpsc::Sender<Job>>,
    exited: tokio::sync::watch::Receiver<bool>,
}

#[derive(Debug)]
pub(crate) struct Connection {
    options: Options,
    stopped: bool,
    handle_closed: bool,
    worker: Option<Worker>,
    shared: Arc<Shared>,
}
type ConnectionRef = Rc<RefCell<Connection>>;

impl Connection {
    fn broken(&self) -> bool {
        self.shared.broken.load(Ordering::SeqCst)
    }
    fn jobs(&self) -> Option<mpsc::Sender<Job>> {
        self.worker.as_ref().and_then(|w| w.jobs.clone())
    }
}

/// The Engine is created on, and never leaves, its worker thread.
fn spawn_worker(connection: &mut Connection) -> std::io::Result<()> {
    let (jobs, queue) = mpsc::channel::<Job>();
    let (exit, exited) = tokio::sync::watch::channel(false);
    let options = connection.options.clone();
    let shared = connection.shared.clone();
    std::thread::Builder::new()
        .name("okkama-sqlite".into())
        .spawn(move || {
            let mut engine = Engine::new(options, shared);
            while let Ok(job) = queue.recv() {
                job(&mut engine);
            }
            drop(engine);
            let _ = exit.send(true);
        })?;
    connection.worker = Some(Worker {
        jobs: Some(jobs),
        exited,
    });
    Ok(())
}

fn perform(engine: &mut Engine, operation: Operation) -> (bool, Data) {
    engine.shared.cancelled.store(false, Ordering::SeqCst);
    let (ok, data) = match operation(engine) {
        Ok(data) => (true, data),
        Err(Stop::Sqlite {
            kind,
            code,
            autocommit,
        }) => (
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
    if let Some(autocommit) = data.autocommit {
        engine.shared.autocommit.store(autocommit, Ordering::SeqCst);
    }
    (ok, data)
}

/// Queue a service operation without awaiting it; a failure breaks the connection.
fn maintenance(state: &ConnectionRef, operation: Operation) -> bool {
    let connection = state.borrow();
    if connection.stopped {
        return true;
    }
    let Some(jobs) = connection.jobs() else {
        return true;
    };
    let shared = connection.shared.clone();
    let job: Job = Box::new(move |engine| {
        if !perform(engine, operation).0 {
            shared.broken.store(true, Ordering::SeqCst);
        }
    });
    if jobs.send(job).is_err() {
        connection.shared.broken.store(true, Ordering::SeqCst);
    }
    true
}

/// A cancelled operation that still succeeded releases what it acquired.
fn cleanup_cancelled(state: &ConnectionRef, op: &str, data: &Data) {
    let Some(id) = data.id else {
        return;
    };
    let operation: Operation = match op {
        "rows" | "prepared_rows" => {
            Box::new(move |engine| engine.release(id).map(|()| Data::default()))
        }
        "prepare" => Box::new(move |engine| engine.unprepare(id)),
        "blob_open" => Box::new(move |engine| engine.blob_close(id)),
        "backup_open" => Box::new(move |engine| engine.backup_close(id)),
        _ => return,
    };
    maintenance(state, operation);
}

struct ReplyOutput(bool, Data);
impl Output for ReplyOutput {
    fn publish(self: Box<Self>, heap: &mut Heap) -> Result<Slot, Error> {
        let reply = Rc::new(Reply {
            ok: self.0,
            data: RefCell::new(self.1),
            closed: std::cell::Cell::new(false),
        });
        Ok(Slot {
            tag: HANDLE,
            bits: heap.handles.allocate(Handle::SqliteReply(reply))?,
        })
    }
}
struct Flag(bool);
impl Output for Flag {
    fn publish(self: Box<Self>, _heap: &mut Heap) -> Result<Slot, Error> {
        Ok(Slot::boolean(self.0))
    }
}

const KINDS: [&str; 7] = [
    "null_value",
    "boolean",
    "integer",
    "floating",
    "decimal",
    "text",
    "bytes",
];

pub(crate) fn handles(native: Native) -> bool {
    native.symbol().starts_with("runtime/database@1::sqlite_")
}

impl Heap {
    pub(crate) fn native_sqlite(&mut self, n: Native, a: &[Slot]) -> Result<Outcome, Error> {
        match self.sqlite_execute(n, a) {
            Ok(value) => Ok(Outcome::Value(value)),
            Err(ProviderError::Engine(error)) => Err(error),
            Err(ProviderError::Native(error)) => {
                let payload = self.native_failure_new(error)?;
                self.failure_new(crate::failure::FailureKind::Native(payload))
                    .map(Outcome::Failure)
            }
        }
    }

    fn sqlite_state(&self, n: Native, v: Slot) -> Result<ConnectionRef, ProviderError> {
        if v.tag == HANDLE {
            if let Handle::Sqlite(state) = self.handles.get(v.bits)? {
                return Ok(state.clone());
            }
        }
        Err(ProviderError::Native(fail(
            n,
            "invalid_input",
            "Invalid SQLite state",
        )))
    }

    fn sqlite_connection(&self, n: Native, v: Slot) -> Result<ConnectionRef, ProviderError> {
        let state = self.sqlite_state(n, v)?;
        if state.borrow().handle_closed {
            return Err(ProviderError::Native(fail(
                n,
                "already_closed",
                "Native handle is already closed",
            )));
        }
        Ok(state)
    }

    fn sqlite_reply(&self, n: Native, v: Slot) -> Result<Rc<Reply>, ProviderError> {
        let reply = if v.tag == HANDLE {
            match self.handles.get(v.bits)? {
                Handle::SqliteReply(reply) => Some(reply.clone()),
                _ => None,
            }
        } else {
            None
        };
        let reply = reply.ok_or_else(|| {
            ProviderError::Native(fail(n, "invalid_input", "Invalid SQLite reply"))
        })?;
        if reply.closed.get() {
            return Err(ProviderError::Native(fail(
                n,
                "already_closed",
                "Native handle is already closed",
            )));
        }
        Ok(reply)
    }

    fn sqlite_failed(&mut self, failure: NativeFailure) -> Result<Slot, Error> {
        self.native_io(async move { Err::<Flag, _>(failure) })
    }

    fn sqlite_ready(&mut self, value: bool) -> Result<Slot, Error> {
        self.native_io(async move { Ok(Flag(value)) })
    }

    /// An operation on the connection worker, delivered as a native I/O Future.
    /// Cancellation interrupts SQLite and waits for the worker's reply; a reply
    /// that still succeeded releases its cursor, statement, BLOB or backup.
    fn sqlite_request(
        &mut self,
        n: Native,
        state: &ConnectionRef,
        op: &'static str,
        operation: Operation,
    ) -> Result<Slot, Error> {
        let (jobs, shared) = {
            let connection = state.borrow();
            if connection.stopped || connection.broken() {
                drop(connection);
                return self.sqlite_failed(fail(
                    n,
                    "already_closed",
                    "SQLite connection is closed",
                ));
            }
            if connection.worker.is_none() {
                drop(connection);
                return self.sqlite_failed(fail(
                    n,
                    "invalid_input",
                    "SQLite connection is not acquired",
                ));
            }
            (connection.jobs(), connection.shared.clone())
        };
        let (reply, receiver) = tokio::sync::oneshot::channel::<(bool, Data)>();
        let job: Job = Box::new(move |engine| {
            let _ = reply.send(perform(engine, operation));
        });
        if jobs.is_none_or(|jobs| jobs.send(job).is_err()) {
            return self.sqlite_failed(fail(n, "already_closed", "SQLite connection is closed"));
        }
        let receiver = Rc::new(RefCell::new(Some(receiver)));
        let body = {
            let receiver = receiver.clone();
            let shared = shared.clone();
            async move {
                let delivered = std::future::poll_fn(|cx| match receiver.borrow_mut().as_mut() {
                    Some(receiver) => std::pin::Pin::new(receiver).poll(cx).map(Result::ok),
                    None => std::task::Poll::Ready(None),
                })
                .await;
                match delivered {
                    Some((ok, data)) => Ok(ReplyOutput(ok, data)),
                    None => {
                        shared.broken.store(true, Ordering::SeqCst);
                        Err(fail(n, "connection_reset", "SQLite worker lost"))
                    }
                }
            }
        };
        let state = state.clone();
        let cancel: CancelHook = Box::new(move || {
            Box::pin(async move {
                shared.cancelled.store(true, Ordering::SeqCst);
                let handle = shared
                    .interrupt
                    .lock()
                    .expect("SQLite interrupt lock")
                    .clone();
                if let Some(handle) = handle {
                    // rusqlite synchronizes interruption with connection closure.
                    handle.interrupt();
                }
                let pending = receiver.borrow_mut().take();
                if let Some(pending) = pending {
                    match pending.await {
                        Ok((true, data)) => cleanup_cancelled(&state, op, &data),
                        Ok((false, _)) => {}
                        Err(_) => shared.broken.store(true, Ordering::SeqCst),
                    }
                }
                Ok(())
            })
        });
        self.native_io_with_cleanup(body, Some(cancel))
    }

    fn sqlite_settled(&mut self, state: ConnectionRef) -> Result<Slot, Error> {
        let (jobs, mut exited, stopped) = {
            let mut connection = state.borrow_mut();
            if connection.handle_closed {
                drop(connection);
                return self.sqlite_ready(true);
            }
            let Some(worker) = &connection.worker else {
                if connection.stopped {
                    connection.handle_closed = true;
                }
                drop(connection);
                return self.sqlite_ready(true);
            };
            (
                worker.jobs.clone(),
                worker.exited.clone(),
                connection.stopped,
            )
        };
        if !stopped {
            if let Some(jobs) = jobs {
                // A FIFO barrier behind every previously queued operation.
                let (done, barrier) = tokio::sync::oneshot::channel::<()>();
                let _ = jobs.send(Box::new(move |_engine| {
                    let _ = done.send(());
                }));
                return self.native_io(async move {
                    let _ = barrier.await;
                    let mut connection = state.borrow_mut();
                    if connection.stopped {
                        connection.handle_closed = true;
                    }
                    Ok(Flag(true))
                });
            }
        }
        self.native_io(async move {
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                exited.wait_for(|done| *done),
            )
            .await;
            let mut connection = state.borrow_mut();
            if connection.stopped {
                connection.handle_closed = true;
            }
            Ok(Flag(true))
        })
    }

    fn sqlite_int(&self, n: Native, v: Slot) -> Result<i64, ProviderError> {
        self.integer(v)?.to_i64().ok_or_else(|| {
            ProviderError::Native(fail(n, "invalid_input", "Invalid SQLite integer"))
        })
    }

    fn sqlite_text(&self, n: Native, v: Slot, path: bool) -> Result<String, ProviderError> {
        let data = self.native_bytes(v)?;
        let text = strict(&data)
            .filter(|text| {
                !text.contains('\0') && data.len() <= if path { 4096 } else { 1_048_576 }
            })
            .ok_or_else(|| {
                ProviderError::Native(fail(n, "invalid_input", "Invalid SQLite text"))
            })?;
        Ok(text)
    }

    fn sqlite_path(&mut self, text: &str) -> Result<String, Error> {
        let path = std::path::PathBuf::from(text);
        let path = if path.is_absolute() {
            path
        } else {
            self.working_directory()?.join(path)
        };
        Ok(path.to_string_lossy().into_owned())
    }

    fn sqlite_params(
        &self,
        n: Native,
        tags: Slot,
        payloads: Slot,
    ) -> Result<Vec<(&'static str, Param)>, NativeFailure> {
        let invalid = |message: &str| fail(n, "invalid_input", message);
        let tags = self
            .elements(tags)
            .map_err(|_| invalid("Invalid SQLite bindings"))?;
        let payloads = self
            .elements(payloads)
            .map_err(|_| invalid("Invalid SQLite bindings"))?;
        if tags.len() != payloads.len() || tags.len() > 1024 {
            return Err(invalid("Invalid SQLite bindings"));
        }
        let mut out = Vec::with_capacity(tags.len());
        let mut used = 0usize;
        for (tag, payload) in tags.iter().zip(payloads) {
            let Some(tag) = self
                .integer(*tag)
                .ok()
                .and_then(|t| t.to_i64())
                .filter(|tag| (0..=6).contains(tag))
            else {
                return Err(invalid("Invalid SQLite binding kind"));
            };
            let data = self
                .native_bytes(*payload)
                .map_err(|_| invalid("Invalid SQLite bindings"))?;
            used += data.len();
            if used > 8_388_608 {
                return Err(fail(n, "resource_exhausted", "SQLite binding byte budget"));
            }
            let value = if tag == 6 {
                Param::Bytes(data)
            } else {
                Param::Text(
                    strict(&data).ok_or_else(|| invalid("Invalid SQLite binding encoding"))?,
                )
            };
            out.push((KINDS[tag as usize], value));
        }
        Ok(out)
    }

    /// Copy only owned typed payloads to the worker; no VM value crosses threads.
    fn sqlite_cell_params(
        &self,
        n: Native,
        value: Slot,
    ) -> Result<Vec<(&'static str, Param)>, NativeFailure> {
        use crate::native_db::Cell as DbCell;
        let invalid = || fail(n, "invalid_input", "Invalid SQLite bindings");
        let cells = self.elements(value).map_err(|_| invalid())?;
        if cells.len() > 1024 {
            return Err(invalid());
        }
        let mut used = 0usize;
        let mut params = Vec::with_capacity(cells.len());
        for value in cells {
            let cell = self.db_cell(*value).map_err(|_| invalid())?;
            let (width, payload) = match cell {
                DbCell::Null => (0, Param::Text(String::new())),
                DbCell::Boolean(value) => (1, Param::Integer(Some(i64::from(*value)))),
                DbCell::Integer(value) => (value.to_string().len(), Param::Integer(value.to_i64())),
                DbCell::Floating(value) => (float_width(*value), Param::Floating(*value)),
                DbCell::Decimal(value) | DbCell::Text(value) => {
                    (value.len(), Param::Text(value.to_string()))
                }
                DbCell::Bytes(value) => (value.len(), Param::Bytes(value.to_vec())),
            };
            used += width;
            if used > 8_388_608 {
                return Err(fail(n, "resource_exhausted", "SQLite binding byte budget"));
            }
            params.push((KINDS[cell.kind() as usize], payload));
        }
        Ok(params)
    }

    fn sqlite_texts(&mut self, values: impl IntoIterator<Item = String>) -> Result<Slot, Error> {
        let values = values
            .into_iter()
            .map(|s| self.allocate_text(s))
            .collect::<Result<Vec<_>, _>>()?;
        self.allocate_array(values)
    }

    fn sqlite_execute(&mut self, n: Native, a: &[Slot]) -> Result<Slot, ProviderError> {
        use Native::*;
        let yes = Slot::boolean(true);
        // Asynchronous operations report argument failures through their Future.
        macro_rules! future {
            ($body:expr) => {
                match (|| -> Result<Slot, ProviderError> { $body })() {
                    Err(ProviderError::Native(failure)) => Ok(self.sqlite_failed(failure)?),
                    other => other,
                }
            };
        }
        match n {
            DatabaseSqliteCreate => {
                let path = self.native_bytes(a[0])?;
                let (read_only, create, foreign) = (a[1].bits != 0, a[2].bits != 0, a[3].bits != 0);
                let busy = self
                    .integer(a[4])?
                    .to_i64()
                    .filter(|b| (0..=60000).contains(b));
                let journal = self.text(a[5])?.to_owned();
                let memory = a[6].bits != 0;
                let invalid = path.len() > 4096
                    || path.contains(&0)
                    || busy.is_none()
                    || (read_only && create)
                    || !matches!(journal.as_str(), "delete" | "wal" | "memory");
                if invalid {
                    return Err(ProviderError::Native(fail(
                        n,
                        "invalid_input",
                        "Invalid SQLite options",
                    )));
                }
                let Some(text) = strict(&path).filter(|text| memory || !text.is_empty()) else {
                    return Err(ProviderError::Native(fail(
                        n,
                        "invalid_input",
                        "SQLite path must be UTF8",
                    )));
                };
                let path = if memory {
                    ":memory:".into()
                } else {
                    self.sqlite_path(&text)?
                };
                let connection = Connection {
                    options: Options {
                        path,
                        read_only,
                        create,
                        foreign_keys: foreign,
                        busy_ms: busy.unwrap_or(0) as c_int,
                        journal,
                    },
                    stopped: false,
                    handle_closed: false,
                    worker: None,
                    shared: Arc::new(Shared {
                        autocommit: AtomicBool::new(true),
                        ..Shared::default()
                    }),
                };
                Ok(Slot {
                    tag: HANDLE,
                    bits: self
                        .handles
                        .allocate(Handle::Sqlite(Rc::new(RefCell::new(connection))))?,
                })
            }
            DatabaseSqliteConnect => future!({
                let state = self.sqlite_connection(n, a[0])?;
                if state.borrow().worker.is_some() {
                    return Err(ProviderError::Native(fail(
                        n,
                        "invalid_input",
                        "SQLite connection already acquired",
                    )));
                }
                spawn_worker(&mut state.borrow_mut()).map_err(|_| {
                    ProviderError::Native(fail(
                        n,
                        "resource_exhausted",
                        "SQLite worker unavailable",
                    ))
                })?;
                Ok(self.sqlite_request(
                    n,
                    &state,
                    "open",
                    Box::new(|engine| {
                        engine.open()?;
                        Ok(Data {
                            autocommit: Some(true),
                            ..Data::default()
                        })
                    }),
                )?)
            }),
            DatabaseSqliteExecution | DatabaseSqliteRows => future!({
                let rows = n == DatabaseSqliteRows;
                let state = self.sqlite_connection(n, a[0])?;
                let sql = self.sqlite_text(n, a[1], false)?;
                if sql.is_empty() {
                    return Err(ProviderError::Native(fail(
                        n,
                        "invalid_input",
                        "Invalid SQLite SQL",
                    )));
                }
                let params = self
                    .sqlite_params(n, a[2], a[3])
                    .map_err(ProviderError::Native)?;
                let operation: Operation = if rows {
                    Box::new(move |engine| engine.rows_sql(&sql, &params))
                } else {
                    Box::new(move |engine| engine.execution(&sql, &params, false))
                };
                Ok(self.sqlite_request(
                    n,
                    &state,
                    if rows { "rows" } else { "execution" },
                    operation,
                )?)
            }),
            DatabaseSqliteExecutionCells
            | DatabaseSqliteRowsCells
            | DatabaseSqlitePreparedExecutionCells
            | DatabaseSqlitePreparedRowsCells => future!({
                let rows = matches!(n, DatabaseSqliteRowsCells | DatabaseSqlitePreparedRowsCells);
                let prepared = matches!(
                    n,
                    DatabaseSqlitePreparedExecutionCells | DatabaseSqlitePreparedRowsCells
                );
                let state = self.sqlite_connection(n, a[0])?;
                let params = self
                    .sqlite_cell_params(n, a[2])
                    .map_err(ProviderError::Native)?;
                let operation: Operation = if prepared {
                    let id = self
                        .integer(a[1])?
                        .to_i64()
                        .filter(|id| *id >= 1)
                        .ok_or_else(|| {
                            ProviderError::Native(fail(
                                n,
                                "invalid_input",
                                "Invalid SQLite statement",
                            ))
                        })?;
                    Box::new(move |engine| {
                        let statement = engine.take_prepared(id)?;
                        if rows {
                            engine.rows(statement, &params)
                        } else {
                            engine.execute(statement, &params)
                        }
                    })
                } else {
                    let sql = self.text(a[1])?.to_owned();
                    if sql.is_empty() || sql.contains('\0') || sql.len() > 1_048_576 {
                        return Err(ProviderError::Native(fail(
                            n,
                            "invalid_input",
                            "Invalid SQLite SQL",
                        )));
                    }
                    Box::new(move |engine| {
                        if rows {
                            engine.rows_sql(&sql, &params)
                        } else {
                            engine.execution(&sql, &params, false)
                        }
                    })
                };
                let op = match (prepared, rows) {
                    (false, false) => "execution",
                    (false, true) => "rows",
                    (true, false) => "prepared_execution",
                    (true, true) => "prepared_rows",
                };
                Ok(self.sqlite_request(n, &state, op, operation)?)
            }),
            DatabaseSqlitePrepare => future!({
                let state = self.sqlite_connection(n, a[0])?;
                let sql = self.sqlite_text(n, a[1], false)?;
                Ok(self.sqlite_request(
                    n,
                    &state,
                    "prepare",
                    Box::new(move |engine| engine.prepare_statement(&sql)),
                )?)
            }),
            DatabaseSqlitePreparedExecution | DatabaseSqlitePreparedRows => future!({
                let rows = n == DatabaseSqlitePreparedRows;
                let state = self.sqlite_connection(n, a[0])?;
                let id = self
                    .integer(a[1])?
                    .to_i64()
                    .filter(|id| *id >= 1)
                    .ok_or_else(|| {
                        ProviderError::Native(fail(n, "invalid_input", "Invalid SQLite bindings"))
                    })?;
                let params = self
                    .sqlite_params(n, a[2], a[3])
                    .map_err(ProviderError::Native)?;
                let operation: Operation = Box::new(move |engine| {
                    let statement = engine.take_prepared(id)?;
                    if rows {
                        engine.rows(statement, &params)
                    } else {
                        engine.execute(statement, &params)
                    }
                });
                Ok(self.sqlite_request(
                    n,
                    &state,
                    if rows {
                        "prepared_rows"
                    } else {
                        "prepared_execution"
                    },
                    operation,
                )?)
            }),
            DatabaseSqliteUnprepare
            | DatabaseSqliteRelease
            | DatabaseSqliteBlobClose
            | DatabaseSqliteBackupClose => {
                let id = self.sqlite_int(n, a[1])?;
                let state = self.sqlite_connection(n, a[0])?;
                let operation: Operation = match n {
                    DatabaseSqliteUnprepare => Box::new(move |engine| engine.unprepare(id)),
                    DatabaseSqliteRelease => {
                        Box::new(move |engine| engine.release(id).map(|()| Data::default()))
                    }
                    DatabaseSqliteBlobClose => Box::new(move |engine| engine.blob_close(id)),
                    _ => Box::new(move |engine| engine.backup_close(id)),
                };
                Ok(Slot::boolean(maintenance(&state, operation)))
            }
            DatabaseSqliteRollback => {
                let state = self.sqlite_connection(n, a[0])?;
                Ok(Slot::boolean(maintenance(
                    &state,
                    Box::new(|engine| engine.rollback()),
                )))
            }
            DatabaseSqliteBlobOpen => future!({
                let state = self.sqlite_connection(n, a[0])?;
                let (database, table, column, row) = (
                    self.text(a[1])?.to_owned(),
                    self.text(a[2])?.to_owned(),
                    self.text(a[3])?.to_owned(),
                    self.text(a[4])?.to_owned(),
                );
                let writable = a[5].bits != 0;
                Ok(self.sqlite_request(
                    n,
                    &state,
                    "blob_open",
                    Box::new(move |engine| {
                        engine.blob_open(&database, &table, &column, &row, writable)
                    }),
                )?)
            }),
            DatabaseSqliteBlobRead => future!({
                let state = self.sqlite_connection(n, a[0])?;
                let blob = self.sqlite_int(n, a[1])?;
                let offset = self.text(a[2])?.to_owned();
                let size = self
                    .integer(a[3])?
                    .to_i64()
                    .filter(|size| i32::try_from(*size).is_ok())
                    .unwrap_or(-1);
                Ok(self.sqlite_request(
                    n,
                    &state,
                    "blob_read",
                    Box::new(move |engine| engine.blob_read(blob, &offset, size)),
                )?)
            }),
            DatabaseSqliteBlobWrite => future!({
                let state = self.sqlite_connection(n, a[0])?;
                let data = self.native_bytes(a[3])?;
                if data.len() > 65536 {
                    return Err(ProviderError::Native(fail(
                        n,
                        "resource_exhausted",
                        "SQLite blob byte budget",
                    )));
                }
                let blob = self.integer(a[1])?.to_i64().unwrap_or(0);
                let offset = self.text(a[2])?.to_owned();
                Ok(self.sqlite_request(
                    n,
                    &state,
                    "blob_write",
                    Box::new(move |engine| engine.blob_write(blob, &offset, &data)),
                )?)
            }),
            DatabaseSqliteBackupOpen => future!({
                let state = self.sqlite_connection(n, a[0])?;
                let path = self.sqlite_text(n, a[1], true)?;
                let path = self.sqlite_path(&path)?;
                Ok(self.sqlite_request(
                    n,
                    &state,
                    "backup_open",
                    Box::new(move |engine| engine.backup_open(&path)),
                )?)
            }),
            DatabaseSqliteBackupStep => future!({
                let state = self.sqlite_connection(n, a[0])?;
                let backup = self.sqlite_int(n, a[1])?;
                let pages = self
                    .integer(a[2])?
                    .to_i64()
                    .filter(|pages| i32::try_from(*pages).is_ok())
                    .unwrap_or(-1);
                Ok(self.sqlite_request(
                    n,
                    &state,
                    "backup_step",
                    Box::new(move |engine| engine.backup_step(backup, pages)),
                )?)
            }),
            DatabaseSqliteScript | DatabaseSqliteControl => future!({
                let state = self.sqlite_connection(n, a[0])?;
                let sql = self.sqlite_text(n, a[1], false)?;
                let script = n == DatabaseSqliteScript;
                let operation: Operation = if script {
                    Box::new(move |engine| engine.script(&sql))
                } else {
                    Box::new(move |engine| {
                        let allowed = matches!(
                            sql.as_str(),
                            "BEGIN DEFERRED"
                                | "BEGIN IMMEDIATE"
                                | "BEGIN EXCLUSIVE"
                                | "COMMIT"
                                | "ROLLBACK"
                        );
                        if !allowed {
                            return Err(Stop::Unexpected);
                        }
                        engine.execution(&sql, &[], true)
                    })
                };
                Ok(self.sqlite_request(
                    n,
                    &state,
                    if script { "script" } else { "control" },
                    operation,
                )?)
            }),
            DatabaseSqliteFetch => future!({
                let state = self.sqlite_connection(n, a[0])?;
                let invalid = || {
                    ProviderError::Native(fail(
                        n,
                        "invalid_input",
                        "Invalid SQLite fetch coordinates",
                    ))
                };
                let id = self
                    .integer(a[1])?
                    .to_i64()
                    .filter(|id| *id >= 1)
                    .ok_or_else(invalid)?;
                let maximum = self
                    .integer(a[2])?
                    .to_i64()
                    .filter(|m| (1..=64).contains(m))
                    .ok_or_else(invalid)?;
                Ok(self.sqlite_request(
                    n,
                    &state,
                    "fetch",
                    Box::new(move |engine| engine.fetch(id, maximum)),
                )?)
            }),
            DatabaseSqliteClose => {
                let state = self.sqlite_state(n, a[0])?;
                let mut connection = state.borrow_mut();
                if connection.handle_closed || connection.stopped {
                    return Ok(yes);
                }
                connection.stopped = true;
                if let Some(worker) = &mut connection.worker {
                    if let Some(jobs) = worker.jobs.take() {
                        // Closing at shutdown: a refusal is no longer observable.
                        let _ = jobs.send(Box::new(|engine| {
                            let _ = engine.close();
                        }));
                        // Dropping the queue lets the worker exit after closing.
                    }
                }
                Ok(yes)
            }
            DatabaseSqliteClosed => {
                let state = self.sqlite_state(n, a[0])?;
                let connection = state.borrow();
                Ok(Slot::boolean(
                    connection.handle_closed || connection.stopped || connection.broken(),
                ))
            }
            DatabaseSqliteSettled => {
                let state = self.sqlite_state(n, a[0])?;
                Ok(self.sqlite_settled(state)?)
            }
            DatabaseSqliteAutocommit => {
                let state = self.sqlite_connection(n, a[0])?;
                let autocommit = state.borrow().shared.autocommit.load(Ordering::SeqCst);
                Ok(Slot::boolean(autocommit))
            }
            DatabaseSqliteReplyOk => Ok(Slot::boolean(self.sqlite_reply(n, a[0])?.ok)),
            DatabaseSqliteReplyId => {
                let reply = self.sqlite_reply(n, a[0])?;
                let id = reply.data.borrow().id.unwrap_or(0);
                Ok(Slot::small(id))
            }
            DatabaseSqliteReplyEof => {
                Ok(Slot::boolean(self.sqlite_reply(n, a[0])?.data.borrow().eof))
            }
            DatabaseSqliteReplyCount => {
                let reply = self.sqlite_reply(n, a[0])?;
                let count = reply.data.borrow().rows.as_ref().map_or(0, Vec::len);
                Ok(Slot::small(count as i64))
            }
            DatabaseSqliteReplyAffectedKnown => {
                let reply = self.sqlite_reply(n, a[0])?;
                let known = matches!(reply.data.borrow().affected, Some(Some(_)));
                Ok(Slot::boolean(known))
            }
            DatabaseSqliteReplyAffected => {
                let reply = self.sqlite_reply(n, a[0])?;
                let affected = reply
                    .data
                    .borrow()
                    .affected
                    .clone()
                    .flatten()
                    .unwrap_or_else(|| "0".into());
                Ok(self.allocate_text(affected)?)
            }
            DatabaseSqliteReplyInfo => {
                let reply = self.sqlite_reply(n, a[0])?;
                let flag = |value: bool| if value { "1" } else { "0" }.to_string();
                let entries = {
                    let data = reply.data.borrow();
                    [
                        ("ok", flag(reply.ok)),
                        ("code", data.code.clone().unwrap_or_default()),
                        ("kind", data.kind.clone().unwrap_or_default()),
                        (
                            "affected_known",
                            flag(matches!(data.affected, Some(Some(_)))),
                        ),
                        (
                            "affected",
                            data.affected
                                .clone()
                                .flatten()
                                .unwrap_or_else(|| "0".into()),
                        ),
                        ("id", data.id.unwrap_or(0).to_string()),
                        ("eof", flag(data.eof)),
                        ("rows", data.rows.as_ref().map_or(0, Vec::len).to_string()),
                        ("autocommit", flag(data.autocommit.unwrap_or(true))),
                        ("size", data.size.unwrap_or(0).to_string()),
                        ("done", flag(data.done)),
                        ("busy", flag(data.busy)),
                        ("copies", data.copies.unwrap_or(0).to_string()),
                        ("total", data.total.unwrap_or(0).to_string()),
                    ]
                };
                let mut slots = Vec::with_capacity(entries.len());
                for (key, value) in entries {
                    slots.push((self.allocate_text(key.into())?, self.allocate_text(value)?));
                }
                Ok(self.map_new(slots.into_iter())?)
            }
            DatabaseSqliteReplyNames | DatabaseSqliteReplyDeclarations => {
                let reply = self.sqlite_reply(n, a[0])?;
                let values = if !reply.ok {
                    None
                } else if n == DatabaseSqliteReplyNames {
                    reply.data.borrow().names.clone()
                } else {
                    reply.data.borrow().declarations.clone()
                };
                Ok(self.sqlite_texts(values.unwrap_or_default())?)
            }
            DatabaseSqliteReplyCells => {
                use crate::native_db::Cell as DbCell;
                let reply = self.sqlite_reply(n, a[0])?;
                let at = self.integer(a[1])?.to_i64();
                let cells: Vec<DbCell> = {
                    let data = reply.data.borrow();
                    let rows = data.rows.as_ref().filter(|_| reply.ok).ok_or_else(|| {
                        ProviderError::Native(fail(n, "invalid_input", "missing SQLite rows"))
                    })?;
                    let at = at
                        .and_then(|i| usize::try_from(i).ok())
                        .filter(|i| *i < rows.len())
                        .ok_or_else(|| {
                            ProviderError::Native(fail(
                                n,
                                "invalid_input",
                                "SQLite row outside bounds",
                            ))
                        })?;
                    rows[at]
                        .iter()
                        .map(|cell| match cell {
                            Cell::Null => DbCell::Null,
                            Cell::Integer(v) => DbCell::Integer(Rc::new((*v).into())),
                            Cell::Floating(v) => DbCell::Floating(*v),
                            Cell::Text(v) => DbCell::Text(Rc::from(v.as_str())),
                            Cell::Bytes(v) => DbCell::Bytes(Rc::from(v.as_slice())),
                        })
                        .collect()
                };
                let values = cells
                    .into_iter()
                    .map(|cell| self.db_cell_new(cell))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(self.allocate_array(values)?)
            }
            DatabaseSqliteReplyKind | DatabaseSqliteReplyText | DatabaseSqliteReplyBytes => {
                let reply = self.sqlite_reply(n, a[0])?;
                let (row, column) = (self.integer(a[1])?.to_i64(), self.integer(a[2])?.to_i64());
                let cell = {
                    let data = reply.data.borrow();
                    let rows: &[Vec<Cell>] = if reply.ok {
                        data.rows.as_deref().unwrap_or(&[])
                    } else {
                        &[]
                    };
                    let outside = || {
                        ProviderError::Native(fail(
                            n,
                            "invalid_input",
                            "SQLite reply cell outside bounds",
                        ))
                    };
                    let row = row
                        .and_then(|r| usize::try_from(r).ok())
                        .filter(|r| *r < rows.len())
                        .ok_or_else(outside)?;
                    let column = column
                        .and_then(|c| usize::try_from(c).ok())
                        .filter(|c| *c < rows[row].len())
                        .ok_or_else(outside)?;
                    rows[row][column].clone()
                };
                match n {
                    DatabaseSqliteReplyKind => Ok(Slot::small(match cell {
                        Cell::Null => 0,
                        Cell::Integer(_) => 1,
                        Cell::Floating(_) => 2,
                        Cell::Text(_) => 3,
                        Cell::Bytes(_) => 4,
                    })),
                    DatabaseSqliteReplyText => match cell {
                        Cell::Bytes(_) => Err(ProviderError::Native(fail(
                            n,
                            "invalid_input",
                            "SQLite cell is binary",
                        ))),
                        Cell::Null => Ok(self.allocate_text(String::new())?),
                        Cell::Integer(value) => Ok(self.allocate_text(value.to_string())?),
                        Cell::Floating(value) => {
                            Ok(self.allocate_text(crate::scalar::float_text(value))?)
                        }
                        Cell::Text(value) => Ok(self.allocate_text(value)?),
                    },
                    _ => match cell {
                        Cell::Bytes(data) => Ok(self.native_byte_array(data)?),
                        _ => Err(ProviderError::Native(fail(
                            n,
                            "invalid_input",
                            "SQLite cell is not binary",
                        ))),
                    },
                }
            }
            DatabaseSqliteReplyBlobBytes => {
                let reply = self.sqlite_reply(n, a[0])?;
                let data = if reply.ok {
                    reply.data.borrow().bytes.clone().unwrap_or_default()
                } else {
                    Vec::new()
                };
                Ok(self.native_byte_array(data)?)
            }
            DatabaseSqliteReplyExecutions => {
                let reply = self.sqlite_reply(n, a[0])?;
                let entries = if reply.ok {
                    reply.data.borrow().executions.clone().unwrap_or_default()
                } else {
                    Vec::new()
                };
                if entries.len() > 256 {
                    return Err(ProviderError::Native(fail(
                        n,
                        "invalid_data",
                        "SQLite script reply limit",
                    )));
                }
                Ok(self.sqlite_texts(
                    entries
                        .into_iter()
                        .map(|affected| affected.unwrap_or_else(|| "unknown".into())),
                )?)
            }
            DatabaseSqliteReplyClose => {
                let reply = self.sqlite_reply(n, a[0])?;
                reply.closed.set(true);
                // Future outcomes can outlive the handle; closing releases their payload.
                *reply.data.borrow_mut() = Data::default();
                Ok(yes)
            }
            _ => Err(ProviderError::Engine(Error(format!(
                "unsupported native provider {} in new engine",
                n.symbol()
            )))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::future::State;
    use crate::native_db::Cell as DbCell;

    fn call(heap: &mut Heap, n: Native, a: &[Slot]) -> Slot {
        match heap.native_sqlite(n, a).unwrap() {
            Outcome::Value(v) => v,
            Outcome::Failure(_) => panic!("unexpected failure: {}", n.symbol()),
        }
    }

    /// Drive a native I/O Future to completion; replies come from the worker.
    fn wait(heap: &mut Heap, future: Slot) -> Result<Slot, Slot> {
        let request = heap.future(future).unwrap().io.clone().unwrap();
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let started = std::time::Instant::now();
        while request.poll(&mut context).is_pending() {
            assert!(
                started.elapsed().as_secs() < 10,
                "SQLite worker did not reply"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        heap.native_io_settle(future).unwrap();
        match heap.future(future).unwrap().state {
            State::Ready(result) => result,
            _ => panic!("settled Future is not ready"),
        }
    }

    fn reply(heap: &mut Heap, n: Native, a: &[Slot]) -> Slot {
        let future = call(heap, n, a);
        let reply = wait(heap, future).expect("SQLite reply");
        assert!(
            call(heap, Native::DatabaseSqliteReplyOk, &[reply]).bits != 0,
            "{} failed",
            n.symbol()
        );
        reply
    }

    fn text(heap: &mut Heap, value: &str) -> Slot {
        heap.allocate_text(value.into()).unwrap()
    }

    fn cells(heap: &mut Heap, values: Vec<DbCell>) -> Slot {
        let values = values
            .into_iter()
            .map(|cell| heap.db_cell_new(cell).unwrap())
            .collect();
        heap.allocate_array(values).unwrap()
    }

    fn open(heap: &mut Heap) -> Slot {
        let path = heap.native_byte_array(Vec::new()).unwrap();
        let journal = text(heap, "memory");
        let state = call(
            heap,
            Native::DatabaseSqliteCreate,
            &[
                path,
                Slot::boolean(false),
                Slot::boolean(true),
                Slot::boolean(true),
                Slot::small(0),
                journal,
                Slot::boolean(true),
            ],
        );
        reply(heap, Native::DatabaseSqliteConnect, &[state]);
        state
    }

    #[test]
    fn rows_reach_the_program_as_typed_cells_through_prepared_statements() {
        let mut h = Heap::new(&[], &[]);
        let state = open(&mut h);
        let none = cells(&mut h, vec![]);
        let ddl = text(
            &mut h,
            "CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT, score REAL, data BLOB)",
        );
        let created = reply(
            &mut h,
            Native::DatabaseSqliteExecutionCells,
            &[state, ddl, none],
        );
        assert_eq!(
            call(&mut h, Native::DatabaseSqliteReplyAffectedKnown, &[created]).bits,
            0
        );
        let sql = h
            .native_byte_array(b"INSERT INTO t VALUES(?,?,?,?)".to_vec())
            .unwrap();
        let prepared = reply(&mut h, Native::DatabaseSqlitePrepare, &[state, sql]);
        let id = call(&mut h, Native::DatabaseSqliteReplyId, &[prepared]);
        let first = cells(
            &mut h,
            vec![
                DbCell::Integer(Rc::new(1.into())),
                DbCell::Text("я😀".into()),
                DbCell::Floating(1.5),
                DbCell::Bytes(Rc::from(&[0u8, 255][..])),
            ],
        );
        let second = cells(
            &mut h,
            vec![
                DbCell::Integer(Rc::new(2.into())),
                DbCell::Null,
                DbCell::Null,
                DbCell::Null,
            ],
        );
        for values in [first, second] {
            let inserted = reply(
                &mut h,
                Native::DatabaseSqlitePreparedExecutionCells,
                &[state, id, values],
            );
            let affected = call(&mut h, Native::DatabaseSqliteReplyAffected, &[inserted]);
            assert_eq!(h.text(affected).unwrap(), "1");
        }
        let query = text(&mut h, "SELECT id, name, score, data FROM t ORDER BY id");
        let opened = reply(
            &mut h,
            Native::DatabaseSqliteRowsCells,
            &[state, query, none],
        );
        let names = call(&mut h, Native::DatabaseSqliteReplyNames, &[opened]);
        let names: Vec<_> = h.elements(names).unwrap().to_vec();
        assert_eq!(h.text(names[1]).unwrap(), "name");
        let cursor = call(&mut h, Native::DatabaseSqliteReplyId, &[opened]);
        let batch = reply(
            &mut h,
            Native::DatabaseSqliteFetch,
            &[state, cursor, Slot::small(64)],
        );
        assert_eq!(
            call(&mut h, Native::DatabaseSqliteReplyCount, &[batch]).bits,
            2
        );
        assert_ne!(
            call(&mut h, Native::DatabaseSqliteReplyEof, &[batch]).bits,
            0
        );
        let row = call(
            &mut h,
            Native::DatabaseSqliteReplyCells,
            &[batch, Slot::small(0)],
        );
        let row: Vec<_> = h.elements(row).unwrap().to_vec();
        assert!(matches!(h.db_cell(row[0]).unwrap(), DbCell::Integer(v) if **v == 1.into()));
        assert!(matches!(h.db_cell(row[1]).unwrap(), DbCell::Text(v) if &**v == "я😀"));
        assert!(matches!(h.db_cell(row[2]).unwrap(), DbCell::Floating(v) if *v == 1.5));
        assert!(matches!(h.db_cell(row[3]).unwrap(), DbCell::Bytes(v) if **v == [0, 255]));
        let row = call(
            &mut h,
            Native::DatabaseSqliteReplyCells,
            &[batch, Slot::small(1)],
        );
        let row: Vec<_> = h.elements(row).unwrap().to_vec();
        assert!(matches!(h.db_cell(row[1]).unwrap(), DbCell::Null));
        assert!(matches!(
            h.native_sqlite(Native::DatabaseSqliteReplyCells, &[batch, Slot::small(2)])
                .unwrap(),
            Outcome::Failure(_)
        ));
        call(&mut h, Native::DatabaseSqliteReplyClose, &[batch]);
        assert!(matches!(
            h.native_sqlite(Native::DatabaseSqliteReplyCount, &[batch])
                .unwrap(),
            Outcome::Failure(_)
        ));
        call(&mut h, Native::DatabaseSqliteUnprepare, &[state, id]);
        call(&mut h, Native::DatabaseSqliteClose, &[state]);
        assert_ne!(call(&mut h, Native::DatabaseSqliteClosed, &[state]).bits, 0);
    }

    #[test]
    fn transactions_failures_and_cancellation_keep_the_connection_usable() {
        let mut h = Heap::new(&[], &[]);
        let state = open(&mut h);
        let none = cells(&mut h, vec![]);
        let begin = h.native_byte_array(b"BEGIN DEFERRED".to_vec()).unwrap();
        reply(&mut h, Native::DatabaseSqliteControl, &[state, begin]);
        assert_eq!(
            call(&mut h, Native::DatabaseSqliteAutocommit, &[state]).bits,
            0
        );
        let rollback = h.native_byte_array(b"ROLLBACK".to_vec()).unwrap();
        reply(&mut h, Native::DatabaseSqliteControl, &[state, rollback]);
        assert_ne!(
            call(&mut h, Native::DatabaseSqliteAutocommit, &[state]).bits,
            0
        );
        // A rejected statement is an ordinary reply carrying kind and code.
        let invalid = text(&mut h, "SELEKT 1");
        let future = call(
            &mut h,
            Native::DatabaseSqliteRowsCells,
            &[state, invalid, none],
        );
        let rejected = wait(&mut h, future).unwrap();
        assert_eq!(
            call(&mut h, Native::DatabaseSqliteReplyOk, &[rejected]).bits,
            0
        );
        let info = call(&mut h, Native::DatabaseSqliteReplyInfo, &[rejected]);
        let kind = text(&mut h, "kind");
        let kind = h
            .map_op(crate::map::MapOp::Get, [info, kind, Slot::default()])
            .unwrap();
        assert_eq!(h.text(kind).unwrap(), "syntax");
        // Cancelling an opened cursor waits for the worker and releases it.
        let slow = text(
            &mut h,
            "WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i+1 FROM n WHERE i<100000) SELECT sum(i) FROM n",
        );
        let future = call(
            &mut h,
            Native::DatabaseSqliteRowsCells,
            &[state, slow, none],
        );
        let request = h.future(future).unwrap().io.clone().unwrap();
        assert!(request.request_cancel());
        assert!(wait(&mut h, future).is_err());
        let again = text(&mut h, "SELECT 1");
        let opened = reply(
            &mut h,
            Native::DatabaseSqliteRowsCells,
            &[state, again, none],
        );
        let cursor = call(&mut h, Native::DatabaseSqliteReplyId, &[opened]);
        let batch = reply(
            &mut h,
            Native::DatabaseSqliteFetch,
            &[state, cursor, Slot::small(1)],
        );
        assert_eq!(
            call(&mut h, Native::DatabaseSqliteReplyCount, &[batch]).bits,
            1
        );
        call(&mut h, Native::DatabaseSqliteClose, &[state]);
        let future = call(
            &mut h,
            Native::DatabaseSqliteRowsCells,
            &[state, again, none],
        );
        assert!(wait(&mut h, future).is_err());
    }
}
