1. Errors no longer objects. Let's make it enum's with a role:

    enum DatabaseError is Error (
        connection(message is string),
        request(message is string),
        auth(message is string)
    );



    catch {
        let data = db.query("select 1").row();

        ...
    }
    catch e is DatabaseError {
        match e {
            .connect(v) { ... }
            .request(v) { ... }
            .auth(v)    { ... }
            else        { ... }
        }
    }

2. Methods must have "throws" expr to push it up to the stack:

    class Database {
        has connection is DatabaseConnection = DatabaseConnection();
        Database(:connection);

        query(sql is string) is SqlQueryResult throws DatabaseError {
            ...
        }

        // if no "throws" method must handle an errors by itself
        do(sql is string) is SqlQueryResult {
            ...
        }
    }

3. Add new syntax to run exceptions catching inline:

    try!
    try?

    let data = try! db.query("select 1").row(); // throws an exception is there is an error
    let data = try? db.query("select 1").row(); // returns Presence<Error> ??? -- no
---

## Status (2026-11-03)

| Phase | Scope | Status |
|---|---|---|
| 1 | Error enums: `enum Name is Error`, throw/catch, generated `text()` | **implemented** |
| 2 | Forbid `does Error` on objects; migrate `std` error objects to error enums | **compiler done, base std migrated** (2026-11-05); rest of `std`: owner Alexander |
| 3 | `throws` in signatures (checked errors) | on hold |
| 4 | Swift-like `try?` / `try!`, catch patterns | **implemented** |

## Phase 1: error enums (implemented)

```okkama
enum DatabaseError is Error = (
    connection(message is string),
    request(sql is string, code is int),
    closed
);

enum RepoError is Error = (db(cause is DatabaseError), missing(id is int));

throw DatabaseError.request("select 1", 42);

let rows = try {
    db.query("select 1");
}
catch e is DatabaseError {
    match e {
        case DatabaseError.connection(message) { 0; }
        case DatabaseError.request(sql, code) { 0; }
        case DatabaseError.closed { 0; }
    }
}
catch e is Error {
    log(e.text());
    throw e;                 // keeps DatabaseError for outer catches
};
```

Syntax: `enum Name is Error = ( ... );` — the existing enum form plus the
`is Error` marker (the original draft without `=` was aligned with the current
enum grammar). `match` patterns stay the full `case Type.case(...)` form.

Rules (spec §9.5.1, §9.6):

- `is Error` is the only role an enum may declare; error enums have no type
  parameters.
- An error enum is always a variant enum: `match` over it is exhaustive, also
  when no case has a payload.
- Payloads: `string`, `int`, `byte`, `float`, `rat`, `bool`, and enums whose
  payloads satisfy the same rule — including other error enums, which is how a
  cause is attached. Objects, roles, collections, `char`, actions and `any`
  are rejected (they could re-open the role-recovery hole or carry resources).
- `text()` is generated (variant (a)): `Type.case` or
  `Type.case(v1, v2, ...)`; strings double-quoted, numbers in `string()` form,
  `true`/`false`, `Enum.case`, nested variants recursively:
  `RepoError.db(DatabaseError.closed)`. Callable on the value and through
  `Error`.
- Error enum values convert to `Error`; `catch e is Error` catches them; a
  typed `catch e is DatabaseError` catches every case.
- Uncaught: `DatabaseError: DatabaseError.request("select 1", 42)` with the
  usual location and stack trace.
- *(Superseded 2026-11-05: objects can no longer do `Error`; see "Phase 2" below.)*

Implementation: an error enum value is already a variant object in OIR, so
lowering gives that object `does Error` with a generated `text()` (built from
existing IR instructions) and names it after the enum. Checker: `throw`,
`catch`, `Error` conversion and `text()` on the value. The IR interpreter,
Sparrow (VM and JIT) and the JVM backend needed no changes.

Verification: conformance contract `ERR.ENUM` with six cases
(`L07.error-enum.*`) on IR, JVM and Sparrow; `portable-core` 757/757,
`portable-stdlib` 65/65; compiler tests 253/253, runtime tests 118/118.

Notes for phase 2:

- `NativeFailure` and `Cancelled` are still system objects (270 catches of
  `NativeFailure` in `std`); turning them into built-in error enums belongs to
  the migration.
- Errors that compute their message (`"invalid transition: " + action`) lose
  that formatting: the generated text shows the payload instead. Enum methods
  are not planned for phase 1.
- `does Error` on objects can be forbidden once `std` no longer uses it; that
  is the step that closes the role-recovery hole through `catch`.

## Phase 4: `try?`, `try!`, catch patterns (implemented)

```okkama
let rows = try? db.query(sql) else 0;          // Maybe<int>
let text = try? fetch(url)
    ?? .some(body): body
    !! .none: "offline";
let config = try! Config.parse(source);        // thrown error -> panic

try { work(); }
catch DatabaseError.request(sql, code) { report(code); }
catch DatabaseError.closed, NetworkError.reset { reconnect(); }
catch e is NetworkError, Denied { log(e.text()); }
```

Decisions (spec §9.6, §9.6.1):

- `try?`/`try!` are written without a space and cover the expression to the
  right up to `else`/`??`, so `try? x else y` is `(try? x) else y`.
- `try? value` is `Maybe<T>`; nested `Maybe` is not flattened. A void call is
  rejected (no value; use `try`/`catch`); a successor result is rejected (its
  must-use obligation would be lost).
- `try! value` keeps the value type, including void and successors; a thrown
  error becomes `panic "Unexpected error: <text()>"` located at the `try!`.
- Both catch `Error` only: panics pass through.
- A catch clause is either `name is T1, T2, ...` (the binding is `Error` when
  there are several types) or a list of patterns: error enum cases
  (`Enum.case`, `Enum.case(x, _)`) and error types. Named payload bindings need
  a single pattern; `_` is always allowed. Patterns of plain enums are rejected;
  a lone lowercase name is still an error (no catch-all).
- A thrown value whose type matches but whose case does not continues with the
  next alternative and the next clause of the same `try`. Unselected failures
  propagate.

Implementation: `try?`/`try!` are desugared in the checker into `try` with a
`catch` of `Error`. Catch alternatives are lowered as a chain of type checks
plus a variant-tag check, all leading to one handler; no new IR instructions.
Conformance contracts `ERR.TRY-MARK` and `ERR.CATCH-PATTERNS` (9 cases) pass on
IR, JVM and Sparrow.

## Phase 2: objects cannot do `Error` (compiler part implemented)

`object X does Error` is a semantic error:

```text
Object "X" cannot do Error: errors are enums, declare `enum X is Error = (...)`
```

Only error enums and the system failures (`NativeFailure`, `Cancelled`)
implement `Error`. `throw` accepts error enum values, `Error` references
(rethrow) and `NativeFailure`; catch types are error enums, `Error` and the
system failures. Objects are never thrown or caught, which closes the
role-recovery hole (`catch e is ConcreteClass` on a value thrown through a
role) completely.

Migrated in the same change:

- base `std` closure of `io/stdout`: `error` (`InvalidArgument`,
  `InvalidState`, `OutOfRange`, `UnsupportedOperation` →
  `X.because(reason)`), `io/stream` (`IncompleteWrite.after(written)`,
  `InvalidReadLimit.limit(value)`), `encoding/utf`
  (`InvalidUtf8/16/32.at(offset)`), and their callers `io/stdout`,
  `encoding/binary`, `text/layout`. 149 `std` modules compile; programs using
  `Stdout` work;
- conformance: 75 cases moved to error enums; 9 expected outputs follow the
  generated `text()`; `ERR.ERROR-ROLE`/`ERR.THROW` restated;
  `L07.error-role.missing-text` now checks that an object cannot do Error;
- Rust tests (compiler, runtime, JVM), examples (except cookbook labs),
  experiments, spec, books, context pack.

Still on the `std` side (phase 2, owner Alexander): 381 modules fail with
`cannot do Error` (`collections/iteration`, `async/*`, `numbers/bigint`,
`fs/path`, `strings/parse`, ...). Conformance `full-release` on IR: every
remaining failure (93) is such a module. Sparrow-engine fixtures
(`okkama-rs/crates/sparrow-engine/tests/fixtures/*.ok`, 8 files) still use
error objects and were left untouched.
