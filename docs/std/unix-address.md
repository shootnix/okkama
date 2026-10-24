# Unix filesystem address

`UnixAddress(origin is Path)` represents one named filesystem socket address
for the current host. `path() is Path` retains the exact supplied Path;
`string() is string` returns its exact text. No normalization, canonicalization,
absolute conversion, existence check or filesystem/socket operation occurs.
Relative paths remain relative; a future acquisition resolves them at that time.
An explicitly foreign PathStyle does not translate its text into native syntax.

Path already rejects empty text. UnixAddress additionally rejects every NUL,
including a leading NUL, with InvalidArgument. Linux abstract addresses and
unnamed sockets are not encoded as special filesystem paths. Pair/connection
endpoint design must represent unnamed endpoints separately rather than invent
an empty Path. Peer credentials and descriptor passing remain separate work.

The text is encoded as UTF-8 without normalization. The provider validates the
byte count against its sockaddr_un path storage, reserving one byte for the
terminating NUL. Thus the current Darwin limit is 103 bytes and Linux is 107;
these are bytes, not Unicode characters. Exceeding the limit throws
InvalidArgument, with no truncation and no attempted filesystem operation.
The private validator must also be used at future bind/connect boundaries.

The provider derives the path offset/capacity from two one-byte sockaddr_un
encodings rather than assuming that the path starts at sizeof(sa_family_t).
It checks the pack/unpack round trip and terminating byte. A missing or
unrecognized host representation raises `UnixSocketsUnsupported()` (an Error
whose text is `Unix sockets are unsupported by this provider`). This diagnosis
is host/provider-specific; it is not an always-failing connection wrapper.
Successful validation proves only address representability, not that a later
socket call is permitted or supported by the kernel.

Both public values are implemented in net/unix. Native validation takes bytes
through runtime/network.unix_path_valid; raw sockaddr bytes remain private to
the provider helper. Other NativeFailure domains/codes are preserved.

References: [Unix pathname rules](https://man7.org/linux/man-pages/man7/unix.7.html)
require room for a terminator and distinguish unnamed/abstract addresses;
[Darwin sockaddr_un](https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/bsd/sys/un.h)
defines its 104-byte path field. Perl Socket packing on the tested host silently
truncates oversized paths with a warning, so validation must precede packing.

This step does not implement connect, listen, accept, socketpair or stream I/O.
Listener cleanup must retain entry identity and avoid removing a replacement;
the old legacy socket/unix always-unsupported implementation is unchanged.
