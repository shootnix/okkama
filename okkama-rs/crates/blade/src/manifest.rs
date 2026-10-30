//! `blade.toml` — the manifest of a Blade project or package, and the project
//! layout the compiler resolves imports by.
//!
//! ```toml
//! [module]
//! name = "app"
//! version = "1.0.0"
//!
//! [dependencies]
//! web = "12.3.0"
//! json = { git = "https://example.org/json", version = "1.*" }
//! ```
//!
//! A directory with `blade.toml` is a package root. Dependencies live in the
//! `pkg/` directory of the project root, one directory per dependency:
//! `pkg/web/blade.toml`. A package inside `pkg/` belongs to the project whose
//! `pkg/` holds it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// File name of the manifest.
pub const MANIFEST: &str = "blade.toml";

/// Directory of installed dependencies, relative to the project root.
pub const PKG: &str = "pkg";

/// An import path into installed dependencies: `pkg/web/router`.
pub fn is_dependency_path(path: &str) -> bool {
    path.strip_prefix(PKG)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// A parsed `blade.toml`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    /// `[module] name`.
    pub name: Option<String>,
    /// `[module] version`.
    pub version: Option<String>,
    /// `[dependencies]`, by name.
    pub dependencies: BTreeMap<String, Dependency>,
}

/// One entry of `[dependencies]`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dependency {
    /// Version requirement: `"12.3.0"` means compatible with 12.3.0, `"12.*"` any 12.x.
    pub version: Option<String>,
    pub git: Option<String>,
    /// A local package, relative to the manifest.
    pub path: Option<String>,
}

impl Manifest {
    /// Parse the text of a manifest; `Err` is a message without the file name.
    pub fn parse(text: &str) -> Result<Manifest, String> {
        let table: toml::Table = text
            .parse()
            .map_err(|error: toml::de::Error| error.message().to_string())?;
        let mut manifest = Manifest::default();
        if let Some(module) = table.get("module") {
            let module = module.as_table().ok_or("[module] must be a table")?;
            manifest.name = string_field(module, "module", "name")?;
            manifest.version = string_field(module, "module", "version")?;
        }
        if let Some(dependencies) = table.get("dependencies") {
            let dependencies = dependencies
                .as_table()
                .ok_or("[dependencies] must be a table")?;
            for (name, value) in dependencies {
                let dependency = match value {
                    toml::Value::String(version) => Dependency {
                        version: Some(version.clone()),
                        ..Dependency::default()
                    },
                    toml::Value::Table(fields) => {
                        let section = format!("dependencies.{name}");
                        Dependency {
                            version: string_field(fields, &section, "version")?,
                            git: string_field(fields, &section, "git")?,
                            path: string_field(fields, &section, "path")?,
                        }
                    }
                    _ => {
                        return Err(format!(
                            "dependency \"{name}\" must be a version string or a table"
                        ));
                    }
                };
                manifest.dependencies.insert(name.clone(), dependency);
            }
        }
        Ok(manifest)
    }

    /// The manifest of the package rooted at `root`.
    pub fn read(root: &Path) -> Result<Manifest, String> {
        let file = root.join(MANIFEST);
        let text = std::fs::read_to_string(&file)
            .map_err(|error| format!("cannot read {}: {error}", file.display()))?;
        Manifest::parse(&text).map_err(|error| format!("{}: {error}", file.display()))
    }

    /// The dependency an import path `pkg/<name>/…` refers to: the longest
    /// declared name that is a whole-segment prefix of the rest of the path.
    pub fn dependency_of<'a>(&'a self, path: &str) -> Option<&'a str> {
        let rest = path.strip_prefix(PKG)?.strip_prefix('/')?;
        self.dependencies
            .keys()
            .filter(|name| {
                rest.strip_prefix(name.as_str())
                    .is_some_and(|tail| tail.is_empty() || tail.starts_with('/'))
            })
            .max_by_key(|name| name.len())
            .map(String::as_str)
    }
}

/// Add or replace `[dependencies] name` in the manifest of `root`, keeping the
/// rest of the file as written.
pub fn add_dependency(root: &Path, name: &str, dependency: &Dependency) -> Result<(), String> {
    edit(root, |document| {
        if !document.contains_key("dependencies") {
            document.insert("dependencies", toml_edit::table());
        }
        let dependencies = document["dependencies"]
            .as_table_like_mut()
            .ok_or("[dependencies] must be a table")?;
        let mut entry = toml_edit::InlineTable::new();
        for (key, value) in [
            ("git", &dependency.git),
            ("path", &dependency.path),
            ("version", &dependency.version),
        ] {
            if let Some(value) = value {
                entry.insert(key, value.as_str().into());
            }
        }
        dependencies.insert(name, toml_edit::value(entry));
        Ok(())
    })
}

/// Remove `[dependencies] name`; `Ok(false)` — there was no such dependency.
pub fn remove_dependency(root: &Path, name: &str) -> Result<bool, String> {
    let mut removed = false;
    edit(root, |document| {
        if let Some(dependencies) = document
            .get_mut("dependencies")
            .and_then(toml_edit::Item::as_table_like_mut)
        {
            removed = dependencies.remove(name).is_some();
        }
        Ok(())
    })?;
    Ok(removed)
}

fn edit(
    root: &Path,
    change: impl FnOnce(&mut toml_edit::DocumentMut) -> Result<(), String>,
) -> Result<(), String> {
    let file = root.join(MANIFEST);
    let text = std::fs::read_to_string(&file)
        .map_err(|error| format!("cannot read {}: {error}", file.display()))?;
    let mut document: toml_edit::DocumentMut = text
        .parse()
        .map_err(|error| format!("{}: {error}", file.display()))?;
    change(&mut document).map_err(|error| format!("{}: {error}", file.display()))?;
    std::fs::write(&file, document.to_string())
        .map_err(|error| format!("cannot write {}: {error}", file.display()))
}

fn string_field(table: &toml::Table, section: &str, key: &str) -> Result<Option<String>, String> {
    match table.get(key) {
        None => Ok(None),
        Some(toml::Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(format!("[{section}] {key} must be a string")),
    }
}

/// The nearest directory with `blade.toml`, starting from `dir` itself.
pub fn find_root(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|dir| dir.join(MANIFEST).is_file())
        .map(Path::to_path_buf)
}

/// The package root of `file`: the nearest directory above it with `blade.toml`.
pub fn package_root(file: &Path) -> Option<PathBuf> {
    file.ancestors()
        .skip(1)
        .find(|dir| dir.join(MANIFEST).is_file())
        .map(Path::to_path_buf)
}

/// The project a package root belongs to: a package installed in
/// `<project>/pkg/<name>` belongs to `<project>`, otherwise it is the project.
pub fn project_root(package: &Path) -> PathBuf {
    let mut root = package.to_path_buf();
    loop {
        let Some(pkg) = root.parent() else {
            return root;
        };
        let Some(project) = pkg.parent() else {
            return root;
        };
        if pkg.file_name().is_some_and(|name| name == PKG) && project.join(MANIFEST).is_file() {
            root = project.to_path_buf();
        } else {
            return root;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_and_table_dependencies_parse() {
        let manifest = Manifest::parse(
            r#"
[module]
name = "app"
version = "1.0.0"

[build]
target = "sparrow"

[dependencies]
web = "12.3.0"
json = { git = "https://example.org/json", version = "1.*" }

[dependencies.local]
path = "../local"
"#,
        )
        .unwrap();
        assert_eq!(manifest.name.as_deref(), Some("app"));
        assert_eq!(manifest.version.as_deref(), Some("1.0.0"));
        assert_eq!(
            manifest.dependencies["web"].version.as_deref(),
            Some("12.3.0")
        );
        assert_eq!(
            manifest.dependencies["json"].git.as_deref(),
            Some("https://example.org/json")
        );
        assert_eq!(
            manifest.dependencies["local"].path.as_deref(),
            Some("../local")
        );
    }

    #[test]
    fn malformed_manifests_are_reported() {
        assert!(Manifest::parse("[dependencies]\nweb = 12").is_err());
        assert!(Manifest::parse("[module]\nname = 1").is_err());
        assert!(Manifest::parse("[dependencies\n").is_err());
    }

    #[test]
    fn imports_resolve_to_whole_segment_dependencies() {
        let manifest =
            Manifest::parse("[dependencies]\nweb = \"1.0.0\"\n\"web/extra\" = \"1.0.0\"\n")
                .unwrap();
        assert_eq!(manifest.dependency_of("pkg/web/router"), Some("web"));
        assert_eq!(manifest.dependency_of("pkg/web"), Some("web"));
        assert_eq!(manifest.dependency_of("pkg/web/extra/x"), Some("web/extra"));
        assert_eq!(manifest.dependency_of("pkg/webby/x"), None);
        assert_eq!(manifest.dependency_of("web/router"), None);
    }

    #[test]
    fn dependencies_are_edited_in_place() {
        let root = std::env::temp_dir().join(format!("blade-edit-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(MANIFEST),
            "# the app\n[module]\nname = \"app\" # name\n",
        )
        .unwrap();
        let web = Dependency {
            version: Some("12.3.1".to_string()),
            path: Some("/home/user/pkg/web".to_string()),
            git: None,
        };
        add_dependency(&root, "web", &web).unwrap();
        let text = std::fs::read_to_string(root.join(MANIFEST)).unwrap();
        assert!(
            text.starts_with("# the app\n[module]\nname = \"app\" # name\n"),
            "{text}"
        );
        assert_eq!(Manifest::read(&root).unwrap().dependencies["web"], web);
        assert!(remove_dependency(&root, "web").unwrap());
        assert!(!remove_dependency(&root, "web").unwrap());
        assert!(Manifest::read(&root).unwrap().dependencies.is_empty());
        assert_eq!(find_root(&root.join("src")), Some(root.clone()));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn installed_packages_belong_to_the_project_holding_them() {
        let base = std::env::temp_dir().join(format!("blade-layout-{}", std::process::id()));
        let web = base.join("pkg").join("web");
        let json = web.join("pkg").join("json");
        std::fs::create_dir_all(&json).unwrap();
        std::fs::create_dir_all(base.join("src")).unwrap();
        for dir in [&base, &web] {
            std::fs::write(dir.join(MANIFEST), "").unwrap();
        }
        assert_eq!(
            package_root(&base.join("src").join("main.ok")),
            Some(base.clone())
        );
        assert_eq!(package_root(&web.join("router.ok")), Some(web.clone()));
        assert_eq!(project_root(&web), base);
        assert_eq!(project_root(&base), base);
        // Без собственного blade.toml каталог внутри pkg/ пакетом не считается.
        assert_eq!(package_root(&json.join("x.ok")), Some(web.clone()));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
