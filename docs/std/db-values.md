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

`Column(index, name, declaration)` and `Row(columns, values)` preserve order and
duplicate names. Row width must agree with contiguous zero-based column indices;
maximum width is 1024 and aggregate row payload is bounded. `item(index)` uses a
checked position; `value(name)` rejects missing or ambiguous exact names. A row is
an immutable observation after fetching; the asynchronous row reader remains an
owned infrastructure capability.

`db/read` provides `RowReader(row)` for strict named projections:

```okkama
import "db/read" take RowReader;

let fields = RowReader(row);
let id = fields.int("id");
let name = fields.text("name");
let verified = fields.flag("verified");
```

`int(name)` accepts only an integer within -9223372036854775808 through
9223372036854775807, checked as `BigInt` before conversion. `text(name)` accepts
only text, including the empty string. `flag(name)` accepts only integer 0 or 1;
boolean values are a distinct category, although SQLite stores boolean bindings
as integers. These methods never coerce textual numbers or replace NULL.

Missing and ambiguous names retain `MissingColumn` and `AmbiguousColumn`.
Projection failures throw `RowValueFailure`, exposing `column()`, `expected()`
("int", "text" or "flag"), `actual_kind()` and `cause()`. Its cause is
`WrongDbValue` for the wrong category, or `InvalidDbValue` for an integer outside
the requested range. The runtime also preserves the failure chain. Diagnostics
name the column and requested projection without printing the payload.
Domain constructors remain explicit and retain their own validation.

Focused checks (Perl 5.42, from `okkama-pl`):

```sh
prove -l t/interpreter/618_std_db_read.t t/interpreter/619_std_db_read_sqlite.t
```

`Execution(affected)` supplies an exact nonnegative `BigInt`; `Execution()` means
unknown and its `affected_rows()` throws `AffectedRowsUnknown`. Row-producing SQL,
including RETURNING, belongs to rows rather than an invented generic insert id.

Portable implementation evidence: two focused files /12 assertions PASS, including retained/malformed printer and NUL tests. Inline/example checks accompany this slice. [Owned SQLite connections/transactions/workers](db-sqlite.md) implement the I/O layer; this portable slice does not implement SQL parsing or I/O.
