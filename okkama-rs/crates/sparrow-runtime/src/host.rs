//! Состояние хоста для providers, как `okkama.rt.Host` JVM: текущий каталог
//! программы виртуальный (процесс не меняет свой cwd, `chdir` программы меняет
//! копию, от неё разрешаются относительные пути natives), окружение — копия,
//! которую получают дочерние процессы; переносимые коды отказов ввода-вывода.
#![allow(unsafe_code)]
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

struct State {
    cwd: PathBuf,
    environment: BTreeMap<String, String>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Переменные, которые запускающий добавляет в окружение программы, если их
/// нет у процесса (как `okkama run` передаёт `OKKAMA_ICU_LIBDIR` JVM).
static DEFAULTS: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());

pub fn default_environment(name: &str, value: &str) {
    if let Ok(mut defaults) = DEFAULTS.lock() {
        defaults.push((name.into(), value.into()));
    }
}

fn initial_environment() -> BTreeMap<String, String> {
    let mut environment: BTreeMap<String, String> = std::env::vars().collect();
    if let Ok(defaults) = DEFAULTS.lock() {
        for (name, value) in defaults.iter() {
            environment.entry(name.clone()).or_insert_with(|| value.clone());
        }
    }
    environment
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let state = state.get_or_insert_with(|| State {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            environment: initial_environment(),
        });
        f(state)
    })
}

/// Абсолютный текущий каталог программы.
pub fn cwd() -> PathBuf {
    with(|state| state.cwd.clone())
}

pub fn set_cwd(path: PathBuf) {
    with(|state| state.cwd = path);
}

/// Путь хоста: относительный — от текущего каталога программы.
pub fn path(text: &str) -> PathBuf {
    with(|state| state.cwd.join(text))
}

pub fn environment() -> BTreeMap<String, String> {
    with(|state| state.environment.clone())
}

pub fn set_environment(name: &str, value: Option<&str>) {
    with(|state| match value {
        Some(value) => {
            state.environment.insert(name.into(), value.into());
        }
        None => {
            state.environment.remove(name);
        }
    });
}

/// Переносимый код отказа (`portable_code_for_errno`).
pub fn code(error: &std::io::Error) -> &'static str {
    use std::io::ErrorKind::*;
    match error.kind() {
        NotFound => return "not_found",
        PermissionDenied => return "permission_denied",
        AlreadyExists => return "already_exists",
        DirectoryNotEmpty => return "not_empty",
        NotADirectory | IsADirectory | InvalidInput | InvalidFilename => return "invalid_input",
        CrossesDevices | Unsupported => return "unsupported",
        StorageFull | OutOfMemory | QuotaExceeded => return "resource_exhausted",
        BrokenPipe => return "broken_pipe",
        Interrupted => return "interrupted",
        WouldBlock => return "would_block",
        ReadOnlyFilesystem => return "permission_denied",
        _ => {}
    }
    let text = error.to_string().to_lowercase();
    if text.contains("no such file") {
        "not_found"
    } else if text.contains("permission denied") || text.contains("not permitted") || text.contains("read-only file system") {
        "permission_denied"
    } else if text.contains("file exists") {
        "already_exists"
    } else if text.contains("not empty") {
        "not_empty"
    } else if ["is a directory", "not a directory", "invalid argument", "too many levels", "name too long", "bad file descriptor"]
        .iter()
        .any(|needle| text.contains(needle))
    {
        "invalid_input"
    } else if text.contains("cross-device") || text.contains("not supported") {
        "unsupported"
    } else if ["no space", "too many open files", "quota", "cannot allocate"]
        .iter()
        .any(|needle| text.contains(needle))
    {
        "resource_exhausted"
    } else if text.contains("broken pipe") {
        "broken_pipe"
    } else if text.contains("interrupted") {
        "interrupted"
    } else if text.contains("resource temporarily unavailable") || text.contains("would block") {
        "would_block"
    } else {
        "unexpected"
    }
}

/// Текст `strerror` ошибки, как причина `FileSystemException` JVM.
pub fn strerror(error: &std::io::Error) -> String {
    let text = error.to_string();
    match text.rfind(" (os error ") {
        Some(at) => text[..at].to_string(),
        None => text,
    }
}

/// Код и текст ошибки, как `_errno` Perl (`SystemNatives.reason` JVM).
pub fn reason(error: &std::io::Error) -> (&'static str, String) {
    let code = code(error);
    let text = match code {
        "not_found" => "No such file or directory".to_string(),
        "permission_denied" => "Permission denied".to_string(),
        "already_exists" => "File exists".to_string(),
        "not_empty" => "Directory not empty".to_string(),
        _ => strerror(error),
    };
    (code, text)
}

/// `access(path, R_OK)`: путь доступен для чтения.
pub fn readable(path: &std::path::Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(text) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: строка с завершающим нулём живёт до конца вызова.
    unsafe { libc::access(text.as_ptr(), libc::R_OK) == 0 }
}

/// Сигнал процессу-потомку (`Process.destroy` JVM).
pub fn signal(pid: u32, number: i32) -> bool {
    // SAFETY: kill(2) по идентификатору потомка.
    unsafe { libc::kill(pid as i32, number) == 0 }
}
