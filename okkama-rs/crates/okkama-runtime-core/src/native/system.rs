//! `runtime/fs_root` (доступ под корнем без следования ссылкам),
//! `runtime/snapshot` (атомарная замена файла) и `runtime/environment` —
//! перенос `SystemNatives` JVM. Каждый компонент пути проверяется без
//! следования ссылкам (как NIO `NOFOLLOW_LINKS`).
use super::support::{
    array, byte_array, bytes, entries, failure, handle, int, int_map, items, new_handle, string,
    string_array, string_map, text,
};
use super::Error;
use crate::host;
use crate::{Value, abi::Native};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

fn operation(native: Native) -> &'static str {
    native
        .symbol()
        .split("::")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("")
}

/// Ошибка как `_errno` Perl: код и текст `strerror`.
struct HostError(&'static str, String);

fn loop_error() -> HostError {
    HostError("invalid_input", "Too many levels of symbolic links".into())
}

fn errno(domain: &str, native: Native, message: &str, error: HostError) -> Error {
    failure(domain, error.0, native, format!("{message}: {}", error.1))
}

fn io_error(error: &std::io::Error) -> HostError {
    let (code, text) = host::reason(error);
    HostError(code, text)
}

fn symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

/// Открытие компонента: без ссылок; `directory` — только каталог.
fn open_check(path: &Path, directory: bool) -> Result<(), HostError> {
    if symlink(path) {
        return Err(loop_error());
    }
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return Err(HostError("not_found", "No such file or directory".into()));
    };
    if directory && !meta.is_dir() {
        return Err(HostError("invalid_input", "Not a directory".into()));
    }
    if !host::readable(path) {
        return Err(HostError("permission_denied", "Permission denied".into()));
    }
    Ok(())
}

#[derive(Debug)]
struct Rooted {
    path: PathBuf,
    closed: Cell<bool>,
}

const ROOT: &str = "core.fs.root";
const SNAPSHOT: &str = "core.snapshot";
const ENVIRONMENT: &str = "core.environment";

fn rooted<'a>(native: Native, value: &'a Value, domain: &str) -> Result<&'a Rooted, Error> {
    let state = handle::<Rooted>(value)?;
    if state.closed.get() {
        return Err(failure(domain, "already_closed", native, "Native handle is already closed"));
    }
    Ok(state)
}

fn root_path(native: Native, value: &Value, domain: &str, message: &str) -> Result<PathBuf, Error> {
    let raw = bytes(value)?;
    if raw.is_empty() || raw.len() > 4096 || raw.contains(&0) {
        return Err(failure(domain, "invalid_input", native, message));
    }
    Ok(host::path(&String::from_utf8_lossy(&raw)))
}

fn component(part: &str) -> bool {
    !(part.is_empty() || part == "." || part == ".." || part.contains(['/', '\\', '\0']))
}

fn stat(native: Native, entry: &Rooted) -> Result<Value, Error> {
    let meta = std::fs::symlink_metadata(&entry.path)
        .map_err(|error| errno(ROOT, native, "Cannot stat rooted descriptor", io_error(&error)))?;
    let mode = meta.mode();
    let kind = match mode & 0o170000 {
        0o100000 => 1,
        0o040000 => 2,
        _ => 0,
    };
    Ok(int_map([
        ("kind", kind),
        ("size", meta.size() as i64),
        ("permissions", i64::from(mode & 0o777)),
        ("accessed_sec", meta.atime()),
        ("modified_sec", meta.mtime()),
        ("changed_sec", meta.ctime()),
        ("created_sec", -1),
        ("device", meta.dev() as i64),
        ("inode", meta.ino() as i64),
    ]))
}

fn read_at(native: Native, entry: &Rooted, offset: &crate::IntValue, limit: &crate::IntValue) -> Result<Value, Error> {
    let offset = offset.to_i64().filter(|offset| (0..=9_007_199_254_740_991).contains(offset));
    let limit = limit.to_i64().filter(|limit| (1..=1_048_576).contains(limit));
    let (Some(offset), Some(limit)) = (offset, limit) else {
        return Err(failure(ROOT, "invalid_input", native, "Invalid rooted read coordinates"));
    };
    let regular = std::fs::symlink_metadata(&entry.path).is_ok_and(|meta| meta.is_file());
    if !regular {
        return Err(failure(ROOT, "invalid_input", native, "Only regular rooted files can be read"));
    }
    use std::os::unix::fs::{FileExt, OpenOptionsExt};
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&entry.path)
        .map_err(|error| errno(ROOT, native, "Cannot read rooted file", io_error(&error)))?;
    let mut buffer = vec![0; limit as usize];
    let count = file
        .read_at(&mut buffer, offset as u64)
        .map_err(|error| errno(ROOT, native, "Cannot read rooted file", io_error(&error)))?;
    buffer.truncate(count);
    Ok(byte_array(buffer))
}

fn names(native: Native, entry: &Rooted, maximum: &crate::IntValue) -> Result<Value, Error> {
    let maximum = maximum
        .to_i64()
        .filter(|maximum| (0..=4096).contains(maximum))
        .ok_or_else(|| failure(ROOT, "invalid_input", native, "Invalid directory entry budget"))?;
    let directory = std::fs::symlink_metadata(&entry.path).is_ok_and(|meta| meta.is_dir());
    if !directory {
        return Err(failure(ROOT, "invalid_input", native, "Rooted entry is not a directory"));
    }
    let listing = std::fs::read_dir(&entry.path)
        .map_err(|error| errno(ROOT, native, "Cannot enumerate retained directory", io_error(&error)))?;
    let mut names = Vec::new();
    for child in listing {
        let child =
            child.map_err(|error| errno(ROOT, native, "Cannot enumerate retained directory", io_error(&error)))?;
        if names.len() as i64 >= maximum {
            return Err(failure(ROOT, "resource_exhausted", native, "Directory entry budget"));
        }
        names.push(child.file_name().to_string_lossy().to_string());
    }
    names.sort();
    Ok(string_array(names))
}

fn snapshot_name<'a>(native: Native, text: &'a str) -> Result<&'a str, Error> {
    if !component(text) {
        return Err(failure(
            SNAPSHOT,
            "invalid_input",
            native,
            "Snapshot name must be one relative component",
        ));
    }
    if text.len() > 255 {
        return Err(failure(
            SNAPSHOT,
            "invalid_input",
            native,
            "Snapshot name must be Unicode and at most 255 bytes",
        ));
    }
    Ok(text)
}

/// Существующий снимок: путь регулярного файла или None.
fn existing(native: Native, root: &Rooted, name: &str) -> Result<Option<PathBuf>, Error> {
    let path = root.path.join(name);
    if symlink(&path) {
        return Err(errno(SNAPSHOT, native, "Cannot open no-follow snapshot", loop_error()));
    }
    let Ok(meta) = std::fs::symlink_metadata(&path) else {
        return Ok(None);
    };
    if !meta.is_file() {
        return Err(failure(SNAPSHOT, "invalid_data", native, "Snapshot must be a regular file"));
    }
    Ok(Some(path))
}

thread_local! {
    static SERIAL: Cell<u64> = const { Cell::new(0) };
}

fn snapshot_write(native: Native, root: &Rooted, name: &str, content: &[u8]) -> Result<Value, Error> {
    if content.len() > 1_048_576 {
        return Err(failure(SNAPSHOT, "resource_exhausted", native, "Snapshot exceeds 1048576 bytes"));
    }
    existing(native, root, name)?;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut temporary = None;
    for _ in 0..64 {
        let serial = SERIAL.with(|serial| {
            serial.set(serial.get() + 1);
            serial.get()
        });
        let candidate = root.path.join(format!(".okkama-snapshot-{}-{serial}", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&candidate)
        {
            Ok(file) => {
                temporary = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(errno(SNAPSHOT, native, "Cannot create snapshot temporary", io_error(&error)));
            }
        }
    }
    let Some((path, mut file)) = temporary else {
        return Err(failure(SNAPSHOT, "resource_exhausted", native, "Snapshot temporary collision budget"));
    };
    let result = file
        .write_all(content)
        .and_then(|()| file.sync_all())
        .map_err(|error| errno(SNAPSHOT, native, "Cannot write snapshot temporary", io_error(&error)))
        .and_then(|()| {
            std::fs::rename(&path, root.path.join(name))
                .map_err(|error| errno(SNAPSHOT, native, "Cannot replace snapshot", io_error(&error)))
        });
    if let Err(error) = result {
        let _ = std::fs::remove_file(&path);
        return Err(error);
    }
    Ok(Value::Bool(true))
}

fn text_name(native: Native, name: &str) -> Result<String, Error> {
    if name.is_empty() {
        return Err(failure(ENVIRONMENT, "invalid_input", native, "Environment name must not be empty"));
    }
    if name.contains('\0') {
        return Err(failure(ENVIRONMENT, "invalid_input", native, "Environment name contains a zero byte"));
    }
    if name.contains('=') {
        return Err(failure(ENVIRONMENT, "invalid_input", native, "Environment name contains \"=\""));
    }
    Ok(if cfg!(windows) { name.to_uppercase() } else { name.to_string() })
}

fn text_value(native: Native, value: &str) -> Result<String, Error> {
    if value.contains('\0') {
        return Err(failure(ENVIRONMENT, "invalid_input", native, "Environment value contains a zero byte"));
    }
    Ok(value.to_string())
}

fn map_hash(native: Native, value: &Value) -> Result<BTreeMap<String, String>, Error> {
    let mut out = BTreeMap::new();
    for (name, value) in entries(value)? {
        let name = text_name(native, &name)?;
        let content = text_value(native, value.as_string()?)?;
        if out.contains_key(&name) {
            return Err(failure(
                ENVIRONMENT,
                "invalid_input",
                native,
                "Environment contains names that collide on this platform",
            ));
        }
        out.insert(name, content);
    }
    Ok(out)
}

fn environment_name(native: Native, value: &Value) -> Result<String, Error> {
    let raw = bytes(value)?;
    if raw.is_empty() {
        return Err(failure(ENVIRONMENT, "invalid_input", native, "Environment name must not be empty"));
    }
    let name = String::from_utf8_lossy(&raw).to_string();
    if name.contains('\0') {
        return Err(failure(ENVIRONMENT, "invalid_input", native, "Environment name contains a zero byte"));
    }
    if name.contains('=') {
        return Err(failure(ENVIRONMENT, "invalid_input", native, "Environment name contains \"=\""));
    }
    Ok(name)
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let name = operation(native);
    match name {
        // ---- fs_root ----
        "file_root_open" => {
            let path = root_path(native, &args[0], ROOT, "Invalid root path")?;
            open_check(&path, true).map_err(|error| errno(ROOT, native, "Cannot open no-follow root", error))?;
            Ok(new_handle(Rooted {
                path,
                closed: Cell::new(false),
            }))
        }
        "file_root_entry" => {
            let root = rooted(native, &args[0], ROOT)?;
            let parts = items(&args[1])?;
            if parts.len() > 256 {
                return Err(failure(ROOT, "invalid_input", native, "Root coordinates exceed component budget"));
            }
            let mut width = 0;
            let mut names = Vec::new();
            for part in parts {
                let part = part.as_string()?;
                if !component(part) {
                    return Err(failure(ROOT, "invalid_input", native, "Invalid root coordinate"));
                }
                width += part.len() + 1;
                if part.len() > 255 || width > 4096 {
                    return Err(failure(ROOT, "invalid_input", native, "Root coordinate byte budget"));
                }
                names.push(part.to_string());
            }
            let mut current = root.path.clone();
            for (index, part) in names.iter().enumerate() {
                current = current.join(part);
                open_check(&current, index + 1 < names.len())
                    .map_err(|error| errno(ROOT, native, "Cannot open no-follow rooted entry", error))?;
            }
            let meta = std::fs::symlink_metadata(&current).ok();
            if !meta.is_some_and(|meta| meta.is_file() || meta.is_dir()) {
                return Err(failure(
                    ROOT,
                    "invalid_data",
                    native,
                    "Rooted entries must be regular files or directories",
                ));
            }
            Ok(new_handle(Rooted {
                path: current,
                closed: Cell::new(false),
            }))
        }
        "file_root_closed" | "rooted_entry_closed" => Ok(Value::Bool(handle::<Rooted>(&args[0])?.closed.get())),
        "file_root_close" | "rooted_entry_close" | "snapshot_root_close" => {
            handle::<Rooted>(&args[0])?.closed.set(true);
            Ok(Value::Bool(true))
        }
        "rooted_entry_stat" => stat(native, rooted(native, &args[0], ROOT)?),
        "rooted_entry_read" => read_at(native, rooted(native, &args[0], ROOT)?, int(&args[1])?, int(&args[2])?),
        "rooted_entry_names" => names(native, rooted(native, &args[0], ROOT)?, int(&args[1])?),
        // ---- snapshot ----
        "snapshot_root_open" => {
            let path = root_path(native, &args[0], SNAPSHOT, "Invalid snapshot root")?;
            open_check(&path, true).map_err(|error| errno(SNAPSHOT, native, "Cannot open snapshot root", error))?;
            Ok(new_handle(Rooted {
                path,
                closed: Cell::new(false),
            }))
        }
        "snapshot_read" => {
            let root = rooted(native, &args[0], SNAPSHOT)?;
            let name = snapshot_name(native, string(&args[1])?)?;
            let bound = int(&args[2])?
                .to_i64()
                .filter(|bound| (1..=1_048_576).contains(bound))
                .ok_or_else(|| failure(SNAPSHOT, "invalid_input", native, "Snapshot limit must be 1..1048576"))?;
            let path = existing(native, root, name)?
                .ok_or_else(|| failure(SNAPSHOT, "not_found", native, "Snapshot is missing"))?;
            let content = std::fs::read(&path)
                .map_err(|error| errno(SNAPSHOT, native, "Cannot read snapshot", io_error(&error)))?;
            if content.len() as i64 > bound {
                return Err(failure(SNAPSHOT, "resource_exhausted", native, "Snapshot exceeds read limit"));
            }
            Ok(byte_array(content))
        }
        "snapshot_write" => {
            let root = rooted(native, &args[0], SNAPSHOT)?;
            let name = snapshot_name(native, string(&args[1])?)?;
            snapshot_write(native, root, name, &bytes(&args[2])?)
        }
        // ---- environment ----
        "environment_snapshot" => {
            let mut values = BTreeMap::new();
            for (name, value) in host::environment() {
                let name = text_name(native, &name)?;
                if values.contains_key(&name) {
                    return Err(failure(
                        ENVIRONMENT,
                        "invalid_data",
                        native,
                        "Process environment contains colliding variable names",
                    ));
                }
                values.insert(name, text_value(native, &value)?);
            }
            Ok(string_map(values))
        }
        "environment_normalize" => Ok(string_map(map_hash(native, &args[0])?)),
        "environment_add" => {
            let mut values = map_hash(native, &args[0])?;
            values.insert(text_name(native, string(&args[1])?)?, text_value(native, string(&args[2])?)?);
            Ok(string_map(values))
        }
        "environment_diff" => {
            let mut values = map_hash(native, &args[0])?;
            values.remove(&text_name(native, string(&args[1])?)?);
            Ok(string_map(values))
        }
        "environment_name" => Ok(text(text_name(native, string(&args[0])?)?)),
        "environment_home" => {
            let values = map_hash(native, &args[0])?;
            let home = if cfg!(windows) {
                values
                    .get("USERPROFILE")
                    .filter(|home| !home.is_empty())
                    .cloned()
                    .or_else(|| Some(format!("{}{}", values.get("HOMEDRIVE")?, values.get("HOMEPATH")?)))
            } else {
                values.get("HOME").cloned()
            };
            match home.filter(|home| !home.is_empty()) {
                Some(home) => Ok(text(home)),
                None => Err(failure(ENVIRONMENT, "not_found", native, "Home directory is unavailable")),
            }
        }
        "environment_path_separator" => Ok(text(if cfg!(windows) { ";" } else { ":" })),
        "environment_get" => {
            let name = environment_name(native, &args[0])?;
            match host::environment().get(&name) {
                Some(value) => Ok(byte_array(value.bytes())),
                None => Err(failure(ENVIRONMENT, "not_found", native, "Environment variable was not found")),
            }
        }
        "environment_set" => {
            let name = environment_name(native, &args[0])?;
            let value = String::from_utf8_lossy(&bytes(&args[1])?).to_string();
            if value.contains('\0') {
                return Err(failure(
                    ENVIRONMENT,
                    "invalid_input",
                    native,
                    "Environment value contains a zero byte",
                ));
            }
            host::set_environment(&name, Some(&value));
            Ok(Value::Bool(true))
        }
        "environment_remove" => {
            let name = environment_name(native, &args[0])?;
            let present = host::environment().contains_key(&name);
            host::set_environment(&name, None);
            Ok(Value::Bool(present))
        }
        "environment_all" => {
            let mut all: Vec<(String, String)> = host::environment().into_iter().collect();
            all.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
            Ok(array(
                all.into_iter()
                    .map(|(name, value)| array(vec![byte_array(name.into_bytes()), byte_array(value.into_bytes())]))
                    .collect(),
            ))
        }
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}
