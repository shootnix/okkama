# HTTP status values

`http/status::Status(code is int)` is an immutable numeric response status.
Construction accepts exactly 100..599, including extension codes, and otherwise
throws `InvalidStatus(code)` preserving the rejected `code()`.
`Status.code()` returns the exact identity; `class()` returns `StatusClass`:
informational, successful, redirection, client_error or server_error, from the
numeric hundred digit. No strings or registered-name lookup determine class.

`phrase()` is optional **display text**, represented by an empty string when no
name is provided. It is never a body/framing/retry policy or an HTTP/2 field.
The local phrase table is an explicit snapshot of the
[IANA registry updated 2025-09-15](https://www.iana.org/assignments/http-status-codes/).
Unused 306 and 418 have empty phrases. The temporary 104 label and historical 510
label are presentation entries only; they imply no implemented extension.
Unknown status codes remain usable without updating this table.

Every query is deterministic and has no effects, resource ownership or
cancellation behavior. Storage and query work are bounded by the fixed table.
The API does not accept arbitrary reason phrases or add named-object aliases.

Tests cover all 500 permitted codes, numeric class, invalid boundaries, current
standard display names and unknown/unused codes. Inline tests and an executable
example use only values; they make no assertion of HTTP wire support.
