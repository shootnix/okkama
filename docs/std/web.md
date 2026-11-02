# web: application layer over std/http

`web/*` turns controller objects into an HTTP service without changing the std HTTP contracts underneath. A route table compiles into one `http/routing.Routes`, every reply is an `http/handler.HandlingResult`, and `WebApplication` hands the table to `http/server.HttpServer`. Code that needs full control can still return a std `ResponseHandling` or `WebSocketHandlingResult` from any endpoint.

```okkama
let routes = WebRoutes(
    [
        Get("/health", system.health),
        Scope("/api", [
            Scope("/users", UserExceptions(), [
                Post("", users.register),
                Get("/:id", users.show),
                Put("/:id/name", users.rename)
            ])
        ])
    ],
    system.missing
);
await WebApplication(routes).listen("127.0.0.1", 8000);
```

Runnable: [examples/web/server-ddd.ok](../../examples/web/server-ddd.ok), a DDD-layered service on SQLite.

## Endpoints

An endpoint is any bound action of type `async (WebRequest) is HandlingResult`, for example `users.show` where `users` is a controller object. One controller serves many routes and keeps its dependencies in its own fields; it is built once in the composition root.

## Modules

| Module | Contents |
|---|---|
| `web/routing` | `WebRoutes(routings, fallback)` and `WebRoutes(routings, exceptions, fallback)`; `Get`, `Post`, `Put`, `Patch`, `Delete` (path, endpoint); `Scope(prefix, routings)` and `Scope(prefix, exceptions, routings)`; roles `WebRouting` and `ExceptionScope`; `WebRejection` (problem replies for 400/405 rejected by std `Routes`); `InvalidWebRoute` |
| `web/request` | `WebRequest`: `path(name)`, `query(name)`, `query(name, fallback)`, `parameters()`, `header(name)`, `header(name, fallback)`, `method()`, async `bytes(limit)`, `text(limit)`, `json(limit)`, `form(limit)`, plus `request()` and `context()` for the std values; `RequestJson` (request-body JSON, reading failures are 422) |
| `web/reply` | `Text`, `Html`, `JsonReply(status, value)`, `Created(location, value)`, `NoContent`, `Redirect(status, location)`; each has the successor `header(name, value)` |
| `web/exception` | `Exception(status, detail)`: an `Error` and a reply with an RFC 9457 `application/problem+json` body |
| `web/app` | `WebApplication(routes)` or `(routes, policy, clock)`; async `listen(address, port)` and `server(address, port)` |

## Paths

A `Scope` prefix starts with `/` and does not end with it. A route path is `""` (the scope itself, only valid inside a `Scope`) or starts with `/`. The pattern is the concatenation, so `Scope("/api", [Scope("/users", [Get("", ...)])])` serves `/api/users`, and `Get("/", ...)` inside that scope would serve `/api/users/`, a different path. All std routing rules apply to the result: `:name` and final `*name` segments, rejection of overlapping patterns for one method at construction, 405 with `Allow` for a known path with another method, and HEAD answered by the GET endpoint without a body. Requests that `Routes` rejects before any endpoint runs are answered through its `RouteRejecting` extension point by `WebRejection`: 405 as a problem reply that keeps `Allow`, and a malformed path (invalid encoding or control characters, parameter budget) as 400 naming the reason.

## Exceptions

Endpoints contain the happy path. Errors are translated in an `ExceptionScope`, a separate object attached to a `Scope` (or to the whole `WebRoutes`):

```okkama
object UserExceptions does ExceptionScope {
    UserExceptions();
    async scoped(request is WebRequest, endpoint is async (WebRequest) is HandlingResult) is HandlingResult {
        try { await endpoint(request); }
        catch failure is UserNotFound { Exception(404, failure.text()); }
        catch failure is EmailTaken { Exception(409, failure.text()); }
    }
}
```

Errors a scope does not catch propagate to the next outer scope. Scopes run outermost first, so the innermost one sees an error first. After all scopes, the framework itself answers:

| Error | Reply |
|---|---|
| thrown `Exception` | that exception's own reply |
| anything else | not caught: the std `HttpServer` answers 500 without a body, and cancellation and timeouts keep their std meaning |

Request bodies read through `RequestJson` raise their own 422 `Exception` at the failing pointer. JSON errors from server-side data read with a plain `Json` are not translated: they are server errors and propagate like any other error. `Exception.detail` goes to the client, so it must never carry internal diagnostics.

`Exception.header(name, value)` is a successor like every reply header, and `throw` consumes successor results, so `throw Exception(401, "expired").header("WWW-Authenticate", "Bearer")` works in place.

## Requests

- `path(name)` reads a parameter of the matched pattern. A name the pattern does not declare throws `MissingRouteParameter`, which is a server bug, not a client error.
- `query(...)` decodes the query as `application/x-www-form-urlencoded`, the way browsers and `URLSearchParams` encode it, so `+` is a space. The raw RFC 3986 query is `request().target().query()`. A missing parameter without a fallback is 400.
- Bodies are read once, under an explicit byte limit. More bytes than the limit is 413, invalid UTF-8 in `text()` is 400.
- `json(limit)` requires `application/json` or `application/*+json` (else 415), parses eagerly (syntax errors and repeated member names are 400) and returns a `RequestJson`: the [`Json`](json.md) reading API (`text`, `int`, `get`, `items`, `optional`, member shortcuts; `value()` gives the plain `Json`) where every reading failure is `Exception(422)` with the JSON pointer in its detail. `form(limit)` requires `application/x-www-form-urlencoded`.

## Replies

Replies own their representation: `Content-Type`, `Content-Length` and `Transfer-Encoding` cannot be added through `header()`. Bodies are UTF-8 with an exact `Content-Length`, so connections stay reusable. `NoContent` carries no framing field; `Redirect` declares length 0. Body replies reject statuses that cannot carry a body (1xx, 204, 205, 304). JSON numbers use the positional spelling described in [JSON](json.md#writing).

## Known limitations

- Scopes and awaits between the server and the innermost read add completion levels; the runtime completion trampoline bounds their stack nesting (PERF-25, fix). Before it, scoped requests on SQLite emitted Perl deep-recursion warnings.
- Plaintext HTTP/1.1 only, as `HttpServer` provides today. The listen address is an IP literal.

Problem, cause and solutions · Evidence
