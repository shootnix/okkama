//! Temporary resources use exclusive OS creation and cryptographic random names.
use crate::host::{
    Error,
    handle::Handle,
    ProviderError,
    native_file::RawFile,
    native_file_state::FileState,
    native_fs::{fail, host},
    *,
};
use okkama_runtime_abi::Native;
use std::{cell::RefCell, fs, path::PathBuf, rc::Rc};
#[derive(Debug)]
pub(crate) struct TemporaryDirectory {
    path: PathBuf,
    /// The path as the program spelled it.
    shown: String,
    closed: bool,
}
pub(crate) fn handles(n: Native) -> bool {
    use Native::*;
    matches!(
        n,
        FsTemporaryFileCreate
            | FsTemporaryFilePath
            | FsTemporaryFilePreserve
            | FsTemporaryDirectoryCreate
            | FsTemporaryDirectoryPath
            | FsTemporaryDirectoryClosed
            | FsTemporaryDirectoryClose
    )
}
fn name(n: Native, prefix: &str) -> Result<String, ProviderError> {
    if prefix.contains(['\0', '/', '\\']) {
        return Err(fail(
            n,
            "invalid_input",
            "Temporary prefix must be one component",
        ));
    }
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_";
    let mut name = prefix.to_owned();
    let end = name.len() + 8;
    while name.len() < end {
        let bytes = okkama_native::call("random", &[b"16"])
            .map_err(|_| fail(n, "unexpected", "Random source unavailable"))?;
        for b in bytes {
            if b < 252 && name.len() < end {
                name.push(ALPHABET[usize::from(b) % 63] as char);
            }
        }
    }
    Ok(name)
}
impl Heap {
    pub(crate) fn temporary_execute(
        &mut self,
        n: Native,
        a: &[Slot],
    ) -> Result<Slot, ProviderError> {
        use Native::*;
        let yes = Slot::boolean(true);
        if matches!(n, FsTemporaryFileCreate | FsTemporaryDirectoryCreate) {
            let parent = self.fs_path(n, a[0])?;
            let spelled = String::from_utf8_lossy(&self.native_bytes(a[0])?).into_owned();
            let prefix = self.text(a[1])?.to_owned();
            for _ in 0..100 {
                let leaf = name(n, &prefix)?;
                let path = parent.join(&leaf);
                // Tidy, as path() and child() make paths: `./t-x` is `t-x`.
                let unix = crate::host::path_syntax::Spec::Unix;
                let shown = unix.canonpath(&crate::host::path_syntax::joined(
                    unix,
                    &[spelled.clone(), leaf],
                ));
                if n == FsTemporaryFileCreate {
                    match RawFile::create(n, &path, 1 | 2 | 4 | 32, 0o600) {
                        Ok(channel) => {
                            let state = FileState::temporary(path, shown, channel);
                            return Ok(Slot {
                                tag: HANDLE,
                                bits: self
                                    .handles
                                    .allocate(Handle::FileState(Rc::new(RefCell::new(state))))?,
                            });
                        }
                        Err(ProviderError::Native(e)) if e.code == "already_exists" => continue,
                        Err(e) => return Err(e),
                    }
                } else {
                    let mut builder = fs::DirBuilder::new();
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::DirBuilderExt;
                        builder.mode(0o700);
                    }
                    match builder.create(&path) {
                        Ok(()) => {
                            return Ok(Slot {
                                tag: HANDLE,
                                bits: self.handles.allocate(Handle::TemporaryDirectory(
                                    TemporaryDirectory {
                                        path,
                                        shown,
                                        closed: false,
                                    },
                                ))?,
                            });
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                        Err(e) => return Err(host(n, e)),
                    }
                }
            }
            return Err(fail(
                n,
                "resource_exhausted",
                "Temporary name attempts exhausted",
            ));
        }
        if matches!(n, FsTemporaryFilePath | FsTemporaryFilePreserve) {
            let shared = self.file_state(a[0])?;
            let mut state = shared.borrow_mut();
            if !state.temporary {
                return Err(fail(n, "invalid_input", "File is not temporary"));
            }
            if n == FsTemporaryFilePath {
                return Ok(self.native_byte_array(state.shown.as_bytes().to_vec())?);
            }
            if state.closed {
                return Err(fail(n, "already_closed", "Temporary file is closed"));
            }
            let destination = self.fs_path(n, a[1])?;
            let spelled = String::from_utf8_lossy(&self.native_bytes(a[1])?).into_owned();
            state.seal(n)?;
            match fs::hard_link(&state.path, &destination) {
                Ok(()) => {}
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::CrossesDevices | std::io::ErrorKind::Unsupported
                    ) =>
                {
                    // Exclusive creation preserves the no-overwrite contract even across devices.
                    use std::io::{Read, Write};
                    let mut source = fs::File::open(&state.path).map_err(|e| host(n, e))?;
                    let mut target = fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&destination)
                        .map_err(|e| host(n, e))?;
                    let mut buffer = [0; 65536];
                    loop {
                        let size = source.read(&mut buffer).map_err(|e| host(n, e))?;
                        if size == 0 {
                            break;
                        }
                        target.write_all(&buffer[..size]).map_err(|e| host(n, e))?;
                    }
                }
                Err(e) => return Err(host(n, e)),
            }
            fs::remove_file(&state.path).map_err(|e| host(n, e))?;
            state.path = destination;
            state.shown = spelled;
            state.delete_on_close = None;
            return Ok(yes);
        }
        if a[0].kind() != HANDLE {
            return Err(Error("expected TemporaryDirectoryState".into()).into());
        }
        let Handle::TemporaryDirectory(state) = self.handles.get_mut(a[0].bits)? else {
            return Err(Error("expected TemporaryDirectoryState".into()).into());
        };
        match n {
            FsTemporaryDirectoryPath => {
                let bytes = state.shown.as_bytes().to_vec();
                Ok(self.native_byte_array(bytes)?)
            }
            FsTemporaryDirectoryClosed => Ok(Slot::boolean(state.closed)),
            FsTemporaryDirectoryClose => {
                if !state.closed {
                    match fs::symlink_metadata(&state.path) {
                        Ok(meta) => {
                            if meta.is_dir() {
                                fs::remove_dir_all(&state.path)
                            } else {
                                fs::remove_file(&state.path)
                            }
                            .map_err(|e| host(n, e))?;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => return Err(host(n, e)),
                    }
                    state.closed = true;
                }
                Ok(yes)
            }
            _ => Err(Error("unsupported temporary native".into()).into()),
        }
    }
}
