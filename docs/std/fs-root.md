# Descriptor-relative filesystem capabilities

`fs/root` acquires a `FileRoot(Directory(...))` eagerly. `entry(@string)` walks
decoded coordinates beneath that retained descriptor, opening each component
with `openat` and `O_NOFOLLOW`. Empty coordinates select the root directory.
All components reject empty strings, `.`/`..`, embedded slash, backslash and
NUL. Limits: 256 components, 255 UTF-8 bytes each, 4096 total path bytes.
Only regular files and directories are published; nonblocking acquisition
prevents a FIFO from waiting for a writer before rejection. The configured root
is trusted; its final component cannot be a symlink.

Every `RootedEntry` owns an independent descriptor. `stat()` observes that
descriptor; `bytes_at(offset, limit)` uses positional reads, with an exact
nonnegative offset and a 1 MiB maximum chunk. Root closure does not revoke
already acquired entries. Both owners support idempotent `close`, `closed`,
and fallback destruction. A handle cannot be compared or exposed as an integer.

`names(maximum)` bounds directory cardinality at 4096 and returns sorted UTF-8
names. Darwin obtains the current descriptor path with `F_GETPATH`, reopens a
directory handle and verifies device/inode against the retained descriptor
before enumeration. Linux uses `/proc/self/fd` with the same identity check.
Other hosts fail with typed `unsupported` rather than using an unchecked path.
Windows does not support this POSIX provider. Non-Windows installations require
`IO::FD v0.3.11`, declared in cpanfile.

Reads and directory operations are synchronous bounded host calls, including
when a consumer wraps them in an async body. Descriptors prevent path substitution;
they do not freeze file contents, prevent hard links, or provide a filesystem
snapshot. Publish trusted content atomically and close explicit owners.

Focused tests include deterministic directory replacement between two `openat`
calls, independent ownership, symlinks, FIFO rejection and cardinality budgets.
Full runtime evidence records804files/13232checks, with the one std-only inline-import failure corrected and its entire test file rerun PASS. Runtime bytes stayed identical.
