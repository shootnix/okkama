# Extension-based media type identification

`MediaIdentifying` has `type(path is Path) is MediaType` and
`extensions(type is MediaType) is Sequencing<string>`. Identification is lexical:
only the final component of the supplied POSIX/Windows path participates. It
does not open, stat, follow, download or inspect the target. Extensions compare
with ASCII case folding; the longest registered compound suffix wins within a
table. A match requires a nonempty stem, so `.json` alone is not an extension.
Directories, trailing separators and extensionless names are unknown.
`Path.style()` exposes its existing immutable grammar coordinate for this purpose.

`StandardMediaTypes()` owns a deterministic common table of registered types;
it is intentionally incomplete. `UnknownMediaType(path)` retains path() and
text(); unknown types return an empty extension sequence. Reverse lookup ignores
media parameters and returns unique extension names without leading dots.
This is a naming convention, not content validation or a trust decision.

`MediaTypeExtension(extension,type)` represents one mapping. Extensions are
1–255 ASCII letters/digits/underscore/plus/hyphen/dot, with nonempty dot-separated
segments and no leading dot. `InvalidMediaExtension(extension)` rejects other
forms. `MappedMediaTypes(entries,fallback[,maximum_entries])` snapshots its
sequence once; default cardinality is 10000. The first identical extension wins;
explicit mappings take precedence over the fallback layer. Reverse lookup omits
shadowed aliases and duplicates. Fallback is borrowed and may have its own
observation effects. Negative sizes/limits throw InvalidArgument; excessive
source/output cardinality throws MediaTypeLimitExceeded("entries").

`SystemMediaTypes(fallback)` eagerly snapshots optional `/etc/mime.types` on
POSIX; Windows defaults to an empty optional file list. Explicit overloads
`(fallback,files is Sequencing<Path>)` and `(fallback,files,limits is MediaTypeFileLimits)`
make configuration portable. Paths are snapshotted once before I/O. Missing files
are skipped; other filesystem errors propagate. Files close on success/failure;
later lookups never reread them. Earlier files/rows win equal-extension conflicts.
Caller mappings remain in separate MappedMediaTypes layers, never in the standard
table. All target-file access remains absent.

File syntax: UTF-8 text, LF/CRLF lines, `type/subtype ext ext`, SP/HTAB separators,
`#` comments, blank lines. Media parameters, malformed extensions, bare CR and
invalid UTF-8 fail with InvalidMediaTypeDatabase(line,reason); line() is one-based,
or zero for an input encoding failure. Other database formats are not guessed.
Rows containing a valid media type without extensions add no aliases.
`MediaTypeFileLimits()` defaults to 16 files, 1 MiB cumulative file bytes and
10000 mappings; `(files,bytes,entries)` overrides nonnegative bounds. Reads are
chunked and capped before appending. MediaTypeLimitExceeded.kind() identifies
files, bytes or entries. Table lookups are bounded linear scans; reverse alias
filtering may be quadratic within the explicit cardinality bound.

The [IANA registry](https://www.iana.org/assignments/media-types) supplies canonical
type names. The built-in file suffix policy includes common aliases and explicit
compound gzip/zstd suffixes; it is not an exhaustive IANA extension database.
JavaScript uses [text/javascript](https://www.iana.org/assignments/media-types/text/javascript),
and compressed files use [application/gzip](https://www.iana.org/assignments/media-types/application/gzip)
and [application/zstd](https://www.iana.org/assignments/media-types/application/zstd).
