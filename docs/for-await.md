# Async iteration: `for await`

```okkama
let rows = await query.rows();
defer rows.close();
for await rows -> row {
    Stdout().writeln(row.text("name"));
}
```

`for await source -> item { body }` is a statement-only async loop. The source
expression is evaluated once. Its accessible zero-argument `next()` action must
return `Future<Maybe<T>>` (normally declared `async next() is Maybe<T>`).
The source can be an object or a role exposing that action; no SQL-specific
protocol or implicit cast is involved. Ordinary `for` retains its synchronous
iteration protocol.

Each iteration awaits one `next()` call. `Maybe.some(value)` binds `item` with
type `T`; `Maybe.none` ends the loop without executing the body. Calls are
sequential, with no prefetch. Errors and cancellation propagate through normal
language cleanup. A returned `Presence<T>`, synchronous `Maybe<T>`, missing
`next()`, multiple bindings or a batched binding are static errors. Async
iteration is legal only inside an async action.

The binding is scoped to the body. Nested async loops have independent sources
and bindings. `next` continues the nearest loop; `last` exits it without another
fetch. `return` exits the enclosing action. Reached `defer` statements execute
with the same rules as in ordinary loops. An async loop does not produce a value.

The loop borrows the source for traversal; it does not implicitly call `close()`.
Keep `defer source.close()` around owned resource streams so an early `last`,
`return` or failure in the body releases them. `QueryRows` additionally closes
itself on EOF or a fetch failure; that is the stream's contract, not magic in
`for await`.

To iterate a collection obtained asynchronously, use ordinary
`for (await query.all(100)) -> row { ... }`. To stream it, use
`for await rows -> row { ... }` as above.
