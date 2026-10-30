//! Дополнение путей `std.…` в редакторе (`okkama lsp`, §5.2.1).
//!
//! Путь у курсора читается назад от него: `std.fs.pa|`. Пока сегменты
//! строчные, это каталоги и модули std: предлагаются папки и файлы
//! текущего уровня, а если путь уже назвал модуль (`std.io.stdout.`) — его
//! объявления. Целые пути (`io.stdout`, а с первой набранной буквой и
//! `io.stdout.Stdout`) ищутся по всей std. После enum (`std.compare.Order.`)
//! предлагаются его варианты. Каталог читается при каждом запросе, разбор
//! модулей для целых путей кэшируется на время работы сервера.
use crate::source::SourceFile;
use crate::syntax::{self, ParseMode, ParseOptions, ast};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// Вид предложения пути std.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StdKind {
    Folder,
    Module,
    Object,
    Role,
    Enum,
    EnumCase,
    Native,
}

/// Предложение дополнения пути std.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StdCompletion {
    /// Вставляемый текст: сегмент (`stdout`) или целый путь (`io.stdout.Stdout`).
    pub label: String,
    pub kind: StdKind,
    pub detail: String,
    pub documentation: String,
    /// Целый путь из глубины std, а не элемент текущего уровня.
    pub deep: bool,
}

/// Предложения для курсора `offset`; `None` — курсор не в пути `std.…`.
pub fn std_completions(text: &str, offset: u32, stdlib: &Path) -> Option<Vec<StdCompletion>> {
    let (segments, partial) = path_at(text, offset as usize)?;
    if let Some(last) = segments.last().filter(|segment| segment.starts_with(char::is_uppercase)) {
        // `std.module.Enum.`: варианты enum.
        let module = segments[..segments.len() - 1].join("/");
        let declarations = declarations(&stdlib.join(format!("{module}.ok")));
        let cases = declarations
            .into_iter()
            .find(|decl| decl.name == *last)
            .map(|decl| decl.cases)
            .unwrap_or_default();
        return Some(
            cases
                .into_iter()
                .map(|case| StdCompletion {
                    detail: format!("{last}.{case}"),
                    label: case,
                    kind: StdKind::EnumCase,
                    documentation: String::new(),
                    deep: false,
                })
                .collect(),
        );
    }
    let directory = segments.iter().fold(stdlib.to_path_buf(), |dir, segment| dir.join(segment));
    let mut items = Vec::new();
    // Папки и модули текущего уровня.
    if let Ok(entries) = std::fs::read_dir(&directory) {
        let mut entries: Vec<(String, bool)> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let folder = entry.file_type().ok()?.is_dir();
                if folder {
                    is_segment(&name).then_some((name, true))
                } else {
                    let stem = name.strip_suffix(".ok")?;
                    is_segment(stem).then(|| (stem.to_string(), false))
                }
            })
            .collect();
        entries.sort();
        entries.dedup();
        for (name, folder) in entries {
            let path = segments.iter().cloned().chain([name.clone()]).collect::<Vec<_>>();
            if folder {
                items.push(StdCompletion {
                    detail: format!("std.{} (folder)", path.join(".")),
                    label: name,
                    kind: StdKind::Folder,
                    documentation: String::new(),
                    deep: false,
                });
            } else {
                let module = path.join("/");
                items.push(StdCompletion {
                    detail: format!("module {module}"),
                    documentation: module_doc(&stdlib.join(format!("{module}.ok"))),
                    label: name,
                    kind: StdKind::Module,
                    deep: false,
                });
            }
        }
    }
    // Путь уже назвал модуль: его объявления.
    if !segments.is_empty() {
        let module = segments.join("/");
        for decl in declarations(&stdlib.join(format!("{module}.ok"))) {
            items.push(decl.completion(decl.name.clone(), false));
        }
    }
    // Целые пути из глубины: модули, а с набранной буквой — и объявления.
    let prefix = segments.join("/");
    let index = deep_index(stdlib);
    let lower = partial.to_lowercase();
    for module in index.iter() {
        let Some(rest) = relative(&module.path, &prefix) else {
            continue;
        };
        if rest.contains('/') {
            items.push(StdCompletion {
                label: rest.replace('/', "."),
                kind: StdKind::Module,
                detail: format!("module {}", module.path),
                documentation: module.doc.clone(),
                deep: true,
            });
        }
        if !lower.is_empty() && !rest.is_empty() {
            for decl in &module.declarations {
                if decl.name.to_lowercase().contains(&lower) {
                    items.push(decl.completion(format!("{}.{}", rest.replace('/', "."), decl.name), true));
                }
            }
        }
    }
    Some(items)
}

/// Сегменты пути до курсора (без `std`) и набираемый сегмент.
fn path_at(text: &str, offset: usize) -> Option<(Vec<String>, String)> {
    let before = text.get(..offset)?;
    let word = |ch: char| ch.is_alphanumeric() || ch == '_';
    let partial_start = before.rfind(|ch: char| !word(ch)).map_or(0, |index| index + 1);
    let partial = before[partial_start..].to_string();
    let mut segments = Vec::new();
    let mut end = partial_start;
    loop {
        // Перед сегментом — точка, перед ней — следующий сегмент назад.
        if !before[..end].ends_with('.') {
            return None;
        }
        let dot = end - 1;
        let start = before[..dot].rfind(|ch: char| !word(ch)).map_or(0, |index| index + 1);
        let segment = &before[start..dot];
        if segment.is_empty() {
            return None;
        }
        if segment == "std" {
            // `std` — начало пути, а не поле: перед ним не точка и не буква.
            if before[..start].ends_with('.') {
                return None;
            }
            segments.reverse();
            return Some((segments, partial));
        }
        segments.push(segment.to_string());
        end = start;
    }
}

/// Сегмент пути модуля: имя каталога или файла std со строчной буквы.
fn is_segment(name: &str) -> bool {
    name.starts_with(|ch: char| ch.is_ascii_lowercase())
        && name.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// `module` внутри `prefix` (`io/stdout` в `io` → `stdout`); `None` — снаружи.
fn relative<'a>(module: &'a str, prefix: &str) -> Option<&'a str> {
    if prefix.is_empty() {
        return Some(module);
    }
    module.strip_prefix(prefix)?.strip_prefix('/')
}

#[derive(Clone, Debug)]
struct Declaration {
    name: String,
    kind: StdKind,
    detail: String,
    doc: String,
    cases: Vec<String>,
}

impl Declaration {
    fn completion(&self, label: String, deep: bool) -> StdCompletion {
        StdCompletion {
            label,
            kind: self.kind,
            detail: self.detail.clone(),
            documentation: self.doc.clone(),
            deep,
        }
    }
}

#[derive(Debug)]
struct Module {
    /// `io/stdout`.
    path: String,
    doc: String,
    declarations: Vec<Declaration>,
}

/// Модули std с объявлениями, разобранные один раз на каталог std.
fn deep_index(stdlib: &Path) -> Arc<Vec<Module>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<Vec<Module>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(found) = cache.lock().ok().and_then(|cache| cache.get(stdlib).cloned()) {
        return found;
    }
    let mut files = Vec::new();
    collect(stdlib, stdlib, &mut files);
    files.sort();
    let modules = Arc::new(
        files
            .into_iter()
            .map(|path| {
                let file = stdlib.join(format!("{path}.ok"));
                Module {
                    doc: module_doc(&file),
                    declarations: declarations(&file),
                    path,
                }
            })
            .collect::<Vec<_>>(),
    );
    if let Ok(mut cache) = cache.lock() {
        cache.insert(stdlib.to_path_buf(), modules.clone());
    }
    modules
}

/// Пути модулей (`io/stdout`) каталога `dir` и вложенных.
fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if is_segment(&name) {
                collect(root, &path, out);
            }
        } else if let Some(stem) = name.strip_suffix(".ok")
            && is_segment(stem)
            && let Ok(relative) = path.with_extension("").strip_prefix(root)
        {
            let parts: Vec<String> =
                relative.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
            out.push(parts.join("/"));
        }
    }
}

/// Объявления модуля-файла: объекты, роли, enum и native-функции.
fn declarations(file: &Path) -> Vec<Declaration> {
    let Ok(text) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    let Ok(source) = SourceFile::new(file.display().to_string(), text) else {
        return Vec::new();
    };
    let options = ParseOptions {
        mode: ParseMode::Production,
        ..ParseOptions::default()
    };
    let Ok(program) = syntax::parse(&source, &options) else {
        return Vec::new();
    };
    let doc = |span: crate::source::Span| comment_above(&source, span.start);
    let params = |params: &[ast::Param]| {
        params.iter().map(|param| param.name.text.clone()).collect::<Vec<_>>().join(", ")
    };
    program
        .items
        .iter()
        .filter_map(|item| {
            Some(match item {
                ast::Item::Object(decl) => {
                    let ctors: Vec<String> = decl
                        .members
                        .iter()
                        .filter_map(|member| match member {
                            ast::ObjectMember::Constructor(ctor) => Some(match &ctor.form {
                                ast::ConstructorForm::Full { params: list, .. } => {
                                    format!("{}({})", decl.name.text, params(list))
                                }
                                ast::ConstructorForm::Shorthand { fields, .. } => {
                                    let fields: Vec<String> =
                                        fields.iter().map(|field| format!(":{}", field.text)).collect();
                                    format!("{}({})", decl.name.text, fields.join(", "))
                                }
                            }),
                            _ => None,
                        })
                        .collect();
                    Declaration {
                        name: decl.name.text.clone(),
                        kind: StdKind::Object,
                        detail: if ctors.is_empty() {
                            format!("object {}", decl.name.text)
                        } else {
                            ctors.join(" | ")
                        },
                        doc: doc(decl.span),
                        cases: Vec::new(),
                    }
                }
                ast::Item::Role(decl) => Declaration {
                    name: decl.name.text.clone(),
                    kind: StdKind::Role,
                    detail: format!("role {}", decl.name.text),
                    doc: doc(decl.span),
                    cases: Vec::new(),
                },
                ast::Item::Enum(decl) => {
                    let cases: Vec<String> = decl.cases.iter().map(|case| case.text.clone()).collect();
                    Declaration {
                        name: decl.name.text.clone(),
                        kind: StdKind::Enum,
                        detail: format!("enum {} = ({})", decl.name.text, cases.join(", ")),
                        doc: doc(decl.span),
                        cases,
                    }
                }
                ast::Item::NativeAction(decl) => Declaration {
                    name: decl.name.text.clone(),
                    kind: StdKind::Native,
                    detail: format!("native {}({})", decl.name.text, params(&decl.params)),
                    doc: doc(decl.span),
                    cases: Vec::new(),
                },
                _ => return None,
            })
        })
        .collect()
}

/// Комментарий модуля: строки `//` сразу после объявления `module`.
fn module_doc(file: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(file) else {
        return String::new();
    };
    let mut lines = text.lines().skip_while(|line| !line.trim_start().starts_with("module "));
    lines.next();
    let doc: Vec<&str> = lines
        .skip_while(|line| line.trim().is_empty() || line.trim_start().starts_with("import "))
        .take_while(|line| line.trim_start().starts_with("//"))
        .map(|line| line.trim_start().trim_start_matches("//").strip_prefix(' ').unwrap_or_else(|| line.trim_start().trim_start_matches("//")))
        .collect();
    doc.join("\n")
}

/// Строки `//` прямо над объявлением, начинающимся в `offset`.
fn comment_above(source: &SourceFile, offset: u32) -> String {
    let line = source.location(offset).line;
    let mut lines = Vec::new();
    let mut current = line.saturating_sub(1);
    while current >= 1 {
        let text = source.line_text(current).trim_start();
        let Some(comment) = text.strip_prefix("//") else {
            break;
        };
        lines.push(comment.strip_prefix(' ').unwrap_or(comment).to_string());
        current -= 1;
    }
    lines.reverse();
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{StdKind, std_completions};
    use std::path::PathBuf;

    fn stdlib() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../std")
    }

    fn labels(text: &str) -> Option<Vec<(String, StdKind, bool)>> {
        std_completions(text, text.len() as u32, &stdlib())
            .map(|items| items.into_iter().map(|item| (item.label, item.kind, item.deep)).collect())
    }

    #[test]
    fn std_lists_folders_modules_and_whole_paths() {
        let found = labels("let out = std.").unwrap();
        assert!(found.contains(&("io".into(), StdKind::Folder, false)));
        assert!(found.contains(&("compare".into(), StdKind::Module, false)));
        assert!(found.contains(&("io.stdout".into(), StdKind::Module, true)));
        let found = labels("let out = std.io.").unwrap();
        assert!(found.contains(&("stdout".into(), StdKind::Module, false)));
        assert!(!found.iter().any(|(label, ..)| label == "io"));
    }

    #[test]
    fn module_path_lists_declarations_and_enum_cases() {
        let found = labels("std.io.stdout.").unwrap();
        assert!(found.contains(&("Stdout".into(), StdKind::Object, false)));
        let found = labels("std.compare.Order.").unwrap();
        assert!(found.contains(&("less".into(), StdKind::EnumCase, false)));
        let found = labels("std.runtime.debug.").unwrap();
        assert!(found.contains(&("data_dump".into(), StdKind::Native, false)));
        // С набранной буквой — объявления из глубины std.
        let found = labels("std.Stdo").unwrap();
        assert!(found.contains(&("io.stdout.Stdout".into(), StdKind::Object, true)));
    }

    #[test]
    fn other_paths_are_not_std() {
        assert_eq!(labels("let x = out."), None);
        assert_eq!(labels("let x = self.std."), None);
        assert_eq!(labels("let x = mystd."), None);
    }
}
