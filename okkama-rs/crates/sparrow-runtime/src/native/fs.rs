//! `runtime/fs`: лексические пути (File::Spec Unix/Win32), файлы, каталоги,
//! блокировки, временные файлы, glob и наблюдение — перенос `FsNatives` JVM.
//! Пути — байты UTF-8; относительные разрешаются от виртуального cwd
//! (`crate::host`). Тексты и коды отказов — как у Perl-provider.
#![allow(unsafe_code)]

use super::jre;
use super::support::{
    byte_array, bytes, failure, handle, int, int_map, int_value, items, new_handle, string,
    string_array, string_map, text,
};
use super::Error;
use crate::host;
use crate::sched;
use crate::{IntValue, Value, abi::Native};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::rc::Rc;

const DOMAIN: &str = "core.fs";
const READ: i64 = 1;
const WRITE: i64 = 2;
const CREATE: i64 = 4;
const TRUNCATE: i64 = 8;
const APPEND: i64 = 16;
const EXCLUSIVE: i64 = 32;
const KNOWN_FLAGS: i64 = READ | WRITE | CREATE | TRUNCATE | APPEND | EXCLUSIVE;

/// `$` Java без MULTILINE: конец текста или перед завершающим переводом строки.
const JAVA_END: &str = r"(?:(?=\r\n\z)|(?=[\n\r\u{85}\u{2028}\u{2029}]\z)|\z)";

fn fail(native: Native, code: &str, message: impl Into<String>) -> Error {
    failure(DOMAIN, code, native, message)
}

fn host_error(native: Native, error: &std::io::Error, message: &str) -> Error {
    fail(native, host::code(error), message)
}

fn closed_handle(native: Native) -> Error {
    fail(native, "already_closed", "Native handle is already closed")
}

/// Байтовый путь программы (`_path`).
fn path(native: Native, value: &Value) -> Result<String, Error> {
    let raw = bytes(value)?;
    if raw.is_empty() || raw.contains(&0) {
        return Err(fail(native, "invalid_input", "Invalid filesystem path"));
    }
    Ok(String::from_utf8_lossy(&raw).to_string())
}

fn valid_mode(mode: &IntValue) -> Option<u32> {
    mode.to_i64().filter(|mode| (0..=0o777).contains(mode)).map(|mode| mode as u32)
}

// ---- лексические пути: File::Spec ----

/// Perl `split` без предела (`keep_trailing` — с пределом -1).
fn perl_split(text: &str, pattern: &str, keep_trailing: bool) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    jre::split(text, pattern, if keep_trailing { -1 } else { 0 })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Spec {
    Unix,
    Win32,
}

const DRIVE: &str = "[a-zA-Z]:";
const UNC: &str = r"(?:\\\\|//)[^\\/]+[\\/][^\\/]+";

fn vol() -> String {
    format!("(?:{DRIVE}|{UNC})")
}

fn unix_canonpath(path: &str) -> String {
    let mut path = jre::replace_all(path, "(?s)/{2,}", "/");
    path = jre::replace_all(&path, r"(?s)(?:/\.)+(?:/|\z)", "/");
    if path != "./" {
        path = jre::replace_first(&path, r"(?s)^(?:\./)+", "");
    }
    path = jre::replace_first(&path, r"(?s)^/(?:\.\./)+", "/");
    path = jre::replace_first(&path, &format!(r"(?s)^/\.\.{JAVA_END}"), "/");
    if path != "/" {
        path = jre::replace_first(&path, r"(?s)/\z", "");
    }
    path
}

fn win32_canon_cat(parts: &[String]) -> String {
    let mut first = parts[0].clone();
    let volume;
    if let Some(drive) = jre::captures(r"\A([A-Za-z]:)([\\/]?)", &first) {
        let letter = drive[1].clone().unwrap_or_default();
        let separator = drive[2].clone().unwrap_or_default();
        volume = format!(
            "{}{}{}",
            letter[..1].to_uppercase(),
            &letter[1..],
            if separator.is_empty() { "" } else { "\\" }
        );
        let length = drive[0].as_ref().map_or(0, String::len);
        first = first[length..].to_string();
    } else if let Some(unc) = jre::captures(r"(?s)\A(?:\\\\|//)([^\\/]+)(?:[\\/]([^\\/]+))?[\\/]?", &first) {
        volume = format!(
            "\\\\{}{}\\",
            unc[1].clone().unwrap_or_default(),
            unc[2].as_ref().map_or(String::new(), |share| format!("\\{share}"))
        );
        let length = unc[0].as_ref().map_or(0, String::len);
        first = first[length..].to_string();
    } else if first.starts_with('\\') || first.starts_with('/') {
        volume = "\\".to_string();
        first = first[1..].to_string();
    } else {
        volume = String::new();
    }
    let mut joined = first;
    for part in &parts[1..] {
        joined.push('\\');
        joined.push_str(part);
    }
    let mut path = jre::replace_all(&joined, r"[\\/]+", "\\");
    path = jre::replace_all(&path, r"(?s)(?:(?:\A|\\)\.(?:\\\.)*(?:\\|\z))+", "\\");
    let up = r"(?s)(?:(?:\A|\\)[^\\]+\\\.\.(?<!\A\.\.\\\.\.)(?<!\\\.\.\\\.\.)(?:\\|\z))+";
    while let Some((start, end)) = jre::find(up, &path) {
        path = format!("{}\\{}", &path[..start], &path[end..]);
    }
    path = jre::replace_first(&path, r"(?s)\A\\", "");
    path = jre::replace_first(&path, r"(?s)\\\z", "");
    if volume.ends_with('\\') {
        path = jre::replace_first(&path, r"(?s)\A\.\.(?:\\\.\.)*(?:\\|\z)", "");
        if path.is_empty() {
            if let Some(share) = jre::captures(r"(?s)\A(\\\\.*)\\\z", &volume) {
                return share[1].clone().unwrap_or_default();
            }
        }
    }
    if !path.is_empty() || !volume.is_empty() {
        format!("{volume}{path}")
    } else {
        ".".to_string()
    }
}

impl Spec {
    fn canonpath(self, path: &str) -> String {
        match self {
            Spec::Unix => unix_canonpath(path),
            Spec::Win32 => {
                if path.is_empty() {
                    String::new()
                } else {
                    win32_canon_cat(&[path.to_string()])
                }
            }
        }
    }

    fn win32_cat(parts: &[String]) -> String {
        let mut list = parts.to_vec();
        if list[0].is_empty() {
            list[0] = "/".into();
        } else if jre::matches(DRIVE, &list[0]) {
            list[0].push('\\');
        }
        win32_canon_cat(&list)
    }

    fn catdir(self, parts: &[String]) -> String {
        match self {
            Spec::Unix => {
                let joined: String = parts.iter().map(|part| format!("{part}/")).collect();
                unix_canonpath(&joined)
            }
            Spec::Win32 => {
                if parts.is_empty() {
                    String::new()
                } else {
                    Self::win32_cat(parts)
                }
            }
        }
    }

    fn catfile(self, parts: &[String]) -> String {
        match self {
            Spec::Unix => {
                let file = unix_canonpath(parts.last().expect("parts"));
                if parts.len() == 1 {
                    return file;
                }
                let mut dir = self.catdir(&parts[..parts.len() - 1]);
                if !dir.ends_with('/') {
                    dir.push('/');
                }
                dir + &file
            }
            Spec::Win32 => {
                if parts.is_empty() {
                    win32_canon_cat(&["/".to_string()])
                } else {
                    Self::win32_cat(parts)
                }
            }
        }
    }

    fn absolute(self, path: &str) -> bool {
        match self {
            Spec::Unix => path.starts_with('/'),
            Spec::Win32 => {
                if let Some(volume) = jre::captures(&format!("^({})", vol()), path) {
                    let volume = volume[1].clone().unwrap_or_default();
                    return jre::matches(&format!("(?s){UNC}.*"), &volume)
                        || jre::is_match(&format!(r"^{DRIVE}[\\/]"), path);
                }
                path.starts_with('\\') || path.starts_with('/')
            }
        }
    }

    fn splitpath(self, path: &str) -> [String; 3] {
        match self {
            Spec::Unix => {
                let found = jre::captures(r"(?s)^((?:.*/(?:\.\.?\z)?)?)([^/]*)", path).expect("always matches");
                [
                    String::new(),
                    found[1].clone().unwrap_or_default(),
                    found[2].clone().unwrap_or_default(),
                ]
            }
            Spec::Win32 => {
                let found = jre::captures(&format!(r"(?s)^({}?)((?:.*[\\/](?:\.\.?\z)?)?)(.*)", vol()), path)
                    .expect("always matches");
                [
                    found[1].clone().unwrap_or_default(),
                    found[2].clone().unwrap_or_default(),
                    found[3].clone().unwrap_or_default(),
                ]
            }
        }
    }

    fn splitdir(self, directories: &str) -> Vec<String> {
        match self {
            Spec::Unix => perl_split(directories, "/", true),
            Spec::Win32 => {
                if !jre::is_match(r"[\\/]\z", directories) {
                    return perl_split(directories, r"[\\/]", false);
                }
                let mut out = perl_split(&format!("{directories}dummy"), r"[\\/]", false);
                if let Some(last) = out.last_mut() {
                    last.clear();
                }
                out
            }
        }
    }

    fn catpath(self, volume: &str, directory: &str, file: &str) -> String {
        match self {
            Spec::Unix => {
                if !directory.is_empty() && !file.is_empty() && !directory.ends_with('/') && !file.starts_with('/') {
                    format!("{directory}/{file}")
                } else {
                    format!("{directory}{file}")
                }
            }
            Spec::Win32 => {
                let mut volume = volume.to_string();
                if let Some(share) = jre::captures(r"(?s)^([\\/])[\\/][^\\/]+[\\/][^\\/]+\z", &volume) {
                    if jre::is_match(r"(?s)^[^\\/]", directory) {
                        volume.push_str(&share[1].clone().unwrap_or_default());
                    }
                }
                volume.push_str(directory);
                if !jre::matches("^[a-zA-Z]:", &volume)
                    && jre::is_match(r"[^\\/]\z", &volume)
                    && jre::is_match(r"[^\\/]", file)
                {
                    let separator = jre::captures(r"([\\/])", &volume)
                        .and_then(|found| found[1].clone())
                        .unwrap_or_else(|| "\\".into());
                    volume.push_str(&separator);
                }
                volume + file
            }
        }
    }

    fn rootdir(self) -> &'static str {
        match self {
            Spec::Unix => "/",
            Spec::Win32 => "\\",
        }
    }
}

fn spec(native: Native, style: &IntValue) -> Result<Spec, Error> {
    match style.to_i64() {
        Some(0) => Ok(Spec::Unix),
        Some(1) => Ok(Spec::Win32),
        _ => Err(fail(native, "invalid_input", "Invalid path style")),
    }
}

fn joined(spec: Spec, parts: &[String]) -> String {
    let mut joined = parts[0].clone();
    for part in &parts[1..] {
        joined = if spec.absolute(part) {
            part.clone()
        } else {
            spec.catfile(&[joined, part.clone()])
        };
    }
    joined
}

fn path_parent(spec: Spec, path: &str) -> String {
    let split = spec.splitpath(path);
    if !split[2].is_empty() {
        let parent = spec.catpath(&split[0], &split[1], "");
        return spec.canonpath(if parent.is_empty() { "." } else { &parent });
    }
    let mut segments = spec.splitdir(&split[1]);
    while segments.len() > 1 && segments.last().is_some_and(String::is_empty) {
        segments.pop();
    }
    segments.pop();
    let directory = if segments.is_empty() {
        String::new()
    } else {
        spec.catdir(&segments)
    };
    let mut parent = spec.catpath(&split[0], &directory, "");
    if parent.is_empty() && spec.absolute(path) {
        parent = spec.rootdir().to_string();
    }
    if parent.is_empty() {
        parent = ".".into();
    }
    spec.canonpath(&parent)
}

/// Путь потомка как `File::Spec->catfile`.
fn child(path: &str, name: &str) -> String {
    Spec::Unix.catfile(&[path.to_string(), name.to_string()])
}

// ---- блокировки: таблица процесса, как FileLockTable JVM ----

type FileKey = (u64, u64);

thread_local! {
    static LOCKS: RefCell<HashMap<FileKey, usize>> = RefCell::new(HashMap::new());
}

/// Блокировка файла (`fcntl`); `Err(None)` — пересечение с блокировкой этого
/// процесса (`OverlappingFileLockException`), `Ok(false)` — занято (tryLock).
fn lock_fd(file: &File, shared: bool, wait: bool) -> Result<bool, Option<std::io::Error>> {
    use std::os::fd::AsRawFd;
    let meta = file.metadata().map_err(Some)?;
    let key = (meta.dev(), meta.ino());
    if LOCKS.with(|locks| locks.borrow().contains_key(&key)) {
        return Err(None);
    }
    // SAFETY: структура flock заполнена нулями и полями блокировки целого файла.
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = if shared { libc::F_RDLCK } else { libc::F_WRLCK } as _;
    lock.l_whence = libc::SEEK_SET as _;
    lock.l_start = 0;
    lock.l_len = 0;
    let command = if wait { libc::F_SETLKW } else { libc::F_SETLK };
    // SAFETY: дескриптор открыт, указатель на flock действителен.
    let result = unsafe { libc::fcntl(file.as_raw_fd(), command, &lock) };
    if result == -1 {
        let error = std::io::Error::last_os_error();
        if !wait && matches!(error.raw_os_error(), Some(libc::EAGAIN) | Some(libc::EACCES)) {
            return Ok(false);
        }
        return Err(Some(error));
    }
    LOCKS.with(|locks| *locks.borrow_mut().entry(key).or_insert(0) += 1);
    Ok(true)
}

fn unlock_fd(file: &File) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    let meta = file.metadata()?;
    let key = (meta.dev(), meta.ino());
    // SAFETY: как в lock_fd.
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_UNLCK as _;
    lock.l_whence = libc::SEEK_SET as _;
    // SAFETY: дескриптор открыт, указатель на flock действителен.
    let result = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &lock) };
    LOCKS.with(|locks| locks.borrow_mut().remove(&key));
    if result == -1 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Открытый файл с признаком захваченной блокировки.
#[derive(Debug)]
struct Channel {
    file: File,
    locked: bool,
    readable: bool,
    writable: bool,
}

impl Channel {
    fn release(&mut self) -> std::io::Result<()> {
        if self.locked {
            self.locked = false;
            return unlock_fd(&self.file);
        }
        Ok(())
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

/// Флаги open: None — недопустимое сочетание.
fn options(flags: i64) -> Option<std::fs::OpenOptions> {
    if flags < 0 || flags & !KNOWN_FLAGS != 0 || flags & (READ | WRITE) == 0 {
        return None;
    }
    if flags & (TRUNCATE | APPEND) != 0 && flags & WRITE == 0 {
        return None;
    }
    if flags & EXCLUSIVE != 0 && flags & CREATE == 0 {
        return None;
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(flags & READ != 0).write(flags & WRITE != 0);
    if flags & EXCLUSIVE != 0 {
        options.create_new(true);
    } else if flags & CREATE != 0 {
        options.create(true);
    }
    if flags & TRUNCATE != 0 {
        options.truncate(true);
    }
    // O_APPEND эмулируется: перед каждой записью позиция — конец файла.
    Some(options)
}

fn open_channel(path: &str, flags: i64, mode: u32) -> std::io::Result<Channel> {
    let mut options = options(flags).expect("validated flags");
    if flags & (CREATE | EXCLUSIVE) != 0 {
        options.mode(mode);
    }
    Ok(Channel {
        file: options.open(host::path(path))?,
        locked: false,
        readable: flags & READ != 0,
        writable: flags & WRITE != 0,
    })
}

fn read_into(file: &mut File, size: usize) -> std::io::Result<Vec<u8>> {
    let mut buffer = vec![0; size];
    let count = file.read(&mut buffer)?;
    buffer.truncate(count);
    Ok(buffer)
}

fn write_appending(file: &mut File, append: bool, data: &[u8]) -> std::io::Result<usize> {
    if append {
        file.seek(SeekFrom::End(0))?;
    }
    file.write(data)
}

fn seek(native: Native, file: &mut File, offset: &IntValue, origin: &IntValue) -> Result<i64, Error> {
    let origin = origin
        .to_i64()
        .filter(|origin| (0..=2).contains(origin))
        .ok_or_else(|| fail(native, "invalid_input", "Invalid seek origin"))?;
    let base = match origin {
        0 => 0,
        1 => file.stream_position().map_err(|error| host_error(native, &error, "Cannot seek file"))? as i64,
        _ => file.metadata().map_err(|error| host_error(native, &error, "Cannot seek file"))?.len() as i64,
    };
    let target = crate::integer::add(offset, &IntValue::from_i64(base));
    let Some(target) = target.to_i64().filter(|target| *target >= 0) else {
        return Err(fail(native, "invalid_input", "Cannot seek file"));
    };
    file.seek(SeekFrom::Start(target as u64))
        .map_err(|error| host_error(native, &error, "Cannot seek file"))?;
    Ok(target)
}

/// `truncate(2)`: и укорачивает, и удлиняет нулями.
fn truncate(file: &mut File, size: u64) -> std::io::Result<()> {
    let current = file.metadata()?.len();
    if size < current {
        let position = file.stream_position()?;
        file.set_len(size)?;
        file.seek(SeekFrom::Start(position))?;
    } else if size > current {
        use std::os::unix::fs::FileExt;
        file.write_at(&[0], size - 1)?;
    }
    Ok(())
}

#[derive(Debug)]
struct OpenFile {
    channel: Option<Channel>,
    append: bool,
}

fn open_file<'a>(native: Native, value: &'a Value) -> Result<std::cell::RefMut<'a, OpenFile>, Error> {
    let file = handle::<RefCell<OpenFile>>(value)?.borrow_mut();
    if file.channel.is_none() {
        return Err(closed_handle(native));
    }
    Ok(file)
}

// ---- метаданные ----

fn kind(mode: u32) -> i64 {
    match mode & 0o170000 {
        0o100000 => 1,
        0o040000 => 2,
        0o120000 => 3,
        0o060000 => 4,
        0o020000 => 5,
        0o010000 => 6,
        0o140000 => 7,
        _ => 0,
    }
}

fn stat_value(meta: &std::fs::Metadata) -> Value {
    int_map([
        ("kind", kind(meta.mode())),
        ("size", meta.size() as i64),
        ("permissions", i64::from(meta.mode() & 0o777)),
        ("accessed_sec", meta.atime()),
        ("modified_sec", meta.mtime()),
        ("changed_sec", meta.ctime()),
        ("created_sec", -1),
        ("device", meta.dev() as i64),
        ("inode", meta.ino() as i64),
    ])
}

fn directory(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
}

fn exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// Имена каталога без `.`/`..`, по возрастанию code points.
fn names(path: &Path) -> std::io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(path)? {
        names.push(entry?.file_name().to_string_lossy().to_string());
    }
    names.sort();
    Ok(names)
}

fn remove_tree(native: Native, path: &Path) -> Result<(), Error> {
    let message = "Cannot remove directory tree";
    if !directory(path) {
        return std::fs::remove_file(path).map_err(|error| host_error(native, &error, message));
    }
    for name in names(path).map_err(|error| host_error(native, &error, message))? {
        remove_tree(native, &path.join(name))?;
    }
    std::fs::remove_dir(path).map_err(|error| host_error(native, &error, message))
}

#[derive(Debug)]
struct Dir {
    path: String,
    entries: Vec<String>,
    index: usize,
    closed: bool,
}

// ---- glob ----

enum Token {
    Any,
    One,
    Literal(char),
    Class(bool, Vec<(char, char)>),
}

fn glob_segment(native: Native, segment: &str) -> Result<Option<Vec<Token>>, Error> {
    if segment != "**" && segment.contains("**") {
        return Err(fail(native, "invalid_input", "Recursive ** must be a complete path segment"));
    }
    if segment == "**" {
        return Ok(None);
    }
    let characters: Vec<char> = segment.chars().collect();
    let mut tokens = Vec::new();
    let mut position = 0;
    while position < characters.len() {
        match characters[position] {
            '*' => tokens.push(Token::Any),
            '?' => tokens.push(Token::One),
            '[' => {
                let mut ending = position + 1;
                while ending < characters.len() && characters[ending] != ']' {
                    ending += 1;
                }
                if ending >= characters.len() {
                    return Err(fail(native, "invalid_input", "Unclosed glob character class"));
                }
                let mut from = position + 1;
                if from == ending {
                    return Err(fail(native, "invalid_input", "Empty glob character class"));
                }
                let negated = characters[from] == '!';
                if negated {
                    from += 1;
                }
                if from == ending {
                    return Err(fail(native, "invalid_input", "Empty negated glob character class"));
                }
                let mut ranges = Vec::new();
                let mut index = from;
                while index < ending {
                    let first = characters[index];
                    index += 1;
                    if index + 1 < ending && characters[index] == '-' {
                        let last = characters[index + 1];
                        if first > last {
                            return Err(fail(native, "invalid_input", "Descending glob character range"));
                        }
                        ranges.push((first, last));
                        index += 2;
                    } else {
                        ranges.push((first, first));
                    }
                }
                tokens.push(Token::Class(negated, ranges));
                position = ending;
            }
            c => tokens.push(Token::Literal(c)),
        }
        position += 1;
    }
    Ok(Some(tokens))
}

fn glob_matches(tokens: &[Token], name: &[char], token: usize, at: usize, memo: &mut HashMap<(usize, usize), bool>) -> bool {
    if let Some(known) = memo.get(&(token, at)) {
        return *known;
    }
    let result = if token == tokens.len() {
        at == name.len()
    } else {
        match &tokens[token] {
            Token::Any => {
                glob_matches(tokens, name, token + 1, at, memo)
                    || (at < name.len() && glob_matches(tokens, name, token, at + 1, memo))
            }
            _ if at >= name.len() => false,
            current => {
                let character = name[at];
                let accepted = match current {
                    Token::One => true,
                    Token::Literal(value) => character == *value,
                    Token::Class(negated, ranges) => {
                        let included = ranges.iter().any(|(from, to)| (*from..=*to).contains(&character));
                        *negated != included
                    }
                    Token::Any => unreachable!("handled above"),
                };
                accepted && glob_matches(tokens, name, token + 1, at + 1, memo)
            }
        }
    };
    memo.insert((token, at), result);
    result
}

#[allow(clippy::too_many_arguments)]
fn visit(
    path: &str,
    is_directory: bool,
    index: usize,
    is_root: bool,
    segments: &[Option<Vec<Token>>],
    matches: &mut BTreeSet<String>,
    visited: &mut HashSet<(usize, String)>,
) -> std::io::Result<()> {
    if !visited.insert((index, path.to_string())) {
        return Ok(());
    }
    if index == segments.len() {
        if !is_root {
            matches.insert(path.to_string());
        }
        return Ok(());
    }
    let segment = &segments[index];
    let recursive = segment.is_none();
    if recursive {
        visit(path, is_directory, index + 1, is_root, segments, matches, visited)?;
    }
    if !is_directory {
        return Ok(());
    }
    for name in names(&host::path(path))? {
        if let Some(tokens) = segment {
            let characters: Vec<char> = name.chars().collect();
            if !glob_matches(tokens, &characters, 0, 0, &mut HashMap::new()) {
                continue;
            }
        }
        let child_path = child(path, &name);
        let child_directory = directory(&host::path(&child_path));
        visit(
            &child_path,
            child_directory,
            if recursive { index } else { index + 1 },
            false,
            segments,
            matches,
            visited,
        )?;
    }
    Ok(())
}

// ---- FileState ----

#[derive(Debug)]
struct LockState {
    file: Option<std::rc::Weak<RefCell<FileState>>>,
    closed: bool,
}

#[derive(Debug, Default)]
struct FileState {
    path: String,
    mode: i64,
    flags: i64,
    binary: bool,
    encoding: String,
    channel: Option<Channel>,
    buffer: Vec<u8>,
    eof: bool,
    closed: bool,
    last: Option<&'static str>,
    lock: Option<Rc<RefCell<LockState>>>,
    temporary: bool,
    delete_on_close: Option<String>,
}

type StateRef = Rc<RefCell<FileState>>;

fn state_flags(mode: &IntValue) -> Option<i64> {
    match mode.to_i64()? {
        0 => Some(READ),
        1 => Some(WRITE | CREATE | TRUNCATE),
        2 => Some(WRITE | CREATE | APPEND),
        3 => Some(READ | WRITE),
        4 => Some(READ | WRITE | CREATE | APPEND),
        _ => None,
    }
}

fn known_encoding(encoding: &str) -> bool {
    matches!(
        encoding,
        "utf-8" | "utf-16-le" | "utf-16-be" | "utf-32-le" | "utf-32-be" | "ascii" | "latin-1"
    )
}

fn file_state(native: Native, value: &Value) -> Result<StateRef, Error> {
    let state = handle::<StateRef>(value)?.clone();
    if state.borrow().closed {
        return Err(fail(native, "already_closed", "File is already closed"));
    }
    Ok(state)
}

fn state_open(native: Native, state: &mut FileState) -> Result<(), Error> {
    if state.channel.is_none() {
        let channel = open_channel(&state.path, state.flags, 0o666)
            .map_err(|error| host_error(native, &error, "Cannot open file"))?;
        state.channel = Some(channel);
    }
    Ok(())
}

fn readable(state: &FileState) -> bool {
    matches!(state.mode, 0 | 3 | 4)
}

fn writable(state: &FileState) -> bool {
    state.mode >= 1
}

fn appending(state: &FileState) -> bool {
    state.flags & APPEND != 0
}

fn prepare_read(native: Native, state: &mut FileState, textual: bool) -> Result<(), Error> {
    if !readable(state) {
        return Err(fail(native, "invalid_input", "File mode does not permit reading"));
    }
    if textual && state.binary {
        return Err(fail(native, "invalid_input", "Text operation required"));
    }
    if !textual && !state.binary {
        return Err(fail(native, "invalid_input", "Binary operation required"));
    }
    state_open(native, state)?;
    state.last = Some("read");
    Ok(())
}

fn prepare_write(native: Native, state: &mut FileState, textual: bool) -> Result<(), Error> {
    if !writable(state) {
        return Err(fail(native, "invalid_input", "File mode does not permit writing"));
    }
    if textual && state.binary {
        return Err(fail(native, "invalid_input", "Text operation required"));
    }
    if !textual && !state.binary {
        return Err(fail(native, "invalid_input", "Binary operation required"));
    }
    state_open(native, state)?;
    if state.last == Some("read") && !state.buffer.is_empty() {
        let back = state.buffer.len() as i64;
        let file = &mut state.channel.as_mut().expect("opened").file;
        file.seek(SeekFrom::Current(-back))
            .map_err(|error| host_error(native, &error, "Cannot restore logical file position"))?;
        state.buffer.clear();
        state.eof = false;
    }
    state.last = Some("write");
    Ok(())
}

fn decode_text(native: Native, state: &FileState, data: &[u8]) -> Result<String, Error> {
    jre::decode(data, &state.encoding)
        .ok_or_else(|| fail(native, "invalid_data", "File contains invalid encoded text"))
}

fn fill(native: Native, state: &mut FileState) -> Result<bool, Error> {
    let file = &mut state.channel.as_mut().expect("opened").file;
    let chunk = read_into(file, 8192).map_err(|error| host_error(native, &error, "Cannot read file"))?;
    if chunk.is_empty() {
        state.eof = true;
        return Ok(false);
    }
    state.buffer.extend(chunk);
    Ok(true)
}

fn newline(encoding: &str) -> &'static [u8] {
    match encoding {
        "utf-16-le" => &[0x0a, 0],
        "utf-16-be" => &[0, 0x0a],
        "utf-32-le" => &[0x0a, 0, 0, 0],
        "utf-32-be" => &[0, 0, 0, 0x0a],
        _ => &[0x0a],
    }
}

fn line_offset(buffer: &[u8], needle: &[u8]) -> Option<usize> {
    let unit = needle.len();
    (0..=buffer.len().saturating_sub(unit))
        .filter(|offset| offset + unit <= buffer.len())
        .find(|offset| buffer[*offset..*offset + unit] == *needle && offset % unit == 0)
}

fn release_lock(state: &mut FileState) {
    if let Some(lock) = state.lock.take() {
        let mut lock = lock.borrow_mut();
        if !lock.closed {
            if let Some(channel) = state.channel.as_mut() {
                let _ = channel.release();
            }
            lock.closed = true;
            lock.file = None;
        }
    }
}

fn cleanup_temporary(native: Native, state: &mut FileState) -> Result<(), Error> {
    if let Some(path) = state.delete_on_close.clone() {
        match std::fs::remove_file(host::path(&path)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(host_error(native, &error, "Cannot remove temporary file")),
        }
        state.delete_on_close = None;
    }
    Ok(())
}

// ---- временные файлы ----

const TEMPLATE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_";

/// Имя как у File::Temp: префикс и восемь случайных символов.
fn temporary_name(prefix: &str) -> String {
    let mut name = prefix.to_string();
    while name.len() < prefix.len() + 8 {
        let random = okkama_native::call("random", &[b"16"]).unwrap_or_else(|_| vec![0; 16]);
        for byte in random {
            // Равномерно по 63 символам: отбрасываем 252..255.
            if byte < 252 && name.len() < prefix.len() + 8 {
                name.push(char::from(TEMPLATE[usize::from(byte) % 63]));
            }
        }
    }
    name
}

fn prefix<'a>(native: Native, prefix: &'a str) -> Result<&'a str, Error> {
    if prefix.contains(['\0', '/', '\\']) {
        return Err(fail(native, "invalid_input", "Temporary prefix must be one path component"));
    }
    Ok(prefix)
}

#[derive(Debug)]
struct TemporaryDirectory {
    path: String,
    closed: std::cell::Cell<bool>,
}

// ---- наблюдение за путём (опрос раз в 50 мс) ----

#[derive(Clone, Debug)]
struct Metadata {
    device: u64,
    inode: u64,
    mode: u32,
    size: u64,
    modified: i64,
    changed: i64,
    directory: bool,
}

impl Metadata {
    fn identity(&self) -> (u64, u64) {
        (self.device, self.inode)
    }

    fn differs(&self, other: &Metadata) -> bool {
        self.mode != other.mode || self.size != other.size || self.modified != other.modified || self.changed != other.changed
    }
}

fn metadata(path: &str) -> std::io::Result<Option<Metadata>> {
    match std::fs::symlink_metadata(host::path(path)) {
        Ok(meta) => Ok(Some(Metadata {
            device: meta.dev(),
            inode: meta.ino(),
            mode: meta.mode(),
            size: meta.size(),
            modified: meta.mtime(),
            changed: meta.ctime(),
            directory: meta.mode() & 0o170000 == 0o040000,
        })),
        Err(error) if host::code(&error) == "not_found" => Ok(None),
        Err(error) => Err(error),
    }
}

fn children(path: &str) -> std::io::Result<Vec<(String, Metadata)>> {
    let mut out = Vec::new();
    for name in names(&host::path(path))? {
        if let Some(meta) = metadata(&child(path, &name))? {
            out.push((name, meta));
        }
    }
    Ok(out)
}

type Event = [String; 3];

fn child_events(root: &str, before: &[(String, Metadata)], after: &[(String, Metadata)], events: &mut Vec<Event>) {
    let before_map: BTreeMap<&str, &Metadata> = before.iter().map(|(name, meta)| (name.as_str(), meta)).collect();
    let after_map: BTreeMap<&str, &Metadata> = after.iter().map(|(name, meta)| (name.as_str(), meta)).collect();
    let mut removed = Vec::new();
    let mut created = Vec::new();
    for (name, previous) in &before_map {
        match after_map.get(name) {
            Some(next) if previous.identity() == next.identity() => {
                if previous.differs(next) {
                    events.push(["modified".into(), child(root, name), String::new()]);
                }
            }
            _ => removed.push(name.to_string()),
        }
    }
    for (name, next) in &after_map {
        if before_map.get(name).is_none_or(|previous| previous.identity() != next.identity()) {
            created.push(name.to_string());
        }
    }
    let mut by_identity: HashMap<(u64, u64), Vec<String>> = HashMap::new();
    for name in &created {
        by_identity.entry(after_map[name.as_str()].identity()).or_default().push(name.clone());
    }
    let mut renamed = HashSet::new();
    for name in &removed {
        let candidates = by_identity.get_mut(&before_map[name.as_str()].identity());
        match candidates.filter(|candidates| !candidates.is_empty()) {
            Some(candidates) => {
                let new_name = candidates.remove(0);
                events.push(["renamed".into(), child(root, &new_name), child(root, name)]);
                renamed.insert(new_name);
            }
            None => events.push(["removed".into(), child(root, name), String::new()]),
        }
    }
    for name in created {
        if !renamed.contains(&name) {
            events.push(["created".into(), child(root, &name), String::new()]);
        }
    }
}

#[derive(Debug)]
struct Watch {
    path: String,
    metadata: Option<Metadata>,
    children: Vec<(String, Metadata)>,
    events: Vec<Event>,
    waiter: Option<sched::Future>,
    closed: bool,
}

fn poll(watch: &mut Watch) -> std::io::Result<Option<Event>> {
    if !watch.events.is_empty() {
        return Ok(Some(watch.events.remove(0)));
    }
    let current = metadata(&watch.path)?;
    let previous = watch.metadata.clone();
    let path = watch.path.clone();
    match (&previous, &current) {
        (None, Some(_)) => watch.events.push(["created".into(), path.clone(), String::new()]),
        (Some(_), None) => watch.events.push(["removed".into(), path.clone(), String::new()]),
        (Some(previous), Some(current)) if previous.identity() != current.identity() => {
            watch.events.push(["removed".into(), path.clone(), String::new()]);
            watch.events.push(["created".into(), path.clone(), String::new()]);
        }
        (Some(previous), Some(current)) if !current.directory && previous.differs(current) => {
            watch.events.push(["modified".into(), path.clone(), String::new()]);
        }
        _ => {}
    }
    if current.as_ref().is_some_and(|current| current.directory) {
        let next = children(&path)?;
        if previous.as_ref().is_some_and(|previous| previous.directory) {
            let before = std::mem::take(&mut watch.children);
            child_events(&path, &before, &next, &mut watch.events);
        }
        watch.children = next;
    } else {
        watch.children.clear();
    }
    watch.metadata = current;
    Ok(if watch.events.is_empty() {
        None
    } else {
        Some(watch.events.remove(0))
    })
}

fn event_value(event: Event) -> Value {
    let [kind, path, previous] = event;
    string_map([
        ("kind".to_string(), kind),
        ("path".to_string(), path),
        ("previous".to_string(), previous),
    ])
}

fn native_failure(error: Error) -> Rc<crate::failure::Failure> {
    match error {
        Error::Failure(failure) => crate::failure::Failure::new(crate::failure::Kind::Native(Rc::new(failure))),
        Error::Trap(message) | Error::InvalidState(message) | Error::Unsupported(message) => {
            crate::failure::Failure::new(crate::failure::Kind::Program(message))
        }
    }
}

fn schedule_poll(native: Native, watch: Rc<RefCell<Watch>>, promise: sched::Future) {
    let target = promise.clone();
    let op = sched::offload(
        || std::thread::sleep(std::time::Duration::from_millis(50)),
        move |()| {
            if !target.running() {
                return;
            }
            if watch.borrow().closed {
                sched::settle(
                    &target,
                    Err(native_failure(fail(native, "already_closed", "File watch is already closed"))),
                );
                return;
            }
            let polled = poll(&mut watch.borrow_mut());
            match polled {
                Ok(Some(event)) => {
                    watch.borrow_mut().waiter = None;
                    sched::settle(&target, Ok(event_value(event)));
                }
                Ok(None) => schedule_poll(native, watch.clone(), target.clone()),
                Err(error) => {
                    watch.borrow_mut().waiter = None;
                    sched::settle(
                        &target,
                        Err(native_failure(host_error(native, &error, "Cannot inspect watched path"))),
                    );
                }
            }
        },
    );
    promise.listen(move || op.detach());
}

fn watch_event(native: Native, value: &Value) -> Result<Value, Error> {
    let watch = handle::<Rc<RefCell<Watch>>>(value)?.clone();
    let resolved = |outcome: sched::Outcome| Ok(Value::Future(sched::resolved(outcome)));
    if watch.borrow().closed {
        return resolved(Err(native_failure(fail(native, "already_closed", "File watch is already closed"))));
    }
    if watch.borrow().waiter.as_ref().is_some_and(|waiter| waiter.running()) {
        return resolved(Err(native_failure(fail(
            native,
            "resource_exhausted",
            "File watch already has a pending event",
        ))));
    }
    let polled = poll(&mut watch.borrow_mut());
    match polled {
        Ok(Some(event)) => return resolved(Ok(event_value(event))),
        Ok(None) => {}
        Err(error) => {
            return resolved(Err(native_failure(host_error(native, &error, "Cannot inspect watched path"))));
        }
    }
    let promise = sched::promise();
    watch.borrow_mut().waiter = Some(promise.clone());
    schedule_poll(native, watch, promise.clone());
    Ok(Value::Future(promise))
}

fn paths(_native: Native, value: &Value) -> Result<Vec<String>, Error> {
    items(value)?.iter().map(|item| Ok(item.as_string()?.to_string())).collect()
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let name = native.name();
    let ok = Ok(Value::Bool(true));
    match name {
        // ---- лексические пути ----
        "path_encode" => Ok(byte_array(string(&args[0])?.bytes())),
        "path_decode" => match String::from_utf8(bytes(&args[0])?) {
            Ok(path) => Ok(text(path)),
            Err(_) => Err(fail(native, "invalid_data", "Filesystem path is not valid UTF-8")),
        },
        "path_join" => {
            let spec = spec(native, int(&args[1])?)?;
            let parts = paths(native, &args[0])?;
            if parts.is_empty() {
                return Err(fail(native, "invalid_input", "Path join requires at least one part"));
            }
            if parts.iter().any(|part| part.is_empty() || part.contains('\0')) {
                return Err(fail(native, "invalid_input", "Path join rejects empty parts"));
            }
            Ok(text(joined(spec, &parts)))
        }
        "path_parent" => {
            let spec = spec(native, int(&args[1])?)?;
            let path = string(&args[0])?;
            if path.is_empty() || path.contains('\0') {
                return Err(fail(native, "invalid_input", "Invalid lexical path"));
            }
            Ok(text(path_parent(spec, path)))
        }
        "path_child" => {
            let spec = spec(native, int(&args[2])?)?;
            let (path, part) = (string(&args[0])?, string(&args[1])?);
            if path.is_empty() || part.is_empty() {
                return Err(fail(native, "invalid_input", "Path child rejects empty values"));
            }
            if path.contains('\0') || part.contains('\0') {
                return Err(fail(native, "invalid_input", "Invalid lexical path"));
            }
            Ok(text(joined(spec, &[path.to_string(), part.to_string()])))
        }
        "path_absolute" => {
            let spec = spec(native, int(&args[1])?)?;
            let path = string(&args[0])?;
            if path.is_empty() || path.contains('\0') {
                return Err(fail(native, "invalid_input", "Invalid lexical path"));
            }
            Ok(Value::Bool(spec.absolute(path)))
        }
        // ---- низкоуровневые файлы ----
        "open" => {
            let path = path(native, &args[0])?;
            let flags = int(&args[1])?.to_i64().filter(|flags| *flags <= i64::from(i32::MAX)).unwrap_or(-1);
            if options(flags).is_none() {
                return Err(fail(native, "invalid_input", "Invalid open flags"));
            }
            let mode = valid_mode(int(&args[2])?).ok_or_else(|| fail(native, "invalid_input", "Invalid file mode"))?;
            let channel = open_channel(&path, flags, mode).map_err(|error| host_error(native, &error, "Cannot open file"))?;
            Ok(new_handle(RefCell::new(OpenFile {
                channel: Some(channel),
                append: flags & APPEND != 0,
            })))
        }
        "read" => {
            let mut file = open_file(native, &args[0])?;
            let channel = file.channel.as_mut().expect("open");
            if !channel.readable {
                return Err(fail(native, "invalid_input", "File handle is not readable"));
            }
            let size = int(&args[1])?;
            if size.is_negative() {
                return Err(fail(native, "invalid_input", "Read size must be non-negative"));
            }
            if size.is_zero() {
                return Ok(byte_array([]));
            }
            let limit = size.to_i64().map_or(1 << 26, |size| size.min(1 << 26)) as usize;
            let data = read_into(&mut channel.file, limit).map_err(|error| host_error(native, &error, "Cannot read file"))?;
            Ok(byte_array(data))
        }
        "write" => {
            let mut file = open_file(native, &args[0])?;
            let append = file.append;
            let channel = file.channel.as_mut().expect("open");
            if !channel.writable {
                return Err(fail(native, "invalid_input", "File handle is not writable"));
            }
            let data = bytes(&args[1])?;
            if data.is_empty() {
                return Ok(int_value(0));
            }
            let written = write_appending(&mut channel.file, append, &data)
                .map_err(|error| host_error(native, &error, "Cannot write file"))?;
            Ok(int_value(written as i64))
        }
        "seek" => {
            let mut file = open_file(native, &args[0])?;
            let channel = file.channel.as_mut().expect("open");
            Ok(int_value(seek(native, &mut channel.file, int(&args[1])?, int(&args[2])?)?))
        }
        "flush" => {
            let file = open_file(native, &args[0])?;
            if !file.channel.as_ref().expect("open").writable {
                return Err(fail(native, "invalid_input", "File handle is not writable"));
            }
            ok
        }
        "truncate" => {
            let mut file = open_file(native, &args[0])?;
            let channel = file.channel.as_mut().expect("open");
            if !channel.writable {
                return Err(fail(native, "invalid_input", "File handle is not writable"));
            }
            let size = int(&args[1])?;
            if size.is_negative() {
                return Err(fail(native, "invalid_input", "Truncate size must be non-negative"));
            }
            truncate(&mut channel.file, size.to_i64().unwrap_or(i64::MAX) as u64)
                .map_err(|error| host_error(native, &error, "Cannot truncate file"))?;
            ok
        }
        "sync" => {
            let file = open_file(native, &args[0])?;
            let channel = file.channel.as_ref().expect("open");
            if !channel.writable {
                return Err(fail(native, "invalid_input", "File handle is not writable"));
            }
            channel.file.sync_all().map_err(|error| host_error(native, &error, "Cannot synchronize file"))?;
            ok
        }
        "lock_shared" | "lock_exclusive" => {
            let shared = name == "lock_shared";
            let mut file = open_file(native, &args[0])?;
            let channel = file.channel.as_mut().expect("open");
            let capability = if shared { channel.readable } else { channel.writable };
            if !capability {
                return Err(fail(
                    native,
                    "invalid_input",
                    format!("File handle is not {}", if shared { "readable" } else { "writable" }),
                ));
            }
            let message = format!("Cannot {name} file");
            channel.release().map_err(|error| host_error(native, &error, &message))?;
            match lock_fd(&channel.file, shared, true) {
                Ok(_) => {
                    channel.locked = true;
                    ok
                }
                // `Host.io(OverlappingFileLockException)`: код по тексту — unexpected.
                Err(None) => Err(fail(native, "unexpected", message)),
                Err(Some(error)) => Err(host_error(native, &error, &message)),
            }
        }
        "unlock" => {
            let mut file = open_file(native, &args[0])?;
            file.channel
                .as_mut()
                .expect("open")
                .release()
                .map_err(|error| host_error(native, &error, "Cannot unlock file"))?;
            ok
        }
        "close" => {
            let mut file = handle::<RefCell<OpenFile>>(&args[0])?.borrow_mut();
            if let Some(mut channel) = file.channel.take() {
                channel.release().map_err(|error| host_error(native, &error, "Cannot close file"))?;
            }
            ok
        }
        // ---- метаданные и пути ----
        "stat" | "lstat" => {
            let path = path(native, &args[0])?;
            let meta = if name == "stat" {
                std::fs::metadata(host::path(&path))
            } else {
                std::fs::symlink_metadata(host::path(&path))
            };
            let message = if name == "stat" { "Cannot stat path" } else { "Cannot lstat path" };
            Ok(stat_value(&meta.map_err(|error| host_error(native, &error, message))?))
        }
        "mkdir" => {
            let path = path(native, &args[0])?;
            let mode = valid_mode(int(&args[1])?).ok_or_else(|| fail(native, "invalid_input", "Invalid directory mode"))?;
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .mode(mode)
                .create(host::path(&path))
                .map_err(|error| host_error(native, &error, "Cannot create directory"))?;
            ok
        }
        "rmdir" => {
            let path = host::path(&path(native, &args[0])?);
            if !directory(&path) && exists(&path) {
                return Err(fail(native, "invalid_input", "Cannot rmdir path"));
            }
            match std::fs::remove_dir(&path) {
                Ok(()) => ok,
                Err(error) if host::code(&error) == "not_empty" => {
                    Err(fail(native, "not_empty", "Cannot remove non-empty directory"))
                }
                Err(error) => Err(host_error(native, &error, "Cannot rmdir path")),
            }
        }
        "unlink" => {
            let path = host::path(&path(native, &args[0])?);
            if directory(&path) {
                // unlink(2) каталога: EPERM на macOS/BSD.
                return Err(fail(native, "permission_denied", "Cannot unlink path"));
            }
            std::fs::remove_file(&path).map_err(|error| host_error(native, &error, "Cannot unlink path"))?;
            ok
        }
        "rename" => {
            let (source, target) = (path(native, &args[0])?, path(native, &args[1])?);
            std::fs::rename(host::path(&source), host::path(&target))
                .map_err(|error| host_error(native, &error, "Cannot rename path"))?;
            ok
        }
        "link" => {
            let (source, target) = (path(native, &args[0])?, path(native, &args[1])?);
            std::fs::hard_link(host::path(&source), host::path(&target))
                .map_err(|error| host_error(native, &error, "Cannot link path"))?;
            ok
        }
        "symlink" => {
            let (target, link) = (path(native, &args[0])?, path(native, &args[1])?);
            std::os::unix::fs::symlink(&target, host::path(&link))
                .map_err(|error| host_error(native, &error, "Cannot symlink path"))?;
            ok
        }
        "readlink" => {
            let path = path(native, &args[0])?;
            let target = std::fs::read_link(host::path(&path))
                .map_err(|error| host_error(native, &error, "Cannot read symbolic link"))?;
            Ok(byte_array(target.to_string_lossy().bytes()))
        }
        "cwd" => Ok(byte_array(host::cwd().to_string_lossy().bytes())),
        "chdir" => {
            let path = path(native, &args[0])?;
            let message = "Cannot change current directory";
            let real = std::fs::canonicalize(host::path(&path)).map_err(|error| host_error(native, &error, message))?;
            if !real.is_dir() {
                return Err(fail(native, "invalid_input", message));
            }
            if !executable(&real) {
                return Err(fail(native, "permission_denied", message));
            }
            host::set_cwd(real);
            ok
        }
        "chmod" => {
            let path = path(native, &args[0])?;
            let mode = valid_mode(int(&args[1])?).ok_or_else(|| fail(native, "invalid_input", "Invalid permission mode"))?;
            std::fs::set_permissions(host::path(&path), std::fs::Permissions::from_mode(mode))
                .map_err(|error| host_error(native, &error, "Cannot change path permissions"))?;
            ok
        }
        "chown" => {
            let path = path(native, &args[0])?;
            let (owner, group) = (int(&args[1])?, int(&args[2])?);
            let minus_one = IntValue::from_i64(-1);
            if crate::integer::compare(owner, &minus_one).is_lt() || crate::integer::compare(group, &minus_one).is_lt() {
                return Err(fail(native, "invalid_input", "Invalid owner or group"));
            }
            let id = |value: &IntValue| (*value != minus_one).then(|| super::support::java_int(value) as u32);
            let target = host::path(&path);
            let message = "Cannot change path ownership";
            match (id(owner), id(group)) {
                (None, None) => {
                    std::fs::metadata(&target).map_err(|error| host_error(native, &error, message))?;
                }
                (owner, group) => {
                    std::os::unix::fs::chown(&target, owner, group).map_err(|error| host_error(native, &error, message))?;
                }
            }
            ok
        }
        // ---- каталоги ----
        "opendir" => {
            let path = path(native, &args[0])?;
            let entries = names(&host::path(&path)).map_err(|error| host_error(native, &error, "Cannot open directory"))?;
            Ok(new_handle(RefCell::new(Dir {
                path,
                entries,
                index: 0,
                closed: false,
            })))
        }
        "readdir" => {
            let mut dir = handle::<RefCell<Dir>>(&args[0])?.borrow_mut();
            if dir.closed {
                return Err(closed_handle(native));
            }
            if dir.index >= dir.entries.len() {
                return Ok(byte_array([]));
            }
            dir.index += 1;
            Ok(byte_array(dir.entries[dir.index - 1].bytes()))
        }
        "rewinddir" => {
            let mut dir = handle::<RefCell<Dir>>(&args[0])?.borrow_mut();
            if dir.closed {
                return Err(closed_handle(native));
            }
            dir.entries = names(&host::path(&dir.path)).unwrap_or_default();
            dir.index = 0;
            ok
        }
        "closedir" => {
            handle::<RefCell<Dir>>(&args[0])?.borrow_mut().closed = true;
            ok
        }
        "directory_entries" => {
            let path = path(native, &args[0])?;
            let entries = names(&host::path(&path)).map_err(|error| host_error(native, &error, "Cannot open directory"))?;
            Ok(string_array(entries))
        }
        "directory_remove_tree" => {
            let path = host::path(&path(native, &args[0])?);
            if !exists(&path) {
                return Err(fail(native, "not_found", "Cannot inspect directory tree"));
            }
            if !directory(&path) {
                return Err(fail(native, "invalid_input", "Tree root must be a directory"));
            }
            remove_tree(native, &path)?;
            ok
        }
        "file_content" => {
            let path = path(native, &args[0])?;
            Ok(byte_array(
                std::fs::read(host::path(&path)).map_err(|error| host_error(native, &error, "Cannot open file"))?,
            ))
        }
        "matching_paths" => {
            let root = path(native, &args[0])?;
            let pattern = string(&args[1])?;
            if pattern.is_empty() {
                return Err(fail(native, "invalid_input", "Glob pattern cannot be empty"));
            }
            if pattern.starts_with('/') {
                return Err(fail(native, "invalid_input", "Glob pattern must be relative to its root"));
            }
            let parts: Vec<&str> = pattern.split('/').collect();
            if parts.iter().any(|part| part.is_empty()) {
                return Err(fail(native, "invalid_input", "Glob pattern contains an empty path segment"));
            }
            let segments = parts
                .iter()
                .map(|part| glob_segment(native, part))
                .collect::<Result<Vec<_>, _>>()?;
            let root_path = host::path(&root);
            if !exists(&root_path) {
                return Err(fail(native, "not_found", "Cannot inspect glob root"));
            }
            if !directory(&root_path) {
                return Err(fail(native, "invalid_input", "Glob root must be a directory"));
            }
            let mut matches = BTreeSet::new();
            visit(&root, true, 0, true, &segments, &mut matches, &mut HashSet::new())
                .map_err(|error| host_error(native, &error, "Cannot open directory"))?;
            Ok(string_array(matches))
        }
        // ---- FileState ----
        "file_state_create" => {
            let path = path(native, &args[0])?;
            let flags = state_flags(int(&args[1])?).ok_or_else(|| fail(native, "invalid_input", "Invalid file mode"))?;
            let encoding = string(&args[2])?;
            let binary = args[3].as_bool()?;
            if binary && !encoding.is_empty() {
                return Err(fail(native, "invalid_input", "Binary files cannot have an encoding"));
            }
            if !binary && !known_encoding(encoding) {
                return Err(fail(native, "unsupported", "Unsupported file encoding"));
            }
            Ok(new_handle(Rc::new(RefCell::new(FileState {
                path,
                mode: int(&args[1])?.to_i64().expect("valid mode"),
                flags,
                binary,
                encoding: encoding.to_string(),
                ..FileState::default()
            }))))
        }
        "file_state_read" => {
            let state = file_state(native, &args[0])?;
            let mut state = state.borrow_mut();
            let limit = int(&args[1])?;
            if limit.signum() <= 0 {
                return Err(fail(native, "invalid_input", "Read limit must be positive"));
            }
            prepare_read(native, &mut state, false)?;
            let size = limit.to_i64().map_or(1 << 26, |limit| limit.min(1 << 26)) as usize;
            let file = &mut state.channel.as_mut().expect("opened").file;
            Ok(byte_array(read_into(file, size).map_err(|error| host_error(native, &error, "Cannot read file"))?))
        }
        "file_state_read_text" => {
            let state = file_state(native, &args[0])?;
            let mut state = state.borrow_mut();
            prepare_read(native, &mut state, true)?;
            while !state.eof {
                fill(native, &mut state)?;
            }
            let data = std::mem::take(&mut state.buffer);
            Ok(text(decode_text(native, &state, &data)?))
        }
        "file_state_read_line" => {
            let state = file_state(native, &args[0])?;
            let mut state = state.borrow_mut();
            prepare_read(native, &mut state, true)?;
            let needle = newline(&state.encoding);
            loop {
                if let Some(offset) = line_offset(&state.buffer, needle) {
                    let line: Vec<u8> = state.buffer[..offset].to_vec();
                    state.buffer.drain(..offset + needle.len());
                    let decoded = decode_text(native, &state, &line)?;
                    return Ok(text(decoded.strip_suffix('\r').unwrap_or(&decoded).to_string()));
                }
                if state.eof {
                    if state.buffer.is_empty() {
                        return Err(fail(native, "end_of_input", "End of input"));
                    }
                    let data = std::mem::take(&mut state.buffer);
                    return Ok(text(decode_text(native, &state, &data)?));
                }
                fill(native, &mut state)?;
            }
        }
        "file_state_write" => {
            let state = file_state(native, &args[0])?;
            let mut state = state.borrow_mut();
            prepare_write(native, &mut state, false)?;
            let data = bytes(&args[1])?;
            if data.is_empty() {
                return Ok(int_value(0));
            }
            let append = appending(&state);
            let file = &mut state.channel.as_mut().expect("opened").file;
            let written = write_appending(file, append, &data).map_err(|error| host_error(native, &error, "Cannot write file"))?;
            Ok(int_value(written as i64))
        }
        "file_state_write_text" => {
            let state = file_state(native, &args[0])?;
            let mut state = state.borrow_mut();
            prepare_write(native, &mut state, true)?;
            let mut content: String = paths(native, &args[1])?.concat();
            if args[2].as_bool()? {
                content.push('\n');
            }
            let encoded =
                jre::encode(&content, &state.encoding).ok_or_else(|| fail(native, "invalid_data", "Text cannot be encoded"))?;
            let append = appending(&state);
            let file = &mut state.channel.as_mut().expect("opened").file;
            if append {
                file.seek(SeekFrom::End(0)).map_err(|error| host_error(native, &error, "Cannot write file"))?;
            }
            file.write_all(&encoded).map_err(|error| host_error(native, &error, "Cannot write file"))?;
            Ok(int_value(encoded.len() as i64))
        }
        "file_state_flush" => {
            let state = file_state(native, &args[0])?;
            let mut state = state.borrow_mut();
            if !writable(&state) {
                return Err(fail(native, "invalid_input", "File mode does not permit writing"));
            }
            state_open(native, &mut state)?;
            ok
        }
        "file_state_close" => {
            let state = handle::<StateRef>(&args[0])?.clone();
            let mut state = state.borrow_mut();
            if state.closed {
                cleanup_temporary(native, &mut state)?;
                return ok;
            }
            release_lock(&mut state);
            state.channel = None;
            state.buffer.clear();
            state.closed = true;
            cleanup_temporary(native, &mut state)?;
            ok
        }
        "file_state_closed" => Ok(Value::Bool(handle::<StateRef>(&args[0])?.borrow().closed)),
        "file_state_position" => {
            let state = file_state(native, &args[0])?;
            let mut state = state.borrow_mut();
            if !state.binary {
                return Err(fail(native, "invalid_input", "Byte position is unavailable in text mode"));
            }
            state_open(native, &mut state)?;
            let file = &mut state.channel.as_mut().expect("opened").file;
            let position = file
                .stream_position()
                .map_err(|error| host_error(native, &error, "Cannot inspect file position"))?;
            Ok(int_value(position as i64))
        }
        "file_state_seek" => {
            let state = file_state(native, &args[0])?;
            let mut state = state.borrow_mut();
            if !state.binary {
                return Err(fail(native, "invalid_input", "Byte seeking is unavailable in text mode"));
            }
            if !int(&args[2])?.to_i64().is_some_and(|origin| (0..=2).contains(&origin)) {
                return Err(fail(native, "invalid_input", "Invalid seek origin"));
            }
            state_open(native, &mut state)?;
            let file = &mut state.channel.as_mut().expect("opened").file;
            let position = seek(native, file, int(&args[1])?, int(&args[2])?)?;
            state.buffer.clear();
            state.eof = false;
            state.last = None;
            Ok(int_value(position))
        }
        "file_state_truncate" => {
            let state = file_state(native, &args[0])?;
            let mut state = state.borrow_mut();
            if !state.binary {
                return Err(fail(native, "invalid_input", "Truncation is unavailable in text mode"));
            }
            if !writable(&state) {
                return Err(fail(native, "invalid_input", "File mode does not permit writing"));
            }
            let size = int(&args[1])?;
            if size.is_negative() {
                return Err(fail(native, "invalid_input", "Truncate size must be non-negative"));
            }
            state_open(native, &mut state)?;
            let file = &mut state.channel.as_mut().expect("opened").file;
            truncate(file, size.to_i64().unwrap_or(i64::MAX) as u64)
                .map_err(|error| host_error(native, &error, "Cannot truncate file"))?;
            ok
        }
        "file_state_lock" => {
            let shared_state = file_state(native, &args[0])?;
            let mut state = shared_state.borrow_mut();
            if state.lock.as_ref().is_some_and(|lock| !lock.borrow().closed) {
                return Err(fail(native, "invalid_input", "File already has a live lock"));
            }
            let mode = int(&args[1])?.to_i64().filter(|mode| (0..=1).contains(mode));
            let Some(mode) = mode else {
                return Err(fail(native, "invalid_input", "Invalid file lock mode"));
            };
            let waiting = int(&args[2])?.to_i64().filter(|waiting| (0..=1).contains(waiting));
            let Some(waiting) = waiting else {
                return Err(fail(native, "invalid_input", "Invalid file lock waiting mode"));
            };
            state_open(native, &mut state)?;
            let shared = mode == 0;
            let channel = state.channel.as_mut().expect("opened");
            if (shared && !channel.readable) || (!shared && !channel.writable) {
                return Err(fail(native, "invalid_input", "Cannot acquire file lock"));
            }
            match lock_fd(&channel.file, shared, waiting == 0) {
                Ok(true) => channel.locked = true,
                Ok(false) => return Err(fail(native, "would_block", "File lock is unavailable")),
                Err(None) => {
                    let message = if waiting != 0 { "File lock is unavailable" } else { "Cannot acquire file lock" };
                    return Err(fail(native, "would_block", message));
                }
                Err(Some(error)) => return Err(host_error(native, &error, "Cannot acquire file lock")),
            }
            let lock = Rc::new(RefCell::new(LockState {
                file: Some(Rc::downgrade(&shared_state)),
                closed: false,
            }));
            state.lock = Some(lock.clone());
            Ok(new_handle(lock))
        }
        "file_lock_closed" => Ok(Value::Bool(handle::<Rc<RefCell<LockState>>>(&args[0])?.borrow().closed)),
        "file_lock_close" => {
            let lock = handle::<Rc<RefCell<LockState>>>(&args[0])?.clone();
            if lock.borrow().closed {
                return ok;
            }
            let file = lock.borrow_mut().file.take().and_then(|file| file.upgrade());
            if let Some(file) = file {
                let mut file = file.borrow_mut();
                if let Some(channel) = file.channel.as_mut() {
                    channel.release().map_err(|error| host_error(native, &error, "Cannot release file lock"))?;
                }
                if file.lock.as_ref().is_some_and(|known| Rc::ptr_eq(known, &lock)) {
                    file.lock = None;
                }
            }
            lock.borrow_mut().closed = true;
            ok
        }
        // ---- временные файлы и каталоги ----
        "temporary_file_create" => {
            let directory = path(native, &args[0])?;
            let prefix = prefix(native, string(&args[1])?)?;
            for _ in 0..100 {
                let path = child(&directory, &temporary_name(prefix));
                match open_channel(&path, READ | WRITE | CREATE | EXCLUSIVE, 0o600) {
                    Ok(channel) => {
                        return Ok(new_handle(Rc::new(RefCell::new(FileState {
                            path: path.clone(),
                            mode: 3,
                            flags: READ | WRITE,
                            binary: true,
                            channel: Some(channel),
                            temporary: true,
                            delete_on_close: Some(path),
                            ..FileState::default()
                        }))));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => return Err(host_error(native, &error, "Cannot create temporary file")),
                }
            }
            Err(fail(native, "unexpected", "Cannot create temporary file"))
        }
        "temporary_file_path" => {
            let state = handle::<StateRef>(&args[0])?.borrow();
            if !state.temporary {
                return Err(fail(native, "invalid_input", "File is not temporary"));
            }
            Ok(byte_array(state.path.bytes()))
        }
        "temporary_file_preserve" => {
            let shared = handle::<StateRef>(&args[0])?.clone();
            let destination = path(native, &args[1])?;
            let mut state = shared.borrow_mut();
            if !state.temporary {
                return Err(fail(native, "invalid_input", "File is not temporary"));
            }
            if state.closed {
                return Err(fail(native, "already_closed", "Temporary file is already closed"));
            }
            release_lock(&mut state);
            state.channel = None;
            state.closed = true;
            let (source, target) = (host::path(&state.path), host::path(&destination));
            let message = "Cannot preserve temporary file";
            if let Err(error) = std::fs::hard_link(&source, &target) {
                if host::code(&error) != "unsupported" {
                    return Err(host_error(native, &error, message));
                }
                std::fs::copy(&source, &target).map_err(|error| host_error(native, &error, message))?;
            }
            std::fs::remove_file(&source).map_err(|error| host_error(native, &error, message))?;
            state.delete_on_close = None;
            state.path = destination;
            ok
        }
        "temporary_directory_create" => {
            let parent = path(native, &args[0])?;
            let prefix = prefix(native, string(&args[1])?)?;
            use std::os::unix::fs::DirBuilderExt;
            for _ in 0..100 {
                let path = child(&parent, &temporary_name(prefix));
                match std::fs::DirBuilder::new().mode(0o700).create(host::path(&path)) {
                    Ok(()) => {
                        return Ok(new_handle(TemporaryDirectory {
                            path,
                            closed: std::cell::Cell::new(false),
                        }));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => return Err(host_error(native, &error, "Cannot create temporary directory")),
                }
            }
            Err(fail(native, "unexpected", "Cannot create temporary directory"))
        }
        "temporary_directory_path" => Ok(byte_array(handle::<TemporaryDirectory>(&args[0])?.path.bytes())),
        "temporary_directory_closed" => Ok(Value::Bool(handle::<TemporaryDirectory>(&args[0])?.closed.get())),
        "temporary_directory_close" => {
            let directory = handle::<TemporaryDirectory>(&args[0])?;
            if directory.closed.get() {
                return ok;
            }
            let path = host::path(&directory.path);
            if exists(&path) {
                remove_tree(native, &path)?;
            }
            directory.closed.set(true);
            ok
        }
        // ---- наблюдение ----
        "file_watch_create" => {
            let path = path(native, &args[0])?;
            let message = "Cannot inspect watched path";
            let metadata = metadata(&path)
                .map_err(|error| host_error(native, &error, message))?
                .ok_or_else(|| fail(native, "not_found", "Watched path does not exist"))?;
            let children = if metadata.directory {
                children(&path).map_err(|error| host_error(native, &error, message))?
            } else {
                Vec::new()
            };
            Ok(new_handle(Rc::new(RefCell::new(Watch {
                path,
                metadata: Some(metadata),
                children,
                events: Vec::new(),
                waiter: None,
                closed: false,
            }))))
        }
        "file_watch_event" => watch_event(native, &args[0]),
        "file_watch_closed" => Ok(Value::Bool(handle::<Rc<RefCell<Watch>>>(&args[0])?.borrow().closed)),
        "file_watch_close" => {
            let watch = handle::<Rc<RefCell<Watch>>>(&args[0])?.clone();
            if watch.borrow().closed {
                return ok;
            }
            watch.borrow_mut().closed = true;
            let waiter = watch.borrow_mut().waiter.take();
            if let Some(waiter) = waiter {
                sched::settle(
                    &waiter,
                    Err(native_failure(fail(native, "already_closed", "File watch is already closed"))),
                );
            }
            ok
        }
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}

/// `access(path, X_OK)`.
fn executable(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(text) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: строка с завершающим нулём живёт до конца вызова.
    unsafe { libc::access(text.as_ptr(), libc::X_OK) == 0 }
}

#[cfg(test)]
mod tests {
    use super::{Spec, path_parent};

    #[test]
    fn file_spec_paths_match_perl() {
        assert_eq!(Spec::Unix.canonpath("a//b/./c/"), "a/b/c");
        assert_eq!(Spec::Unix.canonpath("/../x"), "/x");
        assert_eq!(path_parent(Spec::Unix, "/a/b"), "/a");
        assert_eq!(path_parent(Spec::Unix, "/a"), "/");
        assert_eq!(path_parent(Spec::Unix, "a"), ".");
        assert_eq!(Spec::Win32.canonpath("c:/a/../b"), "C:\\b");
        assert_eq!(Spec::Win32.canonpath("\\\\server\\share\\x"), "\\\\server\\share\\x");
    }
}
