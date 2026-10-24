# JSON reading

`json/read.JsonView(origin is JsonValue)` reads any JSON value with typed accessors and no casts. It works on `ParsedJson`, on constructed `JsonObject`/`JsonArray` values and on custom `JsonValue` implementations alike, because it observes values through the same `print(JsonWriting)` protocol that printers use.

```okkama
let body = JsonView(ParsedJson("{\"name\":\"Ada\",\"age\":36,\"tags\":[\"a\"]}"));
body.text("name");                 // "Ada"
body.int("age");                   // 36
body.member("tags").item(0).text() // "a"
```

## API

| Action | Result | Fails with |
|---|---|---|
| `kind()`, `value()`, `pointer()` | kind, the wrapped value, RFC 6901 pointer (`""` is the root) | |
| `text()`, `boolean()`, `number()` | `string`, `bool`, exact `Decimal` | `WrongJsonKind` |
| `integer()` | exact `BigInt` | `WrongJsonKind`; `InvalidJsonNumber` for fractions or more than 4096 bits |
| `int()` | builtin `int` | as `integer()`; `InvalidJsonNumber` outside the signed 64-bit range |
| `size()` | element or member count | `WrongJsonKind` for scalars |
| `items()`, `item(index)` | child views of an array | `WrongJsonKind`; `MissingJsonValue` for a missing index |
| `members()`, `names()`, `membership(name)` | members in document order, names, presence | `WrongJsonKind` |
| `member(name)` | child view | `WrongJsonKind`; `MissingJsonValue` |
| `text(name)`, `int(name)`, `integer(name)`, `number(name)`, `boolean(name)` | shortcut for `member(name).text()` and so on | as above |

Every failure exposes `pointer()` and names the location in `text()`, for example `JSON value at /email must be string, found number`. Member names are escaped as RFC 6901 tokens (`~` becomes `~0`, `/` becomes `~1`).

## Semantics

- A view reads one level at a time. `member(name)` materializes the member list of that object, not its children's subtrees. Each access costs O(size of that container); bind a child view once when you read it repeatedly.
- `ParsedJson` is lazy: syntax errors (`InvalidJson`, `DuplicateJsonMember`) appear on the first observation. `web/request.WebRequest.json()` forces that observation, so request bodies fail early with 400.
- A `JsonValue` whose `print()` does not issue exactly one `JsonWriting` command matching its `kind()` raises `InvalidJsonProtocol`. Reading never coerces a mismatching value.
- `null` is a kind: test `view.kind() == JsonKind.null_value`. There is no implicit default for missing members; use `membership(name)` to branch.

Tests: `okkama-pl/t/interpreter/660_std_json_read.t` (8 assertions) and the inline tests in `std/json/read.ok`. Evidence.

## Duplicate member source location

`DuplicateJsonMember.located()` reports whether source coordinates exist.
Parser-created duplicates retain zero-based offset and one-based line/column.
The explicit located constructor validates those ranges through `confirm`.
Programmatic duplicates have no source location: `offset()`, `line()` and
`column()` throw `json/value.MissingJsonLocation` (does `Error`) rather than
returning `-1`. The error keeps the member `name()`; `text()` is unchanged.

```okkama
if failure.located() {
    report(failure.offset(), failure.line(), failure.column());
}
else {
    report_programmatic_duplicate(failure.name());
}
```

Code that previously compared coordinates with `-1` should use `located()` or
catch `MissingJsonLocation`. A located offset of zero remains a real position.
