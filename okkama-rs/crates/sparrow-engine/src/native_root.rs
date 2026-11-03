//! Descriptor-relative rooted access. No child operation reconstructs an ambient
//! path; each component is opened with NOFOLLOW and an owned Rust descriptor.
use crate::{
    Error,
    handle::Handle,
    native::{Outcome, ProviderError},
    native_fs,
    value::*,
};
use okkama_runtime_abi::{Native, NativeFailure};
use rustix::fs::{AtFlags, Mode, OFlags};
use std::{
    fs::File,
    io::{Read, Write},
    os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt},
};
#[derive(Debug)]
pub(crate) struct Rooted {
    file: Option<File>,
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
fn component(s: &str) -> bool {
    !(s.is_empty() || s == "." || s == ".." || s.contains(['\0', '/', '\\']))
}
fn child(n: Native, parent: &File, name: &str, directory: bool) -> Result<File, ProviderError> {
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
    Ok(file)
}
impl Heap {
    fn rooted(&self, n: Native, v: Slot) -> Result<&File, ProviderError> {
        if v.tag != HANDLE {
            return Err(Error("expected rooted native handle".into()).into());
        }
        let Handle::Rooted(state) = self.handles.get(v.bits)? else {
            return Err(Error("expected rooted native handle".into()).into());
        };
        state
            .file
            .as_ref()
            .ok_or_else(|| fail(n, "already_closed", "Rooted handle is closed"))
    }
    fn root_new(&mut self, file: File) -> Result<Slot, Error> {
        Ok(Slot {
            tag: HANDLE,
            bits: self
                .handles
                .allocate(Handle::Rooted(Rooted { file: Some(file) }))?,
        })
    }
    pub(crate) fn native_root(&mut self, n: Native, a: &[Slot]) -> Result<Outcome, Error> {
        match self.root_execute(n, a) {
            Ok(v) => Ok(Outcome::Value(v)),
            Err(ProviderError::Engine(e)) => Err(e),
            Err(ProviderError::Native(e)) => {
                let payload = self.native_failure_new(e)?;
                self.failure_new(crate::failure::FailureKind::Native(payload))
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
            return Ok(self.root_new(file)?);
        }
        if matches!(
            n,
            FsRootFileRootClosed
                | FsRootRootedEntryClosed
                | FsRootFileRootClose
                | FsRootRootedEntryClose
                | SnapshotSnapshotRootClose
        ) {
            if a[0].tag != HANDLE {
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
                    let name = self.text(*part)?;
                    width += name.len() + 1;
                    if !component(name) || name.len() > 255 || width > 4096 {
                        return Err(fail(n, "invalid_input", "Invalid root coordinate"));
                    }
                    names.push(name);
                }
                let mut file = root.try_clone().map_err(|e| host(n, e))?;
                for (i, name) in names.iter().enumerate() {
                    file = child(n, &file, name, i + 1 < names.len())?;
                }
                Ok(self.root_new(file)?)
            }
            FsRootRootedEntryStat => {
                let m = root.metadata().map_err(|e| host(n, e))?;
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
                if !root.metadata().map_err(|e| host(n, e))?.is_file() {
                    return Err(fail(
                        n,
                        "invalid_input",
                        "Only regular rooted files can be read",
                    ));
                }
                let mut bytes = vec![0; limit as usize];
                let size = root
                    .read_at(&mut bytes, offset as u64)
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
                    Ok(file) => {
                        if !file.metadata().map_err(|e| host(n, e))?.is_file() {
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
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    fn directory() -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "sparrow-root-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        p
    }
    fn open(h: &mut Heap, n: Native, path: &std::path::Path) -> Slot {
        let p = h
            .native_byte_array(path.to_string_lossy().as_bytes().to_vec())
            .unwrap();
        h.root_execute(n, &[p]).unwrap()
    }
    fn parts(h: &mut Heap, names: &[&str]) -> Slot {
        let names = names
            .iter()
            .map(|s| h.allocate_text((*s).into()).unwrap())
            .collect();
        h.allocate_array(names).unwrap()
    }
    fn code(result: Result<Slot, ProviderError>) -> String {
        match result {
            Err(ProviderError::Native(e)) => e.code,
            other => panic!("expected failure: {other:?}"),
        }
    }
    #[test]
    fn retained_descriptors_survive_root_close_and_ambient_path_replacement() {
        let dir = directory();
        let original = dir.join("root");
        let moved = dir.join("moved");
        std::fs::create_dir(&original).unwrap();
        std::fs::write(original.join("data"), b"original").unwrap();
        let mut h = Heap::new(&[], &[]);
        let root = open(&mut h, Native::FsRootFileRootOpen, &original);
        std::fs::rename(&original, &moved).unwrap();
        std::fs::create_dir(&original).unwrap();
        std::fs::write(original.join("data"), b"replacement").unwrap();
        let names = parts(&mut h, &["data"]);
        let entry = h
            .root_execute(Native::FsRootFileRootEntry, &[root, names])
            .unwrap();
        let empty = parts(&mut h, &[]);
        let retained = h
            .root_execute(Native::FsRootFileRootEntry, &[root, empty])
            .unwrap();
        for _ in 0..2 {
            let names = h
                .root_execute(Native::FsRootRootedEntryNames, &[retained, Slot::small(1)])
                .unwrap();
            assert_eq!(h.elements(names).unwrap().len(), 1);
        }
        h.root_execute(Native::FsRootFileRootClose, &[root])
            .unwrap();
        h.collect([entry, retained].into_iter()).unwrap();
        std::fs::remove_file(moved.join("data")).unwrap();
        std::fs::write(moved.join("data"), b"new inode").unwrap();
        let bytes = h
            .root_execute(
                Native::FsRootRootedEntryRead,
                &[entry, Slot::small(0), Slot::small(100)],
            )
            .unwrap();
        assert_eq!(h.native_bytes(bytes).unwrap(), b"original");
        h.root_execute(Native::FsRootRootedEntryClose, &[entry])
            .unwrap();
        assert_eq!(
            code(h.root_execute(
                Native::FsRootRootedEntryRead,
                &[entry, Slot::small(0), Slot::small(1)]
            )),
            "already_closed"
        );
        h.root_execute(Native::FsRootRootedEntryClose, &[retained])
            .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn root_coordinates_reject_links_parent_components_and_special_files() {
        let dir = directory();
        std::fs::create_dir(dir.join("inside")).unwrap();
        std::fs::write(dir.join("inside/file"), b"data").unwrap();
        std::os::unix::fs::symlink("inside", dir.join("link")).unwrap();
        nix::unistd::mkfifo(
            &dir.join("pipe"),
            nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
        )
        .unwrap();
        let mut h = Heap::new(&[], &[]);
        let root = open(&mut h, Native::FsRootFileRootOpen, &dir);
        for names in [
            vec![".."],
            vec!["inside/file"],
            vec!["link", "file"],
            vec!["link"],
            vec!["pipe"],
        ] {
            let p = parts(&mut h, &names);
            assert!(
                h.root_execute(Native::FsRootFileRootEntry, &[root, p])
                    .is_err()
            );
        }
        let empty = parts(&mut h, &[]);
        let entry = h
            .root_execute(Native::FsRootFileRootEntry, &[root, empty])
            .unwrap();
        assert_eq!(
            code(h.root_execute(Native::FsRootRootedEntryNames, &[entry, Slot::small(0)])),
            "resource_exhausted"
        );
        h.root_execute(Native::FsRootFileRootClose, &[root])
            .unwrap();
        h.root_execute(Native::FsRootRootedEntryClose, &[entry])
            .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn snapshot_atomic_replace_and_bounded_read_stay_under_retained_root() {
        let dir = directory();
        let source = dir.join("root");
        std::fs::create_dir(&source).unwrap();
        let mut h = Heap::new(&[], &[]);
        let root = open(&mut h, Native::SnapshotSnapshotRootOpen, &source);
        let moved = dir.join("moved");
        std::fs::rename(&source, &moved).unwrap();
        std::fs::create_dir(&source).unwrap();
        let name = h.allocate_text("state".into()).unwrap();
        let bytes = h.native_byte_array(b"payload".to_vec()).unwrap();
        for _ in 0..2 {
            h.root_execute(Native::SnapshotSnapshotWrite, &[root, name, bytes])
                .unwrap();
        }
        assert!(!source.join("state").exists());
        assert_eq!(std::fs::read(moved.join("state")).unwrap(), b"payload");
        assert_eq!(std::fs::read_dir(&moved).unwrap().count(), 1);
        assert_eq!(
            code(h.root_execute(Native::SnapshotSnapshotRead, &[root, name, Slot::small(3)])),
            "resource_exhausted"
        );
        let read = h
            .root_execute(Native::SnapshotSnapshotRead, &[root, name, Slot::small(7)])
            .unwrap();
        assert_eq!(h.native_bytes(read).unwrap(), b"payload");
        std::fs::remove_file(moved.join("state")).unwrap();
        std::os::unix::fs::symlink(source.join("outside"), moved.join("state")).unwrap();
        assert!(
            h.root_execute(Native::SnapshotSnapshotWrite, &[root, name, bytes])
                .is_err()
        );
        assert!(!source.join("outside").exists());
        h.root_execute(Native::SnapshotSnapshotRootClose, &[root])
            .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}
