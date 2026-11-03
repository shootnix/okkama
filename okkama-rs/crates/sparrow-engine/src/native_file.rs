//! Owned OS files; the VM stores a typed handle, never a legacy runtime object.
use crate::native_fs::{fail, host};
use crate::{Error, handle::Handle, native::ProviderError, value::*};
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
    pub(crate) lock: Option<crate::file_lock::Lock>,
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
        let file = options.open(path).map_err(|e| host(n, e))?;
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
        if let Some(lock) = self.lock.take() {
            if let Some(file) = &self.file {
                lock.release(file).map_err(|e| host(n, e))?;
            }
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
    pub(crate) fn working_directory(&mut self) -> Result<PathBuf, Error> {
        if let Some(path) = &self.cwd {
            return Ok(path.clone());
        }
        let path = std::env::current_dir().map_err(|e| Error(e.to_string()))?;
        self.cwd = Some(path.clone());
        Ok(path)
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
        if v.tag != HANDLE {
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
                        state.lock = crate::file_lock::Lock::acquire(
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
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    pub(super) fn directory() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "sparrow-files-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        p
    }
    fn code(result: Result<Slot, ProviderError>) -> String {
        match result {
            Err(ProviderError::Native(e)) => e.code,
            other => panic!("expected native failure, got {other:?}"),
        }
    }
    #[test]
    fn raw_file_position_append_truncate_close_and_generation() {
        let dir = directory();
        let mut h = Heap::new(&[], &[]);
        h.cwd = Some(dir.clone());
        let p = h.native_byte_array(b"data".to_vec()).unwrap();
        let file = h
            .file_execute(Native::FsOpen, &[p, Slot::small(7), Slot::small(0o600)])
            .unwrap();
        let data = h.native_byte_array(b"abc".to_vec()).unwrap();
        assert_eq!(
            h.file_execute(Native::FsWrite, &[file, data]).unwrap().bits,
            3
        );
        h.file_execute(Native::FsSeek, &[file, Slot::small(1), Slot::small(0)])
            .unwrap();
        h.file_execute(Native::FsTruncate, &[file, Slot::small(5)])
            .unwrap();
        let result = h
            .file_execute(Native::FsRead, &[file, Slot::small(8)])
            .unwrap();
        assert_eq!(h.native_bytes(result).unwrap(), b"bc\0\0");
        let result = h.file_execute(Native::FsSeek, &[file, Slot::small(-20), Slot::small(1)]);
        assert_eq!(code(result), "invalid_input");
        h.collect([file].into_iter()).unwrap();
        h.file_execute(Native::FsClose, &[file]).unwrap();
        h.file_execute(Native::FsClose, &[file]).unwrap();
        assert_eq!(
            code(h.file_execute(Native::FsRead, &[file, Slot::small(1)])),
            "already_closed"
        );
        let p = h.native_byte_array(b"data".to_vec()).unwrap();
        let file = h
            .file_execute(Native::FsOpen, &[p, Slot::small(18), Slot::small(0o600)])
            .unwrap();
        let data = h.native_byte_array(b"z".to_vec()).unwrap();
        h.file_execute(Native::FsWrite, &[file, data]).unwrap();
        h.file_execute(Native::FsClose, &[file]).unwrap();
        assert_eq!(std::fs::read(dir.join("data")).unwrap(), b"abc\0\0z");
        h.collect(std::iter::empty()).unwrap();
        assert!(h.file_execute(Native::FsClose, &[file]).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn file_locks_overlap_across_heaps_and_release_on_close_and_gc() {
        let dir = directory();
        let path = dir.join("locked");
        std::fs::write(&path, []).unwrap();
        let mut h = Heap::new(&[], &[]);
        let mut other = Heap::new(&[], &[]);
        let p = h
            .native_byte_array(path.to_string_lossy().as_bytes().to_vec())
            .unwrap();
        let q = other
            .native_byte_array(path.to_string_lossy().as_bytes().to_vec())
            .unwrap();
        let f = h
            .file_execute(Native::FsOpen, &[p, Slot::small(3), Slot::small(0o600)])
            .unwrap();
        let g = other
            .file_execute(Native::FsOpen, &[q, Slot::small(3), Slot::small(0o600)])
            .unwrap();
        h.file_execute(Native::FsLockExclusive, &[f]).unwrap();
        assert_eq!(
            code(other.file_execute(Native::FsLockShared, &[g])),
            "invalid_input"
        );
        h.file_execute(Native::FsUnlock, &[f]).unwrap();
        other.file_execute(Native::FsLockShared, &[g]).unwrap();
        other.collect(std::iter::empty()).unwrap();
        h.file_execute(Native::FsLockExclusive, &[f]).unwrap();
        h.file_execute(Native::FsClose, &[f]).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn virtual_cwd_directory_snapshot_and_symlink_do_not_change_host_cwd() {
        let dir = directory();
        std::fs::write(dir.join("z"), b"z").unwrap();
        std::fs::write(dir.join("a"), b"a").unwrap();
        let host = std::env::current_dir().unwrap();
        let mut h = Heap::new(&[], &[]);
        let p = h
            .native_byte_array(dir.to_string_lossy().as_bytes().to_vec())
            .unwrap();
        h.directory_execute(Native::FsChdir, &[p]).unwrap();
        assert_eq!(std::env::current_dir().unwrap(), host);
        let dot = h.native_byte_array(b".".to_vec()).unwrap();
        let d = h.directory_execute(Native::FsOpendir, &[dot]).unwrap();
        let first = h.directory_execute(Native::FsReaddir, &[d]).unwrap();
        assert_eq!(h.native_bytes(first).unwrap(), b"a");
        std::fs::write(dir.join("b"), b"b").unwrap();
        let next = h.directory_execute(Native::FsReaddir, &[d]).unwrap();
        assert_eq!(h.native_bytes(next).unwrap(), b"z");
        h.directory_execute(Native::FsRewinddir, &[d]).unwrap();
        h.directory_execute(Native::FsReaddir, &[d]).unwrap();
        let next = h.directory_execute(Native::FsReaddir, &[d]).unwrap();
        assert_eq!(h.native_bytes(next).unwrap(), b"b");
        h.directory_execute(Native::FsClosedir, &[d]).unwrap();
        assert_eq!(
            code(h.directory_execute(Native::FsReaddir, &[d])),
            "already_closed"
        );
        let a = h.native_byte_array(b"a".to_vec()).unwrap();
        let link = h.native_byte_array(b"alias".to_vec()).unwrap();
        h.directory_execute(Native::FsSymlink, &[a, link]).unwrap();
        let target = h.directory_execute(Native::FsReadlink, &[link]).unwrap();
        assert_eq!(h.native_bytes(target).unwrap(), b"a");
        assert_eq!(std::fs::read(dir.join("alias")).unwrap(), b"a");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
