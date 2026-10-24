# Filesystem errors and native diagnostics

Application logic should catch the typed errors exported by the relevant
filesystem module, or inspect `NativeFailure.domain()` / `.code()` at a native
boundary. `fs/file` maps `invalid_input` and `already_closed` to
`InvalidFileOperation`; other native failures become `FileSystemFailure`, whose
`code()` retains the portable category. Wrapping in a catch preserves the
original failure as a cause using the existing runtime error-chain protocol.

Common portable `core.fs` categories are:

| Category | Meaning |
| --- | --- |
| `not_found` | Missing entry or parent |
| `permission_denied` | Access denied, including a read-only filesystem |
| `already_exists` | Exclusive-create collision or existing destination |
| `invalid_input` | Invalid operand, handle operation, or path-kind mismatch |
| `not_empty` | A directory cannot be removed because it contains entries |
| `resource_exhausted` | Storage, descriptor, quota or other resource limit |
| `unsupported` | Operation unavailable on this host or filesystem |
| `interrupted` / `would_block` | Interrupted operation or unavailable nonblocking lock |
| `already_closed` | Closed capability |
| `invalid_data` | Invalid encoding |
| `end_of_input` | End of a reader where the API signals it as an error |
| `unexpected` | Unclassified expected native failure |

These reuse the existing category mapping. A path-kind mismatch is not assigned
a new OS-derived category; on the tested POSIX host, opening a directory for
writing yields `invalid_input`. Host checks can fail in a different order on
other platforms, so a particular path setup need not produce the same category
on every filesystem.

For a failed host operation, runtime diagnostics additionally retain:

- `operation`: the failed filesystem operation, such as `open`, `read`, `rename`;
- `path`: the exact operand, or the path originally used to open a handle;
- `target_path`: the second operand of rename/link/symlink;
- `native_code` and `native_reason`: optional host diagnostic information;
- `native_symbol`: the canonical Okkama native call identity.

The portable category remains the diagnostic node's `code`. Public
`NativeFailure.operation()` still returns the canonical native symbol. The
extra payload is internal and immutable: there are no new Okkama accessors,
mutable fields, reflection facilities or host exception objects.

The failed operation can differ from the requested native action. For example,
`file_state_write` can fail while lazily opening its file; the diagnostic then
reports `operation: open` and retains `file_state_write` in `native_symbol`.
Current-directory lookup has no path operand. Temporary creation may report
only its supplied parent if the host library never exposed the generated name.
No native code/reason is fabricated when the host supplies none. Validation
failures before a syscall may carry only the portable error information.

Wrapping, rethrow, await and secondary cleanup failures retain this payload
alongside existing source stacks. The default renderer displays it inside the
normal cause chain. Paths may contain sensitive data. They are not canonicalized;
rendering escapes terminal controls and invalid UTF-8 bytes, and bounds long
output. The structured path retains its exact bytes on POSIX.

OS reason text can be localized; numeric codes and availability vary by host.
Do not branch on either, or promise identical text across Perl and future
backends. Successful filesystem behavior and stdlib action signatures are
unchanged.
