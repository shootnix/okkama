# PostgreSQL: a protocol driver written in Okkama

`db/postgres` speaks the PostgreSQL frontend/backend protocol 3.0 itself. The
host only provides TCP (`net/tcp`), TLS (`net/tls`) and cryptography
(`crypto/*`, i.e. the Rust host library). Framing, authentication, the query
conversation, value decoding and error mapping are Okkama code:

| module | contents |
| --- | --- |
| `db/postgres/wire` | message framing, `PgInput` (buffered message reader), `PgOutput` |
| `db/postgres/auth` | `ScramExchange` (SCRAM-SHA-256, RFC 5802/7677), `Md5Password` |
| `db/postgres` | connection, sessions, transactions, prepared statements, row streaming |

```okkama
let database = PostgresDatabase(
    PostgresTcp(IpEndpoint(IpAddress("127.0.0.1"), 5432)),
    PostgresCredentials("app", SecretBytes(Utf8Encoded("secret").bytes())),
    PostgresOptions("app"),
    SystemEntropy());
let connection = await database.connection();
defer connection.close();
let rows = await connection.rows(Statement("SELECT name FROM customers WHERE id = $1",
    [DbInteger(BigInt("7"))]));
let row = await rows.row();
rows.close();
ObservedDbValue(row.value("name")).text();
```

A complete program (transaction, prepared lookup, streamed report,
constraint failure) is [db-postgres.ok](../../examples/std-v4/db-postgres.ok).

## Connecting

`PostgresDatabase(transport, credentials, options, entropy)` is an immutable
connector. Every `connection()` opens a new server session:

- `PostgresTcp(IpEndpoint)` is plaintext TCP (keep-alive and no-delay on).
- `PostgresTls(IpEndpoint, CertificateIdentity, TlsClientPolicy)` sends
  `SSLRequest` and performs a verified TLS handshake against the given trust
  store and identity. A server that declines TLS is
  `DbFailure(connection, connecting, "PG_TLS_REFUSED")`. There is no
  plaintext fallback (no `sslmode=prefer`). Certificate failures surface as
  `CertificateVerificationFailed`.
- `PostgresCredentials(user)` has no password (trust/peer setups);
  `PostgresCredentials(user, SecretBytes)` holds one.
- `PostgresOptions(database)` uses application name `okkama`, 256 rows per
  fetch, and SCRAM only. `PostgresOptions(database, application, md5,
  cleartext, batch)` opts in to the legacy methods explicitly.
- `entropy` supplies the SCRAM client nonce: `SystemEntropy()` in programs,
  never `Entropy.fake()`.

The startup message fixes `client_encoding=UTF8`, `DateStyle=ISO`,
`bytea_output=hex` and `extra_float_digits=3`, so decoding never depends on
server defaults. `PostgresConnection.server_version()` reports the server's
`server_version`.

For pools and other portable consumers there are two adapters.
`PostgresConnector(database)` is a `DbConnecting` for `DbPool`, and
`PostgresAcquiring(database)` is a `DbAcquiring` for `DbPreparedPool`; it
hands the pool the connection's preparation capabilities too.

## Authentication

| server request | behavior |
| --- | --- |
| trust (AuthenticationOk) | accepted |
| SCRAM-SHA-256 | always enabled; the server's final signature is verified in constant time, and AuthenticationOk without it is a failure |
| MD5 | refused (`PG_AUTH_DISABLED`) unless `md5` is enabled |
| cleartext password | refused (`PG_AUTH_DISABLED`) unless `cleartext` is enabled |
| GSS, SSPI, Kerberos, SCRAM-PLUS only | `DbFailure(unsupported, connecting, "PG_AUTH_METHOD")` |

SCRAM uses no channel binding (`n,,`). The password goes in as UTF-8 bytes
without SASLprep, so a password that SASLprep would change must be stored
already normalized. A wrong password is the server's `28P01`, reported with
kind `permission`.

## Statements and parameters

Every statement runs through the extended query protocol (Parse/Bind/Execute).
Parameters therefore never become SQL text, and one `Statement` is exactly
one SQL command. Placeholders are PostgreSQL's own `$1..$n`; the driver does
not rewrite `?`.

Parameters are sent untyped (OID 0), and the server infers each type from its
context:

| `DbValue` | wire form |
| --- | --- |
| `DbNull` | NULL |
| `DbBoolean` | text `true` / `false` |
| `DbInteger`, `DbFloating`, `DbDecimal`, `DbText` | text literal |
| `DbBytes` | binary format |

A parameter without context (`SELECT $1`) is inferred as text, so cast it
explicitly (`$1::int8`).

`Execution.affected_rows()` is known for INSERT, UPDATE, DELETE, MERGE and
COPY command tags. Other commands (DDL, SELECT) leave it unknown.

## Results

Results arrive in text format and decode by column type:

| column type | value |
| --- | --- |
| `bool` | `DbBoolean` |
| `int2`, `int4`, `int8`, `oid` | `DbInteger` |
| `float4`, `float8` | `DbFloating` |
| `numeric` | `DbDecimal` |
| `bytea` | `DbBytes` |
| anything else (dates, json, uuid, arrays, enums...) | `DbText` |

NaN and infinite float/numeric values have no portable value, so they are
`DbFailure(data, fetching, "PG_NONFINITE")`. `Column.declaration()` is the type
name (`int4`, `text`, `timestamptz`...) or `oid:<n>` for other types.

`rows()` streams through an unnamed portal, `batch` rows per round trip. While
a reader is open its connection is busy: any other operation throws `DbBusy`.
Read to `RowsEnded` or call `close()`. A result that fits in the first batch
frees the connection immediately, while its rows stay readable. The open portal
is released lazily by the next operation, so `close()` is synchronous. The
same lazy rule applies to `PreparedStatement.close()` (a `Close` message
before the next request) and to a transaction dropped without commit or
rollback (a `ROLLBACK` before the next request).

## Transactions and preparation

`transaction(TransactionOptions)` issues a single `BEGIN` with the requested
isolation, `READ ONLY`/`READ WRITE` and `DEFERRABLE`. PostgreSQL enforces all
of them, including read-only (`25006`, kind `permission`). While a
transaction is open, the connection itself is busy, so work goes through the
`Transaction`. A statement prepared through `prepared_transaction(...)` is
closed when its transaction ends.

`commit()` of a transaction that the server already aborted (PostgreSQL
answers COMMIT with ROLLBACK) throws `DbFailure(..., "25P02")` instead of
reporting success. A transport failure during COMMIT is `IndeterminateCommit`:
the outcome is unknown and the connection is closed.

Prepared statements are named server-side statements described once. Their
parameter count is checked before any I/O (`PG_PARAMETER_COUNT`), and rows
reuse the described columns.

## Failures

A server `ErrorResponse` becomes `DbFailure` with the SQLSTATE as `code()`:

| SQLSTATE | kind |
| --- | --- |
| `23xxx` | constraint |
| `42xxx` | syntax (`42501`: permission) |
| `22xxx` | data |
| `28xxx`, `25006` | permission |
| `40001`, `40xxx` | serialization (`40P01`: deadlock) |
| `55P03` | busy |
| `57014` | cancelled |
| `08xxx`, `57xxx` | connection |
| `53xxx`, `54xxx` | resource |
| `0Axxx` | unsupported |
| other | unexpected |

The message is written by the driver. It names only the constraint, table or
column the server reported, because the server's own text can echo bound
values (`invalid input syntax for type integer: "..."`).

A FATAL error (for example, the backend was terminated, `57P01`) closes the
connection immediately. A lost transport is `DbFailure(connection, ...,
"PG_CONNECTION_LOST")`. An operation interrupted mid-conversation (cancelled
future, transport error) also closes the connection, because the protocol
position is then unknown. Query cancellation through `CancelRequest` is not
offered.

`COPY` is refused with `DbFailure(unsupported, ..., "PG_COPY")`: the driver
answers `CopyIn` with `CopyFail` and drains `CopyOut`.

## Measured

On the JVM backend against PostgreSQL 18 in Docker on macOS:

| operation | time |
| --- | --- |
| stream 20,000 two-column rows (`batch` 256) | ~0.22 s (~11 µs/row) |
| round-trip `SELECT 1` | ~2 ms |
| insert 6 MB bytea + 1 MB text in one statement | 0.14 s |
| read the same back | 0.38 s |

The per-row cost is mostly one async task switch (`RowReading.row()`) plus
`Row` construction. Messages already in the receive buffer are parsed without
further awaits.
