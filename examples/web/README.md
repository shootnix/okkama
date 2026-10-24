# Web server example

[server.ok](server.ok) is a long-running HTTP/1.1 server on `127.0.0.1:8000` built from `std/http/*`: `HttpServer` serves an immutable `Routes` table that dispatches requests to `Handling` objects.

| Method | Path | Handler |
|---|---|---|
| GET | `/` | route index |
| GET | `/hello/:name` | greeting with the decoded parameter |
| GET | `/users/:id/posts/:post` | JSON with two parameters |
| GET | `/files/*path` | wildcard remainder (display only, not a filesystem path) |
| POST | `/echo` | echoes the body, at most 64 KiB, otherwise 413 |

Unknown paths go to the `MissingPage` fallback (404). A known path with another method gets 405 with `Allow` from `Routes`; HEAD uses the GET route without a body.

```sh
cd okkama-pl
OKKAMA_STDLIB_DIR=$(cd ../std && pwd) perl bin/okkama ../examples/web/server.ok
```

```sh
curl -i http://127.0.0.1:8000/hello/Барак
curl -i -d 'привет' http://127.0.0.1:8000/echo
```

## DDD service on web/*

[server-ddd.ok](server-ddd.ok) is the same kind of server written with the [web/* layer](../../docs/std/web.md): domain values and entity, a `UserRepository` port with SQLite and in-memory adapters, a `UserService`, and a thin `UsersController` whose actions are the endpoints. Domain errors are translated to `Exception` (RFC 9457 problem details) in a separate `UserExceptions` scope, so controllers contain only the happy path. It listens on `127.0.0.1:8000` with an in-memory SQLite database.

```sh
cd okkama-pl
OKKAMA_STDLIB_DIR=$(cd ../std && pwd) perl bin/okkama ../examples/web/server-ddd.ok
```

```sh
curl -i -H 'Content-Type: application/json' -d '{"name":"Ada","email":"ada@example.test"}' http://127.0.0.1:8000/api/users
curl -i http://127.0.0.1:8000/api/users/1
curl -i -X PUT -H 'Content-Type: application/json' -d '{"name":"Ada Lovelace"}' http://127.0.0.1:8000/api/users/1/name
```
