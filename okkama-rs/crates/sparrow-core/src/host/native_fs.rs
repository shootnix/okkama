//! Filesystem native boundary. Lexical paths are portable and independent of
//! the host platform; host operations are added without legacy runtime values.
use crate::host::{
    Error,
    {Outcome, ProviderError},
    path_syntax::{Spec, joined, path_parent},
    *,
};
use okkama_runtime_abi::{Native, NativeFailure};
pub(crate) fn handles(n: Native) -> bool {
    use Native::*;
    #[cfg(unix)]
    if crate::host::native_watch::handles(n) {
        return true;
    }
    crate::host::native_temporary::handles(n)
        || crate::host::native_file_state::handles(n)
        || crate::host::native_file::handles(n)
        || crate::host::native_directory::handles(n)
        || matches!(
            n,
            FsMatchingPaths
                | FsPathEncode
                | FsPathDecode
                | FsPathJoin
                | FsPathParent
                | FsPathChild
                | FsPathAbsolute
        )
}
pub(crate) fn fail(n: Native, code: &str, message: &str) -> ProviderError {
    ProviderError::Native(NativeFailure {
        domain: "core.fs".into(),
        code: code.into(),
        operation: n.symbol().into(),
        message: message.into(),
    })
}
/// [`host`] with the phrase of the step that failed, where one native does
/// several (`file_state_read_text` opens the file lazily, #319).
pub(crate) fn host_as(n: Native, e: std::io::Error, message: &str) -> ProviderError {
    match host(n, e) {
        ProviderError::Native(mut failure) => {
            failure.message = message.into();
            ProviderError::Native(failure)
        }
        other => other,
    }
}
pub(crate) fn host(n: Native, e: std::io::Error) -> ProviderError {
    use std::io::ErrorKind::*;
    let code = match e.kind() {
        NotFound => "not_found",
        PermissionDenied | ReadOnlyFilesystem => "permission_denied",
        AlreadyExists => "already_exists",
        DirectoryNotEmpty => "not_empty",
        NotADirectory | IsADirectory | InvalidInput => "invalid_input",
        CrossesDevices | Unsupported => "unsupported",
        StorageFull | OutOfMemory | QuotaExceeded => "resource_exhausted",
        BrokenPipe => "broken_pipe",
        Interrupted => "interrupted",
        WouldBlock => "would_block",
        _ => match e.raw_os_error() {
            Some(libc::EMFILE | libc::ENFILE) => "resource_exhausted",
            Some(libc::ELOOP | libc::ENAMETOOLONG | libc::EBADF) => "invalid_input",
            _ => "unexpected",
        },
    };
    match host_message(n) {
        Some(message) => fail(n, code, message),
        None => fail(n, code, &e.to_string()),
    }
}

/// The message of a failed host file operation, the phrase the IR engine and
/// the JVM write for it; the reason is in the code (#319). Other natives keep
/// the host's own text.
fn host_message(n: Native) -> Option<&'static str> {
    Some(match n.name() {
        "open" | "file_content" => "Cannot open file",
        "read" | "file_state_read" | "file_state_read_text" | "file_state_read_line" => "Cannot read file",
        "write" | "file_state_write" | "file_state_write_text" => "Cannot write file",
        "close" | "file_state_close" => "Cannot close file",
        "seek" | "file_state_seek" => "Cannot seek file",
        "truncate" | "file_state_truncate" => "Cannot truncate file",
        "sync" => "Cannot synchronize file",
        "unlock" => "Cannot unlock file",
        "file_state_lock" => "Cannot acquire file lock",
        "file_lock_close" => "Cannot release file lock",
        "file_state_position" => "Cannot inspect file position",
        "opendir" | "directory_entries" | "matching_paths" => "Cannot open directory",
        "mkdir" => "Cannot create directory",
        "rmdir" => "Cannot rmdir path",
        "directory_remove_tree" => "Cannot remove directory tree",
        "unlink" => "Cannot unlink path",
        "rename" => "Cannot rename path",
        "link" => "Cannot link path",
        "symlink" => "Cannot symlink path",
        "readlink" => "Cannot read symbolic link",
        "stat" => "Cannot stat path",
        "lstat" => "Cannot lstat path",
        "chmod" => "Cannot change path permissions",
        "chown" => "Cannot change path ownership",
        "chdir" => "Cannot change current directory",
        "temporary_file_create" => "Cannot create temporary file",
        "temporary_directory_create" => "Cannot create temporary directory",
        "file_watch_create" => "Cannot inspect watched path",
        _ => return None,
    })
}
impl Heap {
    pub(crate) fn native_fs(&mut self, n: Native, a: &[Slot]) -> Result<Outcome, Error> {
        match self.fs_execute(n, a) {
            Ok(v) => Ok(Outcome::Value(v)),
            Err(ProviderError::Engine(e)) => Err(e),
            Err(ProviderError::Native(e)) => {
                let payload = self.native_failure_new(e)?;
                self.failure_new(crate::host::FailureKind::Native(payload))
                    .map(Outcome::Failure)
            }
        }
    }
    fn fs_execute(&mut self, n: Native, a: &[Slot]) -> Result<Slot, ProviderError> {
        use Native::*;
        #[cfg(unix)]
        if crate::host::native_watch::handles(n) {
            return self.watch_execute(n, a);
        }
        if n == FsMatchingPaths {
            return self.glob_execute(n, a);
        }
        if crate::host::native_temporary::handles(n) {
            return self.temporary_execute(n, a);
        }
        if crate::host::native_file_state::handles(n) {
            return self.file_state_execute(n, a);
        }
        if crate::host::native_file::handles(n) {
            return self.file_execute(n, a);
        }
        if crate::host::native_directory::handles(n) {
            return self.directory_execute(n, a);
        }
        if n == FsPathEncode {
            return Ok(self.native_byte_array(self.text(a[0])?.as_bytes().to_vec())?);
        }
        if n == FsPathDecode {
            let text = String::from_utf8(self.native_bytes(a[0])?)
                .map_err(|_| fail(n, "invalid_data", "Filesystem path is not valid UTF-8"))?;
            return Ok(self.allocate_text(text)?);
        }
        let spec = match self
            .integer(a[if n == FsPathChild { 2 } else { 1 }])?
            .to_i64()
        {
            Some(0) => Spec::Unix,
            Some(1) => Spec::Win32,
            _ => return Err(fail(n, "invalid_input", "Invalid path style")),
        };
        let text = match n {
            FsPathJoin => {
                let parts = self
                    .elements(a[0])?
                    .iter()
                    .map(|v| self.text(*v).map(str::to_owned))
                    .collect::<Result<Vec<_>, _>>()?;
                if parts.is_empty() {
                    return Err(fail(
                        n,
                        "invalid_input",
                        "Path join requires at least one part",
                    ));
                }
                if parts.iter().any(|s| s.is_empty() || s.contains('\0')) {
                    return Err(fail(n, "invalid_input", "Path join rejects empty parts"));
                }
                joined(spec, &parts)
            }
            FsPathChild => {
                let (path, part) = (self.text(a[0])?, self.text(a[1])?);
                if path.is_empty() || part.is_empty() {
                    return Err(fail(n, "invalid_input", "Path child rejects empty values"));
                }
                if path.contains('\0') || part.contains('\0') {
                    return Err(fail(n, "invalid_input", "Invalid lexical path"));
                }
                joined(spec, &[path.into(), part.into()])
            }
            FsPathParent | FsPathAbsolute => {
                let path = self.text(a[0])?;
                if path.is_empty() || path.contains('\0') {
                    return Err(fail(n, "invalid_input", "Invalid lexical path"));
                }
                if n == FsPathAbsolute {
                    return Ok(Slot::boolean(spec.absolute(path)));
                }
                path_parent(spec, path)
            }
            _ => return Err(Error("unsupported filesystem native".into()).into()),
        };
        Ok(self.allocate_text(text)?)
    }
}
