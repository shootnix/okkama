//! Portable POSIX/Windows path syntax; no filesystem access or VM values.
use crate::host::path_regex as jre;
const JAVA_END: &str = r"(?:(?=\r\n\z)|(?=[\n\r\u{85}\u{2028}\u{2029}]\z)|\z)";
/// Perl `split` без предела (`keep_trailing` — с пределом -1).
fn perl_split(text: &str, pattern: &str, keep_trailing: bool) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    jre::split(text, pattern, if keep_trailing { -1 } else { 0 })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Spec {
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
    } else if let Some(unc) = jre::captures(
        r"(?s)\A(?:\\\\|//)([^\\/]+)(?:[\\/]([^\\/]+))?[\\/]?",
        &first,
    ) {
        volume = format!(
            "\\\\{}{}\\",
            unc[1].clone().unwrap_or_default(),
            unc[2]
                .as_ref()
                .map_or(String::new(), |share| format!("\\{share}"))
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

    pub(crate) fn absolute(self, path: &str) -> bool {
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
                let found = jre::captures(r"(?s)^((?:.*/(?:\.\.?\z)?)?)([^/]*)", path)
                    .expect("always matches");
                [
                    String::new(),
                    found[1].clone().unwrap_or_default(),
                    found[2].clone().unwrap_or_default(),
                ]
            }
            Spec::Win32 => {
                let found = jre::captures(
                    &format!(r"(?s)^({}?)((?:.*[\\/](?:\.\.?\z)?)?)(.*)", vol()),
                    path,
                )
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
                if !directory.is_empty()
                    && !file.is_empty()
                    && !directory.ends_with('/')
                    && !file.starts_with('/')
                {
                    format!("{directory}/{file}")
                } else {
                    format!("{directory}{file}")
                }
            }
            Spec::Win32 => {
                let mut volume = volume.to_string();
                if let Some(share) =
                    jre::captures(r"(?s)^([\\/])[\\/][^\\/]+[\\/][^\\/]+\z", &volume)
                {
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

pub(crate) fn joined(spec: Spec, parts: &[String]) -> String {
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

pub(crate) fn path_parent(spec: Spec, path: &str) -> String {
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
