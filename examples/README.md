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

## http/client

Each program starts a stand-in server on 127.0.0.1 (`LocalServer`, at the
end of the file) and talks to it, so it opens loopback sockets but needs no
network. A real program writes `HttpClient()` and a real URL.

| Program | Task |
|---|---|
| `http/client/get_text.ok` | GET a page and print it as text, decoded by its charset |
| `http/client/post_json.ok` | POST a JSON document and read the JSON answer |
| `http/client/statuses.ok` | compare a status with a case, match on it, take its class, number and phrase |
