//! Installing the dependencies of a project into its `pkg/`.
//!
//! A dependency comes from a git repository (versions are its `vX.Y.Z` tags)
//! or from a local directory (its version is the one in its `blade.toml`).
//! Resolution keeps one version of each package per project: requirements
//! of the same major meet at the highest version that satisfies all of them;
//! requirements no version satisfies are an error naming who wants what.
//!
//! Installation is all or nothing: the new `pkg/` is assembled next to the
//! old one and swapped in, then `blade.lock` records what was installed.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

use okkama_compiler::source::SourceFile;
use okkama_compiler::syntax::{self, ParseOptions, ast};
use sha2::{Digest, Sha256};

use crate::lock::{Lock, Locked};
use crate::manifest::{self, Dependency, MANIFEST, Manifest, PKG};
use crate::version::{Requirement, Version};

/// Where a package comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Git(String),
    /// A canonical local directory.
    Path(PathBuf),
}

impl Source {
    /// `git+<url>` / `path+<dir>`: the source as `blade.lock` records it.
    pub fn id(&self) -> String {
        match self {
            Source::Git(url) => format!("git+{url}"),
            Source::Path(dir) => format!("path+{}", dir.display()),
        }
    }

    /// The source of dependency `name` declared in a manifest located in
    /// `base` (`None` — a git checkout, where local paths mean nothing).
    fn of(name: &str, dependency: &Dependency, base: Option<&Path>) -> Result<Source, String> {
        match (&dependency.git, &dependency.path) {
            (Some(url), None) => Ok(Source::Git(url.clone())),
            (None, Some(path)) => {
                let Some(base) = base else {
                    return Err(format!(
                        "dependency \"{name}\" of a git package uses a local path \"{path}\""
                    ));
                };
                let dir = base.join(path);
                std::fs::canonicalize(&dir)
                    .map(Source::Path)
                    .map_err(|error| format!("dependency \"{name}\": {}: {error}", dir.display()))
            }
            (Some(_), Some(_)) => Err(format!(
                "dependency \"{name}\" has both git and path; use one"
            )),
            (None, None) => Err(format!(
                "dependency \"{name}\" has no source: add git = \"…\" or path = \"…\""
            )),
        }
    }
}

/// Which locked versions an install may change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unlock {
    /// Keep every locked version that still satisfies the manifests.
    Nothing,
    /// `blade update`: pick the newest versions.
    All,
    /// `blade update web`: only these packages.
    Packages(BTreeSet<String>),
}

impl Unlock {
    fn allows(&self, name: &str) -> bool {
        match self {
            Unlock::Nothing => false,
            Unlock::All => true,
            Unlock::Packages(names) => names.contains(name),
        }
    }
}

/// What an install changed, line by line: `+ web 12.3.1`, `~ web 12.3.0 -> 12.3.1`, `- json 1.0.0`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub lines: Vec<String>,
}

/// Install the dependencies of the project at `root`.
pub fn install(root: &Path, unlock: &Unlock) -> Result<Report, String> {
    let _busy = Busy::take(root)?;
    let manifest = Manifest::read(root)?;
    let lock = Lock::read(root)?;
    let mut work = Work::new()?;
    let chosen = resolve(root, &manifest, &lock, unlock, &mut work)?;
    let new_lock = Lock {
        packages: chosen
            .iter()
            .map(|(name, package)| (name.clone(), package.locked()))
            .collect(),
    };
    swap_in(root, &chosen)?;
    new_lock.write(root)?;
    Ok(report(&lock, &new_lock))
}

/// `blade install <source> [version]`: add the package at `source` to
/// `[dependencies]` (by the name in its `blade.toml`), then install.
pub fn add(
    root: &Path,
    source: &str,
    version: Option<&str>,
    git: bool,
) -> Result<(String, Report), String> {
    let mut work = Work::new()?;
    let (source, spelled) = if git || looks_like_git(source) {
        (Source::Git(source.to_string()), source.to_string())
    } else {
        let dir = std::fs::canonicalize(source).map_err(|error| format!("{source}: {error}"))?;
        // Внутри проекта путь пишется относительно его корня.
        let spelled = match dir.strip_prefix(std::fs::canonicalize(root).unwrap_or(root.into())) {
            Ok(relative) => relative.display().to_string(),
            Err(_) => dir.display().to_string(),
        };
        (Source::Path(dir), spelled)
    };
    let requirement = version.map(Requirement::parse).transpose()?;
    let available = work.versions(&source)?;
    let picked = available
        .keys()
        .rev()
        .find(|found| {
            requirement
                .as_ref()
                .is_none_or(|wanted| wanted.matches(found))
        })
        .copied()
        .ok_or_else(|| match &requirement {
            Some(wanted) => format!(
                "{} has no version matching {wanted}; available: {}{}",
                source.id(),
                list(available.keys()),
                match &source {
                    Source::Path(dir) if dir.join(".git").exists() => format!(
                        " (the version of its {MANIFEST}; it is a git repository: \
                         `blade install --git {}` installs its vX.Y.Z tags)",
                        dir.display()
                    ),
                    _ => String::new(),
                }
            ),
            None => format!("{} has no versions (no vX.Y.Z tags)", source.id()),
        })?;
    let fetched = work.fetch(&source, &picked, None)?;
    let name = fetched
        .manifest
        .name
        .clone()
        .ok_or_else(|| format!("{}: {MANIFEST} has no [module] name", source.id()))?;
    let dependency = Dependency {
        version: Some(version.map_or_else(|| picked.to_string(), str::to_string)),
        git: matches!(source, Source::Git(_)).then(|| spelled.clone()),
        path: matches!(source, Source::Path(_)).then_some(spelled),
    };
    let file = root.join(MANIFEST);
    let before = std::fs::read(&file).map_err(|error| format!("{}: {error}", file.display()))?;
    manifest::add_dependency(root, &name, &dependency)?;
    match install(root, &Unlock::Packages(BTreeSet::from([name.clone()]))) {
        Ok(report) => Ok((name, report)),
        Err(error) => {
            // Неудачная установка не оставляет зависимость в манифесте.
            let _ = std::fs::write(&file, before);
            Err(error)
        }
    }
}

fn looks_like_git(source: &str) -> bool {
    source.contains("://") || source.starts_with("git@") || source.ends_with(".git")
}

fn list<'a>(versions: impl Iterator<Item = &'a Version>) -> String {
    let all: Vec<String> = versions.map(Version::to_string).collect();
    if all.is_empty() {
        "none".to_string()
    } else {
        all.join(", ")
    }
}

// ----- разрешение версий -----

/// A fetched package version.
#[derive(Clone, Debug)]
struct Package {
    version: Version,
    source: Source,
    commit: Option<String>,
    /// Checked files, ready to be copied into `pkg/<name>`.
    dir: PathBuf,
    manifest: Manifest,
    checksum: String,
}

impl Package {
    fn locked(&self) -> Locked {
        Locked {
            version: self.version,
            source: self.source.id(),
            commit: self.commit.clone(),
            checksum: self.checksum.clone(),
            dependencies: self.manifest.dependencies.keys().cloned().collect(),
        }
    }

    /// The directory its path dependencies are relative to.
    fn base(&self) -> Option<&Path> {
        match &self.source {
            Source::Path(dir) => Some(dir),
            Source::Git(_) => None,
        }
    }
}

/// One requirement on a package, with who made it.
struct Want {
    source: Source,
    requirement: Requirement,
    by: String,
}

fn resolve(
    root: &Path,
    manifest: &Manifest,
    lock: &Lock,
    unlock: &Unlock,
    work: &mut Work,
) -> Result<BTreeMap<String, Package>, String> {
    let project = manifest
        .name
        .clone()
        .unwrap_or_else(|| "the project".to_string());
    let mut chosen: BTreeMap<String, Package> = BTreeMap::new();
    for _ in 0..64 {
        // Требования от проекта и от уже выбранных версий, достижимых из него.
        let mut wants: BTreeMap<String, Vec<Want>> = BTreeMap::new();
        let mut queue = vec![(project.clone(), manifest.clone(), Some(root.to_path_buf()))];
        let mut seen = BTreeSet::new();
        while let Some((by, declaring, base)) = queue.pop() {
            for (name, dependency) in &declaring.dependencies {
                let requirement = Requirement::parse(dependency.version.as_deref().unwrap_or("*"))
                    .map_err(|error| format!("dependency \"{name}\" of {by}: {error}"))?;
                let source = Source::of(name, dependency, base.as_deref())
                    .map_err(|error| format!("{by}: {error}"))?;
                wants.entry(name.clone()).or_default().push(Want {
                    source,
                    requirement,
                    by: by.clone(),
                });
                if let Some(package) = chosen.get(name) {
                    if seen.insert(name.clone()) {
                        queue.push((
                            format!("{name} {}", package.version),
                            package.manifest.clone(),
                            package.base().map(Path::to_path_buf),
                        ));
                    }
                }
            }
        }
        let mut changed = false;
        chosen.retain(|name, _| {
            let keep = wants.contains_key(name);
            changed |= !keep;
            keep
        });
        for (name, wanted) in &wants {
            let source = &wanted[0].source;
            if let Some(other) = wanted.iter().find(|want| want.source != *source) {
                return Err(format!(
                    "\"{name}\" comes from two sources: {} (by {}) and {} (by {})",
                    source.id(),
                    wanted[0].by,
                    other.source.id(),
                    other.by
                ));
            }
            let available = work.versions(source)?;
            let fits =
                |version: &Version| wanted.iter().all(|want| want.requirement.matches(version));
            let locked = lock
                .packages
                .get(name)
                .filter(|locked| locked.source == source.id() && !unlock.allows(name))
                .map(|locked| locked.version)
                .filter(|version| fits(version) && available.contains_key(version));
            let Some(version) =
                locked.or_else(|| available.keys().rev().copied().find(|v| fits(v)))
            else {
                let who: Vec<String> = wanted
                    .iter()
                    .map(|want| format!("{} (by {})", want.requirement, want.by))
                    .collect();
                return Err(format!(
                    "no version of \"{name}\" satisfies {}; available: {}",
                    who.join(", "),
                    list(available.keys())
                ));
            };
            if chosen
                .get(name)
                .is_some_and(|package| package.version == version && package.source == *source)
            {
                continue;
            }
            // `blade update` переписывает запись lock, а не сверяется с ней.
            let expected = lock.packages.get(name).filter(|locked| {
                locked.source == source.id() && locked.version == version && !unlock.allows(name)
            });
            let package = work.fetch(source, &version, expected)?;
            check_name(name, &package)?;
            chosen.insert(name.clone(), package);
            changed = true;
        }
        if !changed {
            return Ok(chosen);
        }
    }
    Err("dependency versions do not settle; check the requirements for cycles".to_string())
}

fn check_name(name: &str, package: &Package) -> Result<(), String> {
    match &package.manifest.name {
        Some(declared) if declared == name => Ok(()),
        Some(declared) => Err(format!(
            "dependency \"{name}\" is the package \"{declared}\" ({}); the name in [dependencies] must match",
            package.source.id()
        )),
        None => Err(format!(
            "{}: {MANIFEST} has no [module] name",
            package.source.id()
        )),
    }
}

// ----- загрузка -----

/// Scratch space of one install: clones and fetched package versions.
struct Work {
    dir: PathBuf,
    clones: HashMap<String, PathBuf>,
    tags: HashMap<String, BTreeMap<Version, String>>,
    fetched: HashMap<(String, Version), Package>,
    count: usize,
}

impl Work {
    fn new() -> Result<Work, String> {
        // Счётчик различает установки одного процесса, время — разные запуски.
        static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let count = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |time| time.as_nanos());
        let dir =
            std::env::temp_dir().join(format!("blade-{}-{stamp}-{count}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        Ok(Work {
            dir,
            clones: HashMap::new(),
            tags: HashMap::new(),
            fetched: HashMap::new(),
            count: 0,
        })
    }

    /// Versions of a source; for git — with the commit of each tag.
    fn versions(&mut self, source: &Source) -> Result<BTreeMap<Version, Option<String>>, String> {
        match source {
            Source::Path(dir) => {
                let manifest = Manifest::read(dir)?;
                let version = manifest
                    .version
                    .as_deref()
                    .and_then(Version::parse)
                    .ok_or_else(|| {
                        format!(
                            "{}: [module] version must be like 1.2.3",
                            dir.join(MANIFEST).display()
                        )
                    })?;
                Ok(BTreeMap::from([(version, None)]))
            }
            Source::Git(url) => {
                if !self.tags.contains_key(url) {
                    let tags = remote_tags(url)?;
                    self.tags.insert(url.clone(), tags);
                }
                Ok(self.tags[url]
                    .iter()
                    .map(|(version, commit)| (*version, Some(commit.clone())))
                    .collect())
            }
        }
    }

    /// Copy version `version` of `source` into the scratch space and check it.
    /// `expected` — what `blade.lock` says this version is.
    fn fetch(
        &mut self,
        source: &Source,
        version: &Version,
        expected: Option<&Locked>,
    ) -> Result<Package, String> {
        let key = (source.id(), *version);
        if let Some(package) = self.fetched.get(&key) {
            return Ok(package.clone());
        }
        self.count += 1;
        let dir = self.dir.join(format!("package-{}", self.count));
        let commit = match source {
            Source::Path(origin) => {
                copy_package(origin, &dir)?;
                None
            }
            Source::Git(url) => {
                let commit = self.versions(source)?[version].clone().unwrap_or_default();
                if let Some(locked) = expected.and_then(|locked| locked.commit.as_ref()) {
                    if *locked != commit {
                        return Err(format!(
                            "tag v{version} of {url} now points at {commit}, but {} has {locked}; \
                             the tag was moved — check the package, then run `blade update`",
                            crate::lock::LOCK
                        ));
                    }
                }
                let clone = self.clone_of(url)?;
                git(&[
                    "-C",
                    path_str(&clone)?,
                    "checkout",
                    "--quiet",
                    "--force",
                    &commit,
                ])?;
                copy_package(&clone, &dir)?;
                Some(commit)
            }
        };
        let manifest = Manifest::read(&dir).map_err(|error| format!("{}: {error}", source.id()))?;
        let declared = manifest.version.as_deref().and_then(Version::parse);
        if declared != Some(*version) {
            return Err(format!(
                "{} v{version}: {MANIFEST} declares version {}",
                source.id(),
                manifest.version.as_deref().unwrap_or("none")
            ));
        }
        let name = manifest
            .name
            .clone()
            .ok_or_else(|| format!("{}: {MANIFEST} has no [module] name", source.id()))?;
        check_package(&dir, &name)
            .map_err(|error| format!("{name} {version} ({}): {error}", source.id()))?;
        let checksum = checksum(&dir)?;
        if let (Some(locked), Source::Git(_)) = (expected, source) {
            if !locked.checksum.is_empty() && locked.checksum != checksum {
                return Err(format!(
                    "{name} {version}: the files differ from {} ({} != {})",
                    crate::lock::LOCK,
                    checksum,
                    locked.checksum
                ));
            }
        }
        let package = Package {
            version: *version,
            source: source.clone(),
            commit,
            dir,
            manifest,
            checksum,
        };
        self.fetched.insert(key, package.clone());
        Ok(package)
    }

    fn clone_of(&mut self, url: &str) -> Result<PathBuf, String> {
        if let Some(clone) = self.clones.get(url) {
            return Ok(clone.clone());
        }
        let clone = self.dir.join(format!("clone-{}", self.clones.len() + 1));
        git(&["clone", "--quiet", "--no-checkout", url, path_str(&clone)?])?;
        self.clones.insert(url.to_string(), clone.clone());
        Ok(clone)
    }
}

impl Drop for Work {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn path_str(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("{} is not a UTF-8 path", path.display()))
}

fn git(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// `vX.Y.Z` tags of a repository and their commits (annotated tags peeled).
fn remote_tags(url: &str) -> Result<BTreeMap<Version, String>, String> {
    let mut tags = BTreeMap::new();
    let mut peeled = BTreeMap::new();
    for line in git(&["ls-remote", "--tags", url])?.lines() {
        let Some((commit, reference)) = line.split_once('\t') else {
            continue;
        };
        let Some(tag) = reference.strip_prefix("refs/tags/") else {
            continue;
        };
        let (tag, target) = match tag.strip_suffix("^{}") {
            Some(tag) => (tag, &mut peeled),
            None => (tag, &mut tags),
        };
        if tag.starts_with('v') {
            if let Some(version) = Version::parse(tag) {
                target.insert(version, commit.to_string());
            }
        }
    }
    tags.extend(peeled);
    Ok(tags)
}

// ----- проверка и копирование -----

/// Copy a package tree without `.git`, its own `pkg/` and blade scratch
/// directories. Links are refused: a package is plain files.
fn copy_package(from: &Path, to: &Path) -> Result<(), String> {
    copy_tree(from, to, true)
}

fn copy_tree(from: &Path, to: &Path, top: bool) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|error| format!("{}: {error}", to.display()))?;
    let entries =
        std::fs::read_dir(from).map_err(|error| format!("{}: {error}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", from.display()))?;
        let name = entry.file_name();
        let text = name.to_string_lossy();
        if text == ".git" || (top && (text == PKG || text.starts_with("pkg.blade-"))) {
            continue;
        }
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", entry.path().display()))?;
        let target = to.join(&name);
        if kind.is_symlink() {
            return Err(format!(
                "{} is a symbolic link; a package must contain plain files only",
                entry.path().display()
            ));
        } else if kind.is_dir() {
            copy_tree(&entry.path(), &target, false)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &target)
                .map_err(|error| format!("{}: {error}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// Relative paths of the files of a tree, sorted, with `/` separators.
fn files(dir: &Path) -> Result<Vec<String>, String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
        let entries =
            std::fs::read_dir(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        for entry in entries {
            let path = entry
                .map_err(|error| format!("{}: {error}", dir.display()))?
                .path();
            if path.is_dir() {
                walk(root, &path, out)?;
            } else {
                let relative = path.strip_prefix(root).unwrap_or(&path);
                let parts: Vec<String> = relative
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect();
                out.push(parts.join("/"));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out)?;
    out.sort();
    Ok(out)
}

/// The rules a package must follow to be installed.
fn check_package(dir: &Path, name: &str) -> Result<(), String> {
    let all = files(dir)?;
    // macOS и Windows не различают регистр: такие два файла станут одним.
    let mut folded: HashMap<String, &String> = HashMap::new();
    for file in &all {
        if let Some(other) = folded.insert(file.to_lowercase(), file) {
            return Err(format!("{other} and {file} differ only in letter case"));
        }
    }
    for file in all.iter().filter(|file| file.ends_with(".ok")) {
        let text =
            std::fs::read_to_string(dir.join(file)).map_err(|error| format!("{file}: {error}"))?;
        let source =
            SourceFile::new(file.clone(), text).map_err(|error| format!("{file}: {error}"))?;
        let program = syntax::parse(&source, &ParseOptions::default())
            .map_err(|_| format!("{file} has syntax errors"))?;
        let module: Vec<&str> = program
            .module
            .path
            .iter()
            .map(|part| part.text.as_str())
            .collect();
        let module = module.join("/");
        if let Some(version) = &program.module.version {
            return Err(format!(
                "{file} declares module version {}; a package keeps its version in {MANIFEST}",
                version.text()
            ));
        }
        if module != name && !module.starts_with(&format!("{name}/")) {
            return Err(format!(
                "{file} declares module \"{module}\"; modules of package \"{name}\" are named \"{name}\" or \"{name}/…\""
            ));
        }
        if program.items.iter().any(|item| {
            matches!(
                item,
                ast::Item::NativeAction(_) | ast::Item::NativeObject(_)
            )
        }) {
            return Err(format!(
                "{file} declares a native; natives are provided by the Okkama runtime and cannot come from a package"
            ));
        }
    }
    Ok(())
}

fn checksum(dir: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    for file in files(dir)? {
        let bytes = std::fs::read(dir.join(&file)).map_err(|error| format!("{file}: {error}"))?;
        hasher.update(file.as_bytes());
        hasher.update([0]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(format!("sha256:{hex}"))
}

// ----- установка -----

/// Assemble the new `pkg/` beside the old one and swap it in.
fn swap_in(root: &Path, chosen: &BTreeMap<String, Package>) -> Result<(), String> {
    let staged = root.join("pkg.blade-new");
    let old = root.join("pkg.blade-old");
    let pkg = root.join(PKG);
    let io = |path: &Path| {
        let path = path.display().to_string();
        move |error: std::io::Error| format!("{path}: {error}")
    };
    for dir in [&staged, &old] {
        if dir.exists() {
            std::fs::remove_dir_all(dir).map_err(io(dir))?;
        }
    }
    std::fs::create_dir_all(&staged).map_err(io(&staged))?;
    for (name, package) in chosen {
        copy_tree(&package.dir, &staged.join(name), false)?;
    }
    if pkg.exists() {
        std::fs::rename(&pkg, &old).map_err(io(&pkg))?;
    }
    if let Err(error) = std::fs::rename(&staged, &pkg) {
        let _ = std::fs::rename(&old, &pkg);
        return Err(io(&staged)(error));
    }
    if old.exists() {
        std::fs::remove_dir_all(&old).map_err(io(&old))?;
    }
    Ok(())
}

/// Only one blade works on a project at a time.
struct Busy(PathBuf);

impl Busy {
    fn take(root: &Path) -> Result<Busy, String> {
        let file = root.join("pkg.blade-busy");
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file)
            .map(|_| Busy(file.clone()))
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::AlreadyExists => format!(
                    "another blade is working on this project ({} exists; remove it if no blade is running)",
                    file.display()
                ),
                _ => format!("{}: {error}", file.display()),
            })
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn report(old: &Lock, new: &Lock) -> Report {
    let mut lines = Vec::new();
    for (name, package) in &new.packages {
        match old.packages.get(name) {
            None => lines.push(format!("+ {name} {}", package.version)),
            Some(before)
                if before.version != package.version || before.source != package.source =>
            {
                lines.push(format!(
                    "~ {name} {} -> {}",
                    before.version, package.version
                ));
            }
            Some(before) if before.commit != package.commit => {
                let short = |commit: &Option<String>| {
                    commit
                        .as_deref()
                        .unwrap_or("-")
                        .chars()
                        .take(10)
                        .collect::<String>()
                };
                lines.push(format!(
                    "~ {name} {} (commit {} -> {})",
                    package.version,
                    short(&before.commit),
                    short(&package.commit)
                ));
            }
            Some(_) => {}
        }
    }
    for (name, package) in &old.packages {
        if !new.packages.contains_key(name) {
            lines.push(format!("- {name} {}", package.version));
        }
    }
    Report { lines }
}
