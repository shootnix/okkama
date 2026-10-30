//! Подсказки путей зависимостей в `import "pkg/…"` (spec §5.2.2): `pkg/`,
//! прямые зависимости пакета файла из его `blade.toml`, затем каталоги и
//! модули установленного пакета в `pkg/` проекта.

use std::path::Path;

use blade::manifest::{self, Manifest};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PathKind {
    /// `pkg/` или каталог внутри пакета: после выбора — следующий уровень.
    Folder,
    Dependency,
    Module,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PathCompletion {
    pub label: String,
    pub kind: PathKind,
    pub detail: String,
    pub insert: String,
}

/// Подсказки, если курсор `offset` стоит в строке пути `import "…"` файла
/// `file` из blade-проекта.
pub(crate) fn pkg_completions(text: &str, offset: u32, file: &Path) -> Option<Vec<PathCompletion>> {
    let typed = import_path_at(text, offset as usize)?;
    let package = manifest::package_root(file)?;
    if !typed.contains('/') {
        return manifest::PKG.starts_with(typed).then(|| {
            vec![PathCompletion {
                label: format!("{}/", manifest::PKG),
                kind: PathKind::Folder,
                detail: "dependencies installed by blade".to_string(),
                insert: format!("{}/", manifest::PKG),
            }]
        });
    }
    let rest = typed.strip_prefix(manifest::PKG)?.strip_prefix('/')?;
    let found = Manifest::read(&package).ok()?;
    let installed = manifest::project_root(&package).join(manifest::PKG);
    let Some((dir, _)) = rest.rsplit_once('/') else {
        // `pkg/` — прямые зависимости этого пакета: только их и можно импортировать.
        return Some(
            found
                .dependencies
                .iter()
                .map(|(name, dependency)| {
                    let source = dependency
                        .git
                        .as_deref()
                        .map(|git| format!("git {git}"))
                        .or_else(|| {
                            dependency
                                .path
                                .as_deref()
                                .map(|path| format!("path {path}"))
                        })
                        .unwrap_or_default();
                    let version = dependency.version.as_deref().unwrap_or("*");
                    let state = if installed.join(name).is_dir() {
                        String::new()
                    } else {
                        " — not installed, run blade install".to_string()
                    };
                    PathCompletion {
                        label: name.clone(),
                        kind: PathKind::Dependency,
                        detail: format!("{version} · {source}{state}"),
                        insert: format!("{name}/"),
                    }
                })
                .collect(),
        );
    };
    found.dependency_of(&format!("{}/{dir}", manifest::PKG))?;
    let mut out: Vec<PathCompletion> = std::fs::read_dir(installed.join(dir))
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }
            let path = entry.path();
            if path.is_dir() {
                Some(PathCompletion {
                    label: format!("{name}/"),
                    kind: PathKind::Folder,
                    detail: String::new(),
                    insert: format!("{name}/"),
                })
            } else {
                let module = name.strip_suffix(".ok")?;
                Some(PathCompletion {
                    label: module.to_string(),
                    kind: PathKind::Module,
                    detail: declared_module(&path).unwrap_or_default(),
                    insert: module.to_string(),
                })
            }
        })
        .collect();
    out.sort_by(|a, b| a.label.cmp(&b.label));
    Some(out)
}

/// Набранный путь, если курсор внутри строки `import "…` (до закрывающей кавычки).
fn import_path_at(text: &str, offset: usize) -> Option<&str> {
    let before = text.get(..offset)?;
    let line = &before[before.rfind('\n').map_or(0, |at| at + 1)..];
    let rest = line.trim_start().strip_prefix("import")?;
    let typed = rest.trim_start().strip_prefix('"')?;
    (!typed.contains('"')).then_some(typed)
}

/// `module web/router;` — первая строка-объявление файла.
fn declared_module(file: &Path) -> Option<String> {
    let text = std::fs::read_to_string(file).ok()?;
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("module "))?;
    Some(line.trim_end_matches(';').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(found: Option<Vec<PathCompletion>>) -> Vec<String> {
        found
            .unwrap_or_default()
            .into_iter()
            .map(|found| found.label)
            .collect()
    }

    #[test]
    fn import_strings_complete_pkg_dependencies_and_their_modules() {
        let root = std::env::temp_dir().join(format!("okkama-pkg-paths-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let write = |path: &str, text: &str| {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "blade.toml",
            "[dependencies]\nweb = { git = \"https://x/web\", version = \"12.3.0\" }\njson = { path = \"../json\", version = \"1.*\" }\n",
        );
        write("pkg/web/blade.toml", "[module]\nname = \"web\"\n");
        write("pkg/web/router.ok", "module web/router;\n");
        write("pkg/web/http/client.ok", "module web/http/client;\n");
        write("pkg/web/README.md", "");
        let main = root.join("src/main.ok");
        let at = |line: &str| {
            let text = format!("module app;\n{line}");
            pkg_completions(&text, text.len() as u32, &main)
        };
        assert_eq!(labels(at("import \"")), ["pkg/"]);
        assert_eq!(labels(at("import \"pk")), ["pkg/"]);
        assert_eq!(labels(at("import \"io")), Vec::<String>::new());
        let deps = at("import \"pkg/").unwrap();
        assert_eq!(
            deps.iter().map(|d| d.label.as_str()).collect::<Vec<_>>(),
            ["json", "web"]
        );
        assert_eq!(
            deps[0].detail,
            "1.* · path ../json — not installed, run blade install"
        );
        assert_eq!(deps[1].detail, "12.3.0 · git https://x/web");
        assert_eq!(deps[1].insert, "web/");
        let modules = at("import \"pkg/web/").unwrap();
        assert_eq!(
            modules.iter().map(|m| m.label.as_str()).collect::<Vec<_>>(),
            ["http/", "router"]
        );
        assert_eq!(modules[1].detail, "module web/router");
        assert_eq!(labels(at("import \"pkg/web/http/")), ["client"]);
        // Не зависимость, закрытая строка, не импорт, файл вне проекта.
        assert!(at("import \"pkg/other/").is_none());
        assert!(at("import \"pkg/web/\" take ").is_none());
        assert!(at("let x = \"pkg/").is_none());
        assert!(pkg_completions("import \"", 8, &std::env::temp_dir().join("loose.ok")).is_none());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
