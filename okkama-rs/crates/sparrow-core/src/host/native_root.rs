//! Descriptor-relative rooted access. No child operation reconstructs an ambient
//! path; each component is opened with NOFOLLOW and an owned Rust descriptor.
use crate::host::{
    Error,
    handle::Handle,
    {Outcome, ProviderError},
    native_fs,
    *,
};
use okkama_runtime_abi::{Native, NativeFailure};
use rustix::fs::{AtFlags, Mode, OFlags};
use std::{
    fs::File,
    io::{Read, Write},
    os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt},
    sync::{Arc, OnceLock},
};
#[derive(Debug)]
pub(crate) struct Rooted {
    /// Shared with a read in flight (#121), which keeps the descriptor open
    /// until it ends even if the entry is closed meanwhile.
    file: Option<Arc<File>>,
    /// A regular file, as the open saw it: a descriptor never changes kind,
    /// so reads need no stat of their own (#108).
    regular: bool,
    /// The metadata of the open's check, given to the first stat() instead
    /// of asking the descriptor again (#108).
    opened: Option<std::fs::Metadata>,
    /// How far the page cache was found to hold the file (#121).
    cached: okkama_runtime_core::host::CachedRange,
}
pub(crate) fn handles(n: Native) -> bool {
    n.symbol().starts_with("runtime/fs_root@1::") || n.symbol().starts_with("runtime/snapshot@1::")
}
fn domain(n: Native) -> &'static str {
    if n.symbol().starts_with("runtime/snapshot@") {
        "core.snapshot"
    } else {
        "core.fs.root"
    }
}
fn fail(n: Native, code: &str, message: &str) -> ProviderError {
    ProviderError::Native(NativeFailure {
        domain: domain(n).into(),
        code: code.into(),
        operation: n.symbol().into(),
        message: message.into(),
    })
}
fn host(n: Native, error: std::io::Error) -> ProviderError {
    match native_fs::host(n, error) {
        ProviderError::Native(mut e) => {
            e.domain = domain(n).into();
            ProviderError::Native(e)
        }
        other => other,
    }
}
/// Rooted reads off the VM thread at once (#121): more than the slow reads a
/// server usually has in flight, so a fast read does not wait behind them, and
/// still a bound on blocking threads and buffers.
const MAX_ROOTED_READS: usize = 64;
static READ_SLOTS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
/// An I/O error of a read on Tokio's blocking pool as the native's failure.
fn read_failure(n: Native, error: std::io::Error) -> NativeFailure {
    match host(n, error) {
        ProviderError::Native(failure) => failure,
        ProviderError::Engine(_) => NativeFailure {
            domain: domain(n).into(),
            code: "unexpected".into(),
            operation: n.symbol().into(),
            message: "Cannot read rooted file".into(),
        },
    }
}
fn component(s: &str) -> bool {
    !(s.is_empty() || s == "." || s == ".." || s.contains(['\0', '/', '\\']))
}
fn child(n: Native, parent: &File, name: &str, directory: bool) -> Result<(File, std::fs::Metadata), ProviderError> {
    let mut flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    if directory {
        flags |= OFlags::DIRECTORY;
    }
    let file: File = rustix::fs::openat(parent, name, flags, Mode::empty())
        .map_err(|e| host(n, e.into()))?
        .into();
    let meta = file.metadata().map_err(|e| host(n, e))?;
    if !(meta.is_file() || meta.is_dir()) {
        return Err(fail(
            n,
            "invalid_data",
            "Rooted entry must be regular file or directory",
        ));
    }
    Ok((file, meta))
}
impl Heap {
    fn rooted(&self, n: Native, v: Slot) -> Result<&File, ProviderError> {
        if v.kind() != HANDLE {
            return Err(Error("expected rooted native handle".into()).into());
        }
        let Handle::Rooted(state) = self.handles.get(v.bits)? else {
            return Err(Error("expected rooted native handle".into()).into());
        };
        state
            .file
            .as_deref()
            .ok_or_else(|| fail(n, "already_closed", "Rooted handle is closed"))
    }
    fn rooted_shared(&self, n: Native, v: Slot) -> Result<Arc<File>, ProviderError> {
        self.rooted_state(v)?
            .file
            .clone()
            .ok_or_else(|| fail(n, "already_closed", "Rooted handle is closed"))
    }
    /// Offset and limit of a rooted read, checked.
    fn rooted_coordinates(&self, n: Native, a: &[Slot]) -> Result<(u64, usize), ProviderError> {
        let offset = self
            .integer(a[1])?
            .to_i64()
            .filter(|v| (0..=9_007_199_254_740_991).contains(v))
            .ok_or_else(|| fail(n, "invalid_input", "Invalid rooted offset"))?;
        let limit = self
            .integer(a[2])?
            .to_i64()
            .filter(|v| (1..=1_048_576).contains(v))
            .ok_or_else(|| fail(n, "invalid_input", "Invalid rooted limit"))?;
        if !self.rooted_state(a[0])?.regular {
            return Err(fail(
                n,
                "invalid_input",
                "Only regular rooted files can be read",
            ));
        }
        Ok((offset as u64, limit as usize))
    }
    /// `rooted_entry_piece`: the read on Tokio's blocking pool (#121).
    fn rooted_piece(&mut self, n: Native, a: &[Slot]) -> Result<Slot, ProviderError> {
        let (offset, limit) = self.rooted_coordinates(n, a)?;
        let file = self.rooted_shared(n, a[0])?;
        // From the page cache at once; only a read that would wait for the
        // disk goes to the blocking pool.
        let Handle::Rooted(state) = self.handles.get_mut(a[0].bits)? else {
            return Err(Error("expected rooted native handle".into()).into());
        };
        if let Some(bytes) = state.cached.read(&file, offset, limit) {
            let bytes = self.native_byte_array(bytes)?;
            return Ok(self.future_with_state(State::Ready(Ok(bytes)))?);
        }
        let slots = READ_SLOTS
            .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(MAX_ROOTED_READS)))
            .clone();
        Ok(self.native_io(async move {
            let lost = || NativeFailure {
                domain: domain(n).into(),
                code: "unexpected".into(),
                operation: n.symbol().into(),
                message: "The rooted read was lost".into(),
            };
            let permit = slots.acquire_owned().await.map_err(|_| lost())?;
            let read = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let mut bytes = vec![0; limit];
                file.read_at(&mut bytes, offset).map(|size| {
                    bytes.truncate(size);
                    bytes
                })
            });
            match read.await {
                Ok(Ok(bytes)) => Ok(bytes),
                Ok(Err(error)) => Err(read_failure(n, error)),
                Err(_) => Err(lost()),
            }
        })?)
    }
    fn root_new(&mut self, file: File, opened: Option<std::fs::Metadata>) -> Result<Slot, Error> {
        let regular = opened.as_ref().is_some_and(|meta| meta.is_file());
        Ok(Slot {
            tag: HANDLE,
            bits: self.handles.allocate(Handle::Rooted(Rooted {
                file: Some(Arc::new(file)),
                regular,
                opened,
                cached: Default::default(),
            }))?,
        })
    }
    fn rooted_state(&self, v: Slot) -> Result<&Rooted, ProviderError> {
        if v.kind() != HANDLE {
            return Err(Error("expected rooted native handle".into()).into());
        }
        let Handle::Rooted(state) = self.handles.get(v.bits)? else {
            return Err(Error("expected rooted native handle".into()).into());
        };
        Ok(state)
    }
    /// The metadata kept from the open, once; afterwards the descriptor's own.
    fn entry_metadata(&mut self, n: Native, v: Slot) -> Result<std::fs::Metadata, ProviderError> {
        if v.kind() != HANDLE {
            return Err(Error("expected rooted native handle".into()).into());
        }
        let Handle::Rooted(state) = self.handles.get_mut(v.bits)? else {
            return Err(Error("expected rooted native handle".into()).into());
        };
        let file = state
            .file
            .as_ref()
            .ok_or_else(|| fail(n, "already_closed", "Rooted handle is closed"))?;
        match state.opened.take() {
            Some(meta) => Ok(meta),
            None => file.metadata().map_err(|e| host(n, e)),
        }
    }
    pub(crate) fn native_root(&mut self, n: Native, a: &[Slot]) -> Result<Outcome, Error> {
        match self.root_execute(n, a) {
            Ok(v) => Ok(Outcome::Value(v)),
            Err(ProviderError::Engine(e)) => Err(e),
            Err(ProviderError::Native(e)) => {
                let payload = self.native_failure_new(e)?;
                self.failure_new(crate::host::FailureKind::Native(payload))
                    .map(Outcome::Failure)
            }
        }
    }
    fn root_execute(&mut self, n: Native, a: &[Slot]) -> Result<Slot, ProviderError> {
        use Native::*;
        if matches!(n, FsRootFileRootOpen | SnapshotSnapshotRootOpen) {
            let bytes = self.native_bytes(a[0])?;
            if bytes.is_empty() || bytes.len() > 4096 || bytes.contains(&0) {
                return Err(fail(n, "invalid_input", "Invalid root path"));
            }
            let path = self
                .working_directory()?
                .join(String::from_utf8_lossy(&bytes).as_ref());
            let file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(path)
                .map_err(|e| host(n, e))?;
            return Ok(self.root_new(file, None)?);
        }
        if matches!(
            n,
            FsRootFileRootClosed
                | FsRootRootedEntryClosed
                | FsRootFileRootClose
                | FsRootRootedEntryClose
                | SnapshotSnapshotRootClose
        ) {
            if a[0].kind() != HANDLE {
                return Err(Error("expected rooted handle".into()).into());
            }
            let Handle::Rooted(state) = self.handles.get_mut(a[0].bits)? else {
                return Err(Error("expected rooted handle".into()).into());
            };
            if matches!(n, FsRootFileRootClosed | FsRootRootedEntryClosed) {
                return Ok(Slot::boolean(state.file.is_none()));
            }
            state.file.take();
            return Ok(Slot::boolean(true));
        }
        if n == FsRootRootedEntryPiece {
            return self.rooted_piece(n, a);
        }
        let root = self.rooted(n, a[0])?;
        match n {
            FsRootFileRootEntry => {
                let parts = self.elements(a[1])?;
                if parts.len() > 256 {
                    return Err(fail(
                        n,
                        "invalid_input",
                        "Root coordinate component budget exceeded",
                    ));
                }
                let mut width = 0;
                let mut names = Vec::with_capacity(parts.len());
                for part in parts {
                    let name = self.text(part)?;
                    width += name.len() + 1;
                    if !component(name) || name.len() > 255 || width > 4096 {
                        return Err(fail(n, "invalid_input", "Invalid root coordinate"));
                    }
                    names.push(name);
                }
                let mut file = root.try_clone().map_err(|e| host(n, e))?;
                let mut opened = None;
                for (i, name) in names.iter().enumerate() {
                    let (next, meta) = child(n, &file, name, i + 1 < names.len())?;
                    file = next;
                    opened = Some(meta);
                }
                Ok(self.root_new(file, opened)?)
            }
            FsRootRootedEntryStat => {
                let m = self.entry_metadata(n, a[0])?;
                let kind = if m.is_file() {
                    1
                } else if m.is_dir() {
                    2
                } else {
                    0
                };
                let pairs = [
                    ("kind", kind),
                    ("size", m.size() as i64),
                    ("permissions", i64::from(m.mode() & 0o777)),
                    ("accessed_sec", m.atime()),
                    ("modified_sec", m.mtime()),
                    ("changed_sec", m.ctime()),
                    ("created_sec", -1),
                    ("device", m.dev() as i64),
                    ("inode", m.ino() as i64),
                ];
                let entries = pairs
                    .into_iter()
                    .map(|(k, v)| self.allocate_text(k.into()).map(|k| (k, Slot::small(v))))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(self.map_new(entries.into_iter())?)
            }
            FsRootRootedEntryRead => {
                let (offset, limit) = self.rooted_coordinates(n, a)?;
                let mut bytes = vec![0; limit];
                let size = root
                    .read_at(&mut bytes, offset)
                    .map_err(|e| host(n, e))?;
                bytes.truncate(size);
                Ok(self.native_byte_array(bytes)?)
            }
            FsRootRootedEntryNames => {
                let max = self
                    .integer(a[1])?
                    .to_i64()
                    .filter(|v| (0..=4096).contains(v))
                    .ok_or_else(|| fail(n, "invalid_input", "Invalid directory entry budget"))?
                    as usize;
                if !root.metadata().map_err(|e| host(n, e))?.is_dir() {
                    return Err(fail(n, "invalid_input", "Rooted entry is not a directory"));
                }
                let mut entries =
                    rustix::fs::Dir::read_from(root).map_err(|e| host(n, e.into()))?;
                let mut names = vec![];
                for entry in &mut entries {
                    let entry = entry.map_err(|e| host(n, e.into()))?;
                    let bytes = entry.file_name().to_bytes();
                    if matches!(bytes, b"." | b"..") {
                        continue;
                    }
                    if names.len() == max {
                        return Err(fail(
                            n,
                            "resource_exhausted",
                            "Directory entry budget exceeded",
                        ));
                    }
                    names.push(String::from_utf8_lossy(bytes).into_owned());
                }
                names.sort();
                let slots = names
                    .into_iter()
                    .map(|s| self.allocate_text(s))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(self.allocate_array(slots)?)
            }
            SnapshotSnapshotRead | SnapshotSnapshotWrite => {
                let name = self.text(a[1])?;
                if !component(name) || name.len() > 255 {
                    return Err(fail(
                        n,
                        "invalid_input",
                        "Snapshot name must be one component of at most 255 bytes",
                    ));
                }
                let existing = match child(n, root, name, false) {
                    Ok((file, meta)) => {
                        if !meta.is_file() {
                            return Err(fail(n, "invalid_data", "Snapshot must be a regular file"));
                        }
                        Some(file)
                    }
                    Err(ProviderError::Native(e)) if e.code == "not_found" => None,
                    Err(e) => return Err(e),
                };
                if n == SnapshotSnapshotRead {
                    let limit = self
                        .integer(a[2])?
                        .to_i64()
                        .filter(|v| (1..=1_048_576).contains(v))
                        .ok_or_else(|| fail(n, "invalid_input", "Invalid snapshot read limit"))?
                        as u64;
                    let file =
                        existing.ok_or_else(|| fail(n, "not_found", "Snapshot is missing"))?;
                    let mut bytes = vec![];
                    file.take(limit + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|e| host(n, e))?;
                    if bytes.len() as u64 > limit {
                        return Err(fail(n, "resource_exhausted", "Snapshot exceeds read limit"));
                    }
                    return Ok(self.native_byte_array(bytes)?);
                }
                let content = self.native_bytes(a[2])?;
                if content.len() > 1_048_576 {
                    return Err(fail(
                        n,
                        "resource_exhausted",
                        "Snapshot exceeds 1048576 bytes",
                    ));
                }
                let mut temporary = None;
                for _ in 0..64 {
                    let random = okkama_native::call("random", &[b"16"])
                        .map_err(|_| fail(n, "unexpected", "Random source unavailable"))?;
                    let name = format!(
                        ".okkama-snapshot-{}",
                        random
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<String>()
                    );
                    match rustix::fs::openat(
                        root,
                        name.as_str(),
                        OFlags::WRONLY
                            | OFlags::CREATE
                            | OFlags::EXCL
                            | OFlags::CLOEXEC
                            | OFlags::NOFOLLOW,
                        Mode::RUSR | Mode::WUSR,
                    ) {
                        Ok(fd) => {
                            temporary = Some((name, File::from(fd)));
                            break;
                        }
                        Err(e) if e == rustix::io::Errno::EXIST => {}
                        Err(e) => return Err(host(n, e.into())),
                    }
                }
                let (temporary, mut file) = temporary.ok_or_else(|| {
                    fail(
                        n,
                        "resource_exhausted",
                        "Snapshot temporary collision budget",
                    )
                })?;
                let result = file
                    .write_all(&content)
                    .and_then(|()| file.sync_all())
                    .and_then(|()| {
                        rustix::fs::renameat(root, temporary.as_str(), root, name)
                            .map_err(Into::into)
                    });
                if let Err(e) = result {
                    let _ = rustix::fs::unlinkat(root, temporary.as_str(), AtFlags::empty());
                    return Err(host(n, e));
                }
                Ok(Slot::boolean(true))
            }
            _ => Err(Error("unsupported rooted filesystem native".into()).into()),
        }
    }
}
