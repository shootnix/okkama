//! Owned OS files; the VM stores a typed handle, never a legacy runtime object.
use crate::host::native_fs::{fail, host, host_as};
use crate::host::{Error, handle::Handle, ProviderError, *};
use num_traits::ToPrimitive;
use okkama_runtime_abi::Native;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};
pub(crate) fn big(v: Integer) -> num_bigint::BigInt {
    match v {
        Integer::Small(n) => n.into(),
        Integer::Big(n) => (*n).clone(),
    }
}
#[derive(Debug)]
pub(crate) struct RawFile {
    pub(crate) file: Option<File>,
    #[cfg(unix)]
    pub(crate) lock: Option<crate::host::file_lock::Lock>,
    readable: bool,
    writable: bool,
    append: bool,
}
impl RawFile {
    pub(crate) fn create(
        n: Native,
        path: &std::path::Path,
        flags: i64,
        mode: u32,
    ) -> Result<Self, ProviderError> {
        let readable = flags & 1 != 0;
        let writable = flags & 2 != 0;
        if (!readable && !writable)
            || (flags & 24 != 0 && !writable)
            || (flags & 32 != 0 && flags & 4 == 0)
        {
            return Err(fail(n, "invalid_input", "Invalid open flags"));
        }
        let mut options = OpenOptions::new();
        options
            .read(readable)
            .write(writable)
            .create(flags & 4 != 0)
            .truncate(flags & 8 != 0)
            .create_new(flags & 32 != 0);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(mode);
        }
        let file = options.open(path).map_err(|e| host_as(n, e, "Cannot open file"))?;
        Ok(Self {
            file: Some(file),
            #[cfg(unix)]
            lock: None,
            readable,
            writable,
            append: flags & 16 != 0,
        })
    }

    pub(crate) fn close(&mut self, n: Native) -> Result<(), ProviderError> {
        #[cfg(unix)]
        if let Some(lock) = self.lock.take()
            && let Some(file) = &self.file {
                lock.release(file).map_err(|e| host(n, e))?;
            }
        self.file.take();
        Ok(())
    }
    pub(crate) fn open(&mut self, n: Native) -> Result<&mut File, ProviderError> {
        self.file
            .as_mut()
            .ok_or_else(|| fail(n, "already_closed", "File is closed"))
    }
    pub(crate) fn writable(&mut self, n: Native) -> Result<&mut File, ProviderError> {
        if self.file.is_none() {
            return Err(fail(n, "already_closed", "File is closed"));
        }
        if !self.writable {
            return Err(fail(n, "invalid_input", "File is not writable"));
        }
        self.open(n)
    }
}
pub(crate) fn handles(n: Native) -> bool {
    use Native::*;
    matches!(
        n,
        FsOpen
            | FsRead
            | FsWrite
            | FsSeek
            | FsFlush
            | FsSync
            | FsTruncate
            | FsClose
            | FsLockShared
            | FsLockExclusive
            | FsUnlock
    )
}
impl Heap {
    /// The program's working directory, from the run's host session (#40).
    pub(crate) fn working_directory(&mut self) -> Result<PathBuf, Error> {
        Ok(okkama_runtime_core::host::cwd())
    }
    pub(crate) fn fs_path(&mut self, n: Native, v: Slot) -> Result<PathBuf, ProviderError> {
        let bytes = self.native_bytes(v)?;
        if bytes.is_empty() || bytes.contains(&0) {
            return Err(fail(n, "invalid_input", "Invalid filesystem path"));
        }
        let path = PathBuf::from(String::from_utf8_lossy(&bytes).as_ref());
        Ok(if path.is_absolute() {
            path
        } else {
            self.working_directory()?.join(path)
        })
    }
    fn raw_file(&mut self, v: Slot) -> Result<&mut RawFile, Error> {
        if v.kind() != HANDLE {
            return Err(Error("expected file handle".into()));
        }
        match self.handles.get_mut(v.bits)? {
            Handle::File(file) => Ok(file),
            _ => Err(Error("expected file handle".into())),
        }
    }
    pub(crate) fn file_execute(&mut self, n: Native, a: &[Slot]) -> Result<Slot, ProviderError> {
        use Native::*;
        let yes = Slot::boolean(true);
        match n {
            FsOpen => {
                let path = self.fs_path(n, a[0])?;
                let flags = self
                    .integer(a[1])?
                    .to_i64()
                    .filter(|f| *f >= 0 && *f & !63 == 0)
                    .ok_or_else(|| fail(n, "invalid_input", "Invalid open flags"))?;
                let mode = self
                    .integer(a[2])?
                    .to_i64()
                    .filter(|m| (0..=0o777).contains(m))
                    .ok_or_else(|| fail(n, "invalid_input", "Invalid permission mode"))?;
                let file = RawFile::create(n, &path, flags, mode as u32)?;
                Ok(Slot {
                    tag: HANDLE,
                    bits: self.handles.allocate(Handle::File(file))?,
                })
            }

            FsClose => {
                self.raw_file(a[0])?.close(n)?;
                Ok(yes)
            }
            FsRead => {
                let limit = big(self.integer(a[1])?);
                if limit.sign() == num_bigint::Sign::Minus {
                    return Err(fail(n, "invalid_input", "Invalid read size"));
                }
                let limit = limit.to_i64().unwrap_or(i64::MAX);
                let file = self.raw_file(a[0])?;
                file.open(n)?;
                if !file.readable {
                    return Err(fail(n, "invalid_input", "File is not readable"));
                }
                let mut bytes = vec![0; limit.min(64 * 1024 * 1024) as usize];
                let size = file.open(n)?.read(&mut bytes).map_err(|e| host(n, e))?;
                bytes.truncate(size);
                Ok(self.native_byte_array(bytes)?)
            }
            FsWrite => {
                let bytes = self.native_bytes(a[1])?;
                let state = self.raw_file(a[0])?;
                let append = state.append;
                let file = state.writable(n)?;
                if append {
                    file.seek(SeekFrom::End(0)).map_err(|e| host(n, e))?;
                }
                let size = file.write(&bytes).map_err(|e| host(n, e))?;
                Ok(Slot::small(size as i64))
            }
            FsLockShared | FsLockExclusive | FsUnlock => {
                let state = self.raw_file(a[0])?;
                state.open(n)?;
                #[cfg(unix)]
                {
                    if n == FsUnlock {
                        if let Some(lock) = state.lock.take() {
                            lock.release(state.file.as_ref().expect("open"))
                                .map_err(|e| host(n, e))?;
                        }
                    } else {
                        let shared = n == FsLockShared;
                        if (shared && !state.readable) || (!shared && !state.writable) {
                            return Err(fail(
                                n,
                                "invalid_input",
                                "File mode does not permit this lock",
                            ));
                        }
                        if state.lock.is_some() {
                            return Err(fail(n, "invalid_input", "File already locked"));
                        }
                        state.lock = crate::host::file_lock::Lock::acquire(
                            n,
                            state.file.as_ref().expect("open"),
                            shared,
                            true,
                        )?;
                    }
                    Ok(yes)
                }
                #[cfg(not(unix))]
                {
                    Err(fail(n, "unsupported", "Record locking unavailable"))
                }
            }
            FsFlush => {
                self.raw_file(a[0])?.writable(n)?;
                Ok(yes)
            }
            FsSync => {
                self.raw_file(a[0])?
                    .writable(n)?
                    .sync_all()
                    .map_err(|e| host(n, e))?;
                Ok(yes)
            }
            FsTruncate => {
                let size = big(self.integer(a[1])?);
                if size.sign() == num_bigint::Sign::Minus {
                    return Err(fail(n, "invalid_input", "Negative file size"));
                }
                self.raw_file(a[0])?
                    .writable(n)?
                    .set_len(size.to_i64().unwrap_or(i64::MAX) as u64)
                    .map_err(|e| host(n, e))?;
                Ok(yes)
            }
            FsSeek => {
                let offset = big(self.integer(a[1])?);
                let origin = self.integer(a[2])?.to_i64();
                let file = self.raw_file(a[0])?.open(n)?;
                let base = match origin {
                    Some(0) => 0,
                    Some(1) => file.stream_position().map_err(|e| host(n, e))?,
                    Some(2) => file.metadata().map_err(|e| host(n, e))?.len(),
                    _ => return Err(fail(n, "invalid_input", "Invalid seek origin")),
                };
                use num_traits::ToPrimitive;
                let target = (num_bigint::BigInt::from(base) + offset)
                    .to_i64()
                    .filter(|n| *n >= 0)
                    .ok_or_else(|| fail(n, "invalid_input", "Invalid seek target"))?;
                file.seek(SeekFrom::Start(target as u64))
                    .map_err(|e| host(n, e))?;
                Ok(Slot::small(target))
            }
            _ => Err(Error("unsupported raw file native".into()).into()),
        }
    }
}

impl Drop for RawFile {
    fn drop(&mut self) {
        let _ = self.close(Native::FsClose);
    }
}
