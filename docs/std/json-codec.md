# JSON codecs

`json/codec` maps a domain type to its JSON shape and back with a hand-written object. There is no reflection: a codec states every member name once per direction, both directions sit side by side, and the domain type never learns its wire format.

```okkama
import "json/codec" take JsonCodec, JsonList, JsonDecoded, JsonEncoded;

object LineJson does JsonCodec<Line> {
    LineJson();

    value(view is JsonView) is Line {
        Line(sku: view.text("sku"), qty: view.optional("qty").int(1));
    }

    json(line is Line) is JsonValue {
        JsonObject([JsonMember("sku", line.sku()), JsonMember("qty", line.quantity())]);
    }
}

object OrderJson does JsonCodec<Order> {
    OrderJson();

    value(view is JsonView) is Order {
        Order(
            id: view.int("id"),
            name: view.required("customer").text("name"),
            note: view.optional("note").text(""),
            lines: JsonList(item: LineJson()).value(view.required("lines"))
        );
    }

    json(order is Order) is JsonValue {
        JsonObject([
            JsonMember("id", order.id()),
            JsonMember("customer", JsonObject([JsonMember("name", order.name())])),
            JsonMember("note", order.note()),
            JsonMember("lines", JsonList(item: LineJson()).json(order.lines()))
        ]);
    }
}

let orders = JsonDecoded(codec: JsonList(item: OrderJson()), source: text).value();
let written = JsonEncoded(codec: JsonList(item: OrderJson()), origin: orders).string();
```

| Declaration | Meaning |
|---|---|
| `role JsonCodec<T> { value(view is JsonView) is T; json(value is T) is JsonValue; }` | the two directions of one type |
| `JsonList<T>(item is JsonCodec<T>) does JsonCodec<@T>` | a JSON array of `item` values, in order; codecs nest (`JsonList(item: JsonList(item: …))`) |
| `JsonDecoded<T>(codec, source is string).value() is T` | parses `source` (`InvalidJson`, `DuplicateJsonMember`) and reads it through `codec` |
| `JsonEncoded<T>(codec, origin is T).string()` | the compact JSON text of `origin` |
| `JsonTextCodec`, `JsonIntCodec`, `JsonBooleanCodec`, `JsonDecimalCodec` | scalar codecs, for arrays of scalars: `JsonList(item: JsonTextCodec())` |

Reading failures are the `JsonView` errors and keep the pointer of the failing value inside the whole document, for example `JSON value at /0/lines/0/qty must be number, found string`. `JsonDecoded` reads the parsed tree directly (see [JSON reading](json-reading.md)).

Conformance case `L09.json.codec` covers nesting, optional members and both error kinds.
