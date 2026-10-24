# `archive/zip`: writing choice contracts

The first slice represents a ZIP writing choice. It does not compress data or
claim that a corresponding codec is installed. The user-deferred host compression
work remains deferred. ZIP wire readers/writers are separate roadmap tasks.

`ZipMethod = (stored, deflated, bzip2, lzma, zstandard)` identifies the method.
It is an ordinary enum, not a numeric ZIP header identifier or codec capability.

`ZipCompression(method is ZipMethod)` represents that method with a resolved
default level. `ZipCompression(method is ZipMethod, level is int)` validates and
resolves an explicit level. `method()` returns ZipMethod and `level()` returns
the effective int. Both are repeatable observations of an immutable value, with
no I/O, ambient configuration, provider discovery, resources or mutable state.
Construction rejects invalid choices immediately with `error.InvalidArgument`.

| Method | Default `level()` | Explicit level |
| --- | ---: | --- |
| stored | 0 | Rejected, including 0 |
| deflated | 6 | 0 through 9 inclusive |
| bzip2 | 9 | 1 through 9 inclusive |
| lzma | 0 | Rejected, including 0 |
| zstandard | 3 | -131072 through 22 inclusive; 0 resolves to 3 |

For stored/LZMA, zero means there is no configurable level in this ZIP contract;
it is not LZMA preset zero. Their two-argument constructors reject every int.
DEFLATE zero is a real level and remains zero. The Zstandard interval is a fixed
v4 policy, independent of a future installed host library; the codec integration
must explicitly honor or reject a valid choice, never clamp it silently. A
Zstandard zero alias and the default both normalize to the same value as an
explicit level 3. No zero-argument constructor chooses a method implicitly.

The level intervals follow the primary [Python ZIP documentation](https://docs.python.org/3/library/zipfile.html#zipfile.ZipFile).
Zstandard's zero/default semantics are documented in its [public header](https://github.com/facebook/zstd/blob/v1.5.7/lib/zstd.h).
Defaults 6/9/3 follow the existing v4 compression plan. Explicit-level rejection
for stored/LZMA is the existing v4 ZIP policy; unlike Python, it is an error rather
than an ignored argument. These references describe parameter meanings, not an
implementation dependency or a claim that compressed ZIP members are supported.

## ZipArchiveEntry

`ZipArchiveEntry(entry is ArchiveEntry, compression is ZipCompression)` denotes
one member's explicit writing request: the archive value plus its validated ZIP
method/level choice. `entry() is ArchiveEntry` and
`compression() is ZipCompression` return the bound values without observing,
traversing or rebuilding either dependency. Construction and observations perform
no I/O, codec calls, visitor dispatch, path/metadata access or resource cleanup.

The two coordinates describe one coherent protocol-boundary value. The wrapper
does not itself implement ArchiveEntry: callers deliberately unwrap the value
when using an archive visitor or a format-independent archive writer. Two choices
for the same entry remain independent; equality follows ordinary Okkama value
semantics and includes the selected compression. No default compression overload
or resource lifecycle is added.

All nominal ArchiveEntry implementations can be bound, including every existing
variant and user-defined implementations. This is not a claim that a future ZIP
writer can encode every kind or metadata value. Wire representability, format
limits and supported codecs must be checked by that writer before emitting a
member; the binding neither silently changes an entry nor pretends to validate
those future capabilities.

Remaining ZIP work, including stored wire encoding/decoding, is deferred by user
decision on 2026-09-25 along with all remaining archive work.
The planned default DEFLATE writer and compressed members remain open until the
deferred codecs are available; no silent fallback to stored is permitted.
