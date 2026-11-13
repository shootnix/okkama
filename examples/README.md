# Examples

Runnable programs that show how to use the standard library, one folder per
module, laid out like the module path: `examples/fs/file/` shows `fs/file`.
Each program prepares what it needs, shows one task, and cleans up after
itself, so it runs from any directory:

    okkama run examples/fs/file/read_lines.ok

Next to each program, `NAME.expected` holds its output. `examples/check.py`
runs every program on IR and Sparrow (or the engines given with `--engine`)
and compares the output.

## fs/file

| Program | Task |
|---|---|
| `fs/file/read_whole.ok` | read a whole text file and print it |
| `fs/file/read_lines.ok` | read line by line with `for`, take the lines as an array, a header then the rest |
| `fs/file/write.ok` | write a text file and close it |
| `fs/file/append.ok` | append to a text file |
| `fs/file/bytes.ok` | bytes one by one, a piece, the rest, at an offset |
| `fs/file/lock.ok` | lock a file without waiting, handle a busy file, read-change-write |
| `fs/file/errors.ok` | catch the cases of `FileError` |

## fs/directory

| Program | Task |
|---|---|
| `fs/directory/list.ok` | list one directory, sorted, with each entry's kind and a file's size |
| `fs/directory/walk.ok` | walk a whole tree, a directory before its contents, and count files of a kind |
| `fs/directory/manage.ok` | create, rename and remove a directory; remove a full one |
| `fs/directory/errors.ok` | catch the cases of `FileError`: missing, already there, not empty |

## fs/path, fs/entry, fs/temp, fs/glob, fs/link, fs/watch

| Program | Task |
|---|---|
| `fs/path/paths.ok` | join parts, walk up to parents, tidy `.` and doubled separators, name a foreign grammar |
| `fs/entry/inspect.ok` | ask whether a path exists, its kind, size and permissions; move it, remove it |
| `fs/temp/scratch.ok` | a scratch directory and file with unpredictable names; keep one file, let the rest go |
| `fs/glob/find.ok` | find files by `*`, `?`, classes and `**`; refuse a bad pattern |
| `fs/link/links.ok` | switch a release with a symbolic link, share a file with a hard link |
| `fs/watch/changes.ok` | react to files created, modified, renamed and removed in a directory |

## os

| Program | Task |
|---|---|
| `os/command/capture.ok` | run a program without a shell, collect its output and exit code, feed a pipeline, stop at a limit |
| `os/process/talk.ok` | write to a running program, read its answer, stop a long-running one with a signal |
| `os/environment/variables.ok` | read a snapshot, derive a child's environment, find home and the search path |

## toml

| Program | Task |
|---|---|
| `toml/read.ok` | parse a document, walk every value with a visitor, read dates and times, report an error's line |

## csv

| Program | Task |
|---|---|
| `csv/rows.ok` | read rows with quotes inside, write them back, use another dialect, stream rows, report a bad quote |

## archive

| Program | Task |
|---|---|
| `archive/tar.ok` | build a TAR in memory, read it back within limits, refuse an unsafe path and an oversized member |

## statistics

| Program | Task |
|---|---|
| `statistics/describe.ok` | centre, spread and quartiles of a sample; population variance; modes; a fitted line |

## diff

| Program | Task |
|---|---|
| `diff/compare.ok` | compare two versions once: unified and context patches, similarity, the list of edits |

## uuid

| Program | Task |
|---|---|
| `uuid/ids.ok` | read a UUID from text and bytes, derive v3/v5 from a namespace, pack v8, draw v4 |

## calendar

| Program | Task |
|---|---|
| `calendar/month.ok` | print a month as a wall calendar, Monday first; leap years; an ISO week's dates |

## url

| Program | Task |
|---|---|
| `url/parts.ok` | parse a URL into parts, resolve links against it, read its query, build one from parts |

## cli

| Program | Task |
|---|---|
| `cli/todo.ok` | a tool with subcommands: declare it, read arguments, act on the selected command, report a bad argument, print usage |

## digest, crypto, net

| Program | Task |
|---|---|
| `digest/hash.ok` | SHA-2, SHA-3 and BLAKE2 of text; streaming and branching states |
| `crypto/secrets.ok` | sign with HMAC and verify, store and check a password, encrypt a record and detect a change |
| `crypto/jwt.ok` | issue a session token and check expiry and leeway, sign with Ed25519 and verify with only the public key, refuse a swapped algorithm or another audience |
| `net/addresses.ok` | parse and classify IP addresses, networks, endpoints; resolve through a fixed DNS table |
| `net/echo.ok` | a TCP echo over loopback (run it in a container, like the async tests) |

On the JVM, the digest and crypto examples need the host crypto library. An
installed Okkama finds it; a development build takes `OKKAMA_NATIVE_LIB`.

## logging, config, template

| Program | Task |
|---|---|
| `logging/app.ok` | log through a named logger: filter by level per name, hide a secret field, write text lines and keep records, log a failure with its cause |
| `config/layers.ok` | layer defaults, an INI file, environment variables and the command line; read a value with its source, a list, a fallback; report a broken file's line |
| `template/pages.ok` | fill a text template from bindings, walk JSON with sections and a partial, escape values in HTML and refuse an event attribute |

## terminal, db, locale, i18n

| Program | Task |
|---|---|
| `terminal/colors.ok` | decide the color depth from the terminal and the environment, render styled text for each depth, measure display width |
| `db/notes.ok` | an in-memory SQLite table: bound parameters, rows by column name, a prepared insert, a transaction undone by a constraint |
| `locale/formats.ok` | choose a locale from the user's ranges; write numbers, money, dates and lists four ways; read a number back; plural categories |
| `i18n/messages.ok` | messages by key and context, plural forms for English and Russian, a gettext PO file parsed and printed |

## strings/parse

| Program | Task |
|---|---|
| `strings/parse/numbers.ok` | read integers in a base, floats, rationals, bytes and truth values; fall back on bad input |

## time

| Program | Task |
|---|---|
| `time/dates.ok` | build a date, shift it by days, count days between, read and write ISO 8601 |
| `time/durations.ok` | add, scale and compare exact durations |
| `time/zones.ok` | a local time in one zone as an instant and in another zone; a repeated and a missing hour |
| `time/formats.ok` | RFC 3339 in and out, a custom pattern that reads its own text back |

## regex

| Program | Task |
|---|---|
| `regex/search.ok` | the first match, every match with named groups, a whole string ignoring case |
| `regex/replace.ok` | replace by a literal or a template with groups, up to a limit; split text |

## encoding

| Program | Task |
|---|---|
| `encoding/text_and_bytes.ok` | text to UTF-8 bytes and back, as hex and Base64; refuse broken UTF-8 |
| `encoding/binary.ok` | write and read a fixed-width binary record with explicit byte orders |

## collections

| Program | Task |
|---|---|
| `collections/sort_and_search.ok` | sort by value or by a key, then binary-search the sorted values |
| `collections/views.ok` | filter, map, take, chunk and pair lazily; fold to a total |

## random

| Program | Task |
|---|---|
| `random/seeded.ok` | reproducible dice, a choice and a shuffle from a seed; secure bytes |

## text

| Program | Task |
|---|---|
| `text/layout.ok` | wrap, indent, dedent and shorten text |

## async

These run real timers and tasks; like the async tests, check them in a
container.

| Program | Task |
|---|---|
| `async/channel.ok` | a producer and a consumer over a bounded channel, until it closes |
| `async/timeout.ok` | bound an operation by time, race two answers, wait for all of several |

## http/client, http/xmlrpc

Each program starts a small `HttpServer` on 127.0.0.1 with a handler, talks
to it and drains it, so it opens loopback sockets but needs no network. A real
program writes `HttpClient()` and a real URL.

| Program | Task |
|---|---|
| `http/client/get_text.ok` | GET a page and print it as text, decoded by its charset |
| `http/client/post_json.ok` | POST a JSON document and read the JSON answer |
| `http/client/statuses.ok` | compare a status with a case, match on it, take its class, number and phrase |
| `http/xmlrpc/greet.ok` | an XML-RPC server answers with `XmlRpcSuccess` or an `XmlRpcFailure` fault; the client calls it once of each kind |

## web, http/websocket

Each program except `web/hello.ok` serves on a free loopback port and calls
itself, so it needs no network, and drains its server at the end.

| Program | Task |
|---|---|
| `web/hello.ok` | the smallest web application: one route answers «Привет, Вася!» from `?name=Вася`, «Привет, Незнакомец!» without it; it serves on port 8080 until stopped, for a browser (no `.expected`, `check.py` skips it) |
| `web/notes.ok` | a JSON service: routes under a prefix, a path parameter, a JSON body, a query parameter with a default, a fallback |
| `http/websocket/echo.ok` | a WebSocket server checks the Origin against its own URL and answers each message; the client sends, reads and closes |

## mail

| Program | Task |
|---|---|
| `mail/compose.ok` | compose a message with headers and a text body, render it, parse it back, keep it in a Maildir |
