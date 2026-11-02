# Database values and portable statements: contract

The portable database boundary represents seven immutable SQL value categories:
NULL, boolean, arbitrary-precision integer, finite floating point, decimal, text
and bytes. It carries no domain-object reflection, implicit date/UUID/JSON
adapter, credentials, native handle or cursor state.

`DbValue` exposes `kind()` and `print(DbValueWriting)`. Printing must emit exactly
one event of the declared kind. This works within the language's role boundary;
a driver never downcasts to call a concrete value's private projection. Concrete
values expose a typed `value()` observation. `ObservedDbValue` validates and
captures one print, provides checked typed projections, and can return an
immutable `DbValue` snapshot. Wrong projections throw `WrongDbValue`; a malformed
printer throws `InvalidDbValue`. NULL remains its own value, not an empty item.

Boundaries cap individual text/binary payloads at 8 MiB, integer spellings at
4096 decimal characters, and decimal spellings at 4096 characters. Embedded NUL
is legal in data. Floating values must be finite. SQLite will additionally reject
integers outside signed 64-bit range, bind boolean as 0/1 and decimal as canonical
text, and return results by actual SQLite storage class rather than declaration.

`Statement(text)` or `Statement(text, Sequencing<DbValue>)` captures a bounded
immutable SQL text and value snapshot. SQL is at most 1 MiB of UTF-8, without NUL;
at most 1024 parameters and 8 MiB aggregate parameter representations. Placeholders
are driver-specific. The driver must prove exactly one executable statement using
its own parser; the portable object does not scan or replace question marks.

## Binding parameters

Use `Param` from `db/param` for every supported scalar:

```okkama
import "db/param" take Param;

let statement = Statement(
    "INSERT INTO users(id, name, nickname) VALUES(?, ?, ?)",
    [Param(id), Param(name), Param(nickname)]
);
await connection.execution(statement);
```

`Param(int)` converts exactly to the portable integer category without a manual
`BigInt` conversion. Other overloads accept `BigInt`, `Decimal`, `float`, `bool`,
`string` and `@byte`. `Param()` binds SQL NULL. Each type also has a
`Param(Maybe<T>)` overload: `.some(value)` binds the scalar, `.none` binds NULL.
For an untyped empty value, use `Param(Maybe<string>.none)` or simply `Param()`;
use `@byte()` to identify an empty binary payload.

These objects implement the existing `DbValue` protocol. They work in both
`Statement(sql, parameters)` and `prepared.rows(parameters)` /
`prepared.execution(parameters)`. Existing `DbText`, `DbInteger` and other
wrappers remain supported. Individual value validation and Statement/driver
snapshot limits remain in force. SQL text and data are never interpolated.
Driver-specific type behavior remains unchanged (for example SQLite booleans
are stored as INTEGER 0/1 and decimals bind as canonical text).

`Presence<T>` is deliberately not accepted: missing data must not silently
become a NULL assignment. The caller chooses whether to omit an update or
explicitly `.fold()` when treating missing as NULL is intended.

`Column(index, name, declaration)` and `Row(columns, values)` preserve order and
duplicate names. Row width must agree with contiguous zero-based column indices;
maximum width is 1024 and aggregate row payload is bounded. `item(index)` uses a
checked position; `value(name)` rejects missing or ambiguous exact names. A row is
an immutable observation after fetching; the asynchronous row reader remains an
owned infrastructure capability.

Read directly from a `Row`:

```okkama
let id = row.int("id");
let name = row.text("name");
let nickname = row.maybe("nickname").text() else "";
let verified = row.flag("verified");
```

Strict readers are `int`, `integer` (BigInt), `decimal` (Decimal), `floating`
(float), `boolean` (bool), `flag` (integer 0/1 to bool), `text` (string), and
`bytes` (@byte). `RowReader(row)` from `db/read` remains a compatibility adapter
with the same strict methods.

`row.maybe(name)` returns `DbMaybeReader`: its argument-free typed methods
return `Maybe<T>`. Only SQL NULL becomes `Maybe.none`. A missing column still
throws `MissingColumn`; an ambiguous name still throws `AmbiguousColumn`.
The `else` fallback is lazy, and never masks a type or range error.

`row.presence(name)` returns `DbPresenceReader`: the same methods return
`Presence<T>`, distinguishing a missing column from SQL NULL. Ambiguous names
remain errors. Use this explicitly when the query's result columns can vary:

```okkama
let label = row.presence("nickname").text()
    ?? .some(name): name
    !! .none: "SQL NULL"
    !! .missing: "Column not selected";
```

Both readers also offer `value()` for `Maybe<DbValue>` / `Presence<DbValue>`.
Empty text, zero, false and empty bytes stay present. To collapse Presence,
call `.fold()` explicitly before `else`.

`int(name)` accepts only an integer within -9223372036854775808 through
9223372036854775807, checked as `BigInt` before conversion. `text(name)` accepts
only text, including the empty string. `flag(name)` accepts only integer 0 or 1;
boolean values are a distinct category, although SQLite stores boolean bindings
as integers. These methods never coerce textual numbers or replace NULL.

Missing and ambiguous names retain `MissingColumn` and `AmbiguousColumn`.
Projection failures throw `RowValueFailure`, exposing `column()`, `expected()`
(the requested reader name), `actual_kind()` and `cause()`. Its cause is
`WrongDbValue` for the wrong category, or `InvalidDbValue` for an integer outside
the requested range. The runtime also preserves the failure chain. Diagnostics
name the column and requested projection without printing the payload.
Domain constructors remain explicit and retain their own validation.

Focused checks (Perl 5.42, from `okkama-pl`):

```sh
prove -l t/interpreter/618_std_db_read.t t/interpreter/619_std_db_read_sqlite.t
```

`Execution(affected)` supplies an exact nonnegative `BigInt`; `Execution()` means
unknown. `affected() is Maybe<BigInt>` returns `.some(count)` (including zero)
or `.none`. Use `execution.affected() else fallback` or match explicitly. The
legacy `affected_known()` and strict `affected_rows()` remain compatible; the
latter throws `AffectedRowsUnknown` when the count is unknown. Row-producing SQL,
including RETURNING, belongs to rows rather than an invented generic insert id.

Portable implementation evidence: two focused files /12 assertions PASS, including retained/malformed printer and NUL tests. Inline/example checks accompany this slice. [Owned SQLite connections/transactions/workers](db-sqlite.md) implement the I/O layer; this portable slice does not implement SQL parsing or I/O.

## Single-row queries

```okkama
import "db/query" take Query, NoRows, TooManyRows;

let query = Query(connection, Statement("SELECT id, name FROM users WHERE id = ?", parameters));
let row = await query.one();       // Row: exactly one result
let found = await query.maybe();   // Maybe<Row>: zero or one result
```

`Query` takes a `DbExecuting` capability and a `Statement`. SQLite/PostgreSQL
connections and transactions implementing that role can be used. Each method
call executes the statement anew; the object is not a cached result or a
prepared statement. A capability confined to another role must expose
`DbExecuting` through its existing capability boundary; Query does not downcast.

| Cardinality | `one()` | `maybe()` |
|---|---|---|
| Zero rows | `NoRows` | `Maybe.none` |
| One row | `Row` | `Maybe.some(row)` |
| Two or more rows | `TooManyRows` | `TooManyRows` |

SQL NULL in a cell never means absence of the row. Read nullable cells through
`row.maybe(name)` as described above. Query errors propagate; only the cursor's
explicit `RowsEnded` is converted to absence. The implementation requests at
most two rows, though drivers may internally fetch batches.

The query owns and closes its acquired cursor using `defer`, including on
cardinality and fetch errors. It does not close the caller's connection, commit,
or roll back a transaction. Connection cleanup remains the caller's responsibility.
A fetch error followed by a close error becomes `QueryCleanupFailure` with
`cause()` and `cleanup()` preserving both errors. Explicit close failures remain
visible. `close()` is terminal/idempotent even if the underlying close fails;
release of a broken underlying resource cannot be guaranteed by this wrapper.
Cancellation closes the cursor and propagates; a simultaneous cancellation and
close failure follows the language's ordinary cleanup error semantics.

## Collections and streaming

```okkama
let users = await query.all(100); // @Row, in result order; never silently truncated
```

`all(max_rows)` requires an explicit non-negative limit. Negative limits throw
`InvalidQueryLimit` before executing SQL. Zero permits only an empty result.
An extra row throws `QueryLimitExceeded`, whose `limit()` reports the limit.
At most `max_rows + 1` rows are requested; the extra row detects overflow.
The cursor closes on success, overflow and fetch errors. Rows accumulate through
a persistent list and are materialized as an array in source order, without
copying the growing array on each append. The limit counts rows, not bytes:
large payloads can still use substantial memory. Prefer streaming for large results.

```okkama
let rows = await query.rows(); // QueryRows owns the acquired cursor
defer rows.close();
for await rows -> row {
    Stdout().writeln(row.text("name"));
}
```

`next()` returns `Maybe<Row>`; only EOF becomes `.none`. EOF closes the cursor,
and repeated `next()` calls after EOF return `.none` without fetching again.
Fetch errors close the cursor and propagate. `close()` is idempotent. Explicitly
closing before EOF makes subsequent `next()` throw `DbClosed`. Concurrent
`next()` calls are rejected with `DbBusy`; use a single consumer. `columns()`
exposes the driver's column metadata; `closed()` reports cursor state.

Keep `defer rows.close()` even though EOF closes automatically: early loop exit
and failures in the consumer must release the cursor too. The connection stays
owned by the caller. Existing `connection.rows(statement)` remains available
for the low-level `RowReading` protocol. See [example](../../examples/db-query.ok).

## Prepared queries

The same `Query` interface accepts a prepared statement and a parameter sequence:

```okkama
let prepared = await Prepared(connection, "SELECT name FROM users WHERE id=?").statement();
defer prepared.close();
let query = Query(prepared, [Param(id)]);
let user = await query.maybe();
// query.one(), query.all(limit), query.rows() have identical contracts.
```

Parameters are validated and snapshotted when the Query is constructed. Each
method call executes the prepared statement again. Query closes its result
cursor, not the prepared statement. The caller owns preparation and its connection
or transaction. Pool lease/transaction revocation is still enforced by the driver.

The lifetime conformance tests cover cancellation while awaiting a row, overlapping
`next()` calls, explicit close during a pending read, and failures in cleanup.
A row completing after close is rejected as `DbClosed`. SQLite pool tests cover
returning an unfinished transaction, revocation of prepared statements/readers,
rollback before reuse, and reuse after query cardinality/limit errors.


## Нативное хранение значений и строк

`Row` хранит проверенные неизменяемые ячейки в `runtime/db`. Схема колонок
и поиск имён разделяются между строками одного результата. Строгое чтение,
`maybe` и `presence` используют эти ячейки без повторного вызова `DbValue.print`.
Публичные вызовы и ошибки MissingColumn/AmbiguousColumn/RowValueFailure сохранены.

Пользовательский `DbValue` по-прежнему проходит проверку: ровно одно событие
объявленного вида, ограничения размера, затем закрытие приёмника для записи.
Полученный снимок не зависит от дальнейшего поведения пользовательского объекта.
`value()` и `item()` предоставляют совместимый `DbValue` для существующего кода.
`int()` по-прежнему требует диапазон signed 64-bit, `integer()` хранит точное
целое произвольной точности в пределах DB-лимитов.

SQLite передаёт числа в нативные ячейки без преобразования в текст; возвращённые
строки сохраняют данные после закрытия пакета, курсора и соединения. PostgreSQL
использует общие нативные ячейки и схему; его сетевой протокол и декодирование
текстового ответа пока остаются в `std/db/postgres`.

IR и Sparrow исполняют общий Rust-provider. JVM использует эквивалентный
provider JVM, как и JSON. Это не подключение Rust-provider через JNI.
