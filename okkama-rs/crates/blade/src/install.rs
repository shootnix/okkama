//! Installing the dependencies of a project into its `pkg/`.
//!
//! A dependency comes from a git repository (versions are its `vX.Y.Z` tags)
//! or from a local directory (its version is the one in its `blade.toml`).
//! Resolution keeps one version of each package per project: the lowest
//! version that meets every minimum required of it, within one major
//! (`resolve`, #347); requirements no version meets, two majors or two
//! sources of one package are errors naming the chains that asked.
//!
//! A package is a repository with its modules in `src/` (#348). Its name is
//! the key it is declared under and its version is the tag; `blade.toml` is
//! needed only for dependencies, and a `[module]` in it must agree with both.
//! Only `src/**/*.ok` is checked; a package without `blade.toml` is installed
//! with a generated one, so the compiler finds where it belongs.
//!
//! Installation is all or nothing: the new `pkg/` is assembled next to the
//! old one and swapped in, then `blade.lock` records what was installed.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

use okkama_compiler::source::SourceFile;
use okkama_compiler::syntax::{self, ParseOptions, ast};
use sha2::{Digest, Sha256};

use crate::lock::{LOCK, Lock, Locked};
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

    /// The source `blade.lock` records as `id`; a git URL only if it is one
    /// git may be given (#346).
    fn from_id(id: &str) -> Option<Source> {
        if let Some(url) = id.strip_prefix("git+") {
            check_git_url(url).ok()?;
            Some(Source::Git(url.to_string()))
        } else {
            id.strip_prefix("path+")
                .map(|dir| Source::Path(PathBuf::from(dir)))
        }
    }

    /// The source of dependency `name` declared in a manifest located in
    /// `base` (`None` — a git checkout, where local paths mean nothing).
    fn of(name: &str, dependency: &Dependency, base: Option<&Path>) -> Result<Source, String> {
        match (&dependency.git, &dependency.path) {
            (Some(url), None) => {
                check_git_url(url).map_err(|error| format!("dependency \"{name}\": {error}"))?;
                Ok(Source::Git(url.clone()))
            }
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
    /// `blade update`: resolve again without the locked versions.
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
    install_busy(root, unlock)
}

/// `install` while this blade holds the project.
fn install_busy(root: &Path, unlock: &Unlock) -> Result<Report, String> {
    recover(root)?;
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
    commit(root, &chosen, &lock, &new_lock, false, &mut work)?;
    Ok(report(&lock, &new_lock))
}

/// `blade sync` (#349): install exactly what `blade.lock` records.
///
/// A package already in `pkg/` with its locked checksum stays as it is, so a
/// project whose `pkg/` matches the lock syncs without the network. Any other
/// is fetched by its locked commit, whatever its tags say now, and must have
/// its locked checksum; a local `path` package is copied again. Packages the
/// lock does not have leave `pkg/`. Without `blade.lock` this is the first
/// install: resolve, install, write the lock.
///
/// A lock that does not match the manifests changes nothing: the error says
/// what disagrees and what to run. A sync never picks a version.
pub fn sync(root: &Path) -> Result<Report, String> {
    let _busy = Busy::take(root)?;
    recover(root)?;
    if !root.join(LOCK).exists() {
        return install_busy(root, &Unlock::Nothing);
    }
    let manifest = Manifest::read(root)?;
    let lock = Lock::read(root)?;
    let roots = wants_of(&manifest, Some(root), MANIFEST)?;

    // What the lock reaches from the project by its own lists: no network.
    let mut lines = Vec::new();
    disagreements(&roots, &lock, &mut lines);
    let mut reached = BTreeSet::new();
    let mut queue: Vec<&String> = roots.iter().map(|want| &want.name).collect();
    while let Some(name) = queue.pop() {
        if let Some(locked) = lock.packages.get(name)
            && reached.insert(name.clone())
        {
            queue.extend(&locked.dependencies);
        }
    }
    unrequired(&lock, &reached, &mut lines);
    if !lines.is_empty() {
        return Err(disagree(&lines));
    }

    // The locked version of each package, through the manifests it has.
    let mut work = Work::new()?;
    let mut kept: BTreeMap<String, Package> = BTreeMap::new();
    let mut fetched: BTreeMap<String, Package> = BTreeMap::new();
    let mut queue = roots;
    while let Some(want) = queue.pop() {
        let name = want.name.clone();
        if kept.contains_key(&name) || fetched.contains_key(&name) {
            continue;
        }
        // A requirement the lock does not meet already has its line.
        let Some(locked) = lock.packages.get(&name) else {
            continue;
        };
        if locked.source != want.source.id() || !want.requirement.matches(&locked.version) {
            continue;
        }
        let installed = installed_checksum(root, &name, &lock)?;
        let package = match &want.source {
            Source::Path(_) => {
                // A local package is live: its files may change, its version
                // may not.
                let version = work.versions(&want.source)?.into_keys().next();
                if version != Some(locked.version) {
                    lines.push(format!(
                        "{name}: {} is version {}, {LOCK} has {} — run `blade update {name}`",
                        want.source.id(),
                        version.map_or("none".to_string(), |version| version.to_string()),
                        locked.version
                    ));
                    continue;
                }
                work.fetch_locked(&name, &want.source, locked)?
            }
            Source::Git(_) if installed.as_ref() == Some(&locked.checksum) => {
                let dir = root.join(PKG).join(&name);
                let manifest = Manifest::read_package(&dir)?;
                Package {
                    version: locked.version,
                    source: want.source.clone(),
                    commit: locked.commit.clone(),
                    dir,
                    manifest,
                    checksum: locked.checksum.clone(),
                }
            }
            Source::Git(_) => work.fetch_locked(&name, &want.source, locked)?,
        };
        let by = format!("{name} {}", locked.version);
        let wants = wants_of(&package.manifest, package.base(), &by)?;
        disagreements(&wants, &lock, &mut lines);
        queue.extend(wants);
        if installed.as_ref() == Some(&package.checksum) {
            kept.insert(name, package);
        } else {
            fetched.insert(name, package);
        }
    }
    let names: BTreeSet<String> = kept.keys().chain(fetched.keys()).cloned().collect();
    unrequired(&lock, &names, &mut lines);
    if !lines.is_empty() {
        return Err(disagree(&lines));
    }

    // What changes in pkg/, against what Blade installed there.
    let installed = installed_lock(root, &lock)?;
    let mut report = Vec::new();
    for (name, package) in &fetched {
        let version = package.version;
        report.push(match installed.packages.get(name) {
            _ if !root.join(PKG).join(name).is_dir() => format!("+ {name} {version}"),
            Some(before) if before.version != version => {
                format!("~ {name} {} -> {version}", before.version)
            }
            _ => format!("~ {name} {version} (files changed)"),
        });
    }
    let chosen: BTreeMap<String, Package> = kept.into_iter().chain(fetched).collect();
    for (name, before) in &installed.packages {
        if !chosen.contains_key(name) && root.join(PKG).join(name).is_dir() {
            report.push(format!("- {name} {}", before.version));
        }
    }
    let new_lock = Lock {
        packages: chosen
            .iter()
            .map(|(name, package)| (name.clone(), package.locked()))
            .collect(),
    };
    if !commit(root, &chosen, &lock, &new_lock, true, &mut work)? {
        return Ok(Report::default());
    }
    Ok(Report { lines: report })
}

/// Requirements that the lock does not meet, one line each.
fn disagreements(wants: &[Want], lock: &Lock, lines: &mut Vec<String>) {
    for want in wants {
        let (name, by, min) = (&want.name, &want.by, want.requirement.min());
        let line = match lock.packages.get(name) {
            None => format!("{name}: {by} wants ≥ {min}, {LOCK} has none"),
            Some(locked) if locked.source != want.source.id() => format!(
                "{name}: {by} wants it from {}, {LOCK} has {}",
                want.source.id(),
                locked.source
            ),
            Some(locked) if !want.requirement.matches(&locked.version) => {
                format!("{name}: {by} wants ≥ {min}, {LOCK} has {}", locked.version)
            }
            Some(_) => continue,
        };
        let line = format!("{line} — run `blade update {name}`");
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
}

/// Locked packages that nothing requires any more.
fn unrequired(lock: &Lock, required: &BTreeSet<String>, lines: &mut Vec<String>) {
    for name in lock
        .packages
        .keys()
        .filter(|name| !required.contains(*name))
    {
        let line = format!("{name}: no longer required — run `blade update`");
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
}

fn disagree(lines: &[String]) -> String {
    format!(
        "{LOCK} does not match {MANIFEST}:\n  {}",
        lines.join("\n  ")
    )
}

/// `blade remove NAME`: drop the dependency from `blade.toml`, then install.
/// A failure leaves `blade.toml` as it was.
pub fn remove(root: &Path, name: &str) -> Result<Report, String> {
    let _busy = Busy::take(root)?;
    let file = root.join(MANIFEST);
    let before = std::fs::read(&file).map_err(io(&file))?;
    if !manifest::remove_dependency(root, name)? {
        return Err(format!("\"{name}\" is not a dependency in {MANIFEST}"));
    }
    install_busy(root, &Unlock::Nothing).inspect_err(|_| {
        let _ = std::fs::write(&file, &before);
    })
}

/// `blade add NAME GIT [MIN]` (#351): declare `NAME = { git = GIT, version =
/// MIN }` in the project's `[dependencies]`, then install it with every other
/// package where it is. Without `MIN` the minimum is the newest `vX.Y.Z` tag
/// of `GIT`, so a new dependency starts current and the manifest says so.
/// Adding a name declared from the same URL moves its minimum, up or down;
/// from another source it is an error. A failure leaves `blade.toml` as it
/// was.
pub fn add(root: &Path, name: &str, git: &str, min: Option<&str>) -> Result<Report, String> {
    manifest::check_package_name(name)?;
    check_git_url(git)?;
    let source = Source::Git(git.to_string());
    let min = match min {
        Some(min) => Requirement::parse(min)?.min(),
        None => Work::new()?
            .versions(&source)?
            .into_keys()
            .next_back()
            .ok_or_else(|| format!("{} has no versions (no vX.Y.Z tags)", source.id()))?,
    };
    let _busy = Busy::take(root)?;
    if let Some(declared) = Manifest::read(root)?.dependencies.get(name)
        && declared.git.as_deref() != Some(git)
    {
        let from = declared
            .git
            .as_deref()
            .or(declared.path.as_deref())
            .unwrap_or("nowhere");
        return Err(format!(
            "\"{name}\" is already a dependency, from {from}; `blade remove {name}` first to take it from {git}"
        ));
    }
    let dependency = Dependency {
        version: Some(min.to_string()),
        git: Some(git.to_string()),
        path: None,
    };
    let file = root.join(MANIFEST);
    let before = std::fs::read(&file).map_err(io(&file))?;
    manifest::add_dependency(root, name, &dependency)?;
    install_busy(root, &Unlock::Packages(BTreeSet::from([name.to_string()]))).inspect_err(|_| {
        let _ = std::fs::write(&file, &before);
    })
}

/// `blade update [NAME...]`: resolve again without the locked versions of
/// `names`, the others staying where they are unless the new requirements
/// force them up; without names, nothing stays.
pub fn update(root: &Path, names: &[String]) -> Result<Report, String> {
    let _busy = Busy::take(root)?;
    let unlock = if names.is_empty() {
        Unlock::All
    } else {
        let (manifest, lock) = (Manifest::read(root)?, Lock::read(root)?);
        if let Some(name) = names.iter().find(|name| {
            !lock.packages.contains_key(*name) && !manifest.dependencies.contains_key(*name)
        }) {
            return Err(format!("\"{name}\" is not a dependency of this project"));
        }
        Unlock::Packages(names.iter().cloned().collect())
    };
    install_busy(root, &unlock)
}

/// `blade install SOURCE [VERSION]`, as before `blade add` (kept for one
/// release): add the package at `source`, a git repository or a local
/// directory, to `[dependencies]`, then install. Its name is the `[module]
/// name` of its `blade.toml`, or else the last part of the URL or directory.
pub fn add_source(
    root: &Path,
    source: &str,
    version: Option<&str>,
    git: bool,
) -> Result<(String, Report), String> {
    let mut work = Work::new()?;
    let (source, spelled) = if git || looks_like_git(source) {
        check_git_url(source)?;
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
    let name = match work.declared_name(&source, &picked)? {
        Some(name) => name,
        None => name_of(&source)?,
    };
    work.fetch(&name, &source, &picked, None)?;
    let dependency = Dependency {
        version: Some(version.map_or_else(|| picked.to_string(), str::to_string)),
        git: matches!(source, Source::Git(_)).then(|| spelled.clone()),
        path: matches!(source, Source::Path(_)).then_some(spelled),
    };
    // The manifest edit and the install are one operation of this blade.
    let _busy = Busy::take(root)?;
    let file = root.join(MANIFEST);
    let before = std::fs::read(&file).map_err(|error| format!("{}: {error}", file.display()))?;
    manifest::add_dependency(root, &name, &dependency)?;
    match install_busy(root, &Unlock::Packages(BTreeSet::from([name.clone()]))) {
        Ok(report) => Ok((name, report)),
        Err(error) => {
            // Неудачная установка не оставляет зависимость в манифесте.
            let _ = std::fs::write(&file, before);
            Err(error)
        }
    }
}

/// A git source as git will see it (#346): never an option, never a
/// transport helper (`ext::…`), no control characters. Which transports git
/// may use is also fixed in `git`.
fn check_git_url(url: &str) -> Result<(), String> {
    let helper = url
        .split_once("::")
        .is_some_and(|(transport, _)| !transport.contains('/'));
    if url.is_empty() || url.starts_with('-') || url.chars().any(char::is_control) || helper {
        return Err(format!(
            "invalid git source {url:?}: use an https://, ssh://, git:// or file:// URL, user@host:path, or a local path"
        ));
    }
    Ok(())
}

/// The name a package without `[module] name` is added under: the last part
/// of its URL or directory, without `.git`.
fn name_of(source: &Source) -> Result<String, String> {
    let spelled = match source {
        Source::Git(url) => url.trim_end_matches('/').to_string(),
        Source::Path(dir) => dir.display().to_string(),
    };
    let last = spelled.rsplit(['/', ':', '\\']).next().unwrap_or_default();
    let name = last.strip_suffix(".git").unwrap_or(last).to_string();
    manifest::check_package_name(&name).map_err(|error| {
        format!(
            "{}: cannot name the package \"{name}\" ({error}); give it [module] name in its {MANIFEST}",
            source.id()
        )
    })?;
    Ok(name)
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

/// A requirement as resolution meets it: the package, where it comes from,
/// its minimum, and the chain of package versions that asked for it.
#[derive(Clone)]
struct Want {
    name: String,
    source: Source,
    requirement: Requirement,
    by: String,
}

/// The requirements a manifest declares; `by` names its chain.
fn wants_of(declaring: &Manifest, base: Option<&Path>, by: &str) -> Result<Vec<Want>, String> {
    declaring
        .dependencies
        .iter()
        .map(|(name, dependency)| {
            let version = dependency.version.as_deref().ok_or_else(|| {
                format!("dependency \"{name}\" of {by} has no version: add version = \"MAJOR.MINOR.PATCH\", the lowest that works")
            })?;
            let requirement =
                Requirement::parse(version).map_err(|error| format!("dependency \"{name}\" of {by}: {error}"))?;
            let source = Source::of(name, dependency, base).map_err(|error| format!("{by}: {error}"))?;
            Ok(Want { name: name.clone(), source, requirement, by: by.to_string() })
        })
        .collect()
}

/// The lowest available version of `min`'s major that is at least `min`.
fn lowest(available: &BTreeMap<Version, Option<String>>, min: Version) -> Option<Version> {
    available
        .keys()
        .copied()
        .find(|version| version.major == min.major && *version >= min)
}

/// A package, its source and the major a requirement is in.
type Key = (String, String, u64);

fn key_of(want: &Want) -> Key {
    (
        want.name.clone(),
        want.source.id(),
        want.requirement.min().major,
    )
}

/// Minimum version selection (#347): one version per package, the lowest
/// that meets every minimum, in two passes that end on any input.
///
/// Pass 1 walks the requirement graph: from the project, every requirement
/// leads to the lowest version that meets that requirement, whose manifest
/// gives more. It keeps, per package, source and major, the largest minimum
/// seen. Every requirement is followed, not only the largest, so what is
/// visited does not depend on the order of the manifests; versions are
/// finite and none is expanded twice, so cycles end. A locked version of a
/// package that is not being updated counts as one more minimum.
///
/// Pass 2 walks again from the project, through the selected versions only
/// (the lowest that meets the largest minimum). The packages it reaches are
/// installed. A package reached with two majors or two sources is a
/// conflict, reported with both chains. A version that pass 1 visited but
/// pass 2 does not select (a superseded one) never causes an error: a bad
/// package or a missing minimum matters only on the way pass 2 takes.
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
    let roots = wants_of(manifest, Some(root), &project)?;

    // Pass 1.
    let mut mins: BTreeMap<Key, (Version, String)> = BTreeMap::new();
    let mut nodes: BTreeMap<(String, String, Version), Result<Package, String>> = BTreeMap::new();
    let mut queue = roots.clone();
    while let Some(want) = queue.pop() {
        let key = key_of(&want);
        let mut minimums = vec![(want.requirement.min(), want.by.clone())];
        if !mins.contains_key(&key) {
            minimums.extend(floor(lock, unlock, &want).map(|locked| (locked, LOCK.to_string())));
        }
        for (min, by) in minimums {
            let largest = mins.entry(key.clone()).or_insert((min, by.clone()));
            if largest.0 < min {
                *largest = (min, by);
            }
            // An unreachable source matters only if pass 2 goes there.
            let Ok(available) = work.versions(&want.source) else {
                break;
            };
            let Some(version) = lowest(&available, min) else {
                continue;
            };
            let node = (want.name.clone(), want.source.id(), version);
            if nodes.contains_key(&node) {
                continue;
            }
            let fetched = fetch_checked(work, &want, version, lock, unlock).and_then(|package| {
                let by = format!("{} → {} {version}", want.by, want.name);
                queue.extend(wants_of(&package.manifest, package.base(), &by)?);
                Ok(package)
            });
            nodes.insert(node, fetched);
        }
    }

    // Pass 2.
    let mut chosen: BTreeMap<String, (Want, Package)> = BTreeMap::new();
    let mut queue = roots;
    while let Some(want) = queue.pop() {
        let key = key_of(&want);
        if let Some((first, _)) = chosen.get(&want.name) {
            if first.source != want.source {
                return Err(format!(
                    "\"{}\" comes from two sources:\n  {} → {} ({})\n  {} → {} ({})",
                    want.name,
                    first.by,
                    want.name,
                    first.source.id(),
                    want.by,
                    want.name,
                    want.source.id()
                ));
            }
            if first.requirement.min().major != want.requirement.min().major {
                return Err(format!(
                    "no single major of \"{}\":\n  {} → {} {}\n  {} → {} {}",
                    want.name,
                    first.by,
                    want.name,
                    first.requirement,
                    want.by,
                    want.name,
                    want.requirement
                ));
            }
            continue;
        }
        let (min, by) = mins[&key].clone();
        let available = work.versions(&want.source)?;
        let version = lowest(&available, min).ok_or_else(|| {
            format!(
                "no version of \"{}\" is ≥ {min} within major {}: wanted by {by}; available: {}",
                want.name,
                min.major,
                list(available.keys())
            )
        })?;
        let package = nodes
            .get(&(want.name.clone(), want.source.id(), version))
            .cloned()
            .unwrap_or_else(|| fetch_checked(work, &want, version, lock, unlock))?;
        let by = format!("{} → {} {version}", want.by, want.name);
        queue.extend(wants_of(&package.manifest, package.base(), &by)?);
        chosen.insert(want.name.clone(), (want, package));
    }
    Ok(chosen
        .into_iter()
        .map(|(name, (_, package))| (name, package))
        .collect())
}

/// The locked version of a package that is not being updated, when it is
/// from the same source and major and meets the requirement: it stays.
fn floor(lock: &Lock, unlock: &Unlock, want: &Want) -> Option<Version> {
    let min = want.requirement.min();
    lock.packages
        .get(&want.name)
        .filter(|locked| locked.source == want.source.id() && !unlock.allows(&want.name))
        .map(|locked| locked.version)
        .filter(|version| version.major == min.major && *version >= min)
}

/// Fetch a version and check that it is the package the requirement names;
/// a version `blade.lock` records is checked against its commit and files.
fn fetch_checked(
    work: &mut Work,
    want: &Want,
    version: Version,
    lock: &Lock,
    unlock: &Unlock,
) -> Result<Package, String> {
    // `blade update` rewrites the lock entry instead of checking against it.
    let expected = lock.packages.get(&want.name).filter(|locked| {
        locked.source == want.source.id() && locked.version == version && !unlock.allows(&want.name)
    });
    work.fetch(&want.name, &want.source, &version, expected)
}

// ----- загрузка -----

/// Scratch space of one install: clones and fetched package versions.
struct Work {
    dir: PathBuf,
    clones: HashMap<String, PathBuf>,
    tags: HashMap<String, BTreeMap<Version, String>>,
    fetched: HashMap<(String, String, Version), Package>,
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

    /// The `[module] name` of a version, if it has `blade.toml` with one.
    fn declared_name(
        &mut self,
        source: &Source,
        version: &Version,
    ) -> Result<Option<String>, String> {
        let manifest = match source {
            Source::Path(dir) => Manifest::read_package(dir)?,
            Source::Git(url) => {
                let commit = self.versions(source)?[version].clone().unwrap_or_default();
                let clone = self.clone_of(url)?;
                let blob = format!("{commit}:{MANIFEST}");
                match git(&["-C", path_str(&clone)?, "show", &blob]) {
                    Ok(text) => Manifest::parse(&text).map_err(|error| {
                        format!("{} v{version}: {MANIFEST}: {error}", source.id())
                    })?,
                    Err(_) => Manifest::default(),
                }
            }
        };
        Ok(manifest.name)
    }

    /// Copy version `version` of package `name` from `source`, by its tag,
    /// into the scratch space and check it. `expected` — what `blade.lock`
    /// says this version is.
    fn fetch(
        &mut self,
        name: &str,
        source: &Source,
        version: &Version,
        expected: Option<&Locked>,
    ) -> Result<Package, String> {
        let commit = match source {
            Source::Path(_) => None,
            Source::Git(url) => {
                let commit = self.versions(source)?[version].clone().unwrap_or_default();
                if let Some(locked) = expected.and_then(|locked| locked.commit.as_ref())
                    && *locked != commit
                {
                    return Err(format!(
                        "tag v{version} of {url} now points at {commit}, but {LOCK} has {locked}; \
                             the tag was moved — check the package, then run `blade update`"
                    ));
                }
                Some(commit)
            }
        };
        self.build(name, source, version, commit, expected)
    }

    /// The version `blade.lock` records, by its commit and never by its tag
    /// (#349): no tags are listed, and the files must have the locked
    /// checksum.
    fn fetch_locked(
        &mut self,
        name: &str,
        source: &Source,
        locked: &Locked,
    ) -> Result<Package, String> {
        let version = locked.version;
        let commit = match source {
            Source::Path(_) => None,
            Source::Git(url) => {
                let commit = locked.commit.clone().unwrap_or_default();
                check_commit(&commit)
                    .map_err(|error| format!("{name} {version}: {LOCK}: {error}"))?;
                let clone = self.clone_of(url)?;
                let clone = path_str(&clone)?;
                let object = format!("{commit}^{{commit}}");
                let present = || git(&["-C", clone, "cat-file", "-e", &object]).is_ok();
                if !present() {
                    // A server may still give a commit that no ref reaches.
                    let _ = git(&["-C", clone, "fetch", "--quiet", "origin", &commit]);
                }
                if !present() {
                    return Err(format!(
                        "{name} {version}: the locked commit {commit} is not in {url} any more; \
                         run `blade update {name}` to choose a version again"
                    ));
                }
                Some(commit)
            }
        };
        self.build(name, source, &version, commit, Some(locked))
    }

    /// Copy a version of package `name` into the scratch space, from its
    /// directory or its git `commit`, and check it.
    fn build(
        &mut self,
        name: &str,
        source: &Source,
        version: &Version,
        commit: Option<String>,
        expected: Option<&Locked>,
    ) -> Result<Package, String> {
        let key = (name.to_string(), source.id(), *version);
        if let Some(package) = self.fetched.get(&key) {
            return Ok(package.clone());
        }
        // Every refusal names the package and its tag.
        let label = match source {
            Source::Git(_) => format!("{name} v{version} ({})", source.id()),
            Source::Path(_) => format!("{name} {version} ({})", source.id()),
        };
        self.count += 1;
        let dir = self.dir.join(format!("package-{}", self.count));
        match source {
            Source::Path(origin) => {
                copy_package(origin, &dir).map_err(|error| format!("{label}: {error}"))?;
            }
            Source::Git(url) => {
                let commit = commit.as_deref().unwrap_or_default();
                check_commit(commit).map_err(|error| format!("{label}: {error}"))?;
                let clone = self.clone_of(url)?;
                git(&[
                    "-C",
                    path_str(&clone)?,
                    "checkout",
                    "--quiet",
                    "--force",
                    commit,
                ])?;
                copy_package(&clone, &dir).map_err(|error| format!("{label}: {error}"))?;
            }
        }
        let manifest = Manifest::read_package(&dir).map_err(|error| format!("{label}: {error}"))?;
        if let Some(declared) = &manifest.name
            && declared != name
        {
            return Err(format!(
                "{label}: {MANIFEST} declares [module] name \"{declared}\"; the name in [dependencies] must match"
            ));
        }
        if let Some(declared) = &manifest.version
            && Version::parse(declared) != Some(*version)
        {
            return Err(format!(
                "{label}: {MANIFEST} declares [module] version {declared}, but the tag is v{version}"
            ));
        }
        check_package(&dir, name).map_err(|error| format!("{label}: {error}"))?;
        // Without it the compiler would take the files for the project's.
        let generated = dir.join(MANIFEST);
        if !generated.exists() {
            std::fs::write(&generated, generated_manifest(name, version))
                .map_err(io(&generated))?;
        }
        let checksum = checksum(&dir)?;
        if let (Some(locked), Source::Git(_)) = (expected, source)
            && !locked.checksum.is_empty()
            && locked.checksum != checksum
        {
            return Err(format!(
                "{name} {version}: the files differ from {LOCK} ({checksum} != {})",
                locked.checksum
            ));
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
        git(&[
            "clone",
            "--quiet",
            "--no-checkout",
            "--",
            url,
            path_str(&clone)?,
        ])?;
        self.clones.insert(url.to_string(), clone.clone());
        Ok(clone)
    }
}

impl Drop for Work {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A commit id as `blade.lock` may hold it: hex, never an option for git.
fn check_commit(commit: &str) -> Result<(), String> {
    if commit.len() >= 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!("{commit:?} is not a commit id"))
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
        // The transports a package may come by (#346); `ext::` and the rest
        // could run commands.
        .env("GIT_ALLOW_PROTOCOL", "file:git:https:ssh")
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
    for line in git(&["ls-remote", "--tags", "--", url])?.lines() {
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
        if !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        if tag.starts_with('v')
            && let Some(version) = Version::parse(tag)
        {
            target.insert(version, commit.to_string());
        }
    }
    tags.extend(peeled);
    Ok(tags)
}

// ----- проверка и копирование -----

/// The directory of a package's modules.
const SRC: &str = "src";

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

/// The `blade.toml` installed with a package that has none: the same bytes
/// for the same name and version, since they are part of the checksum.
fn generated_manifest(name: &str, version: &Version) -> String {
    format!("[module]\nname = \"{name}\"\nversion = \"{version}\"\n")
}

/// The rules a package must follow to be installed. Only its modules in
/// `src/` are read; the rest (a licence, examples, tests) is copied as is.
fn check_package(dir: &Path, name: &str) -> Result<(), String> {
    if !dir.join(SRC).is_dir() {
        return Err(format!(
            "no {SRC}/ directory; a package keeps its modules in {SRC}/"
        ));
    }
    let all = files(dir)?;
    // macOS и Windows не различают регистр: такие два файла станут одним.
    let mut folded: HashMap<String, &String> = HashMap::new();
    for file in &all {
        if let Some(other) = folded.insert(file.to_lowercase(), file) {
            return Err(format!("{other} and {file} differ only in letter case"));
        }
    }
    let modules = all
        .iter()
        .filter(|file| file.starts_with(&format!("{SRC}/")) && file.ends_with(".ok"));
    for file in modules {
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
                "{file} declares module version {}; a package's version is its tag",
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
    checksum_of(dir, &files(dir)?)
}

/// The checksum of `pkg/<name>` as it is installed, without the packages
/// nested in it (`acme/web` in `acme`); `None` when it is not there.
fn installed_checksum(root: &Path, name: &str, lock: &Lock) -> Result<Option<String>, String> {
    let dir = root.join(PKG).join(name);
    if !dir.is_dir() {
        return Ok(None);
    }
    let nested = nested_in(name, lock);
    let own: Vec<String> = files(&dir)?
        .into_iter()
        .filter(|file| {
            !nested
                .iter()
                .any(|inner| file.starts_with(&format!("{inner}/")))
        })
        .collect();
    checksum_of(&dir, &own).map(Some)
}

/// The locked packages inside `pkg/<name>`, relative to it.
fn nested_in(name: &str, lock: &Lock) -> Vec<String> {
    let prefix = format!("{name}/");
    lock.packages
        .keys()
        .filter_map(|other| other.strip_prefix(&prefix).map(str::to_string))
        .collect()
}

fn checksum_of(dir: &Path, files: &[String]) -> Result<String, String> {
    let mut hasher = Sha256::new();
    for file in files {
        let bytes = std::fs::read(dir.join(file)).map_err(|error| format!("{file}: {error}"))?;
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

// ----- фиксация pkg/ и blade.lock -----
//
// pkg/ and blade.lock change together: the entries of pkg/ that change and
// the lock are prepared next to the old ones (`pkg.blade-new`,
// `blade.lock.blade-new`), then a marker (`pkg.blade-commit`) records that
// the new state is complete and which entries it replaces or removes, then
// the renames happen: the old entries into `pkg.blade-old`, the new ones in.
// A blade that finds the marker finishes the renames; without it the prepared
// files are discarded and the old state stays. Entries that do not change are
// not touched (#350).

const STAGED: &str = "pkg.blade-new";
/// Blade's record of what it installed, in `pkg/`: the lock as it was when
/// `pkg/` was last committed. A pulled `blade.lock` may say something else;
/// local changes are what differs from this record (#350).
const INSTALLED: &str = ".blade-installed";
const OLD: &str = "pkg.blade-old";
const STAGED_LOCK: &str = "blade.lock.blade-new";
const COMMIT: &str = "pkg.blade-commit";

fn io(path: &Path) -> impl Fn(std::io::Error) -> String + use<> {
    let path = path.display().to_string();
    move |error: std::io::Error| format!("{path}: {error}")
}

thread_local! {
    static FAIL_AT: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
}

/// Tests only: stop the next commit after `step` as if the process died
/// there (`staged`, `lock`, `marker`, `pkg-moved`, `pkg-in`, `lock-in`).
#[doc(hidden)]
pub fn fail_after(step: Option<&'static str>) {
    FAIL_AT.with(|cell| cell.set(step));
}

fn step(name: &'static str) -> Result<(), String> {
    if FAIL_AT.with(|cell| cell.get()) == Some(name) {
        FAIL_AT.with(|cell| cell.set(None));
        return Err(format!("interrupted after {name}"));
    }
    Ok(())
}

/// Write `text` to `file` and sync it to disk before the next step.
fn write_synced(file: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    let mut handle = std::fs::File::create(file).map_err(io(file))?;
    handle
        .write_all(text.as_bytes())
        .and_then(|()| handle.sync_all())
        .map_err(io(file))
}

/// What a commit does to `pkg/`, by its top-level entries (`acme` holds
/// `acme` and `acme/web`): the entries rebuilt in the staging directory and
/// the entries removed. No other entry is touched.
#[derive(Default)]
struct Plan {
    replace: Vec<String>,
    remove: Vec<String>,
}

impl Plan {
    fn is_empty(&self) -> bool {
        self.replace.is_empty() && self.remove.is_empty()
    }

    /// The marker's text; `v2` tells it from the whole-`pkg/` swap of 0.5.1.
    fn text(&self) -> String {
        let mut text = String::from("v2\n");
        for entry in &self.replace {
            text.push_str(&format!("replace {entry}\n"));
        }
        for entry in &self.remove {
            text.push_str(&format!("remove {entry}\n"));
        }
        text
    }
}

/// The top-level entry of `pkg/` a package lives in.
fn top(name: &str) -> &str {
    name.split('/').next().unwrap_or(name)
}

/// What `pkg/` holds by Blade's record of it, or by `blade.lock` when there
/// is no record (a `pkg/` of Blade 0.5.1).
fn installed_lock(root: &Path, lock: &Lock) -> Result<Lock, String> {
    let file = root.join(PKG).join(INSTALLED);
    if !file.is_file() {
        return Ok(lock.clone());
    }
    let text = std::fs::read_to_string(&file).map_err(io(&file))?;
    Lock::parse(&text).map_err(|error| format!("{}: {error}", file.display()))
}

/// What going from what is installed to `new` does to `pkg/`, refusing to
/// lose the user's work (#350): an entry of `pkg/` that Blade did not
/// install, or a package with local changes that would be replaced or
/// removed, stops the operation before anything changes. A `strict` commit
/// (`blade sync`) also replaces a package whose files differ from what was
/// installed when its entry stays.
fn plan(
    root: &Path,
    old: &Lock,
    new: &Lock,
    strict: bool,
    work: &mut Work,
) -> Result<Plan, String> {
    let installed = installed_lock(root, old)?;
    let mut known = installed.clone();
    for (name, locked) in &old.packages {
        known
            .packages
            .entry(name.clone())
            .or_insert_with(|| locked.clone());
    }
    let (old, known) = (&installed, &known);
    let unknown = unknown_entries(root, known)?;
    if !unknown.is_empty() {
        let list: Vec<String> = unknown
            .iter()
            .map(|entry| format!("{PKG}/{entry}"))
            .collect();
        return Err(format!(
            "{} {} not in {LOCK}, and Blade deletes only what it installed;\n\
             move {} out of {PKG}/, then run the command again",
            list.join(", "),
            if list.len() == 1 { "is" } else { "are" },
            if list.len() == 1 { "it" } else { "them" }
        ));
    }
    let entries: BTreeSet<&str> = old
        .packages
        .keys()
        .chain(new.packages.keys())
        .map(|name| top(name))
        .collect();
    let mut plan = Plan::default();
    let mut refusals = Vec::new();
    for entry in entries {
        let of = |lock: &Lock| -> Vec<(String, Locked)> {
            lock.packages
                .iter()
                .filter(|(name, _)| top(name) == entry)
                .map(|(name, locked)| (name.clone(), locked.clone()))
                .collect()
        };
        let (before, after) = (of(old), of(new));
        let mut changed = Vec::new();
        for (name, locked) in known.packages.iter().filter(|(name, _)| top(name) == entry) {
            if installed_checksum(root, name, known)?.is_some_and(|sum| sum != locked.checksum) {
                changed.push((name, locked));
            }
        }
        let missing = after
            .iter()
            .any(|(name, _)| !root.join(PKG).join(name).is_dir());
        if before == after && !missing && !(strict && !changed.is_empty()) {
            continue;
        }
        for (name, locked) in changed {
            refusals.push(local_changes(root, name, locked, known, work));
        }
        if after.is_empty() {
            plan.remove.push(entry.to_string());
        } else {
            plan.replace.push(entry.to_string());
        }
    }
    if !refusals.is_empty() {
        return Err(refusals.join("\n"));
    }
    Ok(plan)
}

/// Entries of `pkg/` that hold no package of `lock`, as paths inside `pkg/`.
fn unknown_entries(root: &Path, lock: &Lock) -> Result<Vec<String>, String> {
    let mut found = Vec::new();
    let mut dirs = vec![String::new()];
    while let Some(dir) = dirs.pop() {
        let path = root.join(PKG).join(&dir);
        if !path.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&path).map_err(io(&path))? {
            let entry = entry.map_err(io(&path))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let inner = if dir.is_empty() {
                name
            } else {
                format!("{dir}/{name}")
            };
            let holds = lock
                .packages
                .keys()
                .any(|locked| locked.starts_with(&format!("{inner}/")));
            if lock.packages.contains_key(&inner) || (dir.is_empty() && inner == INSTALLED) {
                continue;
            } else if holds && entry.path().is_dir() {
                dirs.push(inner);
            } else {
                found.push(inner);
            }
        }
    }
    found.sort();
    Ok(found)
}

/// The refusal for a package whose files in `pkg/` are not the locked ones,
/// with the first file that differs when the locked commit can be fetched.
fn local_changes(root: &Path, name: &str, locked: &Locked, lock: &Lock, work: &mut Work) -> String {
    let detail = Source::from_id(&locked.source)
        .filter(|source| matches!(source, Source::Git(_)))
        .and_then(|source| work.fetch_locked(name, &source, locked).ok())
        .and_then(|original| {
            first_difference(
                &root.join(PKG).join(name),
                &original.dir,
                &nested_in(name, lock),
            )
        })
        .map(|detail| format!(" ({detail})"))
        .unwrap_or_default();
    format!(
        "{PKG}/{name} has local changes{detail};\n\
         move them away or delete {PKG}/{name}, then run the command again"
    )
}

/// The first file, in order, that differs between an installed package and
/// the locked original, apart from the packages nested in it.
fn first_difference(installed: &Path, original: &Path, nested: &[String]) -> Option<String> {
    let own = |dir: &Path| -> Option<BTreeSet<String>> {
        Some(
            files(dir)
                .ok()?
                .into_iter()
                .filter(|file| {
                    !nested
                        .iter()
                        .any(|inner| file.starts_with(&format!("{inner}/")))
                })
                .collect(),
        )
    };
    let (now, then) = (own(installed)?, own(original)?);
    now.union(&then)
        .find_map(|file| match (now.contains(file), then.contains(file)) {
            (true, false) => Some(format!("{file} was added")),
            (false, true) => Some(format!("{file} was removed")),
            _ => {
                let differs = std::fs::read(installed.join(file)).ok()?
                    != std::fs::read(original.join(file)).ok()?;
                differs.then(|| format!("{file} differs from {LOCK}"))
            }
        })
}

/// Commit `new`, whose packages are `chosen`, over `old`: the entries of
/// `pkg/` that change are rebuilt beside it and moved in; the others keep
/// their files, inodes and times (#350). False when nothing was to be done.
fn commit(
    root: &Path,
    chosen: &BTreeMap<String, Package>,
    old: &Lock,
    new: &Lock,
    strict: bool,
    work: &mut Work,
) -> Result<bool, String> {
    let mut plan = plan(root, old, new, strict, work)?;
    let record = root.join(PKG).join(INSTALLED);
    let recorded = std::fs::read_to_string(&record).ok();
    let settled = recorded.as_deref() == Some(new.text().as_str())
        || (recorded.is_none() && !root.join(PKG).exists() && new.packages.is_empty());
    if plan.is_empty() && old == new && root.join(LOCK).exists() && settled {
        return Ok(false);
    }
    let staged = root.join(STAGED);
    for dir in [&staged, &root.join(OLD)] {
        if dir.exists() {
            std::fs::remove_dir_all(dir).map_err(io(dir))?;
        }
    }
    std::fs::create_dir_all(&staged).map_err(io(&staged))?;
    crate::manifest::check_distinct(chosen.keys().map(String::as_str))?;
    // Shorter names first: a nested package (`acme/web`) goes into the
    // directory of its parent (`acme`) and must not land on its files.
    let mut names: Vec<&String> = chosen
        .keys()
        .filter(|name| plan.replace.iter().any(|entry| entry == top(name)))
        .collect();
    names.sort_by_key(|name| name.len());
    let pkg = root.join(PKG);
    for name in names {
        crate::manifest::check_package_name(name)?;
        let target = staged.join(name);
        if !target.starts_with(&staged) || target.exists() {
            return Err(format!(
                "package \"{name}\" would overwrite files of another package in {PKG}/"
            ));
        }
        let package = &chosen[name];
        copy_tree(&package.dir, &target, false)?;
        // A package kept from pkg/ comes without the packages nested in it.
        if package.dir.starts_with(&pkg) {
            for inner in nested_in(name, new) {
                let inside = target.join(inner);
                if inside.exists() {
                    std::fs::remove_dir_all(&inside).map_err(io(&inside))?;
                }
            }
        }
    }
    write_synced(&staged.join(INSTALLED), &new.text())?;
    plan.replace.push(INSTALLED.to_string());
    step("staged")?;
    write_synced(&root.join(STAGED_LOCK), &new.text())?;
    step("lock")?;
    write_synced(&root.join(COMMIT), &plan.text())?;
    step("marker")?;
    finish_commit(root)?;
    Ok(true)
}

/// The renames after the marker; each is skipped when already done, so a
/// blade interrupted anywhere here is finished by the next one.
fn finish_commit(root: &Path) -> Result<(), String> {
    let (staged, old, pkg) = (root.join(STAGED), root.join(OLD), root.join(PKG));
    let marker = root.join(COMMIT);
    let text = std::fs::read_to_string(&marker).map_err(io(&marker))?;
    if let Some(lines) = text.strip_prefix("v2\n") {
        let entries: Vec<(&str, &str)> = lines
            .lines()
            .filter_map(|line| line.split_once(' '))
            .collect();
        std::fs::create_dir_all(&old).map_err(io(&old))?;
        // Out first: what is replaced or removed goes into pkg.blade-old.
        for (action, entry) in &entries {
            let (current, gone) = (pkg.join(entry), old.join(entry));
            let pending = *action == "remove" || staged.join(entry).exists();
            if pending && current.exists() && !gone.exists() {
                std::fs::rename(&current, &gone).map_err(io(&current))?;
            }
        }
        step("pkg-moved")?;
        for (action, entry) in &entries {
            let new = staged.join(entry);
            if *action == "replace" && new.exists() {
                std::fs::create_dir_all(&pkg).map_err(io(&pkg))?;
                std::fs::rename(&new, pkg.join(entry)).map_err(io(&new))?;
            }
        }
        if staged.exists() {
            std::fs::remove_dir_all(&staged).map_err(io(&staged))?;
        }
    } else if staged.exists() {
        // A commit of Blade 0.5.1 swaps the whole of pkg/.
        if pkg.exists() {
            if old.exists() {
                std::fs::remove_dir_all(&old).map_err(io(&old))?;
            }
            std::fs::rename(&pkg, &old).map_err(io(&pkg))?;
        }
        std::fs::rename(&staged, &pkg).map_err(io(&staged))?;
    }
    step("pkg-in")?;
    let staged_lock = root.join(STAGED_LOCK);
    if staged_lock.exists() {
        std::fs::rename(&staged_lock, root.join(LOCK)).map_err(io(&staged_lock))?;
    }
    step("lock-in")?;
    std::fs::remove_file(&marker).map_err(io(&marker))?;
    if old.exists() {
        std::fs::remove_dir_all(&old).map_err(io(&old))?;
    }
    Ok(())
}

/// Bring an interrupted commit to one state: forward when its marker is
/// there, back to the old pkg/ and lock otherwise. `install` does it first;
/// the caller holds the project (or nothing else runs on it).
pub fn recover(root: &Path) -> Result<(), String> {
    if root.join(COMMIT).exists() {
        return finish_commit(root);
    }
    let (staged, old, pkg) = (root.join(STAGED), root.join(OLD), root.join(PKG));
    if staged.exists() {
        std::fs::remove_dir_all(&staged).map_err(io(&staged))?;
    }
    let staged_lock = root.join(STAGED_LOCK);
    if staged_lock.exists() {
        std::fs::remove_file(&staged_lock).map_err(io(&staged_lock))?;
    }
    if old.exists() {
        if pkg.exists() {
            std::fs::remove_dir_all(&old).map_err(io(&old))?;
        } else {
            std::fs::rename(&old, &pkg).map_err(io(&old))?;
        }
    }
    Ok(())
}

/// The installed state is consistent (no interrupted commit) — for commands
/// that read pkg/ and blade.lock without changing them (`blade build`).
pub fn settled(root: &Path) -> Result<(), String> {
    for name in [COMMIT, STAGED, STAGED_LOCK, OLD] {
        if root.join(name).exists() {
            return Err(format!(
                "an earlier blade was interrupted ({name} exists); run `blade sync` to finish it"
            ));
        }
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
                    "~ {name} {} (tag moved: {} -> {})",
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
