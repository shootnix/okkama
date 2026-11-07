//! `blade build` and `blade run`: the `[build]` and `[run]` sections of
//! `blade.toml` and the `okkama` commands they become.
//!
//! ```toml
//! [build]
//! entry = "src/main.ok"      # or a list: the files of the root module
//! target = "sparrow"         # sparrow | jvm
//! output = "target/app.spb"  # default target/<name>.spb or .jar
//!
//! [run]
//! engine = "sparrow"         # sparrow | ir | jvm; default: the build target
//! args = []                  # `blade run -- a b` appends
//! env = {}
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::lock::{LOCK, Lock};
use crate::manifest::{MANIFEST, Manifest, PKG};

/// Backend of `okkama build`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Sparrow,
    Jvm,
}

/// What `blade run` runs the program on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    /// The built `.spb` (`okkama exec`).
    Sparrow,
    /// The sources on the IR interpreter (`okkama run --engine ir`); no build.
    Ir,
    /// The built JAR (`java -jar`).
    Jvm,
}

/// A project with its build and run settings resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub root: PathBuf,
    pub name: String,
    /// Files of the root module, relative to `root`.
    pub entry: Vec<String>,
    pub target: Target,
    /// The built file, relative to `root`.
    pub output: String,
    pub engine: Engine,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

impl Project {
    /// The project at `root` (its `blade.toml`).
    pub fn read(root: &Path) -> Result<Project, String> {
        let file = root.join(MANIFEST);
        let text = std::fs::read_to_string(&file).map_err(|error| format!("cannot read {}: {error}", file.display()))?;
        Project::parse(root, &text).map_err(|error| format!("{}: {error}", file.display()))
    }

    /// Parse `text` as the manifest of the project at `root`.
    pub fn parse(root: &Path, text: &str) -> Result<Project, String> {
        let manifest = Manifest::parse(text)?;
        let table: toml::Table = text.parse().map_err(|error: toml::de::Error| error.message().to_string())?;
        let section = |name: &str, keys: &[&str]| -> Result<toml::Table, String> {
            let Some(value) = table.get(name) else { return Ok(toml::Table::new()) };
            let section = value.as_table().ok_or(format!("[{name}] must be a table"))?.clone();
            if let Some(key) = section.keys().find(|key| !keys.contains(&key.as_str())) {
                return Err(format!("unknown key \"{key}\" in [{name}] (known: {})", keys.join(", ")));
            }
            Ok(section)
        };
        let build = section("build", &["entry", "target", "output"])?;
        let run = section("run", &["engine", "args", "env"])?;
        let name = manifest.name.clone().unwrap_or_else(|| "app".to_string());
        let entry = match build.get("entry") {
            None => vec!["src/main.ok".to_string()],
            Some(toml::Value::String(file)) => vec![file.clone()],
            Some(toml::Value::Array(files)) if !files.is_empty() => strings(files, "[build] entry")?,
            Some(_) => return Err("[build] entry must be a file or a non-empty list of files".into()),
        };
        let target = match text_of(&build, "build", "target")?.as_deref() {
            None | Some("sparrow") => Target::Sparrow,
            Some("jvm") => Target::Jvm,
            Some(other) => return Err(format!("[build] target \"{other}\" is not sparrow or jvm")),
        };
        let output = text_of(&build, "build", "output")?.unwrap_or_else(|| {
            let leaf = name.rsplit('/').next().unwrap_or(&name);
            format!("target/{leaf}.{}", if target == Target::Jvm { "jar" } else { "spb" })
        });
        let engine = match text_of(&run, "run", "engine")?.as_deref() {
            None => match target {
                Target::Sparrow => Engine::Sparrow,
                Target::Jvm => Engine::Jvm,
            },
            Some("sparrow") => Engine::Sparrow,
            Some("ir") => Engine::Ir,
            Some("jvm") => Engine::Jvm,
            Some(other) => return Err(format!("[run] engine \"{other}\" is not sparrow, ir or jvm")),
        };
        match (engine, target) {
            (Engine::Sparrow, Target::Jvm) | (Engine::Jvm, Target::Sparrow) => {
                return Err(format!(
                    "[run] engine \"{}\" needs [build] target \"{}\"",
                    if engine == Engine::Jvm { "jvm" } else { "sparrow" },
                    if engine == Engine::Jvm { "jvm" } else { "sparrow" },
                ));
            }
            _ => {}
        }
        let args = match run.get("args") {
            None => Vec::new(),
            Some(toml::Value::Array(args)) => strings(args, "[run] args")?,
            Some(_) => return Err("[run] args must be a list of strings".into()),
        };
        let env = match run.get("env") {
            None => BTreeMap::new(),
            Some(toml::Value::Table(env)) => env
                .iter()
                .map(|(key, value)| match value {
                    toml::Value::String(value) => Ok((key.clone(), value.clone())),
                    _ => Err(format!("[run] env \"{key}\" must be a string")),
                })
                .collect::<Result<_, _>>()?,
            Some(_) => return Err("[run] env must be a table of strings".into()),
        };
        Ok(Project { root: root.to_path_buf(), name, entry, target, output, engine, args, env })
    }

    fn output_path(&self) -> PathBuf {
        self.root.join(&self.output)
    }
}

fn strings(values: &[toml::Value], what: &str) -> Result<Vec<String>, String> {
    values
        .iter()
        .map(|value| value.as_str().map(str::to_string).ok_or(format!("{what} must be strings")))
        .collect()
}

fn text_of(table: &toml::Table, section: &str, key: &str) -> Result<Option<String>, String> {
    match table.get(key) {
        None => Ok(None),
        Some(toml::Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(format!("[{section}] {key} must be a string")),
    }
}

/// The `okkama` Blade uses: `BLADE_OKKAMA`, the one next to `blade`, or `okkama` on PATH.
pub fn okkama() -> PathBuf {
    if let Some(path) = std::env::var_os("BLADE_OKKAMA") {
        return PathBuf::from(path);
    }
    let name = format!("okkama{}", std::env::consts::EXE_SUFFIX);
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(&name)))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from(name))
}

/// Every declared dependency is installed as `blade.lock` says.
fn check_installed(project: &Project) -> Result<(), String> {
    crate::install::settled(&project.root)?;
    let manifest = Manifest::read(&project.root)?;
    let lock = Lock::read(&project.root)?;
    let missing = manifest
        .dependencies
        .keys()
        .find(|name| !lock.packages.contains_key(*name) || !project.root.join(PKG).join(name).is_dir());
    match missing {
        Some(name) => Err(format!(
            "dependency \"{name}\" is not installed (no {LOCK} entry or {PKG}/{name}); run `blade install`"
        )),
        None => Ok(()),
    }
}

/// Is the output missing or older than a source Blade builds from: an `.ok`
/// file outside the output directory, `blade.toml` or `blade.lock`?
fn sources_changed(project: &Project) -> Result<bool, String> {
    let output = project.output_path();
    let Ok(built) = std::fs::metadata(&output).and_then(|m| m.modified()) else {
        return Ok(true);
    };
    let skip = output.parent().map(Path::to_path_buf);
    let mut stack = vec![project.root.clone()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("{}: {error}", dir.display()))?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|error| format!("{}: {error}", path.display()))?;
            let hidden = entry.file_name().to_string_lossy().starts_with('.');
            if kind.is_dir() {
                if !hidden && Some(&path) != skip.as_ref() {
                    stack.push(path);
                }
                continue;
            }
            let name = entry.file_name();
            let relevant = path.extension().is_some_and(|e| e == "ok") || name == MANIFEST || name == LOCK;
            if relevant && entry.metadata().and_then(|m| m.modified()).is_ok_and(|time| time > built) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Build the project unless the output is newer than its sources (`force`
/// always builds). `Ok(true)` — it was built.
pub fn build(project: &Project, force: bool) -> Result<bool, String> {
    check_installed(project)?;
    for file in &project.entry {
        if !project.root.join(file).is_file() {
            return Err(format!("[build] entry \"{file}\" does not exist"));
        }
    }
    if !force && !sources_changed(project)? {
        return Ok(false);
    }
    let output = project.output_path();
    if let Some(dir) = output.parent() {
        std::fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    }
    let target = match project.target {
        Target::Sparrow => "sparrow",
        Target::Jvm => "jvm",
    };
    let status = Command::new(okkama())
        .current_dir(&project.root)
        .arg("build")
        .args(&project.entry)
        .args(["--target", target, "-o"])
        .arg(&project.output)
        .status()
        .map_err(|error| format!("cannot run {}: {error}", okkama().display()))?;
    if status.success() {
        Ok(true)
    } else {
        Err(format!("build failed ({status})"))
    }
}

/// The command `blade run` executes, after `extra` arguments.
pub fn run_command(project: &Project, extra: &[String]) -> Command {
    let mut command = match project.engine {
        Engine::Sparrow => {
            let mut command = Command::new(okkama());
            command.arg("exec").arg(project.output_path());
            command
        }
        Engine::Ir => {
            let mut command = Command::new(okkama());
            command.args(["run", "--engine", "ir"]).args(&project.entry);
            command
        }
        Engine::Jvm => {
            let java = std::env::var_os("JAVA_HOME")
                .map(|home| PathBuf::from(home).join("bin").join("java"))
                .filter(|java| java.is_file())
                .unwrap_or_else(|| PathBuf::from("java"));
            let mut command = Command::new(java);
            command.arg("-jar").arg(project.output_path());
            command
        }
    };
    command.current_dir(&project.root).args(&project.args).args(extra).envs(&project.env);
    command
}

/// Build when needed (not for `ir`), run, and return the program's exit status.
pub fn run(project: &Project, extra: &[String]) -> Result<i32, String> {
    if project.engine == Engine::Ir {
        check_installed(project)?;
    } else {
        build(project, false)?;
    }
    let mut command = run_command(project, extra);
    let status = command
        .status()
        .map_err(|error| format!("cannot run {}: {error}", command.get_program().to_string_lossy()))?;
    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_the_module_and_the_target() {
        let root = Path::new("/p");
        let project = Project::parse(root, "[module]\nname = \"acme/web\"\n").unwrap();
        assert_eq!(project.entry, ["src/main.ok"]);
        assert_eq!((project.target, project.engine), (Target::Sparrow, Engine::Sparrow));
        assert_eq!(project.output, "target/web.spb");
        assert!(project.args.is_empty() && project.env.is_empty());

        let project = Project::parse(root, "[build]\ntarget = \"jvm\"\n").unwrap();
        assert_eq!((project.output.as_str(), project.engine), ("target/app.jar", Engine::Jvm));
        let project = Project::parse(root, "[build]\ntarget = \"jvm\"\n[run]\nengine = \"ir\"\n").unwrap();
        assert_eq!(project.engine, Engine::Ir);
    }

    #[test]
    fn malformed_settings_are_reported() {
        let root = Path::new("/p");
        for (text, expected) in [
            ("[build]\nentry = []\n", "non-empty list"),
            ("[build]\nentry = 3\n", "[build] entry"),
            ("[build]\ntarget = \"wasm\"\n", "not sparrow or jvm"),
            ("[run]\nargs = \"a\"\n", "[run] args must be a list"),
            ("[run]\nargs = [1]\n", "[run] args must be strings"),
            ("[run]\nenv = { A = 1 }\n", "[run] env \"A\" must be a string"),
            ("[run]\nmode = \"x\"\n", "unknown key \"mode\" in [run]"),
            ("build = 1\n", "[build] must be a table"),
        ] {
            let error = Project::parse(root, text).unwrap_err();
            assert!(error.contains(expected), "{text}: {error}");
        }
    }
}
