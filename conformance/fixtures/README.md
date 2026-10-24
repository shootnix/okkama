# Fixtures

Suite-owned helpers declared by cases in `requires.fixtures`
(`fixture.json`, schema `ocs-fixture/1`). Service fixtures bind `127.0.0.1` on an
OS-assigned port, announce `READY port=N`, own every resource they create and are
terminated by the runner after the case; their stderr log is kept in the result.

| Fixture | Purpose |
|---|---|
| `tcp-echo` | replies `echo:` + one LF-terminated line, then closes |
| `tcp-truncating-peer` | malformed peer: sends an unterminated fragment and closes |
| `free-port` | announces a port whose listener has been closed (refused connection) |

No fixture uses the public network, credentials or files outside its scratch
directory. Database cases use in-memory or working-directory SQLite databases
created and destroyed by the case itself.
