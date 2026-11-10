//! Состояние хоста для providers, как `okkama.rt.Host` JVM: текущий каталог
//! программы виртуальный (процесс не меняет свой cwd, `chdir` программы меняет
//! копию, от неё разрешаются относительные пути natives), окружение — копия,
//! которую получают дочерние процессы; переносимые коды отказов ввода-вывода.
//! Состояние принадлежит сессии запуска ([`Session`]), а не потоку.
#![allow(unsafe_code)]
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Начальное состояние хоста запуска: текущий каталог и окружение, которые
/// программа видит при старте.
#[derive(Clone, Debug)]
pub struct Context {
    pub cwd: PathBuf,
    pub environment: BTreeMap<String, String>,
}

impl Context {
    /// Снимок процесса: его текущий каталог и окружение плюс переменные
    /// запускающего ([`default_environment`]), которых у процесса нет.
    pub fn process() -> Self {
        let mut environment: BTreeMap<String, String> = std::env::vars().collect();
        if let Ok(defaults) = DEFAULTS.lock() {
            for (name, value) in defaults.iter() {
                environment.entry(name.clone()).or_insert_with(|| value.clone());
            }
        }
        Self { cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")), environment }
    }

    /// Без переменной `name` (служебные метки запускающего).
    pub fn without(mut self, name: &str) -> Self {
        self.environment.remove(name);
        self
    }
}

type State = Context;

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Переменные, которые запускающий добавляет в окружение программы, если их
/// нет у процесса (как `okkama run` передаёт `OKKAMA_ICU_LIBDIR` JVM).
static DEFAULTS: std::sync::Mutex<BTreeMap<String, String>> = std::sync::Mutex::new(BTreeMap::new());

pub fn default_environment(name: &str, value: &str) {
    if let Ok(mut defaults) = DEFAULTS.lock() {
        defaults.insert(name.into(), value.into());
    }
}

/// Сессия хоста одного запуска (#40): пока она жива, natives потока видят её
/// каталог и окружение; `chdir` и изменения окружения программы остаются в
/// ней и исчезают вместе с ней, а прежнее состояние потока возвращается.
/// Процесс не меняется никогда. Каждая точка входа движка открывает сессию,
/// поэтому второй запуск в том же потоке не наследует состояние первого.
#[derive(Debug)]
pub struct Session {
    previous: Option<State>,
    /// Сессия принадлежит потоку, в котором открыта.
    _thread: std::marker::PhantomData<*const ()>,
}

/// Открыть сессию с начальным состоянием `context`.
pub fn enter(context: Context) -> Session {
    let previous = STATE.with(|state| state.borrow_mut().replace(context));
    Session { previous, _thread: std::marker::PhantomData }
}

/// Открыть сессию со снимком процесса ([`Context::process`]).
pub fn session() -> Session {
    enter(Context::process())
}

impl Drop for Session {
    fn drop(&mut self) {
        let previous = self.previous.take();
        STATE.with(|state| *state.borrow_mut() = previous);
    }
}

/// Вне сессии (REPL, прямые вызовы natives) состояние потока создаётся
/// снимком процесса при первом обращении.
fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        f(state.get_or_insert_with(Context::process))
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

/// Значение одной переменной окружения программы.
pub fn variable(name: &str) -> Option<String> {
    with(|state| state.environment.get(name).cloned())
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

/// SIGKILL группе процессов потомка, который её возглавляет (#359).
pub fn kill_group(pid: u32) -> bool {
    // SAFETY: kill(2) с отрицательным pid — группе процессов `pid`.
    unsafe { libc::kill(-(pid as i32), libc::SIGKILL) == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(cwd: &str, variable: &str) -> Context {
        Context { cwd: cwd.into(), environment: BTreeMap::from([(variable.to_string(), "1".to_string())]) }
    }

    #[test]
    fn runs_in_one_thread_do_not_share_state() {
        {
            let _first = enter(context("/first", "FIRST"));
            set_cwd("/elsewhere".into());
            set_environment("LEAKED", Some("yes"));
            assert_eq!(path("file"), PathBuf::from("/elsewhere/file"));
        }
        let _second = enter(context("/second", "SECOND"));
        assert_eq!(cwd(), PathBuf::from("/second"));
        assert_eq!(environment().keys().collect::<Vec<_>>(), ["SECOND"]);
    }

    #[test]
    fn a_nested_session_restores_the_outer_one() {
        let _outer = enter(context("/outer", "OUTER"));
        {
            let _inner = enter(context("/inner", "INNER"));
            set_environment("OUTER", Some("changed"));
        }
        assert_eq!(cwd(), PathBuf::from("/outer"));
        assert_eq!(environment()["OUTER"], "1");
    }

    #[test]
    fn a_session_never_changes_the_process() {
        let before = (std::env::current_dir().unwrap(), std::env::var_os("OKKAMA_HOST_SESSION_TEST"));
        {
            let _session = session();
            set_cwd("/".into());
            set_environment("OKKAMA_HOST_SESSION_TEST", Some("1"));
        }
        assert_eq!((std::env::current_dir().unwrap(), std::env::var_os("OKKAMA_HOST_SESSION_TEST")), before);
    }
}
