# SQLite: owned database capabilities

`SqliteMemory().connection()` and `SqliteDatabase(Path, SqliteOptions).connection()`
return a concrete `SqliteConnection`. Each acquisition owns one POSIX worker and
SQLite connection. Memory acquisitions are independent databases. Windows reports
unsupported; the portable database contracts and external connectors remain usable.
The provider needs the system SQLite library, version 3.41 or newer (`libsqlite3`;
macOS ships one, on Debian/Ubuntu it is `libsqlite3-0`, 3.41+ from Ubuntu 24.04 and
Debian 13). The library is loaded on the first connection; an older one reports
`unsupported` with code `SQLITE_VERSION`. Provider loading itself does not acquire a
database.

```okkama
let connection=await SqliteMemory().connection();
defer await connection.settled();
defer connection.close();
await connection.execution(Statement("CREATE TABLE customers(id INTEGER,name TEXT)"));
await connection.execution(Statement("INSERT INTO customers VALUES(?,?)",
    [DbInteger(BigInt("7")),DbText("Ada")]));
let rows=await connection.rows(Statement("SELECT name FROM customers WHERE id=?",
    [DbInteger(BigInt("7"))]));
let row=await rows.row();
rows.close();
ObservedDbValue(row.value("name")).text();
```

Imports and a complete runnable example are in
[database-stack.ok](../../examples/std-v4/database-stack.ok). Run from repository
root with `OKKAMA_STDLIB_DIR=std perl okkama-pl/bin/okkama examples/std-v4/database-stack.ok`.

## Portable and concrete capabilities

`DbExecuting` has execution/rows; `DbConnection` adds transaction, closed, close
and asynchronous settled. Role boundaries restrict the receiver to those actions.
Concrete SQLite capabilities implement separate preparation, BLOB, script and
backup roles; passing the portable role does not expose these extra methods.
`SqliteConnector(SqliteDatabase)` adapts a file connector to `DbConnecting`.
Independent memory databases deliberately have no built-in pool adapter.

Statement values are immutable bounded observations passed through a typed print
role. SQL never reflects on a domain object or interpolates its private fields.
An application mapper may bind an explicitly supplied domain view into Statement;
the domain object does not need to own a storage or infrastructure dependency.
The wire representation is explicit at the infrastructure boundary.

One physical connection serializes requests and permits one active reader or
incremental BLOB. Unread rows and incompatible transaction ownership fail with
DbBusy. Fetches contain at most eight rows and 8 MiB aggregate value representations.
A next valid row which would exceed the batch byte budget is retained for the
next fetch; at most one extra8 MiB row is buffered in the worker. Each individual
row remains bounded, and large rows are not truncated or rejected merely because
two of them cannot share one batch.
Row and Column are immutable copies with ordered duplicate names. RowsEnded is
exhaustion; close discards remaining rows. Connection close revokes all dependent
capabilities, schedules interruption/resource release and rolls back uncommitted
work. `settled()` joins cleanup; use the shown defer order.

## SQL and SQLite configuration

SQLite prepare-tail certifies exactly one statement before execution. Native
bind slots determine parameter count; values bind as data, including NUL in TEXT
and BLOB. SQL is limited to 1 MiB, parameters to 1024/8 MiB, columns to1024, and
prepared statements to64 per connection. Driver frames have a separate16 MiB cap.
Signed64 INTEGER is exact; DbBoolean binds as0/1 and DbDecimal as canonical TEXT.
Results follow actual storage class; column affinity may convert stored values.
Row-producing SQL, including RETURNING, requires rows. DDL counts are unknown;
there is no universal last-insert identifier. SQLite failures expose extended numeric codes; driver validation uses stable
SQLITE_* names. Messages are safe generic diagnostics, never sqlite3_errmsg/SQL/bindings.

SqliteOptions defaults to read/write/create, foreign keys on, zero busy wait and
verified DELETE journal for files/MEMORY for memory databases. Explicit journal
profile: default_mode, delete_mode, wal, memory. Busy timeout is integral milliseconds
from0 through60000. Read-only plus create is rejected; read-only memory acquisition
is rejected because SQLite cannot enforce that opening promise. Paths are literal
validated UTF-8 paths; SQLite URI options are not implicit. Raw PRAGMA, ATTACH,
DETACH, transaction/savepoint control are rejected; reviewed configuration and
transaction capabilities use distinct native commands. Loadable extensions stay
explicitly disabled or verified absent in an OMIT_LOAD_EXTENSION build.

## Transactions and preparation

`connection.transaction(TransactionOptions())` returns portable Transaction.
`SqliteTransactions(connection, SqliteTransactionMode.immediate).transaction(options)`
returns concrete SqliteTransaction for BLOB/scripts; deferred/exclusive are also
available. Default/serializable isolation is supported. Other isolation, deferrable,
and read-only promises without a physically read-only connection are rejected.
Only the transaction capability can issue its operations; ordinary connection
operations and nested transactions fail while it owns the connection.

Confirmed commit/rollback is terminal and repeatable only for the same outcome.
Lexical abandonment queues rollback before subsequent work. A cancelled or lost
COMMIT acknowledgment closes the physical connection and records an indeterminate
outcome; a later commit throws IndeterminateCommit. The cancellation caller retains
its cancellation/TimedOut channel. A confirmed busy/constraint failure does not
pretend commit succeeded. Transaction bodies are never retried implicitly.

`Prepared(connection,text).statement()` uses the DbPreparing capability and returns
PreparedStatement. It retains the originating connection, resets and clears all
bindings after success/failure/early reader close, and excludes unread reuse.
Prepared close also revokes buffered rows. Concrete SqliteTransaction implements
DbPreparing; its compiled statements and buffered readers are revoked at transaction
end. Reader close cancels a pending fetch before joining release, rather than
waiting for an abandoned long query. SQLite automatic schema recompilation
which changes column count/name/declaration throws SQLITE_SCHEMA_CHANGED before
publishing values. Reuse after that reset observes refreshed metadata; it cannot
silently drop new columns or attach old names to changed results.

## Incremental BLOB, scripts and backup

`OpenedSqliteBlob(connectionOrConcreteTransaction,database,table,column,row,writable)`
uses SQLite's identifier-taking BLOB API. No identifier becomes executable SQL.
Reads/writes are capped at65536 bytes and checked against fixed size with exact
BigInt offsets. Writes require a writable handle. Commit requires explicit BLOB
close; rollback and connection shutdown revoke it. Modifying its row externally
can expire the native BLOB and produce a typed failure.

`SqliteScript(connectionOrConcreteTransaction,source).executions()` uses prepare-tail,
including quotes/comments/trigger bodies. At most1 MiB/256 statements; row-producing
statements are rejected. It stops at first failure, preserves previous effects
under the caller's current transaction, and never commits implicitly. Transaction
control in raw scripts is rejected too.

`SqliteBackup(source,targetPath,pages,clock,timeout).result()` opens an owned file
destination in the source's worker. Existing connection handles cannot cross the
worker boundary, so the v3 two-existing-connections candidate is superseded.
The target is an ordinary read/write/create file connection owned only by backup.
Known same-file/alias destinations are rejected. Each step copies1..256 pages and
yields; busy retries stay inside the explicit monotonic deadline. Both opening
and steps share that deadline. BackupReport copies/total are final page counts,
not bytes or a monotonic progress promise. Cancellation joins worker acknowledgment
and finishes the backup; unfinished target transactions are rolled back.

## Worker and measured limits

SQLite pointers, callbacks and allocator-owned memory stay in the owning worker.
Private generation/request identifiers and bounded owned scalar/byte copies cross
pollable nonblocking sockets. A separate control lane services interruption from
SQLite progress/busy callbacks. Internal acknowledgment futures cannot be cancelled
by a public waiter. Maintenance is joined before shutdown so closing a connection
cannot cancel its own queued release chain. Only the owned child PID is reaped.

All SQLite work is isolated; bounded framing/encoding and Okkama value observation
still consume parent CPU. Normal close joins asynchronously and has a force-abort
signal deadline; registry teardown retains a synchronous killed-child wait fallback.
The deadline cannot guarantee an OS task leaves a stuck kernel I/O syscall. These
limits and investigation directions are in
database performance report.
[Pool contract](db-pool.md) describes acquisition/reset/lifetime policies.

Primary references: [prepare-tail](https://www.sqlite.org/c3ref/prepare.html),
[progress handler](https://www.sqlite.org/c3ref/progress_handler.html),
[interrupt](https://www.sqlite.org/c3ref/interrupt.html),
[BLOB API](https://www.sqlite.org/c3ref/blob_open.html), and
[backup lifecycle](https://www.sqlite.org/c3ref/backup_finish.html).


For prepared pooling use DbPreparedPool(SqliteAcquiring(fileDatabase),options,clock).
Explicit DbAcquisition facets preserve preparation while the physical connection
stays behind DbConnection. PooledTransactions(lease) preserves preparation for a
concrete transaction; ordinary portable role acquisition retains its restrictions.
See [pool preparation](db-pool.md#explicit-preparation-facets).
