//! POSIX record locks through nix's safe boundary. A process-wide reservation
//! prevents overlapping locks even when independent VM hosts use different threads.
use crate::{
    native::ProviderError,
    native_fs::{fail, host},
};
use okkama_runtime_abi::Native;
use std::{
    collections::HashSet,
    fs::File,
    os::{fd::AsRawFd, unix::fs::MetadataExt},
    sync::{Mutex, OnceLock},
};
static LOCKS: OnceLock<Mutex<HashSet<(u64, u64)>>> = OnceLock::new();
#[derive(Debug)]
pub(crate) struct Lock {
    key: (u64, u64),
}
fn table() -> &'static Mutex<HashSet<(u64, u64)>> {
    LOCKS.get_or_init(Mutex::default)
}
fn command(file: &File, kind: i16, wait: bool) -> std::io::Result<()> {
    let record = libc::flock {
        l_start: 0,
        l_len: 0,
        l_pid: 0,
        l_type: kind,
        l_whence: libc::SEEK_SET as _,
    };
    let arg = if wait {
        nix::fcntl::FcntlArg::F_SETLKW(&record)
    } else {
        nix::fcntl::FcntlArg::F_SETLK(&record)
    };
    nix::fcntl::fcntl(file.as_raw_fd(), arg)
        .map(|_| ())
        .map_err(Into::into)
}
impl Lock {
    pub(crate) fn acquire(
        n: Native,
        file: &File,
        shared: bool,
        wait: bool,
    ) -> Result<Option<Self>, ProviderError> {
        let meta = file.metadata().map_err(|e| host(n, e))?;
        let key = (meta.dev(), meta.ino());
        if !table()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key)
        {
            return Err(fail(
                n,
                "invalid_input",
                "File lock overlaps an existing lock",
            ));
        }
        let lock = Self { key };
        match command(
            file,
            if shared {
                libc::F_RDLCK as _
            } else {
                libc::F_WRLCK as _
            },
            wait,
        ) {
            Ok(()) => Ok(Some(lock)),
            Err(e) if !wait && matches!(e.raw_os_error(), Some(libc::EAGAIN | libc::EACCES)) => {
                Ok(None)
            }
            Err(e) => Err(host(n, e)),
        }
    }
    pub(crate) fn release(self, file: &File) -> std::io::Result<()> {
        command(file, libc::F_UNLCK as _, false)
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        table()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.key);
    }
}
