//! Host paths and directory snapshots, scoped to each VM's working directory.
use crate::host::{
    Error,
    handle::Handle,
    ProviderError,
    native_fs::{fail, host},
    *,
};
use okkama_runtime_abi::Native;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Debug)]
pub(crate) struct Directory {
    path: PathBuf,
    names: Vec<String>,
    index: usize,
    closed: bool,
}
fn names(path: &Path) -> std::io::Result<Vec<String>> {
    let mut out = fs::read_dir(path)?
        .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    out.sort();
    Ok(out)
}
pub(crate) fn handles(n: Native) -> bool {
    use Native::*;
    matches!(
        n,
        FsChown
            | FsCwd
            | FsChdir
            | FsMkdir
            | FsMkdirAll
            | FsRmdir
            | FsUnlink
            | FsRename
            | FsRenameNoreplace
            | FsCopyFile
            | FsLink
            | FsSymlink
            | FsReadlink
            | FsChmod
            | FsStat
            | FsLstat
            | FsOpendir
            | FsReaddir
            | FsRewinddir
            | FsClosedir
            | FsDirectoryEntries
            | FsDirectoryRemoveTree
            | FsFileContent
    )
}
impl Heap {
    fn directory(&mut self, v: Slot) -> Result<&mut Directory, Error> {
        if v.kind() != HANDLE {
            return Err(Error("expected directory handle".into()));
        }
        match self.handles.get_mut(v.bits)? {
            Handle::Directory(d) => Ok(d),
            _ => Err(Error("expected directory handle".into())),
        }
    }
    pub(crate) fn directory_execute(
        &mut self,
        n: Native,
        a: &[Slot],
    ) -> Result<Slot, ProviderError> {
        use Native::*;
        let yes = Slot::boolean(true);
        if n == FsCwd {
            let path = self.working_directory()?;
            return Ok(self.native_byte_array(path.to_string_lossy().as_bytes().to_vec())?);
        }
        if matches!(n, FsReaddir | FsRewinddir | FsClosedir) {
            let d = self.directory(a[0])?;
            if n == FsClosedir {
                d.closed = true;
                d.names.clear();
                return Ok(yes);
            }
            if d.closed {
                return Err(fail(n, "already_closed", "Directory is closed"));
            }
            if n == FsRewinddir {
                d.names = names(&d.path).unwrap_or_default();
                d.index = 0;
                return Ok(yes);
            }
            let bytes = d
                .names
                .get(d.index)
                .map(|s| s.as_bytes().to_vec())
                .unwrap_or_default();
            d.index = d.index.saturating_add(1).min(d.names.len());
            return Ok(self.native_byte_array(bytes)?);
        }
        let path = self.fs_path(n, a[0])?;
        match n {
            FsChdir => {
                let real = fs::canonicalize(path).map_err(|e| host(n, e))?;
                if !real.is_dir() {
                    return Err(fail(
                        n,
                        "invalid_input",
                        "Working directory must be a directory",
                    ));
                }
                #[cfg(unix)]
                nix::unistd::access(&real, nix::unistd::AccessFlags::X_OK)
                    .map_err(|e| host(n, e.into()))?;
                okkama_runtime_core::host::set_cwd(real);
            }
            FsMkdir | FsMkdirAll | FsChmod => {
                let mode = self
                    .integer(a[1])?
                    .to_i64()
                    .filter(|v| (0..=0o777).contains(v))
                    .ok_or_else(|| fail(n, "invalid_input", "Invalid permission mode"))?
                    as u32;
                #[cfg(unix)]
                {
                    if n == FsMkdir || n == FsMkdirAll {
                        // mkdir_all makes every missing parent; an existing
                        // directory is then no failure (#125).
                        fs::DirBuilder::new().recursive(n == FsMkdirAll).mode(mode).create(path)
                    } else {
                        fs::set_permissions(path, fs::Permissions::from_mode(mode))
                    }
                    .map_err(|e| host(n, e))?;
                }
                #[cfg(not(unix))]
                {
                    let _ = (mode, path);
                    return Err(fail(
                        n,
                        "unsupported",
                        "Filesystem permission mode unavailable",
                    ));
                }
            }
            FsRmdir => {
                if fs::symlink_metadata(&path).is_ok_and(|m| !m.is_dir()) {
                    return Err(fail(n, "invalid_input", "Not a directory"));
                }
                fs::remove_dir(path).map_err(|e| host(n, e))?;
            }
            FsUnlink => {
                if fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) {
                    return Err(fail(n, "permission_denied", "Cannot unlink directory"));
                }
                fs::remove_file(path).map_err(|e| host(n, e))?;
            }
            FsRename | FsRenameNoreplace | FsLink => {
                let target = self.fs_path(n, a[1])?;
                if n == FsRename {
                    fs::rename(path, target)
                } else if n == FsRenameNoreplace {
                    okkama_runtime_core::native::fs::rename_noreplace(&path, &target)
                } else {
                    fs::hard_link(path, target)
                }
                .map_err(|e| host(n, e))?;
            }
            FsCopyFile => {
                let target = self.fs_path(n, a[1])?;
                okkama_runtime_core::native::fs::copy_file(&path, &target, a[2].bits != 0)
                    .map_err(|e| host(n, e))?;
            }
            FsSymlink => {
                let raw = self.native_bytes(a[0])?;
                let target = String::from_utf8_lossy(&raw);
                let link = self.fs_path(n, a[1])?;
                #[cfg(unix)]
                std::os::unix::fs::symlink(target.as_ref(), link).map_err(|e| host(n, e))?;
                #[cfg(not(unix))]
                {
                    let _ = (target, link);
                    return Err(fail(n, "unsupported", "Symlink creation unavailable"));
                }
            }
            FsChown => {
                use num_traits::ToPrimitive;
                let owner = crate::host::native_file::big(self.integer(a[1])?);
                let group = crate::host::native_file::big(self.integer(a[2])?);
                if owner < (-1).into() || group < (-1).into() {
                    return Err(fail(n, "invalid_input", "Invalid owner or group"));
                }
                let id = |v: num_bigint::BigInt| {
                    if v == (-1).into() {
                        None
                    } else {
                        Some(
                            (v & num_bigint::BigInt::from(u32::MAX))
                                .to_u32()
                                .expect("masked"),
                        )
                    }
                };
                let (owner, group) = (id(owner), id(group));
                #[cfg(unix)]
                if owner.is_none() && group.is_none() {
                    fs::metadata(path).map_err(|e| host(n, e))?;
                } else {
                    std::os::unix::fs::chown(path, owner, group).map_err(|e| host(n, e))?;
                }
                #[cfg(not(unix))]
                {
                    let _ = (path, owner, group);
                    return Err(fail(n, "unsupported", "POSIX ownership unavailable"));
                }
            }
            FsReadlink => {
                let target = fs::read_link(path).map_err(|e| host(n, e))?;
                return Ok(self.native_byte_array(target.to_string_lossy().as_bytes().to_vec())?);
            }
            FsDirectoryRemoveTree => {
                let meta = fs::symlink_metadata(&path).map_err(|e| host(n, e))?;
                if !meta.is_dir() {
                    return Err(fail(n, "invalid_input", "Tree root must be a directory"));
                }
                fs::remove_dir_all(path).map_err(|e| host(n, e))?;
            }
            FsFileContent => {
                return Ok(self.native_byte_array(fs::read(path).map_err(|e| host(n, e))?)?);
            }
            FsOpendir | FsDirectoryEntries => {
                let names = names(&path).map_err(|e| host(n, e))?;
                if n == FsOpendir {
                    return Ok(Slot {
                        tag: HANDLE,
                        bits: self.handles.allocate(Handle::Directory(Directory {
                            path,
                            names,
                            index: 0,
                            closed: false,
                        }))?,
                    });
                }
                let values = names
                    .into_iter()
                    .map(|s| self.allocate_text(s))
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(self.allocate_array(values)?);
            }
            FsStat | FsLstat => {
                let meta = if n == FsStat {
                    fs::metadata(path)
                } else {
                    fs::symlink_metadata(path)
                }
                .map_err(|e| host(n, e))?;
                #[cfg(unix)]
                {
                    let kind = match meta.mode() & 0o170000 {
                        0o100000 => 1,
                        0o040000 => 2,
                        0o120000 => 3,
                        0o060000 => 4,
                        0o020000 => 5,
                        0o010000 => 6,
                        0o140000 => 7,
                        _ => 0,
                    };
                    let pairs = [
                        ("kind", kind),
                        ("size", meta.size() as i64),
                        ("permissions", i64::from(meta.mode() & 0o777)),
                        ("accessed_sec", meta.atime()),
                        ("modified_sec", meta.mtime()),
                        ("changed_sec", meta.ctime()),
                        ("created_sec", -1),
                        ("device", meta.dev() as i64),
                        ("inode", meta.ino() as i64),
                    ];
                    let entries = pairs
                        .into_iter()
                        .map(|(k, v)| self.allocate_text(k.into()).map(|k| (k, Slot::small(v))))
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(self.map_new(entries.into_iter())?);
                }
                #[cfg(not(unix))]
                {
                    let _ = meta;
                    return Err(fail(n, "unsupported", "POSIX metadata unavailable"));
                }
            }
            _ => return Err(Error("unsupported directory native".into()).into()),
        }
        Ok(yes)
    }
}
