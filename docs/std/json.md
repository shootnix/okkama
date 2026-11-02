# JSON

`import "json"` is the whole JSON library: one immutable `Json` for a value, parsed or built in code, read without casts and written by the runtime in one call. It replaces `json/value`, `json/parse`, `json/read`, `json/write` and `json/codec` (removed after 0.3.1).

```okkama
import "json" take Json, JsonField, ParsedJson;

let order = ParsedJson(text).value();
order.int("id");                                // 4711
order.get("customer").text("name");             // "Ada"
order.optional("note").text("");                // "" when absent or null
let lines = order.list("items").map() -> line {
    Line(sku: line.text("sku"), qty: line.int("qty"));
};

let reply = Json([
    JsonField("id", 4711),
    JsonField("customer", Json([JsonField("name", "Ada")])),
    JsonField("tags", Json(items: [Json("new"), Json("vip")])),
]);
reply.string();                                 // {"id":4711,"customer":{"name":"Ada"},"tags":["new","vip"]}
reply.pretty(2);                                // the same, indented
```

A `Json` holds a node of the runtime's tree. A read is one native call, and a child is one small object. There are no reading roles, pointer chains or option trees.

## Parsing

`ParsedJson(text)` and `ParsedJson(text, maximum_depth)` parse strict RFC 8259 text. `value()` returns the root `Json` or throws:

- `InvalidJson`: syntax errors and nesting deeper than `maximum_depth` (default 128). `offset()` is zero-based; `line()` and `column()` are one-based. A non-positive `maximum_depth` is `InvalidJson` too.
- `DuplicateJsonMember`: a repeated member name, with the same source location.

Member order is kept. Numbers keep their exact spelling and are never read through `float`.

## Reading

| Action | Result | Fails with |
|---|---|---|
| `kind()` | `JsonKind`: `null_value`, `boolean_value`, `number_value`, `string_value`, `array_value`, `object_value` | |
| `pointer()` | RFC 6901 pointer of the value in its document (`""` is the root) | |
| `text()`, `boolean()`, `number()` | `string`, `bool`, exact `Decimal` | `WrongJsonKind` |
| `integer()` | exact `BigInt` | `WrongJsonKind`; `InvalidJsonNumber` for fractions or more than 4096 bits |
| `int()` | builtin `int` | as `integer()`; `InvalidJsonNumber` outside the signed 64-bit range |
| `size()` | number of items or members | `WrongJsonKind` for scalars |
| `at(index)`, `items()` | items of an array | `WrongJsonKind`; `MissingJsonValue` for an index out of range |
| `names()`, `values()`, `fields()` | member names, values and `JsonField`s in document order | `WrongJsonKind` |
| `has(name)` | whether the object has the member | `WrongJsonKind` |
| `get(name)` | the member | `WrongJsonKind`; `MissingJsonValue` |
| `text(name)`, `number(name)`, `int(name)`, `integer(name)`, `boolean(name)` | `get(name).text()` and so on | as above |
| `list(name)` | `get(name).items()` | as above |
| `optional(name)` | `JsonOptional`: the member, or absence when it is missing or `null` | `WrongJsonKind` when the value is not an object |

`text(name)`, `number(name)`, `int(name)` and `boolean(name)` read the member in one native call. When that fails they read it again through `get(name)` only to raise the typed error.

Every error exposes `pointer()` and names the location in `text()`, for example `JSON value at /0/lines/0/qty must be number, found string`. Member names are escaped as RFC 6901 tokens: `~` becomes `~0` and `/` becomes `~1`. The pointer is computed by the runtime only when it is asked for, so a read that succeeds costs nothing extra. A value built in code and placed twice in a document reports its first place.

`null` is a kind, not a missing value: test it with `kind() == JsonKind.null_value`.

## Optional members

`optional(name)` never fails for a member that is missing or `null`. Each reader of the returned `JsonOptional` takes the value to use in that case:

```okkama
let nick = body.optional("nick").text("anonymous");
let age = body.optional("age").int(0);
let city = body.optional("address").optional("city").text("");   // absent address: ""
let tags = body.optional("tags").items();                        // absent or null: []
```

| `JsonOptional` action | Result |
|---|---|
| `text(fallback)`, `number(fallback)`, `integer(fallback)`, `int(fallback)`, `boolean(fallback)` | the member's value, or `fallback` |
| `items()` | the member's items, or none |
| `optional(name)` | a member of the member; absent whenever this one is |
| `present()`, `pointer()` | presence; the member's pointer, even while absent |
| `value()` | the member; `MissingJsonValue` when absent or `null` |

A present member of another kind is still `WrongJsonKind`: `optional("age").text("")` on `"age": 36` fails rather than hiding wrong data. Code that must tell a missing member from `null` uses `has(name)` and `get(name).kind()`.

## Building

| Constructor | Value |
|---|---|
| `Json()` | `null` |
| `Json(value)` for `string`, `int`, `Decimal`, `BigInt`, `float`, `bool` | a scalar; a `float` is written as its shortest exact decimal |
| `Json(fields)` with `@JsonField` | an object; a repeated name is `DuplicateJsonMember` without a location (`located()` is false) |
| `Json(items: values)` with `@Json` | an array |

`JsonField(name, value)` takes a `Json` or any scalar above, so scalar members need no wrapper. An empty literal needs its label, because both list constructors accept it: `Json(fields: [])` is `{}` and `Json(items: [])` is `[]`. Items of an array are `Json` values: `Json(items: [Json(1), Json("a")])`.

## Writing

`string()` is compact text; `pretty(indentation)` puts one member or item per line with `indentation` spaces per level, `": "` after names and no final line break. Empty containers stay `[]` and `{}`. A negative indentation is `InvalidJsonIndentation`. The runtime writes the whole tree in one call.

Strings escape `"`, `\`, `\n`, `\r`, `\t` by name and the other controls U+0000..U+001F as lowercase `\u00xx`; everything else, including `/` and U+007F, is written as is.

Numbers are exact and written positionally while that stays short, with the thresholds of JavaScript's `Number#toString`:

| Value | Written |
|---|---|
| `404`, `1000`, `-12000` | `404`, `1000`, `-12000` |
| integers up to 21 digits | all digits |
| `1e21` and larger exponents | canonical form, `1e21` |
| `1.5`, `-0.005`, `0.000001` | positional |
| below `1e-6` (`1e-7`) | canonical form |

`1e1000000` is never expanded.

## Codecs

A codec is a plain object with two actions; no role is needed:

```okkama
object LineJson {
    LineJson();
    value(view is Json) is Line { Line(sku: view.text("sku"), qty: view.optional("qty").int(1)); }
    json(line is Line) is Json { Json([JsonField("sku", line.sku()), JsonField("qty", line.quantity())]); }
}

let lines is @Line = order.list("lines").map() -> line { LineJson().value(line); };
let written is @Json = lines.map() -> line { LineJson().json(line); };
Json(items: written).string();
```

Reading errors keep the pointer of the failing value inside the whole document. Conformance case `L09.json.codec` checks this.

## Performance

`benchmarks/json` parses, walks and writes six shapes of documents and compares Okkama with Python `json` and Perl `JSON::PP`; `run.py` prints the table. Parsing and writing run in the runtime and match or beat Python. Walking a document runs in Okkama code, one object per value.

## Errors

| Error | Accessors |
|---|---|
| `WrongJsonKind` | `pointer()`, `expected()`, `actual()`, `text()` |
| `MissingJsonValue` | `pointer()`, `text()` |
| `InvalidJsonNumber` | `pointer()`, `number()`, `reason()`, `text()` |
| `InvalidJson` | `offset()`, `line()`, `column()`, `text()` |
| `DuplicateJsonMember` | `name()`, `located()`, `offset()`, `line()`, `column()`, `text()` |
| `MissingJsonLocation` | thrown by the coordinates of an unlocated `DuplicateJsonMember` |
| `InvalidJsonIndentation` | `indentation()`, `text()` |

Tests: the inline tests of `std/json.ok` and `std/runtime/json.ok`, and the conformance cases `L09.json.*`, `L12.exact-number-json-sqlite` and `L12.malformed-boundary-inputs`.
